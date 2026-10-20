import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {isPermissionGranted,requestPermission} from "@tauri-apps/plugin-notification";
import * as echarts from "echarts";
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
'<header class="app-header"><h1>Codex Tools</h1><button class="refresh-icon" id="top-refresh" title="更新配額">↻</button></header>',
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
'<section class="card compact"><h2>實際下一個 Alarm</h2><p id="alarm">尚未排程</p><small>Windows：App 執行中才能排程；關閉視窗仍常駐系統匣，結束 App 便停止，不使用 Android AlarmManager／保底 Job。</small></section>',
'<section class="card compact"><h2>預估</h2><p id="schedule-preview">排程已關閉</p></section>',
'<p class="note">錨點到時若距 5h reset ≤ 60 秒，會等到 reset 後 15 秒再觸發；自動接續依後端實際 reset，不累積秒差。Windows 睡眠或結束 App 期間的逾期事件不會補執行。</p>',
'<section class="card editor-card"><div class="card-title"><strong>每週預設</strong><button class="small" id="weekly-edit">編輯</button></div><p id="weekly-summary">—</p></section>',
'<section class="card editor-card"><div class="card-title"><strong>今天</strong><button class="small" id="today-edit">編輯</button></div><p id="today-summary">使用每週預設</p></section>',
'<section class="card editor-card"><div class="card-title"><strong>明天</strong><button class="small" id="tomorrow-edit">編輯</button></div><p id="tomorrow-summary">使用每週預設</p></section>',
'</main>',
'<main class="page history-page" id="page-history">',
'<div class="history-charts"><div class="history-chart" id="yesterday-chart"></div><div class="history-chart" id="today-chart"></div></div>',
'<p class="chart-footnote">Global Reset：codex-reset.com · 實線＝實際剩餘 · 虛線＝理想/排程</p>',
'<section class="card history-events"><h2>事件與採樣</h2><div id="event-list">尚無配額採樣</div></section>',
'</main>',
'<main class="page" id="page-settings">',
'<section class="card compact"><h2>ChatGPT</h2><p id="settings-account">尚未登入</p></section>',
'<div class="button-row"><button class="action" id="login">登入</button><button class="action" id="logout">登出</button></div>',
'<div class="setting-row"><label for="theme">主題</label><select id="theme"><option value="system">跟隨系統</option><option value="light">淺色</option><option value="dark">深色</option></select></div>',
'<div class="setting-row"><label for="sample">採樣間隔</label><select id="sample"><option value="10">10 分鐘</option><option value="15">15 分鐘</option><option value="30">30 分鐘</option><option value="60">60 分鐘</option></select></div>',
'<h3>Windows 系統通知</h3><div class="button-row"><label class="notification-switch"><input type="checkbox" id="native-toggle">開啟系統通知</label><button id="native-permission" class="small" hidden>允許通知</button></div><p class="note" id="native-status">正在檢查通知權限…</p>',
'<h3>ntfy（選用，預設關閉）</h3><input id="ntfy" type="url" spellcheck="false" placeholder="未設定，按「建立」產生隨機專屬網址">',
'<div class="button-row ntfy-actions"><button id="ntfy-new">建立</button><button id="ntfy-test">測試</button><button id="ntfy-copy">複製</button><button id="ntfy-save">儲存</button></div>',
'<p class="note">最近發送：<span id="ntfy-status">未設定</span></p>',
'<section class="card compact"><h2>背景執行</h2><p>關閉視窗：持續監測。系統匣選擇「結束」：停止採樣、記錄及自動觸發。Windows 睡眠／關機時不會執行，重開 App 不補觸發。</p></section>',
'<p class="note">CodexCredits · MIT · 使用的 Codex 後端介面可能變動。</p>',
'</main></div>',
'<output role="status" aria-live="polite" id="toast" class="toast" hidden></output>',
'<dialog id="reset-dialog" class="modal"><h2 id="reset-dialog-title">Banked Reset</h2><p id="reset-dialog-desc"></p><div class="dialog-buttons">',
'<button id="reset-arm">用盡後自動使用一次</button><button id="reset-exhausted">已用盡，立即重置</button><button id="reset-now">立即使用（含部分用量）</button><button id="reset-cancel">取消</button></div></dialog>',
'<dialog id="rule-dialog" class="modal"><h2 id="rule-title">編輯排程</h2><p class="note">每個錨點可設定「後續自動」次數；每次依實際 5h reset 決定接續時間。</p><label class="override-row" id="override-wrap" hidden><input type="checkbox" id="override-checkbox">覆寫每週預設</label>',
'<p class="note" id="override-hint" hidden>關閉覆寫＝照每週預設；開啟但不加任何時段＝當天完全不觸發。</p><div id="rule-list"></div>',
'<button class="action" id="rule-add">＋ 新增錨點</button><button class="action" id="rule-recommended">套用建議：06:00 ＋ 接續 3 次</button>',
'<div class="dialog-footer"><button id="rule-cancel">取消</button><button class="primary" id="rule-save">儲存</button></div></dialog>'
].join("");
const androidRuntime=/Android/i.test(navigator.userAgent);
if(androidRuntime){
  const alarmText=document.querySelector("#page-schedule .card.compact small");
  if(alarmText)alarmText.textContent="Android：沿用原版 AlarmManager 與 JobScheduler 背景排程，支援重新開機後恢復。";
  const background=document.querySelector("#page-settings .card.compact:last-of-type p");
  if(background)background.textContent="Android：系統背景工作負責採樣與自動觸發；實際執行受省電政策、精確鬧鐘與通知權限影響。";
  const heading=document.querySelector("#page-settings h3");
  if(heading)heading.textContent="Android 系統通知";
  const policy=document.createElement("section");
  policy.className="card compact";
  policy.innerHTML=[
    '<h2>Android 背景排程權限</h2>',
    '<p id="android-platform-state">正在檢查精準鬧鐘與省電設定…</p>',
    '<div class="button-row"><button id="android-exact">精準鬧鐘設定</button><button id="android-battery">電池最佳化</button></div>',
    '<div class="button-row"><button id="android-app-settings">App 電池設定</button><button id="android-policy-refresh">重新檢查</button></div>'
  ].join("");
  document.querySelector("#page-settings")?.append(policy);
  const policyRefresh=async()=>{
    try{
      const value=await invoke<{exact_alarm:boolean;battery_exempt:boolean}>("android_platform_status");
      const p=document.getElementById("android-platform-state");
      if(p)p.textContent="精準鬧鐘："+(value.exact_alarm?"已允許":"尚未允許")
        +" · 省電最佳化豁免："+(value.battery_exempt?"是":"否");
    }catch(e){
      const p=document.getElementById("android-platform-state");
      if(p)p.textContent="無法讀取權限："+String(e);
    }
  };
  for(const [id,kind] of [["android-exact","exact"],["android-battery","battery"],["android-app-settings","app"]]){
    document.getElementById(id)?.addEventListener("click",async()=>{
      try{await invoke("android_open_system_settings",{kind});}
      catch(e){const p=document.getElementById("android-platform-state");if(p)p.textContent=String(e);}
    });
  }
  document.getElementById("android-policy-refresh")?.addEventListener("click",()=>void policyRefresh());
  document.addEventListener("visibilitychange",()=>{if(!document.hidden)void policyRefresh();});
  void policyRefresh();
}

