use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub five_hour: Option<f64>,
    pub weekly: Option<f64>,
    pub five_reset: Option<String>,
    pub weekly_reset: Option<String>,
}
fn bucket(root: &Value, name: &str) -> (Option<f64>, Option<String>) {
    let b = &root[name];
    let pct = b["used_percent"].as_f64();
    let reset = b["reset_at"].as_str().map(str::to_owned)
        .or_else(||b["reset_at"].as_i64().and_then(|n|chrono::DateTime::from_timestamp(n,0).map(|t|t.to_rfc3339())));
    (pct,reset)
}
pub async fn fetch_usage(access_token: &str, account_id:Option<&str>) -> Result<Usage,String> {
    let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(25)).build().map_err(|e|e.to_string())?;
    let mut request=http.get("https://chatgpt.com/backend-api/wham/usage")
        .bearer_auth(access_token).header("User-Agent","CodexCredits/0.1");
    if let Some(id)=account_id{ request=request.header("ChatGPT-Account-Id",id); }
    let response=request.send().await.map_err(|e|e.to_string())?;
    if !response.status().is_success() { return Err(format!("API HTTP {}",response.status())); }
    let json:Value = response.json().await.map_err(|e|e.to_string())?;
    let limits = &json["rate_limit"];
    let (five_hour,five_reset) = bucket(limits,"primary_window");
    let (weekly,weekly_reset) = bucket(limits,"secondary_window");
    Ok(Usage{five_hour,weekly,five_reset,weekly_reset})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn extracts_percent_and_timestamp() {
        let json=serde_json::json!({"primary_window":{"used_percent":12.0,"reset_at":1740000000}});
        let (percent,reset)=bucket(&json,"primary_window");
        assert_eq!(percent,Some(12.0));assert!(reset.is_some());
    }
}
