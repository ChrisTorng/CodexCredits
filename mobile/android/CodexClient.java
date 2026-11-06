package com.christorng.androidcodextools;

import android.content.Context;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.util.Base64;

import org.json.JSONArray;
import org.json.JSONObject;

import java.net.InetAddress;
import java.net.UnknownHostException;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.concurrent.TimeUnit;

import okhttp3.Dns;
import okhttp3.FormBody;
import okhttp3.HttpUrl;
import okhttp3.MediaType;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.ResponseBody;
import okhttp3.dnsoverhttps.DnsOverHttps;

final class CodexClient {
    static final String CLIENT_ID="app_EMoamEEZ73f0CkXaXp7hrann";
    static final String AUTH_URL="https://auth.openai.com/oauth/authorize";
    static final String TOKEN_URL="https://auth.openai.com/oauth/token";
    static final String USAGE_URL="https://chatgpt.com/backend-api/wham/usage";
    static final String RESPONSES_URL="https://chatgpt.com/backend-api/codex/responses";
    static final String RESET_CREDITS_URL="https://chatgpt.com/backend-api/wham/rate-limit-reset-credits";
    static final String RESET_CONSUME_URL="https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume";
    static final String SCOPE="openid profile email offline_access api.connectors.read api.connectors.invoke";

    private final SecureStore store;
    private final Context context;
    private final OkHttpClient http;

    CodexClient(Context c){
        context=c.getApplicationContext();
        store=new SecureStore(context);
        http=new OkHttpClient.Builder()
                .dns(buildDns(context))
                .connectTimeout(20, TimeUnit.SECONDS)
                .readTimeout(60, TimeUnit.SECONDS)
                .writeTimeout(20, TimeUnit.SECONDS)
                .build();
    }

    static final class Pkce {
        final String verifier,challenge,state;
        Pkce(String v,String c,String s){verifier=v;challenge=c;state=s;}
    }

    static final class Window {
        final double usedPercent; final long resetAt;
        Window(double u,long r){usedPercent=u;resetAt=r;}
    }

    static final class Quota {
        final String plan; final boolean allowed,limitReached; final Window primary,secondary;
        Quota(String p,boolean a,boolean l,Window pr,Window se){plan=p;allowed=a;limitReached=l;primary=pr;secondary=se;}
        String summary(){
            String p=primary==null?"5h: inactive":String.format(Locale.US,"5h used %.0f%%, reset %d",primary.usedPercent,primary.resetAt);
            String s=secondary==null?"Weekly: inactive":String.format(Locale.US,"Weekly used %.0f%%, reset %d",secondary.usedPercent,secondary.resetAt);
            return "Plan: "+plan+"\n"+p+"\n"+s+"\nAllowed: "+allowed;
        }
    }

    static final class ResetCredit {
        final String id,title,description,expiresAt;
        ResetCredit(String id,String title,String description,String expiresAt){
            this.id=id;this.title=title;this.description=description;this.expiresAt=expiresAt;
        }
    }

    static final class ResetCredits {
        final int availableCount;
        final List<ResetCredit> available;
        ResetCredits(int count,List<ResetCredit> available){
            this.availableCount=count;this.available=available;
        }
        ResetCredit first(){return available.isEmpty()?null:available.get(0);}
    }

    static final class ResetResult {
        final String code;
        final int windowsReset;
        ResetResult(String code,int windowsReset){this.code=code;this.windowsReset=windowsReset;}
    }

    static Pkce newPkce() throws Exception {
        SecureRandom r=new SecureRandom();
        byte[] vb=new byte[48]; r.nextBytes(vb);
        String v=Base64.encodeToString(vb,Base64.URL_SAFE|Base64.NO_WRAP|Base64.NO_PADDING);
        String c=Base64.encodeToString(MessageDigest.getInstance("SHA-256").digest(v.getBytes(StandardCharsets.US_ASCII)),
                Base64.URL_SAFE|Base64.NO_WRAP|Base64.NO_PADDING);
        byte[] sb=new byte[24]; r.nextBytes(sb);
        String s=Base64.encodeToString(sb,Base64.URL_SAFE|Base64.NO_WRAP|Base64.NO_PADDING);
        return new Pkce(v,c,s);
    }

