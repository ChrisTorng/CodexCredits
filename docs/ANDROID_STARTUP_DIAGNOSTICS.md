# Android 啟動問題修復與診斷（v2.0.4）

## v2.0.4 Release 原生 JNI 缺失修正

Android 15 x86_64 **Release** 模擬器重現 v2.0.2 啟動閃退，crash buffer 紀錄：

```text
java.lang.NoSuchMethodError:
  no static method "Lcom/christorng/androidcodextools/LegacyBridge;.startupCheckpoint(
    Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;"
```

這解釋了 Debug 模擬器可以進到 `61 local-state-loaded`，而 Release APK 卻閃退：Release Dex 缺少由 Rust 透過 JNI 動態呼叫的方法。v2.0.4 為 `LegacyBridge` 的所有 JNI 公開靜態方法加 `androidx.annotation.Keep`，加上 `jni-keep.pro` 的 `-keep` 條件，並在 JNI 錯誤時先清除 Java pending exception，以免一個診斷節點問題導致行程結束。

另新增 `mobile/android/check_dex_jni.py`，於 Release APK 編譯完成後檢查 `classes*.dex` **實際包含**所有由 Rust 呼叫的原生橋接方法名稱；不能只以編譯成功當作放行條件。

此版同時保留第一頁顯示目前／上次執行版本、最後 16 個啟動節點與可選的單次略過舊資料匯入模式。



## v2.0.3 第一頁版本與啟動歷程

- 「目前執行版本」：從 Android PackageManager 讀取 **實際安裝** 的 versionName 與 versionCode（v2.0.4 = 2000004），不是憑檔名推定。
- 「上次啟動版本」：上一次進入原生診斷首頁時保存的版本；從 v2.0.3 **首次啟用追蹤**，第一次若沒有舊紀錄會明確顯示「無紀錄（較早版本未提供追蹤）」。無法由 Android API 憑空得知未記錄的上版版本。
- 「上次不同版本」：跨版本升級時保存的最近前一個不同版本，後續同版反覆進入不會清空。
- 「Android 套件」：PackageManager 用來讀取的套件識別。
- 「上一輪經過的初始化節點」：保存最近 16 個階段，時間順序與最大階段分開呈現。先前主 Activity 的 `44` 可能在 Rust `57` 之後才寫入，將 `57` 覆蓋；新版已修正這個競態，較低的階段不再覆蓋較高的紀錄。
- 額外提供「進階測試：只開主程式（本次略過舊資料匯入）」按鈕，用來區分啟動錯誤是否源自歷史匯入。**僅本次**有效，不修改 `android_legacy_import_v1` 完成旗標、不清除資料；下次一般啟動仍會匯入。

診斷的版本及節點紀錄與原始 `SharedPreferences("app")`、`secure_tokens` 分開儲存在 `codexcredits_startup_diagnostic` 中，僅含版號與常數階段，不含任何憑證。

## v2.0.2 根因與修正

實機 v2.0.1 檢查：\`SharedPreferences\` 37 欄位、\`usage_samples_v1\` 582 筆、
\`usage_history_events_v1\` 3 筆、每週排程 1 筆、Android Keystore 4 個可解密欄位，
且 \`LegacyBridge.exportLegacy()\` 執行成功。上次啟動最後停在
\`44 native-sampling-ready\`，沒有到達 Rust \`.setup()\` 的 \`50\`。

程式碼發現誤把 \`#[cfg_attr(mobile, tauri::mobile_entry_point)]\`
套在 \`android_platform_status()\` 命令而非 \`pub fn run()\`。
Tauri 官方巨集會從所標註函式產生 Android 進入點，故可編譯出 APK，
但 Android 端沒有執行建立 Builder 的 \`run()\`，無法初始化主畫面。
v2.0.2 將巨集移至 \`run()\`，補上 \`45 rust-run-entry\` 標記、
\`mobile/android/check_entrypoint.py\` 靜態驗證，
並強化模擬器檢查，必須讀到 Rust 完成 \`.setup()\` 的節點，
不能再僅以程序存活判斷成功。

v2.0.2 的 Android App ID 和原始簽章均維持不變，
版號為 2000002，可直接從 v2.0.1 或 v2.0.0 覆蓋安裝，
不必清除資料。實機端仍需要確認 UI 與原始設定完整性。



此含診斷功能的版本是為了排查「原版 Android App 覆蓋升級後，開啟 CodexCredits 立即閃退」問題。它不是確認已修復閃退的正式版。

## 安裝

- 安裝與舊版相同 package ID `com.christorng.androidcodextools`、相同簽章憑證的 `CodexCredits-Android-v2.0.4-jni-fixed-arm64.apk`，可以原地更新 v2.0.0（版本碼由 2000000 增至 2000004）。
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
| `45` | Android Rust 入口呼叫真正的 `run()`（發生順序可能與 Activity 回呼交錯） |
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

- `ChrisTorng/android-codex-tools/.github/workflows/codexcredits-startup-fix.yml`：以原 App 簽章建置 ARM64 升級 APK，驗證包名與簽章，再附加至獨立 diagnostic GitHub Release。
- `ChrisTorng/android-codex-tools/.github/workflows/codexcredits-diagnostic-emulator.yml`：於 Android 15 x86_64 模擬器確認原生診斷頁、Tauri 首次/第二次啟動及使用仿造舊版 SharedPreferences 的資料匯入。模擬器無法等同實機 Keystore、原先加密 Token 或實際 ARM64 OEM 環境。
