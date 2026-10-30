//! Windows tray: live 5h/weekly remaining quota with in-window time pointers.
#![cfg(desktop)]
use tauri::{AppHandle,Manager,image::Image};
use crate::api::Usage;

const WIDTH:usize=32;
fn pixel(data:&mut [u8],x:usize,y:usize,rgba:[u8;4]){
 if x>=WIDTH||y>=WIDTH{return;}
 data[(y*WIDTH+x)*4..(y*WIDTH+x)*4+4].copy_from_slice(&rgba);
}
fn rect(data:&mut [u8],x:usize,y:usize,w:usize,h:usize,color:[u8;4]){
 for yy in y..(y+h).min(WIDTH){for xx in x..(x+w).min(WIDTH){pixel(data,xx,yy,color);}}
}
fn glyph(data:&mut [u8],left:usize,top:usize,pattern:[u8;5]){
 for (y,row) in pattern.iter().enumerate(){for x in 0..3{
  if (row & (1<<(2-x)))!=0 {pixel(data,left+x,top+y,[240,243,248,255]);}
 }}
}
fn row(data:&mut [u8],y:usize,used:Option<f64>,reset:Option<&str>,window_seconds:i64,color:[u8;4],now:i64){
 let remaining=used.map(|v|(100.-v).clamp(0.,100.));
 rect(data,7,y,24,9,[52,59,70,255]);
 if let Some(p)=remaining{
  rect(data,7,y,((p/100.*24.).round() as usize).min(24),9,color);
 }
 let end=reset.and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d|d.timestamp());
 if let Some(end)=end.filter(|e|*e>now&&*e<=now+window_seconds){
  let elapsed=(now-(end-window_seconds)).clamp(0,window_seconds);
  let x=7+(elapsed*23/window_seconds) as usize;
  rect(data,x,y,2,9,[250,250,250,255]);
 }
}
pub fn icon(usage:Option<&Usage>,now:i64)->Image<'static>{
 let mut data=vec![0u8;WIDTH*WIDTH*4];
 rect(&mut data,0,0,WIDTH,WIDTH,[29,35,45,255]);
 rect(&mut data,1,1,30,30,[17,23,33,255]);
 glyph(&mut data,2,7,[7,4,7,1,7]); // 5
 glyph(&mut data,2,20,[5,5,7,7,5]); // W (stylised)
 row(&mut data,5,usage.and_then(|v|v.five_hour),usage.and_then(|v|v.five_reset.as_deref()),5*3600,[39,128,237,255],now);
 row(&mut data,18,usage.and_then(|v|v.weekly),usage.and_then(|v|v.weekly_reset.as_deref()),7*86400,[228,143,49,255],now);
 Image::new_owned(data,WIDTH as u32,WIDTH as u32)
}
fn readable_reset(s:Option<&str>,now:i64)->String{
 let Some(end)=s.and_then(|v|chrono::DateTime::parse_from_rfc3339(v).ok()).map(|d|d.timestamp()) else{return "未提供重置時間".into();};
 if end<=now{return "待重新確認".into();}
 let remaining=end-now;
 format!("{}天 {:02}:{:02} 後",remaining/86400,(remaining%86400)/3600,(remaining%3600)/60)
}
pub fn tooltip(usage:Option<&Usage>,now:i64)->String{
 let Some(v)=usage else{return "Codex Credits · 尚未取得配額".into()};
 let pct=|used:Option<f64>|used.map(|n|format!("{:.0}%",(100.-n).clamp(0.,100.))).unwrap_or_else(||"—".into());
 format!("Codex Credits\n5h {} · {}\n1w {} · {}",
  pct(v.five_hour),readable_reset(v.five_reset.as_deref(),now),
  pct(v.weekly),readable_reset(v.weekly_reset.as_deref(),now))
}
pub fn refresh(app:&AppHandle,usage:Option<&Usage>){
 if let Some(tray)=app.tray_by_id("quota"){
  let now=chrono::Utc::now().timestamp();
  let _=tray.set_icon(Some(icon(usage,now)));
  let _=tray.set_tooltip(Some(tooltip(usage,now)));
 }
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn icon_has_correct_rgba_size(){
  let img=icon(None,1);
  assert_eq!(img.rgba().len(),WIDTH*WIDTH*4);
  assert_eq!(img.width(),WIDTH as u32);
 }
 #[test]fn tooltip_reports_remaining_not_used(){
  let q=Usage{five_hour:Some(27.),weekly:Some(40.),..Default::default()};
  let t=tooltip(Some(&q),0);
  assert!(t.contains("5h 73%"));
  assert!(t.contains("1w 60%"));
 }
}
