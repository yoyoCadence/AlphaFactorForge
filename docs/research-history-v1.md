# Research history v1（`hypothesis-v1` / `candidate-result-v1` / migration 0007）

> 由 P05（完整研究歷史，2026-09-19）登記。上游規劃：[`plans/active-plan.md`](plans/active-plan.md)
> §3.3「Hypothesis」「Research attempt」資料群組與儲存規則；需求來源
> [`../handoffs/2026-09-15-alphabtc-capability-transfer-v1.md`](../handoffs/2026-09-15-alphabtc-capability-transfer-v1.md)
> §4 ABC-05（假說與候選凍結紀錄）。實作：`src-tauri/src/research/`、`migrations/0007_research_history.sql`。

**狀態：已實作（P05）。** 本文件定義後續 phase（P11 DSL、P12 試驗帳本、P13 一次性驗證、P14 備份、
P15 AI 提案、P16 MCP、P21 UX）必須遵守的資料形狀與不變量；偏離先修訂本文並提升版本。

---

## 0. 與既有資料的關係（附加層）

| 既有資料 | 關係 |
| --- | --- |
| `backtest_summary`／`backtest_trades`（依 strategy＋dataset＋segment upsert） | 保留為**最新結果投影**：重跑仍覆寫投影，但每次嘗試的完整結果另存於不可變 artifact。**不宣稱能恢復 P05 之前已被覆寫的交易明細。** |
| `validation_records`（append-only） | 不變；attempt 的 `outcome.recordId` 指向它 |
| `discovery_runs`／`discovery_jobs` | 不變；attempt 與 job pair 一一對應（`attempt_key = run:<run>:candidate:<index>`），狀態在**同一 transaction** 內同步 |
| `strategy_def` | 不變；attempt.`strategy_id` 指向候選策略列，hypothesis.`strategy_hash` 是**基底**策略文件的 hash |

---

## 1. 資料表（migration `0007_research_history`）

### 1.1 `hypotheses` — 執行前凍結，永不修改

| 欄位 | 內容 |
| --- | --- |
| `hypothesis_hash` UNIQUE | SHA-256（canonical JSON，見 §3）over `content_json`：同內容 = 同一列（ABC-05「重複請求不新增」） |
| `version` | `hypothesis-v1` |
| `source` | `manual` \| `discovery` \| `ai` |
| `mechanism` | 為何可能有效（必填、非空） |
| `applicability_json` | 適用條件（資料集、interval、split、embargo、axes …） |
| `failure_modes` | 失效情境（必填、非空；discovery 自動登記時為明示的「未陳述」） |
| `strategy_hash` | 凍結的策略識別；discovery 為 `strategy-doc-v1:<sha256 of canonical base strategy JSON>` |
| `strategy_id`／`parent_strategy_id`／`variation_kind` | 血緣：父策略、變異方式（`param-sweep:<axes>`、`ai-proposal`、`manual`） |
| `content_json` | 完整文件（含 `version`） |

觸發器：`BEFORE UPDATE` / `BEFORE DELETE` 一律 `RAISE(ABORT)`。

### 1.2 `research_attempts` — 每個候選一筆，凍結欄位不變、狀態只前進

| 欄位 | 內容 |
| --- | --- |
| `attempt_key` UNIQUE | 冪等鍵；discovery 為 `run:<run_id>:candidate:<index>` |
| `hypothesis_id`／`strategy_id`／`dataset_id`／`discovery_run_id`／`candidate_index` | 凍結 |
| `status` | `submitted` → `running` → `completed` \| `failed` \| `skipped`；`running` → `submitted` 只在崩潰恢復把工作重新排隊時發生（同一嘗試，不是新嘗試） |
| `input_fingerprint_json` | `envelopeVersion`、`configHash`（raw config canonical hash）、`rootSeed`、`datasetId/Hash`、`interval`、`strategyId/Hash`、`baseId`、`candidateIndex`、`appliedAxes`、`seeds` |
| `engine_fingerprint_json` | `package`（Cargo 版本）、`configContracts`（config 宣告的全部契約版本）、`execution`、`metrics`、`benchmarks`、`gate`、`score`、`validationRecord`、`benchmarkRecord` |
| `outcome_json` | completed：`{recordId, trainSummaryId, validationSummaryId, gatePassed, score}`；failed：`{error}`；skipped：`{reason}` |
| `result_artifact_id` | completed 時指向 §1.3（宿主無 artifact store 時為 NULL，只有測試） |
| `epoch`、`submitted_at`、`finished_at` | 提交時的 ownership epoch；時間戳 |

