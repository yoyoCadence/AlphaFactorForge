# Trial ledger v1（`trial-ledger-v1` / `trial-family-v1`）— P12b 規格

> **狀態：規格草案（2026-09-23），待維護者審查；尚未實作。** 沒有 migration、程式或命令依本文存在。
> 修訂（同日）：依 [PR #116 驗收審查](../handoffs/2026-09-23-pr116-acceptance-review-v1.md) R1–R4 修正——
> 匯出含批次與收據（§8）、以雜湊鏈取代計數水位（§7）、payload 納入 split／seed／benchmark 身分（§4.3）、
> 批次 ID 綁定完整內容且重播前一律逐筆比對（§6.1）；`originRegistryId` 移出事件雜湊（§4.3）。
> 修訂二（同日）：依覆驗 R5 區分「歷史收據」與「admission 用的目前計數」（§5、§6.3、§6.4），
> 並加入新鮮度圍欄與 A31–A32。
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

本文把這些決策轉成可驗收的規則；§15 記錄 v1 的六項政策決定。

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
  跨場所別名延後至具明確等價與合併規則的契約版本（§15）；v1 的計數保證限於單一標準化 instrument。

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
  判定所需的身分寫在事件本身（§4.3 `benchmarkId`、`benchmarkParamsHash`），因此匯入時可重新驗證白名單，
  不依賴登記端的說法。
- `reproduction`：必須指明 `reproductionOf = <eventId>`，且下列欄位與被參照事件的 payload **逐一相等**且皆非 null：
  `strategyHash`、`datasetHash`、`splitHash`（split＋embargo）、`seedsHash`、`engineFingerprintHash`。
  任何一項不同或為 null → 拒絕（`reproduction_mismatch`），須登記為 `variant`。被參照事件必須已在同一 registry
  （或同一匯入檔）中；找不到 → `reproduction_reference_missing`。這些欄位都在不可變 payload 內，所以匯入端可獨立重驗。
  §9 回填的 `legacy` 事件若無法還原 `splitHash`／`seedsHash`（為 null），就不能作為 reproduction 的對象。
- 事件的 `kind` 與 `effective` 屬於事件內容的一部分（進入 `eventId` 雜湊），因此「改標籤」只能透過新增事件，
  且同一冪等鍵已存在時會觸發 §4.3 的衝突。

### 4.3 冪等鍵與事件 ID

```text
idempotencyKey = "<workspaceId>:request:<requestId>:candidate:<index>"   # 新登記（§6.2）
               | "<workspaceId>:<attemptKey>"                              # 僅 §9 legacy 回填
eventPayload   = canonical_json({version:"trial-event-v1", familyId, kind, effective,
                                 idempotencyKey, workspaceId, requestId, candidateIndex, legacyAttemptKey,
                                 hypothesisHash, strategyHash, datasetHash, snapshotId,
                                 splitHash, seedsHash, engineFingerprintHash,
                                 benchmarkId, benchmarkParamsHash, reproductionOf})
eventId        = "trial-event-v1:" + sha256(eventPayload)
```

| 欄位 | 定義 |
| --- | --- |
| `splitHash` | `sha256(canonical_json(split plan + embargo breakdown))`：Train／Validation／Test 範圍與 embargo 推導結果（split／embargo 契約版本一併納入） |
| `seedsHash` | `sha256(canonical_json({rootSeed, seeds}))`，取自 P05 attempt 的 `input_fingerprint_json` |
| `benchmarkId`／`benchmarkParamsHash` | 僅 `kind = benchmark` 時非 null；參數為 canonical JSON 的 sha256 |
| `reproductionOf` | 僅 `kind = reproduction` 時非 null |

- 所有欄位**一律出現**在 canonical JSON 中；不適用者為 `null`，因此「缺欄位」與「欄位為 null」不會產生兩種雜湊。
- **`originRegistryId` 不在 payload 內**，而是事件列的來源欄位（`origin_registry_id`，第一個登記它的 registry；
  不參與識別）。因此同一次登記（相同冪等鍵與內容）即使在兩個 registry 各自寫入（例如 registry 被替換後重試），
  仍得到**同一個 `eventId`**，聯集時自然去重；同一冪等鍵但內容不同才是衝突（§8.2）。

- 新登記不能使用 P05 `attempt_key`（`run:<run_id>:…`），因為 `run_id` 要到工作區交易才產生，晚於登記。
  改用入隊命令既有的 `research-command-v1` `requestId`（P03b，入隊前即存在且冪等）；attempt 另存 `trial_event_id`
  作為連結。直接呼叫、沒有命令信封的內部入隊路徑必須先產生並持久化一個 requestId 才能登記。
- 同一 `idempotencyKey` 再次登記：payload 相同 → 回傳既有事件（重試）；payload 不同 → 拒絕
  （`idempotency_conflict`），不寫入。**新的變體不是重試**：它有新的冪等鍵（新的 candidate index 或新的 requestId），因此是新事件。
- 時間戳（`registered_at`）不在 payload 內，以免同一登記在兩份 registry 產生不同 ID。
- `eventId` 是內容雜湊；在上述 `originRegistryId` 規則下，跨 registry 聯集時同一登記必得同一 ID（§8）。

---

## 5. 計數與 P12a 對應

P12a plan 只能由 §6.4 的 `admissionCount`（**讀取當下**的計數快照）組成：

```text
familyEffectiveTrials = 快照當下家族中 effective = 1 的事件數（含本批）
batchEffectiveTrials  = 本批 effective = 1 的事件數
P12a priorTrials      = familyEffectiveTrials − batchEffectiveTrials
P12a plannedTrials    = batchEffectiveTrials
testsPerTrial         = 家族 protocol_json.testsPerTrial
m                     = familyEffectiveTrials × testsPerTrial      # 即 P12a familyTests
```

- 這裡的 `priorTrials` 是「快照當下家族中**本批以外**的全部有效試驗」，**包含本批之後才登記的試驗**；
  它不是「本批登記時已存在的試驗數」。後者只存在於歷史收據（§6.3），**不得**用於 plan（R5）。
- 只有後端從 registry 計算這些數字；**沒有任何命令、CLI 參數或 UI 欄位可以傳入 `priorTrials`**。
  P12a 的 plan JSON 由後端組裝。
- 失敗、取消、跳過、孤兒登記（§6.2）全部仍計入：登記即代表一次選擇機會，不退還。
- 計數是從事件表重新計算的投影，不是可寫欄位。若加快取表，必須可由事件表完全重建並在啟動時驗證。

---

## 6. 登記交易與跨資料庫順序

### 6.1 單一 registry 交易

一批只屬於一個家族（`familyId` 可為 null，代表 `family_unknown`），且每個事件恰好屬於一批。

```text
batchId = "trial-batch-v1:" + sha256(canonical_json({version:"trial-batch-v1", familyId,
                                     members: [[idempotencyKey, eventId], ...]}))   # 依 idempotencyKey 排序
```

`batchId` 綁定**整批每個事件的完整內容**（經由 `eventId`），不只是冪等鍵。
`register_batch(family, events[]) -> {batchId, eventIds, testsPerTrial, admissionCount, registrationReceipt}`
在一個 `BEGIN IMMEDIATE` 交易中依序：

1. 檢查家族衝突隔離（§8.2 第 2 點）；已隔離 → 拒絕。
2. 依 §4 驗證每個事件並計算其 `eventId`，再算出 `batchId`。
3. **逐筆比對（永遠先做，不能被步驟 4 略過）**：對每個冪等鍵查既有事件——
   - 存在但 `eventId` 不同 → 整批拒絕（`idempotency_conflict`），不寫入；
   - 存在且 `eventId` 相同但屬於另一批 → 整批拒絕（`batch_conflict`）：v1 不允許與既有批次部分重疊。
4. 若 `batchId` 已存在（由步驟 3 可知此時每個事件都逐位相同）→ 這是整批重試：不寫入；
   `registrationReceipt` 取既有的歷史收據（§6.3）。
5. 否則（本批沒有任何既有鍵）：依序寫入批次、事件（附 §7 雜湊鏈）與本 registry 的歷史收據
   （`familyEffectiveBefore` = 寫入前家族有效事件數，`batchEffectiveTrials` = 本批有效事件數）；首次有效登記時凍結家族 protocol。
6. **不論步驟 4 或 5**，仍在同一交易內讀出目前計數快照 `admissionCount`（§6.4）。
7. COMMIT。

首次登記時兩者一致（`admissionCount.familyEffectiveTrials` = 收據的 `familyEffectiveBefore` ＋ `batchEffectiveTrials`）；整批重試時，
收據是登記當下的舊數字，`admissionCount` 則包含這段期間其他批次新增的試驗（R5）。

因此「同一組冪等鍵、但 strategy hash 或 kind 改變」的整批重送會在步驟 3 以 `idempotency_conflict` 失敗，
不可能拿到舊收據。

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
| 2 之後、3 之前 | registry 有登記、工作區沒有 run → **孤兒登記**：仍計入，不退還。以同一 `requestId` 重試入隊會取回相同事件（整批重播）與**重試當下**的 `admissionCount`，並完成第 3 步；任何 admission 判定都用這個新快照，不用歷史收據（§6.4）。換新 requestId 則是新批次、新事件 |
| 3 之後 | 正常；重試入隊由 P03b 命令冪等回傳既有結果 |

- claim 的工作區交易必須確認對應 attempt 的 `trial_event_id` 非 NULL（與 P05「恰好一筆 attempt」規則同一交易）；
  否則 rollback（`trial_not_registered`）。事件是否存在於目前 registry 屬跨資料庫檢查，在 claim 前讀 registry
  驗證，並由 §7 的綁定檢查涵蓋 registry 被替換的情況。
- 孤兒登記只能由工作區端辨識（registry 看不到工作區）；啟動時列出 `trial_event_id` 對應不到 attempt 的
  本工作區事件作為報告，不修改任何計數。

### 6.3 批次收據

- 收據 = `(batchId, receiptRegistryId, familyEffectiveBefore, batchEffectiveTrials)`，存於 `batch_receipts`，主鍵
  `(batch_id, receipt_registry_id)`。它記錄「該批在某個 registry 登記當下」的計數，**只是歷史稽核資料**：
  供重試辨認批次、以及事後查閱登記當時的家族規模。它**不是**計數來源（計數一律由事件表重新計算，§5），
  也**不得**作為 P12a plan 或任何資格判定的輸入——那只能用 §6.4 的 `admissionCount`（R5）。
- 整批重試的回傳：若目前 registry 有自己的收據，回傳它；否則（此批只經匯入得知）回傳批次
  `origin_registry_id` 的收據。兩者都沒有 → 資料不一致，拒絕（`batch_receipt_missing`）。
- 同一批可能在兩個 registry 各自登記（例如 registry 被替換後以同一 requestId 重試），因此可能有多張收據；
  匯入時以主鍵聯集，同主鍵不同數值 → 衝突（§8.2）。

### 6.4 目前計數（`admissionCount`）與新鮮度圍欄

```text
admissionCount = {registryId, familyId, familyEffectiveTrials, batchEffectiveTrials,
                  testsPerTrial, seq, chainHead}
```

- 只有兩個來源，而且都在**單一 registry 交易**內一次讀出，所以計數與鏈頭描述同一個 registry 狀態：
  - `register_batch` 的步驟 6：首次登記或整批重播都一樣，與登記／重播驗證在同一筆 `BEGIN IMMEDIATE` 交易內
    （符合 §0「登記與權威計數讀取在同一筆寫入交易」）；
  - `read_admission_count(batchId)`：唯讀交易，供之後重讀。
- `familyId` 為 null（`family_unknown`）或家族已隔離 → 不回傳快照，改回報對應理由；資格判定停止。
- 使用 `admissionCount` 之前，工作區綁定必須已通過 §7 檢查。

**新鮮度圍欄。** 依某個 `admissionCount` 做出的判定，只對該快照的 `familyEffectiveTrials` 有效。之後任何階段要依據
**`ELIGIBLE`** 判定行動之前（P12d 派送可執行的確認工作；P13 凍結確認批次或預約 alpha），都必須以
`read_admission_count` 重讀，並依序：

1. **先證明快照看到的事件都還在**：同一 `registryId` 時，registry 在 `snapshot.seq` 的鏈值必須等於
   `snapshot.chainHead`；`registryId` 不同時，依 §7.3 以 `(snapshot.registryId, snapshot.seq, snapshot.chainHead)`
   驗證檢查點。不成立 → 依 §7 停止資格判定。
2. **再比較家族有效計數**：
   - 相等 → 家族沒有新增有效試驗，原判定可沿用；
   - 變大 → 原判定過時：以新快照重算 P12a，連同新快照保存新判定；重算仍為 `ELIGIBLE` 才能繼續；
   - 變小 → 步驟 1 通過時不可能發生（事件只增不刪、有效性登記時凍結）；若發生即視為不一致，停止資格判定。

- 只有 `ELIGIBLE` 需要圍欄：家族有效計數只增不減，而 P12a 的需求值不會因家族變大而降低
  （P12a `growing_the_family_can_only_raise_requirements`），所以過時的 `NOT_ELIGIBLE` 不可能因重讀變成 `ELIGIBLE`。
- 已保存的判定經 P03b 重送時逐字重播（含其快照）；圍欄仍在該判定被用來行動之前套用。
  尚未保存判定的重試，則由 `register_batch` 取得新的 `admissionCount`（A31）。
- P12d 派送前的重讀，是為了不派送明顯過時的判定；它與工作區寫入之間仍可能有其他批次登記。
  **不留空隙的保證屬於 P13**，並作為交給 P13 的要求：確認凍結所用的最後一次計數，與凍結／alpha 預約之間，
  不得有「有效試驗已登記卻沒被看到」的空隙——在同一筆 registry 寫入交易內讀取並預約；或在預約提交後重讀比對，
  不相等即作廢該預約、重新判定。P12b 不實作這個預約。

---

## 7. 工作區綁定與回退偵測

計數水位不足以證明「看過的事件都還在」：舊副本只有 A，再加入 C 後計數與「A、B」相同，卻遺失了 B。
因此 v1 以**每個 registry 自己的 append-only 雜湊鏈**作為包含證據。

### 7.1 雜湊鏈

```text
chain_0   = sha256("trial-ledger-v1:genesis:" + registryId)
chain_seq = sha256(chain_{seq-1} || eventId_seq)          # seq = 本 registry 的插入順序，從 1 起連續
```

- 每筆事件列保存 `seq` 與 `chain`。本地登記與匯入的事件都依寫入順序接在**本 registry** 的鏈上。
- `chain_seq` 承諾了 seq ≤ 該值的**全部**事件及其順序，所以「在 seq = N 時的鏈頭相同」等於「前 N 筆事件完全相同」。
- registry 開啟時重算整條鏈；任何不一致 → `registry_chain_broken`，停止資格判定。
  （這偵測的是不一致，不是防竄改：能改檔的人也能重算整條鏈，見 §14。）

### 7.2 工作區綁定

工作區在 `app_settings` 保存 `trial_ledger_binding = {registryId, seq, chainHead}`，並在寫入 attempts 的
同一筆工作區交易（§6.2 步驟 3）中更新為 `register_batch` 回傳之 `admissionCount` 的 `(seq, chainHead)`
（與計數同一筆 registry 交易讀出，§6.4）。這個鏈頭涵蓋本工作區當時看得到的所有事件，包括其他工作區的事件。

| 開啟時觀察 | 判定 |
| --- | --- |
| 未綁定，registry 存在或新建 | 綁定目前鏈頭 |
| `registryId` 相同，且 registry 在 `binding.seq` 的 `chain` = `binding.chainHead` | 正常，更新為目前鏈頭 |
| `registryId` 相同，registry 最大 seq < `binding.seq` | **回退**：`registry_rolled_back` |
| `registryId` 相同，`binding.seq` 位置的 `chain` ≠ `binding.chainHead` | **分歧**：`registry_diverged`（同計數、內容不同的副本在此被擋下） |
| registry 不存在，但工作區有綁定 | **缺失**：`registry_missing`；**不自動新建空 registry 取代** |
| `registryId` 不同 | 見 §7.3；不成立 → `registry_replaced` |

以上除第一、二列外都停止資格判定。

### 7.3 更換 registry

新 registry 只有在**持有舊 registry 的已驗證鏈檢查點**時才被接受：
`origin_checkpoints` 中存在 `(origin_registry_id = binding.registryId, origin_seq = binding.seq)`，
且其 `origin_chain = binding.chainHead`。這些檢查點只能經 §8.2 匯入，匯入時已由事件 ID 從
創世值重算驗證，而對應事件也都已寫入本 registry。成立 → 改綁新 registry 的目前鏈頭；否則 `registry_replaced`。

只比對匯入事件數是不夠的：「匯入了等量事件但缺少工作區看過的某一筆」會使鏈值不同而被拒絕。

- 「停止資格判定」表示：探索與回測仍可執行，但任何需要 `priorTrials` 的確認／資格流程回報
  `NOT_ELIGIBLE`，理由為上述代碼。
- 限制：離線系統無法偵測使用者把**所有**副本（registry、全部工作區、全部備份）一致刪除或回退；本設計不宣稱防竄改。
  來源無法證明時不沿用原資格。

---

## 8. 匯出、匯入與還原（事件聯集）

### 8.1 匯出格式 `trial-ledger-export-v1`

JSON Lines；第一行 header：
`{version, registryId, schemaVersion, exportedAt, familyCount, batchCount, eventCount, headSeq, headChain, bodySha256}`。
其後依下列順序，每行一筆帶 `type` 的紀錄（匯出是**完整**的，不做部分匯出）：

| `type` | 內容 |
| --- | --- |
| `family` | `familyId`、`familyKey`、`protocol_json` |
| `batch` | `batchId`、`familyId`、`originRegistryId`、`members`（依 idempotencyKey 排序的 `[idempotencyKey, eventId]`） |
| `event` | 完整 payload、`originRegistryId`、`batchId`、`seq`、`chain`（皆為來源 registry 的值，依 `seq` 遞增） |
| `receipt` | `batchId`、`receiptRegistryId`、`familyEffectiveBefore`、`batchEffectiveTrials` |
| `originCheckpoint` | 來源 registry 持有的其他 registry 檢查點（`originRegistryId`、`originSeq`、`eventId`、`originChain`），每個來源都從 seq 1 起完整列出 |

批次與收據都在檔內，所以匯入不需要合成任何列，重試收據原樣保留（R1）。

### 8.2 匯入規則

1. **完整性（寫入前全部檢查）**：header 與 `bodySha256`；每個 `familyId = hash(familyKey)`；每個
   `eventId = hash(payload)`；每個 `batchId = hash(familyId, members)`，members 中每個 `eventId` 都有對應的
   `event` 紀錄且其 `batchId` 相符，每個事件恰屬一批；每張收據的 `batchId` 存在於檔內；
   依 §4.2 重驗每個 `benchmark`（白名單與參數雜湊）與 `reproduction`（參照事件在檔內或本 registry，且五個識別欄位相等）；
   從來源 `registryId` 的創世值依序重算 `event.chain` 至 `headSeq`／`headChain`；每個 `originCheckpoint`
   來源也從其創世值重算。任一失敗 → 整份拒絕（`import_integrity_failed`），不部分寫入。
2. **衝突偵測**（對照本 registry）：同一 `idempotencyKey` 對應不同 `eventId`；事件已存在但屬於另一批；
   同一 `familyId` 的 `protocol_json` 不同；同一 `(batchId, receiptRegistryId)` 收據數值不同。
   衝突的列不寫入；在 `family_conflicts` 追加紀錄並**隔離該家族**。其他家族照常匯入。
   同一 `(originRegistryId, originSeq)` 的檢查點鏈值不同（同一 registry 出現兩段歷史）→ 該來源的檢查點全部不寫入，
   在 `registry_conflicts` 追加紀錄；§7.3 因而無法以該來源接受替換。
3. **單一交易、依外鍵順序寫入**（`foreign_keys = ON`）：家族 → 批次 → 事件（以來源 `seq` 順序接到本地鏈，
   保存 `origin_registry_id`）→ 收據 → 檢查點（來源 registry 自己的鏈，以及檔內其他來源的檢查點）。
   已存在的相同列略過。
4. 追加 `registry_imports`（來源 `registryId`、檔案 sha256、`headSeq`／`headChain`、新增與略過數、衝突數、時間）。
5. 計數由聯集後的事件表重新計算。**禁止**覆蓋 registry 檔、以 `max(countA, countB)` 合併或刪除任何事件。

### 8.3 還原

- **工作區還原**（P14）不含 registry（§2.1 包含檢查），因此還原舊工作區不會還原乾淨的研究歷史；
  還原後的綁定較舊，只要目前 registry 仍含該鏈頭的前綴就正常接受，計數不會變少。
- **registry 還原**只能以匯入（聯集）進行：從備份匯出檔匯入到目前 registry。直接用舊檔取代 registry
  會在下一次工作區開啟時由 §7 偵測為回退或分歧。
- 隔離中的家族在 v1 不提供解除途徑；解除須由後續契約版本定義可稽核、僅追加的流程（§15）。

---

## 9. 既有資料回填

- P12b 實作的工作區 migration 為每個既有 `research_attempts` 列產生一筆 `legacy` 事件：
  idempotencyKey 使用既有 `workspaceId:attempt_key`，家族依 attempt 的 dataset 解析 snapshot instrument；
  解析不到 → `familyId = NULL`（`family_unknown`）。
- 回填依家族分批：每個 `(workspaceId, familyId)` 一批（`familyId` 為 null 者一批），走一般的
  `register_batch`，因此適用相同的冪等、衝突與收據規則。`splitHash`／`seedsHash` 能由 attempt 的
  input fingerprint 與既有 run 設定還原者填入，否則為 null。
- 回填是冪等的（同一批重跑回傳既有收據），在該工作區第一次綁定 registry 時執行一次，並寫入綁定鏈頭。
- 不回填 P05 之前、沒有 attempt 列的歷史執行（`validation_records` 單獨存在者）：v1 無法可靠還原其候選數。
  這類工作區若其 dataset 有 snapshot，實作須在報告中列出「存在未計入的 pre-P05 紀錄」並對該家族回報
  `legacy_trials_unknown`，停止資格判定；恢復資格須由後續契約版本定義（§15）。
- 不匯入 AlphaBTC 的任何紀錄（plan §6）。

---

## 10. Schema 草案（registry `0001_trial_ledger`）

```sql
-- 建表與寫入順序即外鍵順序：families → batches → events → receipts（§8.2 第 3 點）
CREATE TABLE registry_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);   -- registry_id, schema_version
CREATE TABLE trial_families (
  family_id     TEXT PRIMARY KEY,           -- trial-family-v1:<sha256(family_key)>
  family_key    TEXT NOT NULL,              -- canonical JSON
  protocol_json TEXT,                       -- NULL 直到第一筆有效事件；之後凍結
  created_at    TEXT NOT NULL
);
CREATE TABLE trial_batches (                -- 批次身分（§6.1）；不含計數
  batch_id           TEXT PRIMARY KEY,      -- trial-batch-v1:<sha256(familyId, sorted [key, eventId])>
  family_id          TEXT REFERENCES trial_families(family_id),     -- NULL = family_unknown
  members_json       TEXT NOT NULL,         -- 排序後的 [idempotencyKey, eventId]，可重算 batch_id
  origin_registry_id TEXT NOT NULL,
  created_at         TEXT NOT NULL
);
CREATE TABLE trial_events (
  event_id           TEXT PRIMARY KEY,      -- trial-event-v1:<sha256(payload)>
  seq                INTEGER NOT NULL UNIQUE,   -- 本 registry 的鏈位置，從 1 起連續
  chain              TEXT NOT NULL,         -- §7.1
  family_id          TEXT REFERENCES trial_families(family_id),     -- NULL = family_unknown
  idempotency_key    TEXT NOT NULL UNIQUE,
  kind               TEXT NOT NULL CHECK (kind IN ('hypothesis','variant','diagnostic','benchmark','reproduction','legacy')),
  effective          INTEGER NOT NULL CHECK (effective IN (0,1)),
  batch_id           TEXT NOT NULL REFERENCES trial_batches(batch_id),   -- 每個事件恰屬一批
  payload_json       TEXT NOT NULL,         -- canonical，可重算 event_id
  origin_registry_id TEXT NOT NULL,         -- 來源，不參與識別（§4.3）
  registered_at      TEXT NOT NULL
);
CREATE TABLE batch_receipts (               -- §6.3；不是計數來源
  batch_id            TEXT NOT NULL REFERENCES trial_batches(batch_id),
  receipt_registry_id TEXT NOT NULL,
  family_effective_before INTEGER NOT NULL,   -- 刻意不叫 prior_trials：不是 P12a 輸入
  batch_effective_trials  INTEGER NOT NULL,
  PRIMARY KEY (batch_id, receipt_registry_id)
);
CREATE TABLE origin_checkpoints (           -- §7.3；只經匯入寫入且已重算驗證
  origin_registry_id TEXT NOT NULL,
  origin_seq         INTEGER NOT NULL,
  event_id           TEXT NOT NULL REFERENCES trial_events(event_id),
  origin_chain       TEXT NOT NULL,
  PRIMARY KEY (origin_registry_id, origin_seq)
);
CREATE TABLE registry_imports (...);        -- §8.2 第 4 點
CREATE TABLE family_conflicts (...);        -- §8.2 第 2 點；存在任一列即隔離該家族
CREATE TABLE registry_conflicts (...);      -- §8.2 第 2 點；檢查點分歧
```

所有表以 `BEFORE UPDATE`／`BEFORE DELETE` 觸發器 `RAISE(ABORT)`，唯一例外是 `trial_families.protocol_json`
由 NULL 設為非 NULL 的單次轉換。工作區側：`research_attempts` 追加可為 NULL 的 `trial_event_id`
（新工作區 migration `0009`），P12b 之後建立的 attempt 必須非 NULL（由 runner 的交易檢查保證，舊列允許 NULL
直到回填）。

---

## 11. Rust 介面草案（純後端）

```rust
pub struct TrialLedger { /* registry connection */ }
pub struct BatchRegistration {
    pub batch_id: String,
    pub event_ids: Vec<String>,
    /// Current count, read in the same transaction as the registration or verified replay (§6.4).
    pub admission_count: AdmissionCount,
    /// Historical audit only (§6.3); never an input to a precision plan.
    pub registration_receipt: RegistrationReceipt,
}
/// Fields are private with read-only getters: only the ledger constructs one,
/// so a caller cannot assemble a count from a receipt or from UI/CLI input.
pub struct AdmissionCount {
    registry_id: String,
    family_id: String,
    family_effective_trials: u64,
    batch_effective_trials: u64,
    tests_per_trial: u64,
    seq: u64,
    chain_head: String,
}
pub struct RegistrationReceipt {
    pub receipt_registry_id: String,
    pub family_effective_before: u64, // deliberately not named prior_trials
    pub batch_effective_trials: u64,
}
impl TrialLedger {
    pub fn open(registry_dir: &Path, workspace_dir: &Path) -> Result<Self, LedgerError>;
    pub fn register_batch(&self, batch: &TrialBatch) -> Result<BatchRegistration, LedgerError>;
    pub fn read_admission_count(&self, batch_id: &str) -> Result<AdmissionCount, LedgerError>;
    pub fn export(&self, out: &Path) -> Result<ExportSummary, LedgerError>;
    pub fn import(&self, file: &Path) -> Result<ImportReport, LedgerError>;
}
pub fn check_binding(ledger: &TrialLedger, binding: Option<&LedgerBinding>) -> BindingStatus;
/// The only ledger-to-P12a path. Takes a current `AdmissionCount`; a `RegistrationReceipt` cannot be passed.
pub fn precision_plan_from_count(count: &AdmissionCount, sampling: &SamplingPlan) -> PrecisionPlan;
```

- 不新增前端命令。P12d 以 `register_batch` 回傳的 `admission_count`（或之後以 `read_admission_count` 重讀的值）
  經 `precision_plan_from_count` 組成 P12a plan；歷史收據在型別上無法傳入（R5）。
- 匯出／匯入先只提供 service CLI 子命令（與 P07 `fetch` 同一模式），UI 留給 P21。

---

## 12. 驗收案例（P12b 實作必須全部以隔離 registry＋工作區測試）

| # | 情境 | 期望 |
| --- | --- | --- |
| A1 | 新工作區（新 `AFF_DATA_DIR`）對同一 instrument 登記 | 計數延續既有家族，不歸零 |
| A2 | 同 instrument 改 interval／日期範圍／來源／重新匯入 dataset（hash 不同） | 同一家族，計數延續 |
| A3 | 同一冪等鍵重試登記（含同一 requestId 重試入隊） | 回傳相同 eventId 與相同歷史收據，不增加計數；`admissionCount` 為重試當下的計數 |
| A4 | 同一冪等鍵改 `kind`（例如 variant → diagnostic／benchmark） | `idempotency_conflict`，無寫入 |
| A5 | 不在白名單或參數不同的 benchmark | 拒絕為 benchmark |
| A6 | reproduction 任一識別欄位不同 | `reproduction_mismatch` |
| A7 | 失敗、取消、孤兒登記 | 全部仍計入 |
| A8 | 兩個程序同時 `register_batch` 同一家族 | 兩批皆成功或以 busy 重試成功；最終計數 = 兩批和；兩張收據的 `familyEffectiveBefore` 依交易順序銜接、不重疊；後完成者的 `admissionCount` 含兩批 |
| A9 | registry COMMIT 後、工作區寫入前崩潰 | 孤兒登記計入；重啟報告；重試入隊取回同事件 |
| A10 | 未登記的 attempt 嘗試 claim | `trial_not_registered`，rollback |
| A11 | 刪除 registry 檔後開啟已綁定工作區 | `registry_missing`；不自動新建取代 |
| A12 | 以舊 registry 檔取代 | 依情況 `registry_rolled_back`、`registry_diverged` 或 `registry_replaced`；皆停止資格判定 |
| A13 | 重複匯入同一匯出檔 | 無新增、計數不變 |
| A14 | 兩份 registry 各自新增試驗後互相匯入 | 聯集；計數 = 去重後總數，不是較大值 |
| A15 | 匯入含相同 idempotencyKey、不同 payload 的事件 | 家族隔離，資格判定停止 |
| A16 | 匯出檔被改一個位元組 | `import_integrity_failed`，無部分寫入 |
| A17 | 家族第二批使用不同 `testsPerTrial` | `family_protocol_mismatch` |
| A18 | 無 snapshot 的 legacy dataset | `family_unknown`，不能取得資格 |
| A19 | registry 目錄設在工作區內（或反之） | 開啟被拒 |
| A20 | P12a 串接 | 依 §5 組出的 plan 對 AlphaBTC 等價計數得 `146/1001` 且 `NOT_ELIGIBLE` |
| A21 | 在 `foreign_keys = ON` 下，把匯出檔匯入全新 registry；之後以同一批重試登記 | 匯入成功，無合成列；重試回傳來源的歷史收據（數值不變），計數不變；`admissionCount` 為匯入後的目前計數 |
| A22 | 工作區看過 A、B；以只含 A 的舊副本（同 `registryId`）再加入 C 後開啟 | 計數相同，但 `registry_diverged` |
| A23 | 更換成新 registry：匯入了等量事件，卻缺少工作區看過的某一筆 | 檢查點鏈值不符，`registry_replaced` |
| A24 | 更換成新 registry：匯入含舊 registry 至綁定 seq 的完整匯出 | 接受並改綁；計數延續 |
| A25 | 匯出／匯入後，reproduction 的 `seedsHash` 或 `splitHash` 與原事件不同 | 匯入時 `import_integrity_failed`；直接登記時 `reproduction_mismatch` |
| A26 | 匯出檔中 benchmark 事件的 `benchmarkId` 不在白名單或參數雜湊不符 | `import_integrity_failed` |
| A27 | 整批重送：冪等鍵全相同，但其中一筆的 strategy hash 或 kind 改變 | `idempotency_conflict`，不回傳舊收據、無寫入 |
| A28 | 與既有批次部分重疊的新批次 | `batch_conflict`，無寫入 |
| A29 | 同一登記在兩個 registry 各自寫入後聯集 | 同一 `eventId`，只計一次；兩張收據都保留 |
| A30 | registry 檔中某筆 `chain` 被改動 | 開啟時 `registry_chain_broken` |
| A31 | 家族原為 0；批次 A（1 筆有效）登記後、工作區寫入與 admission 前崩潰；批次 B 新增 10 筆；以同一 requestId 重試 A。P12a 參數：alpha 50,000 ppm、SE 200,000 ppm、`testsPerTrial` 2、B = 975、上限 100,000 | 重試回傳相同事件與歷史收據（`familyEffectiveBefore` 0、`batchEffectiveTrials` 1）；`admissionCount` = 家族 11、本批 1。plan 為 prior 10、planned 1（m = 22）→ `NOT_ELIGIBLE`（precision 需 10,975）。若誤用收據（m = 2）會得到 `ELIGIBLE`——測試必須證明 plan 只由 `admissionCount` 組成 |
| A32 | 已保存的 `ELIGIBLE` 判定，快照為家族 11（m = 22，B = 10,975，其餘同 A31）；派送前批次 C 又新增 5 筆 | 重讀：鏈前綴成立、家族 16 > 11 → 判定過時；以 m = 32 重算為 `NOT_ELIGIBLE`（precision 需 15,975）→ 不建立可執行的確認工作。若期間無新增（仍為 11）→ 原判定沿用 |

---

## 13. 實作切分建議

| 子項 | 範圍 | 不含 |
| --- | --- | --- |
| P12b-1 | registry 模組、schema、`register_batch`（含同交易 `admissionCount`）、`read_admission_count`、歷史收據、雜湊鏈、計數、冪等、衝突、匯出／匯入、包含檢查、`precision_plan_from_count`；A3–A6、A8、A13–A17、A19、A21、A25–A31 | runner 接線、工作區 migration |
| P12b-2 | 工作區 migration `0009`（`trial_event_id`、綁定鏈頭）、runner 入隊先登記、claim 檢查、回填、§7 偵測；A1、A2、A7、A9–A12、A18、A22–A24 | admission 阻擋（P12d） |

P12d 再用 A20 串接 P12a、用 A32 驗收 §6.4 的新鮮度圍欄，並讓預檢失敗時不建立可執行的確認工作。
A31 的 registry 部分（重試回傳目前計數、plan 只能由 `admissionCount` 組成）屬 P12b-1；「崩潰」以
「已登記、未寫工作區」的狀態模擬即可。

---

## 14. 不在範圍與已知限制

- 統計檢定、alpha 分配與噪音模擬（P12e-1..3）；確認批次、alpha 原子預約／消耗、Validation／Test 揭露（P13）。
- 不防竄改：本機使用者可刪除或一致回退所有副本（§7）。§7 的雜湊鏈能偵測回退、分歧與不一致的修改，
  但能改檔的人可以重算整條鏈；它是包含證據，不是簽章。
- v1 家族不處理跨場所同標的與多 instrument 試驗（§3）。
- 不遷移 AlphaBTC 紀錄。

---

## 15. v1 政策決定（2026-09-23 覆驗）

1. **家族粒度**：接受單一標準化 instrument 為家族；interval、期間、dataset、工作區及 protocol 不進家族鍵。
   保證範圍明確限於同一 instrument，不宣稱涵蓋同一經濟標的的所有場所。
2. **跨場所別名**：v1 不加入 alias 表；留待具備經濟等價判準、既有家族合併與 protocol 衝突規則的後續版本。
   v1 不支援跨場所／多 instrument 的聯合確認，不得用隱含別名取得資格。
3. **diagnostic**：v1 一律計入；尚無證據機制能證明結果未參與候選選擇，不提供豁免。
4. **pre-P05 歷史**：保留 `legacy_trials_unknown`，停止受影響家族的資格判定；不接受人工估計值當作已驗證計數。
5. **隔離解除**：v1 不提供人工解除；衝突家族持續不可取得資格，待後續版本定義留下完整紀錄的解除流程。
6. **registry 路徑**：接受 `%LOCALAPPDATA%\com.alphafactorforge.evidence\`。工作區移轉後 registry 缺失時維持不合格，
   直到經已驗證的事件匯出／匯入還原證據。

以上決定不代表本規格已完成驗收；[PR #116 覆驗](../handoffs/2026-09-23-pr116-acceptance-review-v1.md) 的 R5 仍待修正。
（更新：R5 已依 §5、§6.3、§6.4 修正並加入 A31–A32，待再次覆驗。）
