/** Shared Android/Windows Banked Reset countdown and local clock ticks. */
export type CreditExpiry={id:string;expires_at:string};
export const HOUR=3600000,DAY=24*HOUR;
export function nearestCredit<T extends CreditExpiry>(items:T[],now=Date.now()):T|undefined{
 return items.filter(c=>Number.isFinite(Date.parse(c.expires_at))&&Date.parse(c.expires_at)>now)
  .sort((a,b)=>Date.parse(a.expires_at)-Date.parse(b.expires_at))[0];
}
export function bankedCountdown(expiry:string,now=Date.now()):string{
 const ms=Date.parse(expiry)-now;
 if(!Number.isFinite(ms))return "到期時間不明";
 if(ms<=0)return "已到期";
 const d=Math.floor(ms/DAY),h=Math.floor(ms%DAY/HOUR),m=Math.floor(ms%HOUR/60000);
 return [d?d+"天":null,d||h?h+"小時":null,m+"分"].filter(Boolean).join(" ");
}
export function bankedWarningStage(expiry:string,now=Date.now()):string|null{
 const ms=Date.parse(expiry)-now;
 if(!Number.isFinite(ms)||ms<=0||ms>7*DAY)return null;
 if(ms<=5*HOUR)return "5h";
 if(ms<=DAY)return "1d";
 if(ms<=2*DAY)return "2d";
 if(ms<=3*DAY)return "3d";
 return "7d";
}
export function timelineTicks(start:number,end:number,weekly:boolean):number[]{
 if(!Number.isFinite(start)||!Number.isFinite(end)||end<=start)return [];
 const t=new Date(start),out:number[]=[];
 if(weekly){t.setHours(0,0,0,0);t.setDate(t.getDate()+1);}
 else{t.setMinutes(0,0,0);t.setHours(t.getHours()+1);}
 for(let i=0;i<200&&t.getTime()<end;i++){
  if(t.getTime()>start)out.push(t.getTime());
  if(weekly)t.setDate(t.getDate()+1);else t.setHours(t.getHours()+1);
 }
 return out;
}
export function historyFallbackTicks(start:number,end:number):number[]{
 const out:number[]=[],d=new Date(start);d.setHours(0,0,0,0);
 for(let i=0;i<3&&d.getTime()<end;i++){
  for(const h of [6,12,18]){const at=new Date(d);at.setHours(h,0,0,0);if(at.getTime()>start&&at.getTime()<end)out.push(at.getTime());}
  d.setDate(d.getDate()+1);
 }
 return out;
}
