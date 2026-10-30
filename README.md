# CodexCredits

Android and Windows share the same Rust/Tauri 2 backend and TypeScript UI. This is the only maintained cross-platform repository. [android-codex-tools](https://github.com/ChrisTorng/android-codex-tools) is no longer a maintained application source. During the **v2.0.12 signing transition only**, temporary workflows accessed the original Android signing cache. The keystore was then encrypted for the maintainer, migrated into this repository's Actions secret, and independently verified by a successful signed APK build. Those temporary workflows have been retired; the legacy repository remains a source-code reference. MIT license.

## Windows layout

Default small **portrait window (430 × 760)**, resizable down to 340 × 520. Closing the window hides it to the system tray; the background monitor and enabled triggers continue while the process is alive. Quit from the tray stops them. Starting a second process focuses the existing tray app instead of spawning duplicate monitoring or trigger loops. No server or .NET runtime; Windows requires the system WebView2 runtime.

The four tabs reproduce the Android UI hierarchy and Chinese captions: **● 狀態 / ◷ 排程 / ▥ 歷程 / ⚙ 設定**. All quota percentages and historical readings show **remaining percent, not used percent** (`100 - backend.used_percent`). The five-hour curve is blue and the weekly curve orange; solid lines show actual remaining quota, dashed lines show the current-window ideal. History separates yesterday/today and events.

The status page contains the five-hour and weekly cards, next schedule, manual trigger, Banked Reset, and third-party Global Reset forecast with timeline-derived average/median estimates. The schedule page contains Off / Auto after reset / Custom anchor + follow-up mode, actual next in-process event, weekly/default anchor editors and today/tomorrow overrides. Settings contain ChatGPT browser OAuth PKCE with refresh, theme (system/light/dark), sample interval (10/15/30/60 minutes), and private optional ntfy topic generation/testing.

## App name and upgrades

Both Android and Windows display **Codex Credits** (app launcher, header, notifications and tray/shortcut names). Release version: **2.0.13**. Android retains package ID `com.christorng.androidcodextools` and Windows retains app identifier `com.christorng.codexcredits` and the internal NSIS product name `CodexCredits` (visible Windows installer labels are updated using an NSIS hook); do not reset data or uninstall merely to rename the application.

## Download

**Android v2.0.13:** [one-tap signed APK](https://raw.githubusercontent.com/ChrisTorng/CodexCredits/apk-download/CodexCredits-Android.apk) (10,259,047 bytes; SHA-256 `c203b35688595659d7ccb73a20a5e723bf993a794f604b43a3bf2ef3b3fe7438`). This APK is compiled, signed and mirrored **entirely from this repository**. [Original signed release run](https://github.com/ChrisTorng/CodexCredits/actions/runs/41078554804/artifacts/12988947200) and [direct-download mirror run](https://github.com/ChrisTorng/CodexCredits/actions/runs/41079960885) both succeeded. The mirror validates the signed workflow artifact SHA-256 before publishing to the `apk-download` branch. The raw endpoint's behavior in a mobile browser remains device-dependent. Do not uninstall the previous version; the package ID and original signer are unchanged. The encrypted signing key was migrated into this repository's `ANDROID_UPGRADE_KEYSTORE_BASE64` Actions secret. The original signing cache in the legacy repository remains as emergency backup, but its temporary signing/transfer workflow files were removed.

**Windows v2.0.13:** [Download verified Windows EXE and NSIS setup artifact](https://github.com/ChrisTorng/CodexCredits/actions/runs/41078799864/artifacts/12988972836), built and tested successfully in [Windows run 41078799864](https://github.com/ChrisTorng/CodexCredits/actions/runs/41078799864). The ZIP includes `CodexCredits.exe` and `CodexCredits_2.0.13_x64-setup.exe`. **Unzip only once**: the artifact directly contains the portable `CodexCredits.exe`, the Windows `*-setup.exe` installer, `README.md` and `LICENSE`; no nested ZIP. **Choose the setup installer for Windows native notifications**; the portable EXE remains available for users who do not need native Windows toasts.

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

The system theme is applied on the first paint, even when the saved setting equals the default `system`; it also follows live OS theme changes. Initial UI bootstrap hydrates quota history and events directly from local SQLite **before** any online refresh. The startup background refresh broadcasts a UI update, so relaunching the app never requires a manual quota refresh to display existing charts. SQLite data is retained under the app data directory and normally persists after quitting or installing a newer version. **Android v2.0.5 exception:** to prevent showing incorrectly inverted legacy quota percentages, the first launch discards historical quota samples once. It preserves sign-in credentials, scheduling settings and event records; future samples are stored as API `used_percent` and displayed as remaining percentage. Android views apply native status/navigation-bar and keyboard insets, and history may scroll on smaller displays. The Android compilation workflow verifies ARM64 compilation, UI branding and retained JNI methods; it does not distribute a separately signed APK. The [Android signed-release workflow](https://github.com/ChrisTorng/CodexCredits/actions/workflows/android-signed.yml) uses the migrated `ANDROID_UPGRADE_KEYSTORE_BASE64` secret, checks the original Android certificate, package name, versionCode, launcher label and JNI methods, then publishes a signed artifact. The [v2.0.13 signed build](https://github.com/ChrisTorng/CodexCredits/actions/runs/41078554804) succeeded using only this repository's signing secret. The [APK mirror workflow](https://github.com/ChrisTorng/CodexCredits/actions/workflows/android-apk-mirror.yml) publishes the verified signed artifact as the stable raw APK direct-download link. Update version numbers in both workflows together for future releases. Keep signing material out of source control, build logs, public artifacts and chat. Android native background scheduling is preserved.
