# Agent Instructions

These instructions apply to CodexCredits, including Android builds and release delivery.

## Android APK delivery — mandatory (read before giving any download link)

### Known failure modes

- On 2026-10-22 and 2026-10-29, direct GitHub Release asset URLs (`https://github.com/.../releases/download/...apk`) opened through the ChatGPT Android in-app browser showed the full byte count, but stayed at **Downloading...** without finishing.
- On 2026-10-29, a ChatGPT sandbox APK link (`sandbox:/mnt/data/CodexCredits-Android-v2.0.8-arm64.apk`) instead failed with **Library file not found**, even though that path existed and the APK hash matched the release.
- Earlier assistant replies provided sandbox APK files for v2.0.5 / v2.0.6 / v2.0.7, but no retrieved user message conclusively confirmed that the sandbox delivery method worked. **Do not claim a delivery method has been successfully tested by the user without actual confirmation.** Verifying a container path proves bytes exist in the execution container, not that the mobile attachment service can resolve them.

### Required build and verification

1. Build from the intended CodexCredits commit. Use the signed-release GitHub Actions workflow in `ChrisTorng/android-codex-tools/.github/workflows/codexcredits-vNNN-release.yml`; the original repo holds the legacy-compatible signing identity. Confirm that the specific signing workflow finished successfully, rather than relying solely on the unsigned build.
2. Obtain the `CodexCredits-Android-vX.Y.Z-Signed-Release` Actions artifact (a ZIP), not a random or unsigned APK. Download/materialize the bytes into the current execution container, and extract `CodexCredits-Android-vX.Y.Z-arm64.apk`.
3. Verify APK integrity, exact byte size and SHA-256, and match them against the release asset's published SHA-256/size when available. Ensure the signing workflow validated application ID, `versionCode`, original signing certificate, and JNI entries. Never state an unperformed check was successful.
4. If providing a ChatGPT-local file, **create/register a new user-visible attachment in the current response** and verify its exact file path. Use a real file artifact handoff when supported; do not assume that referencing `sandbox:/mnt/data/...` is enough to make a file retrievable in the ChatGPT Android UI. Prefer both a standalone APK and a separate ZIP containing the APK, clearly labeled. The ZIP must actually contain the verified signed APK. Never link to a path that wasn't created or verified in the current container session.
5. **Never claim that ChatGPT sandbox downloads are a confirmed reliable workaround.** The mobile client has shown "Library file not found." State the limitation and provide a practical independent fallback: open the GitHub **Release page** in an external browser (Chrome or Samsung Internet; use the in-app browser's `⋮` → `Open in browser` option), then select the APK under Assets. Alternatively, open the successful GitHub Actions run in an external browser and download/extract its signed-release ZIP artifact if signed in to GitHub. Do not use the direct `releases/download/...apk` endpoint as the in-app download link or wrap it in a button.
6. If both delivery methods fail, do not repeat them or falsely declare success. Diagnose where the failure occurs and consider a separate first-party static file host/CDN (for example an already authorized Cloudflare R2 custom-domain download endpoint) with a proper Android APK content type and download disposition **after confirming that the hosting resource is available and the file was actually uploaded and tested**.
7. A release is complete when the APK builds and is published; **user download success is a separate outcome** that cannot be inferred from CI success or a valid SHA-256.

### User-facing release links

- GitHub Release **page** for provenance or external-browser downloads: `https://github.com/ChrisTorng/android-codex-tools/releases/tag/codexcredits-android-vX.Y.Z-release`.
- Signed Actions run page for an artifact ZIP fallback.
- When the current conversation supports working attachments, give a freshly registered, verified APK file link and optional ZIP link. Do not reuse old conversation sandbox paths.
- If the in-app direct APK link fails, immediately switch to external-browser or other hosting options. **Do not send the user through the same broken pathway again.**

## Android upgrade compatibility

- Android application ID: `com.christorng.androidcodextools`.
- Keep the original Android signing certificate (SHA-256 `177f5e3fed59854e4203dafe13490a75c2438b37731ff0e7af7acbfe45a8e679`) and strictly increase `versionCode` to allow in-place upgrades.
- Preserve the original Java `AlarmManager`/`JobScheduler` integration, account data, settings and history unless specifically asked otherwise.
- Prefer installation over the previous APK with no uninstall, no Clear Data, and no account reset.
- Verify that the published APK was built from the intended CodexCredits commit and matches its stated version.

## Release response checklist

Signed workflow success; correct package/signature/version checks; verified APK bytes and SHA-256; actual user-accessible delivery path(s); explicit instructions for external-browser fallback; do not claim mobile download success without user confirmation.
