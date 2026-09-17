# Handoff: P01 Results Explorer

Date: 2026-09-16
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P00 之後同分支續做；本機 git，GitHub 仍 401）
PR: 尚未建立
Status: P01 完成；下一個可執行 phase 為 P02（runtime 解耦，ABC-02a），須另行明確授權

## Summary

執行 `docs/plans/active-plan.md` §5 的 P01：「既有 records、summaries、trades 查詢與歷史結果
UI」，驗收「可重開查看已保存結果；缺歷史明細如實呈現；不改計算」。使用者於 P00 交接後明確
授權執行下一個 phase，並指示 GitHub 暫鎖、先用本機 git。

## 執行前驗證

- Plan 假設成立：`get_backtest_results`／`list_validation_records`／`get_validation_record` 已存在；
  **trades 從未有讀取路徑**（`tasks.md` 舊註記「a trades-reading UI stays deferred to Results
  Explorer」）；`validation_records.discovery_run_id`（0003）存在但 DTO 未暴露。
- 兩者皆屬 P01「查詢」範圍內的唯讀補充，無 migration、無寫入路徑變更、不改任何計算。
- 與 `tasks.md` 既有「Build Results Explorer UI」任務對照：Validation 排名、篩選、明細、區段對照、
  Test 隱藏在本次完成；**DSL 樹檢視**（尚無可執行 DSL，P11）與 **benchmark deltas**（僅顯示持久化的
  基準淨報酬，不重算差額）明確列為未做，記於 tasks.md。

## 修改檔案

| 檔案 | 變更 |
| --- | --- |
| `src-tauri/src/db/repositories.rs` | `list_trades(conn, summary_id)`；`ValidationRecordRow.discovery_run_id`（serde default、唯讀）；`VALIDATION_RECORD_COLS`／map 讀出；2 個測試（trades 讀回順序／未知 id／覆寫；手動紀錄 run id 為 None） |
| `src-tauri/src/commands/db_commands.rs`、`main.rs` | `get_backtest_result_detail` 命令與註冊（驗收修正前為 `get_trades`） |
| `src-tauri/src/discovery_runner/execution.rs` | 3 個 struct literal 補 `discovery_run_id: None` |
| `src-tauri/src/db/discovery_tests.rs` | runner 提交後以 typed 讀取斷言 `discovery_run_id == Some(run_id)` |
| `src/tauri-client/commands.ts` | `db.getBacktestResultDetail`、`ValidationRecordRow.discovery_run_id?` |
| `src/tauri-client/mockClient.ts` | `getBacktestResultDetail`、摘要 `created_at`、`?mock=1&seedHistory=1` 種子閘門（`afterReady`）、驗收回歸控制 `replaceBeforeDetail`／`detailDelay`／`explorerFailOnce` |
| `src/tauri-client/mockHistorySeed.ts` | 新增：以真實 composer 鏈產生 v2 bundle 的種子（candle seed 34；MA 9/21 預設 gate 失敗、MA 3/8 寬鬆 gate 通過且門檻寫入快照）＋手動 `full` 列＋隱藏的 `test` 列 |
| `src/services/resultsExplorer.ts`（＋`.test.ts`） | 純規則：Test 隱藏、Validation 排名、篩選、最新摘要 join、空值格式、`record_json` 防禦性解析（legacy／unreadable／missing） |
| `src/components/ResultsExplorer.tsx` | 新增元件；`BacktestPanel.tsx` 掛載於 DiscoveryPanel 之下 |
| `e2e/results-explorer.spec.ts` | 2 條：種子歷史（排名、快照、交易、refresh 保留選取、失效選取、Test 隱藏、篩選）；手動存檔流程與空狀態 |
| `tasks.md`、`README.md`（三語）、`CHANGELOG.md`、`docs/autonomous-research-capability-registry.md`、`docs/validation-record-contract.md` | 狀態與說明同步 |

Migration／schema：無。依賴：無新增。契約版本：無改動。