    static String authorizeUrl(Pkce p,String redirect) throws Exception {
        return AUTH_URL+"?response_type=code&client_id="+enc(CLIENT_ID)+
                "&redirect_uri="+enc(redirect)+"&scope="+enc(SCOPE)+
                "&code_challenge="+enc(p.challenge)+"&code_challenge_method=S256&state="+enc(p.state)+
                "&id_token_add_organizations=true&codex_cli_simplified_flow=true&originator=codex_quota_trigger_android";
    }

    void exchangeCode(String code,String verifier,String redirect) throws Exception {
        FormBody form=new FormBody.Builder()
                .add("grant_type","authorization_code")
                .add("client_id",CLIENT_ID)
                .add("code",code)
                .add("redirect_uri",redirect)
                .add("code_verifier",verifier)
                .build();
        saveTokens(new JSONObject(postForm(TOKEN_URL,form)));
    }

    synchronized String validAccessToken() throws Exception {
        String a=store.get("access_token"), e=store.get("expires_at_ms");
        long exp=e==null?0:Long.parseLong(e);
        if(a!=null && System.currentTimeMillis()+300000L<exp)return a;
        refresh();
        a=store.get("access_token");
        if(a==null)throw new IllegalStateException("Not signed in");
        return a;
    }

    synchronized void refresh() throws Exception {
        String r=store.get("refresh_token");
        if(r==null)throw new IllegalStateException("No refresh token; sign in again");
        FormBody form=new FormBody.Builder()
                .add("grant_type","refresh_token")
                .add("client_id",CLIENT_ID)
                .add("refresh_token",r)
                .build();
        saveTokens(new JSONObject(postForm(TOKEN_URL,form)));
    }

    Quota getQuota() throws Exception {
        Request request=authHeaders(new Request.Builder().url(USAGE_URL))
                .get()
                .build();
        JSONObject root=new JSONObject(executeText(request));
        JSONObject rate=root.optJSONObject("rate_limit");
        return new Quota(root.optString("plan_type","unknown"),
                rate==null||rate.optBoolean("allowed",true),
                rate!=null&&rate.optBoolean("limit_reached",false),
                window(rate==null?null:rate.optJSONObject("primary_window")),
                window(rate==null?null:rate.optJSONObject("secondary_window")));
    }

    ResetCredits getResetCredits() throws Exception {
        Request request=authHeaders(new Request.Builder().url(RESET_CREDITS_URL))
                .get()
                .build();
        JSONObject root=new JSONObject(executeText(request));
        int count=root.optInt("available_count",0);
        List<ResetCredit> available=new ArrayList<>();
        JSONArray credits=root.optJSONArray("credits");
        if(credits!=null){
            for(int i=0;i<credits.length();i++){
                JSONObject o=credits.optJSONObject(i);
                if(o==null||!"available".equalsIgnoreCase(o.optString("status","")))continue;
                String id=o.optString("id","");
                if(id.isEmpty())continue;
                available.add(new ResetCredit(
                        id,
                        o.optString("title","Full reset"),
                        o.optString("description",""),
                        o.optString("expires_at","")));
            }
        }
        return new ResetCredits(count,available);
    }

    ResetResult consumeResetCredit(String creditId) throws Exception {
        JSONObject payload=new JSONObject();
        payload.put("redeem_request_id",java.util.UUID.randomUUID().toString());
        if(creditId!=null&&!creditId.isEmpty())payload.put("credit_id",creditId);
        RequestBody body=RequestBody.create(payload.toString(),MediaType.get("application/json; charset=utf-8"));
        Request request=authHeaders(new Request.Builder().url(RESET_CONSUME_URL))
                .header("Accept","application/json")
                .post(body)
                .build();
        JSONObject root=new JSONObject(executeText(request));
        return new ResetResult(root.optString("code","unknown"),root.optInt("windows_reset",0));
    }

    void sendNtfy(String topicUrl,String title,String message) throws Exception {
        HttpUrl topic=HttpUrl.get(topicUrl);
        List<String> segments=topic.pathSegments();
        String topicName="";
        for(int i=segments.size()-1;i>=0;i--){
            String s=segments.get(i);
            if(s!=null&&!s.isEmpty()){topicName=s;break;}
        }
        if(topicName.isEmpty())throw new IllegalArgumentException("ntfy topic URL 缺少 topic");

        HttpUrl root=topic.newBuilder().encodedPath("/").build();
        JSONObject payload=new JSONObject()
                .put("topic",topicName)
                .put("title",title)
                .put("message",message);
        RequestBody body=RequestBody.create(payload.toString(),MediaType.get("application/json; charset=utf-8"));
        Request request=new Request.Builder()
                .url(root)
                .header("Accept","application/json")
                .post(body)
                .build();
        executeText(request);
    }

