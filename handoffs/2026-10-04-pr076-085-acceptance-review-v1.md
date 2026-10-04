# Handoff: PR #76–#85 整合驗收（往前驗收 batch E）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`
Status: 驗收完成；十個 PR 都可以接受，本次**沒有新的缺陷**。#76 的合併後稽核（#77）留下的 P1 都已完成，但 **12 項 P2／P3 與待裁決項目至今仍未處理**，下面列出並抽查其中三項目前仍成立。

## Summary

這十個 PR 是後端策略探索執行器（#76）、它的合併後稽核（#77）、互動式回測結果綁定 context（#78）、METRIC-002 規格（#79），以及主題系統（#80–#85）。
十個合併 commit 的 tree 都與各自的最終 head 相同，所有 head 的 CI 都通過。

#76 本身沒有 PR review，但合併後有一份完整的唯讀稽核（[pr76-post-merge-audit](2026-07-31-pr76-post-merge-audit-v1.md)），
列出 7 個 P1 correctness gate 與一串 P2／P3 項目。#78 有 review（Approve），#79 的三組必改已修正；主題系列在 PR 內有逐步的追加修正。

## #76 稽核的結案狀態

**P1（7 項）全部完成**：BUG-RESULT-CONTEXT-001（#78）、METRIC-002（#88）、PERSIST-AUDIT-001（#89）、RUNNER-OWNERSHIP-001（#90）、
DATA-QUALITY-001（#94）、BUG-SWEEP-CONTEXT-001（#98）、STRATEGY-VALIDATION-001（#99）。P2 的 PERSIST-INVARIANT-001（#104）、IO-ROBUSTNESS-001（#105）也已完成。

**仍未處理（tasks.md Backlog，全部 `[ ]`）**：

| 項目 | 優先 | 內容 | 本次抽查 |
| --- | --- | --- | --- |
| `DB-ASYNC-001` | P2 | 大型匯入、結果保存、檔案寫入都是同步命令並持有 DB mutex | 仍成立；batch A 的 A-R2（campaign 預覽）是之後新增的同類實例 |
| `TEST-MOCK-PARITY-001` | P2 | mock 的 `saveStrategy` 每次給新 id，與 SQLite 依 hash UPSERT 不同 | — |
| `PERF-001` | P2 | 最多 256 組的參數掃描在 React thread 同步執行 | — |
| `PERF-CHART-COMPUTE-001` | P2 | hover／replay 重繪時重算全部指標 | — |
| `PERF-CHART-BRIDGE-001` | P2 | 小幅 view 變更就把全部 candle 重新送到原生視窗 | — |
| `DOC-STATE-002` | P2 | README、`TODO.md`、`PHASE_A_VERIFY.md` 的狀態描述過時 | 相關：batch B 的 B-R1（Rust 版本）、batch C 的 README 進度觀察 |
| `TOOLCHAIN-001` | P3 | 沒有固定 Rust 工具鏈，MSRV 未被證明 | 仍成立：CI 用 `dtolnay/rust-toolchain@stable`（浮動） |
| `CI-RUSTFMT-001` | P3 | `db_commands.rs` 格式漂移；CI 不跑 fmt | 仍成立：`rustfmt --check src/commands/db_commands.rs` 有差異；workflow 中沒有 fmt 步驟 |
| `DB-MIGRATION-DIAGNOSTIC-001` | P3 | migration 存在性查詢用 `unwrap_or(false)` 吞掉真正的 SQLite 錯誤 | 仍成立：`src-tauri/src/db/mod.rs:124,220` |
| `SEC-RUST-001` | P3 | CI 沒有 Rust advisory scan | 仍成立：workflow 中沒有 `cargo audit` 或同等步驟 |
| `INTERVAL-CONTRACT-001` | 待裁決 | 未知的 interval 靜默以日線（365 bars/年）年化 | — |
| `DECISION-ZERO-TRADE-001` | 待裁決 | 零交易候選在 Random Entry 前讓執行失敗 | — |

這些不是本次的新發現，而是早已登記、尚未排入的技術債；列在這裡是因為使用者要求「看是否有問題」，它們仍然是現存的問題。
其中 `INTERVAL-CONTRACT-001` 與 `DECISION-ZERO-TRADE-001` 被稽核明確標為「不得悄悄修正」，需要先裁決再版本化契約。

## Per-PR acceptance

| PR | 內容 | 先前驗收 | 結論 | CI run |
| --- | --- | --- | --- | --- |
| [#76](https://github.com/yoyoCadence/AlphaFactorForge/pull/76) | 後端策略探索執行器 | 合併後稽核（#77） | 可接受；稽核的 P1 都已修正 | 30591324286 |
| [#77](https://github.com/yoyoCadence/AlphaFactorForge/pull/77) | 稽核紀錄 | — | 可接受 | 30633403829 |
| [#78](https://github.com/yoyoCadence/AlphaFactorForge/pull/78) | 回測結果綁定 context | review：Approve | 可接受 | 30643407857 |
| [#79](https://github.com/yoyoCadence/AlphaFactorForge/pull/79) | METRIC-002 規格 | review：三組必改已修正 | 可接受 | 30685407828 |
| [#80](https://github.com/yoyoCadence/AlphaFactorForge/pull/80) | 主題 token 層 | 主題系列逐步追加修正 | 可接受 | 30738275080 |
| [#81](https://github.com/yoyoCadence/AlphaFactorForge/pull/81) | 面板讀取主題 token | 追加 commit 修 e2e | 可接受 | 30739393284 |
| [#82](https://github.com/yoyoCadence/AlphaFactorForge/pull/82) | 畫布與熱力圖主題化 | — | 可接受 | 30741856654 |
| [#83](https://github.com/yoyoCadence/AlphaFactorForge/pull/83) | EMA／BB token 與成交量配色 | — | 可接受 | 30742632664 |
| [#84](https://github.com/yoyoCadence/AlphaFactorForge/pull/84) | 主題選擇器 | 對照設計稿補齊 6 項、熱力圖墨色 | 可接受 | 30890160265 |
| [#85](https://github.com/yoyoCadence/AlphaFactorForge/pull/85) | muted 文字 AA 與最後四個顏色 | 追加 `faint` 處理 | 可接受 | 30912705248 |

（CI run 連結格式：`https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/<id>`。）

主題系列的對比規則之後都寫成了測試（`src/theme/contrast.test.ts`），並在 #87 擴充；本次本機的 1072 項 Vitest 全部通過。

## Verification

- 十組 `git diff <head> <merge>` 都是空的；各 head 的 CI 都是 success。
- 抽查指令：`grep -n "unwrap_or(false)" src/db/mod.rs`、`rustfmt --edition 2021 --check src/commands/db_commands.rs`、`grep` workflow 中的 toolchain／fmt／audit 步驟。
- 全套測試沿用整合 baseline 的結果（見 [#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)）。

## Resolution

Informational. 上表的既有 Backlog 項目照原本的任務處理，本次不另開任務。

## Follow-up work order（2026-10-04 追加）

本檔的發現已整理進 [驗收後續工作單](2026-10-04-acceptance-followups-work-order-v1.md)，請接手的 agent 從那裡開始：

- 本檔列出的 PR #76 稽核既有項目不在工作單範圍內，照原本的 Backlog 處理；其中 `DB-ASYNC-001` 已加入 **FU-5**（campaign 預覽）。
