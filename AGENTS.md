# CodexCredits agent instructions

This is the **only maintained** repository for the cross-platform Codex Credits app: `ChrisTorng/CodexCredits`.

## Scope and tooling

- Maintain all **product source code** exclusively in `ChrisTorng/CodexCredits`.
- **Original signing-key migration completed 2026-10-30:** temporarily signed and encrypted the old cached keystore using one-time workflows, migrated the original key to the **new** repo Actions Secret, verified independent signing in [CodexCredits run 41057160230](https://github.com/ChrisTorng/CodexCredits/actions/runs/41057160230), then removed the temporary old-repo workflows in commit `239000c9`. The legacy repo is now a read-only Java reference; do not modify it for future releases. Keep the legacy signing cache as backup until separate retention and recovery planning.
- Use OpenAI's official GitHub plugin for repository work, **never** request GitHub Repo Creator for this existing repository.

## Unified branding and upgrade requirements

- User-visible name on Android and Windows is `Codex Credits`: installed-app display name, Android launcher, WebView header/title, desktop tray menu, Windows shortcuts, notifications.
- Repository and portable EXE keep the spelling `CodexCredits`.
- Android application ID **must remain** `com.christorng.androidcodextools`; keep the native Java package/JNI ABI, original AlarmManager/JobScheduler, credentials, schedule and usage history.
- Original Android signer SHA-256: `177f5e3fed59854e4203dafe13490a75c2438b37731ff0e7af7acbfe45a8e679`. Strictly increase versionCode with each APK release.
- Windows identifier `com.christorng.codexcredits` and internal NSIS `productName: CodexCredits` remain unchanged to preserve the installed path/upgrade identity; use the supported NSIS `installerHooks` macro to rename visible Start Menu/desktop shortcuts and the uninstall registry DisplayName, plus the window title and tray text.
- Never delete user data or suggest uninstall/clear-data merely to rename.

## Android signed APK distribution

- The original private Android keystore has been securely migrated into `ANDROID_UPGRADE_KEYSTORE_BASE64` in **this** repository. It was verified against signer SHA-256 `177f5e3fed59854e4203dafe13490a75c2438b37731ff0e7af7acbfe45a8e679` and used for a complete signed APK build in [run 41057160230](https://github.com/ChrisTorng/CodexCredits/actions/runs/41057160230). The original signing cache in the legacy repo remains emergency backup only.
- **v2.0.13 signed APK** (10,259,047 bytes), SHA-256 `c203b35688595659d7ccb73a20a5e723bf993a794f604b43a3bf2ef3b3fe7438`, was uploaded as [Actions artifact 12988947200](https://github.com/ChrisTorng/CodexCredits/actions/runs/41078554804/artifacts/12988947200). [Mirror run 41079960885](https://github.com/ChrisTorng/CodexCredits/actions/runs/41079960885) verified the artifact digest and pushed it to `https://raw.githubusercontent.com/ChrisTorng/CodexCredits/apk-download/CodexCredits-Android.apk`. `VERSION.txt` confirms `source_repo=ChrisTorng/CodexCredits`.
- For future versions update `.github/workflows/android-signed.yml` and `.github/workflows/android-apk-mirror.yml` together (versionCode, APK name, artifact name) and validate via successful signed build and mirrored download. Do not require work in the old repo.
- The workflow verifies the actual signer, application ID, versionCode, and JNI methods. Confirm the workflow and artifact are successful before sharing an Android installation link.
- Historical download failure: direct GitHub Release asset URLs stalled at 'Downloading...' in the ChatGPT Android in-app browser; a sandbox APK once showed 'Library file not found'. The user previously succeeded by opening the specific successful signed GitHub Actions run and downloading the artifact. Prefer that precise run URL once available. Do not present an old APK mirror as the current version.
- Don't claim an unsigned/other-key APK is installable over the old app. Verify every new version has the original signer SHA-256, package name and strictly increased Android versionCode; existing on-device app data still needs real-device regression testing.

## Windows

- `.github/workflows/windows.yml` runs TypeScript/Rust tests and builds portable `CodexCredits.exe` and NSIS setup.
- Windows native toasts require the installed package identity; the portable EXE is not a reliable native-toast substitute.
- Preserve cross-platform functional parity through the shared UI and Rust backend. Real account actions, native alarms, tray behavior and sleep/wake require device-specific regression tests.

## Joint releases
- Every change to the product is delivered as a **paired Android and Windows version**; update package.json, Cargo.toml, Android/Windows Tauri configs and workflow versionCode / filenames together.
- Validate the original Android keystore fingerprint and signature compatibility, run the Android signed-release workflow, and verify both Android and Windows workflow artifacts before distributing.
- A successful compile-only Android job is not a signed deliverable. Keep the same app ID to preserve install upgrades and user data.
