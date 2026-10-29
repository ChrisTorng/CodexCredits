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
export function actualFiveHourWindows(
 samples:readonly HistoryQuotaSample[],
 currentReset?:string|null,
 confirmed=false,
 now=Date.now()
):FiveHourWindow[]{
 type Candidate={end:number;count:number;first:number;last:number;used:boolean;confirmed:boolean};
 const candidates:Candidate[]=[];
 function ingest(at:number,end:number,used:boolean,isConfirmed=false){
  if(!valid(at)||!valid(end)||end<at-RESET_TOLERANCE||end>at+FIVE_HOUR_MS+RESET_TOLERANCE)return;
  let item=candidates.find(c=>Math.abs(c.end-end)<=RESET_TOLERANCE);
  if(!item){item={end,count:0,first:at,last:at,used:false,confirmed:false};candidates.push(item);}
  item.count++;item.first=Math.min(item.first,at);item.last=Math.max(item.last,at);
  item.used ||=used;item.confirmed ||=isConfirmed;
 }
 for(const sample of samples){
  const at=millis(sample.at),end=millis(sample.five_reset);
  ingest(at,end,typeof sample.five_hour==="number"&&sample.five_hour>0.05);
 }
 if(confirmed)ingest(now,millis(currentReset),false,true);
 const result=candidates
  .filter(c=>c.used||c.confirmed||(c.count>=2&&c.last-c.first>=60_000))
  .map(c=>({start:c.end-FIVE_HOUR_MS,end:c.end,estimated:false}));
 result.sort((a,b)=>a.start-b.start);
 return result.filter((w,i)=>i===0||Math.abs(w.end-result[i-1].end)>RESET_TOLERANCE);
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
