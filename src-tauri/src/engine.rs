//! In-process scheduler. Unlike Android AlarmManager it cannot run when the
//! desktop process is stopped; stale events (>90s) are skipped on resume.
use chrono::{DateTime,Local,Utc};
use serde::Serialize;
use crate::{api::Usage,schedule::{Mode,Settings}};
const RESET_GRACE_SEC:i64=15;
const TOLERANCE_SEC:i64=60;

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Kind{Auto,Anchor,AnchorRetry,Chain}
#[derive(Debug,Clone,Serialize)]
pub struct Next {
 pub at:i64,
 pub kind:Kind,
 pub auto_count:u8,
 pub reason:String,
}
#[derive(Debug,Default)]
pub struct Engine {
 pub next:Option<Next>,
 pub chain_remaining:u8,
 pub chain_reset:Option<i64>,
}
pub fn reset_timestamp(iso:Option<&String>)->Option<i64>{
 iso.and_then(|s|DateTime::parse_from_rfc3339(s).ok()).map(|t|t.timestamp())
}
pub fn confirmed(usage:&Usage,now:i64)->bool{
 usage.primary_confirmed && reset_timestamp(usage.five_reset.as_ref()).is_some_and(|r|r>now)
}
pub fn exhausted(usage:&Usage)->bool{
 usage.five_hour.is_some_and(|p|p>=100.) ||
 usage.weekly.is_some_and(|p|p>=100.) ||
 usage.limit_reached
}
pub fn blocked(usage:&Usage)->bool{
 !usage.allowed ||usage.weekly.is_some_and(|p|p>=100.)
}
pub fn can_trigger(usage:&Usage,now:i64)->Result<(),String>{
 if blocked(usage){return Err("週配額不可用或已達限制".into());}
 if confirmed(usage,now){return Err("5 小時視窗仍有效，請等待重置後再觸發".into());}
 Ok(())
}
pub fn next_event(settings:&Settings,usage:Option<&Usage>,engine:&Engine,now:i64)->Option<Next>{
 match settings.mode{
  Mode::Off=>None,
  Mode::Auto=>{
   let u=usage?;
   let (at,reason)=if confirmed(u,now){
    (reset_timestamp(u.five_reset.as_ref()).unwrap_or(now)+RESET_GRACE_SEC,"5 小時視窗重置後")
   }else if u.weekly.is_some_and(|p|p>=100.){
    let reset=reset_timestamp(u.weekly_reset.as_ref())?;
    if reset>now {(reset+RESET_GRACE_SEC,"週配額重置後")}
    else{(now+15*60,"週配額仍未恢復，15 分鐘後再確認")}
   }else if !u.allowed{
    (now+15*60,"等待伺服器允許觸發")
   }else{(now+15,"尚未確認 5 小時視窗，準備觸發")};
   Some(Next{at,kind:Kind::Auto,auto_count:0,reason:reason.into()})
  },
  Mode::Custom=>{
   let candidate=DateTime::<Utc>::from_timestamp(now,0)?.with_timezone(&Local);
   let anchor=settings.next_slot(candidate).map(|(t,count)|Next{
    at:t.timestamp(),kind:Kind::Anchor,auto_count:count,reason:format!("錨點，後續自動 {count} 次")
   });
   let chain=if engine.chain_remaining>0 {
    engine.chain_reset.map(|reset|Next{
     at:(reset+RESET_GRACE_SEC).max(now+1),kind:Kind::Chain,auto_count:0,
     reason:format!("自動接續，尚餘 {} 次",engine.chain_remaining)
    })
   }else{None};
   // Never chain into an anchor's next 5-hour window.
   let chain=chain.filter(|c|anchor.as_ref().is_none_or(|a|a.at>=c.at+5*3600-TOLERANCE_SEC));
   match (anchor,chain){
    (Some(a),Some(c))=>Some(if c.at<a.at{c}else{a}),
    (Some(a),None)=>Some(a),
    (None,Some(c))=>Some(c),
    _=>None
   }
  }
 }
}
pub fn is_stale(now:i64,at:i64)->bool{now-at>90}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn modes_off_by_default(){let s=Settings::default();assert!(next_event(&s,None,&Engine::default(),Utc::now().timestamp()).is_none());}
 #[test]fn active_window_waits_for_real_reset(){let mut s=Settings::default();s.mode=Mode::Auto;
  let now=Utc::now().timestamp();
  let u=Usage{five_hour:Some(10.),primary_confirmed:true,five_reset:Some(DateTime::<Utc>::from_timestamp(now+3600,0).unwrap().to_rfc3339()),allowed:true,..Usage::default()};
  let e=next_event(&s,Some(&u),&Engine::default(),now).unwrap();assert_eq!(e.at,now+3615);assert!(!can_trigger(&u,now).is_ok());
 }
 #[test]fn anchor_avoids_past(){let mut s=Settings::default();s.mode=Mode::Custom;
  assert!(next_event(&s,None,&Engine::default(),Utc::now().timestamp()).unwrap().at>Utc::now().timestamp());
 }
 #[test]fn guard_blocks_weekly_exhaustion(){let u=Usage{allowed:true,weekly:Some(100.),..Usage::default()};assert!(can_trigger(&u,0).is_err());}
 #[test]fn stale_event_is_not_replayed(){assert!(is_stale(200,1));assert!(!is_stale(100,90));}
 #[test]fn exhausted_weekly_retry_is_rate_limited(){let mut s=Settings::default();s.mode=Mode::Auto;
  let now=Utc::now().timestamp();
  let u=Usage{allowed:true,weekly:Some(100.),weekly_reset:Some(DateTime::<Utc>::from_timestamp(now-100,0).unwrap().to_rfc3339()),..Usage::default()};
  assert_eq!(next_event(&s,Some(&u),&Engine::default(),now).unwrap().at,now+900);
 }
}
