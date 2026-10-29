use serde::{Deserialize,Serialize};
use serde_json::Value;
#[cfg(target_os="android")]
fn native_json<T:serde::de::DeserializeOwned>(raw:&str,service:&str)->Result<T,String>{
 let value:Value=serde_json::from_str(raw).map_err(|e|format!("{service} 回應格式錯誤: {e}"))?;
 if let Some(error)=value["error"].as_str(){return Err(format!("{service}: {error}"));}
 serde_json::from_value(value).map_err(|e|format!("{service} 資料不完整: {e}"))
}
const USAGE_URL:&str="https://chatgpt.com/backend-api/wham/usage";
const RESET_URL:&str="https://chatgpt.com/backend-api/wham/rate-limit-reset-credits";
const CONSUME_URL:&str="https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume";
#[derive(Debug,Clone,Serialize,Deserialize,Default)]
pub struct Usage {
 pub five_hour:Option<f64>, pub weekly:Option<f64>, pub five_reset:Option<String>,
 pub weekly_reset:Option<String>,pub plan:String,pub allowed:bool,pub limit_reached:bool,
 pub primary_confirmed:bool,pub checked_at:String,
}
#[derive(Debug,Clone,Serialize,Deserialize,Default)]
pub struct Credit {pub id:String,pub title:String,pub description:String,pub expires_at:String}
#[derive(Debug,Clone,Serialize,Deserialize,Default)]
pub struct Credits {pub count:i64,pub available:Vec<Credit>}
#[derive(Debug,Clone,Serialize,Deserialize,Default)]
pub struct Global {pub last_reset_at:Option<String>,pub age_days:Option<f64>,pub p24:Option<i64>,pub p48:Option<i64>,pub avg_days:Option<f64>,pub median_days:Option<f64>,pub median_estimate_at:Option<String>}
fn bucket(root:&Value,name:&str)->(Option<f64>,Option<String>){
 let b=&root[name];
 let pct=b["used_percent"].as_f64().filter(|n|n.is_finite()).map(|n|n.clamp(0.,100.));
 let reset=b["reset_at"].as_str().map(str::to_owned)
  .or_else(||b["reset_at"].as_i64().and_then(|n|chrono::DateTime::from_timestamp(n,0).map(|t|t.to_rfc3339())));
 (pct,reset)
}
fn http()->Result<reqwest::Client,String>{reqwest::Client::builder().timeout(std::time::Duration::from_secs(25)).build().map_err(|e|e.to_string())}
fn authorized(client:&reqwest::Client,url:&str,token:&str,account:Option<&str>)->reqwest::RequestBuilder{
 let mut r=client.get(url).bearer_auth(token).header("User-Agent","CodexCredits/0.2");
 if let Some(id)=account {r=r.header("ChatGPT-Account-Id",id);}
 r
}
pub async fn fetch_usage(token:&str,account_id:Option<&str>)->Result<Usage,String>{
 #[cfg(target_os="android")]
 {
  let _=(token,account_id);
  let raw=tokio::task::spawn_blocking(crate::android_bridge::native_usage)
   .await.map_err(|e|format!("Android 配額工作失敗: {e}"))??;
  return native_json(&raw,"Android 配額查詢");
 }
 #[cfg(not(target_os="android"))]
 {

 let client=http()?;
 let response=authorized(&client,USAGE_URL,token,account_id).send().await.map_err(|e|e.to_string())?;
 if !response.status().is_success(){return Err(format!("配額查詢 HTTP {}",response.status()));}
 let json:Value=response.json().await.map_err(|e|e.to_string())?;
 let limits=&json["rate_limit"];
 let (five_hour,five_reset)=bucket(limits,"primary_window");
 let (weekly,weekly_reset)=bucket(limits,"secondary_window");
 Ok(Usage{five_hour,weekly,five_reset,weekly_reset,
  plan:json["plan_type"].as_str().unwrap_or("—").to_owned(),
  allowed:limits["allowed"].as_bool().unwrap_or(true),
  limit_reached:limits["limit_reached"].as_bool().unwrap_or(false),
  primary_confirmed:false,
  checked_at:chrono::Utc::now().to_rfc3339()})
 }
}
pub async fn fetch_credits(token:&str,account_id:Option<&str>)->Result<Credits,String>{
 #[cfg(target_os="android")]
 {
  let _=(token,account_id);
  let raw=tokio::task::spawn_blocking(crate::android_bridge::native_credits)
   .await.map_err(|e|format!("Android Banked Reset 工作失敗: {e}"))??;
  return native_json(&raw,"Android Banked Reset 查詢");
 }
 #[cfg(not(target_os="android"))]
 {

 let client=http()?;
 let response=authorized(&client,RESET_URL,token,account_id).send().await.map_err(|e|e.to_string())?;
 if !response.status().is_success(){return Err(format!("Banked Reset HTTP {}",response.status()));}
 let json:Value=response.json().await.map_err(|e|e.to_string())?;
 let available:Vec<Credit>=json["credits"].as_array().into_iter().flatten()
  .filter(|v|v["status"].as_str().is_some_and(|s|s.eq_ignore_ascii_case("available")))
  .filter_map(|v|Some(Credit{
    id:v["id"].as_str()?.to_string(),title:v["title"].as_str().unwrap_or("Full reset").into(),
    description:v["description"].as_str().unwrap_or("").into(),
    expires_at:v["expires_at"].as_str().unwrap_or("").into()
  })).collect();
 Ok(Credits{count:json["available_count"].as_i64().unwrap_or(available.len() as i64),available})
 }
}
pub async fn consume_credit(token:&str,account_id:Option<&str>,credit_id:&str)->Result<i64,String>{
 let client=http()?;
 let mut req=client.post(CONSUME_URL).bearer_auth(token).header("User-Agent","CodexCredits/0.2")
  .json(&serde_json::json!({"redeem_request_id":uuid::Uuid::new_v4().to_string(),"credit_id":credit_id}));
 if let Some(id)=account_id{req=req.header("ChatGPT-Account-Id",id);}
 let response=req.send().await.map_err(|e|e.to_string())?;
 if !response.status().is_success(){return Err(format!("Banked Reset HTTP {}",response.status()));}
 let json:Value=response.json().await.map_err(|e|e.to_string())?;
 Ok(json["windows_reset"].as_i64().unwrap_or(0))
}
pub async fn trigger_minimal(token:&str,account_id:Option<&str>)->Result<(),String>{
 let client=reqwest::Client::builder().timeout(std::time::Duration::from_secs(65)).build().map_err(|e|e.to_string())?;
 let payload=serde_json::json!({"model":"gpt-5.6-luna","store":false,"stream":true,"instructions":"Reply .",
  "input":[{"role":"user","content":[{"type":"input_text","text":"."}]}],
  "reasoning":{"effort":"none"},"text":{"verbosity":"low"}});
 let mut req=client.post("https://chatgpt.com/backend-api/codex/responses")
  .bearer_auth(token).header("User-Agent","CodexCredits/0.2")
  .header("originator","codex-quota-trigger-android").header("OpenAI-Beta","responses=experimental")
  .header("Accept","text/event-stream").json(&payload);
 if let Some(id)=account_id{req=req.header("ChatGPT-Account-Id",id);}
 let response=req.send().await.map_err(|e|e.to_string())?;
 if !response.status().is_success(){return Err(format!("立即觸發 HTTP {}",response.status()));}
 Ok(())
}
pub async fn global_reset()->Result<(Global,Vec<i64>),String>{
 let client=http()?;
 let forecast=client.get("https://codex-reset.com/api/forecast")
  .header("User-Agent","CodexCredits/0.2").send().await.map_err(|e|e.to_string())?;
 if !forecast.status().is_success(){return Err(format!("Global Reset HTTP {}",forecast.status()));}
 let v:Value=forecast.json().await.map_err(|e|e.to_string())?;
 let mut last=v["last_reset_at"].as_str().map(str::to_owned);
 let mut gaps:Vec<f64>=Vec::new();
 let mut verified_resets:Vec<i64>=Vec::new();
 // Android source: only announced reset events in the public timeline count.
 if let Ok(response)=client.get("https://codex-reset.com/api/timeline")
     .header("User-Agent","CodexCredits/0.2").send().await{
  if let Ok(timeline)=response.json::<Value>().await{
   // Verified observed global resets are frequently tagged "operator-observed"
   // with medium confidence. "archive/high" alone excludes real resets.
   verified_resets=timeline["events"].as_array().into_iter().flatten()
    .filter(|e|e["group"].as_str()==Some("reset")
      && e["announcement_state"].as_str()==Some("announced")
      && e["confirmed"].as_bool()==Some(true)
      && (e["observation_result"].as_str()==Some("reset_observed")
       || (e["source"].as_str()==Some("archive")
        && e["confidence"].as_str()==Some("high"))))
    .filter_map(|e|e["effective_at"].as_str()
       .or_else(||e["observed_at"].as_str())
       .or_else(||e["announced_at"].as_str()))
    .filter_map(|iso|chrono::DateTime::parse_from_rfc3339(iso).ok())
    .map(|t|t.timestamp()).collect();
   verified_resets.sort_unstable();
   verified_resets.dedup();
   let mut resets:Vec<chrono::DateTime<chrono::FixedOffset>>=timeline["events"].as_array()
    .into_iter().flatten()
    .filter(|e|e["group"].as_str().is_some_and(|v|v.eq_ignore_ascii_case("reset"))
      &&e["announcement_state"].as_str().is_some_and(|v|v.eq_ignore_ascii_case("announced")))
    .filter_map(|e|e["effective_at"].as_str().or_else(||e["announced_at"].as_str()))
    .filter_map(|iso|chrono::DateTime::parse_from_rfc3339(iso).ok())
    .collect();
   resets.sort();resets.dedup();
   if last.is_none(){last=resets.last().map(|d|d.to_rfc3339());}
   let start=resets.len().saturating_sub(10).max(1);
   for i in start..resets.len(){
    let diff=resets[i].signed_duration_since(resets[i-1]).num_seconds();
    if diff>0{gaps.push(diff as f64/86400.)}
   }
  }
 }
 gaps.sort_by(|a,b|a.total_cmp(b));
 let avg=if gaps.is_empty(){None}else{Some(gaps.iter().sum::<f64>()/gaps.len() as f64)};
 let median=if gaps.is_empty(){None}else{
  let n=gaps.len();Some(if n%2!=0{gaps[n/2]}else{(gaps[n/2-1]+gaps[n/2])/2.})};
 let estimate=last.as_deref().and_then(|iso|chrono::DateTime::parse_from_rfc3339(iso).ok())
  .zip(median).and_then(|(time,days)|{
   chrono::Duration::try_seconds((days*86400.) as i64).map(|delta|time+delta)
  }).map(|time|time.to_rfc3339());
 Ok((Global{last_reset_at:last,age_days:v["age_days"].as_f64(),
  p24:v["probabilities"]["rounded_24h"].as_i64(),
  p48:v["probabilities"]["rounded_48h"].as_i64(),
  avg_days:avg,median_days:median,median_estimate_at:estimate},verified_resets))
}
pub async fn ntfy_send(topic:&str,title:&str,message:&str)->Result<(),String>{
 let url=url::Url::parse(topic).map_err(|_|"ntfy 網址格式錯誤")?;
 let name=url.path().trim_matches('/');
 if url.scheme()!="https"||url.host_str()!=Some("ntfy.sh")||url.query().is_some()
  ||url.fragment().is_some()||name.is_empty()||name.contains('/')||
  !name.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
 {return Err("目前僅支援 https://ntfy.sh/<專屬 topic>".into());}
 let response=http()?.post("https://ntfy.sh/").json(&serde_json::json!({
   "topic":name,"title":title,"message":message
 })).send().await.map_err(|e|e.to_string())?;
 if !response.status().is_success(){return Err(format!("ntfy HTTP {}",response.status()));}
 Ok(())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn remaining_is_reverse_of_usage(){let v=serde_json::json!({"primary_window":{"used_percent":27.0,"reset_at":1740000000}});let(p,r)=bucket(&v,"primary_window");assert_eq!(100.-p.unwrap(),73.);assert!(r.is_some());}
 #[test]fn inactive_missing_window_is_none(){let(p,r)=bucket(&Value::Null,"primary_window");assert_eq!(p,None);assert_eq!(r,None);}
}
