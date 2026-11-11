//! In-process scheduler. Unlike Android AlarmManager it cannot run when the
//! desktop process is stopped; stale events (>90s) are skipped on resume.
use chrono::{DateTime,Duration,Local,NaiveDate,Utc};
use serde::{Serialize,Deserialize};
use crate::{api::Usage,schedule::{Mode,Settings,MissedAnchorPolicy}};
const RESET_GRACE_SEC:i64=15;
const TOLERANCE_SEC:i64=60;

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Kind{Auto,Anchor,AnchorRetry,Chain,Recovery,Resume,Verify}
#[derive(Debug,Clone,Serialize)]
pub struct Next {
 pub at:i64,
 pub kind:Kind,
 pub auto_count:u8,
 pub reason:String,
}
#[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq,Eq)]
pub struct Recovery{pub anchor_at:i64,pub at:i64,pub auto_count:u8}
#[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq,Eq)]
pub struct PendingVerification{
 pub at:i64,
 pub expected_reset:i64,
 pub auto_count:u8,
 pub from_recovery:bool,
 #[serde(default)]pub resume_date:Option<NaiveDate>,
}
#[derive(Debug,Default)]
pub struct Engine {
 pub next:Option<Next>,
 pub chain_remaining:u8,
 pub chain_reset:Option<i64>,
 pub chain_retry_at:Option<i64>,
 pub recovery:Option<Recovery>,
 pub recovery_done_anchor:i64,
 pub pending:Option<PendingVerification>,
 pub resume_done_date:Option<NaiveDate>,
 pub resume_retry_at:Option<i64>,
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
/// Predicted end of the active window and an already-started continuation chain.
/// Future times are advisory: execution rechecks the official reset.
pub fn projected_busy_until(usage:Option<&Usage>,engine:&Engine,now:i64)->Option<i64>{
 let active=usage.filter(|u|confirmed(u,now))
  .and_then(|u|reset_timestamp(u.five_reset.as_ref())).unwrap_or(0);
 let mut end=active;
 if engine.chain_remaining>0 {
  let chain_base=engine.chain_reset.unwrap_or(0).max(active);
  if chain_base>0{end=end.max(chain_base.saturating_add(engine.chain_remaining as i64*5*3600));}
 }
 if let Some(pending)=engine.pending{
  end=end.max(pending.expected_reset.saturating_add(pending.auto_count as i64*5*3600));
 }
 if end>now{Some(end)}else{None}
}
/// One-shot day override: continue at the *actual* confirmed reset, never at a
/// guessed fixed anchor. If no window is running, it becomes eligible now (or
/// at the start of the selected future date). An entry is consumed once.
pub fn next_day_resume(settings:&Settings,usage:Option<&Usage>,engine:&Engine,now:i64)->Option<Next>{
 let local=DateTime::<Utc>::from_timestamp(now,0)?.with_timezone(&Local);
 for offset in 0..=7{
  let date=local.date_naive()+Duration::days(offset);
  let Some(configured_count)=settings.resume_for_date(date)else{continue};
  if engine.resume_done_date.is_some_and(|done|done>=date)
    || engine.pending.as_ref().is_some_and(|p|p.resume_date==Some(date)){continue;}
  let start=date.and_hms_opt(0,0,0)?.and_local_timezone(Local).earliest()?.timestamp();
  let end=(date+Duration::days(1)).and_hms_opt(0,0,0)?
   .and_local_timezone(Local).earliest()?.timestamp();
  let reset=usage.filter(|u|confirmed(u,now))
   .and_then(|u|reset_timestamp(u.five_reset.as_ref())).unwrap_or(0);
  let reserved=projected_busy_until(usage,engine,now).unwrap_or(0);
  let mut at=now.max(start).max(reset).max(reserved)+RESET_GRACE_SEC;
  if offset==0{at=at.max(engine.resume_retry_at.unwrap_or(0));}
  if at>=end{continue;}
  let next_local=DateTime::<Utc>::from_timestamp(at-1,0)?.with_timezone(&Local);
  let next_anchor=settings.next_slot(next_local).map(|(t,_)|t.timestamp());
  // Do not create a reset-based window that would block the next fixed
  // anchor. Day-limited continuation starts may end after midnight.
  let next_fixed=next_anchor.unwrap_or(i64::MAX);
  if at+5*3600>next_fixed.saturating_add(TOLERANCE_SEC){continue;}
  let weekly_continuous=!settings.overrides.get(&date.to_string())
   .is_some_and(|o|o.enabled && o.resume_auto_count.is_some());
  let latest_start=if weekly_continuous{end-1}else{at+6*5*3600};
  let min_latest=latest_start.min(next_fixed.saturating_sub(5*3600));
  let possible=((min_latest-at).max(0)/(5*3600)).min(6) as u8;
  let count=configured_count.min(possible);
  return Some(Next{at,kind:Kind::Resume,auto_count:count,
   reason:format!("{}首次接續，後續自動 {} 次",date,count)});
 }
 None
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
    // Do not post again while the previous request awaits official confirmation.
    if let Some(v)=engine.pending{
     return Some(Next{at:v.at.max(now+1),kind:Kind::Verify,auto_count:v.auto_count,
      reason:"確認先前 Codex 請求是否已啟動 5h".into()});
    }
   let candidate=DateTime::<Utc>::from_timestamp(now,0)?.with_timezone(&Local);
   let anchor=settings.next_slot(candidate).map(|(t,count)|Next{
    at:t.timestamp(),kind:Kind::Anchor,auto_count:count,reason:format!("錨點，後續自動 {count} 次")
   });
   let chain=if engine.chain_remaining>0 {
    engine.chain_reset.map(|reset|Next{
     at:(reset+RESET_GRACE_SEC).max(now+1).max(engine.chain_retry_at.unwrap_or(0)),kind:Kind::Chain,auto_count:0,
     reason:format!("自動接續，尚餘 {} 次",engine.chain_remaining)
    })
   }else{None};
   // Never chain into an anchor's next 5-hour window.
   let chain=chain.filter(|c|anchor.as_ref().is_none_or(|a|a.at>=c.at+5*3600-TOLERANCE_SEC));
   let recovery=engine.recovery.map(|r|Next{
    at:r.at.max(now+1),kind:Kind::Recovery,auto_count:r.auto_count,
    reason:format!("錨點錯過 · 補接續，後續自動 {} 次",r.auto_count)
   });
    [anchor,chain,recovery,next_day_resume(settings,usage,engine,now)].into_iter().flatten().min_by_key(|v|v.at)
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
 for rule in settings.rules_for_date(date).into_iter().filter(|r|r.valid()&&r.start_kind==crate::schedule::StartKind::Anchor){
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
  let mut verifying=e;
  verifying.pending=Some(PendingVerification{at:now+120,expected_reset:reset,auto_count:2,from_recovery:true,resume_date:None});
  assert_eq!(next_event(&s,Some(&q),&verifying,now).unwrap().kind,Kind::Verify);
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
  #[test]fn day_resume_follows_real_reset_and_respects_remaining_slots(){
   let mut s=Settings::default();s.mode=Mode::Custom;
   let date=Local::now().date_naive();
   s.overrides.insert(date.to_string(),crate::schedule::Override{
    enabled:true,rules:vec![],resume_auto_count:Some(6)});
   let start=date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
   let now=start+7*3600;
   let reset=start+10*3600;
   let q=Usage{primary_confirmed:true,five_reset:Some(DateTime::<Utc>::from_timestamp(reset,0).unwrap().to_rfc3339()),..Default::default()};
   let engine=Engine::default();
   let next=next_day_resume(&s,Some(&q),&engine,now).unwrap();
   assert_eq!(next.at,reset+15);
   assert_eq!(next.kind,Kind::Resume);
   assert_eq!(next.auto_count,2); // Tomorrow 06:00 anchor must remain protected
   let done=Engine{resume_done_date:Some(date),..Default::default()};
   assert!(next_day_resume(&s,Some(&q),&done,now).is_none());
  }
  #[test]fn tomorrow_resume_cannot_start_before_todays_active_window_expires(){
   let mut s=Settings::default();s.mode=Mode::Custom;
   let date=Local::now().date_naive();
   let tomorrow=date+Duration::days(1);
   s.overrides.insert(tomorrow.to_string(),crate::schedule::Override{
    enabled:true,rules:vec![],resume_auto_count:Some(3)});
   let today_evening=date.and_hms_opt(22,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
   let tomorrow_reset=tomorrow.and_hms_opt(4,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
   let q=Usage{primary_confirmed:true,five_reset:Some(DateTime::<Utc>::from_timestamp(tomorrow_reset,0).unwrap().to_rfc3339()),..Default::default()};
   let next=next_day_resume(&s,Some(&q),&Engine::default(),today_evening).unwrap();
   assert_eq!(next.at,tomorrow_reset+RESET_GRACE_SEC);
   assert_eq!(next.kind,Kind::Resume);
  }
   #[test]fn pending_verification_beats_due_chain(){
    let mut s=Settings::default();s.mode=Mode::Custom;
    let now=Utc::now().timestamp();
    let e=Engine{chain_remaining:2,chain_reset:Some(now-30),
     pending:Some(PendingVerification{at:now+150,expected_reset:now+5*3600,
      auto_count:1,from_recovery:false,resume_date:None}),..Default::default()};
    let n=next_event(&s,None,&e,now).unwrap();
    assert_eq!(n.kind,Kind::Verify);assert_eq!(n.at,now+150);
   }
   #[test]fn chain_retry_is_rate_limited(){
    let mut s=Settings::default();s.mode=Mode::Custom;s.rules.clear();
    let now=Utc::now().timestamp();
    let e=Engine{chain_remaining:1,chain_reset:Some(now-30),
     chain_retry_at:Some(now+900),..Default::default()};
    let n=next_event(&s,None,&e,now).unwrap();
    assert_eq!(n.kind,Kind::Chain);assert_eq!(n.at,now+900);
   }
  #[test]fn tomorrow_resume_waits_for_todays_chain_across_midnight(){
    let mut s=Settings::default();s.mode=Mode::Custom;s.rules.clear();
    let today=Local::now().date_naive();
    let tomorrow=today+Duration::days(1);
    s.overrides.insert(tomorrow.to_string(),crate::schedule::Override{
     enabled:true,rules:vec![],resume_auto_count:Some(2)});
    let tomorrow_start=tomorrow.and_hms_opt(0,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
    let now=tomorrow_start-2*3600;
    let reset=tomorrow_start+3600;
    let q=Usage{primary_confirmed:true,five_reset:Some(DateTime::<Utc>::from_timestamp(reset,0).unwrap().to_rfc3339()),..Default::default()};
    let e=Engine{chain_remaining:3,chain_reset:Some(reset),..Default::default()};
    let n=next_day_resume(&s,Some(&q),&e,now).unwrap();
    assert_eq!(n.at,reset+3*5*3600+RESET_GRACE_SEC);
    assert_eq!(n.auto_count,2);
   }
   #[test]fn day_resume_without_active_window_starts_at_earliest_time(){
   let mut s=Settings::default();s.mode=Mode::Custom;
   let date=Local::now().date_naive();
   s.overrides.insert(date.to_string(),crate::schedule::Override{
    enabled:true,rules:vec![],resume_auto_count:Some(0)});
   let now=date.and_hms_opt(12,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
   let next=next_day_resume(&s,None,&Engine::default(),now).unwrap();
   assert_eq!(next.at,now+15);
  }

