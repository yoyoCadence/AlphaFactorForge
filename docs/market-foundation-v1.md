# Market foundation v1（`market-foundation-v1` / migration 0008）

> 由 P06（資料基礎契約，2026-09-20）登記。上游規劃：[`plans/active-plan.md`](plans/active-plan.md)
> §3.3「Market」「Provenance」「Snapshot」資料群組、§4.1 市場與資料來源、§5 P06 驗收；
> 契約來源 [`market-contract.md`](market-contract.md)（`market-instrument-v1`／`market-provenance-v1`／
> `market-snapshot-v1`）；需求來源
> [`../handoffs/2026-09-15-alphabtc-capability-transfer-v1.md`](../handoffs/2026-09-15-alphabtc-capability-transfer-v1.md)
> ABC-04。實作：`src/core/market-data/foundation.ts`、
> `src-tauri/src/discovery_core/market_foundation.rs`、`src-tauri/src/market/`、
> `migrations/0008_market_foundation.sql`、`fixtures/rs-core/market-foundation-v1.json`。

**狀態：已實作（P06）。** 本文件定義後續 phase（P07 Crypto adapter、P08 ETF 語意、P09／P10 ETF
adapters、P12 試驗帳本、P19／P20 paper、P21 UX）必須遵守的資料形狀與不變量；偏離先修訂本文並提升版本。

**P06 不做的事**（避免誤讀）：不連網、不下載任何資料、不換算時間單位、不修補序列、不改
`dataset-content-v2`、不改 `barsPerYear`、不改 0001–0007、不加新相依套件、不動任何既有 UI 行為。

---

## 0. 與既有資料的關係（附加層）

| 既有 | 關係 |
| --- | --- |
| `datasets`／`candles`（0001） | **不改**。欄位與語意不變；匯入、回測、discovery 全部照舊 |
| `market-data-quality-v1`（`src/core/market-data/quality.ts`／`discovery_core/market_data.rs`） | **不改**。它是每根 K 線的合理性閘門；本契約在其**之上**，處理序列與 instrument／calendar／來源的關係。時間單位的區間邊界**直接沿用**它的 `MIN`／`MAX_EXCLUSIVE`，兩者不可能漂移 |
| `dataset-content-v2` hash | **不改**。同一組 candles 的 hash 不變；snapshot 以自己的內容雜湊識別 |
| `barsPerYear`（`backtestRunner.ts`／`benchmarks.rs`，含 `1d` = 365 與未知 interval 回退） | **不改**。本契約新增的是**節奏**（`market-interval-v1`，嚴格、無回退），與年化是不同問題；ETF 交易日年化仍屬 P08 |
| 沒有 snapshot 的 dataset | 一律 **legacy**：照常匯入、照常回測，但**不能自動取得新資格**（`market-contract.md` §0） |

---

## 1. 純契約（雙語鏡像）

`src/core/market-data/foundation.ts` 與 `discovery_core/market_foundation.rs` 為**逐條鏡像**，由
`fixtures/rs-core/market-foundation-v1.json` 綁定（fixture 依規格撰寫，不由任一實作錄製）。

### 1.1 Interval（`market-interval-v1`）

`1m`／`3m`／`5m`／`15m`／`1h`／`4h`／`1d` → 毫秒。**嚴格**：未知 interval 回 `null`／`None`，
不回退成日線。這解掉 `market-data-quality-contract.md` §7 所列 `INTERVAL-CONTRACT-001` 的**節奏**半邊；
年化半邊未動。

### 1.2 Instrument id

`<market>:<venue>:<symbol>`，規則依序評估，回報第一個失敗者：

| 規則 | 拒絕條件 |
| --- | --- |
| `instrument_id_shape` | 不是三段非空欄位 |
| `unknown_market` | market 不是 `crypto`／`us-etf`／`tw-etf` |
| `venue_not_normalized` | venue 不是 `[a-z0-9-]+` |
| `symbol_not_supported` | symbol 不是 `[A-Za-z0-9.-]+` |

symbol 保留來源拼法：**`USDT` 永不改寫成 `USD`**，大小寫不折疊。

### 1.3 時間單位（偵測，不換算）

以 `market-data-quality-v1` 的合理區間乘上 `1/1000`／`1`／`1000`／`1e6` 得到
`seconds`／`milliseconds`／`microseconds`／`nanoseconds` 四個區間。判定：

