# Android v2.0.1 啟動診斷說明

此診斷版是為了排查「原版 Android App 覆蓋升級後，開啟 CodexCredits 立即閃退」問題。它不是確認已修復閃退的正式版。

## 安裝

- 安裝與舊版相同 package ID `com.christorng.androidcodextools`、相同簽章憑證的 `CodexCredits-Android-v2.0.1-diagnostic-arm64.apk`，可以原地更新 v2.0.0（版本碼由 2000000 增至 2000001）。
- **不要清除資料、解除安裝，或關閉既有排程。** 診斷工具不修改原本 `app`、`secure_tokens` 設定或刪除資料。
- 啟動時先出現原生 Java 診斷頁，而不是 Tauri 主畫面。沒有自動繼續；請自行逐項點擊。

## 四步操作

1. 「查看舊版資料項目」：只統計 `SharedPreferences("app")` 欄位數與資料型態，不顯示欄位內容。
2. 「檢查 JSON 格式」：唯讀解析使用量採樣、歷史事件、每週規則、每日覆寫的 JSON，檢查根節點及筆數；任一欄位超過 200 萬字元就跳過。
3. 「檢查加密憑證與匯出橋接」：先讀取 Keystore alias 是否存在與舊版 `secure_tokens` 的欄位存在狀態，再執行與 Rust 匯入相同的 `LegacyBridge.exportLegacy()` Java 程序。對外只顯示欄位數，不顯示內容，不記錄 Token。
4. 「啟動 CodexCredits」：才啟動 Tauri Activity 與 Rust。若閃退，**重新點 App 圖示**，將上方「上次啟動最後紀錄」拍照或複製即可。

## Rust 啟動節點對照

| 紀錄 | 已走到的程序 |
| --- | --- |
| `00` | 原生診斷頁載入 |
| `01` ～ `03` | 三段舊資料診斷執行中／已結束 |
| `40` | 使用者要求啟動 Tauri |
| `41` | MainActivity 即將執行 Tauri 的 onCreate |
| `42` | Tauri Activity onCreate 返回 |
| `43` | 原生通知頻道建立成功 |
| `44` | 原生背景採樣排程初始化返回 |
| `50` | Rust `.setup()` 進入 |
| `51` | App 資料目錄建立 |
| `52` | SQLite 開啟成功 |
| `53` | Rust 舊版資料匯入開始 |
| `54 ... ok` | Rust 舊版資料匯入返回成功 |
| `54 ... error` | Rust 舊版資料匯入返回錯誤；錯誤本身已被轉成狀態 |
| `55` | 本機設定已載入 |
| `56` | Rust 應用程式狀態已建立 |
| `57` | Rust setup 完成 |
| `60` | WebView 前端開始載入本機資料 |
| `61` | WebView 本機資料載入及初次渲染完成 |
| `62` | WebView 本機資料載入失敗，前端改走錯誤處理 |
| `99` | Java 未處理例外；同一行附帶例外類型和最後節點 |

節點寫入 Android app 私有的 `codexcredits_startup_diagnostic` SharedPreferences（僅常數代碼，沒有 Token、帳號、檔案內容）。每次打開診斷頁前會讀取前一次最後節點，再記錄新的 `00`。JNI 與 native 層突然中止不一定有 Java Exception，仍能留下前一個成功節點。

> 限制：節點是粗粒度定位，不等於錯誤堆疊。某些多執行緒初始化會造成標記順序交錯；如仍無法重現，後續可能需要針對單一節點加入進一步檢查。

## CI

- `ChrisTorng/android-codex-tools/.github/workflows/codexcredits-diagnostic.yml`：以原 App 簽章建置 ARM64 升級 APK，驗證包名與簽章，再附加至獨立 diagnostic GitHub Release。
- `ChrisTorng/android-codex-tools/.github/workflows/codexcredits-diagnostic-emulator.yml`：於 Android 15 x86_64 模擬器確認原生診斷頁、Tauri 首次/第二次啟動及使用仿造舊版 SharedPreferences 的資料匯入。模擬器無法等同實機 Keystore、原先加密 Token 或實際 ARM64 OEM 環境。