let snapshot:Snapshot|null=null;
let samples:Sample[]=[];
let events:Event[]=[];
let page="status";
let theme="system";
let notificationPermission:boolean|null=null;
let notificationPermissionError="";
let nextMessages:Array<string>=[];
let editing:"weekly"|"today"|"tomorrow"="weekly";
let draft:Rule[]=[];
let overrideOn=false;
const charts:Record<string,echarts.ECharts>={};
const chartIds=["primary-chart","weekly-chart","yesterday-chart","today-chart"] as const;
const getChart=(id:string)=>charts[id]??(charts[id]=echarts.init($(id),undefined,{renderer:"canvas"}));
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
const toast=(message:string)=>{
 const node=$("toast");node.textContent=message;node.hidden=false;
 nextMessages.push(message);
 if(nextMessages.length>3)nextMessages.shift();
};
const closeToast=()=>{$("toast").hidden=true};
const call=async<T>(name:string,args?:Record<string,unknown>):Promise<T>=>invoke<T>(name,args);

function showPage(name:string){
 page=name;
 if(name==="settings")void refreshNotificationPermission();
 for(const btn of Array.from(document.querySelectorAll<HTMLButtonElement>(".tab"))){
  const selected=btn.dataset.tab===name;
  btn.classList.toggle("active",selected);btn.setAttribute("aria-selected",String(selected));
 }
 for(const item of Array.from(document.querySelectorAll<HTMLElement>(".page"))){
  item.classList.toggle("visible",item.id==="page-"+name);
 }
 window.requestAnimationFrame(()=>{Object.values(charts).forEach(c=>c.resize());draw();});
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
 const optedIn=snapshot.settings.native_notify_enabled;
 const permission=notificationPermission===true?"系統權限：允許":
                  notificationPermission===false?"系統權限：未允許":"系統權限：尚未確認";
 const portableNote="Windows 原生通知須使用安裝版（Setup EXE）；免安裝版不保證能顯示通知。";
 $("native-status").textContent=(optedIn?"程式通知已開啟。":"程式通知已關閉。")+
   " "+permission+"。 "+portableNote+
   (notificationPermissionError?" 查詢錯誤："+notificationPermissionError:"");
 ($("native-permission") as HTMLButtonElement).hidden=notificationPermission!==false||!optedIn;
}
async function refreshNotificationPermission(){
 try{notificationPermission=await isPermissionGranted();notificationPermissionError="";}
 catch(e){notificationPermission=null;notificationPermissionError=String(e);}
 renderNotificationStatus();
}
function chartPalette(){return theme==="dark"||(theme==="system"&&matchMedia("(prefers-color-scheme: dark)").matches)
 ?{text:"#a6acb7",grid:"#3d424b",bg:"#1f2227"}:{text:"#737983",grid:"#e2e5ea",bg:"#ffffff"};}
