use std::path::Path;
use chrono::Utc;
use rusqlite::{params,Connection};
use crate::schedule::Settings;

pub fn open(path:&Path)->Result<Connection,String>{
 let conn=Connection::open(path).map_err(|e|e.to_string())?;
 conn.execute_batch("PRAGMA journal_mode=WAL;
  CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY,value TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS samples(id INTEGER PRIMARY KEY,at TEXT NOT NULL,five_hour REAL,weekly REAL,five_reset TEXT,weekly_reset TEXT);
  CREATE TABLE IF NOT EXISTS events(id INTEGER PRIMARY KEY,at TEXT NOT NULL,kind TEXT NOT NULL,detail TEXT NOT NULL);
  CREATE INDEX IF NOT EXISTS idx_samples_at ON samples(at);
  CREATE INDEX IF NOT EXISTS idx_events_at ON events(at);").map_err(|e|e.to_string())?;
 Ok(conn)
}
pub fn get_meta(db:&Connection,key:&str)->Option<String>{
 db.query_row("SELECT value FROM kv WHERE key=?1",params![key],|r|r.get::<_,String>(0)).ok()
}
pub fn set_meta(db:&Connection,key:&str,value:Option<&str>)->Result<(),String>{
 if let Some(text)=value{
  db.execute("INSERT INTO kv(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,text])
   .map_err(|e|e.to_string())?;
 }else{
  db.execute("DELETE FROM kv WHERE key=?1",params![key]).map_err(|e|e.to_string())?;
 }
 Ok(())
}
pub fn get_settings(db:&Connection)->Settings{
 db.query_row("SELECT value FROM kv WHERE key='settings'",[],|r|r.get::<_,String>(0))
  .ok().and_then(|x|serde_json::from_str(&x).ok()).unwrap_or_default()
}
pub fn save_settings(db:&Connection,s:&Settings)->Result<(),String>{
 if !s.valid(){return Err("設定值超出允許範圍".into());}
 let json=serde_json::to_string(s).map_err(|e|e.to_string())?;
 db.execute("INSERT INTO kv(key,value) VALUES('settings',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
  params![json]).map_err(|e|e.to_string())?;
 Ok(())
}
pub fn add_sample(db:&Connection,five:Option<f64>,weekly:Option<f64>,f_reset:Option<&str>,w_reset:Option<&str>)->Result<(),String>{
 // Android: coalesce snapshots within 30 seconds to avoid noisy duplicate samples.
 let old:Option<(i64,String)>=db.query_row("SELECT id,at FROM samples ORDER BY id DESC LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).ok();
 let now=Utc::now();
 if let Some((id,at))=old{
  if chrono::DateTime::parse_from_rfc3339(&at).is_ok_and(|t|now.signed_duration_since(t).num_seconds().abs()<30){
   db.execute("UPDATE samples SET at=?1,five_hour=?2,weekly=?3,five_reset=?4,weekly_reset=?5 WHERE id=?6",
    params![now.to_rfc3339(),five,weekly,f_reset,w_reset,id]).map_err(|e|e.to_string())?;return Ok(());
  }
 }
 db.execute("INSERT INTO samples(at,five_hour,weekly,five_reset,weekly_reset) VALUES(?1,?2,?3,?4,?5)",
  params![now.to_rfc3339(),five,weekly,f_reset,w_reset]).map_err(|e|e.to_string())?;Ok(())
}
pub fn event(db:&Connection,kind:&str,detail:&str)->Result<(),String>{
 let duplicate:Option<(String,String)>=db.query_row("SELECT at,detail FROM events WHERE kind=?1 ORDER BY id DESC LIMIT 1",
  params![kind],|r|Ok((r.get(0)?,r.get(1)?))).ok();
 if let Some((at,old))=duplicate{
  if old==detail&&chrono::DateTime::parse_from_rfc3339(&at).is_ok_and(|t|Utc::now().signed_duration_since(t).num_seconds().abs()<120){return Ok(());}
 }
 db.execute("INSERT INTO events(at,kind,detail) VALUES(?1,?2,?3)",
  params![Utc::now().to_rfc3339(),kind,detail]).map_err(|e|e.to_string())?;Ok(())
}
pub fn cleanup(db:&Connection,days:i64)->Result<(),String>{
 let cutoff=(Utc::now()-chrono::Duration::days(days)).to_rfc3339();
 db.execute("DELETE FROM samples WHERE at < ?1",params![cutoff]).map_err(|e|e.to_string())?;
 db.execute("DELETE FROM events WHERE at < ?1",params![cutoff]).map_err(|e|e.to_string())?;Ok(())
}
pub fn history(db:&Connection)->Result<Vec<serde_json::Value>,String>{
 let mut stmt=db.prepare("SELECT at,five_hour,weekly,five_reset,weekly_reset FROM samples ORDER BY id DESC LIMIT 1440").map_err(|e|e.to_string())?;
 let rows=stmt.query_map([],|r|Ok(serde_json::json!({
  "at":r.get::<_,String>(0)?,"five_hour":r.get::<_,Option<f64>>(1)?,"weekly":r.get::<_,Option<f64>>(2)?,
  "five_reset":r.get::<_,Option<String>>(3)?,"weekly_reset":r.get::<_,Option<String>>(4)?
 }))).map_err(|e|e.to_string())?;
 let mut out=Vec::new();for row in rows{out.push(row.map_err(|e|e.to_string())?);}out.reverse();Ok(out)
}
pub fn events(db:&Connection)->Result<Vec<serde_json::Value>,String>{
 let mut stmt=db.prepare("SELECT at,kind,detail FROM events ORDER BY id DESC LIMIT 60").map_err(|e|e.to_string())?;
 let rows=stmt.query_map([],|r|Ok(serde_json::json!({"at":r.get::<_,String>(0)?,"kind":r.get::<_,String>(1)?,"detail":r.get::<_,String>(2)?})))
  .map_err(|e|e.to_string())?;
 let mut out=Vec::new();for row in rows{out.push(row.map_err(|e|e.to_string())?);}Ok(out)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn persisted_history_survives_reopening_database(){
  let path=std::env::temp_dir().join(format!("codexcredits-history-test-{}.sqlite3",uuid::Uuid::new_v4()));
  {
   let db=open(&path).unwrap();
   add_sample(&db,Some(37.),Some(56.),Some("2026-10-18T00:00:00Z"),None).unwrap();
   event(&db,"restart","本機歷程需在重新啟動後保留").unwrap();
  }
  {
   let db=open(&path).unwrap();
   let rows=history(&db).unwrap();
   let logs=events(&db).unwrap();
   assert_eq!(rows.len(),1);
   assert_eq!(rows[0]["five_hour"],37.0);
   assert_eq!(logs.len(),1);
   assert_eq!(logs[0]["kind"],"restart");
  }
  let _=std::fs::remove_file(path);
 }
 #[test]fn roundtrip(){
  let db=Connection::open_in_memory().unwrap();
  db.execute_batch("CREATE TABLE kv(key TEXT PRIMARY KEY,value TEXT NOT NULL);CREATE TABLE samples(id INTEGER PRIMARY KEY,at TEXT,five_hour REAL,weekly REAL,five_reset TEXT,weekly_reset TEXT);CREATE TABLE events(id INTEGER PRIMARY KEY,at TEXT,kind TEXT,detail TEXT);").unwrap();
  let s=get_settings(&db);save_settings(&db,&s).unwrap();assert_eq!(get_settings(&db).sampling_minutes,10);
  add_sample(&db,Some(27.),Some(45.),None,None).unwrap();assert_eq!(history(&db).unwrap().len(),1);
  event(&db,"test","測試").unwrap();assert_eq!(events(&db).unwrap().len(),1);
  set_meta(&db,"banked_armed",Some("credit-test")).unwrap();assert_eq!(get_meta(&db,"banked_armed"),Some("credit-test".into()));
  set_meta(&db,"banked_armed",None).unwrap();assert_eq!(get_meta(&db,"banked_armed"),None);
 }
}
