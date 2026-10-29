// Pure 5-hour window calculations shared by Android and Windows charts.
export const FIVE_HOUR_MS=5*60*60*1000;
const RESET_TOLERANCE=2*60*1000;
export type HistoryQuotaSample={at:string;five_hour:number|null;five_reset:string|null};
export type FiveHourWindow={start:number;end:number;estimated:boolean};
export type ScheduledStart={at:number;kind:string;auto_count:number};
const millis=(iso:string|null|undefined)=>iso?Date.parse(iso):NaN;
const valid=(n:number)=>Number.isFinite(n)&&n>0;
export function clockLabel(time:number,includeDate=false):string{
 const d=new Date(time);
 const hours=String(d.getHours()).padStart(2,"0");
 const minute=d.getMinutes();
 const clock=hours+(minute===0?"":":"+String(minute).padStart(2,"0"));
 return includeDate?(d.getMonth()+1)+"/"+d.getDate()+" "+clock:clock;
}
// The backend may return a moving "now + 5h" reset while idle.
// A positive usage sample, multiple stable samples, or an explicitly confirmed
// current window is required before treating a reset as an actual boundary.
export type QuotaKey="five_hour"|"weekly";
export type QuotaHistorySample=HistoryQuotaSample&{weekly?:number|null;weekly_reset?:string|null};
export type QuotaWindow=FiveHourWindow&{nominalStart?:number;nominalEnd?:number};

// Each reset replaces the previous ideal prediction, even if the old reset_at
// was much later. Preserve the original slope, only shorten the visible segment.
export function interruptAtNextReset<T extends QuotaWindow>(windows:readonly T[]):T[]{
 const sorted=windows.filter(w=>valid(w.start)&&valid(w.end)&&w.end>w.start)
  .sort((a,b)=>a.start-b.start||a.end-b.end);
 const distinct:T[]=[];
 for(const w of sorted){
  const last=distinct.at(-1);
  if(last&&Math.abs(w.start-last.start)<=RESET_TOLERANCE){
   // One active window at a time: newer or observed overrides speculation.
   if(w.end>last.end||(!w.estimated&&last.estimated))distinct[distinct.length-1]={...w};
  }else distinct.push({...w});
 }
 return distinct.map((w,i)=>({
  ...w,nominalStart:w.nominalStart??w.start,nominalEnd:w.nominalEnd??w.end,
  end:Math.min(w.end,distinct[i+1]?.start??Infinity)
 })).filter(w=>w.end>w.start);
}

// A repeated stable reset_at is NOT proof that a five-hour window started.
// Idle API placeholders may stay unchanged for multiple samples or jump with
// rounding. Historical five-hour windows require actual consumption; the
// current confirmed window is separately supplied by the live snapshot.
// The weekly reset remains fixed until natural or Global Reset, and a changed
// reset timestamp starts a replacement window rather than an overlapping line.
export function observedQuotaWindows(
 samples:readonly QuotaHistorySample[],
 kind:QuotaKey,
 currentReset?:string|null,
 confirmed=false,
 now=Date.now()
):QuotaWindow[]{
 const span=kind==="five_hour"?FIVE_HOUR_MS:7*24*FIVE_HOUR_MS/5;
 type Group={end:number;count:number;first:number;last:number;used:boolean;current:boolean};
 const groups:Group[]=[];
 function ingest(at:number,end:number,used:boolean,current=false){
  if(!valid(at)||!valid(end)||end<at-RESET_TOLERANCE||end>at+span+RESET_TOLERANCE)return;
  let g=groups.find(x=>Math.abs(x.end-end)<=RESET_TOLERANCE);
  if(!g){g={end,count:0,first:at,last:at,used:false,current:false};groups.push(g);}
  g.count++;g.first=Math.min(g.first,at);g.last=Math.max(g.last,at);
  g.used ||=used;g.current ||=current;
 }
 for(const s of samples){
  const used=kind==="five_hour"?s.five_hour:s.weekly;
  const end=kind==="five_hour"?s.five_reset:s.weekly_reset;
  ingest(millis(s.at),millis(end),kind==="five_hour"&&typeof used==="number"&&used>0.05);
 }
 // Only an active/confirmed 5h window can become the current projected anchor.
 // For weekly the server's current reset_at identifies the latest window.
 if(kind==="weekly"||confirmed)ingest(now,millis(currentReset),false,true);
 const list=groups.filter(g=>kind==="five_hour"
  ?(g.used||g.current)
  :(g.current||g.count>=2&&g.last-g.first>=60_000))
  .map(g=>({start:g.end-span,end:g.end,nominalStart:g.end-span,
   nominalEnd:g.end,estimated:false}));
 return interruptAtNextReset(list);
}
export function idealQuotaSegment(w:QuotaWindow,from:number,to:number,span:number):number[][]{
 const a=Math.max(w.start,from),b=Math.min(w.end,to);
 if(b<=a)return [];
 const anchor=w.nominalStart??w.start;
 return [[a,Math.max(0,100-100*(a-anchor)/span)],
         [b,Math.max(0,100-100*(b-anchor)/span)]];
}
export function readableBoundaries(ticks:readonly number[],from:number,to:number,widthPx:number):number[]{
 const gapMs=48*Math.max(1,to-from)/Math.max(1,widthPx);
 const result:number[]=[];
 for(const t of [...ticks].sort((a,b)=>a-b)){
  if(t<from||t>to)continue;
  if(result.length&&t-result[result.length-1]<gapMs)continue;
  result.push(t);
 }
 return result;
}
export function actualFiveHourWindows(
 samples:readonly HistoryQuotaSample[],
 currentReset?:string|null,
 confirmed=false,
 now=Date.now()
):FiveHourWindow[]{
 return observedQuotaWindows(samples,"five_hour",currentReset,confirmed,now);
}
export function scheduledFiveHourWindows(
 next:ScheduledStart|null|undefined,
 now:number,
 visibleStart:number,
 visibleEnd:number,
 actual:readonly FiveHourWindow[]
):FiveHourWindow[]{
 if(!next||!valid(next.at)||next.at*1000<now-60_000)return [];
 const initial=next.at*1000;
 // Only project followups when explicitly scheduled from an anchor.
 const count=next.kind==="anchor"?Math.min(Math.max(next.auto_count,0),6):0;
 const proposed:Array<FiveHourWindow>=[];
 for(let i=0;i<=count;i++){
  const start=initial+i*FIVE_HOUR_MS,end=start+FIVE_HOUR_MS;
  if(start>=visibleEnd||end<=visibleStart)continue;
  if(actual.some(w=>start<w.end&&end>w.start))continue;
  proposed.push({start,end,estimated:true});
 }
 return proposed;
}
export function idealSegment(w:FiveHourWindow,from:number,to:number):number[][]{
 const a=Math.max(w.start,from),b=Math.min(w.end,to);
 if(b<=a)return [];
 return [[a,100-100*(a-w.start)/FIVE_HOUR_MS],[b,100-100*(b-w.start)/FIVE_HOUR_MS]];
}
export function windowBoundaries(windows:readonly FiveHourWindow[],from:number,to:number):number[]{
 const ticks=windows.flatMap(w=>[w.start,w.end])
  .filter(x=>x>=from&&x<=to).sort((a,b)=>a-b);
 return ticks.filter((t,i)=>i===0||t-ticks[i-1]>RESET_TOLERANCE);
}