function paintQuota(id:string,used:number|null|undefined,reset:string|null|undefined,weekly:boolean,confirmed:boolean){
 const chart=getChart(id),p=chartPalette();
 const until=reset?new Date(reset).getTime():0,span=(weekly?7*24:5)*3600000;
 const start=until-span;
 const source=samples.filter(x=>{
  const t=new Date(x.at).getTime();
  const r=weekly?x.weekly_reset:x.five_reset;
  return t>=start-60000&&t<=until+60000&&r&&Math.abs(new Date(r).getTime()-until)<=300000;
 });
 const valid=until>0&&(weekly||confirmed);
 const color=weekly?"#e89134":"#2f80ed";
 const actual=source.map(s=>[new Date(s.at).getTime(),safeRemaining(weekly?s.weekly:s.five_hour)]);
 const ideal=valid?[[start,100],[until,0]]:[];
 const mark=Date.now()>=start&&Date.now()<=until?{xAxis:Date.now()}:undefined;
 chart.setOption({
  backgroundColor:"transparent",
  animation:false,
  grid:{left:5,right:5,top:6,bottom:20,containLabel:false},
  xAxis:{type:"time",min:valid?start:undefined,max:valid?until:undefined,axisLine:{show:false},
    splitLine:{show:true,lineStyle:{color:p.grid}},axisLabel:{fontSize:9,color:p.text,hideOverlap:true,formatter:(v:number)=>weekly?new Date(v).getDate().toString():String(new Date(v).getHours()).padStart(2,"0")},axisTick:{show:false}},
  yAxis:{type:"value",min:0,max:100,interval:50,axisLabel:{show:false},axisLine:{show:false},axisTick:{show:false},splitLine:{lineStyle:{color:p.grid}}},
  tooltip:{trigger:"axis",valueFormatter:(value:unknown)=>typeof value==="number"?Math.round(value)+"% 剩餘":String(value)},
  series:[
   {name:"理想剩餘",type:"line",data:ideal,showSymbol:false,silent:true,lineStyle:{color,type:"dashed",width:1.5,opacity:.8},areaStyle:{color,opacity:.08}},
   {name:"實際剩餘",type:"line",data:valid?actual:[],showSymbol:false,lineStyle:{width:2.4,color},itemStyle:{color},areaStyle:{color,opacity:.18},
      markLine:mark?{symbol:["none","none"],silent:true,label:{show:false},lineStyle:{color:p.text,width:1},data:[mark]}:undefined}
  ],
  graphic:!valid?[{type:"text",left:10,top:"45%",style:{text:weekly?"未啟動":"5h 尚未啟動 · 等待觸發",fill:p.text,fontSize:11}}]:[]
 },true);
}
function paintHistory(id:string,offset:number){
 const chart=getChart(id),p=chartPalette();
 const day=new Date();day.setHours(0,0,0,0);day.setDate(day.getDate()+offset);
 const start=day.getTime(),end=start+86400000;
 const todaySamples=samples.filter(s=>{const t=new Date(s.at).getTime();return t>=start&&t<end;});
 const actual=(key:"five_hour"|"weekly")=>todaySamples.map((s,i)=>{
  const current=key==="five_hour"?s.five_reset:s.weekly_reset;
  const before=i>0?(key==="five_hour"?todaySamples[i-1].five_reset:todaySamples[i-1].weekly_reset):current;
  return [new Date(s.at).getTime(),current&&before&&Math.abs(new Date(current).getTime()-new Date(before).getTime())>300000?null:safeRemaining(s[key])];
 });
 const ideal=(reset:string|null|undefined,hours:number)=>{
  if(!reset)return [];
  const e=new Date(reset).getTime(),s=e-hours*3600000;
  const a=Math.max(s,start),b=Math.min(e,end);
  return b>a?[[a,100-(a-s)/(e-s)*100],[b,100-(b-s)/(e-s)*100]]:[];
 };
 const u=snapshot?.usage;
 const globalTime=snapshot?.global?.last_reset_at?new Date(snapshot.global.last_reset_at).getTime():null;
 const globalMark=globalTime!=null&&globalTime>=start&&globalTime<end?
  {symbol:["none","none"],silent:true,label:{show:true,formatter:"Global Reset",fontSize:9},
   lineStyle:{type:"dashed",color:"#999",width:1},data:[{xAxis:globalTime}]}:undefined;
 const palette=[["5 小時","#2f80ed"],["週配額","#e89134"]] as const;
 chart.setOption({
  animation:false,backgroundColor:"transparent",
  title:{text:offset===0?"今天":"昨天",left:8,top:5,textStyle:{fontSize:12,color:p.text}},
  legend:{data:palette.map(x=>x[0]),top:2,right:8,textStyle:{fontSize:10,color:p.text},itemWidth:12,itemHeight:3},
  grid:{left:5,right:5,top:24,bottom:19},
  xAxis:{type:"time",min:start,max:end,interval:6*3600000,axisLabel:{fontSize:9,color:p.text,formatter:(v:number)=>String(new Date(v).getHours()).padStart(2,"0")},axisLine:{show:false},splitLine:{show:true,lineStyle:{color:p.grid}},axisTick:{show:false}},
  yAxis:{type:"value",min:0,max:100,interval:50,axisLabel:{show:false},axisLine:{show:false},splitLine:{lineStyle:{color:p.grid}}},
  tooltip:{trigger:"axis",valueFormatter:(value:unknown)=>typeof value==="number"?Math.round(value)+"% 剩餘":String(value)},
  series:[
    {name:"5 小時理想",type:"line",data:u?.primary_confirmed?ideal(u.five_reset,5):[],showSymbol:false,silent:true,lineStyle:{color:palette[0][1],type:"dashed",width:1}},
    {name:"每週理想",type:"line",data:ideal(u?.weekly_reset,168),showSymbol:false,silent:true,lineStyle:{color:palette[1][1],type:"dashed",width:1}},
    {name:"5 小時",type:"line",data:actual("five_hour"),showSymbol:false,lineStyle:{color:palette[0][1],width:2},itemStyle:{color:palette[0][1]},markLine:globalMark},
    {name:"週配額",type:"line",data:actual("weekly"),showSymbol:false,lineStyle:{color:palette[1][1],width:2},itemStyle:{color:palette[1][1]}}
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
 $("account").textContent=s.signed_in?"ChatGPT 已登入 · "+(u?.plan??"—")+(u?.checked_at?" · 更新 "+fdate(u.checked_at):""):"ChatGPT 尚未登入";
 $("primary-percent").textContent=percent(u?.five_hour);
 $("weekly-percent").textContent=percent(u?.weekly);
 $("status-next").textContent=mode==="off"?"已關閉 · 尚未排程":
   (mode==="auto"?"Reset 後自動 · ":"錨點＋接續 · ")+(s.next?fdate(new Date(s.next.at*1000).toISOString())+" · "+s.next.reason:"尚未排程");
 const confirmed=!!u?.primary_confirmed&&!!u.five_reset&&new Date(u.five_reset).getTime()>Date.now();
 $("trigger-preview").textContent=confirmed?"5h 進行中 · 可再開始：約 "+fdate(u?.five_reset):"現在觸發 · 預估重置 "+fdate(new Date(Date.now()+5*3600000).toISOString());
 ($("trigger") as HTMLButtonElement).disabled=!s.signed_in||confirmed||!u?.allowed||u?.weekly===100;
 const credits=s.credits;const available=credits?.count??0;
 $("reset-title").textContent="Banked Reset ×"+available+(s.armed?" · 待命":"");
 $("reset-action").textContent="Banked Reset ×"+available+(s.armed?" · 待命":"");
 ($("reset-action") as HTMLButtonElement).hidden=available<=0&&!s.armed;
 const first=credits?.available[0];
 const expiries=(credits?.available??[]).map(x=>x.expires_at).filter(Boolean).map(x=>fdate(x)).join(" · ");
 $("reset-expiry").textContent=(s.armed?"用盡後自動重置":"")+(expiries?(s.armed?" · ":"")+"到期 "+expiries:"");
 const global=s.global;
 $("global-reset").textContent=global?.last_reset_at?
  "上次 "+fdate(global.last_reset_at)+(global.age_days!=null?" · 已 "+global.age_days.toFixed(1)+" 天":"")+
  (global.median_days!=null&&global.avg_days!=null?
    "\n近期間隔 均 "+global.avg_days.toFixed(1)+" 天 / 中 "+global.median_days.toFixed(1)+" 天 · 推估 "+fdate(global.median_estimate_at):"")+
  (global.p24!=null||global.p48!=null?" · "+(global.p24!=null?"24h "+global.p24+"%":"")+(global.p48!=null?" / 48h "+global.p48+"%":""):"")
  :"尚無資料 · codex-reset.com";
 ($("mode") as HTMLSelectElement).value=mode;
 $("alarm").textContent=s.next?fdate(new Date(s.next.at*1000).toISOString())+" · "+s.next.reason+"\nAlarm：桌面程式內執行 · 保底 Job：不適用 Windows":"尚未排程\nAlarm：未設定 · 保底 Job：不適用 Windows";
 $("schedule-preview").textContent=mode==="off"?"排程已關閉":mode==="auto"?"下一次以目前 quota reset 為基準":"下一錨點："+(s.next?fdate(new Date(s.next.at*1000).toISOString()):"尚未排程");
 $("weekly-summary").textContent=s.settings.rules.filter(r=>r.enabled).map(r=>ruleLabel(r,true)).join("\n")||"沒有錨點";
 for(const [key,date,output] of [["today",today(0),"today-summary"],["tomorrow",today(1),"tomorrow-summary"]] as const){
  const over=s.settings.overrides[date];
  $(output).textContent=over?.enabled?(over.rules.length?over.rules.filter(r=>r.enabled).map(r=>ruleLabel(r,false)).join("\n"):"覆寫：當天不觸發"):"使用每週預設";
 }
 $("settings-account").textContent=s.signed_in?"已登入":"尚未登入";
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
 if(s.error){toast(s.error);}
 renderEvents();
 draw();
}
function renderEvents(){
 const box=$("event-list");box.replaceChildren();
 const latest=samples.at(-1);
 const summary=document.createElement("div");summary.className="event-item";
 summary.textContent=latest?"最近採樣 "+fdate(latest.at)+" · 5h "+percent(latest.five_hour)+" · 週 "+percent(latest.weekly):"尚無配額採樣";
 box.append(summary);
 for(const e of events.slice(0,6)){
  const item=document.createElement("div");item.className="event-item";
  item.textContent=fdate(e.at)+" · "+e.detail;box.append(item);
 }
 for(const sample of samples.slice(-8).reverse()){
  const item=document.createElement("div");item.className="event-item";
  item.textContent=fdate(sample.at)+" · 5h "+percent(sample.five_hour)+" · 週 "+percent(sample.weekly);box.append(item);
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
  samples=history;events=logs;renderEvents();draw();
 }catch(e){toast(String(e));}
}
async function refresh(){
 closeToast();
 try{await call("refresh");await Promise.all([load(),loadHistory()]);}
 catch(e){toast("更新失敗："+String(e));}
}
$("top-refresh").onclick=refresh;$("check").onclick=refresh;
$("trigger").onclick=async()=>{
 if(!confirm("要依 Android 原版發送最小 Codex 請求，建立 5 小時視窗嗎？這會消耗配額。"))return;
 try{await call("manual_trigger");toast("手動觸發成功");await refresh();}
 catch(e){toast("手動觸發失敗："+String(e));}
};
$("reset-action").onclick=()=>{
 const s=snapshot;if(!s)return;
 $("reset-dialog-title").textContent="Banked Reset ×"+(s.credits?.count??0)+(s.armed?" · 待命":"");
 $("reset-dialog-desc").textContent="已用盡時可以立即重置，或選擇待命直到 5h／週配額任一用盡。也可以明確選擇重置部分已用配額。每次使用會消耗一個 Banked Reset。";
 $("reset-arm").textContent=s.armed?"取消待命":"用盡後自動使用一次";
 ($("reset-exhausted") as HTMLButtonElement).disabled=!(s.usage?.five_hour===100||s.usage?.weekly===100||s.usage?.limit_reached);
 ($("reset-now") as HTMLButtonElement).disabled=!(!!s.usage?.five_hour||!!s.usage?.weekly);
 ($("reset-dialog") as HTMLDialogElement).showModal();
};
$("reset-cancel").onclick=()=>($("reset-dialog") as HTMLDialogElement).close();
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
 if(mode!=="off"&&!confirm("確定啟用自動排程？程式存活時會依後端實際配額重置，自動發送消耗額度的 Codex 請求。Windows 睡眠或結束 App 時不執行。"))return;
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
 const b=$("login") as HTMLButtonElement;b.disabled=true;toast("請在瀏覽器完成 ChatGPT 登入（5 分鐘內）");
 try{await call("login_with_chatgpt");await refresh();toast("登入成功");}
 catch(e){toast("登入失敗："+String(e));}
 finally{b.disabled=false;}
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
window.addEventListener("resize",()=>{Object.values(charts).forEach(c=>c.resize());draw();});
const obs=new ResizeObserver(()=>{Object.values(charts).forEach(c=>c.resize());});
obs.observe($("app"));
void listen("credits:updated",async()=>{await Promise.allSettled([load(),loadHistory()]);});
type LocalState={snapshot:Snapshot;history:Sample[];events:Event[]};
async function initialize(){
 // Load persisted samples and events first, independent of OAuth, quota API,
 // or slow startup network operations. Never require "更新配額" to restore history.
 try{
  const cached=await call<LocalState>("get_local_state");
  snapshot=cached.snapshot;samples=cached.history;events=cached.events;
  render();
 }catch(e){
  toast("讀取本機資料失敗："+String(e));
  await Promise.allSettled([loadHistory(),load()]);
 }
 requestAnimationFrame(()=>{Object.values(charts).forEach(c=>c.resize());draw();});
 await refreshNotificationPermission();
}
void initialize();
