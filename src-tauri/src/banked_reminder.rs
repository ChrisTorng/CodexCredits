//! Backed Reset expiry milestones shared by Android and Windows.
use crate::api::{Credit,Credits};
use chrono::DateTime;
const DAY:i64=86400;
pub fn nearest(credits:&Credits,now:i64)->Option<(&Credit,i64)>{
 credits.available.iter().filter_map(|c|{
  let expiry=DateTime::parse_from_rfc3339(&c.expires_at).ok()?.timestamp();
  if expiry>now {Some((c,expiry-now))}else{None}
 }).min_by_key(|(_,remaining)|*remaining)
}
pub fn stage(remaining:i64)->Option<(u8,&'static str)>{
 if remaining<=0||remaining>7*DAY {None}
 else if remaining<=5*3600{Some((5,"進入最後 5 小時"))}
 else if remaining<=DAY{Some((4,"剩最後 1 天"))}
 else if remaining<=2*DAY{Some((3,"剩 2 天"))}
 else if remaining<=3*DAY{Some((2,"剩 3 天"))}
 else{Some((1,"進入 7 天提醒範圍"))}
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn milestones_include_exact_boundaries(){
  for (seconds,expected) in [(7*DAY,1),(3*DAY,2),(2*DAY,3),(DAY,4),(5*3600,5)] {
   assert_eq!(stage(seconds).map(|v|v.0),Some(expected));
  }
  assert_eq!(stage(0),None);
  assert_eq!(stage(8*DAY),None);
 }
 #[test]fn nearer_expiry_is_selected_not_api_order(){
  let credits=Credits{count:2,available:vec![
   Credit{id:"later".into(),title:"".into(),description:"".into(),expires_at:"2026-11-20T00:00:00Z".into()},
   Credit{id:"first".into(),title:"".into(),description:"".into(),expires_at:"2026-11-02T00:00:00Z".into()}
  ]};
  let now=DateTime::parse_from_rfc3339("2026-10-30T00:00:00Z").unwrap().timestamp();
  assert_eq!(nearest(&credits,now).unwrap().0.id,"first");
 }
}
