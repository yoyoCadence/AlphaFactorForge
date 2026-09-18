# Market contract v1（`market-instrument-v1` / `market-provenance-v1` / `market-snapshot-v1`）

> 由 P00（契約與相容性預檢，2026-09-16）登記。上游規劃：
> [`plans/active-plan.md`](plans/active-plan.md) §3.3 資料群組、§4.1 市場與資料來源、
> §4.6 EOD 語意。既有契約：[`market-data-quality-contract.md`](market-data-quality-contract.md)
> （`market-data-quality-v1`）、[`durable-identity-contract.md`](durable-identity-contract.md)
> （`dataset-content-v2`）。本機驗證證據：
> [`autonomous-research-capability-registry.md`](autonomous-research-capability-registry.md) §3。

**狀態：契約已定案，尚未實作。** 這是 P06（資料基礎契約）、P07（Crypto adapter）、
P08（ETF 市場語意）、P09／P10（ETF adapters）的共同輸入；P00 不匯入任何正式資料、
不改 hash preimage、不改 `barsPerYear`。

---

## 0. 不變的既有契約

| 既有 | 位置 | 關係 |
| --- | --- | --- |
| `dataset-content-v2` hash preimage | `src/core/hashing`、`src-tauri/src/identity.rs` | 不改寫。新市場語意以新版本識別；同一組 candles 的 hash 維持不變 |
| `market-data-quality-v1` | `src/core/market-data`、`discovery_core/market_data.rs` | 保留為 candles 層的 plausibility gate；本契約在其**之上**增加 instrument／calendar／provenance 語意 |
| `datasets` / `candles` 表（0001） | `src-tauri/migrations/0001_init.sql` | 不改；`exchange`／`symbol`／`interval`／`source` 欄位保留，新語意用新表關聯（0004+） |
| `barsPerYear('1d') = 365` | `src/services/backtestRunner.ts` | **保留於舊契約**（`metrics-v2`）。ETF 交易日年化以新版本 metrics 契約識別（P08），舊結果不重算 |
| 一致性：舊資料標 legacy | — | 既有 datasets 沒有 venue／quote／adjustment 資訊者標為 `legacy`；未知來源、未知調整方式**不能自動取得新資格** |

---

## 1. Instrument（`market-instrument-v1`）

每個可研究商品一筆，campaign 凍結其快照：

| 欄位 | 說明 | 範例 |
| --- | --- | --- |
| `instrumentId` | 標準化識別，`<market>:<venue>:<symbol>` | `crypto:binance:BTCUSDT`、`us-etf:nyse-arca:SPY`、`tw-etf:twse:0050` |
| `market` | `crypto` / `us-etf` / `tw-etf` | — |
| `venue` | 交易場所；不同 venue 永不合併成同一條資料 | `binance`、`coinbase`、`nyse-arca`、`twse` |
| `base` / `quote` | 幣別／計價幣；`USDT` 不改名 `USD` | `BTC`/`USDT`、`SPY`/`USD`、`0050`/`TWD` |
| `assetType` | `spot-crypto` / `etf` | 第一版不含融資、放空、期貨、選擇權 |
| `sessionCalendarId` | 版本化交易日曆 | `crypto-24x7-v1`、`nyse-v1`、`twse-v1` |
| `timezone` | 日線的「交易日」所屬時區 | `UTC`、`America/New_York`、`Asia/Taipei` |
| `lotSize` / `priceStep` / `minNotional` | 交易規格；缺值時該 instrument 不得進 paper | — |
| `listedFrom` / `delistedAt` | 上市／下市期間；campaign 凍結，不以今日存在的 ETF 代表歷史全市場 | — |
| `sourceCapabilities` | 各來源可提供的欄位（raw／adjusted／dividend／split）與實際權限 | 見 §4 |

### 1.1 預設研究清單（可編輯，不是投資建議）

| 市場 | 商品 | 週期 |
| --- | --- | --- |
| Crypto | BTCUSDT、ETHUSDT | 1h |
| US ETF | SPY、QQQ、VTI、TLT、GLD | 1d |
| TW ETF | 0050、006208、0056、00878、00713 | 1d |

---

## 2. Provenance（`market-provenance-v1`）

每次向來源取得資料保存一筆**不可變**原件紀錄：

| 欄位 | 說明 |
| --- | --- |
| `rawResponseHash` | SHA-256（原始 bytes，含 CHECKSUM 檔時一併保存） |
| `rawArtifactPath` | 相鄰非 OneDrive 資料目錄內的原件；staging → 校驗 → 原子更名 |
| `requestScope` | instrument、interval、要求的時間範圍、endpoint、參數 |
| `retrievedAt` | 取得時間（UTC） |
| `availableAt` | 該筆資料**最早可被觀測**的時間（EOD 為收盤後發布時間；歷史重新下載不冒稱 point-in-time） |
| `revisionOf` | 指向被修訂的前一筆原件；修訂不覆蓋，只新增 |
| `qualityEvents` | 缺漏、重複、未收盤、無效值、時間單位變更、來源衝突的結構化事件 |

