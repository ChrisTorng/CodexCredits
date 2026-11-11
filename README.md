# CodexCredits

Android and Windows share the same Rust/Tauri 2 backend and TypeScript UI. This is the only maintained cross-platform repository. [android-codex-tools](https://github.com/ChrisTorng/android-codex-tools) is no longer a maintained application source. During the **v2.0.12 signing transition only**, temporary workflows accessed the original Android signing cache. The keystore was then encrypted for the maintainer, migrated into this repository's Actions secret, and independently verified by a successful signed APK build. Those temporary workflows have been retired; the legacy repository remains a source-code reference. MIT license.

## Windows layout

Default small **portrait window (430 × 760)**, resizable down to 340 × 520. Closing the window hides it to the system tray; the background monitor and enabled triggers continue while the process is alive. Quit from the tray stops them. Starting a second process focuses the existing tray app instead of spawning duplicate monitoring or trigger loops. No server or .NET runtime; Windows requires the system WebView2 runtime.

The four tabs reproduce the Android UI hierarchy and Chinese captions: **● 狀態 / ◷ 排程 / ▥ 歷程 / ⚙ 設定**. All quota percentages and historical readings show **remaining percent, not used percent** (`100 - backend.used_percent`). The five-hour curve is blue and the weekly curve orange; solid lines show actual remaining quota, dashed lines show the current-window ideal. History separates yesterday/today and events.

The status page contains the five-hour and weekly cards, next schedule, manual trigger, Banked Reset, and third-party Global Reset forecast with timeline-derived average/median estimates. The schedule page contains Off / Auto after reset / Custom anchor + follow-up mode, actual next in-process event, weekly/default anchor editors and today/tomorrow overrides. Settings contain ChatGPT browser OAuth PKCE with refresh, theme (system/light/dark), sample interval (10/15/30/60 minutes), and private optional ntfy topic generation/testing.

## App name and upgrades

Both Android and Windows display **Codex Credits** (app launcher, header, notifications and tray/shortcut names). Current build version: **2.0.29**. Android retains package ID `com.christorng.androidcodextools` and Windows retains app identifier `com.christorng.codexcredits` and the internal NSIS product name `CodexCredits`, preserving in-place upgrades and existing data.

## Downloads and verification

- **Windows:** [Windows GitHub Actions builds](https://github.com/ChrisTorng/CodexCredits/actions/workflows/windows.yml) provide `CodexCredits-Windows-v<version>-x64.zip` with portable `CodexCredits.exe`, the versioned `CodexCredits_<version>_x64-setup.exe` and documentation. Prefer the installer for Windows notifications. CI checks that the installer name contains the version.
- **Android:** [Signed Android build workflow](https://github.com/ChrisTorng/CodexCredits/actions/workflows/android-signed.yml). The [APK direct-download mirror](https://raw.githubusercontent.com/ChrisTorng/CodexCredits/apk-download/CodexCredits-Android.apk) is updated by the [APK mirror workflow](https://github.com/ChrisTorng/CodexCredits/actions/workflows/android-apk-mirror.yml) after signature and digest verification. Do not assume the mirror is the newest build until its workflow succeeds.

GitHub Actions checks TypeScript, chart tests, Rust unit tests, Windows Release/NSIS and Android compilation. **A successful CI run is not proof of live account actions, wake/sleep behavior, Windows native notifications or Android device behavior.** Those require real-device acceptance tests.

## Safety and execution

- OAuth access and refresh tokens use OS secure credentials segmented to avoid the Windows Credential Manager 2,560-byte UTF-16 per-secret limit.
- The app does **not** auto-start credit-consuming requests with default settings. The initial scheduling mode is Off. Switching to Auto/Custom requires an explicit confirmation in the UI.
- Manual trigger consumes a minimal Codex request only after explicit user confirmation. The app rechecks quotas before calling it and avoids starting an already-confirmed active five-hour window.
- Automatic scheduling only operates in the running process, unlike Android AlarmManager. No Task Scheduler, JobScheduler, background service or delayed catch-up after sleep/shutdown. The countdown uses actual backend five-hour reset rather than blindly adding five hours.
- Banked Reset consumption requires user confirmation or explicit one-shot arming and is revalidated server-side immediately before use. The chosen Banked Reset one-shot arming state is retained in SQLite across app restarts, but is **never executed while the app is stopped**. It is cleared before a redemption attempt to prevent duplicate redemption if a network response is ambiguous.
- ntfy has no hard-coded topic and is off by default. It only accepts private `https://ntfy.sh/<topic>` URL configuration.
- ChatGPT Codex backend endpoints are not a guaranteed public stable API. Service compatibility, account entitlements, and applicability of service rules remain subject to change.

## Known platform differences / remaining parity checks

Unlike Android, Windows does not use Android exact alarms / Doze exemptions. Windows native notifications use the Tauri notification plugin and require an **installed Windows app** (NSIS setup supplies the Start Menu identity). A portable EXE does not have the required Windows app registration, so native toast delivery is not guaranteed even if the OS permission check reports granted. The Settings page checks plugin notification permission automatically and requests permission when needed; a manual test is not required to configure notifications. Windows Focus Assist and system notification policy can still suppress visual banners. Optional ntfy is also available. The in-process Windows timer is best-effort and cannot execute while asleep or after Quit. A complete pixel-accurate Android Canvas rendering is not claimed; visual components reproduce the original layout, colors, and information semantics using HTML and ECharts.

The desktop app supports the original core scheduling decisions, day overrides and banked operations, but live account-level side effects and edge cases still require end-to-end regression testing. See [the behavior parity matrix](docs/ANDROID_PARITY.md).

## Windows development

Install Node.js 22+, Rust MSVC and Microsoft C++ Build Tools / Windows SDK:

```powershell
npm install
npm run tauri dev
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --no-bundle
```

Copyright (c) 2026 ChrisTorng. MIT license.

## UI startup and persisted history

The system theme is applied on the first paint, even when the saved setting equals the default `system`; it also follows live OS theme changes. Initial UI bootstrap uses one local-state IPC to hydrate saved usage, history and events from SQLite **before** any online refresh. The startup background refresh broadcasts a UI update, so relaunching the app never requires a manual quota refresh to display existing charts. SQLite data is retained under the app data directory and normally persists after quitting or installing a newer version. **Android v2.0.5 exception:** to prevent showing incorrectly inverted legacy quota percentages, the first launch discards historical quota samples once. It preserves sign-in credentials, scheduling settings and event records; future samples are stored as API `used_percent` and displayed as remaining percentage. Android views apply native status/navigation-bar and keyboard insets, and history may scroll on smaller displays. The Android compilation workflow verifies ARM64 compilation, UI branding and retained JNI methods; it does not distribute a separately signed APK. The [Android signed-release workflow](https://github.com/ChrisTorng/CodexCredits/actions/workflows/android-signed.yml) uses the migrated `ANDROID_UPGRADE_KEYSTORE_BASE64` secret, checks the original Android certificate, package name, versionCode, launcher label and JNI methods, then publishes a signed artifact. The [v2.0.13 signed build](https://github.com/ChrisTorng/CodexCredits/actions/runs/41078554804) succeeded using only this repository's signing secret. The [APK mirror workflow](https://github.com/ChrisTorng/CodexCredits/actions/workflows/android-apk-mirror.yml) publishes the verified signed artifact as the stable raw APK direct-download link. Update version numbers in both workflows together for future releases. Keep signing material out of source control, build logs, public artifacts and chat. Android native background scheduling is preserved.