觸發器 `research_attempts_are_frozen`：任何凍結欄位變更、或 `OLD.status` 已是終態時 `RAISE(ABORT)`；`BEFORE DELETE` 一律 ABORT。

狀態與 job 的對應（皆在 runner 的既有 transaction 內）：

| runner 事件 | job rows | attempt |
| --- | --- | --- |
| `start_discovery_run_with_lineage` | queued ×2 | hypothesis 登記＋`submitted`（與入隊**同一 transaction**） |
| `claim_candidate_jobs` | running | `running` |
| `commit_candidate_assessment_with_artifact` | done | artifact 列＋`completed` |
| `fail_discovery_run_with_outcomes` | failed（含未跑的） | `failed`（同一原因） |
| `cancel_discovery_run` | skipped | `skipped`（`run cancelled before this candidate ran`） |
| `recover_orphaned_runs` | running → queued | `running` → `submitted` |
| resume pre-0007 run | queued（paused → running） | 由既有 config／strategy／dataset 重建並凍結缺少的 `submitted`；已有 attempt 則驗證 identity、input 與 engine fingerprint，皆在同一 transaction |

Production claim 與 commit 都要求**恰好一筆**對應 attempt 隨 job 前進；缺少或狀態不符即讓整個 transaction rollback。只有 store 層的 `#[cfg(test)]` legacy wrappers 可刻意不建立 history。

### 1.3 `research_artifacts` — content-addressed，永不刪除

| 欄位 | 內容 |
| --- | --- |
| `sha256` UNIQUE | 檔案內容 hash（也是檔名） |
| `kind` | `candidate-result-v1` |
| `byte_len`、`relative_path` UNIQUE | `<sha256[0..2]>/<sha256>.json`，相對於 artifact 根目錄 |

觸發器：UPDATE／DELETE 一律 ABORT。

---

## 2. Artifact store（`research/artifacts.rs`）

- 根目錄 `<workspace data dir>/artifacts`（與 DB 相鄰、非 OneDrive；桌面與 service 同一解析）。
- 寫入固定順序：`staging/<sha>-<pid>.tmp` → fsync → 讀回校驗 hash → 原子 rename 到最終路徑 → **之後**才在 DB 提交參照
  （`commit_candidate_assessment_with_artifact` 與投影同一 transaction）。同內容重複寫入 = 同一參照，不動既有檔。
- 讀取：`read(reference)` 校驗 sha256 與 byte_len，不符即拒絕（`altered or truncated`），不回傳可疑內容。
- 沒有刪除 API。`unreferenced(referenced_paths)` 只**列出**未被 DB 參照的檔案（含 staging 殘留）；桌面命令
  `list_unreferenced_artifacts` 暴露此清單。
- 路徑守衛：相對路徑不得含 `..`、絕對路徑或空字串。
- 結果不能不可變保存時（staging／rename 失敗），該候選**不提交**（run 以該原因失敗），不留半成品。

### `candidate-result-v1` 文件

```json
{
  "version": "candidate-result-v1",
  "attemptKey": "run:1:candidate:0",
  "runId": 1, "candidateIndex": 0,
  "strategy": { "id": 12, "hash": "strategy-v2:…", "definition": { … } },
  "dataset": { "id": 1, "hash": "dataset-content-v2:…", "interval": "1d" },
  "train": { "summary": { …backtest_summary 欄位… }, "trades": [ …TradeRow… ] },
  "validation": { "summary": { … }, "trades": [ … ] },
  "record": { …validation_records 列… },
  "digest": { "gatePassed": true, "score": 0.42 }
}
```

以 canonical JSON（§3）寫入；**Test 段不存在於此文件**（runner 從不執行 Test）。

---

## 3. Canonical JSON

`research::canonical_json`：物件鍵遞迴排序、無空白、字串以 serde_json 逸出、數字以 serde_json 原生格式。
用於 hypothesis hash 與 artifact 內容，因此同一文件在桌面／service／測試產生的 bytes 與 sha 相同，且讀回即是
一般 JSON。（`identity::canonical_bytes` 是既有識別 hash 的二進位編碼，不用於此。）

