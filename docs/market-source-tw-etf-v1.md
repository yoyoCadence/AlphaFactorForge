# 台灣 ETF 資料來源契約 v1

狀態：**已實作（P10，2026-09-21）**。本文件描述
`finmind-tw-etf-v1`／`tw-etf-settings-v1` 的來源、CLI、稽核及限制。

## 1. 範圍

- 商品：0050、006208、0056、00878、00713。
- 頻率：日線 `1d`；每次 `--from`（含）到 `--to`（不含）最多 31 個日曆日。
- 主要資料：FinMind 公開 `TaiwanStockPrice` **原始價**；不呼叫付費還原價接口。
- 獨立核對：TWSE `STOCK_DAY` 月行情。TWSE 只產生 comparison evidence，絕不補 FinMind 缺漏。
- 交易日／事件：FinMind `TaiwanStockTradingDate`、`TaiwanStockDividend`、
  `TaiwanStockSplitPrice`，並與設定檔內有 TWSE 證據的年度 calendar、停牌、配息及分割清單雙向核對。
- 原生幣別：TWD；price basis 固定 `raw`。

這是 operator-triggered 歷史資料 adapter，不是 scheduler、即時報價、paper 帳戶或另一套回測引擎。

## 2. 使用方式

```powershell
cd alpha-factor-forge/src-tauri
cargo run --locked --bin alpha-factor-forge-service -- `
  fetch-tw-etf `
  --settings ..\config\tw-etf-2025.example.json `
  --from 2025-06-09 `
  --to 2025-06-21 `
  --data-dir <isolated-workspace>
