package com.christorng.androidcodextools;

import android.app.AlarmManager;
import android.app.PendingIntent;
import android.app.job.JobInfo;
import android.app.job.JobScheduler;
import android.content.ComponentName;
import android.os.PersistableBundle;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.net.Uri;
import android.os.Build;
import android.provider.Settings;

import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.time.Instant;
import java.time.LocalDate;
import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.time.format.DateTimeFormatter;
import java.util.Locale;

final class Scheduler {
    static final String PREFS="app";
    static final String KEY_NTFY="ntfy_url";
    // Hash only for one-time migration of the previous shared default topic.
    // Do not ship the old shared URL in the application binary.
    private static final String LEGACY_NTFY_SHA256=
            "2a757a24eedaa8a204429ebff77b63b19ac855ea1e7c20d5255ad63d58fb993e";
    private static final String KEY_NTFY_LAST="ntfy_last_status";
    static final String KEY_LAST="last_status";
    private static final String KEY_HISTORY="event_history_v1";
    static final String KEY_NEXT="next_alarm_ms";
    static final String KEY_NEXT_REASON="next_alarm_reason";
    static final String KEY_MODE="scheduler_mode";
    static final String KEY_SAMPLE_MINUTES="sample_interval_minutes";

    static final String MODE_OFF="off";
    static final String MODE_AUTO="auto_reset";
    static final String MODE_CUSTOM="custom";

    private static final String KEY_PLAN="quota_plan";
    private static final String KEY_ALLOWED="quota_allowed";
    private static final String KEY_LIMIT_REACHED="quota_limit_reached";
    private static final String KEY_PRIMARY_ACTIVE="quota_primary_active";
    private static final String KEY_PRIMARY_USED="quota_primary_used";
    private static final String KEY_PRIMARY_RESET="quota_primary_reset";
    private static final String KEY_SECONDARY_ACTIVE="quota_secondary_active";
    private static final String KEY_SECONDARY_USED="quota_secondary_used";
    private static final String KEY_SECONDARY_RESET="quota_secondary_reset";
    private static final String KEY_LAST_CHECK="quota_last_check";
    private static final String KEY_PRIMARY_CONFIRMED="quota_primary_confirmed";
    private static final String KEY_LAST_ALARM_RECEIVED="last_alarm_received";
    private static final String KEY_LAST_BACKUP_RECEIVED="last_backup_received";
    private static final String KEY_RESET_AVAILABLE="reset_available_count";
    private static final String KEY_RESET_CREDIT_ID="reset_credit_id";
    private static final String KEY_RESET_TITLE="reset_credit_title";
    private static final String KEY_RESET_DESCRIPTION="reset_credit_description";
    private static final String KEY_RESET_EXPIRES="reset_credit_expires";
    private static final String KEY_RESET_EXPIRIES="reset_credit_expiries";
    private static final String KEY_RESET_LAST_CHECK="reset_credit_last_check";
    private static final String KEY_RESET_ERROR="reset_credit_error";
    private static final String KEY_BANKED_ARMED="banked_reset_on_exhaustion";
    private static final String KEY_BANKED_ARMED_ID="banked_reset_armed_credit_id";
    private static final Object BANKED_RESET_LOCK=new Object();

    private static final String KEY_NEXT_KIND="next_event_kind";
    private static final String KEY_NEXT_AUTO_COUNT="next_event_auto_count";
    private static final String KEY_CHAIN_REMAINING="chain_remaining";
    private static final String KEY_CHAIN_RESET="chain_reset_ms";
    // Persist the recovery alarm across process death, reboots and quota refreshes.
    private static final String KEY_MISSED_POLICY="missed_anchor_policy";
    private static final String KEY_RECOVERY_AT="missed_anchor_recovery_at_ms";
    private static final String KEY_RECOVERY_ANCHOR="missed_anchor_recovery_anchor_ms";
    private static final String KEY_RECOVERY_COUNT="missed_anchor_recovery_count";
    private static final String KEY_VERIFY_RESET="missed_anchor_verify_reset_ms";
    private static final String POLICY_AT_RESET="at_reset";
    private static final String POLICY_NEXT_SLOT="next_slot";
    private static final String POLICY_SKIP="skip_today";
    private static final long FIVE_HOURS_MS=5*60*60_000L;

    private static final String EVENT_AUTO="auto";
    private static final String EVENT_ANCHOR="anchor";
    private static final String EVENT_CHAIN="chain";
    private static final String EVENT_ANCHOR_RETRY="anchor_retry";
    private static final String EVENT_RECOVERY="recovery";
    private static final String EVENT_RESUME="resume";
    private static final String KEY_RESUME_DONE_DAY="reset_first_done_day";
    private static final String KEY_RESUME_PENDING_DAY="reset_first_pending_day";
    private static final String KEY_RESUME_COUNT="reset_first_pending_count";
    private static final String KEY_RESUME_NEXT_AT="reset_first_next_at";
    private static final String KEY_RESUME_VERIFY="reset_first_verify_reset";

    private static final long RESET_TOLERANCE_MS=60_000L;
    private static final long RESET_GRACE_MS=15_000L;
    private static final int REQUEST_CODE=1001;
    private static final int BACKUP_JOB_ID=2101;
    private static final int SAMPLE_JOB_ID=2201;
    private static final long BACKUP_DELAY_MS=90_000L;
    private static final long BACKUP_DEADLINE_EXTRA_MS=8*60_000L;

    static final class Snapshot {
        final String plan;
        final boolean allowed,limitReached;
        final boolean primaryActive;
        final boolean primaryConfirmed;
        final double primaryUsed;
        final long primaryResetMs;
        final boolean secondaryActive;
        final double secondaryUsed;
        final long secondaryResetMs;
        final long lastCheckMs;
        final int resetAvailableCount;
        final String resetCreditId,resetTitle,resetDescription,resetExpires,resetExpiries,resetError;

        Snapshot(SharedPreferences p){
            plan=p.getString(KEY_PLAN,"—");
            allowed=p.getBoolean(KEY_ALLOWED,true);
            limitReached=p.getBoolean(KEY_LIMIT_REACHED,false);
            primaryActive=p.getBoolean(KEY_PRIMARY_ACTIVE,false);
            primaryConfirmed=p.getBoolean(KEY_PRIMARY_CONFIRMED,false);
            primaryUsed=Double.longBitsToDouble(p.getLong(KEY_PRIMARY_USED,Double.doubleToLongBits(0)));
            primaryResetMs=p.getLong(KEY_PRIMARY_RESET,0);
            secondaryActive=p.getBoolean(KEY_SECONDARY_ACTIVE,false);
            secondaryUsed=Double.longBitsToDouble(p.getLong(KEY_SECONDARY_USED,Double.doubleToLongBits(0)));
            secondaryResetMs=p.getLong(KEY_SECONDARY_RESET,0);
            lastCheckMs=p.getLong(KEY_LAST_CHECK,0);
            resetAvailableCount=p.getInt(KEY_RESET_AVAILABLE,0);
            resetCreditId=p.getString(KEY_RESET_CREDIT_ID,"");
            resetTitle=p.getString(KEY_RESET_TITLE,"");
            resetDescription=p.getString(KEY_RESET_DESCRIPTION,"");
            resetExpires=p.getString(KEY_RESET_EXPIRES,"");
            resetExpiries=p.getString(KEY_RESET_EXPIRIES,"");
            resetError=p.getString(KEY_RESET_ERROR,"");
        }
    }

    static SharedPreferences prefs(Context c){return c.getSharedPreferences(PREFS,Context.MODE_PRIVATE);}
    static Snapshot snapshot(Context c){return new Snapshot(prefs(c));}

    static String mode(Context c){
        SharedPreferences p=prefs(c);
        if(p.contains(KEY_MODE))return p.getString(KEY_MODE,MODE_OFF);
        return p.getBoolean("scheduler_enabled",false)?MODE_AUTO:MODE_OFF;
    }

    static void setMode(Context c,String mode){
        prefs(c).edit().putString(KEY_MODE,mode).remove("scheduler_enabled").apply();
        if(MODE_OFF.equals(mode)){clearRecovery(c);clearDailyResume(c);cancelAlarm(c);}
        else reschedule(c);
    }

