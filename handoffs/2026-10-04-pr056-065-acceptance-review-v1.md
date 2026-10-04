# Handoff: PR #56–#65 整合驗收（往前驗收 batch G）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`
Status: 驗收完成；十個 PR 都可以接受，本次**沒有可觸發的缺陷**。有一項低優先的契約不一致（G-N1，產品路徑無法觸發）。

## Summary

這十個 PR 是 TS 端驗證流水線：Train／Validation／Test 切分接入回測（VAL-002 #56）、依用途推導 embargo（VAL-003 #57）、
固定基準組（BENCH-001 #58）、Random Entry Monte Carlo（BENCH-002 #59）、硬性淘汰 Gate（GATE-001 #60）、
METRIC-001 的 sortino／calmar 與非有限值保存（#62）、score-v1（#63）、不可變驗證紀錄（PERSIST-001 #65），以及兩份設計決策（#61、#64）。
十個合併 commit 的 tree 都與各自的最終 head 相同，所有 head 的 CI 都通過。

#61、#64 有 Codex 的裁決並已轉錄；#65 經過三輪驗收才通過。**#56–#60、#62、#63 沒有 PR 審查留言**，
但它們的邏輯之後都被移植到 Rust 並以 parity fixture 鎖定（batch F 的 #70、#71、#72，其中 #70 經 Codex 多輪驗收），
驗證紀錄也在 #89 改為嚴格的版本化驗證。所以這些邏輯其實已經被第二次實作與審查過。

## 本次重點檢查：防止未來資料洩漏

AGENTS.md 把「回測正確、不得使用未來資料」列為最高風險。本次檢查 embargo 推導（`src/services/embargo.ts`）與切分（`src/core/validation/split.ts`）：

- embargo = 策略**實際使用**的訊號最慢暖機 + 交叉類訊號多 1 根 + 呼叫端核准的持倉延伸；未使用的週期不會膨脹 embargo。
- 所有加法都先檢查是否超出安全整數範圍再相加（避免 `a + b - 1` 的進位抵消，PR #70 review 的教訓）。
- `planValidationSplit` 要求 embargo 是非負安全整數，並在 Train／Validation、Validation／Test 之間各放一段 embargo。
- 已知、且有文件記錄的 v1 慣例：EMA 與 RSI 的指數尾巴比 embargo 長，Validation 開頭的值仍帶有極少量 Train 期的影響（`embargo.ts` 開頭的註解明確接受）。這不是本次的新發現；若要更嚴格，需要另開版本化的契約變更。

### G-N1 — 低優先：`deriveEmbargoBars` 遇到無效的左運算元會回傳 NaN，而不是如註解所說「fail closed（throw）」

- 位置：`src/services/embargo.ts`，blocks 模式的 `blocksRuleLookback` 直接把 `rule.l` 交給 `operandLookback`，而那個 `switch` 沒有 default。
- 重現（暫時腳本）：`l` 為 `'50'`、`'atr'` 或 `' maFast '` 時，`deriveEmbargoBars` 回傳 `{embargoBars: NaN, maxSignalLookbackBars: NaN}`；`'price'` 正常為 1。
- 為什麼不可觸發：策略庫載入時會以 `OPERAND_IDS` 驗證 `l`（`strategyLibrary.ts:44`），UI 也是下拉選單；
  而且 `planValidationSplit` 會拒絕 NaN；production 也沒有呼叫這個 TS 函式（只有 parity fixture 產生器與 mock 資料，真正的 runner 用 Rust 的推導）。
- 建議：在 `operandLookback` 加上 default 分支並 throw，讓行為與註解一致；屬於防禦性修正，不急。
  注意 `buildBlocksSignals` 會 `trim()` 運算元而 embargo 推導不會，兩邊對空白的處理也不一致；同樣因載入驗證而不可觸發。

## Per-PR acceptance

| PR | 內容 | 先前驗收 | 結論 | CI run |
| --- | --- | --- | --- | --- |
| [#56](https://github.com/yoyoCadence/AlphaFactorForge/pull/56) | VAL-002 切分接入回測 | 無 PR 留言；之後 Rust parity（#70） | 可接受 | 29648254807 |
| [#57](https://github.com/yoyoCadence/AlphaFactorForge/pull/57) | VAL-003 embargo 推導 | 無 PR 留言；之後 Rust parity（#70） | 可接受；附 G-N1 | 29648653823 |
| [#58](https://github.com/yoyoCadence/AlphaFactorForge/pull/58) | BENCH-001 固定基準 | 無 PR 留言；之後 Rust parity（#71） | 可接受 | 29660996410 |
| [#59](https://github.com/yoyoCadence/AlphaFactorForge/pull/59) | BENCH-002 Random Entry | 無 PR 留言；之後 Rust parity（#71） | 可接受 | 29661594620 |
| [#60](https://github.com/yoyoCadence/AlphaFactorForge/pull/60) | GATE-001 | 無 PR 留言；之後 Rust parity（#72） | 可接受 | 29662609744 |
| [#61](https://github.com/yoyoCadence/AlphaFactorForge/pull/61) | SCORE-001 設計提案 | Codex 裁決已轉錄 | 可接受 | 29663583049 |
| [#62](https://github.com/yoyoCadence/AlphaFactorForge/pull/62) | METRIC-001 | 無 PR 留言；之後 Rust parity（#69） | 可接受 | 29664364611 |
| [#63](https://github.com/yoyoCadence/AlphaFactorForge/pull/63) | score-v1 | 無 PR 留言；之後 Rust parity（#72） | 可接受 | 29664840981 |
| [#64](https://github.com/yoyoCadence/AlphaFactorForge/pull/64) | PERSIST-001 提案 | Codex 裁決已轉錄，合併前的兩項後續已處理 | 可接受 | 29674115972 |
| [#65](https://github.com/yoyoCadence/AlphaFactorForge/pull/65) | 不可變驗證紀錄 | 三輪驗收後通過；之後 #89 升級為 v2 嚴格驗證 | 可接受 | 29676562575 |

（CI run 連結格式：`https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/<id>`。）

## Verification

- 十組 `git diff <head> <merge>` 都是空的；各 head 的 CI 都是 success。
- G-N1 以暫時的 vite-node 腳本重現後已刪除，工作區與 baseline 相同。
- 全套測試沿用整合 baseline 的結果（見 [#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)）。

## Resolution

Informational; G-N1 is optional hardening.
