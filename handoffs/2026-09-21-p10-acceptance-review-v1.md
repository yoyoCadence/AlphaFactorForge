# Handoff: P10 驗收審查 — 通過，五項觀察

Date: 2026-09-21
Repo: yoyoCadence/AlphaFactorForge
Reviewed: PR [#113](https://github.com/yoyoCadence/AlphaFactorForge/pull/113)，branch `feat/p10-finmind-tw-etf-adapter`，
implementation head `9cbf0ee`（＋文件 commit `adcd586`；base main `373bba1`）
Reviewer: Claude Code（獨立驗收，未修改產品程式碼）
Status: **通過**。無 P1／P2 defect；五項為觀察事項，不阻擋合併。合併時機由使用者決定，本次未 merge。

## 結論

範圍與 active-plan §5 P10（`FinMind、TWSE 核對、ETF 事件與交易日`）一致，沒有超出：
無 migration、無新 crate（`Cargo.toml`／`Cargo.lock` 未變動）、無 schema／UI／策略引擎／
scheduler 變更。既有 `http.rs` 只多一個公開 host allowlist 建構子，`service.rs` 只加一條
`fetch-tw-etf` 路徑，Binance 與 Tiingo 路徑不受影響。

P10 的驗收／停止點是「**民國日期、成交量單位、0050 分割／停牌、配息案例通過**」。四項全部
成立，而且是以**真實公開來源**驗證的——這是本 phase 與 P09（受限於私人 token）最大的差別。

## 我實際驗證了什麼

### 1. 重跑交接文件宣稱的每一項數字

| 項目 | 交接宣稱 | 我實測 |
| --- | --- | --- |
| `npm.cmd test` | 969 | **969 passed**（53 files） |
| `npm.cmd run typecheck` / `build` | pass | pass |
| `cargo test --locked` | 366（71＋293＋2） | **366 passed**（71＋293＋2） |
| 其中 P10 新增 | 13 | **13**（`tw_etf_ingest` 8、`service` CLI 1、`finmind` 2、`twse` 2） |
| `cargo clippy --locked --all-targets` | 僅既有 5 warnings，新增 0 | **確認**：`backtest.rs` ×2、`score.rs` ×2、`file_commands.rs` ×1，位置逐一相符 |
| `npm.cmd run e2e` | 78/78 | 未於本機重跑（Windows 本機平行 flake 已知），以 **CI e2e 綠燈**為證據 |
| PR CI | 建立時啟動 | **六項全綠**（run 35603510636：build／test／typecheck／e2e／cargo-check／native-smoke） |

### 2. 真實公開來源 smoke（我自己重跑，非引用交接結果）

於隔離 workspace 執行 `fetch-tw-etf --from 2025-06-09 --to 2025-06-21`，結果與
`docs/market-source-tw-etf-v1.md` §6 的表**完全一致**：

| 商品 | bars | TWSE 月份 | 事件 | 狀態 |
|---|---:|---:|---|---|
| 0050 | 5／5 | 1 | 2025-06-18 4:1 分割 | DEGRADED：僅 `costs_unconfirmed` |
| 006208 / 0056 / 00878 | 10／10 | 1 | 無 | DEGRADED：僅 `costs_unconfirmed` |
| 00713 | 10／10 | 1 | 6/20 除息 1.1 TWD、7/11 付款 | DEGRADED：僅 `costs_unconfirmed` |

exit 5、coverage 無 blocking、五檔各 5 筆 provenance（4 FinMind＋1 TWSE）＝共 25 次 GET。
0050 snapshot：`priceBasis=raw`、`kind=historical`、`calendarId=twse-2025-v1`、
`corporateActionVersion` 已設定而 `costProfileVersion` 為 null——degraded 的唯一來源確實是成本。

再讀出落盤的 FinMind 原件，逐列確認**原始價未被調整**：

```
2025-06-09 C=183.70 vol=14,115,012
2025-06-10 C=188.65 vol=31,483,080
2025-06-18 C= 47.57 vol=252,639,825   ← 4:1 分割後，未回補、未還原
```

6/11–6/17 不存在且 coverage 不視為缺漏（停牌證據生效），成交量保持**股**而非千股。

### 3. 官方日曆的機械核對

以設定檔所引用的官方端點（`twse.com.tw/rwd/zh/holidaySchedule?date=2025`）實際抓取後，
程式化比對範例設定的 `twse-2025-v1`：

- 官方 24 列扣除三個「開始／最後交易日」說明列與週末後，**平日休市恰為 18 天**；
- 範例設定的 `holidays` **恰為同樣 18 天**，無遺漏、無多列；
- 2025 年平日 261 天 − 18 = **243**，與設定的 `sessionsPerYear: 243` 一致。

（依既有教訓，此處用程式比對集合差集，不以人工目視核對日期。）

### 4. 獨立探針（reviewer 自撰，執行後已移除，工作區無殘留）

七項吃重不變量，**全部通過**：

- **反分割方向與比例**：設定 0.25 ＋ 來源 `反分割` ＋ published ratio 0.25 → 取得資格；
  同樣數字但宣告成 `分割` → `split_evidence_mismatch`；宣告 4:1 而來源價格只含 2:1 →
  一樣阻擋。方向錯置會在後續調整中製造 16 倍假報酬，這是本 PR 風險最高的單點。
- **範圍邊界精確性**：31 天通過、32 天拒絕；完結門檻在 `end+08:00 UTC` **當下通過、早 1 毫秒拒絕**。
- **TWSE 月份涵蓋**：跨年（2025-12 → 2026-01）與跨月皆正確列舉，不會漏抓月份。
- **一分錢的官方歧異即阻擋**：TWSE 收盤價改 47.66（差 0.01）→ `twse_value_mismatch`，
  不建立 dataset、不建立 snapshot。
- **月資料範圍外的列不進 dataset**：TWSE 月回應多一列 6/24，仍取得資格且 bars 維持 5，
  candles 全部早於 `to`。
- **股票股利 fail closed／現金兩欄相加**：`0.30 + 0.06 = 0.36`；任何股票股利 →
  `unsupported_stock_distribution`。
- **429 停止整批**：第一個 endpoint 429 後只發出 1 次請求，後續商品全部標記
  `quota_exhausted`，不重試、不部分下載。

### 5. 範圍與文件一致性

20 檔、+2460／−22。新程式碼集中在 3 個新檔＋1 個測試檔＋3 處加法式接線。
`tasks.md`／active-plan／roadmap／README／`STRATEGY_DISCOVERY.md`／market contract／
capability registry／changelog 均已同步，且**未誇大**：所有文件都明說範例成本未確認、
snapshot 刻意 degraded、12 日 smoke 不代表全年或多年完整性。`tasks.md` 甚至補了一行
修正長清單裡過時的測試總數。P10 在 roadmap 標為 **Done** 是成立的——與 P09 不同，
P10 的來源是公開的，外部驗收確實可以完成而且已經完成。

## 觀察事項（不阻擋合併）

1. **`PriceRow` 用 `deny_unknown_fields`，同檔其他三個 FinMind dataset 卻是按 key 讀取。**
   FinMind 若在 `TaiwanStockPrice` 增加任一欄位，五檔會全部以
   `invalid_finmind_price_schema` 停擺（而非 degraded）。方向是安全的，但這是「來源加欄位
   ＝服務中斷」，且與 P09 `EodRow` 容忍未知欄位的作法不一致。建議之後統一策略。
2. **範例設定的 `corporateActionsConfirmed` 五檔皆為 `true`**（P09 範例是 `false`）。
   目前因 `costs: null` 不會取得資格，但 operator 一旦填入成本，就等於**繼承了別人對整個
   2025 年公司事件完整性的確認**。建議在契約文件明寫：確認成本之前必須自行重新核對事件清單。
3. **配息／分割端點以整個日曆年查詢，核對卻只涵蓋請求區間。** 區間外的事件會被下載並保存
   為原件，但不做雙向核對。目前無害；日後拉長區間時會重新下載並重新核對同一批事件。
4. **事件只存在於 `tw-etf-report` 原件內，snapshot 的 `corporateActionVersion` 是雜湊承諾
   而非可解析指標。** P18 要消費 split／dividend 時，需要一條由 instrument＋source 找到
   report artifact 的查詢路徑；目前沒有專用資料表。
5. **`months()` 可能多抓一個沒有任何範圍內列的 TWSE 月份**（例如視窗起點落在某月最後一個
   非交易日）。只多一次公開 GET，無正確性影響。

## 尚未證明的部分

- 12 日視窗不代表 2025 全年或多年完整性；範例只宣告 2025，跨年需新 calendar 版本與新證據。
- 成本、`priceStep`、`minNotional` 仍未知；未做策略回測、排程或 paper。
- FinMind 公開額度有限，單次嘗試、429 停批，不保證大範圍回補能一次完成。
- P09 Tiingo 認證來源驗收仍獨立 In Progress，P10 完成並未解除該 blocker。

## 下一步

1. 由使用者決定 PR #113 的合併時機；本次未 merge，PR 維持 draft。
2. 合併後若要實際使用，operator 需依實際券商填入 `costs` 並**自行重新核對** 2025 事件清單
   （見觀察 2），才可能離開 `costs_unconfirmed`。
3. P11（可執行 DSL）依計畫需要新的明確授權，本次未啟動。
