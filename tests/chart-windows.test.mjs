import test from "node:test";
import assert from "node:assert/strict";
import {actualFiveHourWindows,observedQuotaWindows,scheduledFiveHourWindows,historyScheduleGuides,eventConfirmedFiveHourWindows,interruptAtNextReset,idealSegment,idealQuotaSegment,windowBoundaries,readableBoundaries,clockLabel,FIVE_HOUR_MS}
 from "../src/chart-windows.ts";

const iso=(t)=>new Date(t).toISOString();
const h=3600_000;
test("real observed window uses the server reset and persists through midnight",()=>{
 const end=Date.parse("2026-10-29T13:35:00Z"),start=end-FIVE_HOUR_MS;
 const samples=[
  {at:iso(start+5*60_000),five_reset:iso(end),five_hour:0},
  {at:iso(start+20*60_000),five_reset:iso(end),five_hour:12},
 ];
 const windows=actualFiveHourWindows(samples);
 assert.equal(windows.length,1);
 assert.equal(windows[0].start,start);
 assert.deepEqual(idealSegment(windows[0],start+h,start+2*h),[[start+h,80],[start+2*h,60]]);
});
test("idle sliding reset_at values do not invent completed 5h windows",()=>{
 const at=Date.parse("2026-10-29T04:00:00Z");
 const samples=Array.from({length:6},(_,i)=>({
  at:iso(at+i*10*60_000),five_hour:0,five_reset:iso(at+i*10*60_000+FIVE_HOUR_MS)
 }));
 assert.deepEqual(actualFiveHourWindows(samples),[]);
});
test("zero-used but stable confirmed window is real",()=>{
 const now=Date.parse("2026-10-29T08:35:00Z"),end=now+FIVE_HOUR_MS-60_000;
 assert.equal(actualFiveHourWindows([],iso(end),true,now).length,1);
});
test("future scheduling starts precisely at anchor, without treating it as history",()=>{
 const now=Date.parse("2026-10-29T06:00:00Z");
 const next=Math.floor((now+60_000)/1000);
 const projected=scheduledFiveHourWindows({at:next,kind:"anchor",auto_count:2},now,now,now+20*h,[]);
 assert.deepEqual(projected.map(w=>w.start),[next*1000,next*1000+FIVE_HOUR_MS,next*1000+2*FIVE_HOUR_MS]);
 assert.ok(projected.every(w=>w.estimated));
});
test("time labels omit :00 but retain nonzero minutes and include dates on quota cards",()=>{
 const whole=Date.parse("2026-10-29T16:00:00Z");
 const partial=Date.parse("2026-10-29T16:35:00Z");
 assert.ok(!clockLabel(whole).includes(":"));
 assert.ok(clockLabel(partial).endsWith(":35"));
 assert.ok(clockLabel(partial,true).includes("/"));
});
test("shared boundaries are deduplicated",()=>{
 const t=Date.parse("2026-10-29T08:00:00Z");
 const ws=[{start:t,end:t+FIVE_HOUR_MS,estimated:false},
  {start:t+FIVE_HOUR_MS,end:t+2*FIVE_HOUR_MS,estimated:true}];
 assert.equal(windowBoundaries(ws,t,t+2*FIVE_HOUR_MS).length,3);
});