    static boolean enabled(Context c){return !MODE_OFF.equals(mode(c));}
    static int samplingMinutes(Context c){
        return Math.max(10,prefs(c).getInt(KEY_SAMPLE_MINUTES,10));
    }
    static void setSamplingMinutes(Context c,int minutes){
        prefs(c).edit().putInt(KEY_SAMPLE_MINUTES,Math.max(10,minutes)).apply();
        scheduleQuotaSampling(c);
    }
    static String last(Context c){
        return displayEvent(prefs(c).getString(KEY_LAST,""));
    }
    static String history(Context c){
        SharedPreferences p=prefs(c);
        String raw=p.getString(KEY_HISTORY,"");
        if(raw==null||raw.isEmpty()){
            String last=p.getString(KEY_LAST,"");
            return last==null||last.isEmpty()?"尚無紀錄":displayEvent(last);
        }
        String[] items=raw.split("\\u001e",-1);
        StringBuilder out=new StringBuilder();
        for(int i=items.length-1;i>=0;i--){
            String item=items[i].trim();
            if(item.isEmpty())continue;
            if(out.length()>0)out.append("\n");
            out.append(displayEvent(item));
        }
        return out.length()==0?"尚無紀錄":out.toString();
    }
    static void note(Context c,String message){
        SharedPreferences p=prefs(c);
        String encoded="@"+System.currentTimeMillis()+"|"+message;
        String oldHistory=p.getString(KEY_HISTORY,"");
        StringBuilder h=new StringBuilder();
        if(oldHistory!=null&&!oldHistory.isEmpty()){
            String[] items=oldHistory.split("\\u001e",-1);
            int start=Math.max(0,items.length-19);
            for(int i=start;i<items.length;i++){
                String item=items[i].trim();
                if(item.isEmpty())continue;
                if(h.length()>0)h.append("\u001e");
                h.append(item);
            }
        }else{
            String oldLast=p.getString(KEY_LAST,"");
            if(oldLast!=null&&!oldLast.isEmpty()&&!oldLast.equals("尚無紀錄")){
                h.append(oldLast);
            }
        }
        if(h.length()>0)h.append("\u001e");
        h.append(encoded);
        p.edit().putString(KEY_LAST,encoded).putString(KEY_HISTORY,h.toString()).apply();
    }
    private static String displayEvent(String item){
        if(item==null||item.isEmpty())return "尚無紀錄";
        if(item.startsWith("@")){
            int bar=item.indexOf('|');
            if(bar>1){
                try{
                    long ms=Long.parseLong(item.substring(1,bar));
                    return formatTime(ms)+" · "+item.substring(bar+1);
                }catch(Exception ignored){}
            }
        }
        return item;
    }
    static long next(Context c){return prefs(c).getLong(KEY_NEXT,0);}
    static String nextReason(Context c){return prefs(c).getString(KEY_NEXT_REASON,"");}
    static String ntfyUrl(Context c){
        migrateSharedNtfy(c);
        String value=prefs(c).getString(KEY_NTFY,"");
        return value==null?"":value.trim();
    }

    private static void migrateSharedNtfy(Context c){
        SharedPreferences p=prefs(c);
        String value=p.getString(KEY_NTFY,"");
        if(value==null||value.isEmpty())return;
        try{
            byte[] bytes=MessageDigest.getInstance("SHA-256").digest(
                    value.trim().getBytes(StandardCharsets.UTF_8));
            StringBuilder hex=new StringBuilder();
            for(byte b:bytes)hex.append(String.format(Locale.ROOT,"%02x",b&0xff));
            if(LEGACY_NTFY_SHA256.equals(hex.toString())){
                p.edit().remove(KEY_NTFY).remove(KEY_NTFY_LAST).apply();
                note(c,"舊版共用 ntfy 網址已停用；需要時請自行建立新 topic");
            }
        }catch(Exception e){
            note(c,"ntfy 舊設定檢查失敗: "+safe(e));
        }
    }

    static String newNtfyUrl(){
        byte[] id=new byte[24];
        new SecureRandom().nextBytes(id);
        StringBuilder url=new StringBuilder("https://ntfy.sh/codex-");
        for(byte b:id)url.append(String.format(Locale.ROOT,"%02x",b&0xff));
        return url.toString();
    }

    static String ntfyLastStatus(Context c){
        if(ntfyUrl(c).isEmpty())return "未設定";
        return prefs(c).getString(KEY_NTFY_LAST,"尚未測試");
    }

    static void testNtfy(Context c){
        if(ntfyUrl(c).isEmpty()){
            prefs(c).edit().putString(KEY_NTFY_LAST,"未設定網址").apply();
            return;
        }
        sendNtfyOnly(c,"Codex Tools 測試","ntfy 通知測試成功");
    }
    static String lastAlarmReceived(Context c){return prefs(c).getString(KEY_LAST_ALARM_RECEIVED,"—");}
    static String lastBackupReceived(Context c){return prefs(c).getString(KEY_LAST_BACKUP_RECEIVED,"—");}
    static void markAlarmReceived(Context c){
        prefs(c).edit().putString(KEY_LAST_ALARM_RECEIVED,formatTime(System.currentTimeMillis())).apply();
    }
    private static void markBackupReceived(Context c,String source){
        prefs(c).edit().putString(KEY_LAST_BACKUP_RECEIVED,
                formatTime(System.currentTimeMillis())+" · "+source).apply();
    }

    // Default policy: preserve remaining allowance until either window is exhausted.
    static boolean resetEligibleNow(Context c){
        Snapshot s=snapshot(c);
        return s.limitReached ||
                (s.primaryActive && s.primaryUsed>=99.999) ||
                (s.secondaryActive && s.secondaryUsed>=99.999);
    }

    static boolean isBankedResetArmed(Context c){
        return prefs(c).getBoolean(KEY_BANKED_ARMED,false);
    }

    static void armBankedReset(Context c){
        synchronized(BANKED_RESET_LOCK){
            Snapshot s=snapshot(c);
            if(s.resetAvailableCount<=0 || s.resetCreditId==null || s.resetCreditId.isEmpty()){
                note(c,"無可用的 Banked Reset，無法待命");
                return;
            }
            prefs(c).edit()
                    .putBoolean(KEY_BANKED_ARMED,true)
                    .putString(KEY_BANKED_ARMED_ID,s.resetCreditId)
                    .commit();
            note(c,"Banked Reset 已待命：5h 或週配額用盡後自動使用一次");
            scheduleQuotaSampling(c);
        }
    }

    static void cancelArmedBankedReset(Context c){
        synchronized(BANKED_RESET_LOCK){
            clearArmedBankedReset(c);
            note(c,"已取消 Banked Reset 自動待命");
        }
    }

    private static void clearArmedBankedReset(Context c){
        prefs(c).edit().remove(KEY_BANKED_ARMED).remove(KEY_BANKED_ARMED_ID).commit();
    }

    private static boolean quotaExhausted(CodexClient.Quota q){
        return q.limitReached ||
                (q.primary!=null&&q.primary.usedPercent>=99.999) ||
                (q.secondary!=null&&q.secondary.usedPercent>=99.999);
    }

    // Sampling runs independently of the normal 5-hour automatic-trigger mode.
    // At most one saved reset is redeemed for each explicit opt-in.
    private static boolean maybeConsumeArmedBankedReset(Context c,CodexClient.Quota q){
        if(!isBankedResetArmed(c)||!quotaExhausted(q))return false;
        synchronized(BANKED_RESET_LOCK){
            if(!isBankedResetArmed(c))return false;
            String id=prefs(c).getString(KEY_BANKED_ARMED_ID,"");
            return consumeResetCreditInternal(c,false,id,true);
        }
    }

    static int chainRemaining(Context c){return prefs(c).getInt(KEY_CHAIN_REMAINING,0);}
    static long chainResetMs(Context c){return prefs(c).getLong(KEY_CHAIN_RESET,0);}

    static void checkNow(Context c){checkNow(c,true);}

