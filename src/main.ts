import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {isPermissionGranted,requestPermission} from "@tauri-apps/plugin-notification";
// Register only the line charts and components used by this UI.
import * as echarts from "echarts/core";
import {LineChart} from "echarts/charts";
import {GridComponent,TooltipComponent,LegendComponent,TitleComponent,MarkLineComponent,GraphicComponent} from "echarts/components";
import {CanvasRenderer} from "echarts/renderers";

echarts.use([LineChart,GridComponent,TooltipComponent,LegendComponent,TitleComponent,MarkLineComponent,GraphicComponent,CanvasRenderer]);
import {actualFiveHourWindows,observedQuotaWindows,scheduledFiveHourWindows,interruptAtNextReset,idealSegment,idealQuotaSegment,windowBoundaries,readableBoundaries,clockLabel,FIVE_HOUR_MS} from "./chart-windows";
import {buildHistoryFeed} from "./history-feed";
import {nearestCredit,bankedCountdown,timelineTicks,historyFallbackTicks} from "./credit-ui";
import "./style.css";

type Mode="off"|"auto"|"custom";
type Rule={id:string;enabled:boolean;days:boolean[];hour:number;minute:number;auto_count:number};
type Override={enabled:boolean;rules:Rule[]};
type Settings={mode:Mode;sampling_minutes:number;retention_days:number;rules:Rule[];overrides:Record<string,Override>;ntfy_topic:string;theme:string;native_notify_enabled:boolean};
type Usage={five_hour:number|null;weekly:number|null;five_reset:string|null;weekly_reset:string|null;primary_confirmed:boolean;plan:string;allowed:boolean;limit_reached:boolean;checked_at:string};
type Credit={id:string;title:string;description:string;expires_at:string};
type Snapshot={settings:Settings;usage:Usage|null;credits:{count:number;available:Credit[]}|null;global:{last_reset_at:string|null;age_days:number|null;p24:number|null;p48:number|null;avg_days:number|null;median_days:number|null;median_estimate_at:string|null}|null;armed:boolean;ntfy_status:string;next:{at:number;kind:string;reason:string;auto_count:number}|null;error:string|null;signed_in:boolean};
type Sample={at:string;five_hour:number|null;weekly:number|null;five_reset:string|null;weekly_reset:string|null};
type Event={at:string;kind:string;detail:string};
const $=(id:string)=>document.getElementById(id)!;
const app=$("app");
app.innerHTML=[
'<header class="app-header"><h1>Codex Credits</h1><button class="refresh-icon" id="top-refresh" title="更新配額">↻</button></header>',
'<nav class="tabs" aria-label="功能選單"><button class="tab active" data-tab="status">● 狀態</button><button class="tab" data-tab="schedule">◷ 排程</button><button class="tab" data-tab="history">▥ 歷程</button><button class="tab" data-tab="settings">⚙ 設定</button></nav>',
'<div class="content">',
'<main class="page visible" id="page-status">',
'<p id="account" class="account">ChatGPT 尚未登入</p>',
'<section class="card quota"><div class="card-title"><strong>5 小時</strong><strong id="primary-percent">—</strong></div><div class="quota-chart" id="primary-chart"></div></section>',
'<section class="card quota"><div class="card-title"><strong>週配額</strong><strong id="weekly-percent">—</strong></div><div class="quota-chart" id="weekly-chart"></div></section>',
'<section class="card compact"><h2>下一次排程</h2><p id="status-next">尚未排程</p></section>',
'<p id="trigger-preview" class="note">請先登入</p>',
'<div class="button-row"><button id="check" class="action">更新配額</button><button id="trigger" class="action">立即觸發</button></div>',
'<section class="card banked"><div class="card-title"><strong id="reset-title">Banked Reset</strong><button id="reset-action" class="banked-button">Banked Reset</button></div><div id="reset-expiry" class="meta"></div></section>',
'<section class="card compact"><h2>Global Reset</h2><p id="global-reset">尚無資料 · codex-reset.com</p></section>',
'</main>',
'<main class="page" id="page-schedule">',
'<h3>模式</h3><div class="button-row mode-row"><select id="mode"><option value="off">關閉</option><option value="auto">配額重置後自動觸發</option><option value="custom">錨點＋自動接續</option></select><button id="apply-mode">套用</button></div>',
'<section class="card compact"><h2>下次執行</h2><p id="alarm">尚未排程</p></section>',
'<section class="card compact"><h2>預估</h2><p id="schedule-preview">排程已關閉</p></section>',

'<section class="card editor-card"><div class="card-title"><strong>每週預設</strong><button class="small" id="weekly-edit">編輯</button></div><p id="weekly-summary">—</p></section>',
'<section class="card editor-card"><div class="card-title"><strong>今天</strong><button class="small" id="today-edit">編輯</button></div><p id="today-summary">使用每週預設</p></section>',
'<section class="card editor-card"><div class="card-title"><strong>明天</strong><button class="small" id="tomorrow-edit">編輯</button></div><p id="tomorrow-summary">使用每週預設</p></section>',
'</main>',
'<main class="page history-page" id="page-history">',
'<div class="history-charts"><div class="history-chart" id="yesterday-chart"></div><div class="history-chart" id="today-chart"></div></div>',

'<section class="card history-events"><h2>事件與採樣</h2><div id="event-list">尚無配額採樣</div></section>',
'</main>',
'<main class="page" id="page-settings">',
'<section class="card compact"><h2>ChatGPT</h2><p id="settings-account">尚未登入</p></section>',
'<div class="button-row"><button class="action" id="login">登入</button><button class="action" id="logout">登出</button></div>',
'<div class="setting-row"><label for="theme">主題</label><select id="theme"><option value="system">跟隨系統</option><option value="light">淺色</option><option value="dark">深色</option></select></div>',
'<div class="setting-row"><label for="sample">採樣間隔</label><select id="sample"><option value="10">10 分鐘</option><option value="15">15 分鐘</option><option value="30">30 分鐘</option><option value="60">60 分鐘</option></select></div>',
'<h3>通知</h3><div class="button-row"><label class="notification-switch"><input type="checkbox" id="native-toggle">啟用通知</label><button id="native-permission" class="small" hidden>允許權限</button></div>',
'<h3>ntfy（選用，預設關閉）</h3><input id="ntfy" type="url" spellcheck="false" placeholder="未設定，按「建立」產生隨機專屬網址">',
'<div class="button-row ntfy-actions"><button id="ntfy-new">建立</button><button id="ntfy-test">測試</button><button id="ntfy-copy">複製</button><button id="ntfy-save">儲存</button></div>',
'<p class="note">最近發送：<span id="ntfy-status">未設定</span></p>',


'</main></div>',
'<div role="status" aria-live="polite" id="toast" class="toast" hidden><span id="toast-message"></span><button type="button" id="toast-close" aria-label="關閉通知" title="關閉">×</button></div>',
'<dialog id="reset-dialog" class="modal"><h2 id="reset-dialog-title">Banked Reset</h2><p id="reset-dialog-desc"></p><div class="dialog-buttons">',
'<button id="reset-arm">用盡後自動使用一次</button><button id="reset-exhausted">已用盡，立即重置</button><button id="reset-now">立即使用（含部分用量）</button><button id="reset-cancel">取消</button></div></dialog>',
'<dialog id="rule-dialog" class="modal"><h2 id="rule-title">編輯排程</h2><p class="note">每個錨點可設定「後續自動」次數；每次依實際 5h reset 決定接續時間。</p><label class="override-row" id="override-wrap" hidden><input type="checkbox" id="override-checkbox">覆寫每週預設</label>',
'<p class="note" id="override-hint" hidden>關閉覆寫＝照每週預設；開啟但不加任何時段＝當天完全不觸發。</p><div id="rule-list"></div>',
'<button class="action" id="rule-add">＋ 新增錨點</button><button class="action" id="rule-recommended">套用建議：06:00 ＋ 接續 3 次</button>',
'<div class="dialog-footer"><button id="rule-cancel">取消</button><button class="primary" id="rule-save">儲存</button></div></dialog>'
].join("");
const androidRuntime=/Android/i.test(navigator.userAgent);
let checkAlarmPolicy:()=>Promise<void>=async()=>{};
if(androidRuntime){
  // Advanced system settings appear only when a schedule is enabled and
  // the essential exact-alarm permission is missing.
  const policy=document.createElement("section");
  policy.className="card compact";
  policy.hidden=true;
  policy.innerHTML='<h2>排程權限需要設定</h2><p>精準鬧鐘尚未允許，排程可能無法準時執行。</p><div class="button-row"><button id="android-exact">開啟設定</button></div>';
  document.querySelector("#page-settings")?.append(policy);
  checkAlarmPolicy=async()=>{
    try{
      const v=await invoke<{exact_alarm:boolean}>("android_platform_status");
      policy.hidden=v.exact_alarm || snapshot?.settings.mode==="off";
    }catch{
      // A failed optional permission probe is not a permanent warning.
      policy.hidden=true;
    }
  };
  document.getElementById("android-exact")?.addEventListener("click",async()=>{
    try{await invoke("android_open_system_settings",{kind:"exact"});}
    catch(e){toast(String(e));}
  });
  document.addEventListener("visibilitychange",()=>{if(!document.hidden)void checkAlarmPolicy();});
}

