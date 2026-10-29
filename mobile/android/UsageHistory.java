package com.christorng.androidcodextools;

import android.content.Context;
import android.content.SharedPreferences;

import org.json.JSONArray;
import org.json.JSONObject;

import java.time.Instant;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

final class UsageHistory {
    private static final String KEY_SAMPLES="usage_samples_v1";
    private static final String KEY_EVENTS="usage_history_events_v1";
    private static final long KEEP_MS=8L*24*60*60_000L;
    private static final String RESET_API="https://codex-reset.com/api/timeline";
    private static final String RESET_FORECAST_API="https://codex-reset.com/api/forecast";
    private static final String KEY_GLOBAL_STATS="global_reset_stats_v1";
    private static final String KEY_GLOBAL_STATS_AT="global_reset_stats_at_v1";

    static final class Sample {
        final long at;
        final double primaryUsed,weeklyUsed;
        final boolean primaryConfirmed;
        final long primaryReset,weeklyReset;
        final int bankedCount;

        Sample(long at,double p,double w,boolean confirmed,long pr,long wr,int banked){
            this.at=at;primaryUsed=p;weeklyUsed=w;primaryConfirmed=confirmed;
            primaryReset=pr;weeklyReset=wr;bankedCount=banked;
        }
    }

    static final class GlobalResetStats {
        final long lastResetAt;
        final double ageDays,avgDays,medianDays;
        final int p24,p48;
        final long medianEstimateAt;
        GlobalResetStats(long last,double age,double avg,double median,int p24,int p48,long estimate){
            this.lastResetAt=last;this.ageDays=age;this.avgDays=avg;this.medianDays=median;
            this.p24=p24;this.p48=p48;this.medianEstimateAt=estimate;
        }
    }

    static final class Event {
        final long at;
        final String type,label,source;
        Event(long at,String type,String label,String source){
            this.at=at;this.type=type;this.label=label;this.source=source;
        }
    }

    private UsageHistory(){}

    static synchronized void recordSnapshot(Context c,Scheduler.Snapshot s){
        long now=System.currentTimeMillis();
        List<Sample> samples=loadSamples(c);
        Sample prev=samples.isEmpty()?null:samples.get(samples.size()-1);

        Sample cur=new Sample(now,
                s.primaryActive?s.primaryUsed:0,
                s.secondaryActive?s.secondaryUsed:0,
                s.primaryConfirmed,
                s.primaryResetMs,
                s.secondaryResetMs,
                s.resetAvailableCount);

        if(prev!=null && now-prev.at<30_000L){
            samples.set(samples.size()-1,cur);
            prev=samples.size()>=2?samples.get(samples.size()-2):null;
        }else{
            samples.add(cur);
        }
        pruneSamples(samples,now);
        saveSamples(c,samples);

        if(prev!=null)detectTransitions(c,prev,cur);
    }

    static synchronized void addEvent(Context c,String type,String label,String source){
        List<Event> events=loadEvents(c);
        long now=System.currentTimeMillis();
        if(!events.isEmpty()){
            Event last=events.get(events.size()-1);
            if(Math.abs(now-last.at)<120_000L && last.type.equals(type) && last.label.equals(label))return;
        }
        events.add(new Event(now,type,label,source==null?"":source));
        long min=now-KEEP_MS;
        events.removeIf(e->e.at<min);
        saveEvents(c,events);
    }

    static GlobalResetStats loadGlobalResetStats(Context c){
        try{
            String raw=prefs(c).getString(KEY_GLOBAL_STATS,"");
            if(raw==null||raw.isEmpty())return null;
            JSONObject o=new JSONObject(raw);
            return new GlobalResetStats(
                    o.optLong("last",0),
                    o.optDouble("age",0),
                    o.optDouble("avg",0),
                    o.optDouble("median",0),
                    o.optInt("p24",-1),
                    o.optInt("p48",-1),
                    o.optLong("estimate",0));
        }catch(Exception e){
            return null;
        }
    }

