# Handoff: P07 Crypto adapter（Binance 封存／REST、同來源補查、品質報告）

Date: 2026-09-20
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p07-binance-adapter`（自 main `503fc99` ＝ PR #108 merge 之後建立）
PR: 待建立
Status: 實作與真實執行驗證完成；P08 另行授權

## Summary

Plan §5 P07（ABC-04 的來源半邊）。**這是本專案第一次真的取得市場資料。**

`fetch` 依要求區間決定單位（月封存 →（未公布時）日封存 →（仍未公布且允許時）同交易所 REST 的已收盤
bar），逐檔以**官方公布的 SHA-256** 校驗、讀取封存內唯一 entry、保存原件、記錄 provenance（**被拒絕的
連同失敗 bytes 一起留**），合併後匯入既有 dataset 路徑，再交給 P06 決定是否成為 snapshot，最後回報
**要求區間**的涵蓋率。

驗收對照（計畫 §5 P07 停止點「BTC／ETH 實際涵蓋報告；已知缺漏仍拒絕；未宣稱多年完整」）見 §Verification。

## 執行前驗證（plan vs codebase）

- **依賴**：P07 依賴 P06，已 merge（`503fc99`）。P06 的四項 acceptance finding 修正也在 main，本次
  直接建構其上（`build_snapshot` 現在要求至少一筆 provenance、驗證完整 OHLCV、forward-observed 比對
  資料切點；`record_raw` 的重送冪等在 fork 檢查之前）。
- **契約來源**：`market-contract.md` §4 明列 Binance 封存「曾由毫秒改微秒」與 CHECKSUM 校驗要求；
  §6 明列補資料流程六步。逐條對照實作（見 `docs/market-source-binance-v1.md`）。
- **相依套件**：計畫 §5 授權「網路等必要依賴在所屬階段集中加入並鎖版」。鎖檔中的 `reqwest 0.13.4`
  **不含任何 TLS backend**（實際查證 Cargo.lock 的 dependency 清單），因此對外 HTTPS 無論如何都要新增
  TLS 相依。選 `ureq 3.4.2`（blocking、無 async runtime、default features = rustls + ring +
  bundled webpki-roots，不依賴系統 TLS 或系統信任庫，Linux／Windows CI 皆無系統套件需求），
  ZIP 的 deflate 用鎖檔既有的 `flate2 1.1`（不引入 ZIP 套件）。鎖檔新增 13 個 crate。
- **不在 P07 範圍**（計畫未要求、刻意不做）：Coinbase 獨立比對來源、`exchangeInfo` 交易規格、
  封存事後修訂偵測、排程（P17）、UI（P21）。

## 修改檔案

**新增**
- `src-tauri/src/market/http.rs`：`HttpFetcher` trait、`FetchLimits`／`FetchError`、`UreqFetcher`
  （HTTPS 限定、主機白名單、拒 redirect、大小上限、有界重試）、`#[cfg(test)] testing::FakeFetcher`。
- `src-tauri/src/market/sources/{mod,binance}.rs`：URL 版面、CHECKSUM 解析與比對、單一 entry ZIP 讀取、
  kline CSV／REST JSON 解析、整檔時間單位判定與精確換算、14 個穩定拒絕代碼。
- `src-tauri/src/market/ingest.rs`：單位計畫、快取、取得與記錄、合併衝突即拒、未收盤丟棄、匯入、
  snapshot、**要求區間涵蓋率**與 `IngestReport`。
- `src-tauri/src/market/fetch.rs`：`FetchOptions`、契約一致的日期解析、以 owner 身分開工作區執行一次
  取得、`register_known_instrument`（**只在不存在時**註冊，絕不覆蓋既有修訂）。
- `fixtures/binance/*.zip`＋`.CHECKSUM`＋`README.md`：四個真實日封存（毫秒／微秒各兩個）與其公布摘要。
- `docs/market-source-binance-v1.md`：adapter 契約與實測結果。

**修改**
- `src-tauri/Cargo.toml`／`Cargo.lock`：`ureq`、`flate2`。
- `src-tauri/src/market/mod.rs`：新模組登記。
- `src-tauri/src/runtime/service.rs`：`fetch` 子命令（參數解析、USAGE、`EXIT_RANGE_UNUSABLE = 5`、
  操作者摘要輸出）。
- `src-tauri/src/identity.rs`：`dataset_content_hash` 由 `#[cfg(test)]` 改為 `pub(crate)`——後端從此
  也是 dataset 的**產生者**；preimage 未動，寫入路徑仍重新驗證。
- 純契約雙語：`src/core/market-data/foundation.ts`、`discovery_core/market_foundation.rs` 新增
  `sourceOrigin`／`sourcesShareOrigin`；fixture 新增 `sourceOrigins` case 群組。
- `src-tauri/src/market/snapshot.rs`：組成時容許**同 origin** 的 `source_mismatch`。
- 文件：`docs/market-foundation-v1.md`、`docs/market-contract.md`、registry、`STRATEGY_DISCOVERY.md` §4、
  `README.md` 三語、`tasks.md`、`CHANGELOG.md`、本 handoff。

## 真實執行揭露的兩個問題（都已修正）

1. **snapshot 的涵蓋率不等於「我要的區間」的涵蓋率。**
   `market-snapshot-v1` 的 coverage 是以**資料集自身的起訖**推導的，所以少抓了一天的取得看起來「完整」。
   修正：ingest 另外以**要求的區間**做稽核（`IngestReport.rangeCoverage`），blocking 事件以
   `scope: "requested-range"` 寫入 `market_quality_events`，並新增 `IngestOutcome::RangeIncomplete`
   ——**資料集可以是合格 snapshot，而要求的區間仍被拒絕**。這不是繞過 P06，而是補上它刻意不回答的問題。
