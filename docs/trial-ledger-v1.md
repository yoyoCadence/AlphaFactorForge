# Trial ledger v1（`trial-ledger-v1` / `trial-family-v1`）— P12b 規格

> **狀態：規格草案（2026-09-23），待維護者審查；尚未實作。** 沒有 migration、程式或命令依本文存在。
> 上游：[`plans/active-plan.md`](plans/active-plan.md) §4.4／§4.5 與 §5 P12 列；
> 決策來源：[`../handoffs/2026-09-23-pr115-acceptance-review-v1.md`](../handoffs/2026-09-23-pr115-acceptance-review-v1.md)
> 的「Follow-up answers」與維護者 2026-09-23 的回覆；消費端：[`research-precision-v1.md`](research-precision-v1.md)（P12a）。
> 需遵守：[`research-history-v1.md`](research-history-v1.md)（P05 hypothesis／attempt）、
> [`market-foundation-v1.md`](market-foundation-v1.md)（P06 instrument／snapshot）、
> [`research-runtime-contract.md`](research-runtime-contract.md)（workspace 所有權、`workspaceId`）。

本文回答 P12 驗收的後半：**試驗計數不可重設**。它定義試驗帳本存在哪裡、家族如何識別、哪些事件算有效試驗、
登記如何與 runner 排序、以及還原／匯入如何合併。它**不**定義統計檢定、alpha 分配或 Validation／Test 消耗
（P12e／P13）。

---

## 0. 已採用的決策（維護者 2026-09-23）

| 問題 | 決策 |
| --- | --- |
| 精度公式 | 維持 P12a 現況；`ELIGIBLE` 只代表抽樣可行 |
| 帳本位置 | **所有工作區之外**的共用 SQLite registry，是 `priorTrials` 唯一權威來源；工作區只存引用 |
| 防止歸零 | 穩定 family／trial ID；還原與匯入採**事件聯集＋去重＋衝突拒絕**，不覆蓋、不取較大計數 |
| 試驗分類 | 事前定義；會影響候選選擇的 diagnostic 不能靠改標籤免計；固定 benchmark 與完全相同的重播可依明確規則不新增有效試驗，但事件仍保存 |
| 並行 | 登記與權威計數讀取在同一筆 registry 寫入交易；SQLite 單一寫入者 |
| P12e／P13 | P12 做純計算與規則（P12e-1..3）；P13 做確認批次凍結、alpha 原子預約與消耗、Validation／Test 一次性揭露與崩潰／重試恢復 |

本文把這些決策轉成可驗收的規則；§15 列出本文新增、仍需維護者確認的設計選擇。

---

## 1. 名詞

| 名詞 | 定義 |
| --- | --- |
| registry | 本文的 SQLite 資料庫檔，與任何工作區資料庫分離 |
| family（試驗家族） | 共享同一個多重比較校正的試驗集合（§3） |
| trial event（登記事件） | 一次登記；永遠保存，不修改、不刪除 |
| effective trial（有效試驗） | 依 §4.2 規則計入多重比較的事件 |
| event count／effective count／test count | 三個不同量：全部事件數；有效試驗數；有效試驗數 × 家族固定的 `testsPerTrial`（= P12a 的 `m`） |

---

## 2. 位置、開啟與版本

### 2.1 路徑