    static void checkNow(Context c,boolean logEvent){
        try{
            CodexClient cli=new CodexClient(c);
            CodexClient.Quota q=cli.getQuota();
            storeQuota(c,q,false);
            refreshResetCredits(c,cli,logEvent);
            if(maybeConsumeArmedBankedReset(c,q))q=cli.getQuota();
            UsageHistory.refreshGlobalResetStats(c,false);
            if(logEvent)record(c,quotaHistorySummary(q));
            scheduleQuotaSampling(c);
            if(MODE_AUTO.equals(mode(c)))scheduleAutoFromQuota(c,q);
            else if(MODE_CUSTOM.equals(mode(c)))scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
        }catch(Exception e){
            if(logEvent)record(c,"更新配額失敗: "+safe(e));
        }
    }

    private static void refreshResetCredits(Context c,CodexClient cli,boolean force){
        SharedPreferences p=prefs(c);
        long now=System.currentTimeMillis();
        long last=p.getLong(KEY_RESET_LAST_CHECK,0);
        if(!force && now-last<10*60_000L)return;
        try{
            CodexClient.ResetCredits credits=cli.getResetCredits();
            CodexClient.ResetCredit first=credits.first();
            SharedPreferences.Editor e=p.edit()
                    .putInt(KEY_RESET_AVAILABLE,credits.availableCount)
                    .putLong(KEY_RESET_LAST_CHECK,now)
                    .putString(KEY_RESET_ERROR,"");
            StringBuilder expiryList=new StringBuilder();
            int ordinal=1;
            for(CodexClient.ResetCredit credit:credits.available){
                if(credit==null)continue;
                if(expiryList.length()>0)expiryList.append("\n");
                expiryList.append(ordinal++).append("|")
                        .append(credit.expiresAt==null?"":credit.expiresAt);
            }
            e.putString(KEY_RESET_EXPIRIES,expiryList.toString());
            if(first!=null){
                e.putString(KEY_RESET_CREDIT_ID,first.id)
                        .putString(KEY_RESET_TITLE,first.title==null?"Full reset":first.title)
                        .putString(KEY_RESET_DESCRIPTION,first.description==null?"":first.description)
                        .putString(KEY_RESET_EXPIRES,first.expiresAt==null?"":first.expiresAt);
            }else{
                e.remove(KEY_RESET_CREDIT_ID).remove(KEY_RESET_TITLE)
                        .remove(KEY_RESET_DESCRIPTION).remove(KEY_RESET_EXPIRES)
                        .remove(KEY_RESET_EXPIRIES);
            }
            e.apply();
            UsageHistory.recordSnapshot(c,snapshot(c));
            // Cancel an armed one-shot only when a successful server refresh
            // proves that its selected credit no longer exists.
            if(isBankedResetArmed(c)){
                String selected=p.getString(KEY_BANKED_ARMED_ID,"");
                boolean stillAvailable=false;
                for(CodexClient.ResetCredit item:credits.available){
                    if(selected.equals(item.id)){stillAvailable=true;break;}
                }
                if(!stillAvailable){
                    synchronized(BANKED_RESET_LOCK){
                        if(isBankedResetArmed(c) &&
                                selected.equals(p.getString(KEY_BANKED_ARMED_ID,""))){
                            clearArmedBankedReset(c);
                            record(c,"Banked Reset 已失效或到期，取消自動待命");
                            notifyNtfy(c,"Codex Banked Reset 待命取消",
                                    "預存重置已到期或不再可用");
                        }
                    }
                }
            }
        }catch(Exception e){
            p.edit()
                    .putInt(KEY_RESET_AVAILABLE,0)
                    .remove(KEY_RESET_CREDIT_ID)
                    .putLong(KEY_RESET_LAST_CHECK,now)
                    .putString(KEY_RESET_ERROR,safe(e))
                    .apply();
            UsageHistory.recordSnapshot(c,snapshot(c));
        }
    }

    static void consumeResetCredit(Context c){
        consumeResetCreditInternal(c,false,null,false);
    }

    static void consumeResetCreditNow(Context c){
        consumeResetCreditInternal(c,true,null,false);
    }

    private static boolean consumeResetCreditInternal(Context c,boolean allowPartial,
                                                       String requiredId,boolean automatic){
        synchronized(BANKED_RESET_LOCK){
            try{
                CodexClient cli=new CodexClient(c);
                // Always re-check server-side state immediately before spending a credit.
                CodexClient.Quota live=cli.getQuota();
                storeQuota(c,live,false);
                boolean exhausted=quotaExhausted(live);
                boolean used=exhausted ||
                        (live.primary!=null&&live.primary.usedPercent>0.0001) ||
                        (live.secondary!=null&&live.secondary.usedPercent>0.0001);
                if(!(allowPartial?used:exhausted)){
                    record(c,allowPartial?"Reset 未執行：沒有已使用的額度":
                            "Reset 未執行：5 小時與週配額都尚未用盡");
                    refreshResetCredits(c,cli,true);
                    return false;
                }

                CodexClient.ResetCredits credits=cli.getResetCredits();
                CodexClient.ResetCredit credit=null;
                for(CodexClient.ResetCredit item:credits.available){
                    if(requiredId==null || requiredId.equals(item.id)){
                        credit=item;
                        break;
                    }
                }
                if(credit==null || credits.availableCount<=0){
                    refreshResetCredits(c,cli,true);
                    if(automatic){
                        clearArmedBankedReset(c);
                        record(c,"待命的 Banked Reset 已不可用，已取消待命");
                        notifyNtfy(c,"Codex Banked Reset 待命取消","原本選定的 Reset 已不可用或到期");
                    }else{
                        record(c,"目前沒有可用的 Banked Reset");
                    }
                    return false;
                }

                CodexClient.ResetResult result=cli.consumeResetCredit(credit.id);
                if(result.windowsReset<=0){
                    refreshResetCredits(c,cli,true);
                    record(c,"Reset 未套用：伺服器沒有刷新任何用量視窗");
                    return false;
                }

                // A successful manual redemption also fulfills any armed request.
                clearArmedBankedReset(c);
                prefs(c).edit().putBoolean(KEY_PRIMARY_CONFIRMED,false).apply();
                Thread.sleep(1_000L);

                CodexClient.Quota fresh=cli.getQuota();
                storeQuota(c,fresh,false);
                refreshResetCredits(c,cli,true);
                record(c,(automatic?"用盡後自動 Reset 完成":"Reset 完成")+
                        " · "+result.windowsReset+" 個視窗");
                UsageHistory.addEvent(c,"banked_reset",automatic?
                        "Banked Reset（用盡後自動使用）":"Banked Reset（由 Codex Tools 使用）",
                        "Codex Tools");
                notifyNtfy(c,automatic?"Codex Banked Reset 自動完成":"Codex Reset 完成",
                        "已重置 "+result.windowsReset+" 個 usage window");

                if(MODE_AUTO.equals(mode(c)))scheduleAutoFromQuota(c,fresh);
                else if(MODE_CUSTOM.equals(mode(c)))scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
                return true;
            }catch(Exception e){
                record(c,(automatic?"自動 Reset":"Reset")+" 失敗: "+safe(e));
                // Keep armed mode for a later sampling retry on transient failures.
                return false;
            }
        }
    }

    static void manualTrigger(Context c){
        try{
            CodexClient cli=new CodexClient(c);
            cli.triggerMinimal();
            Thread.sleep(2_000L);
            CodexClient.Quota q=cli.getQuota();
            storeQuota(c,q,true);
            record(c,"手動觸發成功");
            notifyNtfy(c,"Codex 手動觸發成功",quotaNotification(q));
            if(MODE_AUTO.equals(mode(c)))scheduleAutoFromQuota(c,q);
            else if(MODE_CUSTOM.equals(mode(c)))scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
        }catch(Exception e){
            record(c,"手動觸發失敗: "+safe(e));
            notifyNtfy(c,"Codex 觸發失敗",safe(e));
        }
    }

    static void onAlarm(Context c){
        String m=mode(c);
        if(MODE_OFF.equals(m)){cancelAlarm(c);return;}
        String kind=prefs(c).getString(KEY_NEXT_KIND,MODE_AUTO.equals(m)?EVENT_AUTO:EVENT_ANCHOR);
        if(MODE_AUTO.equals(m)||EVENT_AUTO.equals(kind)){
            runAutoCycle(c);
        }else if(EVENT_CHAIN.equals(kind)){
            runChain(c);
        }else if(EVENT_RECOVERY.equals(kind)){
            runRecovery(c);
        }else if(EVENT_RESUME.equals(kind)){
            runResume(c);
        }else{
            runAnchor(c,EVENT_ANCHOR_RETRY.equals(kind));
        }
    }