- 空輸入 → `no_timestamps`
- 任何值落在所有區間之外（含非有限值）→ `time_unit_unknown`＋索引
- 同一批出現兩種單位 → `time_unit_mixed`＋**第一個不同者的索引**（Binance 封存 ms→µs 的情境）

**本契約沒有換算函式**：單位變更是待裁定的證據，猜著換算正是它要防止的無聲修補。

### 1.4 Session calendar（`session-calendar-v1`）

| 欄位 | 說明 |
| --- | --- |
| `calendarId` | **版本寫在 id 裡**（`crypto-24x7-v1`、`nyse-v1`、`twse-v1`）；更正＝新 id |
| `kind` | `continuous`（24／7）或 `trading-days` |
| `timezone` | IANA 名稱。v1 **只記錄**，預期範圍運算全部走 UTC（見下） |
| `tradingWeekdays` | ISO 1–7，遞增唯一（`trading-days` 專用） |
| `holidays` | `YYYY-MM-DD`，遞增唯一 |
| `earlyCloses` | `YYYY-MM-DD`；v1 只記錄，不影響預期範圍 |

驗證規則 id：`calendar_id_shape`、`unknown_kind`、`empty_timezone`、
`continuous_carries_trading_days`、`trading_days_without_weekdays`、`weekday_out_of_range`、
`weekdays_not_sorted`、`invalid_date`、`dates_not_sorted`。

**`unknown_kind` 在 Rust 端不可達**（kind 是 enum），因此**沒有共用 fixture 列**，由各語言自行單元測試
——與 `market-data-quality-v1` 的 `timestamp_not_representable` 相同處理方式。

**為何不引入時區資料庫**：v1 的日線 bar 時間戳定義為「交易日的 UTC 午夜」，交易日由 calendar 的
weekday＋holiday 資料直接列舉，因此不需要 tz 資料庫，兩個語言的日期運算逐位元相同。ETF 盤中 session
與當地時間邊界屬 P08。

### 1.5 預期範圍

`expectedBarStarts(calendar, interval, [from, to), listedFrom?, delistedAt?, suspensions[])`：

- `continuous`：以 epoch 對齊的 `intervalMs` 網格。
- `trading-days`：僅支援 `1d`；每個交易日的 UTC 午夜。
- **週末、休市日、停牌、上市前／下市後都不是缺漏**——這是「預期由版本化 calendar 推導，而不是由資料
  推導」的全部意義。
- 拒絕而非臆測：`unknown_interval`、`invalid_calendar`、`unsupported_interval_for_calendar`、
  `invalid_range`、`invalid_suspension`、`range_too_large`（上限 `MAX_EXPECTED_BARS = 1_000_000`）。
- 上市期間與查詢範圍**無交集**時回空集合且**無 issue**：那不是缺陷。

### 1.6 Coverage audit

`auditCoverage(interval, expected[], observed[], asOfMs)` 產生事件與計數，**不修補、不丟棄、不
forward-fill**：

| code | 意義 | action |
| --- | --- | --- |
| `out_of_order` | observed 非遞增（重複值不算） | `resort_source` |
| `duplicate_timestamp` | 同一時間戳多次 | `deduplicate_source` |
| `unaligned_timestamp` | 不在節奏網格上（差 1 ms 也算） | `verify_time_unit` |
| `unexpected_bar` | 對齊但不在預期集合（休市日、上市前…） | `review_calendar` |
| `missing_bar` | 已到期的預期 bar 不存在；**相鄰者合併成一列**（跨週末的缺口仍是一列） | `refetch_range` |
| `unclosed_bar` | observed 的 bar 在 `asOf` 時尚未收盤 | `wait_for_close` |

- 全部 `blocking`：序列整條可用或整條不可用，與 `market-data-quality-v1` 整份接受／整份拒絕一致。
- **尚未到期的 bar 永遠不算缺漏**（計入 `notDueCount`）。
- 事件排序：`(rangeStart, 宣告順序的 code rank)`。**用 rank 不用字串比較**——字串定序在 JavaScript 與
  Rust 之間沒有保證。
- 未知 interval → `issue = unknown_interval`、`blocking = true`、無事件。

### 1.7 來源可否合併

`seriesConflicts(a, b)` 回傳排序後的代碼；空＝可合併：`market_mismatch`、`venue_mismatch`、
`symbol_mismatch`、`quote_mismatch`、`interval_mismatch`、`source_mismatch`、`price_basis_mismatch`、
`comparison_role_not_combinable`、`invalid_instrument_id`。

**第二來源（`comparison`）永遠只是比對證據**，不能成為 snapshot 的組成，也不回填第一來源的缺漏。

