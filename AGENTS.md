# CodexCredits agent instructions

This is the **only maintained** repository for the cross-platform Codex Credits app: `ChrisTorng/CodexCredits`.

## Scope and tooling

- Maintain all **product source code** exclusively in `ChrisTorng/CodexCredits`.
- Temporary exception already used for **v2.0.12 signing**: the old repo `ChrisTorng/android-codex-tools` contains an added signing-only workflow at `.github/workflows/codexcredits-v212-release.yml`, which used the original Actions cache and pinned CodexCredits commit `fc66e7767055b1fadc47644a58d43b7f151dcdf4`. It produced [successful signed run 41035371805](https://github.com/ChrisTorng/android-codex-tools/actions/runs/41035371805). No old app source was changed. Avoid further use of that repo after migrating the key; it remains only a read-only Java source reference.
- Use OpenAI's official GitHub plugin for repository work, **never** request GitHub Repo Creator for this existing repository.

## Unified branding and upgrade requirements

- User-visible name on Android and Windows is `Codex Credits`: installed-app display name, Android launcher, WebView header/title, desktop tray menu, Windows shortcuts, notifications.
- Repository and portable EXE keep the spelling `CodexCredits`.
- Android application ID **must remain** `com.christorng.androidcodextools`; keep the native Java package/JNI ABI, original AlarmManager/JobScheduler, credentials, schedule and usage history.
- Original Android signer SHA-256: `177f5e3fed59854e4203dafe13490a75c2438b37731ff0e7af7acbfe45a8e679`. Strictly increase versionCode with each APK release.
- Windows identifier `com.christorng.codexcredits` and internal NSIS `productName: CodexCredits` remain unchanged to preserve the installed path/upgrade identity; use the supported NSIS `installerHooks` macro to rename visible Start Menu/desktop shortcuts and the uninstall registry DisplayName, plus the window title and tray text.
- Never delete user data or suggest uninstall/clear-data merely to rename.

## Android signed APK distribution

- The original private Android signing key is currently in a **legacy repository-specific Actions cache**, not a CodexCredits repository secret. The compile-only Android workflow does **not** generate a distributable, compatible upgrade APK.
- **v2.0.12 is already signed and published** with the original certificate. Its signed APK was verified SHA-256 `a7b9c458be8e0d5a15e9c7b2efdc96634481fe43a85a8a796b3e6afcc7a1aeaa`, size 10,259,111 bytes, and is mirrored through `.github/workflows/android-apk-mirror.yml` to `https://raw.githubusercontent.com/ChrisTorng/CodexCredits/apk-download/CodexCredits-Android.apk`. Mirror run 41036474718 succeeded and its `VERSION.txt` matches v2.0.12; mobile download behavior not yet confirmed.
- For future Android signing entirely in this repo, securely migrate the original keystore into the `ANDROID_UPGRADE_KEYSTORE_BASE64` Actions secret in this repository, then run `.github/workflows/android-signed.yml`.
- The workflow verifies the actual signer, application ID, versionCode, and JNI methods. Confirm the workflow and artifact are successful before sharing an Android installation link.
- Historical download failure: direct GitHub Release asset URLs stalled at 'Downloading...' in the ChatGPT Android in-app browser; a sandbox APK once showed 'Library file not found'. The user previously succeeded by opening the specific successful signed GitHub Actions run and downloading the artifact. Prefer that precise run URL once available. Do not present an old APK mirror as the current version.
- Don't claim an unsigned/other-key APK is installable over the old app. After the migration, verify the signer is still the original SHA-256 and old app data is retained. The existing signed v2.0.12 is available independently of the future migration.

## Windows

- `.github/workflows/windows.yml` runs TypeScript/Rust tests and builds portable `CodexCredits.exe` and NSIS setup.
- Windows native toasts require the installed package identity; the portable EXE is not a reliable native-toast substitute.
- Preserve cross-platform functional parity through the shared UI and Rust backend. Real account actions, native alarms, tray behavior and sleep/wake require device-specific regression tests.