    private static void runAutoCycle(Context c){
        try{
            CodexClient cli=new CodexClient(c);
            CodexClient.Quota q=cli.getQuota();
            storeQuota(c,q,false);

            Snapshot observed=snapshot(c);
            if(observed.primaryConfirmed && observed.primaryResetMs>System.currentTimeMillis()){
                record(c,"5 小時視窗已確認有效，等待重置");
                scheduleAutoFromQuota(c,q);
                return;
            }
            if(blocked(q)){
                record(c,"週配額已達限制，本次不觸發");
                notifyNtfy(c,"Codex 自動觸發略過","週配額目前不可用");
                scheduleAutoFromQuota(c,q);
                return;
            }

            cli.triggerMinimal();
            Thread.sleep(2_000L);
            CodexClient.Quota after=cli.getQuota();
            storeQuota(c,after,true);
            record(c,"已自動建立新的 5 小時視窗");
            notifyNtfy(c,"Codex 5 小時配額已啟動",quotaNotification(after));
            scheduleAutoFromQuota(c,after);
        }catch(Exception e){
            record(c,"自動排程失敗: "+safe(e));
            notifyNtfy(c,"Codex 排程錯誤",safe(e));
            scheduleEvent(c,System.currentTimeMillis()+15*60_000L,"15 分鐘後重試",EVENT_AUTO,0);
        }
    }

    private static void runAnchor(Context c,boolean retry){
        int autoCount=prefs(c).getInt(KEY_NEXT_AUTO_COUNT,0);
        try{
            CodexClient cli=new CodexClient(c);
            CodexClient.Quota q=cli.getQuota();
            storeQuota(c,q,false);
            long now=System.currentTimeMillis();

            Snapshot observed=snapshot(c);
            if(observed.primaryConfirmed && observed.primaryResetMs>now){
                long reset=observed.primaryResetMs;
                long wait=reset-now;
                if(wait>=0 && wait<=RESET_TOLERANCE_MS){
                    record(c,"錨點距 5 小時重置僅 "+Math.max(1,wait/1000)+" 秒，延後至重置後再觸發");
                    scheduleEvent(c,reset+RESET_GRACE_MS,"等候 5 小時重置",EVENT_ANCHOR_RETRY,autoCount);
                    return;
                }
                long anchor=prefs(c).getLong(KEY_NEXT,now);
                planRecovery(c,anchor,autoCount,reset);
                if(prefs(c).getLong(KEY_RECOVERY_AT,0)>0)
                    record(c,"錨點時已有 5h 視窗，已安排今日補接續");
                else{
                    clearChain(c);
                    record(c,"錨點未啟動，依設定取消今日後續接續");
                }
                scheduleNextCustom(c,now+1_000L);
                return;
            }

            if(blocked(q)){
                record(c,"錨點到時但週配額不可用，本次略過");
                scheduleNextCustom(c,now+1_000L);
                return;
            }

            long requestedAt=System.currentTimeMillis();
            cli.triggerMinimal();
            Thread.sleep(2_000L);
            CodexClient.Quota after=cli.getQuota();
            if(!newWindowConfirmed(q,after,requestedAt)){
                storeQuota(c,after,false);
                record(c,"錨點請求完成，但官方未確認新 5h 視窗");
                long anchor=prefs(c).getLong(KEY_NEXT,requestedAt);
                long actualReset=after.primary==null?0:after.primary.resetAt*1000L;
                if(actualReset>0 && Math.abs(actualReset-requestedAt-FIVE_HOURS_MS)<=180_000L){
                    prefs(c).edit().putLong(KEY_RECOVERY_ANCHOR,anchor)
                            .putLong(KEY_RECOVERY_AT,System.currentTimeMillis()+150_000L)
                            .putInt(KEY_RECOVERY_COUNT,autoCount)
                            .putLong(KEY_VERIFY_RESET,actualReset).apply();
                    record(c,"錨點請求已完成，稍後驗證 5h 是否啟動");
                }else if(actualReset>System.currentTimeMillis()
                    && actualReset-FIVE_HOURS_MS<anchor-2_000L){
                    planRecovery(c,anchor,autoCount,actualReset);
                }
                scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
                return;
            }
            storeQuota(c,after,true);
            record(c,retry?"重置後延遲觸發成功":"錨點觸發成功");
            notifyNtfy(c,"Codex 排程觸發成功",quotaNotification(after));
            startChainFrom(c,after,autoCount);
            scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
        }catch(Exception e){
            record(c,"錨點觸發失敗: "+safe(e));
            notifyNtfy(c,"Codex 排程錯誤",safe(e));
            scheduleNextCustom(c,System.currentTimeMillis()+60_000L);
        }
    }

    private static boolean chainWouldBlockNextAnchor(Context c,long triggerAt){
        if(triggerAt<=0)return false;
        ScheduleConfig.Slot next=ScheduleConfig.nextAnchor(c,triggerAt+1_000L);
        if(next==null)return false;
        long predictedReset=triggerAt+5*60*60_000L;
        return next.whenMs < predictedReset-RESET_TOLERANCE_MS;
    }

    private static boolean stopChainIfWouldBlockAnchor(Context c,long triggerAt){
        if(!chainWouldBlockNextAnchor(c,triggerAt))return false;
        ScheduleConfig.Slot next=ScheduleConfig.nextAnchor(c,triggerAt+1_000L);
        clearChain(c);
        record(c,"自動接續停止：避免影響下一錨點 "+(next==null?"":formatTime(next.whenMs)));
        return true;
    }

    private static void runChain(Context c){
        int remaining=chainRemaining(c);
        if(remaining<=0){
            clearChain(c);
            scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
            return;
        }

        try{
            CodexClient cli=new CodexClient(c);
            CodexClient.Quota q=cli.getQuota();
            storeQuota(c,q,false);
            long now=System.currentTimeMillis();

            Snapshot observed=snapshot(c);
            if(observed.primaryConfirmed && observed.primaryResetMs>now){
                long reset=observed.primaryResetMs;
                long triggerAt=Math.max(now+5_000L,reset+RESET_GRACE_MS);
                if(stopChainIfWouldBlockAnchor(c,triggerAt)){
                    scheduleNextCustom(c,now+1_000L);
                    return;
                }
                record(c,"自動接續等待實際 5 小時重置");
                prefs(c).edit().putLong(KEY_CHAIN_RESET,reset).apply();
                scheduleEvent(c,triggerAt,
                        "自動接續，尚餘 "+remaining+" 次",EVENT_CHAIN,0);
                return;
            }

            if(blocked(q)){
                record(c,"自動接續因週配額不可用而停止");
                clearChain(c);
                scheduleNextCustom(c,now+1_000L);
                return;
            }

            if(stopChainIfWouldBlockAnchor(c,now)){
                scheduleNextCustom(c,now+1_000L);
                return;
            }

            cli.triggerMinimal();
            Thread.sleep(2_000L);
            CodexClient.Quota after=cli.getQuota();
            storeQuota(c,after,true);
            remaining--;
            record(c,"自動接續觸發成功"+(remaining>0?"，尚餘 "+remaining+" 次":""));
            notifyNtfy(c,"Codex 自動接續成功",quotaNotification(after));

            Snapshot afterObserved=snapshot(c);
            if(remaining>0 && afterObserved.primaryConfirmed && afterObserved.primaryResetMs>System.currentTimeMillis()){
                long nextTrigger=afterObserved.primaryResetMs+RESET_GRACE_MS;
                if(chainWouldBlockNextAnchor(c,nextTrigger)){
                    clearChain(c);
                    ScheduleConfig.Slot next=ScheduleConfig.nextAnchor(c,nextTrigger+1_000L);
                    record(c,"後續接續停止：保留下個固定錨點 "+(next==null?"":formatTime(next.whenMs)));
                }else{
                    prefs(c).edit()
                            .putInt(KEY_CHAIN_REMAINING,remaining)
                            .putLong(KEY_CHAIN_RESET,afterObserved.primaryResetMs)
                            .apply();
                }
            }else{
                clearChain(c);
            }
            scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
        }catch(Exception e){
            record(c,"自動接續失敗: "+safe(e));
            notifyNtfy(c,"Codex 自動接續錯誤",safe(e));
            scheduleEvent(c,System.currentTimeMillis()+5*60_000L,
                    "自動接續 5 分鐘後重試",EVENT_CHAIN,0);
        }
    }

