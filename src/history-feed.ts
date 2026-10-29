// Build a recent-first history feed without losing underlying diagnostic data.
export type FeedSample={at:string;five_hour:number|null;weekly:number|null;five_reset:string|null;weekly_reset:string|null};
export type FeedEvent={at:string;kind:string;detail:string};
export type FeedItem={at:string;kind:"event"|"sample";detail:string;source:FeedEvent|FeedSample};
const at=(x:{at:string})=>Date.parse(x.at);
const remaining=(n:number|null)=>n===null?"—":Math.round(Math.min(100,Math.max(0,100-n)))+"%";
const significant=(a:FeedSample,b:FeedSample)=>(
 (a.five_hour!==null&&b.five_hour!==null&&Math.abs(a.five_hour-b.five_hour)>=1.5)||
 (a.weekly!==null&&b.weekly!==null&&Math.abs(a.weekly-b.weekly)>=1.5)
);
export function buildHistoryFeed(samples:readonly FeedSample[],events:readonly FeedEvent[],now:number){
 const ordered=[...samples].filter(s=>Number.isFinite(at(s))).sort((a,b)=>at(a)-at(b));
 const latest=ordered.at(-1);
 const items:FeedItem[]=[];
 for(const e of events){
  if(!Number.isFinite(at(e)))continue;
  items.push({kind:"event",at:e.at,detail:e.detail,source:e});
 }
 const raw:FeedItem[]=[];
 for(let i=0;i<ordered.length;i++){
  const s=ordered[i];
  const item:FeedItem={kind:"sample",at:s.at,
   detail:"5h "+remaining(s.five_hour)+" 剩餘 · 週 "+remaining(s.weekly)+" 剩餘",source:s};
  // The top summary already contains the latest sample. Do not repeat it.
  if(i!==ordered.length-1)raw.push(item);
  if(i>0&&i!==ordered.length-1&&significant(ordered[i-1],s))items.push(item);
 }
 items.sort((a,b)=>at(b)-at(a)||Number(b.kind==="event")-Number(a.kind==="event"));
 const recentCutoff=now-48*3600_000;
 const recent=items.filter(x=>at(x)>=recentCutoff).slice(0,12);
 const recentSet=new Set(recent);
 const archive=items.filter(x=>!recentSet.has(x));
 // Raw samples are never discarded; detailed entries are opt-in.
 const details=raw.sort((a,b)=>at(b)-at(a));
 return {latest,recent,archive,details};
}
