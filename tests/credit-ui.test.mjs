import test from "node:test";
import assert from "node:assert/strict";
import {nearestCredit,bankedCountdown,bankedWarningStage,timelineTicks,historyFallbackTicks,DAY,HOUR} from "../src/credit-ui.ts";
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
