# CodexCredits agent instructions

This is the **only maintained** repository for the cross-platform Codex Credits app: `ChrisTorng/CodexCredits`.

## Scope and tooling

- Change, commit, build, test, and release **only** in `ChrisTorng/CodexCredits`.
- `ChrisTorng/android-codex-tools` is a read-only source reference. A pinned checkout of its native Java code for building is allowed. Do not edit its files, trigger workflows, publish Releases, update APK mirrors or alter its branches and caches.
- Use OpenAI's official GitHub plugin for repository work, **never** request GitHub Repo Creator for this existing repository.

## Unified branding and upgrade requirements

- User-visible name on Android and Windows is `Codex Credits`: installed-app display name, Android launcher, WebView header/title, desktop tray menu, Windows shortcuts, notifications.
- Repository and portable EXE keep the spelling `CodexCredits`.
- Android application ID **must remain** `com.christorng.androidcodextools`; keep the native Java package/JNI ABI, original AlarmManager/JobScheduler, credentials, schedule and usage history.
- Original Android signer SHA-256: `177f5e3fed59854e4203dafe13490a75c2438b37731ff0e7af7acbfe45a8e679`. Strictly increase versionCode with each APK release.
- Windows identifier `com.christorng.codexcredits` and internal NSIS `productName: CodexCredits` remain unchanged to preserve the installed path/upgrade identity; override `shortcutName`, `uninstallDisplayName`, window title and tray text instead.
- Never delete user data or suggest uninstall/clear-data merely to rename.

## Android signed APK distribution

- The original private Android signing key is currently in a **legacy repository-specific Actions cache**, not a CodexCredits repository secret. The compile-only Android workflow does **not** generate a distributable, compatible upgrade APK.
- To ship Android from **this repository**, first securely migrate the original keystore into the `ANDROID_UPGRADE_KEYSTORE_BASE64` Actions secret **in this repository only**, then run `.github/workflows/android-signed.yml`.
- The workflow verifies the actual signer, application ID, versionCode, and JNI methods. Confirm the workflow and artifact are successful before sharing an Android installation link.
- Historical download failure: direct GitHub Release asset URLs stalled at 'Downloading...' in the ChatGPT Android in-app browser; a sandbox APK once showed 'Library file not found'. The user previously succeeded by opening the specific successful signed GitHub Actions run and downloading the artifact. Prefer that precise run URL once available. Do not present an old APK mirror as the current version.
- Don't claim an unsigned/other-key APK is installable over the old app. If signature migration has not happened, state explicitly that delivery is blocked.

## Windows

- `.github/workflows/windows.yml` runs TypeScript/Rust tests and builds portable `CodexCredits.exe` and NSIS setup.
- Windows native toasts require the installed package identity; the portable EXE is not a reliable native-toast substitute.
- Preserve cross-platform functional parity through the shared UI and Rust backend. Real account actions, native alarms, tray behavior and sleep/wake require device-specific regression tests.
