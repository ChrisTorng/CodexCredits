package com.christorng.androidcodextools;

import android.content.Context;

import org.json.JSONArray;
import org.json.JSONObject;

import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalTime;
import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.UUID;
import java.util.LinkedHashSet;
import java.util.Set;

final class ScheduleConfig {
    static final String KEY_WEEKLY="schedule_weekly_v2";
    static final String KEY_DAY_OVERRIDES="schedule_day_overrides_v2";

    static final class Rule {
        String id=UUID.randomUUID().toString();
        boolean enabled=true;
        final boolean[] days={true,true,true,true,true,true,true};
        int hour=6;
        int minute=0;
        int autoCount=0;

        Rule copy(){
            Rule r=new Rule();
            r.id=id;
            r.enabled=enabled;
            System.arraycopy(days,0,r.days,0,7);
            r.hour=hour;
            r.minute=minute;
            r.autoCount=autoCount;
            return r;
        }

        String timeText(){
            return String.format(Locale.TAIWAN,"%02d:%02d",hour,minute);
        }
    }

    static final class DayOverride {
        final LocalDate date;
        boolean enabled=false; // false = use weekly default
        final List<Rule> rules=new ArrayList<>();
        DayOverride(LocalDate d){date=d;}
    }

    static final class Slot {
        final long whenMs;
        final int autoCount;
        final String source;
        Slot(long whenMs,int autoCount,String source){
            this.whenMs=whenMs;
            this.autoCount=autoCount;
            this.source=source;
        }
    }

    static List<Rule> loadWeekly(Context c){
        String raw=Scheduler.prefs(c).getString(KEY_WEEKLY,null);
        if(raw!=null){
            return parseRules(raw,true);
        }

        String legacy=Scheduler.prefs(c).getString("schedule_rules_v1",null);
        if(legacy!=null){
            List<Rule> migrated=parseRules(legacy,true);
            if(!migrated.isEmpty())return migrated;
        }
        return defaultDailyAnchors();
    }

    private static List<Rule> defaultDailyAnchors(){
        List<Rule> out=new ArrayList<>();
        Rule r=new Rule();
        r.hour=6;
        r.minute=0;
        r.autoCount=3;
        for(int i=0;i<7;i++)r.days[i]=true;
        out.add(r);
        return out;
    }

    static void saveWeekly(Context c,List<Rule> rules){
        Scheduler.prefs(c).edit().putString(KEY_WEEKLY,serializeRules(rules,true).toString()).apply();
    }

    static DayOverride loadOverride(Context c,LocalDate date){
        DayOverride out=new DayOverride(date);
        try{
            JSONObject all=new JSONObject(Scheduler.prefs(c).getString(KEY_DAY_OVERRIDES,"{}"));
            JSONObject o=all.optJSONObject(date.toString());
            if(o==null)return out;
            out.enabled=o.optBoolean("enabled",false);
            JSONArray a=o.optJSONArray("rules");
            if(a!=null)out.rules.addAll(parseRules(a.toString(),false));
        }catch(Exception ignored){}
        return out;
    }

    static void saveOverride(Context c,DayOverride override){
        try{
            JSONObject all=new JSONObject(Scheduler.prefs(c).getString(KEY_DAY_OVERRIDES,"{}"));
            JSONObject o=new JSONObject();
            o.put("enabled",override.enabled);
            o.put("rules",serializeRules(override.rules,false));
            all.put(override.date.toString(),o);

            // Keep only today/tomorrow/future-nearby entries; old dates are irrelevant.
            LocalDate today=LocalDate.now();
            List<String> remove=new ArrayList<>();
            java.util.Iterator<String> keys=all.keys();
            while(keys.hasNext()){
                String k=keys.next();
                try{
                    LocalDate d=LocalDate.parse(k);
                    if(d.isBefore(today.minusDays(1))||d.isAfter(today.plusDays(7)))remove.add(k);
                }catch(Exception e){remove.add(k);}
            }
            for(String k:remove)all.remove(k);
            Scheduler.prefs(c).edit().putString(KEY_DAY_OVERRIDES,all.toString()).apply();
        }catch(Exception ignored){}
    }

    static long nextAnchorMs(Context c,long afterMs){
        Slot s=nextAnchor(c,afterMs);
        return s==null?0:s.whenMs;
    }