    static void refreshGlobalResetStats(Context c,boolean force){
        long now=System.currentTimeMillis();
        long lastCheck=prefs(c).getLong(KEY_GLOBAL_STATS_AT,0);
        if(!force&&now-lastCheck<6*60*60_000L)return;
        try{
            CodexClient cli=new CodexClient(c);
            String forecastRaw=cli.getPublicJson(
                    RESET_FORECAST_API,
                    "Codex-Tools/0.1 (+https://github.com/ChrisTorng/android-codex-tools)");
            JSONObject forecast=new JSONObject(forecastRaw);

            long lastReset=0;
            String lastResetText=forecast.optString("last_reset_at","");
            if(!lastResetText.isEmpty())lastReset=Instant.parse(lastResetText).toEpochMilli();
            double age=forecast.optDouble("age_days",
                    lastReset>0?(now-lastReset)/(24d*60*60_000d):0);

            int p24=-1,p48=-1;
            JSONObject probs=forecast.optJSONObject("probabilities");
            if(probs!=null){
                p24=probs.optInt("rounded_24h",-1);
                p48=probs.optInt("rounded_48h",-1);
            }

            String timelineRaw=cli.getPublicJson(
                    RESET_API,
                    "Codex-Tools/0.1 (+https://github.com/ChrisTorng/android-codex-tools)");
            JSONObject timeline=new JSONObject(timelineRaw);
            JSONArray arr=timeline.optJSONArray("events");
            List<Long> resets=new ArrayList<>();
            if(arr!=null){
                for(int i=0;i<arr.length();i++){
                    JSONObject o=arr.optJSONObject(i);
                    if(o==null)continue;
                    if(!"reset".equalsIgnoreCase(o.optString("group","")))continue;
                    if(!"announced".equalsIgnoreCase(o.optString("announcement_state","")))continue;
                    String ts=o.optString("effective_at","");
                    if(ts.isEmpty())ts=o.optString("announced_at","");
                    if(ts.isEmpty())continue;
                    try{resets.add(Instant.parse(ts).toEpochMilli());}catch(Exception ignored){}
                }
            }
            resets.sort(Long::compareTo);
            if(lastReset<=0&&!resets.isEmpty())lastReset=resets.get(resets.size()-1);

            List<Double> gaps=new ArrayList<>();
            // Use recent verified resets only so the estimate adapts to current cadence.
            int start=Math.max(1,resets.size()-10);
            for(int i=start;i<resets.size();i++){
                long gap=resets.get(i)-resets.get(i-1);
                if(gap>0)gaps.add(gap/(24d*60*60_000d));
            }
            double avg=0,median=0;
            if(!gaps.isEmpty()){
                for(double g:gaps)avg+=g;
                avg/=gaps.size();
                gaps.sort(Double::compareTo);
                int n=gaps.size();
                median=n%2==1?gaps.get(n/2):(gaps.get(n/2-1)+gaps.get(n/2))/2d;
            }
            long estimate=lastReset>0&&median>0?lastReset+(long)(median*24d*60*60_000d):0;

            JSONObject out=new JSONObject()
                    .put("last",lastReset)
                    .put("age",age)
                    .put("avg",avg)
                    .put("median",median)
                    .put("p24",p24)
                    .put("p48",p48)
                    .put("estimate",estimate);
            prefs(c).edit()
                    .putString(KEY_GLOBAL_STATS,out.toString())
                    .putLong(KEY_GLOBAL_STATS_AT,now)
                    .apply();
        }catch(Exception ignored){
            prefs(c).edit().putLong(KEY_GLOBAL_STATS_AT,now).apply();
        }
    }

    static List<Sample> loadSamples(Context c){
        List<Sample> out=new ArrayList<>();
        String raw=prefs(c).getString(KEY_SAMPLES,"[]");
        try{
            JSONArray a=new JSONArray(raw);
            for(int i=0;i<a.length();i++){
                JSONObject o=a.optJSONObject(i);
                if(o==null)continue;
                out.add(new Sample(
                        o.optLong("at",0),
                        o.optDouble("p",0),
                        o.optDouble("w",0),
                        o.optBoolean("pc",false),
                        o.optLong("pr",0),
                        o.optLong("wr",0),
                        o.optInt("b",0)));
            }
        }catch(Exception ignored){}
        out.sort(Comparator.comparingLong(s->s.at));
        return out;
    }

    static List<Event> loadEvents(Context c){
        List<Event> out=new ArrayList<>();
        String raw=prefs(c).getString(KEY_EVENTS,"[]");
        try{
            JSONArray a=new JSONArray(raw);
            for(int i=0;i<a.length();i++){
                JSONObject o=a.optJSONObject(i);
                if(o==null)continue;
                out.add(new Event(
                        o.optLong("at",0),
                        o.optString("type","event"),
                        o.optString("label",""),
                        o.optString("source","")));
            }
        }catch(Exception ignored){}
        out.sort(Comparator.comparingLong(e->e.at));
        return out;
    }

