# Handoff: PR #36–#45 整合驗收（往前驗收 batch I）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`
Status: 驗收完成；十個 PR 都可以接受，本次**沒有新的缺陷**。

## Summary

這十個 PR 是 BUG-001 的 e2e 補強（#36）、把 BacktestPanel 拆成 SweepSection／ChartSection／Dataset／Results／StrategySection 的 move-only 重構（#37、#39、#40、#41）、
文件（#38、#42）、回測 golden test 與 legacy parity 報告（#43），以及三個核心回測修正：手續費與結算權益對帳（#44）、nextOpen 成交時序與風控成交價（#45）。
十個合併 commit 的 tree 都與各自的最終 head 相同，所有 head 的 CI 都通過。只有 #42 有 PR 留言（review 指出的三處文件矛盾已修正）。

## 本次重點檢查

| 對象 | 檢查 | 結果 |
| --- | --- | --- |
| #44 對帳 | 以 batch F 產生的 3,600 個隨機回測案例（TS 引擎輸出），檢查「最後權益 = 起始權益 + 所有交易 pnl 之和」 | **3,600 例全部成立**，最大相對誤差 2.5e-14（浮點捨入）；涵蓋手續費、滑價、SL／TP、nextOpen、`both` 與子區間。 |
| #43–#45 行為 | 是否仍被鎖住 | `src/core/backtest/backtest.golden.test.ts`、`backtest.accounting.test.ts`、`backtest.fills.test.ts` 都在本機 1072 項 Vitest 內通過；Rust 端由 #69 的 parity fixture 與 batch F 的差分測試鎖定相同行為。 |
| #37–#41 重構 | move-only 是否引入行為變化 | 這些 PR 以完整 e2e 作為安全網（當時 21 個 spec），且 data-testid 全部保留；目前 82 項 e2e 在本機通過。之後的 ultrareview 指出 REF-003 尚未把 BacktestPanel 壓到 400 行以下，由 #41（REF-003b）補完。 |

## Review Notes

- tasks.md 的「Legacy-parity gaps」三項仍未處理（`PARITY-001` STOCH 指標、`PARITY-002` 後端 candle 抓取命令、`PARITY-003` 策略庫刪除），
  是 #43 legacy parity 報告延伸出來的既有 Backlog，不是新發現。

## Per-PR acceptance

| PR | 內容 | 結論 | CI run |
| --- | --- | --- | --- |
| [#36](https://github.com/yoyoCadence/AlphaFactorForge/pull/36) | BUG-001 e2e 補強 | 可接受 | 29172402741 |
| [#37](https://github.com/yoyoCadence/AlphaFactorForge/pull/37) | REF-001 SweepSection | 可接受 | 29174745950 |
| [#38](https://github.com/yoyoCadence/AlphaFactorForge/pull/38) | 文件 | 可接受 | 29175321118 |
| [#39](https://github.com/yoyoCadence/AlphaFactorForge/pull/39) | REF-002 ChartSection | 可接受 | 29176283630 |
| [#40](https://github.com/yoyoCadence/AlphaFactorForge/pull/40) | REF-003 Dataset／Results | 可接受 | 29178167544 |
| [#41](https://github.com/yoyoCadence/AlphaFactorForge/pull/41) | REF-003b StrategySection | 可接受 | 29180973508 |
| [#42](https://github.com/yoyoCadence/AlphaFactorForge/pull/42) | 文件 | 可接受（review 的三處矛盾已修正） | 29185202734 |
| [#43](https://github.com/yoyoCadence/AlphaFactorForge/pull/43) | golden test、legacy parity 報告 | 可接受 | 29331258512 |
| [#44](https://github.com/yoyoCadence/AlphaFactorForge/pull/44) | 手續費與結算權益對帳 | 可接受；對帳不變式 3,600 例成立 | 29336100168 |
| [#45](https://github.com/yoyoCadence/AlphaFactorForge/pull/45) | nextOpen 成交時序、風控成交價 | 可接受 | 29337886492 |

（CI run 連結格式：`https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/<id>`。）

## Verification

- 十組 `git diff <head> <merge>` 都是空的；各 head 的 CI 都是 success。
- 對帳檢查：用 node 讀取 batch F 的兩個隨機案例檔，計算 `|最後權益 − (起始權益 + Σpnl)| / max(1, |最後權益|)`，門檻 1e-9。
- 全套測試沿用整合 baseline 的結果（見 [#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)）。

## Resolution

Informational.
