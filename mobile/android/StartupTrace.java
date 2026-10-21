package com.christorng.androidcodextools;

import android.content.Context;
import android.content.SharedPreferences;

/**
 * Crash-resistant, non-secret startup breadcrumbs. This class deliberately has
 * no dependency on Tauri, SQLite, network, or the old credential implementation.
 * The very last successfully committed marker is visible on the next launch.
 */
public final class StartupTrace {
    private static final String PREFS = "codexcredits_startup_diagnostic";
    private static final String KEY = "last_checkpoint";

    private StartupTrace() {}

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
