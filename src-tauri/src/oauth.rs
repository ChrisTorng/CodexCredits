use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener, time::{timeout,Duration,Instant}};
use url::Url;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const SCOPE: &str = "openid profile email offline_access api.connectors.read api.connectors.invoke";

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Session {
    access_token:String,
    refresh_token:Option<String>,
    account_id:String,
    expires_at:i64,
}
#[derive(Deserialize)]
struct Response {
    access_token:String,
    refresh_token:Option<String>,
    id_token:Option<String>,
    expires_in:Option<i64>,
}
pub struct Access { pub token:String, pub account_id:Option<String> }

fn pkce()->(String,String,String){
    let mut v=[0u8;48];let mut s=[0u8;24];
    OsRng.fill_bytes(&mut v);OsRng.fill_bytes(&mut s);
    let verifier=URL_SAFE_NO_PAD.encode(v);
    (verifier.clone(),URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),URL_SAFE_NO_PAD.encode(s))
}
fn login_url(redirect:&str,challenge:&str,state:&str)->Result<Url,String>{
    let mut u=Url::parse(AUTH_URL).map_err(|e|e.to_string())?;
    u.query_pairs_mut()
        .append_pair("response_type","code")
        .append_pair("client_id",CLIENT_ID)
        .append_pair("redirect_uri",redirect)
        .append_pair("scope",SCOPE)
        .append_pair("code_challenge",challenge)
        .append_pair("code_challenge_method","S256")
        .append_pair("state",state)
        .append_pair("id_token_add_organizations","true")
        .append_pair("codex_cli_simplified_flow","true")
        .append_pair("originator","codex_quota_trigger_android");
    Ok(u)
}
fn account_from_id_token(jwt:&str)->Result<String,String>{
    let b64=jwt.split('.').nth(1).ok_or("ID Token 無效")?;
    let data=URL_SAFE_NO_PAD.decode(b64).map_err(|_|"ID Token 無法解碼")?;
    let json:serde_json::Value=serde_json::from_slice(&data).map_err(|_|"ID Token 格式錯誤")?;
    json["https://api.openai.com/auth"]["chatgpt_account_id"].as_str()
       .or_else(||json["chatgpt_account_id"].as_str())
       .filter(|s|!s.is_empty())
       .map(str::to_owned).ok_or("無法取得 ChatGPT Account ID".into())
}
fn parse_callback(request:&str,expected:&str)->Result<Option<String>,String>{
    let first=request.split("\r\n").next().unwrap_or("");
    let mut words=first.split_whitespace();
    if words.next()!=Some("GET"){return Err("OAuth 回呼方法無效".into());}
    let target=words.next().ok_or("OAuth 回呼缺少路徑")?;
    if !target.starts_with('/'){return Err("OAuth 回呼路徑無效".into());}
    let url=Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_|"OAuth URL 無效")?;
    if url.path()!="/auth/callback"{return Ok(None);}
    let mut code=None;let mut state=None;let mut error=None;
    for (k,v) in url.query_pairs(){
        match k.as_ref(){"code"=>code=Some(v.into_owned()),"state"=>state=Some(v.into_owned()),
            "error"=>error=Some(v.into_owned()),_=>{}}
    }
    if error.is_some(){return Err("使用者取消授權或 OAuth 伺服器拒絕登入".into());}
    let matches=state.as_deref().is_some_and(|v|
        v==expected || v==format!("{expected}.onboarding_entrypoint=life_sciences"));
    if !matches{return Err("OAuth state 驗證失敗".into());}
    code.filter(|v|!v.is_empty()).map(Some).ok_or("授權碼不存在".into())
}
async fn callback(listener:TcpListener,state:&str)->Result<String,String>{
    let until=Instant::now()+Duration::from_secs(300);
    loop {
        let remaining=until.saturating_duration_since(Instant::now());
        if remaining.is_zero(){return Err("登入等待超過 5 分鐘".into());}
        let (mut stream,_)=timeout(remaining,listener.accept()).await
            .map_err(|_|"登入等待超過 5 分鐘")?.map_err(|e|e.to_string())?;
        let mut bytes=Vec::new();let mut chunk=[0u8;1024];
        let received=timeout(Duration::from_secs(10),async{
            while bytes.len()<8192{
                let n=stream.read(&mut chunk).await.map_err(|e|e.to_string())?;
                if n==0{break;}
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.windows(4).any(|s|s==b"\r\n\r\n"){break;}
            }
            Ok::<(),String>(())
        }).await;
        if !matches!(received,Ok(Ok(()))){continue;}
        let result=parse_callback(&String::from_utf8_lossy(&bytes),state);
        let ok=matches!(result,Ok(Some(_)));
        let body=if ok{"<h2>授權碼已收到</h2>可以返回 CodexCredits。"}
                 else{"<h2>無效的登入回呼</h2>"};
        let status=if ok{"200 OK"}else{"400 Bad Request"};
        let html=format!("<!doctype html><html lang='zh-Hant'><meta charset='utf-8'>{body}</html>");
        let reply=format!("HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{html}",html.len());
        let _=stream.write_all(reply.as_bytes()).await;let _=stream.shutdown().await;
        if let Ok(Some(code))=result{return Ok(code);}
    }
}
fn http()->Result<reqwest::Client,String>{
    reqwest::Client::builder().timeout(Duration::from_secs(30)).build().map_err(|e|e.to_string())
}
fn expires_in(n:Option<i64>)->i64{chrono::Utc::now().timestamp()+n.unwrap_or(3600).clamp(60,86400)}
#[cfg(target_os="android")]
fn parse_native_session(raw:&str)->Result<Session,String>{
    let value:serde_json::Value=serde_json::from_str(raw)
        .map_err(|e|format!("Android 登入回應無效: {e}"))?;
    if let Some(error)=value.get("error").and_then(|x|x.as_str()){
        return Err(error.to_owned());
    }
    let session:Session=serde_json::from_value(value)
        .map_err(|e|format!("Android Token 資料不完整: {e}"))?;
    if session.access_token.is_empty() || session.account_id.is_empty(){
        return Err("Android 登入資料缺少必要 Token 或 Account ID".into());
    }
    Ok(session)
}