    static Slot nextAnchor(Context c,long afterMs){
        ZoneId zone=ZoneId.systemDefault();
        ZonedDateTime after=Instant.ofEpochMilli(afterMs).atZone(zone);
        LocalDate start=after.toLocalDate();

        for(int offset=0;offset<=370;offset++){
            LocalDate date=start.plusDays(offset);
            List<Rule> rules=rulesForDate(c,date);
            List<Rule> sorted=new ArrayList<>(rules);
            sorted.sort(Comparator.comparingInt((Rule r)->r.hour).thenComparingInt(r->r.minute));
            for(Rule r:sorted){
                if(!r.enabled)continue;
                ZonedDateTime z=date.atTime(LocalTime.of(r.hour,r.minute)).atZone(zone);
                long when=z.toInstant().toEpochMilli();
                if(when>afterMs){
                    return new Slot(when,r.autoCount,date.equals(LocalDate.now())?"今天":
                            date.equals(LocalDate.now().plusDays(1))?"明天":"每週");
                }
            }
        }
        return null;
    }

    static List<Rule> rulesForDate(Context c,LocalDate date){
        DayOverride ov=loadOverride(c,date);
        if(ov.enabled)return copyList(ov.rules);

        int day=date.getDayOfWeek().getValue()-1;
        List<Rule> out=new ArrayList<>();
        for(Rule r:loadWeekly(c)){
            if(r.enabled&&r.days[day])out.add(r.copy());
        }
        return out;
    }

    static int maxSafeAutoCount(Rule target,List<Rule> rules,boolean weekly){
        if(target==null||!target.enabled)return 0;
        int minGapMinutes=Integer.MAX_VALUE;

        if(weekly){
            List<Integer> anchors=new ArrayList<>();
            for(Rule r:rules){
                if(!r.enabled)continue;
                for(int d=0;d<7;d++){
                    if(r.days[d])anchors.add(d*1440+r.hour*60+r.minute);
                }
            }
            int week=7*1440;
            for(int d=0;d<7;d++){
                if(!target.days[d])continue;
                int t=d*1440+target.hour*60+target.minute;
                int next=Integer.MAX_VALUE;
                for(int a:anchors){
                    int delta=a-t;
                    if(delta<=0)delta+=week;
                    if(delta<next)next=delta;
                }
                if(next<minGapMinutes)minGapMinutes=next;
            }
        }else{
            List<Integer> anchors=new ArrayList<>();
            for(Rule r:rules){
                if(r.enabled)anchors.add(r.hour*60+r.minute);
            }
            int t=target.hour*60+target.minute;
            int next=Integer.MAX_VALUE;
            for(int a:anchors){
                int delta=a-t;
                if(delta<=0)delta+=1440;
                if(delta<next)next=delta;
            }
            minGapMinutes=next;
        }

        if(minGapMinutes==Integer.MAX_VALUE)return 3;
        // Each continuation starts 5h later and creates another 5h window.
        // An anchor exactly at the previous window's reset is allowed because
        // runtime logic can wait through the small reset tolerance.
        return Math.max(0,Math.min(6,(minGapMinutes/300)-1));
    }

    static Rule suggestedNewRule(List<Rule> rules,boolean weekly){
        Rule candidate=new Rule();
        candidate.autoCount=0;
        for(int i=0;i<7;i++)candidate.days[i]=true;

        if(rules==null||rules.isEmpty()){
            candidate.hour=6;
            candidate.minute=0;
            return candidate;
        }

        int latest=-1;
        for(Rule r:rules){
            if(!r.enabled)continue;
            int base=r.hour*60+r.minute;
            for(int i=0;i<=r.autoCount;i++){
                int t=(base+i*300)%1440;
                if(t>latest)latest=t;
            }
        }
        int first=latest<0?360:(latest+300)%1440;

        // Search from the end of the existing sequence, in 5-minute steps.
        for(int step=0;step<288;step++){
            int minute=(first+step*5)%1440;
            int h=minute/60, m=minute%60;
            if(isFeasibleStart(h,m,rules,weekly)){
                candidate.hour=h;
                candidate.minute=m;
                return candidate;
            }
        }
        return null;
    }

    private static boolean isFeasibleStart(int hour,int minute,List<Rule> rules,boolean weekly){
        if(rules==null||rules.isEmpty())return true;
        int period=weekly?7*1440:1440;
        List<Integer> starts=new ArrayList<>();

        if(weekly){
            for(Rule r:rules){
                if(!r.enabled)continue;
                for(int d=0;d<7;d++){
                    if(!r.days[d])continue;
                    int base=d*1440+r.hour*60+r.minute;
                    for(int i=0;i<=r.autoCount;i++){
                        starts.add((base+i*300)%period);
                    }
                }
            }
            for(int d=0;d<7;d++){
                int cand=d*1440+hour*60+minute;
                for(int s:starts){
                    int diff=Math.abs(cand-s);
                    diff=Math.min(diff,period-diff);
                    if(diff<300)return false;
                }
            }
        }else{
            for(Rule r:rules){
                if(!r.enabled)continue;
                int base=r.hour*60+r.minute;
                for(int i=0;i<=r.autoCount;i++){
                    starts.add((base+i*300)%period);
                }
            }
            int cand=hour*60+minute;
            for(int s:starts){
                int diff=Math.abs(cand-s);
                diff=Math.min(diff,period-diff);
                if(diff<300)return false;
            }
        }
        return true;
    }