let snapshot:Snapshot|null=null;
let samples:Sample[]=[];
const chronological=(rows:Sample[]):Sample[]=>rows.filter(s=>Number.isFinite(Date.parse(s.at)))
 .sort((a,b)=>Date.parse(a.at)-Date.parse(b.at));
let events:Event[]=[];
let expandedHistory=false;
let expandedSamples=false;
let page="status";
let theme="system";
let notificationPermission:boolean|null=null;
let notificationPermissionError="";
let toastTimer:ReturnType<typeof setTimeout>|undefined;
let lastDisplayedError="";
let loginInProgress=false;
let lastForegroundRefresh=0;
let refreshing=false;
let editing:"weekly"|"today"|"tomorrow"="weekly";
let draft:Rule[]=[];
let overrideOn=false;
const charts:Record<string,echarts.ECharts>={};
const chartIds=["primary-chart","weekly-chart","yesterday-chart","today-chart"] as const;
const getChart=(id:string)=>charts[id]??(charts[id]=echarts.init($(id),undefined,{renderer:"canvas"}));
// Hidden pages have no layout box. Let ECharts resize only the visible instances.
const resizeVisibleCharts=()=>{for(const id of chartIds){if($(id).getClientRects().length)charts[id]?.resize();}};
const safeRemaining=(used:number|null|undefined)=>used==null?null:Math.max(0,Math.min(100,100-used));
const percent=(used:number|null|undefined)=>{const n=safeRemaining(used);return n==null?"—":Math.round(n)+"% 剩餘"};
const fdate=(s:string|null|undefined)=>s?new Date(s).toLocaleString("zh-TW",{month:"numeric",day:"numeric",hour:"2-digit",minute:"2-digit",hour12:false}):"—";
const ftime=(s:string|null|undefined)=>s?new Date(s).toLocaleTimeString("zh-TW",{hour:"2-digit",minute:"2-digit",hour12:false}):"—";
const today=(offset=0)=>{const d=new Date();d.setDate(d.getDate()+offset);return [d.getFullYear(),String(d.getMonth()+1).padStart(2,"0"),String(d.getDate()).padStart(2,"0")].join("-")};
const hhmm=(r:Rule)=>String(r.hour).padStart(2,"0")+":"+String(r.minute).padStart(2,"0");
const ruleLabel=(r:Rule,weekly:boolean)=>{
 const days=["一","二","三","四","五","六","日"].filter((_,i)=>r.days[i]).join(" ");
 const base=hhmm(r);
 let continuation="";
 for(let i=1;i<=r.auto_count;i++){
  const t=(r.hour*60+r.minute+i*300)%1440;
  continuation+=" → "+String(Math.floor(t/60)).padStart(2,"0")+":"+String(t%60).padStart(2,"0");
 }
 return (weekly?(days||"未選")+"\n":"")+base+continuation;
};
const allRules=(date:string)=>{
 if(!snapshot)return [];
 const ov=snapshot.settings.overrides[date];
 if(ov?.enabled)return ov.rules.filter(r=>r.enabled);
 const day=(new Date(date+"T12:00:00").getDay()+6)%7;
 return snapshot.settings.rules.filter(r=>r.enabled&&r.days[day]);
};
const closeToast=()=>{
 if(toastTimer!==undefined)clearTimeout(toastTimer);
 toastTimer=undefined;
 $("toast").hidden=true;
};
const toast=(message:string)=>{
 closeToast();
 $("toast-message").textContent=message;
 $("toast").hidden=false;
 // Keep warnings a little longer but never cover the page indefinitely.
 const duration=/失敗|錯誤|401|無法|權限|逾期|不允許/i.test(message)?8000:4000;
 toastTimer=setTimeout(closeToast,duration);
};
$("toast-close").onclick=closeToast;
const call=async<T>(name:string,args?:Record<string,unknown>):Promise<T>=>invoke<T>(name,args);

