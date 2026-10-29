//! One-shot, transactional conversion of the original Android SharedPreferences.
//! Does not mutate/delete original data; a failed conversion can be retried.
use chrono::{DateTime,Utc};
use rusqlite::{params,Connection};
use serde_json::{Value,json};
use crate::{schedule::{Settings,Mode,Rule,Override},storage};

fn as_date(ms:i64)->Option<String>{
    DateTime::<Utc>::from_timestamp_millis(ms).map(|d|d.to_rfc3339())
}
fn used_percent(v:&Value,key:&str)->Option<f64>{
    v.get(key).and_then(Value::as_f64).filter(|n|n.is_finite()).map(|n|n.clamp(0.,100.))
}
fn old_json(p:&Value,key:&str,default:&str)->Result<Value,String>{
    let raw=p.get(key).and_then(Value::as_str).unwrap_or(default);
    serde_json::from_str(raw).map_err(|e|format!("舊版 {key} 損毀: {e}"))
}
fn converted_rules(src:&Value)->Vec<Rule>{
    let mut out=Vec::new();
    for (i,v) in src.as_array().into_iter().flatten().enumerate(){
        let mut rule=Rule::default();
        rule.id=v.get("id").and_then(Value::as_str).filter(|x|!x.is_empty())
            .map(str::to_owned).unwrap_or_else(||format!("migrated-{i}"));
        rule.enabled=v.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        rule.hour=v.get("hour").and_then(Value::as_u64).unwrap_or(6).min(23) as u32;
        rule.minute=v.get("minute").and_then(Value::as_u64).unwrap_or(0).min(59) as u32;
        rule.auto_count=v.get("autoCount").and_then(Value::as_u64).unwrap_or(0).min(6) as u8;
        if let Some(bits)=v.get("days").and_then(Value::as_str){
            rule.days=std::array::from_fn(|day|bits.as_bytes().get(day)==Some(&b'1'));
        }
        out.push(rule);
    }
    out
}
fn settings(p:&Value)->Result<Settings,String>{
    let mut s=Settings::default();
    let old_mode=p.get("scheduler_mode").and_then(Value::as_str);
    s.mode=match old_mode {
        Some("custom")=>Mode::Custom,
        Some("auto_reset")=>Mode::Auto,
        Some("off")=>Mode::Off,
        _ if p.get("scheduler_enabled").and_then(Value::as_bool)==Some(true)=>Mode::Auto,
        _=>Mode::Off
    };
    let m=p.get("sample_interval_minutes").and_then(Value::as_u64).unwrap_or(10);
    s.sampling_minutes=if [10,15,30,60].contains(&m){m}else{10};
    s.theme=p.get("theme_mode").and_then(Value::as_str)
        .filter(|s|["system","light","dark"].contains(s)).unwrap_or("system").into();
    s.native_notify_enabled=p.get("android_notifications_enabled")
        .and_then(Value::as_bool).unwrap_or(true);
    let topic=p.get("ntfy_url").and_then(Value::as_str).unwrap_or("");
    if topic.starts_with("https://ntfy.sh/"){s.ntfy_topic=topic.to_string();}
    let weekly=old_json(p,"schedule_weekly_v2",
        p.get("schedule_rules_v1").and_then(Value::as_str).unwrap_or("[]"))?;
    if p.get("schedule_weekly_v2").is_some()||p.get("schedule_rules_v1").is_some(){
        s.rules=converted_rules(&weekly);
    }
    let overrides=old_json(p,"schedule_day_overrides_v2","{}")?;
    if let Some(object)=overrides.as_object(){
        for (date,override_day) in object{
            s.overrides.insert(date.clone(),Override{
                enabled:override_day.get("enabled").and_then(Value::as_bool).unwrap_or(false),
                rules:converted_rules(override_day.get("rules").unwrap_or(&Value::Null))
            });
        }
    }
    // Legacy rules can be ambiguous due to historic scheduling edits.
    // Preserve them, but never silently activate an invalid credit-consuming plan.
    if !s.valid(){
        s.mode=Mode::Off;
        // An invalid legacy plan must not prevent importing valid tokens/history.
        // The original SharedPreferences remain an untouched recovery copy.
        if !s.valid(){s.rules=vec![Rule::default()];s.overrides.clear();}
    }
    Ok(s)
}
fn import_tokens(tokens:&Value)->Result<(),String>{
    if crate::credentials::load().is_ok(){return Ok(());}
    let Some(access)=tokens.get("access_token").and_then(Value::as_str)else{return Ok(())};
    let Some(account)=tokens.get("account_id").and_then(Value::as_str)else{return Ok(())};
    let refresh=tokens.get("refresh_token").and_then(Value::as_str);
    let expires=tokens.get("expires_at_ms").and_then(Value::as_str)
        .and_then(|v|v.parse::<i64>().ok()).unwrap_or(0)/1000;
    let auth=json!({"access_token":access,"account_id":account,
        "refresh_token":refresh,"expires_at":expires});
    crate::credentials::save(&auth.to_string())
}
pub fn import(db:&Connection)->Result<bool,String>{
    if storage::get_meta(db,"android_legacy_import_v1").is_some(){
        sync_native(db)?;
        return Ok(false);
    }
    let src=crate::android_bridge::export_legacy()?;
    let p=src.get("prefs").ok_or("舊版設定缺少 prefs")?;
    let has_old=p.as_object().is_some_and(|o|!o.is_empty());
    let old_samples=old_json(p,"usage_samples_v1","[]")?;
    let cutover=storage::get_meta(db,"android_quota_cutover_ms_v2").and_then(|v|v.parse::<i64>().ok()).unwrap_or(i64::MIN);
    let old_events=old_json(p,"usage_history_events_v1","[]")?;
    let converted=settings(p)?;
    import_tokens(&src["tokens"])?;
    db.execute_batch("BEGIN IMMEDIATE").map_err(|e|e.to_string())?;
    let done=(||->Result<(),String>{
        if has_old{storage::save_settings(db,&converted)?;}
        if let Some(rows)=old_samples.as_array(){
            for v in rows{
                let Some(at_ms)=v.get("at").and_then(Value::as_i64)else{continue};
                if at_ms<cutover{continue;}
                let Some(at)=as_date(at_ms)else{continue};
                let five=used_percent(v,"p");
                let weekly=used_percent(v,"w");
                let reset=|key:&str|v.get(key).and_then(Value::as_i64).and_then(as_date);
                db.execute("INSERT INTO samples(at,five_hour,weekly,five_reset,weekly_reset) VALUES(?1,?2,?3,?4,?5)",
                    params![at,five,weekly,reset("pr"),reset("wr")]).map_err(|e|e.to_string())?;
            }
        }
        if let Some(events)=old_events.as_array(){
            for v in events{
                let Some(at)=v.get("at").and_then(Value::as_i64).and_then(as_date)else{continue};
                let kind=v.get("type").and_then(Value::as_str).unwrap_or("legacy");
                let label=v.get("label").and_then(Value::as_str).unwrap_or("");
                let source=v.get("source").and_then(Value::as_str).unwrap_or("");
                db.execute("INSERT INTO events(at,kind,detail) VALUES(?1,?2,?3)",
                    params![at,kind,format!("{label} ({source})")]).map_err(|e|e.to_string())?;
            }
        }
        // Commit marker and imported rows atomically; the old data stays untouched.
        storage::set_meta(db,"android_legacy_import_v1",Some("done"))?;
        Ok(())
    })();
    if done.is_ok(){db.execute_batch("COMMIT").map_err(|e|e.to_string())?}
    else{let _=db.execute_batch("ROLLBACK");}
    done?;
    Ok(has_old)
}