- 預設：`<platform data_local_dir>/com.alphafactorforge.evidence/trial-ledger.sqlite3`
  （Windows：`%LOCALAPPDATA%\com.alphafactorforge.evidence\`）。
  預設工作區是 `%APPDATA%\com.alphafactorforge.desktop`（Roaming），兩者不同目錄；Linux／macOS 上
  `data_local_dir` 可能等於 `data_dir`，但 `com.alphafactorforge.evidence` 仍是與工作區目錄**並列**的兄弟目錄。
- 覆寫：環境變數 `AFF_REGISTRY_DIR`（與既有 `AFF_DATA_DIR` 同樣只供隔離測試／native smoke）。
- **包含檢查**：開啟時把 registry 目錄與工作區目錄都 canonicalize；任一方在另一方之內即拒絕開啟
  （`registry_inside_workspace`／`workspace_inside_registry`）。工作區備份（P14）因此不可能帶到 registry。
- 必須在本機非同步磁碟：路徑落在 OneDrive／網路磁碟時拒絕寫入（SQLite 鎖在網路檔案系統上不可靠），
  理由碼 `registry_on_unsupported_volume`。偵測規則由實作定義並列入測試。
- 隔離測試必須同時設定 `AFF_DATA_DIR` 與 `AFF_REGISTRY_DIR`。只設前者時，測試試驗會寫入使用者真實 registry；
  這只會**增加**計數（保守方向），不會繞過規則，但仍列為測試紀律。

### 2.2 開啟與遷移

- registry 有自己的 migration 序列（`registry_migrations/0001_trial_ledger.sql` 起），與工作區
  `migrations/0001..0008` 分開；由同一個 Rust 模組在工作區開啟流程之後開啟。
- `registry_meta` 保存 `registry_id`（首次建立時 16 bytes 隨機 hex，永不改）與 schema 版本；
  schema 比本 build 新時拒絕（與工作區 P03a 規則一致）。
- 連線設定：WAL、`busy_timeout = 5000`、`foreign_keys = ON`；每次寫入使用 `BEGIN IMMEDIATE`。
- **不使用 workspace ownership lease。** 多個工作區（桌面、service、不同 `AFF_DATA_DIR`）可同時寫同一個 registry；
  互斥由 SQLite 寫鎖提供，冪等由 §4.3 的鍵提供。工作區的 OS 鎖不保護 registry。

---

## 3. 家族身分（`trial-family-v1`）

```text
familyKey  = canonical_json({"version":"trial-family-v1","instrumentId":<P06 instrument id>})
familyId   = "trial-family-v1:" + sha256(familyKey)
```

- v1 家族**只以標準化 instrument 劃分**。同一 instrument 上的所有試驗共享一個家族，不論 interval、日期範圍、
  資料來源、dataset hash、策略型別、工作區或目標。理由：這些欄位都能被任意更換；若納入家族鍵，換一個就得到
  零計數的新家族（AlphaBTC 反例的「開新 DB」變形）。
- `instrumentId` 取自試驗所用 dataset 的 P06 snapshot（`market_snapshots.instrument_id`，以 `dataset_id` 對應）。
  同一 dataset 對應到兩個以上不同的 `instrument_id` 時，視為家族不明。
  **沒有 snapshot 的 legacy dataset 沒有家族**：其試驗登記為 `familyId = NULL`，理由 `family_unknown`，
  且依 P06 本就不能取得新資格。家族不明 → 停止資格判定，不猜測。
- v1 只接受**單一 instrument** 的試驗；多 instrument 試驗在 v1 登記時拒絕（`multi_instrument_unsupported`），
  待多資產工作另訂版本，避免以子集／超集切出新家族。
- **Protocol 釘在家族上，不進家族鍵。** 家族第一次有效登記時凍結 `protocol_json =
  {"correction":"holm","testsPerTrial":<n>}`；之後任何登記的 protocol 不同即拒絕（`family_protocol_mismatch`）。
  因此一個家族的 `testsPerTrial` 必然一致，符合 P12a v1 的假設，也不能靠改 protocol 開新家族。
  改變 protocol 需要新的契約版本與維護者決定。
- 已知缺口：同一經濟標的在不同場所（例如兩個交易所的 BTC）是不同 instrument id，v1 視為不同家族。
  是否加入 append-only 的家族別名（alias）表列於 §15。

---

## 4. 試驗事件與分類

### 4.1 種類（v1 封閉列舉）

| `kind` | 何時使用 | 有效試驗？ |
| --- | --- | --- |
| `hypothesis` | 某 hypothesis 的第一個候選評估 | **是** |
| `variant` | 同一 hypothesis 的參數或結構變體（含 discovery 的每個候選） | **是** |
| `diagnostic` | 成本壓力、鄰近參數、敏感度等 | **是**（v1 一律計入，見 §4.2） |
| `benchmark` | `benchmark-suite` 固定白名單上的基準，參數完全等於凍結預設 | 否 |
| `reproduction` | 與既有已登記事件完全相同的重播（§4.2 條件全部成立） | 否 |
| `legacy` | §9 回填的既有 attempt | **是** |

### 4.2 有效性規則（登記時計算並凍結，不能事後改）

- `hypothesis`／`variant`／`diagnostic`／`legacy`：一律有效。v1 不提供「此 diagnostic 不影響選擇」的豁免，
  因為 v1 沒有能證明選擇時未讀取該結果的機制；要豁免需先有該機制並提升版本。
- `benchmark`：只有 [`benchmark-suite-contract.md`](benchmark-suite-contract.md) 的四個確定性基準
  （`buyHold`／`smaCross`／`rsiReversion`／`bollingerReversion`，參數等於契約凍結值）與 `random-entry-v1`
  （依其契約的 seed 與配對規則）不計入；其他任何「基準」都拒絕登記為 `benchmark`，須改登記為 `variant`。
- `reproduction`：必須指明 `reproductionOf = <eventId>`，且下列全部相同：strategy hash、dataset content hash、
  split／embargo 設定、engine fingerprint hash、seed。任何一項不同 → 拒絕（`reproduction_mismatch`），
  須登記為 `variant`。
- 事件的 `kind` 與 `effective` 屬於事件內容的一部分（進入 `eventId` 雜湊），因此「改標籤」只能透過新增事件，
  且同一冪等鍵已存在時會觸發 §4.3 的衝突。

### 4.3 冪等鍵與事件 ID

```text
idempotencyKey = "<workspaceId>:request:<requestId>:candidate:<index>"   # 新登記（§6.2）
               | "<workspaceId>:<attemptKey>"                              # 僅 §9 legacy 回填