function showPage(name:string){
 if(name!==page)closeToast();
 page=name;
 if(name==="settings"){void refreshNotificationPermission();void checkAlarmPolicy();}
 for(const btn of Array.from(document.querySelectorAll<HTMLButtonElement>(".tab"))){
  const selected=btn.dataset.tab===name;
  btn.classList.toggle("active",selected);btn.setAttribute("aria-selected",String(selected));
 }
 for(const item of Array.from(document.querySelectorAll<HTMLElement>(".page"))){
  item.classList.toggle("visible",item.id==="page-"+name);
 }
 window.requestAnimationFrame(()=>{resizeVisibleCharts();draw();});
}
for(const btn of Array.from(document.querySelectorAll<HTMLButtonElement>(".tab"))){
 btn.onclick=()=>showPage(btn.dataset.tab??"status");
}
function applyTheme(value:string){
 theme=value;
 const dark=value==="dark"||(value==="system"&&matchMedia("(prefers-color-scheme: dark)").matches);
 document.documentElement.dataset.theme=dark?"dark":"light";
 draw();
}
matchMedia("(prefers-color-scheme: dark)").addEventListener("change",()=>{if(theme==="system")applyTheme(theme)});
// Apply the OS palette before settings load. The default setting is already
// "system", so a change-only check in render() cannot perform this first step.
applyTheme("system");
function renderNotificationStatus(){
 if(!snapshot)return;
 // The checkbox already conveys opt-in state. Show only actionable denial.
 ($("native-permission") as HTMLButtonElement).hidden=
   !(snapshot.settings.native_notify_enabled && notificationPermission===false);
}
async function refreshNotificationPermission(){
 try{notificationPermission=await isPermissionGranted();notificationPermissionError="";}
 catch(e){notificationPermission=null;notificationPermissionError=String(e);}
 renderNotificationStatus();
}
function chartPalette(){return theme==="dark"||(theme==="system"&&matchMedia("(prefers-color-scheme: dark)").matches)
 ?{text:"#a6acb7",grid:"#3d424b",bg:"#1f2227"}:{text:"#737983",grid:"#e2e5ea",bg:"#ffffff"};}