    private static void startChainFrom(Context c,CodexClient.Quota after,int count){
        Snapshot s=snapshot(c);
        if(count>0 && s.primaryConfirmed && s.primaryResetMs>System.currentTimeMillis()){
            long firstTrigger=s.primaryResetMs+RESET_GRACE_MS;
            if(chainWouldBlockNextAnchor(c,firstTrigger)){
                clearChain(c);
                ScheduleConfig.Slot next=ScheduleConfig.nextAnchor(c,firstTrigger+1_000L);
                record(c,"未啟動自動接續：會影響下一錨點 "+(next==null?"":formatTime(next.whenMs)));
                return;
            }
            prefs(c).edit()
                    .putInt(KEY_CHAIN_REMAINING,count)
                    .putLong(KEY_CHAIN_RESET,s.primaryResetMs)
                    .apply();
        }else{
            clearChain(c);
        }
    }

    private static void clearChain(Context c){
        prefs(c).edit().remove(KEY_CHAIN_REMAINING).remove(KEY_CHAIN_RESET).apply();
    }

    static void reschedule(Context c){
        String m=mode(c);
        if(MODE_OFF.equals(m)){cancelAlarm(c);return;}
        if(MODE_CUSTOM.equals(m)){
            scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
            return;
        }

        Snapshot s=snapshot(c);
        long now=System.currentTimeMillis();
        if(s.primaryConfirmed && s.primaryResetMs>now){
            scheduleEvent(c,s.primaryResetMs+RESET_GRACE_MS,"5 小時視窗重置後",EVENT_AUTO,0);
        }else if(s.secondaryActive && s.secondaryUsed>=100.0 && s.secondaryResetMs>now){
            scheduleEvent(c,s.secondaryResetMs+RESET_GRACE_MS,"週配額重置後",EVENT_AUTO,0);
        }else{
            scheduleEvent(c,now+60_000L,"檢查並視需要觸發",EVENT_AUTO,0);
        }
    }

    private static void scheduleAutoFromQuota(Context c,CodexClient.Quota q){
        long now=System.currentTimeMillis();
        Snapshot s=snapshot(c);
        if(s.primaryConfirmed && s.primaryResetMs>now){
            scheduleEvent(c,s.primaryResetMs+RESET_GRACE_MS,"5 小時視窗重置後",EVENT_AUTO,0);
        }else if(q.secondary!=null&&q.secondary.usedPercent>=100.0&&q.secondary.resetAt>0){
            scheduleEvent(c,q.secondary.resetAt*1000L+RESET_GRACE_MS,"週配額重置後",EVENT_AUTO,0);
        }else{
            // No confirmed 5h window. A zero-usage primary_window may merely be a
            // sliding now+5h placeholder, so don't wait for that synthetic reset.
            scheduleEvent(c,now+15_000L,"尚未確認 5 小時視窗，準備觸發",EVENT_AUTO,0);
        }
    }

    static void clearRecovery(Context c){
        prefs(c).edit().remove(KEY_RECOVERY_AT).remove(KEY_RECOVERY_ANCHOR)
                .remove(KEY_RECOVERY_COUNT).remove(KEY_VERIFY_RESET).apply();
    }

    private static String missedPolicy(Context c){
        return prefs(c).getString(KEY_MISSED_POLICY,POLICY_NEXT_SLOT);
    }

    private static void planRecovery(Context c,long anchor,int autoCount,long reset){
        if(!MODE_CUSTOM.equals(mode(c))||reset<=0||anchor<=0)return;
        String policy=missedPolicy(c);
        if(POLICY_SKIP.equals(policy)||POLICY_NEXT_SLOT.equals(policy)&&autoCount<1)return;
        long at=POLICY_AT_RESET.equals(policy)?reset+RESET_GRACE_MS:
                Math.max(anchor+FIVE_HOURS_MS,reset+RESET_GRACE_MS);
        int remaining=POLICY_AT_RESET.equals(policy)?autoCount:Math.max(0,autoCount-1);
        ScheduleConfig.Slot next=ScheduleConfig.nextAnchor(c,anchor+1_000L);
        if(next!=null && at+FIVE_HOURS_MS>next.whenMs+RESET_TOLERANCE_MS){
            record(c,"錨點補接續會覆蓋下一個錨點，取消補接續");
            return;
        }
        prefs(c).edit().putLong(KEY_RECOVERY_ANCHOR,anchor)
                .putLong(KEY_RECOVERY_AT,at)
                .putInt(KEY_RECOVERY_COUNT,remaining).apply();
    }

    private static void detectMissedToday(Context c,long now){
        if(!MODE_CUSTOM.equals(mode(c))||POLICY_SKIP.equals(missedPolicy(c))
                ||prefs(c).getLong(KEY_RECOVERY_AT,0)>0)return;
        Snapshot q=snapshot(c);
        if(!q.primaryConfirmed||q.primaryResetMs<=now)return;
        LocalDate date=Instant.ofEpochMilli(now).atZone(ZoneId.systemDefault()).toLocalDate();
        for(ScheduleConfig.Rule rule:ScheduleConfig.rulesForDate(c,date)){
            if(!rule.enabled)continue;
            long anchor=date.atTime(rule.hour,rule.minute)
                    .atZone(ZoneId.systemDefault()).toInstant().toEpochMilli();
            if(now>=anchor && now<anchor+FIVE_HOURS_MS
                    && q.primaryResetMs-FIVE_HOURS_MS<anchor-2_000L){
                planRecovery(c,anchor,rule.autoCount,q.primaryResetMs);
                if(prefs(c).getLong(KEY_RECOVERY_AT,0)>0){
                    record(c,"偵測到今日錯過錨點，補接續排程已恢復");
                    break;
                }
            }
        }
    }

    private static boolean newWindowConfirmed(CodexClient.Quota before,CodexClient.Quota after,long requestedAt){
        if(after.primary==null)return false;
        long reset=after.primary.resetAt*1000L;
        long previous=before.primary==null?0:before.primary.resetAt*1000L;
        // Don't mistake a completed request in an already-running 5h window
        // for a new one, even if the remaining quota still rounds to 100%.
        return reset>previous+90_000L && Math.abs(reset-requestedAt-FIVE_HOURS_MS)<=180_000L;
    }

