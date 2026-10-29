# Agent Instructions

These instructions apply to CodexCredits, including Android builds and release delivery.

## Android APK delivery — mandatory (read before giving any download link)

### Known failure modes

- On 2026-10-22 and 2026-10-29, direct GitHub Release asset URLs (`https://github.com/.../releases/download/...apk`) opened through the ChatGPT Android in-app browser showed the full byte count, but stayed at **Downloading...** without finishing.
- On 2026-10-29, a ChatGPT sandbox APK link (`sandbox:/mnt/data/CodexCredits-Android-v2.0.8-arm64.apk`) instead failed with **Library file not found**, even though that path existed and the APK hash matched the release.
- **Successful delivery can be implicit.** Following ChatGPT APK deliveries between 2026-10-21 and 2026-10-24, the user reported installing and running the app, signing in, testing manual triggering, and reporting UI behavior. These follow-up tests are valid evidence that download/install succeeded without saying the words 'download succeeded'. Later, v2.0.8 sandbox attachment failed with 'Library file not found', so old successes do not guarantee current attachment delivery.

### Required build and verification

1. Produce the APK from the intended CodexCredits commit with the original, upgrade-compatible Android signing identity. Confirm that the matching signed-release Actions run succeeded and verified package ID, versionCode, signing certificate and JNI methods.
2. Verify actual APK byte size, ZIP package structure and SHA-256 against the release asset; do not confuse a signed APK with the Actions ZIP artifact.
3. **Prioritize the delivery path the user has actually tested.** On 2026-10-29 the user confirmed successfully downloading and installing the Android APK by opening the successful signed GitHub Actions run page and selecting its download link. Provide the exact successful signed Actions run URL for each new version first. Do not send the user through ChatGPT Library or manual ZIP extraction as the normal installation workflow. A genuine one-tap direct APK link is welcome as an additional option only after its behavior is confirmed on their phone.
4. Direct-download endpoint (not GitHub Releases asset redirect):

   https://raw.githubusercontent.com/ChrisTorng/android-codex-tools/apk-download/CodexCredits-Android.apk

   The dedicated apk-download branch in ChrisTorng/android-codex-tools contains a signed APK named CodexCredits-Android.apk and VERSION.txt. The automation is .github/workflows/direct-apk-download.yml; it fetches the published signed release, verifies the official SHA-256 and copies its bytes to that branch. On 2026-10-29 its Git blob hash was confirmed identical to the v2.0.8 signed APK. Mobile download behavior of this new endpoint has not yet been confirmed by the user; do not claim otherwise.
5. **For every newer release**, explicitly dispatch direct-apk-download.yml with that version's existing signed release tag. GitHub GITHUB_TOKEN-generated release events may not trigger another workflow automatically. Confirm the mirror run succeeded, VERSION.txt indicates the intended version, and the Git blob hash/byte size match the signed APK. Only then share the fixed latest-APK URL.
6. A sandbox:/mnt/data/*.apk chat attachment is an acceptable alternative when the active conversation genuinely registers a retrievable file. APKs attached earlier were followed by successful installation tests; the later v2.0.8 link failed with 'Library file not found'. Never repeatedly give a broken sandbox path or mistake file existence in a runtime for mobile attachment availability.
7. Do not provide a github.com/.../releases/download/...apk direct GitHub Release asset URL as the main mobile link: it stalled at 'Downloading...' after reaching 100%. The GitHub Release page is suitable for release notes and history, not as the default download process.
8. Do not send the user through ChatGPT Library or make them manually download and unzip an artifact. If the one-tap static link is also broken on the phone, diagnose and repair direct hosting.
9. User reports of successfully running/testing the newly provided build count as implicit confirmation of successful download and installation. CI success alone does not.

### User-facing download links

- Primary verified delivery: link directly to the successful version-specific signed GitHub Actions run, not merely the workflow list; the user confirmed this workflow page method worked.\n- Optional one-tap raw APK: https://raw.githubusercontent.com/ChrisTorng/android-codex-tools/apk-download/CodexCredits-Android.apk (only after verifying the expected mirror version; mobile behavior has not yet been confirmed).
- Optional freshly registered ChatGPT APK attachment when it is retrievable.
- Release history: https://github.com/ChrisTorng/android-codex-tools/releases
## Android upgrade compatibility

- Android application ID: `com.christorng.androidcodextools`.
- Keep the original Android signing certificate (SHA-256 `177f5e3fed59854e4203dafe13490a75c2438b37731ff0e7af7acbfe45a8e679`) and strictly increase `versionCode` to allow in-place upgrades.
- Preserve the original Java `AlarmManager`/`JobScheduler` integration, account data, settings and history unless specifically asked otherwise.
- Prefer installation over the previous APK with no uninstall, no Clear Data, and no account reset.
- Verify that the published APK was built from the intended CodexCredits commit and matches its stated version.

## Release response checklist

Signed workflow success, original signing identity and version checks, verified APK SHA-256 and size, matching direct mirror version and Git blob; signed version-specific Actions run link without ChatGPT Library or ZIP extraction; add the verified raw mirror as optional. Recognize implicit successful installation from subsequent user testing, but do not claim an untested new hosting endpoint has been proven on the user's phone.
