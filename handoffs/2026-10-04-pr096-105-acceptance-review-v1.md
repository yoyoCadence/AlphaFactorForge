# Handoff: PR #96–#105 整合驗收（往前驗收 batch C）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`
Status: 驗收完成；十個 PR 都可以接受，本次**沒有新的缺陷**。只有一項低優先的文件觀察（README 進度停在 P11）。

## Summary

這十個 PR 是 post-PR #76 稽核的修正（#98、#99、#104、#105）、discovery runner 的前端契約與面板（#100–#102）、
Windows native smoke CI（#103），以及行銷文件（#96、#97）。
十個合併 commit 的 tree 都與各自的最終 head 相同，所有 head 的 CI 都通過（#103 之前是五項，之後是六項）。

#100、#101、#102、#104、#105 在 PR 留言中都有 Codex 驗收；需要修改的都已修正並經覆驗或確認。
**#98、#99 沒有任何審查紀錄**，所以本次針對它們，以及 #105 的檔名安全邊界，在目前的 main 上重新檢查。

## 本次重點檢查

| PR | 檢查內容 | 結果 |
| --- | --- | --- |
| #99 指標參數驗證 | 驗證是否涵蓋所有模式 | `runParamsBacktest` 是唯一入口（面板執行、Holdout 兩段、每個掃描變體），先呼叫 `assertStrategyParams` 再 `buildSignals`；blocks、code 模式用的是同一組策略層級週期（`strategySignals.ts` 的 `resolveSeries`），所以三種模式都受保護。持久化路徑 `buildStrategyDef` 也呼叫同一個 validator。 |
| #98 掃描結果綁定 context | 失效判斷是否只有一處 | `sweepContextKey`／`sameSweepContext` 以 canonical 形式比較整個 context（dataset、實際最佳化的 bar 範圍含 Holdout、扣掉掃描軸的 strategy），熱力圖、套用最佳、點格與 ✓ 標記都只讀 render 階段導出的 gate；被取代的掃描完成時直接丟棄。`ParamsStrategy` 日後新增欄位會自動納入比較。 |
| #105 報告檔名 | 路徑穿越與覆寫 | 只取 basename，字元以白名單過濾（`:` 也會被換掉，所以不會產生 NTFS 替代資料流），必須以 `.json`／`.csv` 結尾；以 `create_new` 原子建立，不會截斷既有檔案；未過濾的 writer 維持 private（Codex 驗收的修正仍在）。 |
| #96／#97 README | 是否把 roadmap 寫成已完成 | README 第 17–23 行明確寫出「AI 生成／approve 尚待 P15，前端未接線」，沒有誇大。 |

## Review Notes

- **低優先文件觀察**：README 的進度段落最後更新於 P11（`c1391f2`，2026-09-22），完全沒有提到 P12（試驗帳本、walk-forward、campaign、確認統計）。
  README 自己寫明「phase 狀態以 `tasks.md` 為準」，所以不構成錯誤資訊，但讀 README 的人會以為進度停在 P11。建議下次更新文件時補上一句 P12 的狀態，並指向 tasks.md。
- `export_report` 仍是 `NotImplemented` 的 placeholder（`commands/file_commands.rs:32–37`），有明確的 TODO；報告目前由前端組好後經 `save_report` 寫出。不是缺陷，列出以免被誤認為可用的命令。

## Per-PR acceptance

| PR | 內容 | 先前驗收 | 結論 | CI run |
| --- | --- | --- | --- | --- |
| [#96](https://github.com/yoyoCadence/AlphaFactorForge/pull/96) | 行銷 campaign 與 README | — | 可接受 | [31311708585](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/31311708585) |
| [#97](https://github.com/yoyoCadence/AlphaFactorForge/pull/97) | README AI 定位 | — | 可接受 | [31312467160](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/31312467160) |
| [#98](https://github.com/yoyoCadence/AlphaFactorForge/pull/98) | 掃描結果綁定 context | 無 | 可接受（本次新審） | [31874291188](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/31874291188) |
| [#99](https://github.com/yoyoCadence/AlphaFactorForge/pull/99) | 指標參數驗證 | 無 | 可接受（本次新審） | [31911010223](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/31911010223) |
| [#100](https://github.com/yoyoCadence/AlphaFactorForge/pull/100) | discovery-event-v1 前端契約 | Codex：1 項阻擋（`score` 可被省略）→ 已修正 | 可接受 | [31923946689](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/31923946689) |
| [#101](https://github.com/yoyoCadence/AlphaFactorForge/pull/101) | run envelope 組裝 | Codex：通過 | 可接受 | [31930049316](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/31930049316) |
| [#102](https://github.com/yoyoCadence/AlphaFactorForge/pull/102) | discovery runner 面板 | Codex：2 項阻擋競態＋1 項中度 → 已修正並覆驗通過 | 可接受 | [31935000399](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/31935000399) |
| [#103](https://github.com/yoyoCadence/AlphaFactorForge/pull/103) | Windows native smoke CI | — | 可接受（之後每個 PR 都跑這一項） | [31954344216](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/31954344216) |
| [#104](https://github.com/yoyoCadence/AlphaFactorForge/pull/104) | 摘要與交易列一致性 | Codex：1 項阻擋（raw writer 公開）→ 改為 private | 可接受 | [32038389411](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/32038389411) |
| [#105](https://github.com/yoyoCadence/AlphaFactorForge/pull/105) | 報告檔原子建立 | Codex：1 項 P2（未過濾 writer）→ 改為 private | 可接受 | [32375122991](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/32375122991) |

## Verification

- 十組 `git diff <head> <merge>` 都是空的；各 head 的 CI 都是 success。
- 全套測試沿用整合 baseline 的結果（見 [#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)）；e2e 中的 `sweep-context.spec.ts`、`strategy-validation.spec.ts` 都在本次本機通過的 82 項內。
- 本次沒有新增反例。

## Resolution

Informational.

## Follow-up work order（2026-10-04 追加）

本檔的發現已整理進 [驗收後續工作單](2026-10-04-acceptance-followups-work-order-v1.md)，請接手的 agent 從那裡開始：

- README 進度停在 P11 → **FU-8**（併入 `DOC-STATE-002`，可與 FU-2 同一個 PR）。
