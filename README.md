# CodexCredits

Local-first Codex quota/credits monitoring, history, scheduling, and Banked Reset manager.

**Status:** Architecture evaluation. Windows first; planned Ubuntu and Android, potential macOS and iOS. Original [android-codex-tools](https://github.com/ChrisTorng/android-codex-tools) remains untouched.

## Product requirements

- A small Windows **system tray** app, not a hosted website.
- Closing the main window hides it to the tray, with monitoring, history collection and scheduled actions continuing.
- **Quit / Exit** from the tray terminates the application and stops *all* monitoring, sampling and scheduled actions.
- No Windows Task Scheduler, Windows Service, persistent daemon, cloud account or hosted backend is required.
- The main window may be compact or expandable. The history view supports long-term local retention and larger charts.
- Keep the app process lifecycle independent from the UI window lifecycle.
- Core rules and API client should be portable; platform-specific tray, keychain and mobile background integration are adapters.
- Persist user settings and non-secret history locally (SQLite recommended). Store OAuth tokens in OS-backed protected storage.
- Sleep/shutdown: work is suspended; on next launch/resume, reconcile state without replaying stale triggers or spending reset credits unexpectedly.
- Android and iOS cannot inherit desktop tray/lifecycle guarantees; future mobile implementations must adapt to OS restrictions.

## Technology evaluation

Two leading approaches:
1. **Tauri 2 + Rust core + TypeScript UI**, using the system WebView rather than bundling a browser.
2. **Avalonia + .NET Native AOT (C#)**, self-contained with no end-user .NET runtime requirement.

The intended first platform is Windows. Choose the production implementation after checking deployment footprint, tray behavior, and runtime constraints. **No framework is locked in by this document.**

## License

[MIT](LICENSE).
