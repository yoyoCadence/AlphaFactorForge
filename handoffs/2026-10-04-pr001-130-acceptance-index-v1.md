# Handoff: PR #1–#140 驗收總覽（索引）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`（PR #141 合併後的 `main`；程式碼與 `b9b63a3` 相同）
Status: 全部 140 個 PR 都已有整批驗收紀錄。沒有任何 PR 需要回退；有 1 項 P2、4 項 P3 與幾項低優先觀察待維護者安排。本次不改產品程式碼。

## 這份文件的用途

使用者要求「往前驗收所有 PR，看是否有問題」。這份索引是入口：列出每一段由哪份 handoff 涵蓋、
每段的檢查方法，以及所有發現的優先順序。細節、重現步驟與證據都在各段的 handoff 裡。

## 涵蓋範圍

| PR 範圍 | handoff | 結論 | 新發現 |
| --- | --- | --- | --- |
| #1–#35 | [pr001-035](2026-10-04-pr001-035-acceptance-review-v1.md) | 可接受 | J-R1（P3） |
| #36–#45 | [pr036-045](2026-10-04-pr036-045-acceptance-review-v1.md) | 可接受 | — |
| #46–#55 | [pr046-055](2026-10-04-pr046-055-acceptance-review-v1.md) | 可接受 | H-R1（P3） |
| #56–#65 | [pr056-065](2026-10-04-pr056-065-acceptance-review-v1.md) | 可接受 | G-N1（低，不可觸發） |
| #66–#75 | [pr066-075](2026-10-04-pr066-075-acceptance-review-v1.md)＋[差分測試 patch](2026-10-04-pr066-075-differential-harness.patch) | 可接受 | 指標 fixture 覆蓋建議（低） |
| #76–#85 | [pr076-085](2026-10-04-pr076-085-acceptance-review-v1.md) | 可接受 | —（列出 #76 稽核仍未處理的 12 項） |
| #86–#95 | [pr086-095](2026-10-04-pr086-095-acceptance-review-v1.md) | 可接受 | — |
| #96–#105 | [pr096-105](2026-10-04-pr096-105-acceptance-review-v1.md) | 可接受 | README 進度觀察（低） |
| #106–#115 | [pr106-115](2026-10-04-pr106-115-acceptance-review-v1.md) | 可接受 | B-R1（P3，文件） |
| #116–#125 | [pr116-125](2026-09-29-pr116-125-acceptance-review-v1.md)（2026-09-29，前次 session） | R1–R5 已由 #126 修正 | — |
| #126–#130 | [pr126-130](2026-10-04-pr126-130-acceptance-review-v1.md)＋[反例 patch](2026-10-04-pr126-130-acceptance-regressions.patch) | 可接受 | **A-R1（P2）**、A-R2（P3） |
| #131–#140 | [pr131-140](2026-10-04-pr131-140-acceptance-review-v1.md)＋[反例 patch](2026-10-04-pr131-140-acceptance-regressions.patch)（已由 #141 合併） | 可接受 | PR131-140-R1（P3） |

每一段都確認了：合併 commit 的 tree 與 PR 最終 head 相同（140 個全部成立，沒有合併時夾帶的內容）、每個 head 的 CI 都通過。
整合 baseline 的全套本機檢查：566 Rust（1 ignored）、1072 Vitest、82/82 Playwright、typecheck、build、`cargo check --all-targets`、clippy（只有既有 5 個 warning）。

## 檢查方法

- 已有獨立驗收的 PR：確認被驗收的 head 就是最終合併的 head，且發現都已修正；不重做逐行審查。
- 沒有審查紀錄、或屬於 AGENTS.md 高風險面的 PR：在**目前的 main** 上重新檢查，並盡量用可執行的證據：
  - 反例測試（帳本回退：A-R1、PR131-140-R1）；
  - TS↔Rust 隨機差分測試（回測 3,600 例、指標 800 例 × 16 條序列，全部一致）；
  - 回測對帳不變式（3,600 例全部成立）；
  - 暫時腳本實測（embargo 推導、code 模式直譯器）；
  - 對照官方文件（Tauri 同步命令在主執行緒執行）、`npm audit`、`grep` 文件與設定。
- 所有暫時檔案都已移除；工作區與 baseline 相同。

## 發現與建議順序