eventPayload   = canonical_json({version:"trial-event-v1", familyId, kind, effective,
                                 idempotencyKey, originRegistryId, workspaceId, requestId, candidateIndex,
                                 legacyAttemptKey,
                                 hypothesisHash, strategyHash, datasetHash, snapshotId,
                                 engineFingerprintHash, reproductionOf})
eventId        = "trial-event-v1:" + sha256(eventPayload)
```

- 新登記不能使用 P05 `attempt_key`（`run:<run_id>:…`），因為 `run_id` 要到工作區交易才產生，晚於登記。
  改用入隊命令既有的 `research-command-v1` `requestId`（P03b，入隊前即存在且冪等）；attempt 另存 `trial_event_id`
  作為連結。直接呼叫、沒有命令信封的內部入隊路徑必須先產生並持久化一個 requestId 才能登記。
- 同一 `idempotencyKey` 再次登記：payload 相同 → 回傳既有事件（重試）；payload 不同 → 拒絕
  （`idempotency_conflict`），不寫入。**新的變體不是重試**：它有新的冪等鍵（新的 candidate index 或新的 requestId），因此是新事件。
- 時間戳（`registered_at`）不在 payload 內，以免同一登記在兩份 registry 產生不同 ID。
- `eventId` 是內容雜湊，跨 registry 聯集時同一事件必得同一 ID（§8）。

---

## 5. 計數與 P12a 對應

```text
priorTrials   = 家族中 effective = 1 的事件數（本批登記之前）
plannedTrials = 本批新增且 effective = 1 的事件數
testsPerTrial = 家族 protocol_json.testsPerTrial
m             = (priorTrials + plannedTrials) × testsPerTrial      # 即 P12a familyTests
```

- 只有後端從 registry 計算這些數字；**沒有任何命令、CLI 參數或 UI 欄位可以傳入 `priorTrials`**。
  P12a 的 plan JSON 由後端組裝。
- 失敗、取消、跳過、孤兒登記（§6.2）全部仍計入：登記即代表一次選擇機會，不退還。
- 計數是從事件表重新計算的投影，不是可寫欄位。若加快取表，必須可由事件表完全重建並在啟動時驗證。

---

## 6. 登記交易與跨資料庫順序

### 6.1 單一 registry 交易

`register_batch(family, events[]) -> {priorTrials, plannedTrials, testsPerTrial, eventIds}` 在一個
`BEGIN IMMEDIATE` 交易中：

1. 檢查家族衝突隔離（§8.3）；已隔離 → 拒絕。
2. 讀取本批之前的有效計數（`priorTrials`）。
3. 依 §4 驗證並冪等插入每個事件；首次有效登記時凍結家族 protocol。
4. 計算 `plannedTrials` = 本批**新**插入的有效事件數。已存在的事件（重試）不算新增。
   整批以 `batch_id`（排序後冪等鍵的 sha256）冪等：同一批重試時直接回傳 `trial_batches` 中保存的
   prior／planned，不重新計算；只部分重疊的批次是新批次，重疊部分不算新增。
5. COMMIT。

長時間工作（回測、bootstrap）一律在交易外執行。`SQLITE_BUSY` 在 busy timeout 後以同一冪等鍵重試。

### 6.2 與工作區的順序（兩個資料庫不能原子提交）

**順序固定為：registry 先 COMMIT，工作區後寫入。** runner 入隊（P05 `start_discovery_run_with_lineage`）
改為：

1. 以入隊命令的 `requestId` 與候選 index 產生冪等鍵與事件 payload；
2. `register_batch` COMMIT；
3. 工作區交易：建立 jobs／attempts，attempt 保存對應 `trial_event_id`；
4. 之後才可 claim／執行。

崩潰情境：

| 崩潰點 | 結果 |
| --- | --- |
| 2 之前 | 兩邊都沒有寫入；重新入隊是全新批次 |
| 2 之後、3 之前 | registry 有登記、工作區沒有 run → **孤兒登記**：仍計入，不退還。以同一 `requestId` 重試入隊會取回相同事件並完成第 3 步；換新 requestId 則是新批次、新事件 |
| 3 之後 | 正常；重試入隊由 P03b 命令冪等回傳既有結果 |

- claim 的工作區交易必須確認對應 attempt 的 `trial_event_id` 非 NULL（與 P05「恰好一筆 attempt」規則同一交易）；
  否則 rollback（`trial_not_registered`）。事件是否存在於目前 registry 屬跨資料庫檢查，在 claim 前讀 registry
  驗證，並由 §7 的綁定檢查涵蓋 registry 被替換的情況。
- 孤兒登記只能由工作區端辨識（registry 看不到工作區）；啟動時列出 `trial_event_id` 對應不到 attempt 的
  本工作區事件作為報告，不修改任何計數。

---

## 7. 工作區綁定與回退偵測

工作區在 `app_settings` 保存 `trial_ledger_binding = {registryId, eventCount, familyCounts}`：
上次成功登記後、本工作區所見 registry 的 ID 與（全域及各家族）事件數水位。

| 開啟時觀察 | 判定 |
| --- | --- |
| 未綁定，registry 存在或新建 | 綁定目前 registry |
| `registryId` 相同，計數 ≥ 水位 | 正常，更新水位 |
| `registryId` 相同，任一計數 < 水位 | **回退**：`registry_rolled_back`，停止資格判定 |
| registry 不存在，但工作區有綁定 | **缺失**：`registry_missing`，停止資格判定；**不自動新建空 registry 取代** |
| `registryId` 不同 | `registry_replaced`：除非新 registry 的 `registry_imports` 含舊 `registryId` 且匯入事件數 ≥ 水位（§8），否則停止資格判定 |

- 「停止資格判定」表示：探索與回測仍可執行，但任何需要 `priorTrials` 的確認／資格流程回報
  `NOT_ELIGIBLE`，理由為上述代碼。
- 限制：離線系統無法偵測使用者把**所有**副本（registry、全部工作區、全部備份）一致刪除或回退；本設計不宣稱防竄改。
  來源無法證明時不沿用原資格。

---

## 8. 匯出、匯入與還原（事件聯集）

### 8.1 匯出格式 `trial-ledger-export-v1`

JSON Lines：第一行 header `{version, registryId, schemaVersion, exportedAt, familyCount, eventCount,
bodySha256}`；其後每行一個家族（`familyId`、`familyKey`、`protocol_json`），再每行一個事件（完整 payload）。

### 8.2 匯入規則

1. 驗證 header、`bodySha256`、每個家族 `familyId = hash(familyKey)`、每個事件
   `eventId = hash(payload)`。任一失敗 → 整份拒絕（`import_integrity_failed`），不部分寫入。
2. 在一個交易中：家族以 `familyId` 聯集；事件以 `eventId` 聯集（已存在即略過）。
3. **衝突**：同一 `idempotencyKey` 對應不同 `eventId`，或同一 `familyId` 的 `protocol_json` 不同 →
   不寫入衝突事件，在 `family_conflicts` 追加紀錄並**隔離該家族**。
4. 追加 `registry_imports`（來源 `registryId`、檔案 sha256、匯入事件數、新增數、衝突數、時間）。
5. 計數由聯集後的事件表重新計算。**禁止**覆蓋 registry 檔、以 `max(countA, countB)` 合併或刪除任何事件。

### 8.3 還原

- **工作區還原**（P14）不含 registry（§2.1 包含檢查），因此還原舊工作區不會還原乾淨的研究歷史；
  該工作區的 §7 水位若高於目前 registry，會回報回退。
- **registry 還原**只能以匯入（聯集）進行：從備份匯出檔匯入到目前 registry。直接用舊檔取代 registry 會改變
  計數，下一次工作區開啟時由 §7 偵測為回退。
- 隔離中的家族在 v1 沒有自動解除途徑；解除需要維護者決定與新契約版本（§15）。

---

## 9. 既有資料回填

- P12b 實作的工作區 migration 為每個既有 `research_attempts` 列產生一筆 `legacy` 事件：
  idempotencyKey 使用既有 `workspaceId:attempt_key`，家族依 attempt 的 dataset 解析 snapshot instrument；
  解析不到 → `familyId = NULL`（`family_unknown`）。
- 回填是冪等的（同一鍵重跑回傳既有事件），在該工作區第一次綁定 registry 時執行一次，並記錄於水位。
- 不回填 P05 之前、沒有 attempt 列的歷史執行（`validation_records` 單獨存在者）：v1 無法可靠還原其候選數。
  這類工作區若其 dataset 有 snapshot，實作須在報告中列出「存在未計入的 pre-P05 紀錄」並對該家族回報
  `legacy_trials_unknown`，停止資格判定，直到維護者決定（§15）。
- 不匯入 AlphaBTC 的任何紀錄（plan §6）。

---

## 10. Schema 草案（registry `0001_trial_ledger`）

```sql
CREATE TABLE registry_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);   -- registry_id, schema_version
CREATE TABLE trial_batches (                -- 讓整批重試回傳相同 prior/planned（§6.1）
  batch_id       TEXT PRIMARY KEY,          -- sha256 of sorted idempotency keys
  family_id      TEXT,
  prior_trials   INTEGER NOT NULL,
  planned_trials INTEGER NOT NULL,
  created_at     TEXT NOT NULL
);
CREATE TABLE trial_families (
  family_id     TEXT PRIMARY KEY,           -- trial-family-v1:<sha256>
  family_key    TEXT NOT NULL,              -- canonical JSON
  protocol_json TEXT,                       -- NULL 直到第一筆有效事件；之後凍結
  created_at    TEXT NOT NULL
);
CREATE TABLE trial_events (
  event_id        TEXT PRIMARY KEY,         -- trial-event-v1:<sha256(payload)>
  seq             INTEGER NOT NULL UNIQUE,  -- 本 registry 的插入順序（不跨 registry 比較）
  family_id       TEXT REFERENCES trial_families(family_id),   -- NULL = family_unknown
  idempotency_key TEXT NOT NULL UNIQUE,
  kind            TEXT NOT NULL CHECK (kind IN ('hypothesis','variant','diagnostic','benchmark','reproduction','legacy')),
  effective       INTEGER NOT NULL CHECK (effective IN (0,1)),
  batch_id        TEXT NOT NULL REFERENCES trial_batches(batch_id),
  payload_json    TEXT NOT NULL,
  registered_at   TEXT NOT NULL
);
CREATE TABLE registry_imports (...);        -- §8.2 第 4 點
CREATE TABLE family_conflicts (...);        -- §8.2 第 3 點；存在任一列即隔離該家族
```

所有表以 `BEFORE UPDATE`／`BEFORE DELETE` 觸發器 `RAISE(ABORT)`，唯一例外是 `trial_families.protocol_json`
由 NULL 設為非 NULL 的單次轉換。工作區側：`research_attempts` 追加可為 NULL 的 `trial_event_id`
（新工作區 migration `0009`），P12b 之後建立的 attempt 必須非 NULL（由 runner 的交易檢查保證，舊列允許 NULL
直到回填）。

---

## 11. Rust 介面草案（純後端）

```rust
pub struct TrialLedger { /* registry connection */ }
impl TrialLedger {
    pub fn open(registry_dir: &Path, workspace_dir: &Path) -> Result<Self, LedgerError>;
    pub fn register_batch(&self, batch: &TrialBatch) -> Result<BatchRegistration, LedgerError>;
    pub fn family_counts(&self, family_id: &str) -> Result<FamilyCounts, LedgerError>;
    pub fn export(&self, out: &Path) -> Result<ExportSummary, LedgerError>;
    pub fn import(&self, file: &Path) -> Result<ImportReport, LedgerError>;
}
pub fn check_binding(ledger: &TrialLedger, binding: Option<&LedgerBinding>) -> BindingStatus;
```

- 不新增前端命令；P12d 的 admission 呼叫 `register_batch` 後把結果組成 P12a plan。
- 匯出／匯入先只提供 service CLI 子命令（與 P07 `fetch` 同一模式），UI 留給 P21。

---

## 12. 驗收案例（P12b 實作必須全部以隔離 registry＋工作區測試）

| # | 情境 | 期望 |
| --- | --- | --- |
| A1 | 新工作區（新 `AFF_DATA_DIR`）對同一 instrument 登記 | 計數延續既有家族，不歸零 |
| A2 | 同 instrument 改 interval／日期範圍／來源／重新匯入 dataset（hash 不同） | 同一家族，計數延續 |
| A3 | 同一冪等鍵重試登記（含同一 requestId 重試入隊） | 回傳相同 eventId 與相同 prior／planned，不增加計數 |
| A4 | 同一冪等鍵改 `kind`（例如 variant → diagnostic／benchmark） | `idempotency_conflict`，無寫入 |
| A5 | 不在白名單或參數不同的 benchmark | 拒絕為 benchmark |
| A6 | reproduction 任一識別欄位不同 | `reproduction_mismatch` |
| A7 | 失敗、取消、孤兒登記 | 全部仍計入 |
| A8 | 兩個程序同時 `register_batch` 同一家族 | 兩批皆成功或以 busy 重試成功；最終計數 = 兩批和；各批 prior 不重疊 |
| A9 | registry COMMIT 後、工作區寫入前崩潰 | 孤兒登記計入；重啟報告；重試入隊取回同事件 |
| A10 | 未登記的 attempt 嘗試 claim | `trial_not_registered`，rollback |
| A11 | 刪除 registry 檔後開啟已綁定工作區 | `registry_missing`；不自動新建取代 |
| A12 | 以舊 registry 檔取代 | `registry_rolled_back` 或 `registry_replaced` |
| A13 | 重複匯入同一匯出檔 | 無新增、計數不變 |
| A14 | 兩份 registry 各自新增試驗後互相匯入 | 聯集；計數 = 去重後總數，不是較大值 |
| A15 | 匯入含相同 idempotencyKey、不同 payload 的事件 | 家族隔離，資格判定停止 |
| A16 | 匯出檔被改一個位元組 | `import_integrity_failed`，無部分寫入 |
| A17 | 家族第二批使用不同 `testsPerTrial` | `family_protocol_mismatch` |
| A18 | 無 snapshot 的 legacy dataset | `family_unknown`，不能取得資格 |
| A19 | registry 目錄設在工作區內（或反之） | 開啟被拒 |
| A20 | P12a 串接 | 依 §5 組出的 plan 對 AlphaBTC 等價計數得 `146/1001` 且 `NOT_ELIGIBLE` |

---

## 13. 實作切分建議

| 子項 | 範圍 | 不含 |
| --- | --- | --- |
| P12b-1 | registry 模組、schema、`register_batch`、計數、冪等、衝突、匯出／匯入、包含檢查；A3–A6、A8、A13–A17、A19 | runner 接線、工作區 migration |
| P12b-2 | 工作區 migration `0009`（`trial_event_id`、綁定水位）、runner 入隊先登記、claim 檢查、回填、§7 偵測；A1、A2、A7、A9–A12、A18 | admission 阻擋（P12d） |

P12d 再用 A20 串接 P12a 並讓預檢失敗時不建立可執行的確認工作。

---

## 14. 不在範圍與已知限制

- 統計檢定、alpha 分配與噪音模擬（P12e-1..3）；確認批次、alpha 原子預約／消耗、Validation／Test 揭露（P13）。
- 不防竄改：本機使用者可刪除或一致回退所有副本（§7）。
- v1 家族不處理跨場所同標的與多 instrument 試驗（§3）。
- 不遷移 AlphaBTC 紀錄。

---

## 15. 待維護者確認

1. **家族粒度**：v1 以單一 instrument 為家族（不含 interval／期間／protocol）。這比審查建議的「商品／研究範圍／protocol」
   更粗、更保守；是否接受？
2. **跨場所別名**：是否在 v1 加入 append-only 家族別名表，或留待多資產工作？
3. **diagnostic 一律計入**：v1 不提供豁免；是否接受？
4. **pre-P05 歷史**：有 snapshot 但沒有 attempt 的舊紀錄，v1 以 `legacy_trials_unknown` 停止該家族資格判定；
   或改為由維護者一次性宣告保守估計值（須記錄來源）？
5. **隔離解除**：家族衝突隔離是否需要 v1 內的人工解除流程（會留下事件），或維持「僅新契約版本可解除」？
6. **registry 路徑**：`%LOCALAPPDATA%\com.alphafactorforge.evidence\` 是否可接受（不隨 Roaming 同步）？