    static Set<String> conflictingRuleIds(List<Rule> rules,boolean weekly){
        Set<String> conflicts=new LinkedHashSet<>();
        if(rules==null||rules.isEmpty())return conflicts;
        for(Rule r:rules){
            int max=maxSafeAutoCount(r,rules,weekly);
            if(r.autoCount>max)conflicts.add(r.id);
        }

        class Occurrence {
            final Rule rule;
            final int minute;
            Occurrence(Rule r,int m){rule=r;minute=m;}
        }
        List<Occurrence> occurrences=new ArrayList<>();
        int period=weekly?7*1440:1440;
        for(Rule r:rules){
            if(!r.enabled)continue;
            if(weekly){
                for(int d=0;d<7;d++){
                    if(!r.days[d])continue;
                    int base=d*1440+r.hour*60+r.minute;
                    for(int i=0;i<=r.autoCount;i++)
                        occurrences.add(new Occurrence(r,(base+i*300)%period));
                }
            }else{
                int base=r.hour*60+r.minute;
                for(int i=0;i<=r.autoCount;i++)
                    occurrences.add(new Occurrence(r,(base+i*300)%period));
            }
        }
        for(int i=0;i<occurrences.size();i++){
            for(int j=i+1;j<occurrences.size();j++){
                Occurrence a=occurrences.get(i),b=occurrences.get(j);
                if(a.rule.id.equals(b.rule.id))continue;
                int diff=Math.abs(a.minute-b.minute);
                diff=Math.min(diff,period-diff);
                if(diff<300){
                    conflicts.add(a.rule.id);
                    conflicts.add(b.rule.id);
                }
            }
        }
        return conflicts;
    }

    static String estimateRule(Rule r){
        StringBuilder b=new StringBuilder(r.timeText());
        LocalTime t=LocalTime.of(r.hour,r.minute);
        for(int i=1;i<=r.autoCount;i++){
            t=t.plusHours(5);
            b.append(" → ").append(String.format(Locale.TAIWAN,"%02d:%02d",t.getHour(),t.getMinute()));
            if(t.getHour()<r.hour && i==1)b.append("(+1日)");
        }
        return b.toString();
    }

    static String weeklySummary(Rule r){
        String[] names={"一","二","三","四","五","六","日"};
        StringBuilder days=new StringBuilder();
        for(int i=0;i<7;i++){
            if(r.days[i]){
                if(days.length()>0)days.append(" ");
                days.append(names[i]);
            }
        }
        return (days.length()==0?"未選":days.toString())+"\n"+estimateRule(r);
    }

    private static List<Rule> parseRules(String raw,boolean weekly){
        List<Rule> out=new ArrayList<>();
        try{
            JSONArray a=new JSONArray(raw==null?"[]":raw);
            for(int i=0;i<a.length();i++){
                JSONObject o=a.getJSONObject(i);
                Rule r=new Rule();
                r.id=o.optString("id",UUID.randomUUID().toString());
                r.enabled=o.optBoolean("enabled",true);
                r.hour=o.optInt("hour",6);
                r.minute=o.optInt("minute",0);
                r.autoCount=Math.max(0,Math.min(8,o.optInt("autoCount",0)));
                String ds=o.optString("days",weekly?"1111111":"1111111");
                for(int d=0;d<7;d++)r.days[d]=d<ds.length()&&ds.charAt(d)=='1';
                out.add(r);
            }
        }catch(Exception ignored){}
        return out;
    }

    private static JSONArray serializeRules(List<Rule> rules,boolean weekly){
        JSONArray a=new JSONArray();
        for(Rule r:rules){
            try{
                JSONObject o=new JSONObject();
                o.put("id",r.id);
                o.put("enabled",r.enabled);
                o.put("hour",r.hour);
                o.put("minute",r.minute);
                o.put("autoCount",r.autoCount);
                StringBuilder ds=new StringBuilder();
                for(boolean d:r.days)ds.append(d?'1':'0');
                o.put("days",ds.toString());
                a.put(o);
            }catch(Exception ignored){}
        }
        return a;
    }

    private static List<Rule> copyList(List<Rule> rules){
        List<Rule> out=new ArrayList<>();
        for(Rule r:rules)out.add(r.copy());
        return out;
    }
}