    private static void runRecovery(Context c){
        long anchor=prefs(c).getLong(KEY_RECOVERY_ANCHOR,0);
        int autoCount=prefs(c).getInt(KEY_RECOVERY_COUNT,0);
        if(anchor<=0){scheduleNextCustom(c,System.currentTimeMillis()+1_000L);return;}
        try{
            CodexClient.Quota quota=new CodexClient(c).getQuota();
            storeQuota(c,quota,false);
            Snapshot live=snapshot(c);
            long now=System.currentTimeMillis();
            long verifyReset=prefs(c).getLong(KEY_VERIFY_RESET,0);
            if(verifyReset>0){
                // The first post-trigger quota may still look like an idle
                // now+5h placeholder. After >=2 minutes a fixed reset proves
                // this was actually started, without posting another request.
                if(live.primaryConfirmed && Math.abs(live.primaryResetMs-verifyReset)<=20_000L){
                    clearRecovery(c);
                    record(c,"新 5h 已由穩定重置時間確認，接續排程恢復");
                    notifyNtfy(c,"Codex 5h 啟動已確認",quotaNotification(quota));
                    startChainFrom(c,quota,autoCount);
                    scheduleNextCustom(c,now+1_000L);
                }else{
                    if(POLICY_SKIP.equals(missedPolicy(c))){
                        clearRecovery(c);
                        record(c,"伺服器未確認新視窗，依設定取消今日接續");
                    }else{
                        prefs(c).edit().remove(KEY_VERIFY_RESET)
                                .putLong(KEY_RECOVERY_AT,now+15*60_000L).apply();
                        record(c,"伺服器的重置時間仍未固定，15 分鐘後再檢查");
                    }
                    scheduleNextCustom(c,now+1_000L);
                }
                return;
            }
            if(live.primaryConfirmed && live.primaryResetMs>now){
                // The next nominal slot cannot restart an active server window.
                long delayed=live.primaryResetMs+RESET_GRACE_MS;
                prefs(c).edit().putLong(KEY_RECOVERY_AT,delayed).apply();
                record(c,"補接續時仍有有效視窗，延到真正重置後");
                scheduleNextCustom(c,now+1_000L);
                return;
            }
            if(blocked(quota)){
                clearRecovery(c);
                record(c,"補接續因官方配額不可用而停止");
                scheduleNextCustom(c,now+1_000L);
                return;
            }
            CodexClient cli=new CodexClient(c);
            long requestedAt=System.currentTimeMillis();
            cli.triggerMinimal();
            Thread.sleep(2_000L);
            CodexClient.Quota after=cli.getQuota();
            if(newWindowConfirmed(quota,after,requestedAt)){
                clearRecovery(c);
                storeQuota(c,after,true);
                record(c,"錨點失敗後已完成今日補接續");
                notifyNtfy(c,"Codex 今日補接續成功",quotaNotification(after));
                startChainFrom(c,after,autoCount);
                scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
            }else{
                storeQuota(c,after,false);
                long actualReset=after.primary==null?0:after.primary.resetAt*1000L;
                boolean plausible=actualReset>0&&Math.abs(actualReset-requestedAt-FIVE_HOURS_MS)<=180_000L;
                SharedPreferences.Editor editor=prefs(c).edit();
                if(plausible)editor.putLong(KEY_VERIFY_RESET,actualReset);
                else editor.remove(KEY_VERIFY_RESET);
                editor.putLong(KEY_RECOVERY_AT,System.currentTimeMillis()+(plausible?150_000L:15*60_000L)).apply();
                record(c,plausible?"補接續請求已完成，稍後驗證新視窗":"補接續未確認，15 分鐘後再檢查");
                scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
            }
        }catch(Exception e){
            record(c,"補接續失敗: "+safe(e));
            prefs(c).edit().putLong(KEY_RECOVERY_AT,System.currentTimeMillis()+15*60_000L).apply();
            scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
        }
    }

    static final class DayResume {
        final long whenMs;
        final int autoCount;
        final String date;
        DayResume(long at,int count,String date){this.whenMs=at;this.autoCount=count;this.date=date;}
    }

    static void clearDailyResume(Context c){
        prefs(c).edit().remove(KEY_RESUME_DONE_DAY).remove(KEY_RESUME_PENDING_DAY)
                .remove(KEY_RESUME_COUNT).remove(KEY_RESUME_NEXT_AT).remove(KEY_RESUME_VERIFY).apply();
    }

    private static void clearResumePending(Context c){
        prefs(c).edit().remove(KEY_RESUME_PENDING_DAY).remove(KEY_RESUME_COUNT)
                .remove(KEY_RESUME_NEXT_AT).remove(KEY_RESUME_VERIFY).apply();
    }

    private static DayResume nextDayResume(Context c,long afterMs){
        SharedPreferences p=prefs(c);
        ZoneId zone=ZoneId.systemDefault();
        long now=System.currentTimeMillis();
        LocalDate today=Instant.ofEpochMilli(now).atZone(zone).toLocalDate();
        String done=p.getString(KEY_RESUME_DONE_DAY,"");
        String pending=p.getString(KEY_RESUME_PENDING_DAY,"");
        // Retain a pending post-request verification or retry across sampling,
        // process death and Android reboot.
        long pendingAt=p.getLong(KEY_RESUME_NEXT_AT,0);
        if(!pending.isEmpty() && pendingAt>0 && !pending.equals(done)){
            try{
                LocalDate date=LocalDate.parse(pending);
                if(!date.isBefore(today) && !date.isAfter(today.plusDays(1)))
                    return new DayResume(Math.max(now+1_000L,pendingAt),
                        p.getInt(KEY_RESUME_COUNT,0),pending);
            }catch(Exception ignored){}
            clearResumePending(c);
        }
        Snapshot q=snapshot(c);
        for(int offset=0;offset<=7;offset++){
            LocalDate date=today.plusDays(offset);
            if(!done.isEmpty()&&date.toString().compareTo(done)<=0)continue;
            int configured=ScheduleConfig.resumeForDate(c,date);
            if(configured<0)continue;
            long start=date.atStartOfDay(zone).toInstant().toEpochMilli();
            long end=date.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli();
            long reset=q.primaryConfirmed&&q.primaryResetMs>now?q.primaryResetMs:0;
            long at=Math.max(Math.max(afterMs,now),Math.max(start,reset))+RESET_GRACE_MS;
            if(at>=end)continue;
            ScheduleConfig.Slot anchor=ScheduleConfig.nextAnchor(c,at-1_000L);
            long nextFixed=anchor==null?Long.MAX_VALUE:anchor.whenMs;
            if(nextFixed!=Long.MAX_VALUE && at+FIVE_HOURS_MS>nextFixed+RESET_TOLERANCE_MS)continue;
            // Weekly continuation covers the whole day, with the final 5h
            // window allowed to cross midnight. Tomorrow resumes at its reset.
            boolean continuous=ScheduleConfig.continuousForDate(c,date);
            long latest=continuous?end-1:at+6*FIVE_HOURS_MS;
            if(nextFixed!=Long.MAX_VALUE)latest=Math.min(latest,nextFixed-FIVE_HOURS_MS);
            int possible=(int)Math.max(0,Math.min(6,(latest-at)/FIVE_HOURS_MS));
            return new DayResume(at,Math.min(configured,possible),date.toString());
        }
        return null;
    }

    private static void delayResume(Context c,String date,int count,long at,long verify){
        SharedPreferences.Editor e=prefs(c).edit()
                .putString(KEY_RESUME_PENDING_DAY,date)
                .putInt(KEY_RESUME_COUNT,count)
                .putLong(KEY_RESUME_NEXT_AT,Math.max(at,System.currentTimeMillis()+1_000L));
        if(verify>0)e.putLong(KEY_RESUME_VERIFY,verify);
        else e.remove(KEY_RESUME_VERIFY);
        e.apply();
        scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
    }

    private static void runResume(Context c){
        SharedPreferences p=prefs(c);
        String date=p.getString(KEY_RESUME_PENDING_DAY,"");
        int count=p.getInt(KEY_RESUME_COUNT,0);
        if(date.isEmpty()){scheduleNextCustom(c,System.currentTimeMillis()+1_000L);return;}
        LocalDate day;
        try{day=LocalDate.parse(date);}catch(Exception e){clearResumePending(c);reschedule(c);return;}
        if(!day.equals(LocalDate.now())){
            clearResumePending(c);
            scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
            return;
        }
        try{
            CodexClient cli=new CodexClient(c);
            CodexClient.Quota before=cli.getQuota();
            storeQuota(c,before,false);
            Snapshot live=snapshot(c);
            long now=System.currentTimeMillis();
            long verifying=p.getLong(KEY_RESUME_VERIFY,0);
            if(verifying>0){
                if(live.primaryConfirmed&&Math.abs(live.primaryResetMs-verifying)<=20_000L){
                    prefs(c).edit().putString(KEY_RESUME_DONE_DAY,date).apply();
                    clearResumePending(c);
                    record(c,"已確認首次 5h 接續，啟動後續排程");
                    startChainFrom(c,before,count);
                    scheduleNextCustom(c,now+1_000L);
                }else{
                    record(c,"首次接續尚待伺服器確認，15 分鐘後再驗證");
                    delayResume(c,date,count,now+15*60_000L,0);
                }
                return;
            }
            if(live.primaryConfirmed&&live.primaryResetMs>now){
                record(c,"接續等待目前有效 5h 視窗真正結束");
                delayResume(c,date,count,live.primaryResetMs+RESET_GRACE_MS,0);
                return;
            }
            if(blocked(before)){
                record(c,"首次接續因週配額限制，15 分鐘後確認");
                delayResume(c,date,count,now+15*60_000L,0);
                return;
            }
            long requestedAt=System.currentTimeMillis();
            cli.triggerMinimal();
            Thread.sleep(2_000L);
            CodexClient.Quota after=cli.getQuota();
            if(newWindowConfirmed(before,after,requestedAt)){
                storeQuota(c,after,true);
                prefs(c).edit().putString(KEY_RESUME_DONE_DAY,date).apply();
                clearResumePending(c);
                record(c,"首次 5h 接續成功");
                notifyNtfy(c,"Codex 5h 已依重置接續",quotaNotification(after));
                startChainFrom(c,after,count);
                scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
            }else{
                storeQuota(c,after,false);
                long reset=after.primary==null?0:after.primary.resetAt*1000L;
                boolean plausible=reset>0&&Math.abs(reset-requestedAt-FIVE_HOURS_MS)<=180_000L;
                record(c,plausible?"首次接續請求完成，稍後驗證新視窗":"首次接續未確認，稍後重試");
                delayResume(c,date,count,System.currentTimeMillis()+(plausible?150_000L:15*60_000L),
                        plausible?reset:0);
            }
        }catch(Exception e){
            record(c,"首次 5h 接續失敗："+safe(e));
            delayResume(c,date,count,System.currentTimeMillis()+15*60_000L,0);
        }
    }

