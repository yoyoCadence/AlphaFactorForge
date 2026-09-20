# Handoff: P06 資料基礎契約（instrument、calendar、raw、revision、snapshot）

Date: 2026-09-20
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p06-market-foundation`（自 main `a0f738e` ＝ PR #107 merge 之後建立）
PR: https://github.com/yoyoCadence/AlphaFactorForge/pull/108（draft，未 merge）
Status: 實作與驗證完成；P07 另行授權

## Summary

Plan §5 P06（ABC-04 的契約＋儲存半邊）。在既有 `datasets`／`candles` 之上新增市場語意層：
**商品與交易日曆是版本化、不可編輯的紀錄**；**每次取得（含被拒絕的）保存原件校驗碼與請求範圍，修訂只新增
不覆蓋**；**一份資料要成為研究可讀的 snapshot，必須先通過由交易日曆推導的涵蓋率稽核**，阻擋時寫下
「哪段範圍、什麼代碼、該做什麼」的結構化證據。純規則雙語鏡像並以 authored fixture 綁定。

**本階段不連網、不下載任何資料、不改任何既有行為。** 沒有 snapshot 的 dataset 是 legacy：照常匯入、
照常回測，只是不能自動取得資格。

資料形狀與不變量登記於 [`docs/market-foundation-v1.md`](../docs/market-foundation-v1.md)，驗收對照見該文 §6。

## 執行前驗證（plan vs codebase）

- **依賴**：P06 依賴 P03，P03a／P03b 已 Done 且已在 main（PR #106／#107 皆已 merge，本分支自 `a0f738e` 建立）。
- **契約來源存在且未過期**：P00 已凍結 `docs/market-contract.md`（`market-instrument-v1`／
  `market-provenance-v1`／`market-snapshot-v1`），其 §0 的「不得更動」清單（`dataset-content-v2`、
  `market-data-quality-v1`、0001 `datasets`／`candles`、`barsPerYear`）逐條在本次遵守。
- **既有程式路徑查證**：`db/mod.rs` migration 清單、`repositories::import_dataset_with_candles`
  （DATA-QUALITY-001 mount point 3）、`research::artifacts::ArtifactStore`、`runtime::open_workspace`
  的 owner-only 順序、`runtime::boundary_tests` 的 host-agnostic 守衛均與計畫假設一致。
- **計畫未明列而由本次判斷的實作細節**（不改 Objective／Scope／Acceptance／架構意圖）：
  1. 儲存模組落在 `src-tauri/src/market/`，與 P05 的 `src-tauri/src/research/` 對稱（計畫 §5 的新模組表
     未列 `market/`，只列了 `providers/`＝adapter，屬 P07）。
  2. 純規則放進 `discovery_core`（lib crate）＋ `src/core/market-data/foundation.ts`，沿用既有
     `market-data-quality-v1` 的雙語鏡像慣例與 `fixtures/rs-core/*` parity 機制（計畫 §7 要求雙語 fixture）。
  3. **未新增任何套件**。計畫 §5 允許「時區等必要依賴在所屬 phase 加入」，但 v1 把日線定義為
     「交易日的 UTC 午夜」、交易日由版本化 calendar 資料列舉，因此不需要 `chrono-tz`；ETF 盤中 session
     屬 P08，屆時再評估。這是往更保守方向的偏離，已登記於 registry §6 與
     [`docs/market-foundation-v1.md`](../docs/market-foundation-v1.md) §1.4。
  4. **未開 Tauri 命令面**。P06 沒有任何消費者（adapter 在 P07 從 Rust 呼叫、UI 在 P16／P21），
     開一個沒人呼叫、沒有測試的邊界不是功能。`market/mod.rs` 以 `#![allow(dead_code)]`＋註解說明，
     與 `service_main.rs` 既有做法一致，避免 `cargo check` 出現新 warning。
- **與既有開放決策的關係**：`market-data-quality-contract.md` §7 把 interval cadence 列為
  `INTERVAL-CONTRACT-001`（未決）。P06 只解掉**節奏**半邊，且只在本契約內使用（嚴格、無回退）；
  `barsPerYear`（含 `1d` = 365 與未知回退）完全未動，年化仍是未決半邊。已在該文件 §7 補記。

## 修改檔案

**純契約（雙語）**
- `src/core/market-data/foundation.ts`（新）＋ `foundation.test.ts`（新，21）
- `src-tauri/src/discovery_core/market_foundation.rs`（新，含 8 個單元測試）；`discovery_core/mod.rs` 註冊
- `src-tauri/src/discovery_core/market_foundation_parity_tests.rs`（新，7）
- `src/parity/marketFoundationFixture.ts`（新，authored spec＋build-time self-checks）、
  `marketFoundationFixture.test.ts`（新，9）、`scripts/generate-market-foundation-fixtures.ts`（新）、
  `package.json` 新增 `fixtures:market-foundation`、`fixtures/rs-core/market-foundation-v1.json`（新）

**儲存**
- `src-tauri/migrations/0008_market_foundation.sql`（新）：六張表＋索引＋immutable/kept 觸發器
- `src-tauri/src/market/mod.rs`（新）：版本常數、事件詞彙與 `QualityEvent::validate`（1 測試）
- `src-tauri/src/market/registry.rs`（新）：calendar／instrument 註冊與查詢、`ensure_builtin_calendars`、
  `DEFAULT_RESEARCH_SYMBOLS`、`default_crypto_instruments`（4 測試）
- `src-tauri/src/market/provenance.rs`（新）：`record_raw`、修訂鏈、quality events（5 測試）
- `src-tauri/src/market/snapshot.rs`（新）：`build_snapshot`、`dataset_market_status`（9 測試）
- `src-tauri/src/research/artifacts.rs`：新增 `put_with_extension`（raw CSV／ZIP 不冒充 `.json`）＋1 測試
- `src-tauri/src/db/mod.rs`：登記 0008；`db/repositories.rs`、`runtime/mod.rs` 的 migration 計數與名稱更新
- `src-tauri/src/runtime/mod.rs`：`open_workspace` 於 migration 後註冊內建 calendar（owner-only、冪等）
- `src-tauri/src/runtime/boundary_tests.rs`：四個 `market/*.rs` 納入 host-agnostic 掃描
- `src-tauri/src/main.rs`、`service_main.rs`：`mod market;`

**文件**
- `docs/market-foundation-v1.md`（新）、`docs/market-contract.md`（狀態與實作登記）、
  `docs/market-data-quality-contract.md`（§7 補記 cadence 半邊）、
  `docs/autonomous-research-capability-registry.md`（§3 市場資料、§6 依賴、§8 結論）、
  `STRATEGY_DISCOVERY.md` §4、`README.md` 三語、`tasks.md`（Snapshot／phase 表／Done／ABC-04 註記）、
  `CHANGELOG.md`、本 handoff

## 設計要點

- **預期由 calendar 推導，不由資料推導**：週末、休市、停牌、上市前／下市後不是缺漏。這是整個 coverage
  稽核成立的前提，也是為什麼 instrument 攜帶 `listedFrom`／`delistedAt`／`suspensions`。
- **不換算、不修補**：時間單位只偵測（ms→µs 回報索引），coverage 只報告。任何「猜著修好」都被排除。
- **整條可用或整條不可用**：coverage 事件全部 `blocking`，與 `market-data-quality-v1` 整份接受／拒絕一致；
  degraded 專門留給語意未確認（公司事件、成本），它讓 snapshot 存在但**不得晉級**。
- **排序用 rank 不用字串**：JS 與 Rust 的字串定序沒有保證，事件排序改用宣告順序的 code rank。
- **snapshot 識別不含 `asOf`**：dataset、instrument 修訂、calendar、來源皆不可變，同一組輸入就是同一份
  snapshot；`as_of` 與其 coverage 存在列上，重建回傳 `Existing`。
- **誠實阻擋優於捏造**：ETF 的 venue、上市日、交易規格、休市日都必須由其來源提供；
  `default_crypto_instruments()` 只給兩檔 Binance 現貨，`DEFAULT_RESEARCH_SYMBOLS` 保留 12 檔清單但不註冊。
- **不可達規則照實登記**：`unknown_kind`（Rust 端 enum）、`time_unit_mismatch`（走正常匯入的 dataset）、
  `out_of_order`（DB 以 `ORDER BY timestamp` 讀出）都在文件中標明為不可達／縱深防禦，沿用
  `market-data-quality-contract.md` §5 對不可達規則的處理方式。

## Verification

本機 Windows，`CARGO_TARGET_DIR=C:/tmp/aff-target`（OneDrive 連結鎖）。

- `cargo check --locked --all-targets`：**0 warning**（兩個 binary＋lib）。
- `cargo clippy --locked --all-targets`：與 P05 後**完全相同**——core 既有 4 項（backtest ×2、score ×2）
  ＋`file_commands.rs` 1 項；本次新增程式 0 項（過程中出現的 2 項已修掉：identical blocks、
  large enum variant → `Box<SnapshotRow>`）。
- `cargo test --locked`：**297 passed**（lib 68＋bin 227＋service smoke 2），較 P05 的 261 增加 36。
  新增：純契約 8、parity 7＋1（storage-only action 反向斷言）、市場儲存 19、artifact 副檔名 1。
- `npm.cmd run typecheck`、`npm.cmd run build`：通過。
- `npm.cmd test`：**909 passed**（P05 為 879；新增 21 單元＋9 parity）。
- `npx playwright test --workers=1`（`E2E_PORT=5207`）：**78 passed**，與 P05 相同——本次未改任何前端執行
  路徑（只新增 `src/core` 純模組與 parity 測試）。
- 原生 Tauri 未執行：啟動路徑僅新增 owner-only 的內建 calendar 註冊，該路徑由 `runtime` Rust 測試覆蓋
  （`open_workspace` 全套測試通過，含 migration 計數 8、connect 模式的 `SchemaPending`／`SchemaTooNew`）。
- 暫存：`aff-market-test-*`／`aff-snapshot-test-*`／`aff-artifacts-test-*` 皆由 guard 清除；無殘留。
- **未執行且不宣稱**：任何真實來源下載、CHECKSUM 校驗、長期涵蓋率報告。P06 不連網。

## 未完成項目／已知限制

- **沒有任何真實市場資料進入系統**：P06 建立的是形狀與閘門。BTC／ETH 涵蓋率、ETF 權限與事件完整性
  仍為未驗證（registry §3）。
- **`nyse-v1`／`twse-v1` 不存在**：因此 US／TW ETF instrument 目前無法註冊（誠實阻擋，非 bug）。
- **snapshot 尚未與回測／discovery 綁定**：研究目前仍可直接使用 legacy dataset；「研究只讀 snapshot」
  由 P12 起逐步收緊，本階段若強制會直接癱瘓既有功能。
- **無命令面與 UI**：阻擋範圍目前只能由 Rust 查詢（`dataset_quality_events`）；呈現屬 P16／P21。
- **instrument 交易規格全為未知**：`has_trading_specification()` 一律 false，paper（P19／P20）以此為前提。
- `earlyCloses` 只記錄不參與運算；盤中 session 與當地時間邊界屬 P08。
- **重複的被阻擋建置會重複寫入證據**：`build_snapshot` 每次被呼叫都是一次觀測，事件表 append-only，
  因此同一個缺口被檢查兩次會有兩列（時間不同）。這是刻意的稽核語意，不是去重失敗；若 P21 的 UI 需要
  「同一缺口只顯示一次」，在讀取端彙整，不要改寫證據。

## 對 P07／後續的交接點

- 新表遵守與 0001–0007 相同規則（新增序號、原文不改）；`open_migrated` 的「恰好本 build」檢查已含 0008，
  舊 service 與新桌面混跑會以 `SchemaPending` 拒連。
- **P07 的落點**：下載後 → `provenance::record_raw`（`role: primary`、`media_type` 依實際回應、
  `available_at` 為該資料最早可觀測時間）→ 匯入 dataset（既有 `import_dataset_with_candles`，不改）→
  `registry::register_instrument`（把來源回報的 lot／step／上市日寫成新修訂）→ `snapshot::build_snapshot`。
  來源修訂用 `revision_of`，被拒絕的回應**也要記**（`accepted: false` + 理由）。
- 需要新的 media type（例如 `application/gzip`）時：擴充 `provenance::RAW_MEDIA_TYPES` 並補測試；
  不要把非 JSON 塞進 `.json`。
- 需要新的事件代碼或 action：先加進 `SNAPSHOT_EVENT_CODES`／`ACTION_CODES`（兩個語言＋fixture inventory），
  再使用；`QualityEvent::validate` 會拒絕未登記者。
- 想在 snapshot 文件加欄位：提升 `MARKET_SNAPSHOT_VERSION`，讀取端以版本分支；既有列不改。
- 事件排序與代碼清單是**雙語綁定**的：任何修改都必須同步 `foundation.ts`、`market_foundation.rs`、
  `marketFoundationFixture.ts`，並重跑 `npm run fixtures:market-foundation`。

## Resolution (added when acted on)

（待 review／merge 後補）
