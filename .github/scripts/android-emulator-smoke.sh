#!/usr/bin/env bash
# The emulator-runner executes each YAML script line in a fresh shell.
# Keep all variables, error handling and crash-log capture in ONE bash process.
set -euo pipefail
cd "${GITHUB_WORKSPACE:?GITHUB_WORKSPACE is required}"

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
echo "Starting $package"
adb shell am start -W -n "$package/.MainActivity"
sleep 12

if ! adb shell pidof "$package"; then
  echo "::error::Android app exited during initial startup"
  exit 1
fi
echo "Android app process survived initial startup."
adb shell am force-stop "$package"
adb shell am start -W -n "$package/.MainActivity"
sleep 8
if ! adb shell pidof "$package"; then
  echo "::error::Android app exited during second startup"
  exit 1
fi
echo "Android app process survived second startup."