    static void scheduleNextCustom(Context c,long afterMs){
        detectMissedToday(c,System.currentTimeMillis());
        ScheduleConfig.Slot anchor=ScheduleConfig.nextAnchor(c,afterMs);
        long anchorMs=anchor==null?Long.MAX_VALUE:anchor.whenMs;

        int remaining=chainRemaining(c);
        long reset=chainResetMs(c);
        long chainMs=(remaining>0&&reset>0)?Math.max(afterMs+1_000L,reset+RESET_GRACE_MS):Long.MAX_VALUE;
        if(chainMs!=Long.MAX_VALUE && chainWouldBlockNextAnchor(c,chainMs)){
            clearChain(c);
            remaining=0;
            chainMs=Long.MAX_VALUE;
        }
        long recoveryMs=prefs(c).getLong(KEY_RECOVERY_AT,0);
        int recoveryCount=prefs(c).getInt(KEY_RECOVERY_COUNT,0);
        if(recoveryMs<=0)recoveryMs=Long.MAX_VALUE;
        else recoveryMs=Math.max(afterMs+1_000L,recoveryMs);

        if(recoveryMs==Long.MAX_VALUE && chainMs==Long.MAX_VALUE && anchorMs==Long.MAX_VALUE){
            cancelAlarm(c);
            prefs(c).edit().putString(KEY_NEXT_REASON,"尚未設定可用時段").apply();
            return;
        }

        if(recoveryMs<=anchorMs && recoveryMs<=chainMs){
            String reason=prefs(c).getLong(KEY_VERIFY_RESET,0)>0
                    ?"確認已發送的 Codex 請求是否啟動 5h":
                    "錯過錨點 · 今日補接續，後續自動 "+recoveryCount+" 次";
            scheduleEvent(c,recoveryMs,reason,EVENT_RECOVERY,recoveryCount);
        }else if(chainMs<=anchorMs){
            scheduleEvent(c,chainMs,"自動接續，尚餘 "+remaining+" 次",EVENT_CHAIN,0);
        }else{
            scheduleEvent(c,anchor.whenMs,
                    anchor.source+"錨點"+(anchor.autoCount>0?"，後續自動 "+anchor.autoCount+" 次":""),
                    EVENT_ANCHOR,anchor.autoCount);
        }
    }

    private static ScheduleConfig.Slot nextEligibleAnchor(Context c,long afterMs){
        // Do not pre-filter a 06:00 anchor merely because a server window
        // started seconds earlier: onAlarm must apply the chosen recovery policy.
        return ScheduleConfig.nextAnchor(c,afterMs);
    }

    static boolean canExact(Context c){
        AlarmManager am=(AlarmManager)c.getSystemService(Context.ALARM_SERVICE);
        return Build.VERSION.SDK_INT<31||am.canScheduleExactAlarms();
    }

