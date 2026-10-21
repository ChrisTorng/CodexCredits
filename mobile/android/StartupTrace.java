package com.christorng.androidcodextools;

import android.content.Context;
import android.content.SharedPreferences;
import android.content.pm.PackageInfo;
import android.os.Build;

/**
 * Crash-resistant, non-secret startup breadcrumbs. This class deliberately has
 * no dependency on Tauri, SQLite, network, or the old credential implementation.
 * The very last successfully committed marker is visible on the next launch.
 */
public final class StartupTrace {
    private static final String PREFS = "codexcredits_startup_diagnostic";
    private static final String KEY = "last_checkpoint";

    private StartupTrace() {}

    private static final String LAST_VERSION = "last_diagnostic_launch_version";
    private static final String PRIOR_DISTINCT_VERSION = "last_distinct_version";
    private static final String UNKNOWN_VERSION = "無紀錄（較早版本未提供追蹤）";

    public static final class Versions {
        public final String current;
        public final String previousLaunch;
        public final String previousDifferent;
        public final String packageName;
        private Versions(String current, String previousLaunch, String previousDifferent,
                         String packageName) {
            this.current = current;
            this.previousLaunch = previousLaunch;
            this.previousDifferent = previousDifferent;
            this.packageName = packageName;
        }
    }

    /** Read the actual installed package, never a hardcoded filename/version.
     * The previous launch is captured BEFORE updating the version record.
     * First-run older versions could not have recorded their version, so
     * reporting 'unknown' is intentional: do not invent an upgrade history.
     */
    @SuppressWarnings("deprecation")
    public static Versions recordLaunchVersion(Context context) {
        Context app = context.getApplicationContext();
        String current = "無法取得版本";
        try {
            PackageInfo info = app.getPackageManager().getPackageInfo(app.getPackageName(), 0);
            long code = Build.VERSION.SDK_INT >= 28
                    ? info.getLongVersionCode() : (long)info.versionCode;
            String name = info.versionName == null ? "未知" : info.versionName;
            current = name + " (versionCode " + code + ")";
        } catch (Throwable ignored) { }
        String prior = UNKNOWN_VERSION;
        String distinct = UNKNOWN_VERSION;
        try {
            SharedPreferences prefs = app.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
            prior = prefs.getString(LAST_VERSION, UNKNOWN_VERSION);
            distinct = prefs.getString(PRIOR_DISTINCT_VERSION, UNKNOWN_VERSION);
            SharedPreferences.Editor change = prefs.edit();
            if (!UNKNOWN_VERSION.equals(prior) && !current.equals(prior)) {
                distinct = prior;
                change.putString(PRIOR_DISTINCT_VERSION, distinct);
            }
            change.putString(LAST_VERSION, current).commit();
        } catch (Throwable ignored) { }
        return new Versions(current, prior, distinct, app.getPackageName());
    }

    private static final String SKIP_KEY = "skip_legacy_import_once";

    public static void setSkipLegacy(Context context, boolean skip) {
        try {
            SharedPreferences.Editor edit = context.getApplicationContext()
                .getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit();
            if (skip) edit.putBoolean(SKIP_KEY, true);
            else edit.remove(SKIP_KEY);
            edit.commit();
        } catch (Throwable ignored) {}
    }

    public static boolean consumeSkipLegacy(Context context) {
        try {
            SharedPreferences prefs = context.getApplicationContext()
                .getSharedPreferences(PREFS, Context.MODE_PRIVATE);
            boolean skip = prefs.getBoolean(SKIP_KEY, false);
            // Always consume: the next ordinary launch must import normally.
            prefs.edit().remove(SKIP_KEY).commit();
            return skip;
        } catch (Throwable ignored) {
            return false;
        }
    }


    public static void mark(Context context, String checkpoint) {
        try {
            context.getApplicationContext()
                .getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .putString(KEY, checkpoint)
                .commit();
        } catch (Throwable ignored) {
            // A diagnostic facility must never become another startup crash.
        }
    }

    public static String last(Context context) {
        try {
            return context.getApplicationContext()
                .getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .getString(KEY, "無先前紀錄");
        } catch (Throwable ignored) {
            return "診斷紀錄無法讀取";
        }
    }
}
