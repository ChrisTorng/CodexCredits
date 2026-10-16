import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import * as echarts from "echarts";
import "./style.css";
type Settings={mode:string;sampling_minutes:number;retention_days:number;ntfy_topic:string;rules:unknown[]};
type Usage={five_hour:number|null;weekly:number|null;five_reset:string|null;weekly_reset:string|null};
type Snapshot={settings:Settings;usage:Usage|null;error:string|null;signed_in:boolean;next_anchor:string|null};
const app=document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML=`<header><h1>CodexCredits</h1><small id="status">載入中</small></header><div class="cards"><article><h2>5 小時用量</h2><strong id="five">—</strong><small id="reset5"></small></article><article><h2>每週用量</h2><strong id="weekly">—</strong><small id="resetW"></small></article></div><section><button id="refresh">重新整理</button> <span id="msg"></span></section><section><h2>歷史圖表</h2><div id="chart"></div></section><section><h2>設定</h2><label>採樣分鐘 <input type="number" min="1" max="1440" id="minutes"></label><label>保留天數 <input type="number" min="1" max="3650" id="days"></label><button id="save">儲存</button></section><section><h2>登入</h2><p>目前只支援手動貼入 Access Token，保存在作業系統安全憑證庫。完整 OAuth 尚未實作。</p><input id="token" type="password" autocomplete="off" placeholder="Access token"><button id="signin">儲存 Token</button><button id="signout">登出</button></section><section><h2>排程預覽</h2><p id="next"></p><p>自動觸發配額與 ntfy 通知尚未實作，安全起見固定關閉。</p></section><footer>關閉視窗只隱藏到系統匣；透過系統匣「結束」會停止所有背景監測。</footer>`;
const el=(id:string)=>document.getElementById(id)!;
const chart=echarts.init(el("chart"));
let snapshot:Snapshot|undefined;
const fmt=(n:number|null|undefined)=>n==null?"—":n.toFixed(1)+"%";
const dt=(s:string|null|undefined)=>s?new Date(s).toLocaleString("zh-TW"):"—";
const msg=(m:string)=>{el("msg").textContent=m};
async function load(){
 try{snapshot=await invoke<Snapshot>("get_snapshot");
 el("status").textContent=snapshot.signed_in?"已登入":"未登入";
 el("five").textContent=fmt(snapshot.usage?.five_hour);
 el("weekly").textContent=fmt(snapshot.usage?.weekly);
 el("reset5").textContent="重設："+dt(snapshot.usage?.five_reset);
 el("resetW").textContent="重設："+dt(snapshot.usage?.weekly_reset);
 (el("minutes") as HTMLInputElement).value=String(snapshot.settings.sampling_minutes);
 (el("days") as HTMLInputElement).value=String(snapshot.settings.retention_days);
 el("next").textContent="下次預設錨點："+dt(snapshot.next_anchor);
 if(snapshot.error)msg(snapshot.error);
 }catch(e){msg(String(e))}
}
async function history(){try{
 const rows=await invoke<Array<{at:string;five_hour:number|null;weekly:number|null}>>("get_history");
 chart.setOption({tooltip:{trigger:"axis"},legend:{data:["5 小時","每週"]},xAxis:{type:"category",data:rows.map(x=>new Date(x.at).toLocaleString("zh-TW"))},yAxis:{type:"value",min:0,max:100},series:[{type:"line",name:"5 小時",showSymbol:false,data:rows.map(x=>x.five_hour)},{type:"line",name:"每週",showSymbol:false,data:rows.map(x=>x.weekly)}]});
 }catch(e){msg(String(e))}}
el("refresh").onclick=async()=>{msg("更新中");try{await invoke("refresh");msg("更新完成");await load();await history()}catch(e){msg(String(e))}};
el("save").onclick=async()=>{if(!snapshot)return;try{await invoke("save_settings",{settings:{...snapshot.settings,sampling_minutes:Number((el("minutes") as HTMLInputElement).value),retention_days:Number((el("days") as HTMLInputElement).value),mode:"off"}});msg("儲存完成");await load()}catch(e){msg(String(e))}};
el("signin").onclick=async()=>{try{await invoke("set_access_token",{token:(el("token") as HTMLInputElement).value});(el("token") as HTMLInputElement).value="";msg("已儲存");await load()}catch(e){msg(String(e))}};
el("signout").onclick=async()=>{try{await invoke("sign_out");msg("已登出");await load()}catch(e){msg(String(e))}};
window.addEventListener("resize",()=>chart.resize());void listen("credits:updated",async()=>{await load();await history()});void load();void history();
