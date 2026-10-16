# CodexCredits

Windows-first Rust + Tauri 2 Codex quota monitor (MIT).

**Current phase: read-only monitoring MVP.** Includes Windows tray/close-to-hide/quit-to-stop, usage and history graph, SQLite persistence, Windows Credential Manager token storage, configurable sampling and Rust scheduling *model*. Automatic Codex requests, Banked Reset, OAuth PKCE and ntfy sending **are not yet implemented**. This is not feature-parity with the original Android app.

## Build on Windows
Install Node.js 22+, Rust MSVC, Microsoft C++ Build Tools and Windows SDK.

```powershell
npm install
npm run tauri dev
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --no-bundle
```

The [Windows GitHub Actions workflow](https://github.com/ChrisTorng/CodexCredits/actions/workflows/windows.yml) creates `CodexCredits-Windows-x64.zip`. The resulting portable EXE uses Windows WebView2, without .NET runtime.

Security: access tokens stay in the OS credential vault, not SQLite. No fixed ntfy ID. Scheduler is forced Off until automated requests are correctly implemented and tested. Closing the window keeps polling; Quit terminates app and stops polling. Backend Codex API is unofficial and subject to change. Original [android-codex-tools](https://github.com/ChrisTorng/android-codex-tools) repo is unchanged.

MIT Copyright (c) 2026 ChrisTorng.
