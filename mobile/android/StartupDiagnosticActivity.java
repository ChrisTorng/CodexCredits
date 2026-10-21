package com.christorng.androidcodextools;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.os.Bundle;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.ScrollView;
import android.widget.TextView;
import org.json.JSONArray;
import org.json.JSONObject;
import org.json.JSONTokener;
import java.security.KeyStore;
import java.util.Map;

/**
 * Native, user-stepped diagnostic launcher shown before any Rust/Tauri code.
 * All probes are read-only. Never display preference values or token material.
 */
public final class StartupDiagnosticActivity extends Activity {
    private TextView heading;
    private TextView log;
    private Button button;
    private ProgressBar busy;
    private int stage = 0;
    private boolean running = false;
    private final StringBuilder report = new StringBuilder();
    private String previous;
    private String previousTrail;
    private StartupTrace.Versions versions;
    private final String[] actions = {
        "步驟 1／4：查看舊版資料項目",
        "步驟 2／4：檢查 JSON 格式",
        "步驟 3／4：檢查加密憑證與匯出橋接",
        "步驟 4／4：啟動 CodexCredits"
    };

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        // Capture last startup's marker BEFORE writing this launch's marker.
        previous = StartupTrace.last(this);
        previousTrail = StartupTrace.history(this);
        versions = StartupTrace.recordLaunchVersion(this);
        StartupTrace.mark(this, "00 diagnostic-screen-opened");
        installCrashBreadcrumbs();
        draw();
    }

    private void installCrashBreadcrumbs() {
        final Thread.UncaughtExceptionHandler original =
                Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler((thread, error) -> {
            try {
                String name = error.getClass().getSimpleName();
                // Do not persist exception.getMessage(): it might include credentials.
                String last = StartupTrace.last(getApplicationContext());
                StartupTrace.mark(getApplicationContext(),
                    "99 java-uncaught-" + name + " after " + last);
            } catch (Throwable ignored) {}
            if (original != null) original.uncaughtException(thread, error);
            else android.os.Process.killProcess(android.os.Process.myPid());
        });
    }

    private int dp(int value) {
        return (int)(value * getResources().getDisplayMetrics().density + 0.5f);
    }
    private void draw() {
        ScrollView scroll = new ScrollView(this);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(dp(20), dp(24), dp(20), dp(20));

        heading = new TextView(this);
        heading.setTextSize(23);
        heading.setText("CodexCredits 啟動診斷");
        root.addView(heading);

        // Place actual installed version BEFORE the instructions so it is
        // visible immediately in the first screenshot, without any button tap.
        TextView versionInfo = new TextView(this);
        versionInfo.setTextSize(16);
        versionInfo.setPadding(0, dp(10), 0, dp(10));
        versionInfo.setText("目前執行版本：" + versions.current
            + "\n上次啟動版本：" + versions.previousLaunch
            + "\n上次不同版本：" + versions.previousDifferent
            + "\nAndroid 套件：" + versions.packageName);
        root.addView(versionInfo);

        TextView intro = new TextView(this);
        intro.setTextSize(15);
        intro.setPadding(0, dp(12), 0, dp(14));
        intro.setText("此畫面是 Android 原生程式，不需啟動 Rust。每按一次檢查一個階段，沒有自動跳步。\n\n"
            + "只讀取舊版資料；不刪除、不清空，也不會觸發配額或 Banked Reset。"
            + "\n\n上次啟動最後紀錄：" + previous
            + "\n\n若按「啟動 CodexCredits」後閃退，重新點圖示回到本頁，拍下「上次啟動最後紀錄」。");
        root.addView(intro);

        busy = new ProgressBar(this);
        busy.setIndeterminate(true);
        busy.setVisibility(View.GONE);
        root.addView(busy);

        button = new Button(this);
        button.setAllCaps(false);
        button.setOnClickListener(v -> next());
        root.addView(button);

        TextView label = new TextView(this);
        label.setText("檢查結果（僅類型、筆數、最後初始化階段）");
        label.setTextSize(17);
        label.setPadding(0, dp(20), 0, dp(8));
        root.addView(label);

        log = new TextView(this);
        log.setTextSize(14);
        log.setTextIsSelectable(true);
        root.addView(log);

        scroll.addView(root);
        setContentView(scroll);
        report.append("開始診斷。\n先前啟動節點：").append(previous)
            .append("\n上一輪經過的初始化節點（依實際執行順序）：\n")
            .append(previousTrail).append("\n");
        update();
    }

    private void update() {
        button.setEnabled(!running && stage < actions.length);
        button.setText(stage < actions.length ? actions[stage] : "已提交啟動請求");
        busy.setVisibility(running ? View.VISIBLE : View.GONE);
        log.setText(report.toString());
    }
    private void append(String text) {
        report.append("\n").append(text).append("\n");
        update();
    }
    private interface Probe { String read() throws Exception; }
    private void runProbe(int id, String description, Probe probe) {
        running = true;
        StartupTrace.mark(this, "0" + id + " probe-started");
        append("▶ " + description + "：執行中");
        new Thread(() -> {
            String message;
            try {
                message = probe.read();
            } catch (Throwable e) {
                // Never include the exception message, which might contain private data.
                message = "檢查失敗（" + e.getClass().getSimpleName() + "）；仍可繼續下一步";
            }
            String summary = message;
            runOnUiThread(() -> {
                StartupTrace.mark(this, "0" + id + " probe-finished");
                stage = id;
                running = false;
                append("✓ " + description + "\n" + summary);
            });
        }, "cc-diagnostic-" + id).start();
    }

    private void next() {
        if (running) return;
        if (stage == 0) runProbe(1, "舊版 SharedPreferences", this::probePreferences);
        else if (stage == 1) runProbe(2, "舊版 JSON 格式", this::probeJson);
        else if (stage == 2) runProbe(3, "Keystore／匯出橋接", this::probeCredentials);
        else if (stage == 3) {
            StartupTrace.mark(this, "40 user-requested-main-activity");
            append("▶ 已請求啟動 Tauri。下一次開啟本診斷頁可查上次進度。");
            stage = 4;
            update();
            try {
                Intent intent = new Intent(this, MainActivity.class);
                startActivity(intent);
            } catch (Throwable error) {
                StartupTrace.mark(this, "40 main-launch-failed-" + error.getClass().getSimpleName());
                append("無法開啟主畫面：" + error.getClass().getSimpleName());
            }
        }
    }

    private String probePreferences() {
        Map<String,?> entries = getSharedPreferences("app", Context.MODE_PRIVATE).getAll();
        StringBuilder out = new StringBuilder("舊版 app preferences 項目數：" + entries.size());
        int text = 0, numbers = 0, bools = 0, other = 0;
        for (Object value : entries.values()) {
            if (value instanceof String) text++;
            else if (value instanceof Number) numbers++;
            else if (value instanceof Boolean) bools++;
            else other++;
        }
        out.append("\n字串：").append(text).append("；數字：").append(numbers)
            .append("；布林：").append(bools).append("；其他：").append(other);
        Object oldMode = entries.get("scheduler_mode");
        out.append("\n排程模式欄位：").append(oldMode == null ? "不存在" : "已存在（不顯示值）");
        out.append("\n僅統計類型，不回傳任何設定值。");
        return out.toString();
    }

    private String probeJson() throws Exception {
        Map<String,?> entries = getSharedPreferences("app", Context.MODE_PRIVATE).getAll();
        String[][] checks = {
            {"usage_samples_v1", "array"},
            {"usage_history_events_v1", "array"},
            {"schedule_weekly_v2", "array"},
            {"schedule_day_overrides_v2", "object"},
            {"schedule_rules_v1", "array"}
        };
        StringBuilder out = new StringBuilder();
        for (String[] check : checks) {
            Object raw = entries.get(check[0]);
            out.append("\n").append(check[0]).append("：");
            if (raw == null) { out.append("不存在（可正常略過）"); continue; }
            if (!(raw instanceof String)) {
                out.append("類型不符，預期 String，實際 ").append(raw.getClass().getSimpleName());
                continue;
            }
            String data = (String)raw;
            if (data.length() > 2_000_000) {
                out.append("過大，超過 200 萬字元，跳過以避免記憶體耗盡");
                continue;
            }
            try {
                Object parsed = new JSONTokener(data).nextValue();
                boolean isArray = "array".equals(check[1]);
                if (isArray && parsed instanceof JSONArray) {
                    JSONArray arr = (JSONArray)parsed;
                    out.append("JSON Array，").append(arr.length()).append(" 筆");
                    if ("usage_samples_v1".equals(check[0]) && arr.length() > 0) {
                        JSONObject first = arr.optJSONObject(0);
                        if (first == null || !first.has("at"))
                            out.append("；警告：首筆欠缺 at 時戳");
                    }
                } else if (!isArray && parsed instanceof JSONObject) {
                    out.append("JSON Object，").append(((JSONObject)parsed).length()).append(" 日期");
                } else out.append("格式錯誤：根節點類型與舊版不符");
            } catch (Throwable error) {
                out.append("解析錯誤：").append(error.getClass().getSimpleName());
            }
        }
        return out.toString();
    }

    private String probeCredentials() throws Exception {
        // This first check reads only metadata; it never reveals token text.
        SharedPreferences protectedPrefs =
            getSharedPreferences("secure_tokens", Context.MODE_PRIVATE);
        boolean hasAccess = protectedPrefs.contains("access_token");
        boolean hasRefresh = protectedPrefs.contains("refresh_token");
        boolean hasAccount = protectedPrefs.contains("account_id");

        KeyStore store = KeyStore.getInstance("AndroidKeyStore");
        store.load(null);
        boolean hasKey = store.containsAlias("codex_tools_aes");

        // Execute the same Java export code invoked by the Rust migration.
        // The JSON is intentionally discarded after extracting key counts.
        String export = LegacyBridge.exportLegacy(this);
        JSONObject object = new JSONObject(export);
        JSONObject prefs = object.getJSONObject("prefs");
        JSONObject tokens = object.getJSONObject("tokens");
        return "加密資料欄位（只顯示存在與否）：access=" + hasAccess
            + "、refresh=" + hasRefresh + "、account=" + hasAccount
            + "\nAndroid Keystore 舊別名存在：" + hasKey
            + "\n原生 LegacyBridge.exportLegacy()：成功"
            + "\n匯出設定筆數：" + prefs.length()
            + "；可解密憑證欄位數：" + tokens.length()
            + "\n所有 Token 內容均不顯示、不寫入日誌。";
    }
}
