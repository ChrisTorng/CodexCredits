mod api;
mod schedule;
mod storage;
mod credentials;
mod oauth;
use std::{sync::{Arc,Mutex},time::Duration};
use tauri::{AppHandle,Emitter,Manager,WindowEvent};
use tauri::menu::{Menu,MenuItem};
use tauri::tray::TrayIconBuilder;
use serde::Serialize;
struct State { db:Mutex<rusqlite::Connection>, settings:Mutex<schedule::Settings>, usage:Mutex<Option<api::Usage>>, error:Mutex<Option<String>>, updating:tokio::sync::Mutex<()>, login_busy:tokio::sync::Mutex<()> }
#[derive(Serialize)]
struct Snapshot { settings:schedule::Settings, usage:Option<api::Usage>, error:Option<String>, signed_in:bool, next_anchor:Option<String> }
fn token()->Result<String,String>{credentials::load()}
fn snapshot(s:&State)->Snapshot{
 let settings=s.settings.lock().unwrap().clone();
 let next_anchor=schedule::next_anchor(chrono::Local::now(),&settings.rules).map(|v|v.to_rfc3339());
 Snapshot{settings,usage:s.usage.lock().unwrap().clone(),error:s.error.lock().unwrap().clone(),signed_in:token().is_ok(),next_anchor}
}
#[tauri::command]fn get_snapshot(s:tauri::State<'_,Arc<State>>)->Snapshot{snapshot(&s)}
#[tauri::command]fn save_settings(s:tauri::State<'_,Arc<State>>,mut settings:schedule::Settings)->Result<Snapshot,String>{
 settings.mode=schedule::Mode::Off;
 {let db=s.db.lock().unwrap();storage::save_settings(&db,&settings)?;}
 *s.settings.lock().unwrap()=settings;Ok(snapshot(&s))
}
#[tauri::command]async fn set_access_token(s:tauri::State<'_,Arc<State>>,token:String)->Result<Snapshot,String>{
 let v=token.trim();if v.len()<12||v.len()>16384{return Err("無效 Token".into())}
 let _guard=s.updating.lock().await;
 credentials::save(v)?;Ok(snapshot(&s))
}
#[tauri::command]async fn login_with_chatgpt(app:AppHandle,s:tauri::State<'_,Arc<State>>)->Result<Snapshot,String>{
 let _busy=s.login_busy.try_lock().map_err(|_|"已有登入程序進行中")?;
 let session=oauth::browser_login().await?;
 {
  let _guard=s.updating.lock().await;
  oauth::save_session(&session)?;
 }
 *s.error.lock().unwrap()=None;
 let _=app.emit("credits:updated",());
 Ok(snapshot(&s))
}
#[tauri::command]async fn sign_out(s:tauri::State<'_,Arc<State>>)->Result<Snapshot,String>{
 let _guard=s.updating.lock().await;
 credentials::sign_out()?;
 *s.usage.lock().unwrap()=None;
 Ok(snapshot(&s))
}
async fn update(s:&State)->Result<api::Usage,String>{
 let _lock=s.updating.lock().await;
 let access=oauth::valid_access().await?;
 let value=api::fetch_usage(&access.token,access.account_id.as_deref()).await?;
 {let db=s.db.lock().unwrap();storage::add_sample(&db,value.five_hour,value.weekly,value.five_reset.as_deref(),value.weekly_reset.as_deref())?;
 let _=storage::cleanup(&db,s.settings.lock().unwrap().retention_days);}
 *s.usage.lock().unwrap()=Some(value.clone());*s.error.lock().unwrap()=None;Ok(value)
}
#[tauri::command]async fn refresh(app:AppHandle,s:tauri::State<'_,Arc<State>>)->Result<api::Usage,String>{
 let result=update(&s).await;if let Err(ref e)=result{*s.error.lock().unwrap()=Some(e.clone());}let _=app.emit("credits:updated",());result
}
#[tauri::command]fn get_history(s:tauri::State<'_,Arc<State>>)->Result<Vec<serde_json::Value>,String>{storage::history(&s.db.lock().unwrap())}
pub fn run(){
 tauri::Builder::default().setup(|app|{
  let handle=app.handle().clone();
  let path=handle.path().app_data_dir()?;std::fs::create_dir_all(&path)?;
  let db=storage::open(&path.join("codexcredits.sqlite3")).map_err(std::io::Error::other)?;
  let mut settings=storage::get_settings(&db);settings.mode=schedule::Mode::Off;
  let s=Arc::new(State{db:Mutex::new(db),settings:Mutex::new(settings),usage:Mutex::new(None),error:Mutex::new(None),updating:tokio::sync::Mutex::new(()),login_busy:tokio::sync::Mutex::new(())});
  app.manage(s.clone());
  let show=MenuItem::with_id(app,"show","顯示",true,None::<&str>)?;
  let quit=MenuItem::with_id(app,"quit","結束並停止監測",true,None::<&str>)?;
  let menu=Menu::with_items(app,&[&show,&quit])?;
  let icon=tauri::image::Image::new_owned(vec![46_u8,122,222,255].repeat(256),16,16);
  TrayIconBuilder::new().icon(icon).menu(&menu).on_menu_event(|app,event|match event.id.as_ref(){
    "show"=>{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}},
    "quit"=>app.exit(0),_=>{}
  }).build(app)?;
  tauri::async_runtime::spawn(async move{
   loop{let mins=s.settings.lock().unwrap().sampling_minutes.clamp(1,1440);
    tokio::time::sleep(Duration::from_secs(mins*60)).await;
    if token().is_err(){continue;}
    if let Err(e)=update(&s).await{*s.error.lock().unwrap()=Some(e);}
    let _=handle.emit("credits:updated",());
   }
  });Ok(())
 }).on_window_event(|window,event|{
  if let WindowEvent::CloseRequested{api,..}=event{api.prevent_close();let _=window.hide();}
 }).invoke_handler(tauri::generate_handler![get_snapshot,save_settings,set_access_token,login_with_chatgpt,sign_out,refresh,get_history])
 .run(tauri::generate_context!()).expect("Unable to start CodexCredits");
}