| 順序 | 項目 | 優先 | 摘要 | 建議 |
| --- | --- | --- | --- | --- |
| 1 | **A-R1** ＋ PR131-140-R1 | P2 ＋ P3 | 帳本中「不新增事件的證據」（import 偵測到的永久隔離、檢定數升級）不受 workspace binding 保護：把 registry 檔還原成較舊複本後，binding 仍為 `Current`，隔離消失／檢定數下降。隔離那一項沒有其他防線。 | 同一個根因，合併成一個任務處理；在 P13 用帳本計數之前完成。兩份反例 patch 可直接作為回歸測試的起點。 |
| 2 | B-R1 | P3 | README（三語）、子專案 README、AGENTS.md 寫 Rust 1.77／1.77.2+，`Cargo.toml` 要求 1.89。 | 小型文件修正；AGENTS.md 的措辭請維護者決定。 |
| 3 | H-R1 | P3 | `npm audit` 又出現 6 個開發工具依賴的發現（3 high）；production 為 0。 | 依 `docs/security-audit-npm.md`：不得用 `npm audit fix`，改以明確版本升級並審查 lockfile；Vitest 5 另開任務。 |
| 4 | J-R1 | P3 | code 模式直譯器用 `in` 查函式白名單，原型鏈名稱只靠型別不符才被拒絕。 | 改用 `Object.hasOwn`，並加回歸測試。 |
| 5 | A-R2 → `DB-ASYNC-001` | P3（併入既有 P2） | campaign 預覽是同步 Tauri 命令，在主執行緒載入並驗證所有 candle。 | 併入既有的 `DB-ASYNC-001`，不另開任務。 |
| — | G-N1 | 低 | `deriveEmbargoBars` 遇到無效左運算元回傳 NaN 而非 throw（不可觸發）。 | 可選的防禦性修正。 |
| — | 指標 fixture | 低 | `indicators-v1.json` 只有一個案例；邊界語意只由本次的隨機測試確認過。 | 加幾個精確可表示的邊界案例。 |
| — | README 進度 | 低 | README 進度停在 P11。 | 併入既有的 `DOC-STATE-002`。 |
| — | 已保存的 campaign | 設計注意 | 任何被釘選的合約版本升級後，舊 campaign 都無法讀取，清單整個報錯。 | 下次升級合約版本前先決定處理方式。 |

## 仍未完成的 operator acceptance（不是缺陷，但不能當成已驗證）

- P01：native Tauri 的重啟／重開。
- P09：真實 Tiingo 憑證的外部驗收。
- #131：由服務建立 snapshot 的 native campaign smoke。
- 服務的 `control-token` 檔若放在共用位置（`AFF_DATA_DIR`），沒有另外的 ACL 保護。

## 既有、尚未處理的技術債（不是本次新發現）

PR #76 稽核留下的 12 項仍在 Backlog（`DB-ASYNC-001`、`TEST-MOCK-PARITY-001`、`PERF-001`、`PERF-CHART-COMPUTE-001`、`PERF-CHART-BRIDGE-001`、
`DOC-STATE-002`、`TOOLCHAIN-001`、`CI-RUSTFMT-001`、`DB-MIGRATION-DIAGNOSTIC-001`、`SEC-RUST-001`，以及待裁決的 `INTERVAL-CONTRACT-001`、`DECISION-ZERO-TRADE-001`），
其中 4 項本次抽查仍成立；另有 `NUMERIC-JSON-001`、`PARITY-001`～`003` 等既有項目。詳見 batch E。

## Resolution

Pending. 處理任何一項的人，請在對應 batch 的 handoff 追加 Resolution，並在這裡的表格註記。

## Follow-up work order（2026-10-04 追加）

本檔的發現已整理進 [驗收後續工作單](2026-10-04-acceptance-followups-work-order-v1.md)，請接手的 agent 從那裡開始：

- 所有發現與需要確認的項目，已整理成可接手的工作單：FU-1～FU-9（工作項目）、D1～D7（需要維護者確認的決定）、O1～O4（人工驗收步驟），以及給接手 agent 的工作方式與「不需要重做的檢查」。

2026-10-04 追加：D1–D7 已確認，可執行的版本是 [工作單 v2](2026-10-04-acceptance-followups-work-order-v2.md)（取代 v1）。