// Draw precise beginning/end markers instead of rounded fixed-hour ticks.
function paintQuota(id:string,used:number|null|undefined,reset:string|null|undefined,weekly:boolean,confirmed:boolean){
 const chart=getChart(id),p=chartPalette();
 const until=reset?Date.parse(reset):NaN,span=weekly?7*24*3600000:FIVE_HOUR_MS;
 const valid=Number.isFinite(until)&&until>0&&(weekly||confirmed);
 const start=valid?until-span:0;
 const source=valid?samples.filter(x=>{
  const t=Date.parse(x.at),r=weekly?x.weekly_reset:x.five_reset;
  return t>=start-60000&&t<=until+60000&&r&&Math.abs(Date.parse(r)-until)<=120000;
 }):[];
 const color=weekly?"#e89134":"#2f80ed";
 const actual=source.map(x=>[Date.parse(x.at),safeRemaining(weekly?x.weekly:x.five_hour)]);
 const ideal=valid?[[start,100],[until,0]]:[];
 const now=Date.now();
 const inWindow=valid&&now>=start&&now<=until;
 const ticks=valid?timelineTicks(start,until,weekly):[];
 const tickLabels=ticks.map(t=>({
  type:"text",silent:true,bottom:0,
  left:Math.max(3,Math.min(chart.getWidth()-38,5+(chart.getWidth()-10)*(t-start)/span-(weekly?14:9))),
  style:{text:weekly?new Date(t).toLocaleDateString("zh-TW",{month:"numeric",day:"numeric"}):
    new Date(t).toLocaleTimeString("zh-TW",{hour:"2-digit",minute:"2-digit",hour12:false}),
   fill:p.text,fontSize:9}
 }));
 const boundary=(time:number,right:boolean)=>({
  type:"text",bottom:0,...(right?{right:0}:{left:0}),
  silent:true,style:{text:clockLabel(time,true),fill:p.text,fontSize:10}
 });
 chart.setOption({
  backgroundColor:"transparent",animation:false,
  grid:{left:5,right:5,top:6,bottom:20,containLabel:false},
  xAxis:{type:"time",min:valid?start:undefined,max:valid?until:undefined,
   axisLine:{show:false},splitLine:{show:!valid,lineStyle:{color:p.grid}},axisLabel:{show:!valid},axisTick:{show:false}},
  yAxis:{type:"value",min:0,max:100,interval:50,axisLabel:{show:false},axisLine:{show:false},axisTick:{show:false},splitLine:{lineStyle:{color:p.grid}}},
  tooltip:{trigger:"axis",valueFormatter:(v:unknown)=>typeof v==="number"?Math.round(v)+"% 剩餘":String(v)},
  series:[
   {name:"理想剩餘",type:"line",data:ideal,showSymbol:false,silent:true,lineStyle:{color,type:"dashed",width:1.5,opacity:.8},areaStyle:{color,opacity:.08}},
   {name:"實際剩餘",type:"line",data:valid?actual:[],showSymbol:false,lineStyle:{width:2.4,color},itemStyle:{color},areaStyle:{color,opacity:.18},
    markLine:valid?{symbol:["none","none"],silent:true,label:{show:false},
      lineStyle:{color:p.grid,width:1,type:"solid"},
      data:[{xAxis:start},{xAxis:until},...ticks.map(t=>({xAxis:t})),...(inWindow?[{xAxis:now,lineStyle:{color:p.text,type:"dashed",width:1}}]:[])]}:undefined}
  ],
  graphic:valid?[boundary(start,false),...tickLabels,boundary(until,true)]:[{
   type:"text",left:10,top:"45%",style:{text:
    !snapshot?.signed_in?"請先登入":
    !snapshot?.usage?.checked_at?"正在取得最新配額…":
    (now-Date.parse(snapshot.usage.checked_at)>15*60000)?"等待更新配額…":
    weekly?"未啟動":"尚未啟動 · 等待觸發",fill:p.text,fontSize:11}
  }]
 },true);
}
function paintHistory(id:string,offset:number){
 const chart=getChart(id),p=chartPalette();
 const day=new Date();day.setHours(0,0,0,0);day.setDate(day.getDate()+offset);
 const following=new Date(day);following.setDate(following.getDate()+1);
 const start=day.getTime(),end=following.getTime(),now=Date.now();
 const todaySamples=samples.filter(s=>{const t=Date.parse(s.at);return t>=start&&t<end;});
 const actual=(key:"five_hour"|"weekly")=>todaySamples.map((s,i)=>{
  const current=key==="five_hour"?s.five_reset:s.weekly_reset;
  const before=i>0?(key==="five_hour"?todaySamples[i-1].five_reset:todaySamples[i-1].weekly_reset):current;
  return [Date.parse(s.at),current&&before&&Math.abs(Date.parse(current)-Date.parse(before))>300000?null:safeRemaining(s[key])];
 });
 const u=snapshot?.usage;
 const history=actualFiveHourWindows(samples,u?.five_reset,!!u?.primary_confirmed,now);
 const planned=scheduledFiveHourWindows(snapshot?.next,now,start,end,history);
 const windows=interruptAtNextReset([...history,...planned]).filter(w=>w.end>start&&w.start<end);
 const weeklyWindows=observedQuotaWindows(samples,"weekly",u?.weekly_reset,false,now).filter(w=>w.end>start&&w.start<end);
 const boundaries=windowBoundaries(windows,start,end);
 const fallback=historyFallbackTicks(start,end).filter(t=>!boundaries.some(b=>Math.abs(b-t)<15*60000));
 const labels=readableBoundaries([...boundaries,...fallback].sort((a,b)=>a-b),start,end,chart.getWidth()-12);
 const plotWidth=Math.max(1,chart.getWidth()-10);
 const graphics=labels.map(t=>({
  type:"text",silent:true,
  left:Math.max(1,Math.min(chart.getWidth()-30,5+plotWidth*(t-start)/(end-start)-14)),
  top:Math.max(0,chart.getHeight()-14),
  style:{text:clockLabel(t),fill:p.text,fontSize:9}
 }));
 const globalTime=snapshot?.global?.last_reset_at?Date.parse(snapshot.global.last_reset_at):NaN;
 const globalMark=Number.isFinite(globalTime)&&globalTime>=start&&globalTime<end?
  {xAxis:globalTime,lineStyle:{type:"dashed",color:"#999",width:1}}:null;
 const markData=[...boundaries,...fallback].map(t=>({
  xAxis:t,lineStyle:{type:"solid",color:p.grid,width:1.3}
 }));
 const palette=[["5 小時","#2f80ed"],["週配額","#e89134"]] as const;
 const weeklyIdeal=()=>weeklyWindows.map(w=>({
  name:"週配額理想",type:"line",showSymbol:false,silent:true,
  data:idealQuotaSegment(w,start,end,7*24*3600000),
  lineStyle:{color:palette[1][1],type:"dashed",width:1.2,opacity:.85},
  tooltip:{show:false}
 }));
 chart.setOption({
  animation:false,backgroundColor:"transparent",graphic:graphics,
  title:{text:offset===0?"今天":"昨天",left:8,top:5,textStyle:{fontSize:12,color:p.text}},
  legend:{data:palette.map(x=>x[0]),top:2,right:8,textStyle:{fontSize:10,color:p.text},itemWidth:12,itemHeight:3},
  grid:{left:5,right:5,top:24,bottom:20},
  xAxis:{type:"time",min:start,max:end,interval:6*3600000,
   axisLabel:{show:boundaries.length===0,fontSize:9,color:p.text,hideOverlap:true,
    formatter:(v:number)=>clockLabel(v)},
   axisLine:{show:false},splitLine:{show:boundaries.length===0,lineStyle:{color:p.grid}},axisTick:{show:false}},
  yAxis:{type:"value",min:0,max:100,interval:50,axisLabel:{show:false},axisLine:{show:false},splitLine:{lineStyle:{color:p.grid}}},
  tooltip:{trigger:"axis",valueFormatter:(value:unknown)=>typeof value==="number"?Math.round(value)+"% 剩餘":String(value)},
  series:[
   ...windows.map(w=>({
    name:w.estimated?"5 小時預估理想":"5 小時理想",type:"line",data:idealSegment(w,start,end),
    showSymbol:false,silent:true,connectNulls:false,
    lineStyle:{color:palette[0][1],type:"dashed",width:1.5,opacity:w.estimated?.65:.9},
    tooltip:{show:false}
   })),
   ...weeklyIdeal(),
   {name:"5 小時",type:"line",data:actual("five_hour"),showSymbol:false,
    lineStyle:{color:palette[0][1],width:2},itemStyle:{color:palette[0][1]},
    markLine:markData.length?{symbol:["none","none"],silent:true,label:{show:false},data:markData}:undefined},
   {name:"週配額",type:"line",data:actual("weekly"),showSymbol:false,
    lineStyle:{color:palette[1][1],width:2},itemStyle:{color:palette[1][1]},
    markLine:globalMark?{symbol:["none","none"],silent:true,label:{show:false},data:[globalMark]}:undefined}
  ]
 },true);
}
function draw(){
 if(!snapshot)return;
 if(page==="status"){
  const u=snapshot.usage;
  paintQuota("primary-chart",u?.five_hour,u?.five_reset,false,!!u?.primary_confirmed);
  paintQuota("weekly-chart",u?.weekly,u?.weekly_reset,true,true);
 }
 if(page==="history"){
  paintHistory("yesterday-chart",-1);paintHistory("today-chart",0);
 }
}
function render(){
 if(!snapshot)return;
 const s=snapshot,u=s.usage,mode=s.settings.mode;
 const authInvalid=!!s.error&&/401|重新登入|登入逾期|憑證已失效/i.test(s.error);
 $("account").textContent=s.signed_in
  ?(authInvalid?"ChatGPT 憑證已失效 · 請重新登入":"ChatGPT 已登入 · "+(u?.plan??"—")+(u?.checked_at?" · 更新 "+fdate(u.checked_at):""))
  :"ChatGPT 尚未登入";
 $("primary-percent").textContent=percent(u?.five_hour);
 $("weekly-percent").textContent=percent(u?.weekly);
 $("status-next").textContent=mode==="off"?"已關閉 · 尚未排程":
   (mode==="auto"?"Reset 後自動 · ":"錨點＋接續 · ")+(s.next?fdate(new Date(s.next.at*1000).toISOString())+" · "+s.next.reason:"尚未排程");
 const confirmed=!!u?.primary_confirmed&&!!u.five_reset&&new Date(u.five_reset).getTime()>Date.now();
 const checkedAt=u?.checked_at?Date.parse(u.checked_at):0;
 const stale=!Number.isFinite(checkedAt)||checkedAt<=0||Date.now()-checkedAt>15*60000;
 $("trigger-preview").textContent=!s.signed_in?"請先登入":
   stale?"正在取得最新配額…":
   confirmed?"5h 進行中 · 可再開始："+fdate(u?.five_reset):
   "可立即觸發 · 預估重置 "+fdate(new Date(Date.now()+5*3600000).toISOString());
 ($("trigger") as HTMLButtonElement).disabled=!s.signed_in||stale||confirmed||!u?.allowed||u?.weekly===100;
 const credits=s.credits;const available=credits?.count??0;
 $("reset-title").textContent="Banked Reset ×"+available+(s.armed?" · 待命":"");
 $("reset-action").textContent="Banked Reset ×"+available+(s.armed?" · 待命":"");
 ($("reset-action") as HTMLButtonElement).hidden=available<=0&&!s.armed;
 const first=nearestCredit(credits?.available??[]);
 const expiry=first?.expires_at;
 $("reset-expiry").textContent=(s.armed?"用盡後自動重置 · ":"")+
  (expiry?"最近到期 "+fdate(expiry)+" · 剩 "+bankedCountdown(expiry):"目前沒有有效的 Banked Reset");
 const global=s.global;
 $("global-reset").textContent=global?.last_reset_at?
  "上次 "+fdate(global.last_reset_at)+(global.age_days!=null?" · 已 "+global.age_days.toFixed(1)+" 天":"")+
  (global.median_days!=null&&global.avg_days!=null?
    "\n近期間隔 均 "+global.avg_days.toFixed(1)+" 天 / 中 "+global.median_days.toFixed(1)+" 天 · 推估 "+fdate(global.median_estimate_at):"")+
  (global.p24!=null||global.p48!=null?" · "+(global.p24!=null?"24h "+global.p24+"%":"")+(global.p48!=null?" / 48h "+global.p48+"%":""):"")
  :"尚無資料 · codex-reset.com";
 ($("mode") as HTMLSelectElement).value=mode;
 $("alarm").textContent=s.next?fdate(new Date(s.next.at*1000).toISOString())+" · "+s.next.reason:"尚未排程";
 $("schedule-preview").textContent=mode==="off"?"排程已關閉":mode==="auto"?"下一次以目前 quota reset 為基準":"下一錨點："+(s.next?fdate(new Date(s.next.at*1000).toISOString()):"尚未排程");
 $("weekly-summary").textContent=s.settings.rules.filter(r=>r.enabled).map(r=>ruleLabel(r,true)).join("\n")||"沒有錨點";
 for(const [key,date,output] of [["today",today(0),"today-summary"],["tomorrow",today(1),"tomorrow-summary"]] as const){
  const over=s.settings.overrides[date];
  $(output).textContent=over?.enabled?(over.rules.length?over.rules.filter(r=>r.enabled).map(r=>ruleLabel(r,false)).join("\n"):"覆寫：當天不觸發"):"使用每週預設";
 }
 $("settings-account").textContent=s.signed_in?(authInvalid?"登入已失效，請重新登入":"已登入"):"尚未登入";
 ($("sample") as HTMLSelectElement).value=String(s.settings.sampling_minutes);
 ($("theme") as HTMLSelectElement).value=s.settings.theme;
 ($("native-toggle") as HTMLInputElement).checked=s.settings.native_notify_enabled;
 renderNotificationStatus();
 const topic=$("ntfy") as HTMLInputElement;
 if(document.activeElement!==topic){topic.value=s.settings.ntfy_topic;}
 $("ntfy-status").textContent=s.settings.ntfy_topic?(s.ntfy_status||"尚未測試"):"未設定";
 ($("ntfy-test") as HTMLButtonElement).disabled=!s.settings.ntfy_topic;
 ($("ntfy-copy") as HTMLButtonElement).disabled=!topic.value;
 if(theme!==s.settings.theme){applyTheme(s.settings.theme);}
 if(s.error){
  if(s.error!==lastDisplayedError){lastDisplayedError=s.error;toast(s.error);}
 }else{lastDisplayedError="";}
 renderEvents();
 draw();
}
function renderEvents(){
 const box=$("event-list");box.replaceChildren();
 const feed=buildHistoryFeed(samples,events,Date.now());
 const append=(item:{at:string;kind:"event"|"sample";detail:string})=>{
  const line=document.createElement("div");
  line.className="event-item";
  line.textContent=fdate(item.at)+" · "+item.detail;
  box.append(line);
 };
 const summary=document.createElement("div");
 summary.className="event-item";
 summary.textContent=feed.latest?
  "最近採樣 "+fdate(feed.latest.at)+" · 5h "+percent(feed.latest.five_hour)+" · 週 "+percent(feed.latest.weekly):
  "尚無配額採樣";
 box.append(summary);
 for(const item of feed.recent)append(item);
 if(!feed.recent.length){
  const note=document.createElement("div");note.className="event-item";
  note.textContent="近 48 小時沒有重大事件或明顯配額變化";
  box.append(note);
 }
 const toggle=(label:string,active:boolean,callback:()=>void)=>{
  const button=document.createElement("button");
  button.type="button";button.className="history-toggle";
  button.textContent=(active?"收起":"顯示")+label;
  button.onclick=()=>{
   const scroll=box.scrollTop;callback();renderEvents();box.scrollTop=scroll;
  };
  box.append(button);
 };
 if(feed.archive.length){
  toggle("較早事件／變化（"+feed.archive.length+"）",expandedHistory,()=>expandedHistory=!expandedHistory);
  if(expandedHistory)for(const item of feed.archive.slice(0,100))append(item);
 }
 if(feed.details.length){
  toggle("採樣明細（"+feed.details.length+"）",expandedSamples,()=>expandedSamples=!expandedSamples);
  if(expandedSamples)for(const item of feed.details.slice(0,100))append(item);
 }
}
async function load(){
 try{
  snapshot=await call<Snapshot>("get_snapshot");
  render();
 }catch(e){toast(String(e));}
}
async function loadHistory(){
 try{
  const [history,logs]=await Promise.all([call<Sample[]>("get_history"),call<Event[]>("get_events")]);
  samples=chronological(history);events=logs;renderEvents();draw();
 }catch(e){toast(String(e));}
}
// Read one consistent snapshot (including quota samples and events) over one IPC call.
async function loadLocalState(){
 const cached=await call<LocalState>("get_local_state");
 snapshot=cached.snapshot;
 samples=chronological(cached.history);
 events=cached.events;
 render();
}
async function refresh(){
 if(refreshing)return;
 refreshing=true;
 closeToast();
 try{await call("refresh");await loadLocalState();}
 catch(e){toast("更新失敗："+String(e));await load();}
 finally{refreshing=false;}
}
async function refreshIfStale(){
 if(document.hidden||loginInProgress||refreshing)return;
 const checked=snapshot?.usage?.checked_at?Date.parse(snapshot.usage.checked_at):0;
 const now=Date.now();
 if(Number.isFinite(checked)&&checked>0&&now-checked<90_000)return;
 if(now-lastForegroundRefresh<30_000)return;
 lastForegroundRefresh=now;
 await refresh();
}
document.addEventListener("visibilitychange",()=>{if(!document.hidden)void refreshIfStale();});
window.addEventListener("focus",()=>{void refreshIfStale();});
$("top-refresh").onclick=refresh;$("check").onclick=refresh;
$("trigger").onclick=async()=>{
 if(!confirm("將發送最小 Codex 請求，可能消耗配額。確定立即觸發？"))return;
 try{await call("manual_trigger");toast("手動觸發成功");await refresh();}
 catch(e){toast("手動觸發失敗："+String(e));}
};
$("reset-action").onclick=()=>{
 const s=snapshot;if(!s)return;
 $("reset-dialog-title").textContent="Banked Reset ×"+(s.credits?.count??0)+(s.armed?" · 待命":"");
 const nearest=nearestCredit(s.credits?.available??[]);
 $("reset-dialog-desc").textContent="已用盡時可以立即重置，或選擇待命直到 5h／週配額任一用盡。也可以明確選擇重置部分已用配額。每次使用會消耗一個 Banked Reset。"+
  (nearest?"\n最近到期："+fdate(nearest.expires_at)+" · 剩 "+bankedCountdown(nearest.expires_at):"");
 $("reset-arm").textContent=s.armed?"取消待命":"用盡後自動使用一次";
 ($("reset-exhausted") as HTMLButtonElement).disabled=!(s.usage?.five_hour===100||s.usage?.weekly===100||s.usage?.limit_reached);
 ($("reset-now") as HTMLButtonElement).disabled=!(!!s.usage?.five_hour||!!s.usage?.weekly);
 ($("reset-dialog") as HTMLDialogElement).showModal();
};
const resetDialog=$("reset-dialog") as HTMLDialogElement;
resetDialog.addEventListener("click",event=>{
 if(event.target!==resetDialog)return;
 const rect=resetDialog.getBoundingClientRect();
 if(event.clientX<rect.left||event.clientX>rect.right||event.clientY<rect.top||event.clientY>rect.bottom)resetDialog.close();
});
$("reset-cancel").onclick=()=>resetDialog.close();
$("reset-arm").onclick=async()=>{try{await call("toggle_banked_armed");($("reset-dialog") as HTMLDialogElement).close();await load();}catch(e){toast(String(e));}};
for(const [id,partial] of [["reset-exhausted",false],["reset-now",true]] as const){
 $(id).onclick=async()=>{
  if(!confirm(partial?"確定現在使用一個 Banked Reset，重置部分已用額度？":"確定使用一個 Banked Reset？"))return;
  try{await call("banked_reset",{allowPartial:partial});($("reset-dialog") as HTMLDialogElement).close();toast("Banked Reset 已執行");await refresh();}
  catch(e){toast("Banked Reset 失敗："+String(e));}
 };
}
$("apply-mode").onclick=async()=>{
 if(!snapshot)return;
 const mode=($("mode") as HTMLSelectElement).value as Mode;
 if(mode!=="off"&&!confirm("啟用自動排程後，會依設定自動發送消耗配額的請求。確定啟用？"))return;
 try{await call("save_settings",{settings:{...snapshot.settings,mode}});toast("排程模式已更新");await load();}
 catch(e){toast(String(e));}
};
async function settingsPatch(patch:Partial<Settings>){
 if(!snapshot)return;
 try{snapshot=await call<Snapshot>("save_settings",{settings:{...snapshot.settings,...patch}});render();}
 catch(e){toast(String(e));}
}
$("theme").onchange=()=>{void settingsPatch({theme:($("theme") as HTMLSelectElement).value});};
$("native-toggle").onchange=async()=>{
 const enabled=($("native-toggle") as HTMLInputElement).checked;
 if(enabled&&notificationPermission===false){
  try{notificationPermission=(await requestPermission())==="granted";}
  catch(e){notificationPermissionError=String(e);}
 }
 await settingsPatch({native_notify_enabled:enabled});
 await refreshNotificationPermission();
};
$("native-permission").onclick=async()=>{
 try{notificationPermission=(await requestPermission())==="granted";notificationPermissionError="";}
 catch(e){notificationPermissionError=String(e);}
 renderNotificationStatus();
};