    static Intent exactSettings(Context c){
        return new Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM,Uri.parse("package:"+c.getPackageName()));
    }

    private static void scheduleEvent(Context c,long when,String reason,String kind,int autoCount){
        AlarmManager am=(AlarmManager)c.getSystemService(Context.ALARM_SERVICE);
        PendingIntent pi=pendingIntent(c);
        if(Build.VERSION.SDK_INT>=31&&!am.canScheduleExactAlarms()){
            am.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP,when,pi);
        }else{
            am.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP,when,pi);
        }
        prefs(c).edit()
                .putLong(KEY_NEXT,when)
                .putString(KEY_NEXT_REASON,reason)
                .putString(KEY_NEXT_KIND,kind)
                .putInt(KEY_NEXT_AUTO_COUNT,autoCount)
                .apply();
        scheduleBackupJobs(c,when);
    }

    static void cancelAlarm(Context c){
        AlarmManager am=(AlarmManager)c.getSystemService(Context.ALARM_SERVICE);
        am.cancel(pendingIntent(c));
        JobScheduler js=(JobScheduler)c.getSystemService(Context.JOB_SCHEDULER_SERVICE);
        if(js!=null){
            js.cancel(BACKUP_JOB_ID);
        }
        prefs(c).edit()
                .remove(KEY_NEXT)
                .remove(KEY_NEXT_REASON)
                .remove(KEY_NEXT_KIND)
                .remove(KEY_NEXT_AUTO_COUNT)
                .apply();
    }

    private static void scheduleBackupJobs(Context c,long when){
        JobScheduler js=(JobScheduler)c.getSystemService(Context.JOB_SCHEDULER_SERVICE);
        if(js==null)return;

        long now=System.currentTimeMillis();
        long delay=Math.max(0,when-now+BACKUP_DELAY_MS);

        PersistableBundle extras=new PersistableBundle();
        extras.putLong("expected_when",when);
        extras.putString("source","單次保底");
        JobInfo backup=new JobInfo.Builder(BACKUP_JOB_ID,
                new ComponentName(c,BackupJobService.class))
                .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                .setMinimumLatency(delay)
                .setOverrideDeadline(delay+BACKUP_DEADLINE_EXTRA_MS)
                .setPersisted(true)
                .setExtras(extras)
                .build();
        js.schedule(backup);
    }

    static void onBackupJob(Context c,long expectedWhen,String source){
        if(!enabled(c)||expectedWhen<=0)return;
        long now=System.currentTimeMillis();
        long current=next(c);

        if(current<=0 || Math.abs(current-expectedWhen)>2_000L || now<expectedWhen){
            return; // stale backup for an event that has already been rescheduled.
        }

        markBackupReceived(c,source);
        note(c,"保底排程接管 · "+source);
        onAlarm(c);
    }

    private static PendingIntent pendingIntent(Context c){
        return PendingIntent.getBroadcast(c,REQUEST_CODE,new Intent(c,AlarmReceiver.class),
                PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE);
    }

    static String formatTime(long ms){
        if(ms<=0)return "—";
        ZonedDateTime z=Instant.ofEpochMilli(ms).atZone(ZoneId.systemDefault());
        LocalDate d=z.toLocalDate();
        LocalDate today=LocalDate.now(ZoneId.systemDefault());
        String time=String.format(Locale.TAIWAN,"%02d:%02d:%02d",z.getHour(),z.getMinute(),z.getSecond());
        if(d.equals(today))return time;
        if(d.equals(today.plusDays(1)))return "明天 "+time;
        if(d.equals(today.minusDays(1)))return "昨天 "+time;
        String[] weekday={"一","二","三","四","五","六","日"};
        return String.format(Locale.TAIWAN,"%d/%d (%s) %s",
                d.getMonthValue(),d.getDayOfMonth(),weekday[d.getDayOfWeek().getValue()-1],time);
    }

    static String formattedNext(Context c){
        long t=next(c);
        if(t<=0)return "尚未排程";
        String reason=nextReason(c);
        return formatTime(t)+(reason.isEmpty()?"":" · "+reason);
    }

    static String customPreview(Context c){
        StringBuilder b=new StringBuilder();
        if(chainRemaining(c)>0 && chainResetMs(c)>0){
            long t=chainResetMs(c)+RESET_GRACE_MS;
            b.append("接續 ").append(chainRemaining(c)).append(" 次：")
                    .append(formatTime(t));
            long estimated=t;
            for(int i=1;i<Math.min(chainRemaining(c),3);i++){
                estimated+=5*60*60_000L;
                b.append(" → ").append(formatTime(estimated));
            }
        }
        ScheduleConfig.Slot a=ScheduleConfig.nextAnchor(c,System.currentTimeMillis()+1_000L);
        if(a!=null){
            if(b.length()>0)b.append("\n");
            b.append("下一錨點：").append(formatTime(a.whenMs));
            if(a.autoCount>0)b.append(" + 自動 ").append(a.autoCount).append(" 次");
        }
        return b.length()==0?"尚無排程":b.toString();
    }

    private static boolean blocked(CodexClient.Quota q){
        return !q.allowed || (q.secondary!=null&&q.secondary.usedPercent>=100.0);
    }

    private static void storeQuota(Context c,CodexClient.Quota q,boolean confirmBecauseTriggered){
        SharedPreferences p=prefs(c);
        long now=System.currentTimeMillis();
        long previousCheck=p.getLong(KEY_LAST_CHECK,0);
        long previousReset=p.getLong(KEY_PRIMARY_RESET,0);
        boolean previousConfirmed=p.getBoolean(KEY_PRIMARY_CONFIRMED,false);
        double previousWeeklyUsed=Double.longBitsToDouble(
                p.getLong(KEY_SECONDARY_USED,Double.doubleToLongBits(0)));
        long previousWeeklyReset=p.getLong(KEY_SECONDARY_RESET,0);
        boolean naturalWeekly=previousWeeklyReset>0 &&
                now>=previousWeeklyReset-15*60_000L &&
                now<=previousWeeklyReset+60*60_000L;
        boolean unexpectedWeeklyRefresh=q.secondary!=null &&
                previousWeeklyUsed-q.secondary.usedPercent>2.0 && !naturalWeekly;

        boolean confirmed=false;
        long newReset=0;
        if(q.primary!=null){
            newReset=q.primary.resetAt*1000L;
            if(confirmBecauseTriggered || q.primary.usedPercent>0.0001){
                confirmed=true;
            }else if(previousConfirmed && previousReset>now &&
                    Math.abs(newReset-previousReset)<=120_000L){
                confirmed=true;
            }else if(previousCheck>0 && previousReset>0){
                long elapsed=Math.max(0,now-previousCheck);
                long slide=Math.abs(newReset-previousReset);
                // Idle placeholder: reset moves almost one-for-one with each GET.
                // Real active window: reset remains essentially fixed.
                if(elapsed>=15_000L && slide<=Math.max(5_000L,elapsed/4)){
                    confirmed=true;
                }
            }
        }

        // A global reset can replace a confirmed 5h window with a zero-usage
        // sliding placeholder having nearly the same timestamp. Do not preserve
        // the old confirmed window or its 5h forecast line across that event.
        if(unexpectedWeeklyRefresh && q.primary!=null &&
                q.primary.usedPercent<=0.0001){
            confirmed=false;
        }

        SharedPreferences.Editor e=p.edit()
                .putString(KEY_PLAN,q.plan)
                .putBoolean(KEY_ALLOWED,q.allowed)
                .putBoolean(KEY_LIMIT_REACHED,q.limitReached)
                .putLong(KEY_LAST_CHECK,now)
                .putBoolean(KEY_PRIMARY_CONFIRMED,confirmed);

        if(q.primary!=null){
            e.putBoolean(KEY_PRIMARY_ACTIVE,true)
                    .putLong(KEY_PRIMARY_USED,Double.doubleToLongBits(q.primary.usedPercent))
                    .putLong(KEY_PRIMARY_RESET,newReset);
        }else{
            e.putBoolean(KEY_PRIMARY_ACTIVE,false)
                    .putBoolean(KEY_PRIMARY_CONFIRMED,false)
                    .putLong(KEY_PRIMARY_USED,Double.doubleToLongBits(0))
                    .putLong(KEY_PRIMARY_RESET,0);
        }

        if(q.secondary!=null){
            e.putBoolean(KEY_SECONDARY_ACTIVE,true)
                    .putLong(KEY_SECONDARY_USED,Double.doubleToLongBits(q.secondary.usedPercent))
                    .putLong(KEY_SECONDARY_RESET,q.secondary.resetAt*1000L);
        }else{
            e.putBoolean(KEY_SECONDARY_ACTIVE,false)
                    .putLong(KEY_SECONDARY_USED,Double.doubleToLongBits(0))
                    .putLong(KEY_SECONDARY_RESET,0);
        }
        e.apply();
        UsageHistory.recordSnapshot(c,snapshot(c));
    }

    static void scheduleQuotaSampling(Context c){
        if(!new CodexClient(c).signedIn())return;
        JobScheduler js=(JobScheduler)c.getSystemService(Context.JOB_SCHEDULER_SERVICE);
        if(js==null)return;
        long delay=samplingMinutes(c)*60_000L;
        JobInfo job=new JobInfo.Builder(SAMPLE_JOB_ID,
                new ComponentName(c,SamplingJobService.class))
                .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                .setMinimumLatency(delay)
                .setOverrideDeadline(delay+5*60_000L)
                .setPersisted(true)
                .build();
        js.schedule(job);
    }

    static void runQuotaSample(Context c){
        try{
            CodexClient cli=new CodexClient(c);
            if(!cli.signedIn())return;
            CodexClient.Quota q=cli.getQuota();
            storeQuota(c,q,false);
            refreshResetCredits(c,cli,false);
            if(maybeConsumeArmedBankedReset(c,q))q=cli.getQuota();
            UsageHistory.refreshGlobalResetStats(c,false);
            if(MODE_AUTO.equals(mode(c)))scheduleAutoFromQuota(c,q);
            else if(MODE_CUSTOM.equals(mode(c)))scheduleNextCustom(c,System.currentTimeMillis()+1_000L);
        }catch(Exception e){
            note(c,"配額採樣失敗: "+safe(e));
        }finally{
            scheduleQuotaSampling(c);
        }
    }

    private static void record(Context c,String message){
        note(c,message);
    }

    private static String quotaHistorySummary(CodexClient.Quota q){
        String p=q.primary==null?"5h —":
                String.format(Locale.TAIWAN,"5h %.0f%%",Math.max(0,100-q.primary.usedPercent));
        String w=q.secondary==null?"週 —":
                String.format(Locale.TAIWAN,"週 %.0f%%",Math.max(0,100-q.secondary.usedPercent));
        return p+" · "+w;
    }

    private static String quotaNotification(CodexClient.Quota q){
        String p=q.primary==null?"5 小時：未啟動":
                String.format(Locale.TAIWAN,"5 小時剩餘 %.0f%%，重置 %s",
                        Math.max(0,100-q.primary.usedPercent),formatTime(q.primary.resetAt*1000L));
        String w=q.secondary==null?"週配額：無資料":
                String.format(Locale.TAIWAN,"週配額剩餘 %.0f%%，重置 %s",
                        Math.max(0,100-q.secondary.usedPercent),formatTime(q.secondary.resetAt*1000L));
        return p+"\n"+w;
    }

    private static String safe(Exception e){
        String m=e.getMessage();
        return e.getClass().getSimpleName()+(m==null?"":": "+m);
    }

    private static void notifyNtfy(Context c,String title,String body){
        // On-device notifications are primary; ntfy is an optional secondary channel.
        AndroidNotifications.post(c,title,body);
        if(!ntfyUrl(c).isEmpty())sendNtfyOnly(c,title,body);
    }

    private static void sendNtfyOnly(Context c,String title,String body){
        String u=ntfyUrl(c);
        if(u.isEmpty())return;
        try{
            new CodexClient(c).sendNtfy(u,title,body);
            prefs(c).edit().putString(KEY_NTFY_LAST,
                    formatTime(System.currentTimeMillis())+" · 已送出").apply();
        }catch(Exception e){
            String msg="ntfy 發送失敗: "+safe(e);
            prefs(c).edit().putString(KEY_NTFY_LAST,
                    formatTime(System.currentTimeMillis())+" · "+msg).apply();
            note(c,msg);
        }
    }
}
