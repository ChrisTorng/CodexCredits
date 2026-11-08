mod api;
mod schedule;
mod storage;
mod credentials;
mod oauth;
mod engine;
mod banked_reminder;
#[cfg(desktop)] mod tray_status;
#[cfg(target_os="android")] mod android_bridge;
#[cfg(target_os="android")] mod legacy;

use std::{sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},time::Duration};
use serde::Serialize;
use tauri_plugin_notification::NotificationExt;
use tauri::{AppHandle,Emitter,Manager,WindowEvent};
#[cfg(desktop)] use tauri::menu::{Menu,MenuItem,PredefinedMenuItem};
#[cfg(desktop)] use tauri::tray::{TrayIconBuilder,TrayIconEvent,MouseButton};

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
 signed_in:AtomicBool,
 auth_checked:AtomicBool,
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
// An independent read-only SQLite connection allows the first screen to load
// saved usage even if a live refresh or the in-memory state mutex is blocked.
#[cfg(not(target_os="android"))]
#[tauri::command]
async fn get_startup_cache(s:tauri::State<'_,Arc<State>>)->Result<LocalState,String>{
 let app=s.app.clone();
 tokio::task::spawn_blocking(move||{
  let base=app.path().app_data_dir().map_err(|e|e.to_string())?;
  let db=rusqlite::Connection::open_with_flags(base.join("codexcredits.sqlite3"),
   rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
  db.busy_timeout(Duration::from_secs(2)).map_err(|e|e.to_string())?;
  let history=storage::history(&db)?;
  let events=storage::events(&db)?;
  let snapshot=Snapshot{
   settings:storage::get_settings(&db),
   usage:storage::restore_usage(&db),
   credits:storage::get_cached(&db,"last_credits"),
   global:storage::get_cached(&db,"last_global"),
   ntfy_status:"尚未測試".into(),
   armed:storage::get_meta(&db,"banked_armed").is_some(),
   next:None,projected_busy_until:None,error:None,
   signed_in:false,auth_checked:false,
  };
  Ok(LocalState{snapshot,history,events})
 }).await.map_err(|e|format!("本機離線快取工作失敗: {e}"))?
}
#[cfg(target_os="android")]
#[tauri::command]
fn get_startup_cache(s:tauri::State<'_,Arc<State>>)->Result<LocalState,String>{
 get_local_state(s)
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
 projected_busy_until:Option<i64>,
 error:Option<String>,
 signed_in:bool,
 auth_checked:bool,
}
#[cfg(target_os="windows")]
fn refresh_windows_icon_cache(){
 // Refresh old 16px portable EXE icons retained by Windows Explorer.
 #[link(name="shell32")]
 unsafe extern "system" {
  fn SHChangeNotify(event_id:i32,flags:u32,item1:*const std::ffi::c_void,item2:*const std::ffi::c_void);
 }
 unsafe{SHChangeNotify(0x08000000,0,std::ptr::null(),std::ptr::null());}
}
fn now()->i64{chrono::Utc::now().timestamp()}
fn token()->Result<String,String>{credentials::load()}
#[cfg(target_os="android")]
fn native_next()->Option<engine::Next>{
 let platform=android_bridge::platform_status().ok()?;
 let at=platform["next_alarm_ms"].as_i64().unwrap_or(0)/1000;
 if at<=0{return None;}
 let kind=match platform["next_event_kind"].as_str().unwrap_or("auto"){
  "recovery"=>engine::Kind::Recovery,
  "resume"=>engine::Kind::Resume,
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
  projected_busy_until:{
   #[cfg(target_os="android")]
   {android_bridge::platform_status().ok().and_then(|v|v["projected_busy_until_ms"].as_i64()).filter(|v|*v>0).map(|v|v/1000)}
   #[cfg(not(target_os="android"))]
   {
    // Avoid nested engine/usage locks while the UI waits for a snapshot.
     let u=s.usage.lock().unwrap().clone();
     let e=s.engine.lock().unwrap();
     engine::projected_busy_until(u.as_ref(),&e,now())
   }
  },
  error:s.error.lock().unwrap().clone(),
  signed_in:s.signed_in.load(Ordering::Relaxed),
  auth_checked:s.auth_checked.load(Ordering::Relaxed),
 }
}
fn record(s:&State,kind:&str,message:&str){
 let _=storage::event(&s.db.lock().unwrap(),kind,message);
}
fn replan(s:&State){
 let settings=s.settings.lock().unwrap().clone();
 let usage=s.usage.lock().unwrap().clone();
 let mut e=s.engine.lock().unwrap();
 if settings.mode!=schedule::Mode::Custom{e.recovery=None;}
 else if e.recovery.is_none(){
  if let Some(ref usage)=usage{
   if let Some(candidate)=engine::missed_today(&settings,usage,now()){
    if candidate.anchor_at>e.recovery_done_anchor{e.recovery=Some(candidate);}
   }
  }
 }
 e.next=engine::next_event(&settings,usage.as_ref(),&e,now());
 let recovery=e.recovery;
 let pending=e.pending;
 let done=e.recovery_done_anchor;
 let resume_done=e.resume_done_date;
 let chain_remaining=e.chain_remaining;
 let chain_reset=e.chain_reset;
  let chain_retry_at=e.chain_retry_at;
 drop(e);
 let db=s.db.lock().unwrap();
 let _=if let Some(r)=recovery{storage::save_cached(&db,"missed_anchor_recovery_v1",&r)}
 else{storage::set_meta(&db,"missed_anchor_recovery_v1",None)};
 let _=storage::set_meta(&db,"missed_anchor_recovery_done",Some(&done.to_string()));
 let _=storage::set_meta(&db,"day_resume_done_v1",resume_done.as_ref().map(|x|x.to_string()).as_deref());
 let _=storage::set_meta(&db,"chain_remaining_v1",Some(&chain_remaining.to_string()));
 let _=storage::set_meta(&db,"chain_reset_v1",chain_reset.as_ref().map(|x|x.to_string()).as_deref());
  let _=storage::set_meta(&db,"chain_retry_at_v1",chain_retry_at.as_ref().map(|x|x.to_string()).as_deref());
 let _=if let Some(p)=pending{storage::save_cached(&db,"five_pending_verification_v1",&p)}
 else{storage::set_meta(&db,"five_pending_verification_v1",None)};
}
// A zero-used primary window can contain a sliding now+5h placeholder.
// Only real usage, a previously successful local trigger, or a stable
// reset_at over multiple samples proves that a 5h window is running.
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
enum TriggerOutcome{Started,Unchanged,Pending}
/// The Codex request can complete inside an already-running five-hour window.
/// Never report "new 5h started" without evidence that the reset anchor moved.
fn classify_trigger(before:&api::Usage,after:&api::Usage,requested_at:i64)->TriggerOutcome{
 let Some(reset)=engine::reset_timestamp(after.five_reset.as_ref())else{return TriggerOutcome::Pending};
 let previous=engine::reset_timestamp(before.five_reset.as_ref());
 let at_new_window=(reset-(requested_at+5*3600)).abs()<=180;
 let moved=previous.is_none_or(|old|reset-old>90);
 if at_new_window&&moved{return TriggerOutcome::Started;}
 // If reset is still substantially earlier than request+5h, this request
 // was processed in an existing window (e.g. 06:19, reset still 09:47).
 if reset<requested_at+4*3600+30*60{return TriggerOutcome::Unchanged;}
 if previous.is_some_and(|old|(reset-old).abs()<=90)
  &&(before.primary_confirmed||before.five_hour.is_some_and(|u|u>0.0001))
 {return TriggerOutcome::Unchanged;}
 // The idle placeholder may equal the true new reset; only subsequent
 // observations can prove whether it became a stable five-hour window.
 TriggerOutcome::Pending
}
fn confirmed_primary_at(new:&api::Usage,prev:Option<&api::Usage>,triggered_reset:Option<&String>,timestamp:i64)->bool{
 if new.five_hour.is_some_and(|p|p>0.0001){return true;}
 if new.five_hour.is_none(){return false;}
 let Some(fresh)=engine::reset_timestamp(new.five_reset.as_ref())else{return false};
 if fresh<=timestamp || fresh>timestamp+5*3600+60{return false;}
 let natural_weekly=prev.and_then(|old|engine::reset_timestamp(old.weekly_reset.as_ref()))
  .is_some_and(|r|timestamp>=r-900&&timestamp<=r+3600);
 let unexpected_weekly=prev.is_some_and(|old|
  old.weekly.zip(new.weekly).is_some_and(|(a,b)|a-b>2.))&&!natural_weekly;
 if unexpected_weekly{return false;}
 // Persisted only after this app's trigger request has completed.
 if engine::reset_timestamp(triggered_reset).is_some_and(|r|(fresh-r).abs()<=10){return true;}
 let Some(old)=prev else{return false};
 if old.five_hour.is_none(){return false;}
 let Some(previous)=engine::reset_timestamp(old.five_reset.as_ref())else{return false};
 if previous<=timestamp{return false;}
 let Some(elapsed)=chrono::DateTime::parse_from_rfc3339(&old.checked_at)
  .ok().map(|t|timestamp-t.timestamp())else{return false};
 // Wait >=2 minutes to distinguish a real fixed window from a rounded
 // sliding placeholder. A placeholder typically moves with elapsed time.
 if !(120..=6*3600).contains(&elapsed){return false;}
 let drift=(fresh-previous).abs();
 drift<=std::cmp::max(10,elapsed/5)
}
#[cfg(test)]
mod five_hour_detection_tests{
 use super::*;
 fn usage(at:i64,reset:i64,used:f64)->api::Usage{
  api::Usage{
   five_hour:Some(used),
   five_reset:Some(chrono::DateTime::<chrono::Utc>::from_timestamp(reset,0).unwrap().to_rfc3339()),
   checked_at:chrono::DateTime::<chrono::Utc>::from_timestamp(at,0).unwrap().to_rfc3339(),
   ..api::Usage::default()
  }
 }
 #[test]fn first_idle_sample_does_not_activate_five_hour(){
  let t=1_800_000_000;
  assert!(!confirmed_primary_at(&usage(t,t+5*3600,0.),None,None,t));
  assert!(!confirmed_primary_at(&usage(t,t+50*60,0.),None,None,t));
 }
 #[test]fn sliding_reset_follows_the_clock_without_starting_a_window(){
  let t=1_800_000_000;
  let old=usage(t,t+5*3600,0.);
  let next=usage(t+360,t+360+5*3600,0.);
  // Exactly the screenshot's 04:47 -> 09:47 hypothetical idle case.
  assert!(!confirmed_primary_at(&next,Some(&old),None,t+360));
  let mut buggy=old.clone();buggy.primary_confirmed=true;
  assert!(!confirmed_primary_at(&next,Some(&buggy),None,t+360));
 }
 #[test]fn fixed_reset_is_confirmed_after_multiple_zero_usage_samples(){
  let t=1_800_000_000;
  let old=usage(t,t+5*3600,0.);
  let next=usage(t+360,t+5*3600,0.);
  assert!(confirmed_primary_at(&next,Some(&old),None,t+360));
  assert!(!confirmed_primary_at(&next,None,None,t+360));
 }
 #[test]fn explicit_success_and_used_quota_confirm_the_window(){
  let t=1_800_000_000;
  let next=usage(t,t+5*3600,0.);
  assert!(confirmed_primary_at(&next,None,next.five_reset.as_ref(),t));
  assert!(confirmed_primary_at(&usage(t,t+5*3600,0.1),None,None,t));
 }
 #[test]fn invalid_and_stale_reset_are_not_confirmed(){
  let t=1_800_000_000;
  let prev=usage(t-300,t+5*3600,0.);
  assert!(!confirmed_primary_at(&usage(t,t-10,0.),Some(&prev),None,t));
  assert!(!confirmed_primary_at(&usage(t,t+2*24*3600,0.),Some(&prev),None,t));
 }
 #[test]fn global_weekly_refill_does_not_confirm_an_unchanged_idle_window(){
  let t=1_800_000_000;
  let mut old=usage(t,t+5*3600,0.);old.weekly=Some(40.);
  let mut new=usage(t+360,t+5*3600,0.);new.weekly=Some(0.);
  assert!(!confirmed_primary_at(&new,Some(&old),None,t+360));
 }
 #[test]fn minimal_request_does_not_restart_existing_5h_window(){
  let t=1_800_000_000;
  let before=usage(t-60,t+3*3600+28*60,0.);
  let after=usage(t+5,t+3*3600+28*60,0.);
  assert_eq!(classify_trigger(&before,&after,t),TriggerOutcome::Unchanged);
 }
 #[test]fn new_reset_after_completed_codex_request_confirms_new_window(){
  let t=1_800_000_000;
  let before=usage(t-60,t+3*3600+28*60,0.);
  let after=usage(t+6,t+5*3600,0.);
  assert_eq!(classify_trigger(&before,&after,t),TriggerOutcome::Started);
 }
 #[test]fn same_now_plus_five_hours_placeholder_requires_followup_sampling(){
  let t=1_800_000_000;
  let before=usage(t-60,t+5*3600,0.);
  let after=usage(t+6,t+5*3600,0.);
  assert_eq!(classify_trigger(&before,&after,t),TriggerOutcome::Pending);
 }
 #[test]fn no_reset_response_is_not_success(){
  let t=1_800_000_000;
  let before=usage(t-60,t+5*3600,0.);
  let after=api::Usage::default();
  assert_eq!(classify_trigger(&before,&after,t),TriggerOutcome::Pending);
 }

}
async fn refresh_internal(s:&State,force_aux:bool)->Result<api::Usage,String>{
 let _guard=s.updating.lock().await;
 let access=oauth::valid_access().await?;
 let mut value=api::fetch_usage(&access.token,access.account_id.as_deref()).await?;
 let before=s.usage.lock().unwrap().clone();
 let confirmed_reset=storage::get_meta(&s.db.lock().unwrap(),"five_trigger_verified_reset_v2");
 value.primary_confirmed=confirmed_primary_at(&value,before.as_ref(),confirmed_reset.as_ref(),now());
 {
  let db=s.db.lock().unwrap();
  storage::add_sample(&db,value.five_hour,value.weekly,value.five_reset.as_deref(),value.weekly_reset.as_deref())?;
   storage::save_cached(&db,"last_usage",&value)?;
  let _=storage::cleanup(&db,s.settings.lock().unwrap().retention_days);
 }
 *s.usage.lock().unwrap()=Some(value.clone());
 s.signed_in.store(true,Ordering::Relaxed);
 s.auth_checked.store(true,Ordering::Relaxed);
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
    let _=storage::save_cached(&s.db.lock().unwrap(),"last_credits",&c);
     *s.credits.lock().unwrap()=Some(c);
    *s.last_credits.lock().unwrap()=now();
   }
   Err(e)=>record(s,"warning",&format!("Banked Reset 查詢失敗: {e}")),
  }
 }
 let check_global={let last=s.last_global.lock().unwrap();force_aux||now()-*last>=21600};
 if check_global{
  match api::global_reset().await{
   Ok((value,verified))=>{
    // Backfill previously imported Android "unexpected_reset" events, too.
    // Do not fabricate a global reset without confirmed public evidence.
    if !verified.is_empty(){
     let db=s.db.lock().unwrap();
     #[cfg(target_os="android")]
     let _=legacy::sync_native(&db);
     let _=storage::reconcile_global_resets(&db,&verified);
    }
    let _=storage::save_cached(&s.db.lock().unwrap(),"last_global",&value);
     *s.global.lock().unwrap()=Some(value);
    *s.last_global.lock().unwrap()=now();
   }
   Err(_)=>{*s.last_global.lock().unwrap()=now();}
  }
 }
 #[cfg(desktop)]
 tray_status::refresh(&s.app,Some(&value));
 #[cfg(not(target_os="android"))]
 {
  let credits_snapshot={s.credits.lock().unwrap().clone()};
  if let Some(credits)=credits_snapshot{
   remind_expiring_banked(s,&credits).await;
  }
 }
 Ok(value)
}
#[cfg(not(target_os="android"))]
async fn remind_expiring_banked(s:&State,credits:&api::Credits){
 let Some((credit,remaining))=banked_reminder::nearest(credits,now())else{return};
 let Some((stage,description))=banked_reminder::stage(remaining)else{return};
 let settings=s.settings.lock().unwrap().clone();
 if !settings.native_notify_enabled&&settings.ntfy_topic.is_empty(){return;}
 // Do not persist the server-side credit ID in event logs.
 use sha2::{Digest,Sha256};
 let hash=Sha256::digest(credit.id.as_bytes());
 let suffix=hash[..8].iter().map(|b|format!("{b:02x}")).collect::<String>();
 let key=format!("banked_reminder_{suffix}");
 let already={
  let db=s.db.lock().unwrap();
  storage::get_meta(&db,&key).and_then(|s|s.parse::<u8>().ok()).unwrap_or(0)
 };
 if already>=stage{return;}
 // Commit first so a crash, restart, Android resume or repeated refresh
 // cannot deliver duplicate reminder notifications for this milestone.
 if storage::set_meta(&s.db.lock().unwrap(),&key,Some(&stage.to_string())).is_err(){return;}
 let expiry=chrono::DateTime::parse_from_rfc3339(&credit.expires_at)
  .map(|d|d.with_timezone(&chrono::Local).format("%-m/%-d %H:%M").to_string())
  .unwrap_or_else(|_|"—".into());
 record(s,"banked_reminder",&format!("Banked Reset {description} · 到期 {expiry}"));
 notify(s,"Banked Reset 即將到期",&format!("{description} · 到期 {expiry}")).await;
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
async fn trigger(s:&State,followups:u8,kind:&str)->Result<TriggerOutcome,String>{
 let _action=s.action.lock().await;
 let q=refresh_internal(s,false).await?;
 engine::can_trigger(&q,now())?;
 let access=oauth::valid_access().await?;
 let requested_at=now();
 // Wait for the terminal SSE response, not merely HTTP 200.
 api::trigger_minimal(&access.token,access.account_id.as_deref()).await?;
 tokio::time::sleep(Duration::from_secs(2)).await;
 let mut after=refresh_internal(s,false).await?;
 let outcome=classify_trigger(&q,&after,requested_at);
 match outcome {
  TriggerOutcome::Started=>{
   after.primary_confirmed=true;
   {
    let db=s.db.lock().unwrap();
    // v2 key intentionally excludes old false-positive 2xx-only receipts.
    storage::set_meta(&db,"five_trigger_verified_reset_v2",after.five_reset.as_deref())?;
    storage::save_cached(&db,"last_usage",&after)?;
   }
   *s.usage.lock().unwrap()=Some(after.clone());
   record(s,"trigger",&format!("{kind}5h 視窗啟動已確認"));
   if followups>0 {
    let reset=engine::reset_timestamp(after.five_reset.as_ref());
    let mut e=s.engine.lock().unwrap();
    e.chain_remaining=followups;e.chain_reset=reset;e.chain_retry_at=None;
   }
   notify(s,"Codex 5h 視窗啟動已確認",&format!("5h 剩餘 {:.0}%",100.-after.five_hour.unwrap_or(0.))).await;
  },
  TriggerOutcome::Unchanged=>{
   let reset=after.five_reset.as_deref().unwrap_or("—");
   record(s,"trigger_no_change",&format!("{kind}請求完成，但 5h 視窗未重新開始 · 原重置時間不變 · {reset}"));
   notify(s,"Codex 請求完成，5h 視窗未重啟","官方用量仍顯示原來的重置時間，沒有建立新視窗").await;
  },
  TriggerOutcome::Pending=>{
   record(s,"trigger_pending",&format!("{kind}請求完成，但 5h 視窗是否新建仍待確認"));
   notify(s,"Codex 請求完成，5h 尚待確認","尚未觀察到可確認的新視窗；不會提前宣稱已啟動").await;
  }
 }
 replan(s);
 Ok(outcome)
}
async fn consume(s:&State,allow_partial:bool,selected:Option<String>)->Result<(),String>{
 let _action=s.action.lock().await;
 let current=refresh_internal(s,true).await?;
 // Always re-check the server's latest quota before redemption, even if
 // the UI previously showed an eligible state. 100%/98% must not be redeemed.
 if !api::banked_reset_eligible(&current){
  return Err("目前不需要重置：OpenAI 規定至少一項適用用量剩餘低於 80% 才能使用 Banked Reset（Your usage does not need a reset right now）".into());
 }
 if !allow_partial&&!engine::exhausted(&current){
  return Err("用量尚未用盡；如要提早重置，請選擇「立即使用（含部分用量）」".into());
 }
 let access=oauth::valid_access().await?;
 let c=api::fetch_credits(&access.token,access.account_id.as_deref()).await?;
 let credit=c.available.iter()
  .filter(|c|selected.as_ref().is_none_or(|id|id==&c.id))
  .filter_map(|c|chrono::DateTime::parse_from_rfc3339(&c.expires_at)
    .ok().map(|expiry|(c,expiry.timestamp())))
  .filter(|(_,expiry)|*expiry>now())
  .min_by_key(|(_,expiry)|*expiry)
  .map(|(credit,_)|credit)
  .ok_or("目前沒有有效的 Banked Reset")?;
 if c.count<=0{return Err("Banked Reset 可用次數為 0".into())}
 let windows=api::consume_credit(&access.token,access.account_id.as_deref(),&credit.id).await?;
 if windows<=0{return Err("OpenAI 未確認重置任何用量視窗；請重新整理官方 Usage 確認 Banked Reset 仍在，避免重複嘗試".into())}
 *s.armed.lock().unwrap()=None;
 {
  let db=s.db.lock().unwrap();
  storage::set_meta(&db,"banked_armed",None)?;
  storage::set_meta(&db,"five_trigger_reset",None)?;
  storage::set_meta(&db,"five_trigger_verified_reset_v2",None)?;
 }
 tokio::time::sleep(Duration::from_secs(1)).await;
 let _=refresh_internal(s,true).await;
 record(s,"banked_reset",&format!("Banked Reset 已使用 · 重置 {windows} 個視窗"));
 notify(s,"Codex Banked Reset 完成",&format!("已重置 {windows} 個用量視窗")).await;
 replan(s);Ok(())
}
#[tauri::command]fn get_snapshot(s:tauri::State<'_,Arc<State>>)->Snapshot{snapshot(&s)}
#[tauri::command]fn save_settings(s:tauri::State<'_,Arc<State>>,settings:schedule::Settings)->Result<Snapshot,String>{
 if !settings.valid(){return Err("設定不正確：請檢查排程、ntfy 網址與採樣間隔".into())}
 if settings.mode!=schedule::Mode::Off&&!s.signed_in.load(Ordering::Relaxed){
   return Err("請先登入才能啟用自動排程".into())
  }
 {
  let db=s.db.lock().unwrap();storage::save_settings(&db,&settings)?;
 }
 #[cfg(target_os="android")]
 android_bridge::sync_settings(&serde_json::to_string(&settings).map_err(|e|e.to_string())?)?;
 let old=s.settings.lock().unwrap().clone();
 let changed_mode=old.mode!=settings.mode;
 let changed_recovery=changed_mode||old.missed_anchor_policy!=settings.missed_anchor_policy
  ||serde_json::to_string(&old.rules).ok()!=serde_json::to_string(&settings.rules).ok()
  ||serde_json::to_string(&old.overrides).ok()!=serde_json::to_string(&settings.overrides).ok();
 *s.settings.lock().unwrap()=settings;
 if changed_recovery{
  let mut e=s.engine.lock().unwrap();e.recovery=None;e.recovery_done_anchor=0;e.pending=None;e.resume_done_date=None;e.resume_retry_at=None;
   e.chain_remaining=0;e.chain_reset=None;e.chain_retry_at=None;
 }
 if changed_mode{
  let mut e=s.engine.lock().unwrap();e.chain_remaining=0;e.chain_reset=None;e.chain_retry_at=None;
  record(&s,"mode","排程模式已變更");
 }
 replan(&s);Ok(snapshot(&s))
}
#[tauri::command]async fn login_with_chatgpt(app:AppHandle,s:tauri::State<'_,Arc<State>>)->Result<Snapshot,String>{
 let _busy=s.login_busy.try_lock().map_err(|_|"登入程序已進行中")?;
 let session=oauth::browser_login().await?;
 {let _guard=s.updating.lock().await;oauth::save_session(&session)?;}
 s.signed_in.store(true,Ordering::Relaxed);
 s.auth_checked.store(true,Ordering::Relaxed);
 *s.error.lock().unwrap()=None;
 record(&s,"login","ChatGPT 登入成功");
 let _=app.emit("credits:updated",());Ok(snapshot(&s))
}
#[tauri::command]async fn sign_out(s:tauri::State<'_,Arc<State>>)->Result<Snapshot,String>{
 let _guard=s.updating.lock().await;
 credentials::sign_out()?;
 s.signed_in.store(false,Ordering::Relaxed);
 s.auth_checked.store(true,Ordering::Relaxed);
 #[cfg(target_os="android")]
 android_bridge::clear_auth()?;
 *s.usage.lock().unwrap()=None;*s.credits.lock().unwrap()=None;
 *s.armed.lock().unwrap()=None;
 storage::set_meta(&s.db.lock().unwrap(),"five_trigger_reset",None)?;
 storage::set_meta(&s.db.lock().unwrap(),"five_trigger_verified_reset_v2",None)?;
 storage::set_meta(&s.db.lock().unwrap(),"banked_armed",None)?;
 {
  let mut settings=s.settings.lock().unwrap();settings.mode=schedule::Mode::Off;
  let copy=settings.clone();drop(settings);
  storage::save_settings(&s.db.lock().unwrap(),&copy)?;
  #[cfg(target_os="android")]
  android_bridge::sync_settings(&serde_json::to_string(&copy).map_err(|e|e.to_string())?)?;
 }
 {let mut e=s.engine.lock().unwrap();e.chain_remaining=0;e.chain_reset=None;e.chain_retry_at=None;e.next=None;e.recovery=None;e.recovery_done_anchor=0;e.pending=None;e.resume_done_date=None;e.resume_retry_at=None;}
 storage::set_meta(&s.db.lock().unwrap(),"missed_anchor_recovery_v1",None)?;
 storage::set_meta(&s.db.lock().unwrap(),"missed_anchor_recovery_done",None)?;
 storage::set_meta(&s.db.lock().unwrap(),"day_resume_done_v1",None)?;
 storage::set_meta(&s.db.lock().unwrap(),"chain_remaining_v1",None)?;
 storage::set_meta(&s.db.lock().unwrap(),"chain_reset_v1",None)?;
  storage::set_meta(&s.db.lock().unwrap(),"chain_retry_at_v1",None)?;
 storage::set_meta(&s.db.lock().unwrap(),"five_pending_verification_v1",None)?;
 record(&s,"login","已登出；排程已關閉");
 Ok(snapshot(&s))
}
#[tauri::command]async fn refresh(app:AppHandle,s:tauri::State<'_,Arc<State>>)->Result<api::Usage,String>{
 let result=refresh_internal(&s,true).await;
 if let Err(ref e)=result{*s.error.lock().unwrap()=Some(e.clone());}
 let _=app.emit("credits:updated",());result
}
#[tauri::command]async fn manual_trigger(app:AppHandle,s:tauri::State<'_,Arc<State>>)->Result<TriggerOutcome,String>{
 let result=trigger(&s,0,"手動").await;
 if let Err(ref e)=result{record(&s,"error",&format!("手動 Codex 請求失敗: {e}"));}
 let _=app.emit("credits:updated",());
 result
}
#[tauri::command]async fn banked_reset(app:AppHandle,s:tauri::State<'_,Arc<State>>,allow_partial:bool)->Result<Snapshot,String>{
 consume(&s,allow_partial,None).await?;
 let _=app.emit("credits:updated",());Ok(snapshot(&s))
}
#[tauri::command]fn toggle_banked_armed(s:tauri::State<'_,Arc<State>>)->Result<Snapshot,String>{
 let previous=s.armed.lock().unwrap().clone();
 let next=if previous.is_some(){None}else{
  let credit=s.credits.lock().unwrap().as_ref()
   .and_then(|x|banked_reminder::nearest(x,now()).map(|(credit,_)|credit.id.clone()))
   .ok_or("沒有可用的 Banked Reset")?;
  Some(credit)
 };
 storage::set_meta(&s.db.lock().unwrap(),"banked_armed",next.as_deref())?;
 #[cfg(target_os="android")]
 {
  android_bridge::arm_banked(next.is_some())?;
  if next.is_some()&&!android_bridge::banked_armed()?{
   storage::set_meta(&s.db.lock().unwrap(),"banked_armed",None)?;
   return Err("Android 背景排程尚無可用的 Banked Reset，請先更新用量".into());
  }
 }
 *s.armed.lock().unwrap()=next.clone();
 record(&s,"banked_reset",if next.is_some(){"Banked Reset 用盡後自動使用一次 · 已待命"}else{"取消 Banked Reset 待命"});
 Ok(snapshot(&s))
}
#[tauri::command]async fn ntfy_test(s:tauri::State<'_,Arc<State>>)->Result<String,String>{
 let url=s.settings.lock().unwrap().ntfy_topic.clone();
 if url.is_empty(){return Err("請先儲存 ntfy 網址".into())}
 let result=api::ntfy_send(&url,"Codex Credits 測試","ntfy 通知測試成功").await;
 let msg=match &result{Ok(_)=>"已發送測試通知".into(),Err(e)=>format!("發送失敗: {e}")};
 *s.ntfy_status.lock().unwrap()=msg;
 result?;Ok(s.ntfy_status.lock().unwrap().clone())
}
#[tauri::command]fn native_notify_test(s:tauri::State<'_,Arc<State>>)->Result<String,String>{
 if !s.settings.lock().unwrap().native_notify_enabled{return Err("請先開啟系統通知".into())}
 s.app.notification().builder().title("Codex Credits 測試").body("系統通知已啟用")
  .show().map_err(|e|e.to_string())?;
 Ok("已送出測試通知".into())
}
// Android's JNI and native scheduler retain the original synchronous
// commands; only Windows moves SQLite reads off the WebView main thread.
#[cfg(target_os="android")]
#[tauri::command]
fn get_local_state(s:tauri::State<'_,Arc<State>>)->Result<LocalState,String>{
 let (history,events)={
  let db=s.db.lock().unwrap();
  legacy::sync_native(&db)?;
  (storage::history(&db)?,storage::events(&db)?)
 };
 Ok(LocalState{snapshot:snapshot(&s),history,events})
}
#[cfg(not(target_os="android"))]
#[tauri::command]
async fn get_local_state(s:tauri::State<'_,Arc<State>>)->Result<LocalState,String>{
 let state=Arc::clone(&*s);
 tokio::task::spawn_blocking(move||{
  let (history,events)={
   let db=state.db.lock().unwrap();
   (storage::history(&db)?,storage::events(&db)?)
  };
  Ok(LocalState{snapshot:snapshot(&state),history,events})
 }).await.map_err(|e|format!("本機用量歷史讀取工作失敗: {e}"))?
}
#[cfg(target_os="android")]
#[tauri::command]
fn get_history(s:tauri::State<'_,Arc<State>>)->Result<Vec<serde_json::Value>,String>{
 let db=s.db.lock().unwrap();
 legacy::sync_native(&db)?;
 storage::history(&db)
}
#[cfg(not(target_os="android"))]
#[tauri::command]
async fn get_history(s:tauri::State<'_,Arc<State>>)->Result<Vec<serde_json::Value>,String>{
 let state=Arc::clone(&*s);
 tokio::task::spawn_blocking(move||storage::history(&state.db.lock().unwrap()))
  .await.map_err(|e|format!("用量歷史讀取工作失敗: {e}"))?
}
#[cfg(target_os="android")]
#[tauri::command]
fn get_events(s:tauri::State<'_,Arc<State>>)->Result<Vec<serde_json::Value>,String>{
 let db=s.db.lock().unwrap();
 legacy::sync_native(&db)?;
 storage::events(&db)
}
#[cfg(not(target_os="android"))]
#[tauri::command]
async fn get_events(s:tauri::State<'_,Arc<State>>)->Result<Vec<serde_json::Value>,String>{
 let state=Arc::clone(&*s);
 tokio::task::spawn_blocking(move||storage::events(&state.db.lock().unwrap()))
  .await.map_err(|e|format!("事件讀取工作失敗: {e}"))?
}

