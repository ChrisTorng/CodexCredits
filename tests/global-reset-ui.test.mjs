import test from "node:test";
import assert from "node:assert/strict";
import {globalResetSummary} from "../src/global-reset-ui.ts";
const last="2026-10-28T06:35:00Z";
test("outdated point prediction is never displayed as a future forecast",()=>{
 const now=Date.parse("2026-11-04T14:30:00Z");
 const text=globalResetSummary({last_reset_at:last,avg_days:2.1,median_days:2,
  median_estimate_at:"2026-10-30T08:41:00Z",p24:36,p48:59,age_days:2},now);
 assert.match(text,/上次/);
 assert.match(text,/已 7\.3 天/);
 assert.match(text,/已超過近期中位數/);
 assert.doesNotMatch(text,/10\/30|24h|48h|36%|59%|推估 10\/30/);
});
test("future reset estimates do not imply certainty",()=>{
 const text=globalResetSummary({last_reset_at:"2026-11-04T00:00:00Z",avg_days:3,median_days:2},
  Date.parse("2026-11-04T12:00:00Z"));
 assert.match(text,/歷史間隔僅供參考/);
});
