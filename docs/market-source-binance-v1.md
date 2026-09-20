# Binance source adapter v1（`binance-archive` / `binance-rest`）

> 由 P07（Crypto adapter，2026-09-20）登記。上游規劃：
> [`plans/active-plan.md`](plans/active-plan.md) §4.1 市場與資料來源、§5 P07 驗收；
> 契約基礎 [`market-contract.md`](market-contract.md) §4／§6、
> [`market-foundation-v1.md`](market-foundation-v1.md)。實作：
> `src-tauri/src/market/sources/binance.rs`（純解析）、`market/http.rs`（唯一連網處）、
> `market/ingest.rs`（編排）、`market/fetch.rs`＋`alpha-factor-forge-service fetch`（操作入口）。
> 測試資料：`alpha-factor-forge/fixtures/binance/`（真實檔案與其公布的 CHECKSUM）。

**狀態：已實作（P07）。** 本文件描述這個 adapter 實際做什麼、拒絕什麼，以及 2026-09-20 對真實來源
執行後得到的結果。

**P07 不做的事**：不處理 ETF（P09／P10）、不做 Coinbase 比對來源、不排程（P17）、不開 UI（P21）、
不從 `exchangeInfo` 取交易規格（instrument 的 lotSize／priceStep 仍為未知）。

---

## 1. 兩個同交易所來源

| source id | 端點 | 用途 |
| --- | --- | --- |
| `binance-archive` | `data.binance.vision/data/spot/{monthly,daily}/klines/…` | 已公布的月／日封存，**每個檔案都有公布的 SHA-256 CHECKSUM** |
| `binance-rest` | `api.binance.com/api/v3/klines` | 封存尚未公布的最新 K 線（同一交易所自己的資料） |

**出處（origin）規則**：source id 是 `<origin>[-<endpoint>]`。`binance-archive` 與 `binance-rest` 是
**同一個發布者的兩個端點**，可以組成同一條序列；`coinbase-rest` 是另一個發布者，永遠不行。這條規則
在 P07 的真實執行中才被發現是必要的——當月資料需要「日封存＋REST 尾端」，而原本的
`source_mismatch` 會把它判成來源混淆（見 §5）。

### 1.1 連網政策（`market/http.rs`）

只允許 HTTPS、只允許白名單主機、**拒絕 redirect**（換主機等於換來源）、回應大小上限、有界重試
（僅 transport／timeout／429／5xx 重試；4xx 是答案不是失敗）。其餘所有模組都只看到 `HttpFetcher`
trait，所以 ingest 全程可離線測試，只有傳輸層需要真實網路。

---

## 2. 取得一個單位

`ingest` 依要求區間產生單位清單，順序固定：

1. **月封存**：區間涵蓋的每個月各一個 unit。
2. 月封存 **404 → 日封存**：該月落在區間內的每一天。
3. 日封存 404 且允許 REST → **REST 分頁**（每頁最多 1000 根），只要求**已收盤**的 bar。

「404」是資訊而不是失敗：那代表來源還沒公布。

每個封存 unit 的處理：

```
取 .CHECKSUM → 保存原文與解析結果（必須指名同一個檔名）→ 取 .zip → 比對 SHA-256
  → 不符：先以 accepted=false 保存本次 ZIP，再重取一次（上限 2 次）；成功重試也不刪掉前次失敗證據
  → 相符：讀取 ZIP 的單一 entry → 解析 CSV → 記錄 accepted provenance（含原件 bytes）
```

每份 CHECKSUM 回應各有 `text/plain` provenance，保留 URL、檔名、`kind: checksum` 與 `vouchesFor`。
ZIP 的 `requestScope.checksumProvenanceId` 指向實際驗證它的那份回應，`publishedSha256` 保留解析結果；
SHA-256 不符或 ZIP／CSV 解析失敗的原件也有同樣關聯。CHECKSUM 本身格式錯誤時只保存該份拒絕紀錄，
不把文字存成 ZIP，也不再要求 ZIP。