    int triggerMinimal() throws Exception {
        JSONObject root=new JSONObject();
        root.put("model","gpt-5.6-luna").put("store",false).put("stream",true).put("instructions","Reply .");
        JSONObject txt=new JSONObject().put("type","input_text").put("text",".");
        root.put("input",new JSONArray().put(new JSONObject().put("role","user").put("content",new JSONArray().put(txt))));
        root.put("reasoning",new JSONObject().put("effort","none"));
        root.put("text",new JSONObject().put("verbosity","low"));

        RequestBody body=RequestBody.create(root.toString(), MediaType.get("application/json; charset=utf-8"));
        Request request=authHeaders(new Request.Builder().url(RESPONSES_URL))
                .header("originator","codex-quota-trigger-android")
                .header("OpenAI-Beta","responses=experimental")
                .header("Accept","text/event-stream")
                .post(body)
                .build();

        try(Response response=http.newCall(request).execute()){
            String text=bodyText(response.body());
            int status=response.code();
            if(status<200||status>=300)throw new IllegalStateException("Trigger HTTP "+status+": "+text);
            // A 200 response only confirms the SSE stream was accepted.
            // Keep the entire stream alive; require the terminal event before
            // reporting a completed background trigger.
            boolean finished=false;
            for(String row:text.split("\\r?\\n")){
                String event=row.trim();
                if(!event.startsWith("data:"))continue;
                event=event.substring(5).trim();
                try{
                    String type=new JSONObject(event).optString("type","");
                    if("response.completed".equals(type))finished=true;
                    if("response.failed".equals(type)||"response.incomplete".equals(type)||"error".equals(type))
                        throw new IllegalStateException("Codex SSE ended with "+type);
                }catch(org.json.JSONException ignored){}
            }
            if(!finished)throw new IllegalStateException("Codex SSE did not finish with response.completed");
            return status;
        }
    }

    boolean signedIn(){ return store.get("refresh_token")!=null && store.get("account_id")!=null; }
    void signOut(){store.clear();}

    private Request.Builder authHeaders(Request.Builder b) throws Exception {
        return b.header("Authorization","Bearer "+validAccessToken())
                .header("ChatGPT-Account-Id",store.get("account_id"))
                .header("User-Agent","android-codex-tools/0.1");
    }

    private void saveTokens(JSONObject j) throws Exception {
        String a=j.optString("access_token",null), r=j.optString("refresh_token",null), id=j.optString("id_token",null);
        if(a!=null)store.put("access_token",a);
        if(r!=null)store.put("refresh_token",r);
        if(id!=null){
            store.put("id_token",id);
            String account=extractAccountId(id);
            if(account!=null)store.put("account_id",account);
        }
        store.put("expires_at_ms",Long.toString(System.currentTimeMillis()+j.optLong("expires_in",3600)*1000L));
        if(store.get("account_id")==null)throw new IllegalStateException("ChatGPT account id not found in token");
    }

    private static String extractAccountId(String jwt){
        try{
            String[] p=jwt.split("\\.");
            String payload=new String(Base64.decode(p[1],Base64.URL_SAFE|Base64.NO_WRAP|Base64.NO_PADDING),StandardCharsets.UTF_8);
            JSONObject root=new JSONObject(payload);
            JSONObject auth=root.optJSONObject("https://api.openai.com/auth");
            if(auth!=null){
                String x=auth.optString("chatgpt_account_id",null);
                if(x!=null&&!x.isEmpty())return x;
            }
            String x=root.optString("chatgpt_account_id",null);
            return x==null||x.isEmpty()?null:x;
        }catch(Exception e){return null;}
    }

    private static Window window(JSONObject w){
        return w==null?null:new Window(w.optDouble("used_percent",0),w.optLong("reset_at",0));
    }

    String getPublicJson(String url,String userAgent) throws Exception {
        Request request=new Request.Builder()
                .url(url)
                .header("Accept","application/json")
                .header("User-Agent",userAgent)
                .get()
                .build();
        return executeText(request);
    }

