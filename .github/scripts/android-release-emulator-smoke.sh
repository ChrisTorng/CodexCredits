#!/usr/bin/env bash
# Release APK is deliberately NOT debuggable: never use 'adb run-as' here.
# Verify startup from Activity/UI evidence available on unmodified Android.
set -euo pipefail
cd "$(cd "$(dirname "\${BASH_SOURCE[0]}")/../.." && pwd)"
package=com.christorng.androidcodextools
apk_dir="$PWD/src-tauri/gen/android/app/build/outputs/apk"

capture() {
  adb logcat -d -v threadtime > android-emulator-release-launch.log 2>/dev/null || true
  adb logcat -b crash -d -v threadtime > android-emulator-release-crash.log 2>/dev/null || true
  adb exec-out screencap -p > android-emulator-release-screen.png 2>/dev/null || true
  if [ -s android-emulator-release-crash.log ]; then
    echo '=== CRASH LOG ==='
    tail -n 120 android-emulator-release-crash.log
  fi
}
trap capture EXIT

apk="$(find "$apk_dir" -type f -name '*.apk' -print -quit)"
test -n "$apk" && test -f "$apk"
echo "Installing optimized release $apk"
adb install -r "$apk"
adb logcat -c

adb shell am start -W -n "$package/.StartupDiagnosticActivity"
sleep 2
adb shell am start -W -n "$package/.MainActivity"
sleep 12

if ! adb shell pidof "$package"; then
  echo "::error::App process crashed on first optimized release startup"
  exit 1
fi

echo "=== Focused Android Activity ==="
adb shell dumpsys activity activities | grep -Ei 'mResumedActivity|topResumedActivity|ResumedActivity' | head -n 6 || true

# uiautomator is a system-level observer: unlike run-as, it works for a
# non-debuggable production app. MainActivity should expose actual DOM text.
adb shell uiautomator dump /sdcard/codexcredits-ui.xml || true
adb shell cat /sdcard/codexcredits-ui.xml > android-emulator-release-ui.xml || true
echo "=== Tauri WebView text test ==="
if grep -Eq 'Codex Tools|5 小時|週配額|ChatGPT 尚未登入' android-emulator-release-ui.xml; then
  echo "Verified main Tauri UI visible on optimized release build."
else
  echo "::error::Process alive, but no expected Tauri UI text observed"
  head -c 1200 android-emulator-release-ui.xml || true
  exit 1
fi

adb shell am force-stop "$package"
adb shell am start -W -n "$package/.MainActivity"
sleep 7
if ! adb shell pidof "$package"; then
  echo "::error::App process died on second optimized release startup"
  exit 1
fi
echo "Optimized Release Android app survived both startups."
