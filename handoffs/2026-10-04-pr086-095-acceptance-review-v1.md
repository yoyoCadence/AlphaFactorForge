# Handoff: PR #86–#95 整合驗收（往前驗收 batch D）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`
Status: 驗收完成；十個 PR 都可以接受，本次**沒有新的缺陷**。

## Summary

這十個 PR 是十套主題與背景（#87）、跨月報酬基準與 `metrics-v2`（#88）、`validation-record-v2` 稽核契約（#89）、
單一實例防止多程序 recovery 衝突（#90）、資料集匯入時的市場資料驗證（#94），以及文件（#86、#91、#92、#93、#95）。
十個合併 commit 的 tree 都與各自的最終 head 相同，所有 head 的 CI 都通過。

## 本次重點檢查

| PR | 檢查內容 | 結果 |
| --- | --- | --- |
| #87 主題 | 第二輪 review 指出的兩個中度問題，合併前是否修正 | 合併前的最後兩個 commit（`e32027a`、`f2f1ec5`）已處理：`aria-label` 的回歸測試移到 520px 視窗（`e2e/skins.spec.ts:135–139`，真正低於 560px 斷點）；`surface2` 疊在卡片與工作區上的 ink／muted 對比已加進合成對比測試（`src/theme/contrast.test.ts:101–108`）。 |
| #89 驗證紀錄 v2 | 是否所有寫入路徑都經過嚴格驗證 | 手動保存（`commands/db_commands.rs:131`）與 runner 提交（`db/discovery.rs:1181`）都呼叫同一個 `validate_validation_bundle`；v1 只能讀，v1 與未知版本不能寫入（`discovery_runner/execution.rs:1790` 有測試）。這個 PR 沒有 review 紀錄，但有「遞迴刪除每個必填欄位都會被拒絕」的測試。 |
| #90 單一實例 | 是否仍有效 | `tauri-plugin-single-instance` 仍是第一個 plugin，順序由 `single_instance::tests` 鎖定；之後 P03a 再加上 OS 鎖與 ownership epoch（見 batch B），兩層同時存在。 |
| #88 metrics-v2 | review 的三點是否處理 | PR 留言中的追蹤 review 確認三點都已處理（TS／Rust 月份走訪對齊、測試數、證據）。 |
| #94 資料品質 | 裁決是否記錄 | 「規則 1 在匯入點不可達」的結構性例外已由 maintainer 接受，記在 `2026-08-09-data-quality-001-planning-decisions-v1.md` 的 Implementation Resolution；同一個 `ensure_admissible` 之後也被 campaign snapshot 驗證沿用。 |

## Per-PR acceptance

| PR | 內容 | 先前驗收 | 結論 | CI run |
| --- | --- | --- | --- | --- |
| [#86](https://github.com/yoyoCadence/AlphaFactorForge/pull/86) | README 截圖 | — | 可接受 | 31298504608 |
| [#87](https://github.com/yoyoCadence/AlphaFactorForge/pull/87) | 十套主題與背景 | 兩輪 review；第二輪的兩個中度問題已在合併前修正 | 可接受 | 31262796660 |
| [#88](https://github.com/yoyoCadence/AlphaFactorForge/pull/88) | 跨月報酬基準、metrics-v2 | review＋追蹤 review | 可接受 | 31254740931 |
| [#89](https://github.com/yoyoCadence/AlphaFactorForge/pull/89) | validation-record-v2 | 無 | 可接受（本次新審） | 31266838601 |
| [#90](https://github.com/yoyoCadence/AlphaFactorForge/pull/90) | 單一實例 | native 雙啟動驗收記錄在 PR 內 | 可接受 | 31287614496 |
| [#91](https://github.com/yoyoCadence/AlphaFactorForge/pull/91) | 文件 | — | 可接受 | 31295359497 |
| [#92](https://github.com/yoyoCadence/AlphaFactorForge/pull/92) | DATA-QUALITY-001 裁決 | — | 可接受 | 31291616009 |
| [#93](https://github.com/yoyoCadence/AlphaFactorForge/pull/93) | DATA-QUALITY-001 規格 | — | 可接受 | 31291865504 |
| [#94](https://github.com/yoyoCadence/AlphaFactorForge/pull/94) | 匯入時資料驗證 | 裁決與 Implementation Resolution | 可接受 | 31298020208 |
| [#95](https://github.com/yoyoCadence/AlphaFactorForge/pull/95) | 文件 | — | 可接受 | 31298967182 |

（CI run 連結格式：`https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/<id>`。）

## Verification

- 十組 `git diff <head> <merge>` 都是空的；各 head 的 CI 都是 success。
- 全套測試沿用整合 baseline 的結果（見 [#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)）；`skins.spec.ts` 與 contrast 測試都在本機通過的 82 項 e2e 與 1072 項 Vitest 內。
- 本次沒有新增反例。

## Resolution

Informational.
