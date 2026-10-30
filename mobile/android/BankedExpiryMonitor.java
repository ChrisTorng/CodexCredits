package com.christorng.androidcodextools;

import android.content.Context;
import android.content.SharedPreferences;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.time.OffsetDateTime;
import java.util.Locale;

/** Native Android background reminder. Called by the JobScheduler quota collector. */
final class BankedExpiryMonitor {
    private static final long DAY=86_400_000L;
    private BankedExpiryMonitor(){}

    static void check(Context context, CodexClient.ResetCredits credits){
        if(credits==null||credits.available.isEmpty())return;
        long now=System.currentTimeMillis();
        CodexClient.ResetCredit nearest=null;
        long remaining=Long.MAX_VALUE;
        for(CodexClient.ResetCredit c:credits.available){
            try{
                long millis=OffsetDateTime.parse(c.expiresAt).toInstant().toEpochMilli()-now;
                if(millis>0&&millis<remaining){remaining=millis;nearest=c;}
            }catch(Exception ignored){}
        }
        if(nearest==null||remaining>7*DAY)return;
        int stage;
        String stageText;
        if(remaining<=5*60*60_000L){stage=5;stageText="進入最後 5 小時";}
        else if(remaining<=DAY){stage=4;stageText="剩最後 1 天";}
        else if(remaining<=2*DAY){stage=3;stageText="剩 2 天";}
        else if(remaining<=3*DAY){stage=2;stageText="剩 3 天";}
        else {stage=1;stageText="進入 7 天提醒範圍";}
        try{
            // Deduplicate per credit and per milestone, across restarts and JobScheduler retries.
            byte[] digest=MessageDigest.getInstance("SHA-256")
                    .digest(nearest.id.getBytes(StandardCharsets.UTF_8));
            StringBuilder key=new StringBuilder("banked_expiry_");
            for(int i=0;i<8;i++)key.append(String.format(Locale.ROOT,"%02x",digest[i]&255));
            SharedPreferences prefs=Scheduler.prefs(context);
            if(prefs.getInt(key.toString(),0)>=stage)return;
            String ntfy=Scheduler.ntfyUrl(context);
            if(!AndroidNotifications.enabled(context)&&ntfy.isEmpty())return;
            prefs.edit().putInt(key.toString(),stage).apply();
            String message=stageText+" · 到期 "+nearest.expiresAt;
            Scheduler.note(context,"Banked Reset "+message);
            AndroidNotifications.post(context,"Banked Reset 即將到期",message);
            if(!ntfy.isEmpty()){
                try{new CodexClient(context).sendNtfy(ntfy,"Banked Reset 即將到期",message);}
                catch(Exception e){Scheduler.note(context,"Banked Reset ntfy 通知失敗: "+e.getClass().getSimpleName());}
            }
        }catch(Exception e){
            Scheduler.note(context,"Banked Reset 到期提醒失敗: "+e.getClass().getSimpleName());
        }
    }
}