$("sample").onchange=()=>{void settingsPatch({sampling_minutes:Number(($("sample") as HTMLSelectElement).value)});};
$("login").onclick=async()=>{
 const b=$("login") as HTMLButtonElement;b.disabled=true;loginInProgress=true;toast("請在瀏覽器完成登入（5 分鐘內）");
 try{await call("login_with_chatgpt");await refresh();toast("登入成功");}
 catch(e){toast("登入失敗："+String(e));}
 finally{b.disabled=false;loginInProgress=false;}
};
$("logout").onclick=async()=>{if(!confirm("登出 ChatGPT 並關閉自動排程？"))return;
 try{await call("sign_out");await load();toast("已登出；排程已關閉");}
 catch(e){toast(String(e));}
};
$("ntfy-new").onclick=async()=>{
 const el=$("ntfy") as HTMLInputElement;
 if(el.value&& !confirm("更換 ntfy topic 後，舊網址訂閱端不再收到通知。確定建立新 Topic？"))return;
 const bytes=new Uint8Array(16);crypto.getRandomValues(bytes);
 el.value="https://ntfy.sh/codex-"+Array.from(bytes,b=>b.toString(16).padStart(2,"0")).join("");
 await settingsPatch({ntfy_topic:el.value});toast("已建立並儲存專屬 ntfy topic");
};
$("ntfy-save").onclick=async()=>{
 const value=($("ntfy") as HTMLInputElement).value.trim();
 await settingsPatch({ntfy_topic:value});toast(value?"ntfy 網址已儲存":"ntfy 已關閉");
};
$("ntfy-test").onclick=async()=>{
 try{toast(await call<string>("ntfy_test"));await load();}
 catch(e){toast("ntfy 發送失敗："+String(e));}
};
$("ntfy-copy").onclick=async()=>{
 const value=($("ntfy") as HTMLInputElement).value.trim();if(!value)return;
 try{await navigator.clipboard.writeText(value);toast("已複製 ntfy URL");}
 catch(e){toast("複製失敗："+String(e));}
};
function cloned(r:Rule):Rule{return {...r,days:[...r.days]};}
function editDate(key:"weekly"|"today"|"tomorrow"){
 if(!snapshot)return;
 editing=key;
 const date=key==="today"?today(0):key==="tomorrow"?today(1):null;
 const ov=date?snapshot.settings.overrides[date]:null;
 draft=(date?(ov?.rules??[]):snapshot.settings.rules).map(cloned);
 overrideOn=date?(ov?.enabled??false):false;
 $("rule-title").textContent=key==="weekly"?"每週預設":(key==="today"?"今天":"明天")+" · "+date;
 $("override-wrap").hidden=!date;$("override-hint").hidden=!date;
 ($("override-checkbox") as HTMLInputElement).checked=overrideOn;
 $("rule-recommended").hidden=!!date;
 drawRuleEditor();
 ($("rule-dialog") as HTMLDialogElement).showModal();
}
$("weekly-edit").onclick=()=>editDate("weekly");
$("today-edit").onclick=()=>editDate("today");
$("tomorrow-edit").onclick=()=>editDate("tomorrow");
$("rule-cancel").onclick=()=>($("rule-dialog") as HTMLDialogElement).close();
$("override-checkbox").onchange=()=>{overrideOn=($("override-checkbox") as HTMLInputElement).checked;drawRuleEditor();};
function maxSafe(rule:Rule,rules:Rule[],weekly:boolean){
 if(!rule.enabled)return 0;
 const period=weekly?10080:1440;
 const starts:number[]=[];
 for(const r of rules.filter(r=>r.enabled)){
  for(let day=0;day<(weekly?7:1);day++){
   if(weekly&&!r.days[day])continue;
   starts.push(day*1440+r.hour*60+r.minute);
  }
 }
 let min=Infinity;
 for(let day=0;day<(weekly?7:1);day++){
  if(weekly&&!rule.days[day])continue;
  const time=day*1440+rule.hour*60+rule.minute;
  for(const start of starts){
   let gap=(start-time+period)%period;if(gap===0)gap=period;
   min=Math.min(min,gap);
  }
 }
 return Number.isFinite(min)?Math.max(0,Math.min(6,Math.floor(min/300)-1)):3;
}
function drawRuleEditor(){
 const list=$("rule-list");list.replaceChildren();
 const weekly=editing==="weekly";
 list.classList.toggle("disabled-rules",!weekly&&!overrideOn);
 for(const rule of draft){
  const card=document.createElement("div");card.className="rule-card";
  const top=document.createElement("div");top.className="rule-top";
  const enabled=document.createElement("label");enabled.className="check";
  const checkbox=document.createElement("input");checkbox.type="checkbox";checkbox.checked=rule.enabled;
  checkbox.onchange=()=>{rule.enabled=checkbox.checked;drawRuleEditor();};
  enabled.append(checkbox,document.createTextNode("啟用"));top.append(enabled);
  const time=document.createElement("input");time.type="time";time.value=hhmm(rule);time.className="time-input";
  time.onchange=()=>{const [h,m]=time.value.split(":").map(Number);rule.hour=h;rule.minute=m;drawRuleEditor();};top.append(time);
  const del=document.createElement("button");del.textContent="×";del.className="delete-rule";
  del.onclick=()=>{draft=draft.filter(x=>x!==rule);drawRuleEditor();};top.append(del);card.append(top);
  const chain=document.createElement("div");chain.className="rule-chain";
  const label=document.createElement("label");label.textContent="後續自動";
  const count=document.createElement("select");
  const maximum=maxSafe(rule,draft,weekly);
  for(let i=0;i<=6;i++){
   const option=document.createElement("option");option.value=String(i);option.textContent=i+" 次";
   option.disabled=i>maximum;count.append(option);
  }
  count.value=String(Math.min(rule.auto_count,maximum));rule.auto_count=Number(count.value);
  count.onchange=()=>{rule.auto_count=Number(count.value);drawRuleEditor();};
  chain.append(label,count);card.append(chain);
  if(weekly){
   const days=document.createElement("div");days.className="rule-days";
   ["一","二","三","四","五","六","日"].forEach((text,i)=>{
    const day=document.createElement("label");day.className="day";
    const cb=document.createElement("input");cb.type="checkbox";cb.checked=rule.days[i];
    cb.onchange=()=>{rule.days[i]=cb.checked;drawRuleEditor();};
    day.append(cb,document.createTextNode(text));days.append(day);
   });card.append(days);
  }
  const preview=document.createElement("p");preview.className="note";
  preview.textContent="預估："+ruleLabel(rule,false)+(rule.auto_count>=maximum&&maximum>0?" · 已達此錨點安全上限":"");
  card.append(preview);list.append(card);
 }
 if(!draft.length){const p=document.createElement("p");p.className="note";p.textContent="沒有錨點。";list.append(p);}
 ($("rule-add") as HTMLButtonElement).disabled=!weekly&&!overrideOn;
}
$("rule-add").onclick=()=>{
 const base=draft.at(-1),baseMinute=base?base.hour*60+base.minute+300:360;
 let result:Rule|undefined;
 for(let i=0;i<288;i++){
  const t=(baseMinute+5*i)%1440;
  const candidate:Rule={id:crypto.randomUUID(),enabled:true,days:[true,true,true,true,true,true,true],
   hour:Math.floor(t/60),minute:t%60,auto_count:0};
  if(maxSafe(candidate,[...draft,candidate],editing==="weekly")>=0){
   const overlaps=draft.some(x=>x.enabled&&Math.min(Math.abs(x.hour*60+x.minute-t),1440-Math.abs(x.hour*60+x.minute-t))<300);
   if(!overlaps){result=candidate;break;}
  }
 }
 if(!result){toast("目前排程已沒有可容納完整 5 小時視窗的新錨點");return;}
 draft.push(result);drawRuleEditor();
};
$("rule-recommended").onclick=()=>{draft=[{id:crypto.randomUUID(),enabled:true,days:[true,true,true,true,true,true,true],hour:6,minute:0,auto_count:3}];drawRuleEditor();};
$("rule-save").onclick=async()=>{
 if(!snapshot)return;
 const weekly=editing==="weekly";
 const rules=draft.map(cloned);
 if(rules.some(r=>!r.enabled?false:r.auto_count>maxSafe(r,rules,weekly))){
  toast("存在小於 5 小時的錨點衝突，請修正");return;
 }
 const settings:Settings={...snapshot.settings};
 if(weekly){settings.rules=rules;}
 else{settings.overrides={...settings.overrides,[today(editing==="today"?0:1)]:{enabled:overrideOn,rules}};}
 try{await call("save_settings",{settings});($("rule-dialog") as HTMLDialogElement).close();await load();toast("排程設定已儲存");}
 catch(e){toast("無法儲存排程："+String(e));}
};
window.addEventListener("resize",resizeVisibleCharts);
const obs=new ResizeObserver(resizeVisibleCharts);
obs.observe($("app"));
void listen("credits:updated",async()=>{
 try{await loadLocalState();}
 catch{await Promise.allSettled([load(),loadHistory()]);}
});
type LocalState={snapshot:Snapshot;history:Sample[];events:Event[]};
async function initialize(){

 // Load persisted samples and events first, independent of OAuth, quota API,
 // or slow startup network operations. Never require "更新配額" to restore history.
 try{
  await loadLocalState();

 }catch(e){

  toast("讀取本機資料失敗："+String(e));
  await Promise.allSettled([loadHistory(),load()]);
 }
 requestAnimationFrame(()=>{resizeVisibleCharts();draw();});
 await refreshNotificationPermission();
}
void initialize();