fn defer_scheduled_retry(s:&State,planned:&engine::Next,seconds:i64){
 let mut e=s.engine.lock().unwrap();
 if planned.kind==engine::Kind::Resume{
  e.resume_retry_at=Some(now()+seconds);
  drop(e);replan(s);return;
 }
 if planned.kind==engine::Kind::Recovery{
  if let Some(ref mut r)=e.recovery{r.at=now()+seconds;}
  drop(e);replan(s);return;
 }
 if planned.kind==engine::Kind::Verify{
  if let Some(ref mut p)=e.pending{p.at=now()+seconds;}
  drop(e);replan(s);return;
 }
  if planned.kind==engine::Kind::Chain{
   e.chain_retry_at=Some(now()+seconds);
   drop(e);replan(s);return;
  }
  if matches!(planned.kind,engine::Kind::Anchor|engine::Kind::AnchorRetry){
   e.recovery=Some(engine::Recovery{anchor_at:planned.at,at:now()+seconds,auto_count:planned.auto_count});
   drop(e);replan(s);return;
  }
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
 if planned.kind==engine::Kind::Recovery&&engine::confirmed(&quota,now()){
  if let Some(reset)=engine::reset_timestamp(quota.five_reset.as_ref()){
   if reset>now(){
    {let mut e=s.engine.lock().unwrap();if let Some(ref mut r)=e.recovery{r.at=reset+15;}}
    record(s,"scheduler","補接續時現有 5h 仍有效，延至實際重置後");
    replan(s);return;
   }
  }
 }
 if planned.kind==engine::Kind::Verify{
  let actual_reset=engine::reset_timestamp(quota.five_reset.as_ref());
  // Do not hold a std::sync::MutexGuard across the async notification.
  let verification={
   let mut e=s.engine.lock().unwrap();
   if let Some(pending)=e.pending.take(){
    let fixed=engine::confirmed(&quota,now())
     && actual_reset.is_some_and(|r|(r-pending.expected_reset).abs()<=20);
    if fixed{
      e.chain_remaining=pending.auto_count;
      e.chain_reset=if pending.auto_count>0{actual_reset}else{None};
      e.chain_retry_at=None;
     if pending.from_recovery{
      if let Some(r)=e.recovery.take(){e.recovery_done_anchor=r.anchor_at;}
     }
     if let Some(date)=pending.resume_date{e.resume_done_date=Some(date);e.resume_retry_at=None;}
    }else{
      if pending.from_recovery{if let Some(ref mut r)=e.recovery{r.at=now()+900;}}
      else if pending.resume_date.is_some(){e.resume_retry_at=Some(now()+900);}
      else if e.chain_remaining>0{
       e.chain_retry_at=Some(now()+900);
       if let Some(actual)=actual_reset.filter(|v|*v>now()){e.chain_reset=Some(actual);}
      }else{
       e.recovery=Some(engine::Recovery{
        anchor_at:pending.at.saturating_sub(150),at:now()+900,auto_count:pending.auto_count});
      }
    }
    Some(fixed)
   }else{None}
  };
  match verification{
   Some(true)=>{
    record(s,"scheduler","已確認先前請求啟動 5h，後續接續排程恢復");
    notify(s,"Codex 5h 啟動已確認","後續自動接續排程已恢復").await;
   },
   Some(false)=>record(s,"scheduler","無法確認上一筆請求建立新 5h，保留下一次排程"),
   None=>{},
  }
  replan(s);return;
 }
 if mode==schedule::Mode::Custom&&engine::confirmed(&quota,now()){
  let actual_reset=engine::reset_timestamp(quota.five_reset.as_ref()).unwrap_or(now());
  let remaining=actual_reset-now();
  if remaining>60&&planned.kind==engine::Kind::Chain{
   s.engine.lock().unwrap().chain_reset=Some(actual_reset);
   record(s,"scheduler","目前 5h 視窗延後重置，接續改以官方最新時間為準");
   replan(s);return;
  }
  if remaining>=0&&remaining<=60{
   s.engine.lock().unwrap().next=Some(engine::Next{
    at:now()+remaining+15,
     kind:if matches!(planned.kind,engine::Kind::Anchor|engine::Kind::AnchorRetry)
      {engine::Kind::AnchorRetry}else{planned.kind},
     auto_count:planned.auto_count,
    reason:"等候 5 小時重置".into()});
   return;
  }
 }
  if let Err(e)=engine::can_trigger(&quota,now()){
   record(s,"scheduler",&format!("略過排程: {e}"));
   if matches!(planned.kind,engine::Kind::Resume|engine::Kind::Chain)&&engine::blocked(&quota){
    defer_scheduled_retry(s,&planned,900);
   }else{replan(s);}
   return;
  }
 let followups=if mode==schedule::Mode::Custom{
  if planned.kind==engine::Kind::Chain{s.engine.lock().unwrap().chain_remaining.saturating_sub(1)}
  else{planned.auto_count}
 }else{0};
 match trigger(s,followups,"排程").await{
  Ok(TriggerOutcome::Started)=>{
   if planned.kind==engine::Kind::Resume{
    let mut e=s.engine.lock().unwrap();
    e.resume_done_date=chrono::DateTime::<chrono::Utc>::from_timestamp(planned.at,0)
      .map(|v|v.with_timezone(&chrono::Local).date_naive());
    e.resume_retry_at=None;
   }
   if planned.kind==engine::Kind::Recovery{
    let mut e=s.engine.lock().unwrap();
    if let Some(r)=e.recovery.take(){e.recovery_done_anchor=r.anchor_at;}
    drop(e);
    record(s,"scheduler","錨點失敗後已成功補接續");
   }
  },
  Ok(TriggerOutcome::Unchanged)=>{
   if planned.kind==engine::Kind::Recovery{
    let latest_reset={s.usage.lock().unwrap().as_ref().and_then(|u|engine::reset_timestamp(u.five_reset.as_ref()))};
    let mut e=s.engine.lock().unwrap();
    if let Some(ref mut r)=e.recovery{
     r.at=latest_reset.filter(|t|*t>now()).map(|v|v+15).unwrap_or(now()+900);
    }
    drop(e);replan(s);return;
   }
   replan(s);
   let confirmed={s.usage.lock().unwrap().as_ref().is_some_and(|u|engine::confirmed(u,now()))};
   if !confirmed{defer_scheduled_retry(s,&planned,900);}
   return;
  },
  Ok(TriggerOutcome::Pending)=>{
   let latest_reset={s.usage.lock().unwrap().as_ref()
    .and_then(|u|engine::reset_timestamp(u.five_reset.as_ref()))};
   let plausible=latest_reset.is_some_and(|r|(r-(now()+5*3600)).abs()<=180);
   if plausible{
    let mut e=s.engine.lock().unwrap();
    e.pending=Some(engine::PendingVerification{
     at:now()+150,expected_reset:latest_reset.unwrap(),auto_count:followups,
     from_recovery:planned.kind==engine::Kind::Recovery,
     resume_date:if planned.kind==engine::Kind::Resume{
      chrono::DateTime::<chrono::Utc>::from_timestamp(planned.at,0)
       .map(|v|v.with_timezone(&chrono::Local).date_naive())
     }else{None},
    });
    drop(e);
    record(s,"scheduler","Codex 請求完成，2 分鐘後核對 5h 視窗後才接續");
    replan(s);
   }else{
    defer_scheduled_retry(s,&planned,900);
   }
   return;
  },
  Err(e)=>{
   record(s,"error",&format!("排程觸發失敗: {e}"));
   notify(s,"Codex 排程錯誤",&e).await;
   // A network error after POST cannot prove the server rejected it.
   defer_scheduled_retry(s,&planned,900);
   return;
  }
 }
 if planned.kind==engine::Kind::Chain&&followups==0{
  let mut e=s.engine.lock().unwrap();e.chain_remaining=0;e.chain_reset=None;e.chain_retry_at=None;
 }
 replan(s);
}
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
#[cfg_attr(mobile, tauri::mobile_entry_point)]
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
  if storage::get_meta(&db,"android_quota_cutover_ms_v2").is_none(){
   // Discard ambiguous old samples; keep sign-in, settings and events.
   db.execute("DELETE FROM samples",[]).map_err(std::io::Error::other)?;
   let cutoff=chrono::Utc::now().timestamp_millis().to_string();
   storage::set_meta(&db,"android_quota_cutover_ms_v2",Some(&cutoff)).map_err(std::io::Error::other)?;
  }
  #[cfg(target_os="android")]
  let migration_error=legacy::import(&db).err();
  let settings=storage::get_settings(&db);
  // Restore last known quota before OAuth or any network call.
  let cached_usage=storage::restore_usage(&db);
  let cached_credits=storage::get_cached::<api::Credits>(&db,"last_credits");
  let cached_global=storage::get_cached::<api::Global>(&db,"last_global");
  let armed=storage::get_meta(&db,"banked_armed");
  let recovery=storage::get_cached::<engine::Recovery>(&db,"missed_anchor_recovery_v1");
  let pending=storage::get_cached::<engine::PendingVerification>(&db,"five_pending_verification_v1");
  let chain_remaining=storage::get_meta(&db,"chain_remaining_v1")
    .and_then(|v|v.parse::<u8>().ok()).unwrap_or(0).min(6);
   let chain_reset=storage::get_meta(&db,"chain_reset_v1")
    .and_then(|v|v.parse::<i64>().ok());
    let chain_retry_at=storage::get_meta(&db,"chain_retry_at_v1")
     .and_then(|v|v.parse::<i64>().ok());
   let resume_done=storage::get_meta(&db,"day_resume_done_v1")
    .and_then(|v|chrono::NaiveDate::parse_from_str(&v,"%Y-%m-%d").ok());
   let recovery_done=storage::get_meta(&db,"missed_anchor_recovery_done")
   .and_then(|v|v.parse::<i64>().ok()).unwrap_or(0);
  #[cfg(target_os="android")]
  let armed=if android_bridge::banked_armed().unwrap_or(false){Some("android-native".into())}else{armed};
  let state=Arc::new(State{
   app:handle.clone(),db:Mutex::new(db),settings:Mutex::new(settings),usage:Mutex::new(cached_usage),
   credits:Mutex::new(cached_credits),global:Mutex::new(cached_global),last_credits:Mutex::new(0),last_global:Mutex::new(0),
   ntfy_status:Mutex::new("尚未測試".into()),armed:Mutex::new(armed),
   engine:Mutex::new(engine::Engine{recovery,recovery_done_anchor:recovery_done,pending,resume_done_date:resume_done,chain_remaining,chain_reset,chain_retry_at,..Default::default()}),
    signed_in:AtomicBool::new(false),auth_checked:AtomicBool::new(false),error:Mutex::new({
    #[cfg(target_os="android")] {migration_error}
    #[cfg(not(target_os="android"))] {None}
   }),
   updating:tokio::sync::Mutex::new(()),action:tokio::sync::Mutex::new(()),
   login_busy:tokio::sync::Mutex::new(())
  });
  app.manage(state.clone());
  #[cfg(desktop)]
  {
  let show=MenuItem::with_id(app,"show","開啟 Codex Credits",true,None::<&str>)?;
  let quit=MenuItem::with_id(app,"quit","結束並停止監測",true,None::<&str>)?;
  let separator=PredefinedMenuItem::separator(app)?;
  let menu=Menu::with_items(app,&[&show,&separator,&quit])?;
  // Match the Android launcher image and the Windows executable/taskbar icon.
  // The previous 16px blue placeholder was unrelated to the app icon.
  let icon=app.default_window_icon()
   .ok_or_else(||std::io::Error::other("Bundled Codex Credits icon is missing"))?
   .clone();
  if let Some(window)=app.get_webview_window("main"){
   window.set_icon(icon.clone())?;
  }
  #[cfg(target_os="windows")]
  {
   // Shell notifications can stall Explorer; never run them on the UI thread.
   let key=concat!("icon_cache_refreshed_",env!("CARGO_PKG_VERSION"));
   let needs=storage::get_meta(&state.db.lock().unwrap(),key).is_none();
   if needs{
    let background_state=Arc::clone(&state);
    std::thread::spawn(move ||{
     refresh_windows_icon_cache();
     let _=storage::set_meta(&background_state.db.lock().unwrap(),key,Some("1"));
    });
   }
  }
  // Use persisted usage for the initial tray too; live polling comes afterward.
  let initial_tray_usage=state.usage.lock().unwrap().clone();
  TrayIconBuilder::with_id("quota")
   .icon(tray_status::icon(initial_tray_usage.as_ref(),now()))
   .tooltip(tray_status::tooltip(initial_tray_usage.as_ref(),now())).show_menu_on_left_click(false).menu(&menu)
   .on_tray_icon_event(|tray,event|{
     if let TrayIconEvent::DoubleClick{button:MouseButton::Left,..}=event{
       if let Some(window)=tray.app_handle().get_webview_window("main"){
         let _=window.show();let _=window.unminimize();let _=window.set_focus();
       }
     }
   })
   .on_menu_event(|app,event|match event.id.as_ref(){
     "show"=>{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}},
     "quit"=>app.exit(0),_=>{}
   }).build(app)?;
  }
  #[cfg(not(target_os="android"))]
  tauri::async_runtime::spawn(async move{
   // First obtain a fresh quota after restart; never replay old persisted alarms.
   // Check stored credentials away from the WebView event loop.
    let logged_in=tokio::task::spawn_blocking(||token().is_ok()).await.unwrap_or(false);
    state.signed_in.store(logged_in,Ordering::Relaxed);
    state.auth_checked.store(true,Ordering::Relaxed);
    let _=handle.emit("credits:updated",());
    if logged_in{
     if let Err(e)=refresh_internal(&state,true).await{
     *state.error.lock().unwrap()=Some(e);
    }
    // The first refresh must update the UI as well, not only periodic polls.
    let _=handle.emit("credits:updated",());
   }
   let mut last_sample=now();
   loop{
    tokio::time::sleep(Duration::from_secs(2)).await;
    #[cfg(desktop)]
    if now()%60<2{
      let usage=state.usage.lock().unwrap().clone();
      tray_status::refresh(&handle,usage.as_ref());
    }
    let interval=state.settings.lock().unwrap().sampling_minutes.clamp(10,60) as i64*60;
    if now()-last_sample>=interval{
     last_sample=now();
     if state.signed_in.load(Ordering::Relaxed){
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
  #[cfg(target_os="android")]
  tauri::async_runtime::spawn(async move {
   // Android's native AlarmManager/JobScheduler already runs in the background.
   // Do not keep a second, two-second polling loop alive in the Tauri process.
   // Check stored credentials away from the WebView event loop.
    let logged_in=token().is_ok();
    state.signed_in.store(logged_in,Ordering::Relaxed);
    state.auth_checked.store(true,Ordering::Relaxed);
    let _=handle.emit("credits:updated",());
    if logged_in{
     if let Err(e)=refresh_internal(&state,true).await{
     *state.error.lock().unwrap()=Some(e);
    }
    let _=handle.emit("credits:updated",());
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
  banked_reset,toggle_banked_armed,ntfy_test,native_notify_test,get_local_state,get_startup_cache,get_history,get_events,
  android_platform_status,android_open_system_settings
 ])
 .run(tauri::generate_context!()).expect("CodexCredits could not start");
}