   #[test]fn resumed_chain_survives_midnight_without_repeating_day_start(){
    let today=Local::now().date_naive();
    let tomorrow=today+Duration::days(1);
    let now=today.and_hms_opt(22,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
    let reset=tomorrow.and_hms_opt(3,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
    let mut settings=Settings::default();settings.mode=Mode::Custom;settings.rules.clear();
    settings.overrides.insert(today.to_string(),crate::schedule::Override{
     enabled:true,rules:vec![],resume_auto_count:Some(3)});
    let restored=Engine{resume_done_date:Some(today),chain_remaining:2,
     chain_reset:Some(reset),..Default::default()};
    let next=next_event(&settings,None,&restored,now).unwrap();
    assert_eq!(next.kind,Kind::Chain);
    assert_eq!(next.at,reset+RESET_GRACE_SEC);
  }
   #[test]fn pending_verification_prevents_duplicate_day_resume(){
    let day=Local::now().date_naive();
    let now=day.and_hms_opt(12,0,0).unwrap().and_local_timezone(Local).earliest().unwrap().timestamp();
    let mut settings=Settings::default();settings.mode=Mode::Custom;
    settings.overrides.insert(day.to_string(),crate::schedule::Override{
     enabled:true,rules:vec![],resume_auto_count:Some(2)});
    let state=Engine{
     pending:Some(PendingVerification{at:now+120,expected_reset:now+5*3600,
      auto_count:2,from_recovery:false,resume_date:Some(day)}),
     ..Default::default()};
    let next=next_event(&settings,None,&state,now).unwrap();
    assert_eq!(next.kind,Kind::Verify);
    assert_eq!(next.at,now+120);
  }

}
