//! In-process scheduler. Unlike Android AlarmManager it cannot run when the
//! desktop process is stopped; stale events (>90s) are skipped on resume.
use chrono::{DateTime,Local,Utc};
use serde::{Serialize,Deserialize};
use crate::{api::Usage,schedule::{Mode,Settings,MissedAnchorPolicy}};
const RESET_GRACE_SEC:i64=15;
const TOLERANCE_SEC:i64=60;

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Kind{Auto,Anchor,AnchorRetry,Chain,Recovery}
#[derive(Debug,Clone,Serialize)]
pub struct Next {
 pub at:i64,
 pub kind:Kind,
 pub auto_count:u8,
 pub reason:String,
}
#[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq,Eq)]
pub struct Recovery{pub anchor_at:i64,pub at:i64,pub auto_count:u8}
#[derive(Debug,Default)]
pub struct Engine {
 pub next:Option<Next>,
 pub chain_remaining:u8,
 pub chain_reset:Option<i64>,
 pub recovery:Option<Recovery>,
 pub recovery_done_anchor:i64,
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
   let recovery=engine.recovery.map(|r|Next{
    at:r.at.max(now+1),kind:Kind::Recovery,auto_count:r.auto_count,
    reason:format!("錨點錯過 · 補接續，後續自動 {} 次",r.auto_count)
   });
   [anchor,chain,recovery].into_iter().flatten().min_by_key(|v|v.at)
  }
 }
}
/// Determine a same-day recovery from the confirmed pre-existing 5h window.
pub fn missed_today(settings:&Settings,usage:&Usage,now:i64)->Option<Recovery>{
 if settings.mode!=Mode::Custom||settings.missed_anchor_policy==MissedAnchorPolicy::SkipToday{return None;}
 if !confirmed(usage,now){return None;}
 let reset=reset_timestamp(usage.five_reset.as_ref())?;
 let start=reset-5*3600;
 let local=DateTime::<Utc>::from_timestamp(now,0)?.with_timezone(&Local);
 let date=local.date_naive();
 let mut best:Option<Recovery>=None;
 for rule in settings.rules_for_date(date).into_iter().filter(|r|r.valid()){
  let anchor=date.and_hms_opt(rule.hour,rule.minute,0)?.and_local_timezone(Local).earliest()?.timestamp();
  if now<anchor||now>=anchor+5*3600||start>=anchor-2{continue;}
  let (at,count)=match settings.missed_anchor_policy{
   MissedAnchorPolicy::AtReset=>(reset+RESET_GRACE_SEC,rule.auto_count),
   MissedAnchorPolicy::NextSlot if rule.auto_count>0=>((anchor+5*3600).max(reset+RESET_GRACE_SEC),rule.auto_count-1),
   _=>continue
  };
  // A delayed recovery must not occupy the next independent anchor's 5h slot.
  let next=DateTime::<Utc>::from_timestamp(anchor+1,0)?.with_timezone(&Local);
  if settings.next_slot(next).is_some_and(|(slot,_)|at+5*3600>slot.timestamp()+TOLERANCE_SEC){continue;}
  let candidate=Recovery{anchor_at:anchor,at,auto_count:count};
  if best.is_none_or(|old|candidate.anchor_at>old.anchor_at){best=Some(candidate);}
 }
 best
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
 #[test]fn missed_anchor_policy_today_three_choices(){
  let mut s=Settings::default();s.mode=Mode::Custom;
  let date=Local::now().date_naive();
  let anchor=date.and_hms_opt(6,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
  let now=anchor+78*60;let reset=anchor+3*3600+47*60;
  let q=Usage{five_hour:Some(0.),primary_confirmed:true,
   five_reset:Some(DateTime::<Utc>::from_timestamp(reset,0).unwrap().to_rfc3339()),..Default::default()};
  let r=missed_today(&s,&q,now).unwrap();
  assert_eq!((r.at,r.auto_count),(anchor+5*3600,2));
  let e=Engine{recovery:Some(r),..Default::default()};
  assert_eq!(next_event(&s,Some(&q),&e,now).unwrap().kind,Kind::Recovery);
  // A five-hour window started just ten seconds before 06:00 still
  // prevents the anchor and must recover today (not tomorrow).
  let early=Usage{five_hour:Some(0.),primary_confirmed:true,
   five_reset:Some(DateTime::<Utc>::from_timestamp(anchor+5*3600-10,0).unwrap().to_rfc3339()),..Default::default()};
  assert_eq!(missed_today(&s,&early,now).unwrap().at,anchor+5*3600+5);
  s.missed_anchor_policy=MissedAnchorPolicy::AtReset;
  let r=missed_today(&s,&q,now).unwrap();
  assert_eq!((r.at,r.auto_count),(reset+15,3));
  s.missed_anchor_policy=MissedAnchorPolicy::SkipToday;
  assert!(missed_today(&s,&q,now).is_none());
 }
 #[test]fn window_started_at_anchor_never_causes_recovery(){
  let mut s=Settings::default();s.mode=Mode::Custom;
  let date=Local::now().date_naive();
  let a=date.and_hms_opt(6,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
  let q=Usage{five_hour:Some(0.),primary_confirmed:true,five_reset:Some(DateTime::<Utc>::from_timestamp(a+5*3600,0).unwrap().to_rfc3339()),..Default::default()};
  assert!(missed_today(&s,&q,a+3600).is_none());
 }
 #[test]fn exhausted_weekly_retry_is_rate_limited(){let mut s=Settings::default();s.mode=Mode::Auto;
  let now=Utc::now().timestamp();
  let u=Usage{allowed:true,weekly:Some(100.),weekly_reset:Some(DateTime::<Utc>::from_timestamp(now-100,0).unwrap().to_rfc3339()),..Usage::default()};
  assert_eq!(next_event(&s,Some(&u),&Engine::default(),now).unwrap().at,now+900);
 }
}
