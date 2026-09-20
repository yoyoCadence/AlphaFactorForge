# Handoff: P08 驗收審查 — 通過，三項觀察

Date: 2026-09-20
Repo: yoyoCadence/AlphaFactorForge
Reviewed: PR [#110](https://github.com/yoyoCadence/AlphaFactorForge/pull/110)，branch `feat/p08-etf-market-semantics`，head `b8169e7`（base main `bfb4216`）
Reviewer: Claude Code（獨立驗收，未修改產品程式碼）
Status: **通過（可 merge）**。無 P1／P2 defect；三項為觀察事項，不阻擋合併。

## 結論

P08 交付的是**純計算的 ETF 日線語意契約**，範圍與 plan §5 P08 一致，且沒有超出：沒有 migration、
沒有新相依、沒有動到既有 hash／golden／metrics 版本、沒有新增 Tauri 命令或 UI 路徑、沒有下載任何
真實 ETF 資料。文件對「還沒有什麼」的陳述誠實（README、tasks.md、handoff 都明確說 P09／P10 才有真實
資料與 calendar，P18 才有成交與帳務）。

驗收準則「配息／分割／休市／付款日反例及 TS／Rust parity 通過」四類反例都存在並通過，我另以獨立反例
複驗了其中最吃重的財務不變量（見下）。

## 我實際驗證了什麼

### 1. 重跑交接文件宣稱的每一項數字

| 項目 | 交接宣稱 | 我實測 |
| --- | --- | --- |
| `npm test` | 969 | **969 passed**（53 files） |
| `npm run typecheck` / `build` | pass | pass |
| `cargo test --locked` | 337（71＋264＋2） | **337 passed**（71＋264＋2） |
| `cargo clippy --locked --all-targets` | 只有 5 項既有 | **確認**：backtest ×2、score ×2、file_commands ×1，P08 新增 0 項 |
| PR CI | — | **六項全綠**（run 35510342043，含 e2e 與 native-smoke） |
| `npm run e2e` | 78/78 | **77/78**（見觀察 3） |

### 2. 獨立反例（非重跑既有測試）

以臨時探針直接打公開 API，Rust 4 個、TS 3 個，涵蓋 plan §4.1／market-contract §5 的財務不變量。
**全部通過，且兩個語言在同一情境下結果一致**（探針內容見文末，執行後已移除，工作區無殘留）。

- **分割不得產生虛假收益**：2:1、1:10（反向）、3:1 三種比例下，`apply_split` → `value_etf_position`
  的權益完全守恆（誤差 < 1e-9）；`costBasis` 總額不變（正確：每股成本變、總成本不變）；
  未成交限價單的名目金額（數量 × 限價）不變。
- **配息**：除息日以**當時股數**登記應收，付款日才轉現金，兩步驟權益都不跳動；`paymentDate` 未知時
  `payDividend` 回 `unknown_payment_date`，無法變成可用現金；**除息後賣光股票仍保有應收款**。
- **訊號因果性**：只有「已生效**且**在資料切點前已可觀測」的分割會調整價格；生效日當根 bar 不調整；
  一個尚未可觀測的分割完全不影響序列；切點之後的 bar 直接拒絕（`invalid_raw_bar`）而非悄悄丟棄。
- **日曆是證據不是公式**：超出 `calendarFrom`／`calendarToExclusive` 的區間回 `calendar_out_of_range`，
  不外推假日；停牌是「不存在的交易日」而不是缺漏。

### 3. 對既有契約的影響（回歸風險）

- `market_foundation.rs` 與 `foundation.ts` 的改動**逐字檢視過：只有註解**（修正 P06 當時「盤中 session
  屬 P08」的過時敘述），無行為變更。
- `computeEtfMetrics` 的 `annualizedVolatility` 與舊 `computeMetrics` 的 `sharpe` 使用**同一組報酬序列**
  （兩者都從 `startEquity` 起算），且 `std` 與新 vol 都是母體（除以 n），沒有兩個口徑並存的問題。
- 舊 `barsPerYear('1d') = 365` 與 `metrics-v2` 未變，且有測試釘住。

### 4. 流程面

- `docs/plans/active-plan.md` 的修改是**檔頭附加的執行紀錄**，明文保留原始規劃基準並聲明
  「P09 及後續未啟動，整體最終 Acceptance Criteria 尚未全部完成」——符合附加式編輯規則，不是改 scope。
- fixture 使用 `fixture-nyse-v1` 這類明確標示的測試日曆，**沒有捏造任何真實休市資料**。

## 觀察事項（不阻擋合併）

1. **P3 — parity fixture 沒有 generator，也沒有 build-time 覆蓋自檢。**
   其餘每個 `fixtures/rs-core/*.json` 都由 `src/parity/*Fixture.ts` 產生、有 `npm run fixtures:*`
   指令、並被「能從當前 generator 原始碼逐位元重現」的測試釘住，另有覆蓋率自檢（例如「每個 rule id 都
   至少有一列拒絕案例」）。`etf-semantics-v1.json` 是手寫 JSON：envelope 誠實寫明來源，兩個語言也確實
   讀同一份，但**沒有重生成路徑**，日後擴充時也沒有機械保證案例矩陣仍覆蓋所有規則。
   建議：在 `docs/rs-core-parity.md` 明寫此偏離與理由（目前只宣告新 fixture 存在），或日後補 builder。
2. **P3 — `computeEtfMetrics` 不檢查 equity 點是否落在該市場的交易日網格上**，卻以 `sessionsPerYear`
   年化波動。若呼叫端傳入週頻估值，會得到以日頻年化的數字。該設定是契約明示且凍結的，組合責任在 P18，
   因此這是**邊界**而非缺陷；建議在 `docs/etf-semantics-v1.md` 明寫「equity 點必須是逐交易日估值」。
3. **觀察 — 交接文件寫 e2e 78/78，我本機整套跑是 77/78。**
   失敗的是 `results-explorer.spec.ts`（P01 的畫面，P08 完全沒碰），單獨重跑 **7/7 通過**，
   PR CI 的 e2e lane 也是綠的 → 判定為本機 flake，**不歸因於 P08**。建議交接文件對本機 e2e 數字加註
   flake 可能性，避免日後把「本機 78/78」當成硬性事實。

## 我沒有驗證的事（不得據此推論）

- **沒有任何真實 ETF 資料**進入驗證：P08 全部是合成 fixture，真實 calendar／公司事件／權限屬 P09／P10。
  因此「ETF 語意正確」的證據只到合成案例為止。
- 未逐條審查 55 個 fixture 案例的期望值；我的作法是抽查機制並**另外獨立出題**複驗。
- 未重跑原生桌面 UI smoke（P08 無桌面路徑；CI 的 native-smoke 已綠）。
- 未評估策略績效或任何晉級資格——P08 本來就不產生這些。

## 建議

merge PR #110。合併後的下一個可執行 phase 是 **P09 US ETF adapter**，但它在使用者開通 Tiingo 帳戶前
維持阻擋（registry §3）；若要先做不被阻擋的，P10（FinMind＋TWSE）或 P11（可執行 DSL）依賴都已滿足。

## 臨時探針（執行後已移除，保留於此以便重現）

Rust：`src-tauri/src/discovery_core/p08_review_probe.rs`（於 `discovery_core/mod.rs` 以
`#[cfg(test)] mod p08_review_probe;` 註冊），四個測試：
`probe_split_conserves_equity_and_resting_order_notional`、
`probe_dividend_accrual_and_payment_conserve_equity`、
`probe_signal_prices_are_causal_at_the_cut`、
`probe_sessions_never_extrapolate_past_the_evidence`。

TypeScript：`src/core/market-data/p08-review-probe.test.ts`，三個測試（分割守恆、配息守恆、訊號因果）。

兩份檔案的完整內容存於審查者暫存區；移除後 `git status` 乾淨，`discovery_core/mod.rs` 已還原。