    private static void detectTransitions(Context c,Sample prev,Sample cur){
        boolean bankedDown=cur.bankedCount<prev.bankedCount;
        boolean weeklyDropped=cur.weeklyUsed+2.0<prev.weeklyUsed;
        boolean primaryDropped=cur.primaryUsed+2.0<prev.primaryUsed;
        boolean weeklyResetShift=Math.abs(cur.weeklyReset-prev.weeklyReset)>60*60_000L &&
                cur.weeklyReset>0 && prev.weeklyReset>0;

        if(cur.bankedCount>prev.bankedCount){
            addEvent(c,"banked_grant",
                    "Banked Reset +"+(cur.bankedCount-prev.bankedCount),
                    "OpenAI account");
        }

        if(bankedDown && (weeklyDropped||primaryDropped||weeklyResetShift)){
            addEvent(c,"banked_reset",
                    "推測已使用 Banked Reset（可用次數下降）",
                    "OpenAI account");
            return;
        }

        boolean naturalWeekly=prev.weeklyReset>0 &&
                cur.at>=prev.weeklyReset-15*60_000L &&
                cur.at<=prev.weeklyReset+60*60_000L;

        if(weeklyDropped && !naturalWeekly){
            Event confirmed=findRecentPublicGlobalReset(c,cur.at);
            if(confirmed!=null){
                addEvent(c,"global_reset",confirmed.label,"codex-reset.com");
            }else{
                addEvent(c,"unexpected_reset",
                        "疑似外部／未確認 Reset（週用量不預期下降）",
                        "本機推論");
            }
        }else if(naturalWeekly && weeklyDropped){
            addEvent(c,"weekly_reset","週配額自然重置","OpenAI quota");
        }
    }

    private static Event findRecentPublicGlobalReset(Context c,long around){
        try{
            String raw=new CodexClient(c).getPublicJson(
                    RESET_API,
                    "Codex-Tools/0.1 (+https://github.com/ChrisTorng/android-codex-tools)");
            JSONObject root=new JSONObject(raw);
            JSONArray events=root.optJSONArray("events");
            if(events==null)return null;

            long bestDiff=Long.MAX_VALUE;
            Event best=null;
            for(int i=0;i<events.length();i++){
                JSONObject o=events.optJSONObject(i);
                if(o==null)continue;
                if(!"reset".equalsIgnoreCase(o.optString("group","")))continue;

                String state=o.optString("announcement_state","");
                if(!"announced".equalsIgnoreCase(state))continue;
                // The public timeline frequently confirms real resets as
                // operator-observed/medium, not just archive/high. Accept actual
                // observed resets, but never preliminary announcements.
                boolean observed="reset_observed".equalsIgnoreCase(
                        o.optString("observation_result",""));
                boolean archived="archive".equalsIgnoreCase(o.optString("source","")) &&
                        "high".equalsIgnoreCase(o.optString("confidence",""));
                if(!o.optBoolean("confirmed",false) || !(observed||archived))continue;

                String ts=o.optString("effective_at","");
                if(ts.isEmpty()||"null".equals(ts))ts=o.optString("observed_at","");
                if(ts.isEmpty()||"null".equals(ts))ts=o.optString("announced_at","");
                if(ts.isEmpty())continue;

                long at=Instant.parse(ts).toEpochMilli();
                long delay=around-at;
                // Local sampling commonly discovers the reset 10-15m afterward.
                // Permit slight announcement lag and bounded propagation time.
                if(delay>=-15L*60_000L && delay<=3L*60*60_000L &&
                        Math.abs(delay)<bestDiff){
                    bestDiff=Math.abs(delay);
                    best=new Event(at,"global_reset",
                            "Global Reset（公開紀錄確認）",
                            "codex-reset.com");
                }
            }
            return best;
        }catch(Exception ignored){
            return null;
        }
    }

    private static void pruneSamples(List<Sample> samples,long now){
        long min=now-KEEP_MS;
        samples.removeIf(s->s.at<min);
        if(samples.size()>1600){
            samples.subList(0,samples.size()-1600).clear();
        }
    }

    private static void saveSamples(Context c,List<Sample> samples){
        JSONArray a=new JSONArray();
        for(Sample s:samples){
            try{
                a.put(new JSONObject()
                        .put("at",s.at)
                        .put("p",s.primaryUsed)
                        .put("w",s.weeklyUsed)
                        .put("pc",s.primaryConfirmed)
                        .put("pr",s.primaryReset)
                        .put("wr",s.weeklyReset)
                        .put("b",s.bankedCount));
            }catch(Exception ignored){}
        }
        prefs(c).edit().putString(KEY_SAMPLES,a.toString()).apply();
    }

    private static void saveEvents(Context c,List<Event> events){
        JSONArray a=new JSONArray();
        for(Event e:events){
            try{
                a.put(new JSONObject()
                        .put("at",e.at)
                        .put("type",e.type)
                        .put("label",e.label)
                        .put("source",e.source));
            }catch(Exception ignored){}
        }
        prefs(c).edit().putString(KEY_EVENTS,a.toString()).apply();
    }

    private static SharedPreferences prefs(Context c){
        return Scheduler.prefs(c);
    }
}
