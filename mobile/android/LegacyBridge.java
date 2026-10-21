package com.christorng.androidcodextools;

import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.net.Uri;
import android.util.Log;
import org.json.JSONArray;
import org.json.JSONObject;
import java.util.Map;

/**
 * Android-only interoperability layer. The original SecureStore, Scheduler,
 * ScheduleConfig, receivers and jobs are compiled from the pinned legacy source.
 * Never clears the original preferences as part of migration.
 */
public final class LegacyBridge {
    private static final String TAG="CodexCreditsMigration";
    private LegacyBridge(){}

    public static String exportLegacy(Context c){
        try {
            SharedPreferences p=c.getSharedPreferences("app",Context.MODE_PRIVATE);
            JSONObject prefs=new JSONObject();
            for(Map.Entry<String,?> item:p.getAll().entrySet()){
                Object value=item.getValue();
                if(value instanceof String || value instanceof Number || value instanceof Boolean)
                    prefs.put(item.getKey(),value);
            }
            JSONObject tokens=new JSONObject();
            SecureStore secure=new SecureStore(c);
            for(String key:new String[]{"access_token","refresh_token","account_id","expires_at_ms"}){
                String v=secure.get(key);
                if(v!=null)tokens.put(key,v);
            }
            return new JSONObject().put("prefs",prefs).put("tokens",tokens).toString();
        }catch(Exception e){
            Log.e(TAG,"exportLegacy failed",e);
            throw new IllegalStateException("Legacy migration cannot read previous data",e);
        }
    }

