use chrono::{DateTime, Datelike, Duration, Local, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode { Off, Auto, Custom }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub enabled: bool,
    /// Monday through Sunday.
    pub days: [bool; 7],
    pub hour: u32,
    pub minute: u32,
    /// Number of chained 5-hour windows after the anchor.
    pub auto_count: u8,
}
impl Default for Rule {
    fn default() -> Self {
        Self { id: "daily-0600".into(), enabled: true, days: [true; 7], hour: 6, minute: 0, auto_count: 3 }
    }
}
impl Rule {
    pub fn valid(&self) -> bool { self.hour < 24 && self.minute < 60 && self.auto_count <= 6 }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Mode,
    pub sampling_minutes: u64,
    pub retention_days: i64,
    pub rules: Vec<Rule>,
    pub ntfy_topic: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self { mode: Mode::Off, sampling_minutes: 10, retention_days: 90, rules: vec![Rule::default()], ntfy_topic: String::new() }
    }
}
impl Settings {
    pub fn valid(&self) -> bool {
        (1..=1440).contains(&self.sampling_minutes)
            && (1..=3650).contains(&self.retention_days)
            && self.rules.len() <= 100
            && self.rules.iter().all(Rule::valid)
            && (self.ntfy_topic.is_empty()
                || (self.ntfy_topic.len() <= 128 && self.ntfy_topic.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')))
    }
}
fn day_index(weekday: Weekday) -> usize { weekday.num_days_from_monday() as usize }
pub fn next_anchor(after: DateTime<Local>, rules: &[Rule]) -> Option<DateTime<Local>> {
    let mut best = None;
    for offset in 0..=370 {
        let date = after.date_naive() + Duration::days(offset);
        for r in rules.iter().filter(|r| r.enabled && r.valid() && r.days[day_index(date.weekday())]) {
            let Some(naive) = date.and_hms_opt(r.hour, r.minute, 0) else { continue };
            // Nonexistent DST wall times are skipped; ambiguous ones use the earlier instant.
            if let Some(candidate) = naive.and_local_timezone(Local).earliest() {
                if candidate > after && best.map_or(true, |b| candidate < b) { best = Some(candidate); }
            }
        }
        if best.is_some() { break; }
    }
    best
}
pub fn safe_followups(target: &Rule, rules: &[Rule]) -> u8 {
    if !target.enabled || !target.valid() { return 0; }
    let mut starts = Vec::new();
    for r in rules.iter().filter(|r| r.enabled && r.valid()) {
        for day in 0..7 {
            if r.days[day] { starts.push(day * 1440 + r.hour as usize * 60 + r.minute as usize); }
        }
    }
    let mut min_gap = usize::MAX;
    for day in 0..7 {
        if !target.days[day] { continue; }
        let t = day * 1440 + target.hour as usize * 60 + target.minute as usize;
        for &start in &starts {
            let mut gap = (start + 10080 - t) % 10080;
            if gap == 0 { gap = 10080; }
            min_gap = min_gap.min(gap);
        }
    }
    if min_gap == usize::MAX { 3 } else { (min_gap / 300).saturating_sub(1).min(6) as u8 }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn defaults_are_safe() { let s = Settings::default(); assert_eq!(s.mode, Mode::Off); assert!(s.ntfy_topic.is_empty()); assert!(s.valid()); }
    #[test] fn daily_anchor_allows_three_followups() { let r = Rule::default(); assert_eq!(safe_followups(&r, &[r.clone()]), 3); }
    #[test] fn invalid_topic_rejected() { let mut s = Settings::default(); s.ntfy_topic = "http://example.com".into(); assert!(!s.valid()); }
    #[test] fn invalid_time_rejected() { let mut s = Settings::default(); s.rules[0].hour = 24; assert!(!s.valid()); }
    #[test] fn next_anchor_is_in_future() { let now = Local::now(); assert!(next_anchor(now, &[Rule::default()]).unwrap() > now); }
}
