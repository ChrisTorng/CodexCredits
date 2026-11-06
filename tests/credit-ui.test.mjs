import test from "node:test";
import assert from "node:assert/strict";
import {nearestCredit,activeCreditsByExpiry,compactLocalDateTime,chartTooltipPosition,quotaInteriorTicks,bankedCountdown,bankedWarningStage,bankedResetEligibility,bankedQuotaLabel,timelineTicks,historyFallbackTicks,DAY,HOUR} from "../src/credit-ui.ts";
const now=Date.parse("2026-10-30T12:00:00Z"),future=(ms)=>new Date(now+ms).toISOString();
test("nearest unexpired credit uses expiry, not API array order",()=>{
 assert.equal(nearestCredit([{id:"late",expires_at:future(12*DAY)},{id:"old",expires_at:future(-DAY)},{id:"soon",expires_at:future(2*DAY)}],now)?.id,"soon");
});
test("reminder boundaries at 7d 3d 2d 1d 5h",()=>{
 for(const [ms,stage] of [[7*DAY,"7d"],[3*DAY,"3d"],[2*DAY,"2d"],[DAY,"1d"],[5*HOUR,"5h"]])
  assert.equal(bankedWarningStage(future(ms),now),stage);
 assert.equal(bankedWarningStage(future(8*DAY),now),null);
 assert.equal(bankedWarningStage(future(-HOUR),now),null);
 assert.equal(bankedCountdown(future(DAY+2*HOUR+30*60000),now),"1天 2小時 30分");
});
test("5h whole-hour ticks and weekly local-midnight ticks",()=>{
 const at=new Date("2026-10-30T10:32:00").getTime();
 assert.ok(timelineTicks(at,at+5*HOUR,false).every(t=>new Date(t).getMinutes()===0));
 assert.ok(timelineTicks(at,at+7*DAY,true).every(t=>new Date(t).getHours()===0));
});
test("history 06/12/18 labels exist with no real 5h window",()=>{
 const at=new Date("2026-10-30T00:00:00").getTime();
 assert.deepEqual(historyFallbackTicks(at,at+DAY).map(t=>new Date(t).getHours()),[6,12,18]);
});

test("all valid banked credits sorted by expiry, including equal-expiry entries",()=>{
 const credits=[
  {id:"last",expires_at:future(8*DAY)},{id:"expired",expires_at:future(-DAY)},
  {id:"first",expires_at:future(DAY)},{id:"second",expires_at:future(3*DAY)}
 ];
 assert.deepEqual(activeCreditsByExpiry(credits,now).map(c=>c.id),["first","second","last"]);
 assert.equal(credits[0].id,"last"); // API payload remains unchanged
});
test("compact expiration omits year, seconds and timezone suffix",()=>{
 const display=compactLocalDateTime("2026-11-06T22:22:29.313184Z");
 assert.match(display,/\d{1,2}\/\d{1,2}\s+\d{2}:\d{2}/);
 assert.doesNotMatch(display,/2026|Z|313184/);
});
test("tooltip stays inside chart and prefers pointer left",()=>{
 assert.deepEqual(chartTooltipPosition([250,85],[360,180],[120,60]),[112,55]);
 assert.deepEqual(chartTooltipPosition([20,8],[360,180],[120,60]),[38,6]);
 const [x,y]=chartTooltipPosition([5,175],[150,180],[125,80]);
 assert.ok(x>=0&&x+125<=150&&y>=0&&y+80<=180);
});
test("quota edge labels take priority over intermediate ticks",()=>{
 const from=0,to=5*HOUR,width=320;
 const visible=quotaInteriorTicks([HOUR,2*HOUR,3*HOUR,4*HOUR],from,to,width,false);
 assert.ok(visible.every(t=>{const x=5+(width-10)*t/to;return x>=72&&x<=width-72;}));
 assert.ok(visible.length<4);
});

test("manual banked redemption follows the official strict below-80% rule",()=>{
 const quota=(five,weekly,plan="plus")=>bankedResetEligibility({five_hour:five,weekly,plan});
 assert.equal(quota(0,2).eligible,false); // Screenshot: 5h 100%, week 98%
 assert.equal(quota(20,20).eligible,false); // Remaining exactly 80% does not qualify
 assert.equal(quota(20.1,0).eligible,true);
 assert.equal(quota(0,20.1).eligible,true);
 assert.equal(quota(100,0).eligible,true);
 assert.equal(quota(100,19,"pro").eligible,false); // Pro covers only weekly
 assert.equal(quota(100,20.1,"pro").eligible,true);
 assert.equal(quota(null,null).known,false);
 assert.equal(quota(Number.NaN,null).eligible,false);
 assert.equal(bankedQuotaLabel(98),"98%");
 assert.equal(bankedQuotaLabel(null),"尚無資料");
});
