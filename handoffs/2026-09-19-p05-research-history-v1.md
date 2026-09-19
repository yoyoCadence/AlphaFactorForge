# Handoff: P05 完整研究歷史（hypothesis、attempt、lineage、不可變 artifacts）

Date: 2026-09-19
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p05-research-history`（自 main `fcd6905`＝PR #106 merge 之後建立）
PR: 待建立（本 handoff commit 後開 PR，不 merge）
Status: 實作完成、本機驗證通過（Rust 253／Vitest 879／Playwright 77），待 Codex 驗收；P06 另行授權

## Summary

Plan §5 P05（ABC-05）。使用者於 PR #106 merge 後指示「繼續下個任務」。本次在既有 run 紀錄旁新增
append-only 的研究歷史：每個探索候選＝一筆 **research attempt**，對應一個**執行前凍結的 hypothesis**，
帶輸入／引擎指紋；完成時把**完整結果**存成不可變 `candidate-result-v1` artifact；失敗／取消各自留原因。
既有 `backtest_summary`／`backtest_trades` 維持「最新結果投影」語意（重跑仍覆寫），舊明細改由 attempt 的
artifact 保存。資料形狀與不變量登記於 [`docs/research-history-v1.md`](../docs/research-history-v1.md)。

P05 驗收「重跑不覆寫舊明細；失敗可追溯；新舊投影一致」對照見該文件 §5，皆有 Rust 測試。

## 執行前驗證

- **計畫 vs 程式碼**：§3.3 規則「新研究結果以 attempt ID 保存完整不可變 artifacts；舊 summary 保留為相容的最新
  結果投影」「artifacts 先 staging、校驗、原子更名，再提交 DB 參照；未被引用的檔案可辨識，已引用檔案不得悄悄
  刪除」「不宣稱能恢復過去已被覆寫的交易明細」→ 逐條實作（見設計要點），P05 之前被覆寫的明細如實不宣稱可恢復。
- **ABC-05**：「未回測就能查到假說／版本」→ 假說與 attempts 與入隊同一 transaction，首個候選被 claim 時已可查；
  「重複請求不新增候選」→ hypothesis 以內容 hash 去重、`discovery.start` 的 requestId 冪等（P03b）；「原始假說
  不可被後續編輯改寫」→ 觸發器拒絕 UPDATE／DELETE；「AI 生成功能另沿 Phase C 接入」→ 未做，discovery 自動登記的
  假說 failure_modes 明示「未陳述」。
- **where to hook**：runner 的每個 job 轉換都有一個 transaction（enqueue／claim／commit／fail／cancel／recover），
  attempt 狀態在**同一 transaction** 內同步，不另開寫入點；`CandidateAssessment` 結構不動，改以
  `commit_candidate_assessment_with_artifact(…, Option<&ArtifactRef>)` 擴充，舊簽名成為 `#[cfg(test)]` 便利函式
  （`discovery_tests.rs` 17 處呼叫不動）。
- **canonical JSON**：`identity::canonical_bytes` 是識別 hash 的二進位編碼、不是 JSON（初版誤用，artifact 讀回無法
  解析，測試立刻抓到）；改為 `research::canonical_json`（鍵遞迴排序、無空白），同時用於假說 hash 與 artifact 內容，
  有往返與順序無關測試。
- **artifact 根目錄**：`<workspace data dir>/artifacts`，與 DB 相鄰（`%APPDATA%` 非 OneDrive）；`open_workspace`
  把 store 交給 runner；in-memory 測試 DB 的 runner 可指定暫存目錄或不帶 store（attempt 完成但無 artifact，只有
  測試）。
- **結果無法不可變保存時**：staging／rename 失敗 → 該候選不提交、run 以該原因失敗（不留半成品），與既有
  「commit 失敗即 run 失敗」語意一致。

## 修改檔案

- `src-tauri/migrations/0007_research_history.sql`（新）：`hypotheses`、`research_artifacts`、`research_attempts`
  ＋索引＋觸發器（immutable／kept／frozen）。
- `src-tauri/src/research/mod.rs`（新）：版本常數、`canonical_json`、`sha256_hex`（＋測試）。
- `src-tauri/src/research/history.rs`（新）：`HypothesisDraft`（`content`／`hash`）、`register_hypothesis`（去重）、
  `AttemptDraft`／`RunLineage`／`register_run_lineage`、`mark_candidate_running`、`complete_candidate`、
  `fail_unfinished`、`skip_unfinished`、`requeue_running_in_running_runs`、`insert_artifact`、查詢
  （`list_attempts`／`get_attempt`／`list_hypotheses`／`get_hypothesis`／`referenced_artifact_paths`）。
- `src-tauri/src/research/artifacts.rs`（新）：`ArtifactStore { put, read, path_of, unreferenced }`＋測試。
- `src-tauri/src/db/discovery.rs`：`start_discovery_run_with_lineage`、`commit_candidate_assessment_with_artifact`、
  claim／fail／skip／recover 各加一行 history 呼叫；`db/mod.rs` 登記 0007。
- `src-tauri/src/discovery_runner/mod.rs`：`DiscoveryRunner.artifact_store`＋`with_artifact_store`；`start_for_request`
  建 `run_lineage`（每候選一組 hypothesis＋attempt draft）；coordinator commit 前 `store_candidate_artifact`。
  `execution.rs`：`engine_fingerprint`。`runtime/mod.rs`：production runner 帶 store。
