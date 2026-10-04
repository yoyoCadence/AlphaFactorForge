# Handoff: PR #66–#75 整合驗收（往前驗收 batch F）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`
Status: 驗收完成；十個 PR 都可以接受，本次**沒有新的缺陷**。另外做了 TS↔Rust 的隨機差分測試（回測引擎 3,600 例、指標 800 例 × 16 條序列），全部一致；並留下一項測試覆蓋建議與一個測試工具上的陷阱。

## Summary

這十個 PR 是 Rust 核心的 TS 等價實作（指標 #68、回測與 metrics #69、訊號／切分／embargo #70、基準與 Random Entry #71、Gate／Score #72），
durable identity v2（#67）、探索執行的准入與候選列舉（#73）、run／job 儲存層（#74），以及設計提案與文件（#66、#75）。
十個合併 commit 的 tree 都與各自的最終 head 相同，所有 head 的 CI 都通過。

#68、#69、#70、#73、#74 在 PR 留言中都有多輪驗收，且最後都判定通過（#74 曾「暫緩合併」，第三輪修正後合併，並有合併後的更正補記）。
#71、#72 沒有 PR 留言；它們的範圍、案例清單與驗證記在 tasks.md Done（RS-CORE-004、RS-CORE-005）。

## 本次差分測試

既有的 parity fixture 都是手寫的固定案例。為了確認 TS 與 Rust 在手寫案例**之外**也一致，本次產生隨機輸入，
用真正的 TS 引擎算出期望值，再讓 Rust 引擎計算並比較（容差與既有 fixture 相同：絕對 1e-12、相對 1e-10）。

| 對象 | 案例 | 內容 | 結果 |
| --- | --- | --- | --- |
| 回測引擎＋metrics（`run_backtest`） | 600（seed 20261004）＋3,000（seed 7） | 2–300 根 bar、小時／日線、跨月、跳空；long／short／both；close／nextOpen；sizing 0–1；手續費、滑價；約一半帶 SL／TP；起始資金；from／to 子區間。前 600 例中 376 例有交易，共 6,471 筆。 | **0 差異** |
| 指標（SMA、EMA、WMA、RSI、MACD 三條、TR、ATR、BB 三條、STDDEV、HIGHEST、LOWEST、ROC） | 800 | 長度 1–200、價格 0.001–50,000、約 30% 含平盤段、週期 1–250（含大於長度） | **0 差異**（NaN／±Infinity 也逐一比對） |

敏感度：故意把一筆交易的 pnl 改動 1e-6，測試確實報告 1 例不同。

**測試工具的陷阱（請之後做 parity 的人注意）**：第一次跑指標差分時，STDDEV 在平盤段出現 17 例「一邊是 0、一邊約 1e-11」的差異。
追查後兩邊的 `sma`／`stddev` 演算法逐行相同，原因是輸入經過 JSON：Rust 的 `serde_json`（未啟用 `float_roundtrip`）把長小數讀成差 1 ulp 的值，
平盤段因此不再完全平，標準差就出現 1 ulp 的殘差。改用 IEEE-754 位元（hex）傳遞輸入後，差異全部消失。
這是既有的 `NUMERIC-JSON-001` 現象，不是引擎差異；但它說明：**以 JSON 傳遞浮點輸入的 parity 測試，在接近 0 的輸出上會出現假差異**。

可重跑的工具保存在 [差分測試 patch](2026-10-04-pr066-075-differential-harness.patch)（暫時的 TS 產生器兩個、Rust 測試一個；指標輸入已用位元傳遞）。

## Review Notes

- **測試覆蓋建議（低優先）**：`fixtures/rs-core/indicators-v1.json` 只有**一個**案例（48 根 bar、一組固定週期）。
  本次隨機測試證明了平盤、長度 1、週期大於長度、週期 1 等情境目前兩邊一致，但 committed fixture 沒有鎖住這些語意；
  日後任何一邊改動，CI 也不會發現。建議在 fixture 加幾個這類邊界案例（輸入需能精確表示，避免上述 JSON 陷阱）。
- 差分測試**沒有**涵蓋基準／Random Entry（#71）與 Gate／Score（#72）：它們的輸入結構複雜，而既有 fixture 已有大量手寫邊界案例
  （Gate 22、Score 4、複雜度 6、錯誤 27；PRNG 5、Random Entry 6、錯誤 8）。若要擴大，可沿用 patch 的做法。

## Per-PR acceptance

