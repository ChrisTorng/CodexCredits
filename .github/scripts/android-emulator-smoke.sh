#!/usr/bin/env bash
# The emulator-runner executes each YAML script line in a fresh shell.
# Keep all variables, error handling and crash-log capture in ONE bash process.
set -euo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

package=com.christorng.androidcodextools
apk_dir="$PWD/src-tauri/gen/android/app/build/outputs/apk"

capture_logs() {
  adb logcat -d -v threadtime > android-emulator-launch.log 2>/dev/null || true
  adb logcat -b crash -d -v threadtime > android-emulator-crash.log 2>/dev/null || true
  if [ -s android-emulator-crash.log ]; then
    echo "=== Emulator crash buffer (last 120 lines) ==="
    tail -n 120 android-emulator-crash.log || true
  fi
}
trap capture_logs EXIT

if [ ! -d "$apk_dir" ]; then
  echo "::error::APK output directory does not exist: $apk_dir"
  exit 1
fi
echo "=== APK output files ==="
find "$apk_dir" -type f -name '*.apk' -printf '%p %s bytes\n'
apk="$(find "$apk_dir" -type f -name '*.apk' -print -quit)"
if [ -z "$apk" ] || [ ! -f "$apk" ]; then
  echo "::error::No APK found in $apk_dir"
  exit 1
fi

echo "Emulator ABI: $(adb shell getprop ro.product.cpu.abi)"
echo "Installing APK: $apk"
adb install -r "$apk"
adb logcat -c
echo "Launching pure Java diagnostic activity before the Rust backend"
adb shell am start -W -n "$package/.StartupDiagnosticActivity"
sleep 3
if ! adb shell pidof "$package"; then
  echo "::error::Native diagnostic launcher failed"
  exit 1
fi
echo "Native diagnostic Activity survived startup."
echo "Starting $package Tauri Activity"
adb shell am start -W -n "$package/.MainActivity"
sleep 12

if ! adb shell pidof "$package"; then
  echo "::error::Android app exited during initial startup"
  exit 1
fi
echo "Android app process survived initial startup."
# Process existence is NOT proof that the Rust/Tauri app started.
# Check the marker emitted from the real Rust .setup() or WebView.
boot_marker="$(adb shell run-as "$package" cat shared_prefs/codexcredits_startup_diagnostic.xml || true)"
echo "Initial Rust startup marker: $boot_marker"
if ! printf '%s' "$boot_marker" | grep -Eq '(57 rust-setup-complete|6[012] [a-z-]+)'; then
  echo "::error::Android process survived but Rust/Tauri never finished setup"
  exit 1
fi
echo "Verified Rust/Tauri setup executed."
adb shell am force-stop "$package"
adb shell am start -W -n "$package/.MainActivity"
sleep 8
if ! adb shell pidof "$package"; then
  echo "::error::Android app exited during second startup"
  exit 1
fi
echo "Android app process survived second startup."

# Repeat boot with representative old-app data. No real credentials are used,
# and the scheduler is OFF, so this fixture cannot consume quota.
echo "=== Testing legacy SharedPreferences migration on a fresh app DB ==="
adb shell am force-stop "$package"
adb shell pm clear "$package" >/dev/null
cat > legacy-app-fixture.xml <<'XML'
<?xml version="1.0" encoding="utf-8" standalone="yes"?>
<map>
  <string name="scheduler_mode">off</string>
  <int name="sample_interval_minutes" value="15" />
  <string name="theme_mode">system</string>
  <string name="schedule_weekly_v2">[{"id":"legacy-01","enabled":true,"hour":6,"minute":0,"autoCount":3,"days":"1111111"}]</string>
  <string name="schedule_day_overrides_v2">{"2026-10-21":{"enabled":false,"rules":[]}}</string>
  <string name="usage_samples_v1">[{"at":1792525200000,"p":12.5,"w":22.0,"pc":true,"pr":1792543200000,"wr":1792694400000,"b":2}]</string>
  <string name="usage_history_events_v1">[{"at":1792525200000,"type":"weekly_reset","label":"Test event","source":"legacy fixture"}]</string>
</map>
XML
adb push legacy-app-fixture.xml /data/local/tmp/codexcredits-legacy-app.xml >/dev/null
adb shell chmod 644 /data/local/tmp/codexcredits-legacy-app.xml
adb shell run-as "$package" mkdir -p shared_prefs
adb shell run-as "$package" cp /data/local/tmp/codexcredits-legacy-app.xml shared_prefs/app.xml
adb logcat -c
adb shell am start -W -n "$package/.StartupDiagnosticActivity"
sleep 3
adb shell am start -W -n "$package/.MainActivity"
sleep 12
if ! adb shell pidof "$package"; then
  echo "::error::Android app exited during legacy data migration"
  exit 1
fi
echo "Legacy data migration launched without killing the app process."
legacy_marker="$(adb shell run-as "$package" cat shared_prefs/codexcredits_startup_diagnostic.xml || true)"
echo "Migration Rust startup marker: $legacy_marker"
if ! printf '%s' "$legacy_marker" | grep -Eq '(57 rust-setup-complete|6[012] [a-z-]+)'; then
  echo "::error::Seeded legacy migration failed to reach successful Rust setup"
  exit 1
fi
echo "Seeded old-app data reached completed Rust setup."