/// Merge new samples/events written by the original Android background scheduler.
/// Exact timestamps are stable keys; duplicate snapshots are ignored.
pub fn sync_native(db:&Connection)->Result<(),String>{
    if storage::get_meta(db,"android_legacy_import_v1").is_none(){return Ok(());}
    let src=crate::android_bridge::export_legacy()?;
    let p=src.get("prefs").ok_or("舊版資料缺少 prefs")?;
    let old_samples=old_json(p,"usage_samples_v1","[]")?;
    let cutover=storage::get_meta(db,"android_quota_cutover_ms_v2").and_then(|v|v.parse::<i64>().ok()).unwrap_or(i64::MIN);
    let old_events=old_json(p,"usage_history_events_v1","[]")?;
    db.execute_batch("BEGIN IMMEDIATE").map_err(|e|e.to_string())?;
    let done=(||->Result<(),String>{
        if let Some(rows)=old_samples.as_array(){
            for v in rows {
                let Some(at_ms)=v.get("at").and_then(Value::as_i64)else{continue};
                if at_ms<cutover{continue;}
                let Some(at)=as_date(at_ms)else{continue};
                let five=used_percent(v,"p");
                let weekly=used_percent(v,"w");
                let reset=|key:&str|v.get(key).and_then(Value::as_i64).and_then(as_date);
                db.execute("INSERT INTO samples(at,five_hour,weekly,five_reset,weekly_reset)
                    SELECT ?1,?2,?3,?4,?5
                    WHERE NOT EXISTS(SELECT 1 FROM samples WHERE at=?1)",
                    params![at,five,weekly,reset("pr"),reset("wr")])
                    .map_err(|e|e.to_string())?;
            }
        }
        if let Some(rows)=old_events.as_array(){
            for v in rows {
                let Some(at)=v.get("at").and_then(Value::as_i64).and_then(as_date) else{continue};
                let kind=v.get("type").and_then(Value::as_str).unwrap_or("legacy");
                let label=v.get("label").and_then(Value::as_str).unwrap_or("");
                let source=v.get("source").and_then(Value::as_str).unwrap_or("");
                let detail=format!("{label} ({source})");
                db.execute("INSERT INTO events(at,kind,detail)
                    SELECT ?1,?2,?3
                    WHERE NOT EXISTS(SELECT 1 FROM events WHERE at=?1 AND
                       ((kind=?2 AND detail=?3)
                        OR (?2='unexpected_reset' AND kind='global_reset')))",
                    params![at,kind,detail]).map_err(|e|e.to_string())?;
            }
        }
        Ok(())
    })();
    if done.is_ok(){db.execute_batch("COMMIT").map_err(|e|e.to_string())?}
    else{let _=db.execute_batch("ROLLBACK");}
    done
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn legacy_samples_are_used_percent(){
        let sample=json!({"p":27.,"w":11.});
        assert_eq!(used_percent(&sample,"p"),Some(27.));
        assert_eq!(used_percent(&sample,"w"),Some(11.));
        assert_eq!(100.-used_percent(&sample,"p").unwrap(),73.);
    }
    #[test]fn schedule_and_remaining_conversion(){
        let p=json!({"scheduler_mode":"custom","sample_interval_minutes":15,
            "schedule_weekly_v2":r#"[{"id":"abc","hour":6,"minute":0,"autoCount":3,"days":"1111111"}]"#,
            "theme_mode":"dark"});
        let s=settings(&p).unwrap();assert_eq!(s.sampling_minutes,15);
        assert_eq!(s.rules[0].auto_count,3);
        assert_eq!(s.rules[0].days,[true;7]);
        assert!(matches!(s.mode,Mode::Custom));
        assert_eq!((100.-27.0_f64),73.);
    }
}