- `src-tauri/src/commands/research_commands.rs`（新）：`list_research_attempts`、`get_research_attempt`、
  `list_hypotheses`、`list_unreferenced_artifacts`；`main.rs`／`service_main.rs` 宣告 `research`；
  `boundary_tests` 加三個新檔。
- 測試：`discovery_runner/tests/history.rs`（新，5）、`research/artifacts.rs`（3）、`research/mod.rs`（1）、
  migration 計數與 `open_migrated` 測試更新（6→7）。
- 前端：`tauri-client/commands.ts`（`research.*` 與型別）、`dataClient.ts`（seam）、`mockClient.ts`（假說／attempt
  與 mock runner 同步）、`components/ResearchHistory.tsx`（新）、`BacktestPanel.tsx` 掛載、
  `e2e/research-history.spec.ts`（新，2）。
- 文件：`docs/research-history-v1.md`（新）、`tasks.md`（P05 Done、ABC-05 勾選、測試數）、`CHANGELOG.md`、
  registry、`STRATEGY_DISCOVERY.md` §4、README 三語、本 handoff。

## 設計要點

- **狀態機**：`submitted → running → completed | failed | skipped`；`running → submitted` 只在崩潰恢復把 job 重新
  排隊時發生（同一 attempt key，指紋不變）。觸發器把「終態不可改、凍結欄位不可改、不可刪」變成機械規則。
- **假說粒度**：discovery run 的每個 base 一個假說（機制＝preset＋entry／exit 訊號＋變異的 axes；適用條件＝資料集
  hash／interval／split／embargo／axes；`strategy_hash = strategy-doc-v1:<canonical base strategy sha>`；
  `variation_kind = param-sweep:<axes>`）。同 config 重跑 → 同一假說 id、新的 attempts。
- **指紋**：input＝envelopeVersion、configHash（raw config canonical sha）、rootSeed、dataset id/hash/interval、
  strategy id/hash、baseId、candidateIndex、appliedAxes、seeds；engine＝package 版本＋config 宣告的全部契約版本＋
  binary 實作的 execution／metrics／benchmarks／gate／score／validationRecord／benchmarkRecord 版本。
- **artifact 與投影**：commit transaction 內先 `insert_artifact`（同 sha 同列）再 `complete_candidate`；讀取端
  `get_research_attempt` 校驗 sha／長度，檔案被動過或缺失時回 `resultError`，attempt 列仍是證據。

## Verification

- `cargo check --locked` 0 warning（兩個 binary）；`cargo clippy --locked --all-targets` 無新增（core 既存 4 項＋
  `file_commands.rs` 1 項為 main 既有）。
- `cargo test --locked` **253 passed（52 + 199 + 2）**，新增／調整：
  - `discovery_runner::tests::history`（5）：假說與 attempts 與入隊同存、首候選 claim 時 running／其餘 submitted、
    指紋內容、完成後每筆有可校驗 artifact 且無未參照檔；**重跑**：投影列 id 相同、舊 attempt 列與 artifact bytes
    不變、兩個 artifact sha 不同、投影欄位＝各自 artifact；**失敗／取消**：原因、終態凍結、UPDATE／DELETE 被拒；
    假說去重與驗證；恢復 requeue 同一 attempt。
  - `research::artifacts::tests`（3）：put 冪等、staging 無殘留、讀取校驗、篡改／截斷拒絕、路徑守衛、未參照清單
    含 staging 殘留且不刪。`research::tests`（1）：canonical JSON 排序、往返、順序無關。
  - 既有 runner／command／host／service 測試全部走新的 lineage 路徑（production `start`）不變綠。
- `npm.cmd run typecheck`、`npm.cmd run build` 通過；`npm.cmd test` **879**（無新純函式）。
- Playwright 全套（`--workers=1`、`E2E_PORT=5199`）**77 passed**，新增 `research-history.spec.ts`（2）：一次
  探索留 3 筆 completed、detail 顯示假說／指紋／結果、重跑後 6 筆且前 3 筆不變；取消後未跑候選 skipped 帶原因。
- 原生 Tauri 未執行（無宿主／啟動路徑變更；migration 0007 在 `open_workspace` 路徑由 Rust 測試覆蓋）。
- 暫存：`aff-history-test-*`／`aff-artifacts-test-*` 皆由 guard 清除，無殘留。

## 未完成項目／已知限制

- **有陳述失效情境的假說**：AI 提案（P15）與手動登記；本次 discovery 自動登記為明示「未陳述」。
- **attempt 間血緣**：`parent_strategy_id` 由 P11／P15 填入；本次為 NULL。
- **`research-command-v1` 未加入讀取命令**：P16 MCP 需要時修訂契約 §2。
- **既有投影的歷史**：P05 之前已被覆寫的交易明細不可恢復（契約明示）。
- **artifact 備份／匯出**：P14；**全文索引**：P17。
- **UI**：只讀面板，無篩選；P21 整合 UX 再擴。

## 對 P06／後續的交接點

- 新表遵守與 0001–0006 相同的 migration 規則（新增序號、原文不改）；`open_migrated` 已把 0007 納入「恰好本 build」
  檢查，舊 service 與新桌面混跑會以 `SchemaPending` 拒連。
- 任何新的 runner 寫入路徑若影響候選狀態，必須在同一 transaction 內同步 `research_attempts`（見
  `research-history-v1.md` §1.2 表）。
- 想在 artifact 內加欄位：提升 `CANDIDATE_RESULT_VERSION`，讀取端以版本分支；不改既有檔。

## Resolution (added when acted on)

（待補：Codex 驗收結果、PR 編號。）
