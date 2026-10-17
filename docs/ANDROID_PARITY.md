# Android ↔ Windows fidelity matrix

Android source of truth: [ChrisTorng/android-codex-tools](https://github.com/ChrisTorng/android-codex-tools). **Do not modify the Android repository**. This matrix describes implementation and verification separately.

| Android location | Target behavior | Windows port | Verification / difference |
| --- | --- | --- | --- |
| `MainActivity.buildUi` | Title, refresh icon; ● 狀態 / ◷ 排程 / ▥ 歷程 / ⚙ 設定 | `src/main.ts`, `src/style.css` | Four vertically scrolling pages, viewport-responsive portrait window; visual Android device comparison remains |
| `MainActivity.refreshStatus` | Show **remaining**, not used, quota; plan; next schedule | `src/main.ts`, `src-tauri/src/api.rs` | Uses `100 - used_percent`; Rust unit test checks 27% used → 73% remaining |
| `RemainingQuotaView` | 5h blue / weekly orange; confirmed-window actual solid and ideal dashed; clock axis | ECharts in `src/main.ts` | Corresponding information and colors; Canvas pixel-level rendering not identical |
| `UsageHistoryView` | Yesterday / today; solid actual, dashed ideal; event markers and sample list | ECharts + SQLite history & event log | Time/date plots and global marker; secondary detailed event heuristics still require regression |
| `Scheduler.storeQuota` | Detect unconfirmed sliding reset placeholders; weekly global reset invalidation | `confirmed_primary` in `src-tauri/src/lib.rs` | Unit-level comparisons; live reset transitions still need tests |
| `ScheduleConfig` | Daily 06:00+3 default, weekly days, overrides today/tomorrow, conflict checks | `src-tauri/src/schedule.rs` and editors in TypeScript | Tests for defaults/overrides/collisions; backend rejects conflicting anchors; Windows DST and override edge cases need tests |
| `Scheduler` | Mode Off, reset Auto, anchors + actual 5h reset chained triggers, 60s tolerance and 15s grace | `src-tauri/src/engine.rs`, `src-tauri/src/lib.rs` | Rust tests for planning and stale wakeups; process-local runtime **cannot run after Quit** |
| `CodexClient.triggerMinimal` | Minimal Codex request to open usage window | `src-tauri/src/api.rs` | Explicit UI confirmation and backend quota guards; real service response requires user testing |
| `CodexClient.getResetCredits/consumeResetCredit` | Banked Reset count/expiry, exhausted/manual/armed | `src-tauri/src/api.rs` / UI | Backend revalidation; armed selection persists across restarts, but does not run while app is stopped |
| `UsageHistory.refreshGlobalResetStats` | `codex-reset.com` forecast and announced timeline median/average | `src-tauri/src/api.rs` | Public forecast/timeline, best-effort network |
| `CodexClient` PKCE/refresh | OAuth loopback, access/refresh and account header | `src-tauri/src/oauth.rs`, `credentials.rs` | Reported working on Windows; token rotation tested at unit/CI level |
| `Scheduler.testNtfy` | Optional random ntfy topic; generate/test/copy/save | `src/main.ts`, `src-tauri/src/api.rs` | No shared hardcoded ID; opt-in; actual delivery requires user test |
| `AndroidNotifications` | Android native notifications | Tauri native notification plugin with Windows setting and test | Portable EXE delivery depends on Windows notification policy |
| `AlarmManager`, `JobScheduler`, Android Doze | Exact wake-ups, backup jobs and persistent samples | In-process timer | Deliberate desktop process lifecycle difference |

## Windows manual acceptance checklist

1. Sign in through ChatGPT OAuth and verify plan and percentage match Android for the same account and time.
2. Verify the **5h and weekly figures are remaining quota** and the reset axes/ideal curves do not imply confirmation of an inactive window.
3. Verify 10/15/30/60-minute sampling, stored history, yesterday/today plot and persisted light/dark theme.
4. Verify close → tray hides but samples continue, tray Show restores, tray Quit ends process and stops monitoring.
5. Check weekly schedule and today/tomorrow override editors, disabled override and empty-day suppression. **Do not enable a credit-consuming schedule on two devices for the same account.**
6. Only with explicit consent and sufficient quota, test manual trigger, automatic schedule, and Banked Reset. They can consume quota or a stored reset.
7. Verify optional random ntfy topic is private and not shared among app installations. Check notification delivery.
8. Test sleep/resume, timezone changes, network loss, OAuth refresh errors and a system reboot. Stale scheduler actions must not be silently replayed.

Do not claim full Android behavioral parity until these Windows scenarios have passed. Unit tests and CI compiling alone are insufficient.