#### 來源出處（`sourceOrigin` / `sourcesShareOrigin`，P07 追加）

source id 的形狀是 `<origin>[-<endpoint>]`：`binance-archive` 與 `binance-rest` 是**同一個發布者的
兩個端點**，`coinbase-rest` 是另一個發布者。

`seriesConflicts` **仍然**會回報 `source_mismatch`——那是關於取得方式的事實；**要不要因此拒絕組成，
由組成者決定**：snapshot 在兩個組成 `sourcesShareOrigin` 為真時容許它（§5 步驟 3），跨發布者則永遠
拒絕。這條規則由 P07 的真實執行催生：當月資料必須由「日封存＋同交易所 REST 尾端」組成，而原本的
規則會把計畫明文要求的「同交易所補查」判成來源混淆。詳見
[`market-source-binance-v1.md`](market-source-binance-v1.md) §1、§5。

---

## 2. 資料表（migration `0008_market_foundation`）

六張 append-only 表，全部有 `BEFORE UPDATE` / `BEFORE DELETE` 觸發器。

| 表 | 一列是什麼 | 識別 |
| --- | --- | --- |
| `market_calendars` | 一個 calendar **版本** | `calendar_id` UNIQUE＋`content_hash` UNIQUE |
| `market_instruments` | 一個 instrument **修訂** | `content_hash` UNIQUE；`UNIQUE(instrument_id, revision)` |
| `market_provenance` | 一次**取得**（含被拒絕者） | `record_hash` UNIQUE；`revision_of` 指向被修訂者 |
| `market_snapshots` | 研究／paper 可讀的一份資料 | `snapshot_id` UNIQUE（內容雜湊） |
| `market_snapshot_sources` | snapshot ↔ provenance | 複合主鍵 |
| `market_quality_events` | 一則結構化證據 | 自增 id |

規則：

- **calendar 版本不可編輯**：同 id 不同內容一律拒絕，更正＝新 id。
- **instrument 是修訂鏈**：同內容＝同一列；有變更＝`revision + 1`，舊修訂原封不動（snapshot 指向的是
  **那一列**，不是「目前的」instrument）。
- **註冊 instrument 需要其 calendar 已存在**，且 kind 必須相符（`spot-crypto` → `continuous`，
  `etf` → `trading-days`）。這是 ETF 的誠實阻擋點：`nyse-v1`／`twse-v1` 需要真實休市資料，P06 沒有，
  也**不捏造**。
- **秘密不進本契約任何一張表**；raw bytes 存於 workspace 的 artifact store（content-addressed），
  DB 只存 sha256、長度、相對路徑與 media type。

### 2.1 Provenance 的不變量

- 被拒絕的原件**保留**（可重播）；`accepted` 與 `rejection_reason` 互為充要（DB CHECK＋程式檢查）。
- `availableAt` ≤ `retrievedAt`；`availableAt` 為 `NULL` 代表未知，**未知就不能支撐 forward-observed
  snapshot**（重新下載歷史不會變成 point-in-time）。
- 修訂只能指向**同 instrument／interval／source** 的既有列，且**一列只能被修訂一次**（不分叉）；
  鏈頭＝沒有其他列指向它的那一列。
- 相同 observation 與原件的重送回傳原 id（包括已被後續修訂取代的紀錄）；先辨識冪等重送，
  再拒絕不同內容對同一 target 的第二筆修訂。重送仍驗證原件儲存。
- media type 白名單：`application/json`、`text/csv`、`application/zip`、`text/plain`；未列者拒絕，
  不以錯誤副檔名存放（P07／P09／P10 依實際來源擴充）。

---

## 3. 識別與雜湊

| 對象 | preimage |
| --- | --- |
| calendar | canonical JSON（含 `version`） |
| instrument | canonical JSON（含 `version`；含 specs、上市期間、停牌、sourceCapabilities） |
| provenance | canonical JSON of {version, instrumentId, interval, source, role, requestScope, rawSha256, rawByteLen, mediaType, retrievedAt, availableAt, accepted, rejectionReason, revisionOf} |
| snapshot | canonical JSON of {version, instrumentId, **instrumentContentHash**, interval, datasetHash, priceBasis, calendarId, corporateActionVersion, costProfileVersion, kind, sourceRecordHashes（排序）} |

canonical JSON 與 sha256 直接沿用 `research::canonical_json`／`sha256_hex`（PR #107 review 的
「不得出現第二套會漂移的編碼」）。