---

## 4. 讀取介面（桌面命令，兩種宿主模式皆可用）

| 命令 | 內容 |
| --- | --- |
| `list_research_attempts(filter?)` | `AttemptFilter { discoveryRunId, strategyId, datasetId, hypothesisId, limit ≤ 500 }`，新到舊 |
| `get_research_attempt(id)` | `{ attempt, hypothesis, result, resultError }`：`result` 為校驗通過的 `candidate-result-v1`；檔案缺失或被改動時 `resultError` 說明，列仍是證據 |
| `list_hypotheses(limit?)` | 新到舊 |
| `list_unreferenced_artifacts()` | §2 的清單 |

`research-command-v1` 白名單**未**加入這些讀取（P16 MCP 需要時再修訂契約 §2）。

---

## 5. 不變量與驗收對照（plan §5 P05）

| 驗收 | 證明 |
| --- | --- |
| 重跑不覆寫舊明細 | 同 config 跑兩次：投影列 id 相同（被覆寫），但兩筆 attempt、兩個 artifact（sha 不同，因 attemptKey／runId 不同）；第一個 artifact bytes 與第一次讀回完全相同、第一筆 attempt 列不變 — `a_rerun_replaces_the_projection_but_keeps_the_earlier_attempts_artifact` |
| 失敗可追溯 | 候選執行失敗：該候選與被 run 失敗帶走的候選各自 `failed` 並帶原因；取消 → `skipped` 帶原因；終態列不可再改 — `failed_and_cancelled_attempts_keep_their_reasons_and_stay_frozen` |
| 新舊投影一致 | 第二次跑完後投影欄位（net_return／trade_count／score）＝第二個 artifact 的 train summary；第一個 artifact ＝ 第一次跑完時的投影 — 同上測試 |
| 假說先於執行、不可改寫、重複不新增 | 首個候選被 claim 時假說與全部 attempt 已存在；同內容再登記回同 id；UPDATE／DELETE 被觸發器拒絕 — `a_run_freezes_its_hypothesis_…`、`a_hypothesis_registers_once_per_content_…` |
| 崩潰恢復不產生新嘗試 | `recovery_requeues_an_interrupted_attempt_as_the_same_attempt` |
| 舊 schema 未完成 run 不繞過歷史 | pre-0007 queued jobs 直接 production claim 會 rollback；resume 先在 paused → running transaction 補齊 lineage，完成後每個候選皆有 artifact — `a_pre_0007_paused_run_gets_lineage_before_resume_and_cannot_bypass_it`；production commit 缺 attempt 時 artifact／projection／job 全 rollback — `a_production_commit_without_an_attempt_rolls_back_every_write` |
| artifact 完整性 | 被改動／截斷的檔案讀取被拒；未參照檔可列出、不被刪 — `research::artifacts::tests` |

---

## 6. 已知限制

- discovery 自動登記的假說 `failure_modes` 為明示「未陳述」；有陳述失效情境的假說由 P15（AI 提案）／手動登記提供。
- 嘗試以候選為單位；未來 attempt 之間的父子血緣（`parent_strategy_id`）由 P11／P15 填入。
- 未提供匯出／備份（P14）；未提供全文索引（plan §4.3 經驗庫，P17）。
- P05 之前已完成、且明細已被投影覆寫的候選仍不可恢復；升級補建只適用於尚未執行的 queued 候選。若既有 attempt 的 engine fingerprint 與目前 build 不同，resume 拒絕並要求開新 run，不會把新引擎結果寫進舊 attempt。

## P12c-2b 增量 artifact 版本（2026-09-25）

既有 `candidate-result-v1` 格式與歷史 attempt 不變。`discovery-config-v3`
的 walk-forward run 改寫入 `candidate-result-v2`：保留原有 attempt、策略、
資料集、Train/Validation、record 與 digest 欄位，另加 `walkForward`，內含
`walk-forward-evidence-v1`。`research_artifacts.kind` 對應文件版本。

既有 `get_research_attempt` 讀取路徑會先核對 checksum 與長度，再回傳任一
版本；UI 使用的 Train/Validation 摘要欄位仍存在。完成的
`research_attempts.result_artifact_id` 在原有候選提交交易內連結不可變檔案與
attempt。本次無 migration，也不修改先前的 artifact。
