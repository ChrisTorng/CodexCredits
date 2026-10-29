import test from "node:test";
import assert from "node:assert/strict";
import {actualFiveHourWindows,scheduledFiveHourWindows,idealSegment,windowBoundaries,clockLabel,FIVE_HOUR_MS}
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