| PR | 內容 | 先前驗收 | 結論 | CI run |
| --- | --- | --- | --- | --- |
| [#66](https://github.com/yoyoCadence/AlphaFactorForge/pull/66) | discovery runner 設計提案 | — | 可接受 | 29677688880 |
| [#67](https://github.com/yoyoCadence/AlphaFactorForge/pull/67) | durable identity v2 | PR 留言 1 則 | 可接受 | 29678617449 |
| [#68](https://github.com/yoyoCadence/AlphaFactorForge/pull/68) | RS-CORE-001 指標 | Claude 驗收：通過 | 可接受；差分 800 例一致 | 29679763209 |
| [#69](https://github.com/yoyoCadence/AlphaFactorForge/pull/69) | RS-CORE-002 回測與 metrics | Codex 三輪：通過 | 可接受；差分 3,600 例一致 | 29685870130 |
| [#70](https://github.com/yoyoCadence/AlphaFactorForge/pull/70) | RS-CORE-003 訊號／切分／embargo | Codex 多輪：通過 | 可接受 | 29699935715 |
| [#71](https://github.com/yoyoCadence/AlphaFactorForge/pull/71) | RS-CORE-004 基準與 Random Entry | 無 PR 留言（tasks.md Done） | 可接受 | 29909523050 |
| [#72](https://github.com/yoyoCadence/AlphaFactorForge/pull/72) | RS-CORE-005 Gate／Score | 無 PR 留言（tasks.md Done） | 可接受 | 30190755722 |
| [#73](https://github.com/yoyoCadence/AlphaFactorForge/pull/73) | 探索准入與候選列舉 | 三輪 review 修正 | 可接受 | 30200193108 |
| [#74](https://github.com/yoyoCadence/AlphaFactorForge/pull/74) | run／job 儲存層 | 三輪 review＋Codex；合併後更正 | 可接受 | 30545742443 |
| [#75](https://github.com/yoyoCadence/AlphaFactorForge/pull/75) | 文件 | — | 可接受 | 30550925985 |

（CI run 連結格式：`https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/<id>`。）

## Verification

- 十組 `git diff <head> <merge>` 都是空的；各 head 的 CI 都是 success。
- 差分測試：`cargo test --locked --lib review_rust_backtest`（兩個 seed）與 `review_rust_indicators`，結果如上表；跑完已移除所有暫時檔案，工作區與 baseline 相同，`git apply --check` 確認 harness patch 可套用。
- 全套測試沿用整合 baseline 的結果（見 [#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)）。

## Reproduction (optional)

```powershell
git apply handoffs/2026-10-04-pr066-075-differential-harness.patch
Set-Location alpha-factor-forge
$env:OUT = "$env:TEMP\diff-backtest.json"; npx vite-node scripts/_review-diff-backtest.ts
$env:OUT = "$env:TEMP\diff-indicators.json"; npx vite-node scripts/_review-diff-indicators.ts
Set-Location src-tauri
$env:AFF_REVIEW_DIFF_JSON = "$env:TEMP\diff-backtest.json"; $env:AFF_REVIEW_DIFF_IND_JSON = "$env:TEMP\diff-indicators.json"
cargo test --locked --lib review_rust_ -- --nocapture
```

預期兩個測試都通過（`0 differ`）。用完以 `git apply --reverse` 移除；`mod.rs` 會多一行 `mod review_diff_tests;`，也一併移除。

## Resolution

Informational.

## Follow-up work order（2026-10-04 追加）

本檔的發現已整理進 [驗收後續工作單](2026-10-04-acceptance-followups-work-order-v1.md)，請接手的 agent 從那裡開始：

- 指標 fixture 覆蓋建議 → **FU-7**（可選），需要 **D7**；新增案例的輸入值必須能被 Rust 精確讀回，理由見本檔的「測試工具的陷阱」。

### Resolution — FU-7（2026-10-04）

指標 fixture 已在 `test/indicator-fixture-edges` 補上四個手寫邊界案例（`flat-30-bars`、`single-bar`、`period-one-12-bars`、`periods-longer-than-series`），輸入只用整數或 .5，Rust 能精確讀回；原 seed-42 案例逐位元不變。Rust 測試改為逐案執行全部指標（精確案例清單），TS 測試加上各案例的語意斷言。突變驗證：把 Rust RSI 在零損失時的 100 改成 NaN，新案例 `flat-30-bars.rsi[15]` 會失敗——原本只有一個案例時抓不到。