**快取**：同一 URL 最新 accepted 的 ZIP 須具有下述觀測證據、未被修訂，且所連結的 CHECKSUM
與商品、週期、URL、檔名相符且未被修訂；重新讀取兩份原件並重播檔名與 SHA-256 校驗後才能重用。
缺少 CHECKSUM 原件或僅有舊版本 metadata 時重新抓取，不沿用舊的 availability。遺失的原件可以補回；
若既有內容定址檔案已損壞，artifact store 會明確拒絕覆寫，需保留損壞證據後另行處理。

### 2.1 觀測時間與舊資料隔離

CHECKSUM 與 ZIP 分別在各自**回應完整收到後**記錄 `retrievedAt`，`availableAt` 使用同一實際觀測時間，
並標記 `requestScope.availabilityBasis: observed-at-retrieval`。`periodEndsAt` 僅描述封存期間終點，
不能代表發布時間，也不能讓今天下載的歷史資料取得過去 cut 的 forward-observed 資格。

修正前的不可變 provenance 不覆寫：缺少這個觀測標記或 `availableAt` 與 `retrievedAt` 不同的
`binance-archive` 資料，在 forward-observed admission 視為 `availability_unknown`；historical 仍可讀。
已由舊版建立的 forward-observed 快照也會在 get／list／dataset-status 讀取時重新檢查；若無法證明在
其原始 cut 前取得，回傳明確錯誤，不繼續回傳合格狀態。舊列保留作稽核，新的取得與快照另行新增。

### 2.2 ZIP 讀取

Binance 封存是「一個 entry 的 ZIP」。本 adapter 自己讀 local file header（stored 或 deflate），並且：

- 以 end-of-central-directory 確認 **entry 數量恰為 1**（第二個檔案不能夾帶進來）；
- 檢查解壓後長度與 **CRC-32**；
- 拒絕 data descriptor 形式（大小不在 header）、加密 entry、其他壓縮方法、過大的 entry。

用 `flate2`（鎖檔既有）而不是引入 ZIP 套件：輸入形狀單一，且解析前已先用公布的 SHA-256 驗過整個檔案。

---

## 3. 時間單位（實測已證實的風險）

`market-foundation-v1` 刻意**沒有**換算函式：無聲換算正是它要防止的事。換算因此放在這個 adapter，
並且只在以下條件下進行：

1. 單位以**整個檔案**為單位判定（`detect_time_unit` 同時看 open 與 close 兩欄）；
2. 一個檔案內若出現兩種單位 → `time_unit_unresolved`，**拒絕該檔**；
3. 換算必須精確：open time 不是整毫秒 → `time_unit_not_exact`，**拒絕**，絕不四捨五入；
   close time 是「bar 的最後一微秒」，截斷後仍落在同一根 bar 內；
4. 判定出的單位與「是否換算過」寫進該檔的 provenance `requestScope`。

**2026-09-20 實測**：`BTCUSDT-1h-2024-12` 的第一個 open time 是 `1735603200000`（毫秒），
`BTCUSDT-1h-2025-01` 是 `1735689600000000`（微秒）。也就是說封存自 **2025-01 起改用微秒**，
與契約 §4 的警告一致。同一次 `fetch 2024-12-30 → 2025-01-03` 會一次遇到兩種單位，兩個檔案各自
判定、各自換算，合併後 96 根 bar 全部落在同一條每小時網格上。

---

## 4. 組裝與匯入

- 所有 accepted 單位的 bar 依 open time 合併；**同一根 bar 出現兩次且內容不同 → `source_conflict`，
  整批不匯入**（兩份原件都留著）；完全相同則視為同一根。
- 裁切到要求區間，並**丟棄在資料切點尚未收盤的 bar**（未完成的 bar 還不是資料；報告裡會說丟了幾根）。
- 以既有 `import_dataset_with_candles` 匯入：`dataset-content-v2` 識別與 `market-data-quality-v1`
  逐根閘門**原封不動**適用。`datasets.source` 固定寫 `exchange`（精確來源在 provenance），
  所以同一批 bar 重跑會落在同一列。
- 最後交給 P06 `build_snapshot`（`priceBasis: raw`、`kind: historical`、現貨無公司事件）。

---

## 5. 兩種涵蓋率，兩個問題

這是 P07 實際執行時才顯現、而且很容易誤讀的區別：

