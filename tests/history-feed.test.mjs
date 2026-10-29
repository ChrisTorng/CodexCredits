import test from "node:test";
import assert from "node:assert/strict";
import {buildHistoryFeed} from "../src/history-feed.ts";
const stamp=(t)=>new Date(t).toISOString();
const sample=(t,five,weekly)=>({at:stamp(t),five_hour:five,weekly,
 five_reset:null,weekly_reset:null});
test("recent meaningful quota changes appear before older reset events",()=>{
 const now=Date.parse("2026-10-29T13:20:00Z");
 const list=[
  sample(now-90*60_000,42,27),
  sample(now-80*60_000,42,27),
  sample(now-40*60_000,0,27),
  sample(now-10*60_000,0,27),
 ];
 const events=[
  {at:stamp(now-9*86400_000),kind:"global_reset",detail:"10/20 Global Reset"},
  {at:stamp(now-60*60_000),kind:"scheduler",detail:"今日排程執行"}
 ];
 const feed=buildHistoryFeed(list,events,now);
 assert.equal(feed.latest.at,list.at(-1).at);
 assert.equal(feed.recent.length,2);
 assert.equal(feed.recent[0].detail,"5h 100% 剩餘 · 週 73% 剩餘");
 assert.equal(feed.recent[1].detail,"今日排程執行");
 assert.equal(feed.archive[0].detail,"10/20 Global Reset");
});
test("unchanged or duplicate 21:12 samples are not repeated in recent feed",()=>{
 const now=Date.parse("2026-10-29T13:12:00Z");
 const rows=[sample(now-30*60_000,42,27),sample(now-20*60_000,42,27),
  sample(now-1*60_000,0,27),sample(now,0,27),sample(now,0,27)];
 const feed=buildHistoryFeed(rows,[],now);
 assert.equal(feed.recent.length,1);
 assert.equal(feed.details.length,4);
 assert.equal(feed.latest.at,rows.at(-1).at);
});
test("events and changes sort together, preserving older diagnostics on demand",()=>{
 const now=Date.parse("2026-10-29T13:00:00Z");
 const events=[
  {at:stamp(now-5*86400_000),kind:"warning",detail:"older DNS error"},
  {at:stamp(now-12*60_000),kind:"global_reset",detail:"new reset"},
  {at:stamp(now-2*3600_000),kind:"banked_reset",detail:"today banked"}
 ];
 const feed=buildHistoryFeed([sample(now,0,27)],events,now);
 assert.deepEqual(feed.recent.map(x=>x.detail),["new reset","today banked"]);
 assert.deepEqual(feed.archive.map(x=>x.detail),["older DNS error"]);
});