**`asOf` 刻意不在 snapshot 識別內**：dataset、instrument 修訂、calendar、來源全部不可變，因此同一組輸入
描述的就是同一份 snapshot；`as_of` 與它產生的 coverage 一併存在列上。

---

## 4. 事件詞彙

`market_quality_events.code` 只接受 §1.6 的 coverage codes 加上 storage 端的十個：

| code | severity | action | 何時 |
| --- | --- | --- | --- |
| `dataset_instrument_mismatch` | blocking | `separate_sources` | dataset 的 interval／symbol／venue 不是該 instrument 的序列 |
| `expected_range_unavailable` | blocking | `review_calendar` | 預期範圍本身被拒（帶 issue id） |
| `source_conflict` | blocking | `separate_sources` | 組成之間或與請求衝突、或組成是被拒絕的取得 |
| `superseded_source` | blocking | `rebuild_from_revision` | 組成已被修訂取代 |
| `time_unit_mismatch` | blocking | `verify_time_unit` | 儲存的時間戳不是毫秒（縱深防禦，見下） |
| `availability_unknown` | blocking | `record_availability` | forward-observed 但組成沒有可解析的 `availableAt` |
| `availability_after_cut` | blocking | `record_availability` | forward-observed 的 `availableAt > asOf`；相等可接受，保留來源小數秒精度比較 |
| `missing_source` | blocking | `refetch_range` | provenance 清單為空，無法追溯任何原件；所有 kind 均阻擋 |
| `corporate_actions_unverified` | degraded | `verify_corporate_actions` | ETF 且未確認配息／分割完整性 |
| `cost_profile_unconfirmed` | degraded | `confirm_costs` | 使用者尚未確認成本設定 |

未列的 code 或 action 一律**拒絕寫入**：沒有人能據以行動的事件不是證據。

`time_unit_mismatch` 對「走正常匯入路徑的 dataset」**不可達**——`market-data-quality-v1` 在匯入時就會擋下
毫秒區間以外的值。它保留為縱深防禦（成本是一趟掃描，擋下的是一條無聲錯誤的序列），與該契約 §5 對
不可達規則的處理方式一致。同理，`out_of_order` 對 DB 讀出的 candles 不可達（`ORDER BY timestamp`）。

P07 驗收修正：Binance 封存的期間終點不代表發布證據。`binance-archive` 只有帶有
`availabilityBasis: observed-at-retrieval` 且 `availableAt` 與 `retrievedAt` 為同一實際時間的紀錄，
才具有已知 availability；舊版本的期間推測值視為 `availability_unknown`。此規則也在既有
forward-observed 快照的 get／list／dataset-status 讀取套用，拒絕回傳未經證明的舊資格。
原件與快照列維持不可變，historical 不受此隔離限制；重新取證方式見 `market-source-binance-v1.md` §2.1。

---

## 5. Snapshot 生成與 legacy

`build_snapshot` 先驗證，所有寫入在一個 transaction 內完成：

1. 取得 instrument 最新修訂與其 calendar（不存在＝`Err`，不是事件——連序列都指不出來）。
   讀取 dataset 的**全部 OHLCV**（包含宣告期間以外的列），依序重驗
   `verify_dataset_identity` 與 `market-data-quality-v1`。雜湊、數量、起迄或 candle 合理性不符
   → `Err` 且不寫入；不修補、不重新雜湊。回傳 `Existing` 前也必須通過。
2. 身分：dataset 的 interval／symbol／venue 必須是該 instrument 的序列。venue 比對**忽略大小寫**
   （`datasets.exchange` 是使用者自由輸入，instrument 的 venue 已正規化）；symbol **完全比對**。
3. 組成：至少一筆 provenance，每筆必須 accepted、`primary`、同序列、未被修訂；
   forward-observed 另需可解析且不晚於 `asOf` 的 `availableAt`，以實際時間比較（接受時區偏移）。
   兩兩之間也必須可合併（這正是擋下 Binance 與 Coinbase 拼接的地方）。**例外只有一個**：
   兩個組成 `sourcesShareOrigin` 為真時，其 `source_mismatch` 被容許——同一交易所的封存與 REST
   是同一個來源的兩個端點（§1.7）。跨發布者的 `source_mismatch` 永遠 blocking。