| 問題 | 誰回答 | 意義 |
| --- | --- | --- |
| 這個**資料集**自身完整嗎？ | `market-snapshot-v1` 的 coverage | 在資料集**自己的起訖之間**，日曆預期的 bar 都在。少抓了尾端不會讓它不完整 |
| 我**要求的區間**拿到了嗎？ | `IngestReport.rangeCoverage` | 以要求的起訖與商品日曆推導，缺什麼就是缺什麼 |

因此 `IngestOutcome` 有 `rangeIncomplete`：**資料集可以是合格 snapshot，而要求的區間仍被拒絕**。
區間層的 blocking 事件會以 `scope: "requested-range"` 寫進 `market_quality_events`，
和 snapshot 層的證據一樣可查。

---

## 6. 操作入口

```
alpha-factor-forge-service fetch --instrument crypto:binance:BTCUSDT --interval 1h \
    --from 2025-01-01 --to 2026-01-01 [--no-rest] [--cost-profile <version>] [--json] [--data-dir <dir>]
```

- `--from` 含、`--to` 不含，皆為 UTC 日；日期規則與契約一致（`2024-7-15` 會被拒絕）。
- 執行期間**持有工作區**（OS 鎖→migration→ownership epoch→recovery→heartbeat），所以 service 正在
  執行時會以 exit 2 拒絕。由服務代為取得資料屬 P17。
- 若該 instrument 尚未註冊，且它是本 adapter 的預設現貨商品，會**在不存在時**自動註冊；
  已存在的修訂**永不覆蓋**。
- Exit code：`0` 區間可用（snapshot）、`5` 取得了但區間不可用（缺漏／被拒來源／沒有資料）、
  `2` 別的宿主持有工作區、`1` 失敗。

---

## 7. 2026-09-20 實際執行結果（隔離工作區，非使用者正式工作區）

| 執行 | 結果 |
| --- | --- |
| `BTCUSDT 1h 2024-12-30 → 2025-01-03` | `monthly:2024-12` 744 根（毫秒）、`monthly:2025-01` 744 根（微秒→毫秒）；**96／96** 根；snapshot 合格 |
| `BTCUSDT 1h 2025-01-01 → 2026-01-01` | 12 個月封存全部取得；**8760／8760** 根；snapshot 合格 |
| `ETHUSDT 1h 2025-01-01 → 2026-01-01` | 同上；**8760／8760** 根；snapshot 合格 |
| `BTCUSDT 1h 2026-09-01 → 2026-09-21` | `monthly:2026-09` 未公布 → 19 個日封存＋1 頁 REST；**462／463**，1 根尚未到期；snapshot 合格 |
| `BTCUSDT 1h 2017-07-01 → 2017-09-01`（`--no-rest`） | 7 月完全未公布、8 月僅 356 根；報告 **缺 2017-07-01 00:00 → 2017-08-17 03:00 共 1132 根**，動作 `refetch_range`；**exit 5，區間被拒絕** |

**這些數字只代表上述區間**。本文件不宣稱 BTC／ETH 有多年完整資料；任何更長的區間都必須自己跑一次
並看它的 `rangeCoverage`。2017 的那一筆正是「已知缺漏仍拒絕」的實例：本 adapter 不知道商品的上市
日期（`listedFrom` 目前為未知），所以上市前的空白被如實報成缺漏，而不是被當成「本來就沒有」。

---

## 8. 已知限制與後續

- **`listedFrom` 未知**：上市前區間會被報成缺漏。由 `exchangeInfo`（或人工登記）填入是後續工作。
- **交易規格未知**：`lotSize`／`priceStep`／`minNotional` 仍為 `None`，因此這些商品**不可 paper**。
- **REST 無 checksum**：REST 的證據是「請求與回應本身」，不是第三方摘要；因此 REST 取得的 bar
  其 `availableAt` 記為回應當下。
- **無第二交易所比對**：Coinbase 作為獨立比對來源仍未實作（契約 §4 保留）。
- **未處理封存的事後修訂**：`market_provenance` 的 `revision_of` 機制已就緒（P06），但這個 adapter
  尚未主動偵測「同一個月的封存檔內容變了」；目前重跑會直接重用快取。
