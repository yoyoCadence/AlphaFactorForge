# Handoff: PR #169–#174 驗收與未解決 review comment 回答

Date: 2026-10-14（Asia/Taipei）
Repo: yoyoCadence/AlphaFactorForge
Reviewed integrated main: `b08aa38e262b0d32acd329e19ee8ac3705481724`
Status: Review complete — #169／#170／#171 通過；#172／#173 需補修；#174 實作與 release 對話框顯示通過，按確定後的退出碼仍待互動驗收。

## Summary

維護者要求驗收六個已合併快修並回答問題。六個最終 head 的 CI 為 36／36 success，
六個 merge tree 都與其最終 head 相同。但 #172 仍有兩則、#173 仍有一則未解決的 inline review comment，
本輪以三個獨立反例全部確認成立。CI 通過並未涵蓋這三個邊界，不能把六項快修全部標成完整驗收結案。

本輪只新增驗收文件、可重現的測試 patch 與任務紀錄；沒有修產品或變更計算、持久化、依賴及合約。
測試在由 integrated main 取出的隔離 archive 目錄執行，保留當前 `fix/crypto-listing-registration` 的未提交工作。
本機 `origin/main` 指向上述 integrated main。

## Per-PR result

| PR | 最終 head | merge | CI | 結論 |
| --- | --- | --- | --- | --- |
| [#169](https://github.com/yoyoCadence/AlphaFactorForge/pull/169) | `d39e5c0` | `43eea86` | [6／6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38485684360) | 通過；沿用前輪獨立驗收，本輪整合測試亦通過 |
| [#170](https://github.com/yoyoCadence/AlphaFactorForge/pull/170) | `4e1d893` | `db2cecc` | [6／6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38554973063) | 通過；樣式狀態切換與小控制項陰影的迴歸均通過 |
| [#171](https://github.com/yoyoCadence/AlphaFactorForge/pull/171) | `5df0772` | `384ba8d` | [6／6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38555989270) | 通過；價格小數位、平盤範圍與 gutter 調整符合切片範圍 |
| [#172](https://github.com/yoyoCadence/AlphaFactorForge/pull/172) | `14e54f9` | `a3554f8` | [6／6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38556918455) | 需補 R1／R2；原生非有限值同步的實機驗收仍開放 |
| [#173](https://github.com/yoyoCadence/AlphaFactorForge/pull/173) | `9cafde5` | `816bf94` | [6／6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38557896541) | sticky、關閉鈕與錯誤保留通過；相同訊息的倒數需補 R3 |
| [#174](https://github.com/yoyoCadence/AlphaFactorForge/pull/174) | `5bedac5` | `b08aa38` | [6／6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38558891887) | release 真實對話框已確認；OK → exit 1 尚未實測 |

review thread 查詢：#169／#170／#171／#174 各 0；#172 有 2、#173 有 1，均仍 unresolved。
以下是對這三則 comment 的獨立回答。GitHub bot 都標 P2；本輪按影響範圍列為 P3 顯示／回饋問題，
未改變回測計算或結果保存，但仍應補修後才關閉對應快修的驗收。

## R1 — #172：必須檢查每個顯示中的 Holdout 欄位（成立）

對應 [review comment](https://github.com/yoyoCadence/AlphaFactorForge/pull/172#discussion_r4270200134)。

[ResultsSection.tsx:78](https://github.com/yoyoCadence/AlphaFactorForge/blob/b08aa38e262b0d32acd329e19ee8ac3705481724/alpha-factor-forge/src/components/ResultsSection.tsx#L78)
只以全期 dataset 的 start/end 判斷 `shortDays`；當全期超過一年而樣本內／外不足一年，提示消失。

本輪重現：匯入 600 根連續日線，日期從 2024-01-01 起，啟用 30% Holdout，執行回測。
全期首尾相距 **599 天**，樣本外相距 **179 天**，畫面顯示三欄年化值，但 `annualization-note` 數量為 **0**。
樣本外 CAGR = 12.30%、Sharpe = 4.61；測試要求提示可見而失敗。放大面板也沒有提示。

修正條件：從已完成 run 的各欄位取得其實際使用的樣本量／年化基準，
任一顯示欄位不足一年就提示，並指出哪一欄；不能從正在編輯的資料集或 Holdout 草稿推導。
主表、放大面板和原生績效視窗都應攜帶並呈現相同的顯示資訊。
目前 `MetricsWindowSnapshot`／wire 不含樣本期間資訊，
[MetricsPopoutWindow.tsx:44](https://github.com/yoyoCadence/AlphaFactorForge/blob/b08aa38e262b0d32acd329e19ee8ac3705481724/alpha-factor-forge/src/components/MetricsPopoutWindow.tsx#L44)
只渲染表格，因此原生提示也需補上；這是源碼確認，未冒充原生互動實測。

## R2 — #172：一年邊界應與引擎的年化基準一致（成立）

對應 [review comment](https://github.com/yoyoCadence/AlphaFactorForge/pull/172#discussion_r4270200142)。

[sampleSpan.ts:18](https://github.com/yoyoCadence/AlphaFactorForge/blob/b08aa38e262b0d32acd329e19ee8ac3705481724/alpha-factor-forge/src/services/sampleSpan.ts#L18)
用最後一根與第一根的開盤時間差；
[metrics/index.ts:98](https://github.com/yoyoCadence/AlphaFactorForge/blob/b08aa38e262b0d32acd329e19ee8ac3705481724/alpha-factor-forge/src/core/metrics/index.ts#L98)
以 `totalBars / barsPerYear` 計算年化年數。

本輪重現：365 根連續 `1d`、不啟用 Holdout。引擎年數為 **365／365 = 1**，
時間戳差卻是 **364 天**；畫面實際顯示「樣本約 364 天，不足一年…」，測試要求無短樣本警告而失敗。
8,760 根 `1h` 亦有相同一根間距的偏差（源碼推導，本輪未另跑該大案例）。

建議與 R1 同一個小修 PR：提示的「不足一年」判斷共用已完成 run 的年化基準，
而不是兩套彼此不同的定義。若另外顯示首尾時間戳跨度，應明確區分它與引擎的年化基準。
不要在這個顯示修正中修改 CAGR／Sharpe 計算、既有 interval fallback、parity fixture 或統計合約。
補 364／365／366 根日線、8,759／8,760／8,761 根小時線及 Holdout 的逐欄邊界案例。

## R3 — #173：重複相同通知必須重設 8 秒倒數（成立）

對應 [review comment](https://github.com/yoyoCadence/AlphaFactorForge/pull/173#discussion_r4270267207)。

[BacktestPanel.tsx:137](https://github.com/yoyoCadence/AlphaFactorForge/blob/b08aa38e262b0d32acd329e19ee8ac3705481724/alpha-factor-forge/src/components/BacktestPanel.tsx#L137)
的 effect 只依賴 `msg` 字串；再次 `setMsg` 同字串時 React 不更新狀態，舊 timer 繼續倒數。

本輪以 Playwright clock 重現：掃描後點值 7 的格子，推進 **7 秒**，再次點同一格，
再推進 **2 秒**，訊息數量為 **0**。第二次操作才過 2 秒，提示已被第一次的 timer 清掉。
既有「單次訊息 8 秒消失」測試通過，但未涵蓋這個重複通知。

建議另開小修 PR：每次發出通知建立新的 ID／generation，依該 ID 重設 timer；
清理舊 timer，保持訊息文字、`role`、關閉鈕及錯誤不自動消失的行為。
補相同文字重複、不同文字替換、手動關閉及較舊 timer 不得清除新通知的測試。

## 截圖中兩項原生驗收缺口的回答

### #172 原生 pop-out 的非有限值

編解碼方向正確：每欄共用 `encodeMetrics`，接收端恢復 `Infinity`／`-Infinity`／`NaN`，
null／缺省欄及清空事件的行為有單元涵蓋，本輪全部通過。沒有改 pinned 的 `metricsCodec.ts`。

但單元測試 mock 掉 Tauri API，不能證明真正的視窗建立、ready/listen、capabilities 和事件交付正常。
本輪 Windows 操作連線重試仍失敗：`Computer Use native pipe is unavailable ... (os error 2)`。
Browser 連線亦等待逾時；改用專案既有 Chromium E2E 完成非原生驗收。
因此**原生 `∞` 同步仍未驗收完成**，保留任務，不以 CI 的 debug native smoke 或 mock child route 取代。

修正 R1／R2 後的原生驗收：使用固定、非研究 Test 保留資料的上漲測試資料，零手續費／滑價，
選擇可產生正報酬、無虧損交易的策略；實際開新績效視窗，確認 full／in-sample／out-sample 的
Sortino、Calmar、Profit Factor 與主表一致顯示 `∞`；重新回測、清空失效結果及關閉重開亦一致。
同時檢查短樣本提示。`−∞`／NaN 可用受控測試事件補驗，不能把它當成真實保存結果。

### #174 release 啟動錯誤訊息框

這次已由 integrated main 正式建置：
`cargo build --locked --offline --release --features custom-protocol --bin alpha-factor-forge`。
在全新的暫存 workspace 建立 `schema_migrations` 含 `9999_acceptance_future` 的 SQLite，
以 `AFF_DATA_DIR` 指向該目錄後啟動這個 release binary。

實際確認：行程的視窗標題為 **AlphaFactorForge 無法啟動**；
藉截圖技能擷取該行程的 window handle，畫面有錯誤圖示、正確中文更新版本建議、
`0010_campaign_admission`／`9999_acceptance_future` 技術原因、隔離工作區路徑與「確定」鈕。
所以「release 對話框尚未看過」這項缺口已補到畫面證據。

因 Windows 操作連線不可用，沒有按「確定」並量測正常退出碼；最後停止自建的測試行程清理。
`fail()` 在 `MessageBoxW` 返回後 `std::process::exit(1)` 的程式路徑已確認，
但不是本輪實測的 exit 1，這項互動驗收繼續保留。
其他失敗原因與 WebView2 缺少情境未逐一實機觸發。

## Verification and reproducible artifacts

| 本輪命令／檢查 | 結果 |
| --- | --- |
| 隔離 head 的 `npm ci --offline --no-audit --no-fund` | 通過 |
| `npm run typecheck`／`npm run build` | 通過 |
| `npm test` | **1170／1170，72 檔** |
| `E2E_PORT=5189 npm run e2e -- --workers=1` | **93／93**，新增反例前的原套件 |
| `cargo build --locked --offline --release --features custom-protocol --bin alpha-factor-forge` | 通過 |
| `cargo test --locked --offline --release --bin alpha-factor-forge desktop::startup_error::tests` | **6／6** |
| `cargo test --locked --offline --release --bin alpha-factor-forge` | **434／434** 桌面 Rust 測試 |
| 三個新增驗收反例 | **3／3 預期失敗**，分別確認 R1／R2／R3 |
| GitHub final-head CI／merge trees | **36／36 success；6／6 相同** |

截圖所稱 606 Rust 是作者的完整 debug suite 數量；本輪本機 Rust 結果是上表的 release 桌面 target，
沒有把未重跑的 lib／service integration 數量加到本輪結果。

可用 [反例 patch](2026-10-14-pr169-174-acceptance-regressions.patch) 在乾淨修正分支套用：
`git apply handoffs/2026-10-14-pr169-174-acceptance-regressions.patch`，在前端目錄執行
`npm run e2e -- e2e/acceptance-holdout-span.spec.ts e2e/acceptance-review-comments.spec.ts --workers=1`。
patch 只新增三個測試，不含修正；套用前仍應檢查當前 baseline。

畫面證據留在本機暫存 `aff-pr169-174-evidence-20261014-b08aa38/`：
`holdout-short-span.png`、`exact-year-warning.png`、`startup-dialog.png`。
它們不是追蹤檔案；文字證據與反例 patch 才是可跨工作階段使用的紀錄。

## Next step and resolution

用兩個小修 PR 處理 #172 的 R1＋R2 和 #173 的 R3，再補原生驗收。
這些是局部顯示／通知修正，沒有需要回退六個已合併 PR 的證據。
修正後追加 head、CI、反例通過結果及原生驗收；最後回覆／resolve GitHub 的三個 thread。
本輪僅在本機記錄回答，沒有發佈 GitHub comment、review 或修改 thread 狀態。

清理已完成：兩次自建 Vite server 和測試桌面行程均已停止；port 5189 無 listener。
隔離 source／Cargo target 目錄已刪除，保留上述本機畫面證據與 tracked handoff／反例 patch。

Resolution: Pending R1–R3 repairs and the remaining native interaction checks.

### Publication and development authorization（2026-10-14）

Maintainer authorized publishing/merging this review and continuing development.
The documentation PR is [#175](https://github.com/yoyoCadence/AlphaFactorForge/pull/175),
head `f1eaa39`, merged as `63bc2df` after all six CI jobs succeeded.
R1/R2 are now In Progress on `fix/metrics-annualization-notes`; R3 follows in its
own PR after that merges. Original review evidence and native limits remain.
