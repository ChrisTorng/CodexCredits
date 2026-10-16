use std::path::Path;
use chrono::Utc;
use rusqlite::{params, Connection};
use crate::schedule::Settings;

pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS samples (id INTEGER PRIMARY KEY, at TEXT NOT NULL, five_hour REAL, weekly REAL, five_reset TEXT, weekly_reset TEXT);
        CREATE TABLE IF NOT EXISTS events (id INTEGER PRIMARY KEY, at TEXT NOT NULL, kind TEXT NOT NULL, detail TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS idx_samples_at ON samples(at);
        CREATE INDEX IF NOT EXISTS idx_events_at ON events(at);").map_err(|e| e.to_string())?;
    Ok(conn)
}
pub fn get_settings(conn: &Connection) -> Settings {
    conn.query_row("SELECT value FROM kv WHERE key='settings'", [], |r| r.get::<_,String>(0))
        .ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
pub fn save_settings(conn: &Connection, settings: &Settings) -> Result<(), String> {
    if !settings.valid() { return Err("設定值超出允許範圍".into()); }
    let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO kv(key,value) VALUES('settings',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![json]).map_err(|e|e.to_string())?;
    Ok(())
}
pub fn add_sample(conn: &Connection, five: Option<f64>, weekly: Option<f64>, five_reset: Option<&str>, weekly_reset: Option<&str>) -> Result<(), String> {
    conn.execute("INSERT INTO samples(at,five_hour,weekly,five_reset,weekly_reset) VALUES(?1,?2,?3,?4,?5)",
        params![Utc::now().to_rfc3339(),five,weekly,five_reset,weekly_reset]).map_err(|e|e.to_string())?;
    Ok(())
}
pub fn cleanup(conn: &Connection, days: i64) -> Result<(), String> {
    let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    conn.execute("DELETE FROM samples WHERE at < ?1", params![cutoff]).map_err(|e|e.to_string())?;
    conn.execute("DELETE FROM events WHERE at < ?1", params![cutoff]).map_err(|e|e.to_string())?;
    Ok(())
}
pub fn history(conn: &Connection) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = conn.prepare("SELECT at,five_hour,weekly FROM samples ORDER BY id DESC LIMIT 1440").map_err(|e|e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(serde_json::json!({
        "at":r.get::<_,String>(0)?,
        "five_hour":r.get::<_,Option<f64>>(1)?,
        "weekly":r.get::<_,Option<f64>>(2)?
    }))).map_err(|e|e.to_string())?;
    let mut out = Vec::new();
    for row in rows { out.push(row.map_err(|e|e.to_string())?); }
    out.reverse();
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn stores_defaults_and_samples() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE kv(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE samples(id INTEGER PRIMARY KEY,at TEXT,five_hour REAL,weekly REAL,five_reset TEXT,weekly_reset TEXT);").unwrap();
        let s = get_settings(&conn);
        assert!(s.valid());
        save_settings(&conn,&s).unwrap();
        add_sample(&conn,Some(20.0),Some(5.0),None,None).unwrap();
        assert_eq!(history(&conn).unwrap().len(),1);
    }
}
