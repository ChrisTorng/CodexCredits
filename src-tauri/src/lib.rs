mod api;
mod schedule;
mod storage;
mod credentials;
mod oauth;
mod engine;
#[cfg(target_os="android")] mod android_bridge;
#[cfg(target_os="android")] mod legacy;

use std::{sync::{Arc,Mutex},time::Duration};
use serde::Serialize;
use tauri_plugin_notification::NotificationExt;
use tauri::{AppHandle,Emitter,Manager,WindowEvent};
#[cfg(desktop)] use tauri::menu::{Menu,MenuItem};
#[cfg(desktop)] use tauri::tray::TrayIconBuilder;

struct State{
 app:AppHandle,
 db:Mutex<rusqlite::Connection>,
 settings:Mutex<schedule::Settings>,
 usage:Mutex<Option<api::Usage>>,
 credits:Mutex<Option<api::Credits>>,
 global:Mutex<Option<api::Global>>,
 last_credits:Mutex<i64>,
 last_global:Mutex<i64>,
 ntfy_status:Mutex<String>,
 armed:Mutex<Option<String>>,
 engine:Mutex<engine::Engine>,
 error:Mutex<Option<String>>,
 updating:tokio::sync::Mutex<()>,
 action:tokio::sync::Mutex<()>,
 login_busy:tokio::sync::Mutex<()>,
}
#[derive(Serialize)]
struct LocalState{
 snapshot:Snapshot,
 history:Vec<serde_json::Value>,
 events:Vec<serde_json::Value>,
}
#[derive(Serialize)]
struct Snapshot{
 settings:schedule::Settings,
 usage:Option<api::Usage>,
 credits:Option<api::Credits>,
 global:Option<api::Global>,
 ntfy_status:String,
 armed:bool,
 next:Option<engine::Next>,
 error:Option<String>,
 signed_in:bool,
}
fn now()->i64{chrono::Utc::now().timestamp()}
fn token()->Result<String,String>{credentials::load()}
#[cfg(target_os="android")]
fn native_next()->Option<engine::Next>{
 let platform=android_bridge::platform_status().ok()?;
 let at=platform["next_alarm_ms"].as_i64().unwrap_or(0)/1000;
 if at<=0{return None;}
 let kind=match platform["next_event_kind"].as_str().unwrap_or("auto"){
  "anchor"=>engine::Kind::Anchor,
  "anchor_retry"=>engine::Kind::AnchorRetry,
  "chain"=>engine::Kind::Chain,
  _=>engine::Kind::Auto
 };
 Some(engine::Next{
  at,kind,auto_count:platform["next_event_auto_count"].as_u64().unwrap_or(0).min(8) as u8,
  reason:platform["next_alarm_reason"].as_str().unwrap_or("Android 排程").into()
 })
}
fn snapshot(s:&State)->Snapshot{
 Snapshot{
  settings:s.settings.lock().unwrap().clone(),
  usage:s.usage.lock().unwrap().clone(),
  credits:s.credits.lock().unwrap().clone(),
  global:s.global.lock().unwrap().clone(),
  ntfy_status:s.ntfy_status.lock().unwrap().clone(),
  armed:{
    #[cfg(target_os="android")]
    {android_bridge::banked_armed().unwrap_or_else(|_|s.armed.lock().unwrap().is_some())}
    #[cfg(not(target_os="android"))]
    {s.armed.lock().unwrap().is_some()}
   },
  next:{
    #[cfg(target_os="android")]
    {native_next()}
    #[cfg(not(target_os="android"))]
    {s.engine.lock().unwrap().next.clone()}
   },
  error:s.error.lock().unwrap().clone(),
  signed_in:token().is_ok(),
 }
}
fn record(s:&State,kind:&str,message:&str){
 let _=storage::event(&s.db.lock().unwrap(),kind,message);
}
fn replan(s:&State){
 let settings=s.settings.lock().unwrap().clone();
 let usage=s.usage.lock().unwrap().clone();
 let mut e=s.engine.lock().unwrap();
 e.next=engine::next_event(&settings,usage.as_ref(),&e,now());
}
fn confirmed_primary(new:&api::Usage,prev:Option<&api::Usage>)->bool{
 let new_used=new.five_hour.unwrap_or(0.);
 if new_used>0.0001{return true;}
 let Some(old)=prev else{return false};
 let fresh=engine::reset_timestamp(new.five_reset.as_ref());
 let previous=engine::reset_timestamp(old.five_reset.as_ref());
 let timestamp=now();
 let old_confirmed=old.primary_confirmed && previous.is_some_and(|r|r>timestamp)
  && fresh.zip(previous).is_some_and(|(a,b)|(a-b).abs()<=120);
 let elapsed=chrono::DateTime::parse_from_rfc3339(&old.checked_at)
  .ok().map(|t|timestamp-t.timestamp()).unwrap_or(0).max(0);
 let fixed=elapsed>=15&&fresh.zip(previous)
  .is_some_and(|(a,b)|(a-b).abs()<=std::cmp::max(5,elapsed/4));
 let natural_weekly=engine::reset_timestamp(old.weekly_reset.as_ref())
  .is_some_and(|r|timestamp>=r-900&&timestamp<=r+3600);
 let unexpected_weekly=old.weekly.zip(new.weekly).is_some_and(|(a,b)|a-b>2.)&&!natural_weekly;
 !unexpected_weekly && (old_confirmed||fixed)
}
async fn refresh_internal(s:&State,force_aux:bool)->Result<api::Usage,String>{
 let _guard=s.updating.lock().await;
 let access=oauth::valid_access().await?;
 let mut value=api::fetch_usage(&access.token,access.account_id.as_deref()).await?;
 let before=s.usage.lock().unwrap().clone();
 value.primary_confirmed=confirmed_primary(&value,before.as_ref());
 {
  let db=s.db.lock().unwrap();
  storage::add_sample(&db,value.five_hour,value.weekly,value.five_reset.as_deref(),value.weekly_reset.as_deref())?;
  let _=storage::cleanup(&db,s.settings.lock().unwrap().retention_days);
 }
 *s.usage.lock().unwrap()=Some(value.clone());
 *s.error.lock().unwrap()=None;
 replan(s);
 let check_credits={let last=s.last_credits.lock().unwrap();force_aux||now()-*last>=1800};
 if check_credits{
  match api::fetch_credits(&access.token,access.account_id.as_deref()).await{
   Ok(c)=>{
    let selected={s.armed.lock().unwrap().clone()};
    if let Some(selected)=selected{
     if !c.available.iter().any(|x|x.id==selected){
      *s.armed.lock().unwrap()=None;
      let _=storage::set_meta(&s.db.lock().unwrap(),"banked_armed",None);
      record(s,"banked_reset","待命的 Banked Reset 已不可用，取消待命");
     }
    }
    *s.credits.lock().unwrap()=Some(c);
    *s.last_credits.lock().unwrap()=now();
   }
   Err(e)=>record(s,"warning",&format!("Banked Reset 查詢失敗: {e}")),
  }
 }
 let check_global={let last=s.last_global.lock().unwrap();force_aux||now()-*last>=21600};
 if check_global{
  match api::global_reset().await{
   Ok(value)=>{*s.global.lock().unwrap()=Some(value);*s.last_global.lock().unwrap()=now();}
   Err(_)=>{*s.last_global.lock().unwrap()=now();}
  }
 }
 Ok(value)
}
async fn notify(s:&State,title:&str,message:&str){
 let settings=s.settings.lock().unwrap().clone();
 if settings.native_notify_enabled{
  if let Err(e)=s.app.notification().builder().title(title).body(message).show(){
   record(s,"warning",&format!("Windows 通知發送失敗: {e}"));
  }
 }
 let url=settings.ntfy_topic;
 if !url.is_empty(){
  let result=api::ntfy_send(&url,title,message).await;
  *s.ntfy_status.lock().unwrap()=match result{
   Ok(_)=>format!("已發送 · {title}"),
   Err(e)=>format!("發送失敗: {e}")
  };
 }
}
async fn trigger(s:&State,followups:u8,kind:&str)->Result<(),String>{
 let _action=s.action.lock().await;
 let q=refresh_internal(s,false).await?;
 engine::can_trigger(&q,now())?;
 let access=oauth::valid_access().await?;
 api::trigger_minimal(&access.token,access.account_id.as_deref()).await?;
 tokio::time::sleep(Duration::from_secs(2)).await;
 let mut after=refresh_internal(s,false).await?;
 // An accepted trigger explicitly confirms the current 5-hour window.
 after.primary_confirmed=after.five_reset.is_some();
 *s.usage.lock().unwrap()=Some(after.clone());
 record(s,"trigger",&format!("{kind}觸發成功"));
 if followups>0&&after.primary_confirmed{
  let reset=engine::reset_timestamp(after.five_reset.as_ref());
  let mut e=s.engine.lock().unwrap();
  e.chain_remaining=followups;
  e.chain_reset=reset;
 }
 notify(s,"Codex 觸發成功",&format!("5h 剩餘 {:.0}%",100.-after.five_hour.unwrap_or(0.))).await;
 replan(s);Ok(())
}
async fn consume(s:&State,allow_partial:bool,selected:Option<String>)->Result<(),String>{
 let _action=s.action.lock().await;
 let current=refresh_internal(s,true).await?;
 let used=engine::exhausted(&current)||current.five_hour.is_some_and(|v|v>0.)
  ||current.weekly.is_some_and(|v|v>0.);
 if !(if allow_partial{used}else{engine::exhausted(&current)}){
  return Err((if allow_partial{"配額尚未使用，不需要重置"}else{"5h 與週配額尚未用盡"}).into());
 }
 let access=oauth::valid_access().await?;
 let c=api::fetch_credits(&access.token,access.account_id.as_deref()).await?;
 let credit=c.available.iter().find(|c|selected.as_ref().is_none_or(|id|id==&c.id))
  .ok_or("目前沒有可用的 Banked Reset")?;
 if c.count<=0{return Err("Banked Reset 可用次數為 0".into())}
 let windows=api::consume_credit(&access.token,access.account_id.as_deref(),&credit.id).await?;
 if windows<=0{return Err("伺服器未重置任何配額視窗".into())}
 *s.armed.lock().unwrap()=None;
 storage::set_meta(&s.db.lock().unwrap(),"banked_armed",None)?;
 tokio::time::sleep(Duration::from_secs(1)).await;
 let _=refresh_internal(s,true).await;
 record(s,"banked_reset",&format!("Banked Reset 已使用 · 重置 {windows} 個視窗"));
 notify(s,"Codex Banked Reset 完成",&format!("已重置 {windows} 個配額視窗")).await;
 replan(s);Ok(())
}
#[tauri::command]fn get_snapshot(s:tauri::State<'_,Arc<State>>)->Snapshot{snapshot(&s)}
#[tauri::command]fn save_settings(s:tauri::State<'_,Arc<State>>,settings:schedule::Settings)->Result<Snapshot,String>{
 if !settings.valid(){return Err("設定不正確：請檢查排程、ntfy 網址與採樣間隔".into())}
 if settings.mode!=schedule::Mode::Off&&token().is_err(){return Err("請先登入才能啟用自動排程".into())}
 {
  let db=s.db.lock().unwrap();storage::save_settings(&db,&settings)?;
 }
 #[cfg(target_os="android")]
 android_bridge::sync_settings(&serde_json::to_string(&settings).map_err(|e|e.to_string())?)?;
 let changed_mode=s.settings.lock().unwrap().mode!=settings.mode;
 *s.settings.lock().unwrap()=settings;
 if changed_mode{
  let mut e=s.engine.lock().unwrap();e.chain_remaining=0;e.chain_reset=None;
  record(&s,"mode","排程模式已變更");
 }
 replan(&s);Ok(snapshot(&s))
}
#[tauri::command]async fn login_with_chatgpt(app:AppHandle,s:tauri::State<'_,Arc<State>>)->Result<Snapshot,String>{
 let _busy=s.login_busy.try_lock().map_err(|_|"登入程序已進行中")?;
 let session=oauth::browser_login().await?;
 {let _guard=s.updating.lock().await;oauth::save_session(&session)?;}
 *s.error.lock().unwrap()=None;
 record(&s,"login","ChatGPT 登入成功");
 let _=app.emit("credits:updated",());Ok(snapshot(&s))
}
#[tauri::command]async fn sign_out(s:tauri::State<'_,Arc<State>>)->Result<Snapshot,String>{
 let _guard=s.updating.lock().await;
 credentials::sign_out()?;
 #[cfg(target_os="android")]
 android_bridge::clear_auth()?;
 *s.usage.lock().unwrap()=None;*s.credits.lock().unwrap()=None;
 *s.armed.lock().unwrap()=None;
 storage::set_meta(&s.db.lock().unwrap(),"banked_armed",None)?;
 {
  let mut settings=s.settings.lock().unwrap();settings.mode=schedule::Mode::Off;
  let copy=settings.clone();drop(settings);
  storage::save_settings(&s.db.lock().unwrap(),&copy)?;
  #[cfg(target_os="android")]
  android_bridge::sync_settings(&serde_json::to_string(&copy).map_err(|e|e.to_string())?)?;
 }
 {let mut e=s.engine.lock().unwrap();e.chain_remaining=0;e.chain_reset=None;e.next=None;}
 record(&s,"login","已登出；排程已關閉");
 Ok(snapshot(&s))
}
#[tauri::command]async fn refresh(app:AppHandle,s:tauri::State<'_,Arc<State>>)->Result<api::Usage,String>{
 let result=refresh_internal(&s,true).await;
 if let Err(ref e)=result{*s.error.lock().unwrap()=Some(e.clone());}
 let _=app.emit("credits:updated",());result
}
#[tauri::command]async fn manual_trigger(app:AppHandle,s:tauri::State<'_,Arc<State>>)->Result<Snapshot,String>{
 let result=trigger(&s,0,"手動").await;
 if let Err(ref e)=result{record(&s,"error",&format!("手動觸發失敗: {e}"));}
 let _=app.emit("credits:updated",());result?;Ok(snapshot(&s))
}
#[tauri::command]async fn banked_reset(app:AppHandle,s:tauri::State<'_,Arc<State>>,allow_partial:bool)->Result<Snapshot,String>{
 consume(&s,allow_partial,None).await?;
 let _=app.emit("credits:updated",());Ok(snapshot(&s))
}
#[tauri::command]fn toggle_banked_armed(s:tauri::State<'_,Arc<State>>)->Result<Snapshot,String>{
 let previous=s.armed.lock().unwrap().clone();
 let next=if previous.is_some(){None}else{
  let credit=s.credits.lock().unwrap().as_ref().and_then(|x|x.available.first().cloned())
   .ok_or("沒有可用的 Banked Reset")?;
  Some(credit.id)
 };
 storage::set_meta(&s.db.lock().unwrap(),"banked_armed",next.as_deref())?;
 #[cfg(target_os="android")]
 {
  android_bridge::arm_banked(next.is_some())?;
  if next.is_some()&&!android_bridge::banked_armed()?{
   storage::set_meta(&s.db.lock().unwrap(),"banked_armed",None)?;
   return Err("Android 背景排程尚無可用的 Banked Reset，請先更新配額".into());
  }
 }
 *s.armed.lock().unwrap()=next.clone();
 record(&s,"banked_reset",if next.is_some(){"Banked Reset 用盡後自動使用一次 · 已待命"}else{"取消 Banked Reset 待命"});
 Ok(snapshot(&s))
}
#[tauri::command]async fn ntfy_test(s:tauri::State<'_,Arc<State>>)->Result<String,String>{
 let url=s.settings.lock().unwrap().ntfy_topic.clone();
 if url.is_empty(){return Err("請先儲存 ntfy 網址".into())}
 let result=api::ntfy_send(&url,"Codex Tools 測試","ntfy 通知測試成功").await;
 let msg=match &result{Ok(_)=>"已發送測試通知".into(),Err(e)=>format!("發送失敗: {e}")};
 *s.ntfy_status.lock().unwrap()=msg;
 result?;Ok(s.ntfy_status.lock().unwrap().clone())
}
#[tauri::command]fn native_notify_test(s:tauri::State<'_,Arc<State>>)->Result<String,String>{
 if !s.settings.lock().unwrap().native_notify_enabled{return Err("請先開啟系統通知".into())}
 s.app.notification().builder().title("Codex Tools 測試").body("系統通知已啟用")
  .show().map_err(|e|e.to_string())?;
 Ok("已送出測試通知".into())
}
#[tauri::command]fn get_local_state(s:tauri::State<'_,Arc<State>>)->Result<LocalState,String>{
 let (history,events)={
  let db=s.db.lock().unwrap();
  #[cfg(target_os="android")]
  legacy::sync_native(&db)?;
  (storage::history(&db)?,storage::events(&db)?)
 };
 Ok(LocalState{snapshot:snapshot(&s),history,events})
}
#[tauri::command]fn get_history(s:tauri::State<'_,Arc<State>>)->Result<Vec<serde_json::Value>,String>{
 let db=s.db.lock().unwrap();
 #[cfg(target_os="android")]
 legacy::sync_native(&db)?;
 storage::history(&db)
}
#[tauri::command]fn get_events(s:tauri::State<'_,Arc<State>>)->Result<Vec<serde_json::Value>,String>{
 let db=s.db.lock().unwrap();
 #[cfg(target_os="android")]
 legacy::sync_native(&db)?;
 storage::events(&db)
}
fn defer_scheduled_retry(s:&State,planned:&engine::Next,seconds:i64){
 let mut e=s.engine.lock().unwrap();
 e.next=Some(engine::Next{
  at:now()+seconds,kind:planned.kind,auto_count:planned.auto_count,
  reason:format!("失敗後 {} 分鐘再檢查",seconds/60),
 });
}
async fn run_planned(s:&State,planned:engine::Next){
 let mode=s.settings.lock().unwrap().mode.clone();
 if mode==schedule::Mode::Off{return;}
 if engine::is_stale(now(),planned.at){
  record(s,"scheduler","略過睡眠或程式暫停期間逾期的排程");
  replan(s);return;
 }
 // Re-check server state on every scheduled action.
 let quota=match refresh_internal(s,false).await{
  Ok(v)=>v,Err(e)=>{record(s,"error",&format!("排程查詢失敗: {e}"));defer_scheduled_retry(s,&planned,900);return}
 };
 if mode==schedule::Mode::Custom&&engine::confirmed(&quota,now()){
  let remaining=engine::reset_timestamp(quota.five_reset.as_ref()).unwrap_or(now())-now();
  if remaining>=0&&remaining<=60{
   s.engine.lock().unwrap().next=Some(engine::Next{
    at:now()+remaining+15,kind:engine::Kind::AnchorRetry,auto_count:planned.auto_count,
    reason:"等候 5 小時重置".into()});
   return;
  }
 }
 if let Err(e)=engine::can_trigger(&quota,now()){
  record(s,"scheduler",&format!("略過排程: {e}"));
  replan(s);return;
 }
 let followups=if mode==schedule::Mode::Custom{
  if planned.kind==engine::Kind::Chain{s.engine.lock().unwrap().chain_remaining.saturating_sub(1)}
  else{planned.auto_count}
 }else{0};
 if let Err(e)=trigger(s,followups,"排程").await{
  record(s,"error",&format!("排程觸發失敗: {e}"));
  notify(s,"Codex 排程錯誤",&e).await;
  // A network error after POST cannot prove that the server did not
  // accept the trigger. Back off rather than immediately retrying.
  defer_scheduled_retry(s,&planned,900);
  return;
 }
 if planned.kind==engine::Kind::Chain&&followups==0{
  let mut e=s.engine.lock().unwrap();e.chain_remaining=0;e.chain_reset=None;
 }
 replan(s);
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[tauri::command]
fn android_platform_status()->Result<serde_json::Value,String>{
 #[cfg(target_os="android")]
 {android_bridge::platform_status()}
 #[cfg(not(target_os="android"))]
 {Err("僅 Android 支援".into())}
}
#[tauri::command]
fn android_open_system_settings(kind:String)->Result<(),String>{
 #[cfg(target_os="android")]
 {android_bridge::open_system_settings(&kind)}
 #[cfg(not(target_os="android"))]
 {let _=kind;Err("僅 Android 支援".into())}
}
pub fn run(){
 let mut builder=tauri::Builder::default().plugin(tauri_plugin_notification::init());
 #[cfg(desktop)]
 {
  builder=builder.plugin(tauri_plugin_single_instance::init(|app,_args,_cwd|{
   if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}
  }));
 }
 builder
 .setup(|app|{
  let handle=app.handle().clone();
  let path=handle.path().app_data_dir()?;std::fs::create_dir_all(&path)?;
  let db=storage::open(&path.join("codexcredits.sqlite3")).map_err(std::io::Error::other)?;
  #[cfg(target_os="android")]
  let migration_error=legacy::import(&db).err();
  let settings=storage::get_settings(&db);
  let armed=storage::get_meta(&db,"banked_armed");
  #[cfg(target_os="android")]
  let armed=if android_bridge::banked_armed().unwrap_or(false){Some("android-native".into())}else{armed};
  let state=Arc::new(State{
   app:handle.clone(),db:Mutex::new(db),settings:Mutex::new(settings),usage:Mutex::new(None),
   credits:Mutex::new(None),global:Mutex::new(None),last_credits:Mutex::new(0),last_global:Mutex::new(0),
   ntfy_status:Mutex::new("尚未測試".into()),armed:Mutex::new(armed),
   engine:Mutex::new(engine::Engine::default()),error:Mutex::new({
    #[cfg(target_os="android")] {migration_error}
    #[cfg(not(target_os="android"))] {None}
   }),
   updating:tokio::sync::Mutex::new(()),action:tokio::sync::Mutex::new(()),
   login_busy:tokio::sync::Mutex::new(())
  });
  app.manage(state.clone());
  #[cfg(desktop)]
  {
  let show=MenuItem::with_id(app,"show","顯示 Codex Tools",true,None::<&str>)?;
  let quit=MenuItem::with_id(app,"quit","結束並停止監測",true,None::<&str>)?;
  let menu=Menu::with_items(app,&[&show,&quit])?;
  let icon=tauri::image::Image::new_owned(vec![46_u8,122,222,255].repeat(256),16,16);
  TrayIconBuilder::new().icon(icon).menu(&menu)
   .on_menu_event(|app,event|match event.id.as_ref(){
     "show"=>{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}},
     "quit"=>app.exit(0),_=>{}
   }).build(app)?;
  }
  tauri::async_runtime::spawn(async move{
   // First obtain a fresh quota after restart; never replay old persisted alarms.
   if token().is_ok(){
    if let Err(e)=refresh_internal(&state,true).await{
     *state.error.lock().unwrap()=Some(e);
    }
    // The first refresh must update the UI as well, not only periodic polls.
    let _=handle.emit("credits:updated",());
   }
   let mut last_sample=now();
   loop{
    tokio::time::sleep(Duration::from_secs(2)).await;
    let interval=state.settings.lock().unwrap().sampling_minutes.clamp(10,60) as i64*60;
    if now()-last_sample>=interval{
     last_sample=now();
     if token().is_ok(){
      if let Err(e)=refresh_internal(&state,false).await{*state.error.lock().unwrap()=Some(e);}
      #[cfg(not(target_os="android"))]
      let armed_id={state.armed.lock().unwrap().clone()};
     #[cfg(not(target_os="android"))]
     if let Some(id)=armed_id{
       let exhausted=state.usage.lock().unwrap().as_ref().is_some_and(engine::exhausted);
       if exhausted{
        // Disarm before redemption to guarantee one-shot even if response is ambiguous.
        *state.armed.lock().unwrap()=None;
        let _=storage::set_meta(&state.db.lock().unwrap(),"banked_armed",None);
        if let Err(e)=consume(&state,false,Some(id)).await{record(&state,"error",&format!("自動 Banked Reset 未完成: {e}"));}
       }
      }
      let _=handle.emit("credits:updated",());
     }
    }
    #[cfg(not(target_os="android"))]
    let due={
     let mut engine=state.engine.lock().unwrap();
     if engine.next.as_ref().is_some_and(|e|e.at<=now()){engine.next.take()}else{None}
    };
    #[cfg(not(target_os="android"))]
    if let Some(next)=due{
     run_planned(&state,next).await;
     let _=handle.emit("credits:updated",());
    }
   }
  });
  Ok(())
 }).on_window_event(|window,event|{
  #[cfg(desktop)]
   if let WindowEvent::CloseRequested{api,..}=event{api.prevent_close();let _=window.hide();}
   #[cfg(mobile)]
   let _=(window,event);
 })
 .invoke_handler(tauri::generate_handler![
  get_snapshot,save_settings,login_with_chatgpt,sign_out,refresh,manual_trigger,
  banked_reset,toggle_banked_armed,ntfy_test,native_notify_test,get_local_state,get_history,get_events,
  android_platform_status,android_open_system_settings
 ])
 .run(tauri::generate_context!()).expect("CodexCredits could not start");
}