## 設計要點（供 ABC-12／P21 延伸）

- 只在展開／重新整理時讀取，載入時間顯示於標題列；背景探索完成不會替換畫面（Plan §4.7）。
- 選取以 id 保存；refresh 後仍顯示；被篩選隱藏時顯示「失效選取」提示，不清除。
- 快照解析永不補值：v1 標 legacy、不可解析回報原因、缺段列出。Test 區間不渲染。
- 摘要是「最新結果投影」：紀錄對應的 Train／Validation 摘要不存在時明講；trade_count>0 但無明細時明講。

## Verification

| 項目 | 結果 |
| --- | --- |
| `npm.cmd run typecheck` | 通過 |
| `npm.cmd test` | 863 passed（+9） |
| `npm.cmd run build` | 通過 |
| `cargo test --locked`（`CARGO_TARGET_DIR=C:\tmp\aff-target`） | 52 + 104 = 156 passed（+1） |
| `E2E_PORT=5199 npx playwright test --workers=1` | 64 passed（+2） |
| 原生 `cargo tauri dev` | **未執行**：命令註冊由 `generate_handler!` 編譯驗證、DB 讀取由 Rust 測試覆蓋；PR 時 CI 的 native smoke lane 會啟動 binary |

種子探測：以 vitest 掃描 candle seed 1–80 × 6 個策略，唯有寬鬆 gate 才有通過案例（`benchmarkWins` 在上漲樣本上必敗），故種子以寬鬆 gate 且門檻寫入快照的方式取得一筆真實通過紀錄。

GitHub：`git push` 401（`GITHUB_TOKEN` 失效）。本次只做本機 commit。

## 下一階段前置條件

- P02（runtime 解耦）：依賴 P00；以 `docs/research-runtime-contract.md` §0／§6 為邊界，零行為變更、
  既有 Rust 測試證明事件序與 commit-then-emit 不變。
- ABC-12／P21 延伸 explorer 時，保留本文「設計要點」的四條規則。

## Resolution (added when acted on)

（待補：push／PR 編號、review 結果。）

### 2026-09-17 acceptance review

Codex 驗收 `5e029d7`：既有 863 Vitest／156 Rust／64 Playwright 與 build 通過，但驗收尚未通過。
已重現交易明細與摘要跨次保存混用、首次讀取失敗無限自動重試兩項缺陷。
詳見 [P01 acceptance review](2026-09-17-p01-acceptance-review-v1.md)；待修正與重驗後再追加 Resolution。

### 2026-09-17 修正（R1／R2）

- **R1**：`get_trades` 改為 `get_backtest_result_detail`，在同一 transaction 讀回（summary, trades）；前端以 `sameSummaryRow`
  逐欄比對回傳 summary 與畫面上凍結的列（id 與 `created_at` 在同 key 重存時都不變，故以整列為身分），不符即顯示
  「已被重新保存」並列出兩邊淨報酬／交易數、**不掛載**新明細；每次列表讀取遞增 read generation，遲到的明細回應
  被丟棄。同數量替換由 Rust 測試與 Playwright 回歸各覆蓋一次。
- **R2**：讀取狀態改為 `idle → loading → ready | failed`；只在 `idle` 自動讀一次，失敗保留訊息（與先前資料），
  只有「重新整理」會再讀。
- 回歸：`e2e/results-explorer.spec.ts` 新增 3 條（mock 控制 `replaceBeforeDetail`、`detailDelay=1500`、
  `explorerFailOnce`），並以三個突變（略過比對／略過 generation 檢查／恢復舊 effect 守衛）各自驗證測試會失敗。
- 重驗：typecheck、build、865 vitest、156 Rust、67 Playwright 全綠。原生 Tauri 仍未執行。
- 已知限制（如實記錄）：內容完全相同的重存（整列逐欄相等）無法與原列區分，畫面也因此不會錯——完整的世代標記
  屬 P05 不可變 attempt artifacts。