    public static String readCredential(Context c,String key){
        return new SecureStore(c).get("cc2_"+key);
    }
    public static String writeCredential(Context c,String key,String data){
        try { new SecureStore(c).put("cc2_"+key,data); return "ok"; }
        catch(Exception e){throw new IllegalStateException("Secure credential write failed",e);}
    }
    public static String deleteCredential(Context c,String key){
        try { new SecureStore(c).put("cc2_"+key,null); return "ok"; }
        catch(Exception e){throw new IllegalStateException("Secure credential removal failed",e);}
    }
    public static String syncAuth(Context c,String data){
        try{
            JSONObject value=new JSONObject(data);
            SecureStore secure=new SecureStore(c);
            secure.put("access_token",value.getString("access_token"));
            if(value.has("refresh_token")&&!value.isNull("refresh_token"))
                secure.put("refresh_token",value.getString("refresh_token"));
            secure.put("account_id",value.getString("account_id"));
            secure.put("expires_at_ms",Long.toString(value.getLong("expires_at")*1000L));
            // A first-time login may happen after MainActivity.onCreate. Without this,
            // the native sampling job remains absent until the next app restart.
            Scheduler.scheduleQuotaSampling(c);
            return "ok";
        }catch(Exception e){throw new IllegalStateException("Legacy auth synchronization failed",e);}
    }
    public static String clearAuth(Context c){
        try{
            SecureStore s=new SecureStore(c);
            for(String key:new String[]{"access_token","refresh_token","account_id","expires_at_ms"})
                s.put(key,null);
            return "ok";
        }catch(Exception e){throw new IllegalStateException("Legacy auth removal failed",e);}
    }
    public static String openUrl(Context c,String url){
        Intent intent=new Intent(Intent.ACTION_VIEW,Uri.parse(url));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        c.startActivity(intent);
        return "ok";
    }
    public static String syncSettings(Context c,String json){
        try {
            JSONObject s=new JSONObject(json);
            SharedPreferences p=c.getSharedPreferences("app",Context.MODE_PRIVATE);
            SharedPreferences.Editor e=p.edit();
            String mode=s.optString("mode","off");
            String legacyMode="auto".equals(mode)?"auto_reset":mode;
            if(!"off".equals(legacyMode)&&!"auto_reset".equals(legacyMode)&&!"custom".equals(legacyMode))
                throw new IllegalArgumentException("Unsupported scheduling mode");
            e.putString("scheduler_mode",legacyMode);
            e.remove("scheduler_enabled");
            e.putInt("sample_interval_minutes",s.optInt("sampling_minutes",10));
            e.putString("theme_mode",s.optString("theme","system"));
            e.putBoolean("android_notifications_enabled",s.optBoolean("native_notify_enabled",true));
            e.putString("ntfy_url",s.optString("ntfy_topic",""));
            e.putString("schedule_weekly_v2",convertRules(s.optJSONArray("rules")).toString());

            JSONObject converted=new JSONObject();
            JSONObject overrides=s.optJSONObject("overrides");
            if(overrides!=null){
                for(java.util.Iterator<String> dates=overrides.keys();dates.hasNext();){
                    String date=dates.next();
                    JSONObject o=overrides.optJSONObject(date);
                    if(o!=null)converted.put(date,new JSONObject()
                        .put("enabled",o.optBoolean("enabled"))
                        .put("rules",convertRules(o.optJSONArray("rules"))));
                }
            }
            e.putString("schedule_day_overrides_v2",converted.toString());
            if(!e.commit())throw new IllegalStateException("SharedPreferences commit failed");
            Scheduler.setMode(c,legacyMode);
            Scheduler.scheduleQuotaSampling(c);
            return "ok";
        }catch(Exception ex){throw new IllegalStateException("Legacy scheduler synchronization failed",ex);}
    }
    private static JSONArray convertRules(JSONArray rules){
        JSONArray out=new JSONArray();
        if(rules==null)return out;
        for(int i=0;i<rules.length();i++){
            JSONObject r=rules.optJSONObject(i);
            if(r==null)continue;
            JSONArray days=r.optJSONArray("days");
            StringBuilder bits=new StringBuilder();
            for(int d=0;d<7;d++)bits.append(days==null || days.optBoolean(d,true)?'1':'0');
            try{
                out.put(new JSONObject().put("id",r.optString("id",""))
                    .put("enabled",r.optBoolean("enabled",true))
                    .put("hour",r.optInt("hour",6))
                    .put("minute",r.optInt("minute",0))
                    .put("autoCount",r.optInt("auto_count",0))
                    .put("days",bits.toString()));
            }catch(Exception ignored){}
        }
        return out;
    }
    public static String platformStatus(Context c){
        try{
            android.os.PowerManager power=(android.os.PowerManager)
                c.getSystemService(Context.POWER_SERVICE);
            boolean exempt=power!=null && power.isIgnoringBatteryOptimizations(c.getPackageName());
            return new JSONObject()
                .put("exact_alarm",Scheduler.canExact(c))
                .put("battery_exempt",exempt)
                .put("next_alarm_ms",Scheduler.next(c))
                .put("next_alarm_reason",Scheduler.nextReason(c))
                .put("next_event_kind",Scheduler.prefs(c).getString("next_event_kind","auto"))
                .put("next_event_auto_count",Scheduler.prefs(c).getInt("next_event_auto_count",0))
                .toString();
        }catch(Exception e){throw new IllegalStateException("Android platform state unavailable",e);}
    }
    public static String openSystemSettings(Context c,String kind){
        Intent i;
        switch(kind){
            case "exact": i=Scheduler.exactSettings(c); break;
            case "battery": i=new Intent(android.provider.Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS); break;
            case "app": i=new Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                Uri.parse("package:"+c.getPackageName())); break;
            default: throw new IllegalArgumentException("Unsupported settings destination");
        }
        i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        c.startActivity(i);
        return "ok";
    }
    public static String armBanked(Context c,String action){
        if("arm".equals(action))Scheduler.armBankedReset(c);
        else if("cancel".equals(action))Scheduler.cancelArmedBankedReset(c);
        return "ok";
    }
    public static String bankedArmed(Context c){
        return Boolean.toString(Scheduler.isBankedResetArmed(c));
    }
}