```

CLI 逐檔輸出 JSON。五檔都具研究資格才回傳 exit 0；任何 BLOCKED／DEGRADED
回傳 exit 5。範例設定的成本刻意是 `null`，因此來源與事件通過時仍是
`DEGRADED / costs_unconfirmed`，不能為了得到 exit 0 虛構成本。

`tw-etf-2025.example.json` 僅涵蓋 2025；超界拒絕，不外推下一年。新年度須以官方
TWSE 休市與事件資料建立新 calendar id／設定檔。

## 3. 來源與欄位契約

### FinMind primary

固定 host：`api.finmindtrade.com`；公開 v4 data endpoint，不攜帶 token。

| dataset | 用途 | 必要判定 |
|---|---|---|
| `TaiwanStockPrice` | dataset 的 raw OHLCV | `stock_id`、日期與範圍完全一致；OHLC 必須為正且合理；`Trading_Volume` 是**股數**，保持整數，不除以 1,000 |
| `TaiwanStockTradingDate` | 核對設定 calendar | 要求範圍的日期集合須完全等於 calendar 的全市場 sessions；個別 ETF 停牌另由 suspension 排除 |
| `TaiwanStockDividend` | 現金配息／付款日 | 現金盈餘＋公積合計；股票股利在 v1 拒絕；確認模式下與 TWSE 證據清單雙向一致 |
| `TaiwanStockSplitPrice` | 分割觀測 | 日期／類型與 TWSE 證據一致；來源 `before_price / after_price` 要與官方 ratio 在 1% 內 |

FinMind 上游無價格時可能發布 0 OHLC。v1 回報
`unpublished_or_invalid_price` 並停止該檔，不 forward-fill、不沿用前收，也不切換到還原價。

### TWSE comparison

固定 host：`www.twse.com.tw`；每個跨越月份各取一次
`/rwd/zh/afterTrading/STOCK_DAY`。Parser 依欄名定位，將民國日期（例如
`114/06/18`）轉成 `2025-06-18`，保留成交股數／金額／筆數的整數單位；可辨識
TWSE 的前置空白 ` 0.00` 與除權息標記 `X0.00`。

要求範圍內，日期集合、OHLC、漲跌價差、成交股數、成交金額及成交筆數必須逐列一致。
任一不符即 `twse_date_set_mismatch`／`twse_value_mismatch`，不建立 dataset。
月資料範圍外的列只留在原件，不進 dataset。

## 4. 設定與公司事件證據

設定採 `deny_unknown_fields`，不能放 token。每檔必須明示：

- `tw-etf:twse:<symbol>`、TWD、`Asia/Taipei`、calendar 有效期與年化 sessions；
- 上市日及 TWSE ISIN 證據；
- TWSE 休市日及來源；
- 停牌區間與證據；
- `confirmedDividends` 的除息日、付款日、每受益權單位金額與 TWSE 證據；
- `confirmedSplits` 的生效日、ratio 與 TWSE 證據；
- `corporateActionsConfirmed` 及其查核來源；
- 可選、原幣一致且 `confirmed` 的成本設定。

確認模式下，FinMind 少事件、設定少事件、金額／付款日／ratio 不同都會阻擋。
未確認事件或付款日未知時，P08 語意閘門保持 degraded。0050 的範例明示
2025-06-11～17 停止交易、2025-06-18 以 4:1 分割恢復；停牌不算缺 bar。

## 5. 保存與修訂

- 每個 HTTP response 在解析結果回傳前就寫入 P06 `market_provenance` 及內容定址 artifact store；被拒絕的原件也保留。
- 相同 endpoint／範圍的重取形成線性 `revision_of`，不覆寫舊 bytes。
- FinMind 原件是 primary snapshot component；TWSE 是不可混入 series 的 comparison evidence。
- 每檔另保存 `tw-etf-report`，包含全部 provenance ids、coverage、事件、dataset 及 snapshot 結果。
- action version hash 納入完整設定、事件、TWSE 核對來源及報告範圍；snapshot 固定 raw basis。
- 不新增 migration、依賴或 frontend secret；沿用 P03 ownership、P05 artifact store、P06 snapshot、P08 ETF 語意。

## 6. 真實來源驗證（2026-09-21）

在隔離 workspace 執行 `2025-06-09`～`2025-06-21`：

| 商品 | bars／預期 | TWSE 月份 | 範圍內事件 | 結果 |
|---|---:|---:|---|---|
| 0050 | 5／5 | 1 | 2025-06-18 4:1 分割；6/11～17 停牌 | DEGRADED：僅成本未確認 |
| 006208 | 10／10 | 1 | 無 | DEGRADED：僅成本未確認 |
| 0056 | 10／10 | 1 | 無 | DEGRADED：僅成本未確認 |
| 00878 | 10／10 | 1 | 無 | DEGRADED：僅成本未確認 |
| 00713 | 10／10 | 1 | 6/20 除息 1.1 TWD、7/11 付款 | DEGRADED：僅成本未確認 |

所有五檔都建立 raw dataset 與 historical snapshot，coverage 無 blocking；exit 5 是
範例沒有確認成本的預期結果。隔離 workspace 驗證後已刪除，沒有提交市場原件。

## 7. 限制

- 公開 FinMind 額度有限；transport 每 endpoint 只嘗試一次，429 會停止後續 batch，留待 operator 稍後重跑。
- 31 日範圍最多可碰到三個月份；每檔 4 個 FinMind request 加每月 1 個 TWSE request，五檔最多 35 次 GET。
- v1 僅接受現金配息；若來源出現股票股利會 fail closed，須另開 phase 擴充 P08 語意。
- calendar／事件證據由年度設定維護；臨時休市或官方修訂必須新增設定／calendar version，不修改既有版本。
- `lotSize=1` 僅表示最小受益權單位；`priceStep` 與 `minNotional` 仍未知，不能據此宣稱 paper-ready。
- 這次真實 smoke 是 12 日邊界案例，不代表 2025 全年或多年完整，也沒有執行策略回測、排程或 paper。

官方參考：

- [FinMind 台股技術面資料](https://finmind.github.io/tutor/TaiwanMarket/Technical/)
- [TWSE 個股日成交資訊](https://www.twse.com.tw/zh/trading/historical/stock-day.html)
- [TWSE 休市日程](https://www.twse.com.tw/rwd/zh/holidaySchedule/holidaySchedule?response=json&date=2025)
- [TWSE ETF 配息清單](https://www.twse.com.tw/zh/ETFortune-institute/dividendList)
- [TWSE ISIN 上市資料](https://isin.twse.com.tw/isin/e_single_main.jsp)
