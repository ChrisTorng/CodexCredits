# CodexCredits — Android 升級相容性與跨平台報告

## 目標與來源

- 舊版行為基準：`ChrisTorng/android-codex-tools`，固定來源 `7055017ca542b66db8716b458ab8534aa563ebc8`。
- 新版原始碼：`ChrisTorng/CodexCredits`，`android-parity` 分支，Rust/Tauri 2 + TypeScript/ECharts + SQLite。
- Android 必須維持 `applicationId = com.christorng.androidcodextools`，`versionCode = 2000000`，簽名憑證 SHA-256 `177f5e3fed59854e4203dafe13490a75c2438b37731ff0e7af7acbfe45a8e679`。沒有原始簽章不得發佈升級版。

## 共用核心與 OS 邊界

| 責任 | Windows | Android | 預留 Linux |
|---|---|---|---|
| 額度 API、OAuth 協議、時間與配額運算 | Rust 共用 | Rust 共用 | Rust 共用 |
| 狀態/排程/歷程/設定四頁 | TypeScript / ECharts | 同一份 TypeScript / ECharts，Android WebView | 同一份 UI |
| 本機資料、歷程 | SQLite `codexcredits.sqlite3` | SQLite + Android 舊版 SharedPreferences 一次性匯入 | SQLite |
| 安全憑證 | Windows Credential Manager / keyring | Android Keystore + Java JNI adapter | Secret Service/keyring 適配 |
| OAuth 開啟瀏覽器 | Desktop `webbrowser` | Android Intent `ACTION_VIEW` | desktop adapter |
| 系統工作與通知 | tray + 行程內計時器 | 原版 AlarmManager + JobScheduler + BootReceiver + Android 通知 | 待設計 systemd user timer |
| 應用程式生命週期 | 關閉視窗隱藏至 tray | Android Activity / Doze lifecycle | 桌面行程 |

## 功能對照

| 功能 | 方式 | 自動化/實機驗證 |
|---|---|---|
| 5h/週配額狀態，剩餘百分比，理想及實際曲線 | Rust API + 共用介面 | Windows 單元測試/Android 編譯；實機尚需核對 |
| OAuth PKCE、refresh、帳號 ID | Rust 共用 + Android Intent/Keystore | Android 編譯；實機登入尚未測試 |
| 手動觸發、用量檢查 | Rust 共用 | 需要真人同意後的端對端測試 |
| 星期錨點、今天/明天覆寫、自動接續 | 共用設定模型；Android 使用原始 Java 背景執行引擎 | 需實機測試 Doze、精確鬧鐘權限、重開機 |
| Banked Reset / 自動待命 | Rust UI + Android 原始 Scheduler/Job | 需真實帳號與額度驗證；避免兩平台對同帳號同時排程 |
| 歷程及事件圖 | 共用 SQLite、舊資料轉入 | 靜態資料可匯入；背景新寫入的舊資料同步仍待加強 |
| 主題、採樣頻率、ntfy、系統通知 | 共用 UI，Android SharedPreferences 同步 | 須實機測試通知、網路切換及電量政策 |

## Android 原地更新與資料處理

使用者應直接安裝相容簽名且較高版本代碼的 APK，**不要先移除舊版**。安裝保留原 package 私有的 `SharedPreferences` 與 Android Keystore alias `codex_tools_aes`。第一次執行時，JNI `LegacyBridge.exportLegacy` 擷取 `app` prefs、`secure_tokens` 的加密 Token；Rust `legacy::import` 轉換排程、主題、採樣、ntfy、歷史採樣與事件，寫入 SQLite Transaction，最後以 `android_legacy_import_v1` 標示成功。舊資料原封不動留存，不應刪除或重設加密金鑰。

注意：匯入前原版歷史百分比是「已使用」；新 SQLite 使用「剩餘」，轉換式為 `100 - used`。

原始 Android `Scheduler` 和 `BootReceiver` 在 APK 內延續支援背景計時及重啟恢復；Android 不啟用桌面 Rust 排程執行迴圈，避免重複的額度觸發。此架構維持功能責任邊界，但編譯通過不表示已在真機上驗證所有背景觸發和資料流。

## 體積與效能

- 舊版 Android 原始 `app-debug.apk`（GitHub Actions run `39514470634`）：**1,445,499 bytes，約 1.38 MiB**；檔案已從 Artifact 解壓驗證。
- 新版 Android ARM64 **Debug APK**：173,204,573 bytes（run 39512487363），包含大量 Rust 原生除錯符號。
- 新版 Debug GitHub Actions artifact **ZIP**：48,384,073 bytes（同 run），不等於 APK 大小。
- 新版 Android `v2.0.0` ARM64 Release 簽章 APK（run `39576035103`）：**2,888,315 bytes，約 2.75 MiB**；SHA-256 `3b15ca6c101d59cd20f9d53aaeb6c3739a1d897b3e9a11e891a63d0df4cb9999`。相對舊版約 **2.0 倍／增加約 99.8%**。
- Release Rust profile 使用 `opt-level=s`, `lto=thin`, `codegen-units=1`, `strip=symbols`, `panic=abort`，且 Android Rust 停用每 2 秒輪詢，交由原生 JobScheduler/AlarmManager 背景運作。
- **尚無可比的真機冷啟動時間、穩態 PSS/RSS、耗電、背景喚醒耗時**。不可宣稱新架構比原生 Java 快或省記憶體；WebView 與 Rust/SQLite 有額外固定成本，體積大於 Java 原版是預期結果。

建議用 Android Studio Profiler、`adb shell dumpsys meminfo com.christorng.androidcodextools`、`adb shell am force-stop` + launch time、以及固定帳號網路情境比較前後版本：冷啟動 20 次（中位數、p95）、待機前景/背景 PSS、APK bytes、安裝後 app data、24 小時任務準點率與耗電量。應區分 Debug/Release、首次 OAuth 登入後以及資料遷移後的冷啟動。

## 發行與回歸檢查

- 簽署應由**原始 Android repo GitHub Actions 原始 keystore cache** 完成，發佈前由 `aapt` 驗證 package/version，由 `apksigner` 驗證憑證 SHA-256；簽章不符時拒絕產出升級版。
- 原始 repo 可經 GitHub Release 提供直接下載 APK，不需自行解壓縮 ZIP。
- 實機至少要核對舊資料與 OAuth、配額值、錨點/覆寫、Doze、鬧鐘權限、重開機、系統通知及 ntyf/網路故障；Banked Reset 及觸發請避免在實機上不小心消耗額度。
- 在完成上述測試以前，此分支與產出的 APK 都應視為**預覽版**，不可宣稱正式功能完全等價。

## 2026-10-21 發行驗證

- 簽署建置：[Actions #39576035103](https://github.com/ChrisTorng/android-codex-tools/actions/runs/39576035103) 全部步驟成功；原版 signing key SHA-256 檢查、`aapt` package/version 與 `apksigner` 憑證驗證均成功。
- 原地更新條件：`com.christorng.androidcodextools`、`versionCode=2000000`、沿用原始簽章 SHA-256 `177f5e3fed59854e4203dafe13490a75c2438b37731ff0e7af7acbfe45a8e679`；請勿先移除舊版。
- [Release 直接下載 APK](https://github.com/ChrisTorng/android-codex-tools/releases/download/codexcredits-android-v2.0.0-preview/CodexCredits-Android-v2.0.0-arm64.apk)。
- 成功編譯及簽章不等同實機功能完全等價；尚須驗證舊設定/歷程/OAuth 登入、Doze/重開機、通知與排程副作用。
