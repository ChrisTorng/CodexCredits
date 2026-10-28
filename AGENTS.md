# Agent Instructions

These instructions apply to work in the CodexCredits repository, including its Android builds and release delivery.

## Android APK delivery — mandatory

**Do not give the user a GitHub Release asset's direct `releases/download/...apk` URL as the main APK download link.** In the ChatGPT Android in-app browser, it has repeatedly shown all bytes downloaded but remained stuck at `Downloading...` without completing. This happened with this project on 2026-10-22 and again on 2026-10-29. Do not wrap that URL in a download button or use `GenUI.openUrl` as a workaround.

The confirmed successful delivery method from previous conversations (v2.0.5, v2.0.6 and v2.0.7) is to **retrieve the signed GitHub Actions artifact, extract the APK, and attach the APK itself as a ChatGPT sandbox download link**. This is the required workflow whenever the user requests a testable/installable Android APK:

1. Build from the intended CodexCredits commit and run the Android checks. The signing/release workflow is in `ChrisTorng/android-codex-tools/.github/workflows/codexcredits-vNNN-release.yml` (the original repository retains the upgrade-compatible signing identity). Confirm the *specific* signed-release GitHub Actions run finished successfully, not just the unsigned compilation workflow.
2. Retrieve the `CodexCredits-Android-vX.Y.Z-Signed-Release` artifact from that completed signing run using GitHub tools. This artifact is a ZIP container, **not** the APK for installation.
3. Materialize/download the ZIP into the active chat's container, verify the ZIP, and extract `CodexCredits-Android-vX.Y.Z-arm64.apk` into `/mnt/data/`. Confirm that the exact APK file exists in this *current conversation session*. Do not infer an attachment path or reuse a sandbox URL from another session.
4. Check that the extracted APK is a valid ZIP-based Android package; compute its exact byte size and SHA-256. If the corresponding GitHub Release asset exists, check that its published asset size and SHA-256 digest match the extracted APK. The signing workflow must verify the APK package name, versionCode, original signing certificate and JNI entry points. Do not claim a check was performed unless it actually passed.
5. **Give the user a direct sandbox link to the extracted `.apk` itself**, for example `[下載 Android vX.Y.Z APK](sandbox:/mnt/data/CodexCredits-Android-vX.Y.Z-arm64.apk)`. The APK link is the primary deliverable, not a ZIP or a GitHub URL. The GitHub Release *page* (`/releases/tag/...`) or Actions run page may be provided separately for provenance/release notes, but never as a replacement for the extracted APK.
6. If the signed artifact cannot be downloaded or the container file cannot be verified, explicitly report the problem. **Never invent a sandbox link, present a ZIP as an APK, or fall back to the previously broken direct GitHub asset download as the main link.**

## Android upgrade compatibility

- The Android package ID is `com.christorng.androidcodextools`.
- Upgrades must use the original Android signing certificate (SHA-256 `177f5e3fed59854e4203dafe13490a75c2438b37731ff0e7af7acbfe45a8e679`) and monotonically increasing `versionCode`. Keep the original Java `AlarmManager`/`JobScheduler` integration and signing identity intact unless a change is specifically requested.
- Prefer **installing the new APK over the existing version**, preserving credentials, settings and usage history. Never instruct the user to uninstall, clear app data, or install a differently signed APK as routine upgrade instructions.
- When modifying the Android app, ensure the published APK is produced from the intended CodexCredits commit and corresponds to the stated version. The Android publication workflow and original signing cache live in `ChrisTorng/android-codex-tools`, while the current cross-platform implementation is here.

## Release response checklist

Before replying with an APK download: signed release succeeded; extracted APK exists at its exact `/mnt/data/` path; package and signature checks passed in CI; byte size and SHA-256 were checked; sandbox APK link is provided; no direct `releases/download/...apk` link is used as the primary download.