    private String postForm(String url,FormBody form) throws Exception {
        Request request=new Request.Builder()
                .url(url)
                .header("Accept","application/json")
                .header("User-Agent","android-codex-tools/0.1")
                .post(form)
                .build();
        return executeText(request);
    }

    private String executeText(Request request) throws Exception {
        try(Response response=http.newCall(request).execute()){
            String text=bodyText(response.body());
            if(!response.isSuccessful()){
                throw new IllegalStateException("HTTP "+response.code()+": "+text);
            }
            return text;
        }catch(UnknownHostException e){
            throw new UnknownHostException("OkHttp DNS/connect failed for "+request.url().host()+": "+e.getMessage());
        }
    }

    private static String bodyText(ResponseBody body) throws Exception {
        return body==null?"":body.string();
    }

    private static Dns buildDns(Context context){
        OkHttpClient bootstrapClient=new OkHttpClient.Builder()
                .connectTimeout(10,TimeUnit.SECONDS)
                .readTimeout(10,TimeUnit.SECONDS)
                .build();

        Dns googleDoh=null;
        Dns cloudflareDoh=null;
        try{
            googleDoh=new DnsOverHttps.Builder()
                    .client(bootstrapClient)
                    .url(HttpUrl.get("https://dns.google/dns-query"))
                    .bootstrapDnsHosts(
                            InetAddress.getByAddress(new byte[]{8,8,8,8}),
                            InetAddress.getByAddress(new byte[]{8,8,4,4}))
                    .includeIPv6(true)
                    .build();
        }catch(Exception ignored){}

        try{
            cloudflareDoh=new DnsOverHttps.Builder()
                    .client(bootstrapClient)
                    .url(HttpUrl.get("https://cloudflare-dns.com/dns-query"))
                    .bootstrapDnsHosts(
                            InetAddress.getByAddress(new byte[]{1,1,1,1}),
                            InetAddress.getByAddress(new byte[]{1,0,0,1}))
                    .includeIPv6(true)
                    .build();
        }catch(Exception ignored){}

        final Dns finalGoogleDoh=googleDoh;
        final Dns finalCloudflareDoh=cloudflareDoh;

        return hostname -> {
            Set<InetAddress> found=new LinkedHashSet<>();
            List<String> errors=new ArrayList<>();

            try{
                InetAddress[] system=InetAddress.getAllByName(hostname);
                for(InetAddress a:system)found.add(a);
            }catch(Exception e){
                errors.add("system="+e.getClass().getSimpleName()+":"+e.getMessage());
            }

            try{
                ConnectivityManager cm=(ConnectivityManager)context.getSystemService(Context.CONNECTIVITY_SERVICE);
                if(cm!=null){
                    Network active=cm.getActiveNetwork();
                    if(active!=null){
                        try{
                            for(InetAddress a:active.getAllByName(hostname))found.add(a);
                        }catch(Exception e){
                            errors.add("active="+e.getClass().getSimpleName()+":"+e.getMessage());
                        }
                    }
                    for(Network n:cm.getAllNetworks()){
                        NetworkCapabilities caps=cm.getNetworkCapabilities(n);
                        if(caps==null ||
                                !caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) ||
                                !caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED))continue;
                        try{
                            for(InetAddress a:n.getAllByName(hostname))found.add(a);
                        }catch(Exception e){
                            errors.add("network"+n+"="+e.getClass().getSimpleName()+":"+e.getMessage());
                        }
                    }
                }
            }catch(Exception e){
                errors.add("connectivity="+e.getClass().getSimpleName()+":"+e.getMessage());
            }

            if(found.isEmpty() && finalGoogleDoh!=null){
                try{
                    found.addAll(finalGoogleDoh.lookup(hostname));
                }catch(Exception e){
                    errors.add("googleDoH="+e.getClass().getSimpleName()+":"+e.getMessage());
                }
            }

            if(found.isEmpty() && finalCloudflareDoh!=null){
                try{
                    found.addAll(finalCloudflareDoh.lookup(hostname));
                }catch(Exception e){
                    errors.add("cloudflareDoH="+e.getClass().getSimpleName()+":"+e.getMessage());
                }
            }

            if(found.isEmpty()){
                throw new UnknownHostException("No resolver returned addresses for "+hostname+"; "+String.join(" | ",errors));
            }
            return new ArrayList<>(found);
        };
    }

    private static String enc(String s) throws Exception {
        return java.net.URLEncoder.encode(s,"UTF-8");
    }
}