pub async fn browser_login()->Result<Session,String>{
    let listener=TcpListener::bind("127.0.0.1:0").await.map_err(|e|format!("無法啟動本機登入回呼: {e}"))?;
    let port=listener.local_addr().map_err(|e|e.to_string())?.port();
    let redirect=format!("http://127.0.0.1:{port}/auth/callback");
    let (verifier,challenge,state)=pkce();
    let url=login_url(&redirect,&challenge,&state)?;
    #[cfg(target_os="android")]
    crate::android_bridge::open_url(url.as_str())?;
    #[cfg(not(target_os="android"))]
    webbrowser::open(url.as_str()).map_err(|e|format!("無法開啟瀏覽器: {e}"))?;
    let code=callback(listener,&state).await?;
    #[cfg(target_os="android")]
    {
        // The browser callback succeeded; exchange via Android's native
        // CodexClient (OkHttp + network/DNS fallbacks), not Rust reqwest.
        let args=serde_json::json!({"code":code,"verifier":verifier,"redirect":redirect}).to_string();
        let raw=tokio::task::spawn_blocking(move||crate::android_bridge::exchange_auth_code(&args))
            .await.map_err(|e|format!("Android 登入工作失敗: {e}"))??;
        return parse_native_session(&raw);
    }
    #[cfg(not(target_os="android"))]
    {
        let res=http()?.post(TOKEN_URL).form(&[
            ("grant_type","authorization_code"),("client_id",CLIENT_ID),
            ("code",code.as_str()),("redirect_uri",redirect.as_str()),("code_verifier",verifier.as_str()),
        ]).send().await.map_err(|e|format!("登入連線失敗: {e}"))?;
        if !res.status().is_success(){return Err(format!("OAuth Token API HTTP {}",res.status()));}
        let value:Response=res.json().await.map_err(|e|format!("Token 內容解析失敗: {e}"))?;
        let id=value.id_token.ok_or("缺少 ID Token")?;
        let account_id=account_from_id_token(&id)?;
        if value.access_token.is_empty(){return Err("缺少 Access Token".into());}
        Ok(Session{access_token:value.access_token,refresh_token:value.refresh_token,
            account_id,expires_at:expires_in(value.expires_in)})
    }
}
pub fn save_session(s:&Session)->Result<(),String>{
    crate::credentials::save(&serde_json::to_string(s).map_err(|e|e.to_string())?)?;
    #[cfg(target_os="android")]
    crate::android_bridge::sync_auth(&serde_json::to_string(s).map_err(|e|e.to_string())?)?;
    Ok(())
}
pub async fn valid_access()->Result<Access,String>{
    #[cfg(target_os="android")]
    {
        // Both the background scheduler and the UI read one canonical
        // SecureStore. Refreshing from a second Rust store can invalidate
        // a rotated refresh token and cause intermittent HTTP 401.
        let raw=tokio::task::spawn_blocking(||crate::android_bridge::valid_native_session())
            .await.map_err(|e|format!("Android Token 工作失敗: {e}"))??;
        let session=parse_native_session(&raw)?;
        // Mirror to the Tauri credential store only after the native
        // session has been validated. Native remains the refresh authority.
        let previous=crate::credentials::load().ok()
            .and_then(|json|serde_json::from_str::<Session>(&json).ok());
        let changed=previous.as_ref().is_none_or(|p|
            p.access_token!=session.access_token ||
            p.refresh_token!=session.refresh_token ||
            p.account_id!=session.account_id ||
            p.expires_at!=session.expires_at);
        if changed{
            crate::credentials::save(&serde_json::to_string(&session)
                .map_err(|e|e.to_string())?)?;
        }
        return Ok(Access{token:session.access_token,account_id:Some(session.account_id)});
    }
    #[cfg(not(target_os="android"))]
    {
    let stored=crate::credentials::load()?;
    let Ok(mut s)=serde_json::from_str::<Session>(&stored) else{
        return Ok(Access{token:stored,account_id:None});
    };
    if chrono::Utc::now().timestamp()+300>=s.expires_at {
        let old=s.refresh_token.as_deref().ok_or("登入逾期，請重新登入")?;
        let res=http()?.post(TOKEN_URL).form(&[
            ("grant_type","refresh_token"),("client_id",CLIENT_ID),("refresh_token",old),
        ]).send().await.map_err(|e|format!("更新 Token 連線失敗: {e}"))?;
        if !res.status().is_success(){return Err(format!("Token 更新失敗 HTTP {}，請重新登入",res.status()));}
        let response:Response=res.json().await.map_err(|e|format!("Token 更新回應無效: {e}"))?;
        if response.access_token.is_empty(){return Err("Token 更新無效".into());}
        s.access_token=response.access_token;
        if let Some(r)=response.refresh_token{s.refresh_token=Some(r);}
        if let Some(id)=response.id_token{s.account_id=account_from_id_token(&id)?;}
        s.expires_at=expires_in(response.expires_in);
        save_session(&s)?;
    }
    Ok(Access{token:s.access_token,account_id:Some(s.account_id)})
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn challenge_valid(){let(v,c,s)=pkce();assert_eq!(c,URL_SAFE_NO_PAD.encode(Sha256::digest(v.as_bytes())));assert_eq!(s.len(),32);}
    #[test]fn callback_state_enforced(){let request="GET /auth/callback?code=abc&state=good HTTP/1.1\r\n\r\n";assert_eq!(parse_callback(request,"good").unwrap(),Some("abc".into()));assert!(parse_callback(request,"bad").is_err());}
    #[test]fn auth_url_has_offline_access(){let u=login_url("http://127.0.0.1:1000/auth/callback","x","s").unwrap();assert!(u.as_str().contains("offline_access"));}
    #[test]fn id_token_claim(){let p=serde_json::json!({"https://api.openai.com/auth":{"chatgpt_account_id":"abc"}});let jwt=format!("header.{}.sig",URL_SAFE_NO_PAD.encode(p.to_string()));assert_eq!(account_from_id_token(&jwt).unwrap(),"abc");}
}