4. 時間單位縱深防禦 → §4。
5. Coverage：以 instrument 的上市期間與停牌、calendar 版本推導預期範圍，與 dataset 實際 candles 比對。
6. 語意預設：ETF 未確認公司事件 → `degraded`；成本未確認 → `degraded`。
7. 有任何 blocking → **不建立 snapshot**，只寫入整組證據，回 `Blocked { events }`（含阻擋範圍與 action）。
   否則寫入 snapshot＋sources＋degraded 證據，`status` 為 `ok` 或 `degraded`。

資格：`qualification_eligible = status == ok && kind != demo`。**degraded 與 demo 永不取得資格**，
`demo`／`historical`／`forward-observed` 三者不混用。

`dataset_market_status(datasetId)` 回 `Legacy` 或 `Registered(snapshot)`；**legacy 不是錯誤狀態**，
只是「沒有 market 語意」，一切既有功能照常。

---

## 6. P06 驗收對照

計畫 §5 P06 停止點：「時間單位、缺漏、重複、來源混淆及修訂 fixtures 通過」。

| 驗收項 | 雙語 fixture（`market-foundation-v1.json`） | Rust 儲存端測試 |
| --- | --- | --- |
| 時間單位 | `archive-switches-to-microseconds-mid-batch`、四個單位各一列、`epoch-zero-is-in-no-band`、`non-finite-is-in-no-band` | `time_unit_events`（縱深防禦，§4 註明不可達） |
| 缺漏 | `one-missing-bar`、`contiguous-gap-is-one-actionable-range`、`a-weekend-inside-a-gap-does-not-split-it`、`a-bar-that-is-not-due-yet-is-never-missing` | `a_gap_blocks_the_snapshot_and_says_which_bars_and_what_to_do` |
| 重複 | `duplicate-timestamp`、`out-of-order-and-duplicate-at-the-same-bar` | 同上路徑（coverage → 事件） |
| 來源混淆 | `binance-and-coinbase-are-never-one-series`、`a-different-market-conflicts-on-every-axis`、`a-comparison-source-is-evidence-not-a-component` | `a_second_venue_or_a_rejected_or_superseded_source_can_never_be_a_component`、`a_dataset_that_is_not_the_instruments_series_is_refused` |
| 修訂 | （儲存端語意，無純函式對應） | `a_revision_adds_to_the_chain_and_never_overwrites_or_forks_it`、`superseded_source` 阻擋與改用修訂後可建立 |
| 非交易日／停牌 | `weekdays-skip-weekend-holiday-and-halt`、`a-bar-on-a-non-trading-day-is-unexpected` | `an_etf_without_verified_corporate_actions_is_degraded_and_a_holiday_is_not_a_gap` |

---

## 7. 尚未做的事（交接點）

> **P07 追加的一個重要澄清**：snapshot 的 coverage 回答的是「**這個 dataset 在它自己的起訖之間**
> 是否完整」，不是「我要求的區間是否拿到」。少抓了尾端的資料集仍可能是合格 snapshot。要求區間層的
> 稽核由取得端負責（`IngestReport.rangeCoverage`，事件帶 `scope: "requested-range"`），
> 見 [`market-source-binance-v1.md`](market-source-binance-v1.md) §5。

| 項目 | 屬於 |
| --- | --- |
| ~~任何網路下載、CHECKSUM 校驗、分段重試、快取~~ → **Crypto 已完成（P07）** | P09（Tiingo）／P10（FinMind＋TWSE） |
| `nyse-v1`／`twse-v1` 等真實休市資料 | P09／P10（P06 只建立註冊與阻擋機制） |
| ETF 盤中 session、當地時間邊界、配息應收／付款、分割調整、交易日年化、原幣帳務 | P08 |
| instrument 的 lotSize／priceStep／minNotional（目前一律未知＝不可 paper） | 由來源回報，P07／P09／P10 以新修訂寫入 |
| Tauri 命令與 UI（資料來源畫面、阻擋範圍呈現） | P16（MCP）／P21（整合 UX）；P06 不開命令面，避免留下無人呼叫、未受測的邊界 |
| campaign 凍結 instrument 清單 | P12 |
| snapshot 與回測／discovery 的強制綁定（「研究只讀 snapshot」） | P12 起逐步收緊；P06 只記錄，不改變既有回測可跑的資料 |

`registry::DEFAULT_RESEARCH_SYMBOLS` 保存 `market-contract.md` §1.1 的預設研究清單（12 檔，可編輯，
**不是投資建議**）；`default_crypto_instruments()` 只提供兩檔 Binance 現貨——ETF 的 venue、上市日期與交易
規格必須由其來源提供，在此捏造就是偽造市場事實。
