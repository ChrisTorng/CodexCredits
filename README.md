# CodexCredits

Windows-first Rust + Tauri 2 Codex quota monitor (MIT).

**Current phase: read-only monitoring MVP.** Includes Windows tray/close-to-hide/quit-to-stop, usage and history graph, SQLite persistence, Windows Credential Manager segmented token storage, browser OAuth PKCE login and refresh, configurable sampling and Rust scheduling *model*. Automatic Codex requests, Banked Reset and ntfy sending **are not yet implemented**. OAuth PKCE login/refresh is implemented, but live browser login with an actual account remains to be verified on Windows. This is not feature-parity with the original Android app.

## Build on Windows
Install Node.js 22+, Rust MSVC, Microsoft C++ Build Tools and Windows SDK.

```powershell
npm install
npm run tauri dev
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --no-bundle
```

The [Windows GitHub Actions workflow](https://github.com/ChrisTorng/CodexCredits/actions/workflows/windows.yml) uploads one `CodexCredits-Windows-x64` artifact. Download and unzip it once to get `CodexCredits.exe`, `README.md` and `LICENSE`. The resulting portable EXE uses Windows WebView2, without .NET runtime.

Security: OAuth access/refresh tokens stay in the OS credential vault in size-bounded chunks, not SQLite. No fixed ntfy ID. Scheduler is forced Off until automated requests are correctly implemented and tested. Closing the window keeps polling; Quit terminates app and stops polling. Backend Codex API is unofficial and subject to change. Original [android-codex-tools](https://github.com/ChrisTorng/android-codex-tools) repo is unchanged.

MIT Copyright (c) 2026 ChrisTorng.

## Login on Windows

Press **使用 ChatGPT 登入**; complete the browser sign-in within five minutes. The local OAuth callback binds only to `127.0.0.1` on an OS-selected port and checks PKCE/state. The app stores its credential record in Windows Credential Manager as short chunks (the Windows single-item UTF-16 limit still applies). The application automatically refreshes tokens as needed; never paste credentials into GitHub issues. This is the same authorization client ID used by the original Android app, which relies on a service-specific OAuth interface that might change. The application has no remote callback server.
