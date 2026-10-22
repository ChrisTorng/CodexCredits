# CodexCredits

Windows-first, local-first Rust/Tauri 2 adaptation of [android-codex-tools](https://github.com/ChrisTorng/android-codex-tools). Open source under MIT. **Android is the behavioral and visual reference.** This is a separate repository; the Android source is not modified.

## Windows layout

Default small **portrait window (430 × 760)**, resizable down to 340 × 520. Closing the window hides it to the system tray; the background monitor and enabled triggers continue while the process is alive. Quit from the tray stops them. Starting a second process focuses the existing tray app instead of spawning duplicate monitoring or trigger loops. No server or .NET runtime; Windows requires the system WebView2 runtime.

The four tabs reproduce the Android UI hierarchy and Chinese captions: **● 狀態 / ◷ 排程 / ▥ 歷程 / ⚙ 設定**. All quota percentages and historical readings show **remaining percent, not used percent** (`100 - backend.used_percent`). The five-hour curve is blue and the weekly curve orange; solid lines show actual remaining quota, dashed lines show the current-window ideal. History separates yesterday/today and events.

The status page contains the five-hour and weekly cards, next schedule, manual trigger, Banked Reset, and third-party Global Reset forecast with timeline-derived average/median estimates. The schedule page contains Off / Auto after reset / Custom anchor + follow-up mode, actual next in-process event, weekly/default anchor editors and today/tomorrow overrides. Settings contain ChatGPT browser OAuth PKCE with refresh, theme (system/light/dark), sample interval (10/15/30/60 minutes), and private optional ntfy topic generation/testing.

## Download

Open [Windows Build GitHub Actions](https://github.com/ChrisTorng/CodexCredits/actions/workflows/windows.yml), select the latest successful **main** build, then download `CodexCredits-Windows-x64` under **Artifacts**. **Unzip only once**: the artifact directly contains the portable `CodexCredits.exe`, the Windows `*-setup.exe` installer, `README.md` and `LICENSE`; no nested ZIP. **Choose the setup installer for Windows native notifications**; the portable EXE remains available for users who do not need native Windows toasts.

GitHub Actions checks TypeScript, runs Rust unit tests and compiles Windows release. A successful CI build is not a substitute for checking OAuth, real credit-consuming operations, OS tray interactions, Windows sleep/resume and platform notifications on a real machine.

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

The system theme is applied on the first paint, even when the saved setting equals the default `system`; it also follows live OS theme changes. Initial UI bootstrap hydrates quota history and events directly from local SQLite **before** any online refresh. The startup background refresh broadcasts a UI update, so relaunching the app never requires a manual quota refresh to display existing charts. SQLite data is retained under the app data directory and normally persists after quitting or installing a newer version. **Android v2.0.5 exception:** to prevent showing incorrectly inverted legacy quota percentages, the first launch discards historical quota samples once. It preserves sign-in credentials, scheduling settings and event records; future samples are stored as API `used_percent` and displayed as remaining percentage. Android views apply native status/navigation-bar and keyboard insets, and history may scroll on smaller displays. The Android project CI now compiles an optimized **Release** APK and checks retained JNI methods but does **not** distribute an unsigned or differently signed build. Production ARM64 APKs are signed with the original package certificate by the [v2.0.5 Release signing workflow](https://github.com/ChrisTorng/android-codex-tools/actions/workflows/codexcredits-v205-release.yml). This workflow checks out the source from this CodexCredits repository; the original repository is used only because its Actions cache retains the previous signing identity. Download the direct `.apk` asset from the successful workflow's GitHub Release; do not uninstall before upgrading.