test("weekly reset interrupts previous ideal immediately, never overlaps",()=>{
 const h=3_600_000,week=7*24*h;
 const firstEnd=Date.parse("2026-11-02T04:00:00Z");
 const secondEnd=Date.parse("2026-11-04T06:35:00Z");
 const firstStart=firstEnd-week,secondStart=secondEnd-week;
 const rows=[
  {at:iso(firstStart+50*h),five_hour:null,five_reset:null,weekly:30,weekly_reset:iso(firstEnd)},
  {at:iso(firstStart+51*h),five_hour:null,five_reset:null,weekly:35,weekly_reset:iso(firstEnd)},
  {at:iso(secondStart+10*60_000),five_hour:null,five_reset:null,weekly:0,weekly_reset:iso(secondEnd)},
  {at:iso(secondStart+20*60_000),five_hour:null,five_reset:null,weekly:5,weekly_reset:iso(secondEnd)}
 ];
 const windows=observedQuotaWindows(rows,"weekly",iso(secondEnd),false,secondStart+30*60_000);
 assert.equal(windows.length,2);
 assert.equal(windows[0].end,secondStart);
 assert.equal(windows[1].start,secondStart);
 assert.ok(idealQuotaSegment(windows[0],secondStart-h,secondStart+3*h,week)[1][1]>0);
 for(let at=firstStart;at<secondStart+h;at+=h){
  assert.ok(windows.filter(w=>w.start<=at&&at<w.end).length<=1);
 }
});
test("overlapping five-hour reset predictions truncate old ideal at newer start",()=>{
 const anchor=Date.parse("2026-10-29T02:00:00Z");
 const first={start:anchor,end:anchor+FIVE_HOUR_MS,estimated:false};
 const second={start:anchor+2*h,end:anchor+7*h,estimated:false};
 const windows=interruptAtNextReset([second,first]);
 assert.equal(windows[0].end,second.start);
 assert.equal(idealSegment(windows[0],anchor,second.start)[1][1],60);
 assert.equal(windows.length,2);
});
test("moving unconfirmed weekly reset values are not each rendered as ideals",()=>{
 const now=Date.parse("2026-10-29T06:00:00Z");
 const week=7*24*h;
 const samples=Array.from({length:8},(_,i)=>({
  at:iso(now+i*10*60_000),
  five_hour:null,five_reset:null,weekly:0,
  weekly_reset:iso(now+i*10*60_000+week)
 }));
 assert.equal(observedQuotaWindows(samples,"weekly",null,false,now+2*h).length,0);
});
test("labels are selected sparsely on a narrow phone history chart",()=>{
 const start=Date.parse("2026-10-29T00:00:00Z");
 const ticks=[1,2,3,4,5,6,7,8,9,10].map(v=>start+v*h);
 const readable=readableBoundaries(ticks,start,start+24*h,320);
 assert.ok(readable.length<ticks.length);
 assert.ok(readable.every((t,i)=>i===0||t-readable[i-1]>=3*h));
});

test("two stable idle reset_at samples must not fabricate a 04:18 five-hour reset",()=>{
 const at=Date.parse("2026-10-28T20:18:00Z");
 const end=at+FIVE_HOUR_MS;
 const idle=[
  {at:iso(at+1*60_000),five_hour:0,five_reset:iso(end)},
  {at:iso(at+10*60_000),five_hour:0,five_reset:iso(end)},
  {at:iso(at+25*60_000),five_hour:0,five_reset:iso(end)}
 ];
 assert.deepEqual(actualFiveHourWindows(idle),[]);
 const used={at:iso(at+40*60_000),five_hour:4,five_reset:iso(end)};
 assert.equal(actualFiveHourWindows([...idle,used]).length,1);
});

test("history labels use the configured 06/11/16/21 anchors instead of 06/12/18",()=>{
 const day=new Date("2026-11-04T00:00:00").getTime();
 const guides=historyScheduleGuides([{enabled:true,hour:6,minute:0,auto_count:3}],day,day+86400000);
 assert.deepEqual(guides.map(t=>new Date(t).getHours()),[6,11,16,21]);
 const overridden=historyScheduleGuides([{enabled:true,hour:7,minute:30,auto_count:1}],day,day+86400000);
 assert.deepEqual(overridden.map(t=>new Date(t).getHours()),[7,12]);
 assert.deepEqual(historyScheduleGuides([],day,day+86400000),[]);
});
test("only a server-verified new window provides event evidence",()=>{
 const at=Date.parse("2026-11-04T06:00:00Z"),now=at+6*3600000;
 const events=[
  {at:new Date(at).toISOString(),kind:"trigger",detail:"錨點5h 視窗啟動已確認"},
  {at:new Date(at+5*3600000).toISOString(),kind:"scheduler",detail:"排程設定"},
  {at:new Date(at+2*3600000).toISOString(),kind:"trigger",detail:"手動觸發成功"},
  {at:new Date(at+3*3600000).toISOString(),kind:"trigger_no_change",detail:"Codex 請求成功但 5h 未重新開始"},
  {at:new Date(now+3600000).toISOString(),kind:"trigger",detail:"手動觸發成功"}
 ];
 const observed=eventConfirmedFiveHourWindows(events,[],now);
 assert.equal(observed.length,1);
 assert.equal(observed[0].start,at);
 assert.equal(observed[0].end,at+FIVE_HOUR_MS);
 assert.deepEqual(eventConfirmedFiveHourWindows(events,[{start:at,end:at+FIVE_HOUR_MS,estimated:false}],now),[]);
});
