use chrono::{DateTime,Datelike,Duration,Local,NaiveDate,Weekday};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub enum Mode {Off,Auto,Custom}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Rule {
 pub id:String,pub enabled:bool,pub days:[bool;7],pub hour:u32,pub minute:u32,pub auto_count:u8
}
impl Default for Rule {
 fn default()->Self{Self{id:"daily-0600".into(),enabled:true,days:[true;7],hour:6,minute:0,auto_count:3}}
}
impl Rule{pub fn valid(&self)->bool{self.hour<24&&self.minute<60&&self.auto_count<=6}}
#[derive(Debug,Clone,Serialize,Deserialize,Default)]
pub struct Override{pub enabled:bool,pub rules:Vec<Rule>}
fn default_retention()->i64{8}
fn default_sampling()->u64{10}
fn default_theme()->String{"system".into()}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Settings {
 pub mode:Mode,
 #[serde(default="default_sampling")]pub sampling_minutes:u64,
 #[serde(default="default_retention")]pub retention_days:i64,
 pub rules:Vec<Rule>,
 #[serde(default)]pub overrides:BTreeMap<String,Override>,
 #[serde(default)]pub ntfy_topic:String,
 #[serde(default="default_theme")]pub theme:String,
 #[serde(default)]pub native_notify_enabled:bool,
}
impl Default for Settings{
 fn default()->Self{Self{mode:Mode::Off,sampling_minutes:10,retention_days:8,rules:vec![Rule::default()],overrides:BTreeMap::new(),ntfy_topic:String::new(),theme:"system".into(),native_notify_enabled:false}}
}
impl Settings{
 pub fn valid(&self)->bool{
  [10,15,30,60].contains(&self.sampling_minutes)
  && (1..=3650).contains(&self.retention_days)
  && self.rules.len()<=100
  && self.rules.iter().all(Rule::valid)
  && conflict_ids(&self.rules,true).is_empty()
  && self.overrides.len()<=32
  && self.overrides.iter().all(|(date,ov)|NaiveDate::parse_from_str(date,"%Y-%m-%d").is_ok()
      &&ov.rules.len()<=100 &&ov.rules.iter().all(Rule::valid) &&(!ov.enabled||conflict_ids(&ov.rules,false).is_empty()))
  &&["system","light","dark"].contains(&self.theme.as_str())
  && (self.ntfy_topic.is_empty() || ntfy_valid(&self.ntfy_topic))
 }
 pub fn rules_for_date(&self,date:NaiveDate)->Vec<Rule>{
  if let Some(override_day)=self.overrides.get(&date.to_string()){
   if override_day.enabled{return override_day.rules.iter().filter(|r|r.enabled).cloned().collect();}
  }
  let day=date.weekday().num_days_from_monday() as usize;
  self.rules.iter().filter(|r|r.enabled&&r.days[day]).cloned().collect()
 }
 pub fn next_slot(&self,after:DateTime<Local>)->Option<(DateTime<Local>,u8)>{
  for offset in 0..=370{
   let date=after.date_naive()+Duration::days(offset);
   let mut best:Option<(DateTime<Local>,u8)>=None;
   for r in self.rules_for_date(date).iter().filter(|r|r.valid()){
    if let Some(local)=date.and_hms_opt(r.hour,r.minute,0).and_then(|d|d.and_local_timezone(Local).earliest()){
     if local>after && best.as_ref().is_none_or(|(v,_)|local<*v){best=Some((local,r.auto_count));}
    }
   }
   if best.is_some(){return best}
  }
  None
 }
}
fn ntfy_valid(s:&str)->bool{
 let Ok(url)=url::Url::parse(s)else{return false};
 let part=url.path().trim_matches('/');
 url.scheme()=="https"&&url.host_str()==Some("ntfy.sh")&&url.query().is_none()&&url.fragment().is_none()
  &&!part.is_empty()&&part.len()<=128&&!part.contains('/')
  &&part.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
pub fn next_anchor(after:DateTime<Local>,rules:&[Rule])->Option<DateTime<Local>>{
 let mut s=Settings::default();s.rules=rules.to_vec();s.next_slot(after).map(|(date,_)|date)
}
pub fn safe_followups(target:&Rule,rules:&[Rule])->u8{max_safe_count(target,rules,true)}
pub fn max_safe_count(target:&Rule,rules:&[Rule],weekly:bool)->u8{
 if !target.enabled||!target.valid(){return 0}
 let period=if weekly{10080}else{1440};
 let mut starts=Vec::new();
 for r in rules.iter().filter(|r|r.enabled&&r.valid()){
  for d in 0..(if weekly{7}else{1}){
   if weekly&&!r.days[d]{continue;}
   starts.push(d*1440+r.hour as usize*60+r.minute as usize);
  }
 }
 let mut min_gap=usize::MAX;
 for d in 0..(if weekly{7}else{1}){
  if weekly&&!target.days[d]{continue;}
  let time=d*1440+target.hour as usize*60+target.minute as usize;
  for &start in &starts{
   let mut gap=(start+period-time)%period;
   if gap==0{gap=period;}
   min_gap=min_gap.min(gap);
  }
 }
 if min_gap==usize::MAX{3}else{(min_gap/300).saturating_sub(1).min(6) as u8}
}
pub fn conflict_ids(rules:&[Rule],weekly:bool)->Vec<String>{
 let period=if weekly{10080usize}else{1440usize};
 let mut bad=Vec::<String>::new();
 let mut occurrences:Vec<(&str,usize)>=Vec::new();
 for r in rules.iter().filter(|r|r.enabled&&r.valid()){
  if r.auto_count>max_safe_count(r,rules,weekly){bad.push(r.id.clone());}
  for d in 0..(if weekly{7}else{1}){
   if weekly&&!r.days[d]{continue;}
   let anchor=d*1440+r.hour as usize*60+r.minute as usize;
   for index in 0..=r.auto_count{
    occurrences.push((&r.id,(anchor+index as usize*300)%period));
   }
  }
 }
 for (i,a) in occurrences.iter().enumerate(){
  for b in occurrences.iter().skip(i+1){
   if a.0==b.0{continue;}
   let distance=a.1.abs_diff(b.1);
   if distance.min(period-distance)<300{
    if !bad.iter().any(|id|id==a.0){bad.push(a.0.into());}
    if !bad.iter().any(|id|id==b.0){bad.push(b.0.into());}
   }
  }
 }
 bad
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn defaults(){let s=Settings::default();assert_eq!(s.mode,Mode::Off);assert_eq!(s.retention_days,8);assert!(s.valid());}
 #[test]fn daily_three(){let r=Rule::default();assert_eq!(safe_followups(&r,&[r.clone()]),3);}
 #[test]fn next_anchor_future(){let s=Settings::default();assert!(s.next_slot(Local::now()).unwrap().0>Local::now());}
 #[test]fn overrides_suppress_defaults(){let mut s=Settings::default();let day=Local::now().date_naive();s.overrides.insert(day.to_string(),Override{enabled:true,rules:vec![]});assert!(s.rules_for_date(day).is_empty());}
 #[test]fn overlapping_anchors_rejected(){
  let mut s=Settings::default();
  let mut duplicate=Rule::default();duplicate.id="overlap".into();duplicate.hour=7;
  s.rules.push(duplicate);
  assert!(!s.valid());
 }
 #[test]fn topic_validation(){let mut s=Settings::default();s.ntfy_topic="https://ntfy.sh/codex-abc".into();assert!(s.valid());s.ntfy_topic="http://localhost/a".into();assert!(!s.valid());}
}