2. **同一交易所的封存與 REST 被判為來源混淆。**
   當月資料必須由「日封存＋同交易所 REST 尾端」組成，但 `series_conflicts` 的 `source_mismatch` 會擋下
   （實測一次產生 19 個 `source_conflict`）。修正：純契約新增 `<origin>[-<endpoint>]` 的出處概念，
   `series_conflicts` **仍照實回報** `source_mismatch`（那是事實），但 snapshot 組成在兩者
   `sources_share_origin` 為真時容許它；跨交易所永遠 blocking。雙語＋fixture 同步。

## Verification

本機 Windows，`CARGO_TARGET_DIR=C:/tmp/aff-target`。

- `cargo check --locked --all-targets`：**0 warning**。
- `cargo clippy --locked --all-targets`：與 P06 後相同的 5 項既有警告，本次新增程式 0 項
  （過程中出現的 3 項已修：identical blocks 不適用、`filter().next_back()` → `rfind`、
  manual saturating arithmetic、數字分組）。
- `cargo test --locked`：**328 passed**（lib 69＋bin 257＋service smoke 2），較 P06 後的 302 增加 26。
- `npm.cmd run typecheck`、`npm.cmd run build`：通過。
- `npm.cmd test`：**911 passed**（P06 後為 909）。
- `npx playwright test --workers=1`（`E2E_PORT=5209`）：**78 passed**（前端執行路徑未變）。
- **測試從不連網**：ingest 全程以 `FakeFetcher` 驅動，解析層讀 committed 的真實封存 bytes；
  只有傳輸層（`UreqFetcher`）沒有自動化測試，由下列真實執行證明。

### 真實執行（隔離工作區 `C:/tmp/aff-p07-evidence*`，非使用者正式工作區）

| 指令 | 結果 |
| --- | --- |
| `fetch BTCUSDT 1h 2024-12-30 → 2025-01-03` | `monthly:2024-12` 744 根（毫秒）、`monthly:2025-01` 744 根（微秒→毫秒）；**96／96**；snapshot 合格；exit 0 |
| `fetch BTCUSDT 1h 2025-01-01 → 2026-01-01` | 12 個月封存；**8760／8760**；snapshot 合格；exit 0 |
| `fetch ETHUSDT 1h 2025-01-01 → 2026-01-01` | 同上；**8760／8760**；snapshot 合格；exit 0 |
| `fetch BTCUSDT 1h 2026-09-01 → 2026-09-21` | `monthly:2026-09` 未公布 → 19 個日封存＋1 頁 REST；**462／463**（1 根未到期）；snapshot 合格；exit 0 |
| `fetch BTCUSDT 1h 2017-07-01 → 2017-09-01 --no-rest` | 7 月完全未公布、8 月 356 根；報告缺 **2017-07-01 00:00 → 2017-08-17 03:00（1132 根）**、動作 `refetch_range`；**exit 5，區間被拒絕** |

工作區內容：原件 content-addressed 保存（年度兩次執行共 45 個檔案／1.2 MB），DB 2.4 MB。

**不宣稱**：以上以外任何區間的完整性；BTC／ETH 多年完整；任何策略績效。

## 未完成項目／已知限制

- **`listedFrom` 未知**：上市前的空白被報成缺漏（2017 那筆即是）。由 `exchangeInfo` 或人工登記填入
  是後續工作；目前這樣是**誠實**的（我們確實不知道上市日）。
- **交易規格未知**：`lotSize`／`priceStep`／`minNotional` 仍為 `None` → 這些商品不可 paper（P19／P20 前置）。
- **REST 無第三方摘要**：REST 的證據是請求與回應本身；其 `availableAt` 記為回應當下。
- **封存事後修訂未偵測**：`revision_of` 機制已就緒（P06），但 adapter 目前對同一 URL 直接重用快取，
  不會主動發現「同一個月的封存內容變了」。要重抓必須先讓快取失效（目前只有原件被改動或遺失才會）。
- **無第二交易所比對**：Coinbase 仍未實作。
- **無排程與 UI**：取得只能手動執行 `fetch`；由服務代抓屬 P17，畫面屬 P21。
- **`fetch` 需要獨占工作區**：service 執行中會以 exit 2 拒絕。

## 對 P08／後續的交接點

- 新來源請照 `market/sources/<vendor>.rs` 的形狀：純函式、穩定拒絕代碼、單位判定回傳而不自行決定政策。
- 新 media type 要先擴充 `provenance::RAW_MEDIA_TYPES`；新事件代碼或 action 要先進
  `SNAPSHOT_EVENT_CODES`／`ACTION_CODES`（兩語言＋fixture），`QualityEvent::validate` 會擋未登記者。
- **ETF（P08–P10）會遇到 P07 沒遇到的事**：真實休市資料（`nyse-v1`／`twse-v1` 目前不存在，因此 ETF
  instrument 還無法註冊）、公司事件、當地時間 session、民國日期與成交量單位。
- `IngestRequest.cost_profile_version` 目前由呼叫端傳入；使用者確認成本模型的流程仍未建立（P21）。
- 若要讓服務自己排程取得：在 `research-command-v1` 增加一個 `data.fetch` 命令，重用
  `market::ingest::ingest`（它只需要 `&mut Connection`、`ArtifactStore` 與一個 `HttpFetcher`）。

## Resolution (added when acted on)

（待 review／merge 後補）