規則：拒絕的原件也保留（可重播）；第二來源（例如 Coinbase）只產生獨立比對證據，
**不**回填第一來源的缺漏；衝突時保存兩份原件，待裁定後產生新 snapshot。

---

## 3. Snapshot（`market-snapshot-v1`）

研究與 paper 只讀 snapshot，不直接讀原件：

| 欄位 | 說明 |
| --- | --- |
| `snapshotId` | 內容 hash（沿用 `dataset-content-v2` 對 candles 的 hash）＋以下語意欄位的版本 |
| `priceBasis` | `raw` / `adjusted-split` / `adjusted-total-return`；調整價只按明確用途使用，訊號所用調整**不得引入未來尚未發生的公司事件** |
| `calendarVersion` | 交易日曆版本；週末／停牌不是缺漏 |
| `corporateActionVersion` | 分割／除息／付款日資料版本；未確認完整性 → 結果標 `DEGRADED`，阻擋合格晉級 |
| `costProfileVersion` | 佣金、滑價、交易稅設定；使用者確認後才允許資格評估 |
| `sourceProvenanceIds` | 組成此 snapshot 的原件 |
| `kind` | `demo` / `historical` / `forward-observed`；三者不混用 |

---

## 4. 來源矩陣（規劃決策；權限於接線 phase 逐一核對）

| 市場 | 主要來源 | 補充／限制 | P00 本機可達性 |
| --- | --- | --- | --- |
| Crypto | Binance 公開月／日封存（`data.binance.vision`）＋同交易所 REST | 校驗 CHECKSUM、時間單位（封存曾由毫秒改微秒）與修訂；Coinbase 為獨立比對來源 | 封存索引與 `api/v3/ping` 皆 HTTP 200 |
| US ETF | Tiingo EOD 免費帳戶 | 保存 raw、adjusted、dividend、split 欄位；免費方案權限與速率須在設定時驗證，不假設即時行情 | host 可達（301 導向）；**尚無帳戶／token，未驗證權限** |
| TW ETF | FinMind `TaiwanStockPrice` 原始日線 | TWSE 月行情核對；不依賴付費還原價接口；民國日期、成交量單位（股／張）於 P10 處理 | API host 可達（空查詢回 422）；**未驗證配息／分割事件涵蓋** |
| Calendar／事件 | TWSE、NYSE、ETF 發行商 | 休市、臨時停市、分割、除息、付款日 | TWSE 首頁可達（302）；**尚無版本化 calendar 資料** |

上述可達性只是 HTTP 回應碼，**不代表已證明任何商品的長期資料完整性**。

---

## 5. ETF 必要語意（P08 實作，TS／Rust parity）

1. 交易日與時區來自版本化 calendar。
2. 保留原始成交價；調整價只按明確用途。
3. 配息：除息日登記應收款，付款日轉可用現金；缺付款日不得假設可立即再投資。
4. 分割：同步調整股數、成本與未成交訂單；不得產生虛假收益。
5. CAGR 按實際經過時間；波動年化用市場／頻率設定（交易日數）；舊 365 行為留在舊契約。
6. USD、TWD、USDT 帳戶分開；不自動換匯、不合併資產收益。
7. 報表揭露佣金、滑價、交易稅，以及未納入的個人所得稅。
8. 免費日線模式的 paper 明確標為「日線延遲確認模擬」：收到已完成行情後決策，最早
   對之後的交易時點下單；訂單必須在目標開盤前存在；經濟成交時間與觀測／確認時間
   分開保存；服務離線期間不補造訂單。

---

## 6. 補資料流程（P06／P07 實作）

1. 依 instrument、calendar、上市期間產生**預期資料範圍**。
2. 比對缺漏、重複、未收盤、無效值、來源修訂。
3. 同來源有界重試、分段下載、快取。
4. 第二來源只產生比對證據。
5. 衝突保存兩份原件，裁定後產生新 snapshot。
6. 缺漏未解決時顯示阻擋範圍與可執行的補查動作。

禁止：forward-fill 成交量、刪除不利期間、將不同 venue／quote 拼成同一條資料。

---

## 7. 必測 fixtures（P06 起逐一加入，雙語 parity）

時間單位變更（ms→µs）、缺漏、重複、未收盤、來源混淆（Binance vs Coinbase 同時段）、
修訂衝突、非交易日、停牌、除息／付款日、分割、民國日期、成交量單位。每個 fixture
保存原始來源、時間、hash 與預期拒絕結果。
