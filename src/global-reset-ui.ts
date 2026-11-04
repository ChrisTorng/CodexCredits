import {compactLocalDateTime} from "./credit-ui.ts";
export type GlobalResetSummary={
 last_reset_at:string|null;avg_days:number|null;median_days:number|null;
 median_estimate_at?:string|null;p24?:number|null;p48?:number|null;age_days?:number|null
};
/** An expired point forecast is not a forecast of the remaining wait.
 * Old unconditional 24/48h probabilities cannot be reinterpreted as
 * probabilities conditional on a long elapsed interval. */
export function globalResetSummary(global:GlobalResetSummary|null,now=Date.now()):string{
 if(!global?.last_reset_at)return "尚無資料 · codex-reset.com";
 const last=Date.parse(global.last_reset_at);
 if(!Number.isFinite(last)||last>now)return "Global Reset 記錄時間異常，等待重新更新";
 const age=(now-last)/86400000;
 const stats=global.avg_days!=null&&global.median_days!=null?
  "\n近期間隔 平均 "+global.avg_days.toFixed(1)+" 天 / 中位數 "+global.median_days.toFixed(1)+" 天":"";
 const overdue=global.median_days!=null&&age>global.median_days;
 return "上次 "+compactLocalDateTime(global.last_reset_at)+" · 已 "+age.toFixed(1)+" 天"+stats+
  (overdue?"\n已超過近期中位數；無可靠的剩餘時間推估，下一次可能隨時發生":
   "\n歷史間隔僅供參考，尚無可靠的下次重置時間推估");
}
