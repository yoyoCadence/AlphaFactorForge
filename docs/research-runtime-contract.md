# Research runtime contract v1（`research-command-v1` / `research-event-v1` / `ownership-lease-v1`）

> 由 P00（契約與相容性預檢，2026-09-16）登記。上游規劃：
> [`plans/active-plan.md`](plans/active-plan.md) §3.1–§3.3；需求來源：
> [`../handoffs/2026-09-15-alphabtc-capability-transfer-v1.md`](../handoffs/2026-09-15-alphabtc-capability-transfer-v1.md)
> §4（ABC-01）。本機驗證證據與可用／不可用能力矩陣：
> [`autonomous-research-capability-registry.md`](autonomous-research-capability-registry.md)。

**狀態：契約已定案；P02（runtime 解耦）、P03a（§1 lease、§5.2 schema 保護）、P03b
（§2 命令 envelope 與冪等、§3 事件帳本）、P04a（§4 控制介面與 service 宿主，`control-endpoint-v1`）
已於 2026-09-17 實作；桌面 connect 模式待 P04b。** 本文件定義後續 phase 必須遵守的
邊界與識別；P00 不新增程式、migration 或依賴。任何實作 phase 若需偏離本文，先修訂
本文並提升版本，不得在程式內默默改變語意。

---

## 0. 不變的既有契約

本契約是**附加層**，以下既有行為不變：

| 既有契約 | 位置 | 本契約的關係 |
| --- | --- | --- |
| desktop single-instance（RUNNER-OWNERSHIP-001） | `src-tauri/src/single_instance.rs`，PR #103 native smoke lane | 保留為 OS 層第一道 guard，只防兩個桌面程序；本契約的 lease 是 DB 層第二道，跨宿主種類 |
| 嵌入模式啟動 recovery | `src-tauri/src/main.rs` `setup` → `runtime::open_workspace`（`db::open_at` → migration → `DiscoveryRunner::recover_orphans`；P02 起） | 桌面在沒有既存有效 lease 時仍走此路徑並取得 lease；smoke lane 斷言不變 |
| `discovery-event-v1` | `src-tauri/src/discovery_runner/mod.rs`、`src/tauri-client/events.ts`、雙語 fixture | 事件 payload 與「每個狀態區塊獨立排序」行為不改；`research-event-v1` 只包一層 envelope |
| `discovery-config-v1` 與其 12 個 pinned 版本 | `src/services/discoveryRunConfig.ts` | 不改寫；新市場語意用新版本識別（見 [`market-contract.md`](market-contract.md)） |
| migrations 0001–0003 | `src-tauri/migrations/` | 原文不動；本契約需要的表以 0004+ 新增序號加入（實作 phase 才加） |
| 一個 transaction 的候選提交、單一 SQLite writer（RUNNER-STORE/EXEC-001） | `src-tauri/src/db/discovery.rs`、`discovery_runner/execution.rs` | ownership epoch 檢查加在同一 transaction 內，不拆開 |

---

## 1. 宿主模式與所有權模型（`ownership-lease-v1`）

### 1.1 宿主種類

| 宿主 | 說明 | 可否持有 lease |
| --- | --- | --- |
| desktop-embedded | 今天的 `main.rs`：Tauri 程序內建 runner | 可 |
| desktop-connect | 桌面偵測到既存有效 lease，只做代理，不跑 migration／recovery | 否 |
| service | headless service binary `alpha-factor-forge-service`（P04a，`runtime/service.rs`） | 可 |
| mcp-adapter | VS Code stdio adapter（P16），透過控制介面連接 service | 否 |

同一時間一個 workspace 只有一個 lease holder；**只有 holder 可執行 migration、
orphan recovery、runner 寫入**。recovery 權限跟隨 lease，不跟隨程序種類。

### 1.2 取得 lease 的順序（固定）

1. **OS 排他檔案鎖**：對 workspace 資料目錄內的 `ownership.lock` 取得排他鎖。
   失敗即進入 connect 模式或退出；不得重試搶占。此步驟**先於**開啟 SQLite 與
   migration。
2. 開啟 SQLite；設定 `busy_timeout = 5000`（毫秒，預設）。P02 起 `db::open_at` 已設定
   （`db::BUSY_TIMEOUT`）。
3. 執行 migration（只有 holder）。
4. 寫入 ownership row：`epoch = 前一 epoch + 1`、`holder_kind`、`holder_instance_id`
   （隨機 128-bit）、`pid`、`heartbeat_at`。
5. 執行 orphan recovery（沿用 `recover_orphans`）。
6. 開始 heartbeat。

### 1.3 Epoch 與工作提交

- 每個 runner 寫入 transaction 必須帶目前 `epoch`，並在同一 transaction 內以
  `WHERE epoch = ?` 檢查；不符即 rollback，並以 `StaleOwner` 錯誤結束該 worker。
  P03a review 修正（2026-09-17）：所有 runner store 寫入明確接收 epoch，經
  `ownership::write_transaction` 先 `BEGIN IMMEDIATE`、再於 transaction 內比較 epoch。
  範圍含 claim、strategy／run 建立、progress、狀態轉移、cancel／fail／complete、recovery
  與候選 assessment。transaction 外的 preflight 僅供提早拒絕；Rust mutex 不代替此防護。
- 舊 worker（前一 epoch 的 CPU 工作）回報結果時因 epoch 不符被拒，**不得**寫入
  summaries／trades／validation records。
- 休眠喚醒、時鐘跳動後，holder 先重新確認自己仍持有 OS 鎖與 DB 內 epoch，才繼續
  提交；任一不符則自行降級為 connect 模式並停止 workers。

### 1.4 Heartbeat 與失聯

| 參數 | 值 | 依據 |
| --- | --- | --- |
| heartbeat 週期 | 5 秒 | 計畫 §3.2 |
| 失聯判定 | 30 秒未更新 | 計畫 §3.2；6 個週期的容錯 |
| 時鐘來源 | 單調時鐘計算間隔；wall clock 只作顯示 | 休眠／時鐘跳動不得誤判 |

**heartbeat 過期只能把 UI 狀態標為「服務失聯」，不能單獨授權另一宿主搶占仍持有
OS 鎖的程序。** 搶占的唯一途徑是原程序釋放或作業系統回收 OS 鎖。

### 1.5 背景模式切換

桌面啟用「關閉 UI 後繼續」時，順序固定：停止接受新工作 → 完成 checkpoint →
釋放 lease（含 OS 鎖）→ 啟動 service → 桌面轉為 connect 模式。任一步失敗即回滾到
嵌入模式並告知使用者；不得出現雙寫。

---

## 2. 命令 envelope（`research-command-v1`）

所有跨宿主命令（桌面 bridge、MCP adapter、未來 CLI）使用同一 envelope：

```json
{
  "protocolVersion": "research-command-v1",
  "workspaceId": "<workspace 資料目錄的穩定識別>",
  "requestId": "<呼叫端產生的 UUID v4>",
  "command": "discovery.start",
  "payload": { }
}
```

| 欄位 | 規則 |
| --- | --- |
| `protocolVersion` | 必須完全等於已知版本；不同版本一律拒絕（`UnsupportedProtocol`），不做寬鬆相容 |
| `workspaceId` | 與 service 目前 workspace 不符即拒絕（`WorkspaceMismatch`） |
| `requestId` | 冪等鍵。同一 `requestId` 重送回傳**第一次**的結果；不重複建立 run／account／invocation。保存期限至少 24 小時。實作（P03b）：先預約（`pending`）再執行；命令的**不可變第一次結果**寫在決定它的 store transaction 內（`request_outcomes`：`begun` → `accepted`＋結果／`rejected`＋錯誤訊息），回條失敗時重送逐字重播該列、不看 run 之後的狀態、不重做；只有 `begun` 的列重播「未完成 admission」；同一 id 的重疊重送在同程序內以 per-request claim 串行化（第二個等第一個完成後重播，不重做）；已記錄的失敗為該 id 的終局（`retryable=false`），`retryable=true` 只代表 pending 或「結果未能記錄」 |
| `command` | 白名單字串，`<domain>.<verb>`；未知命令拒絕 |
| `payload` | 命令專屬；沿用既有 typed 契約（例如 `discovery.start` 的 payload 就是 `discovery-config-v1` envelope） |

明確**不提供**的命令：任意 shell、任意 SQL、任意檔案路徑讀寫、實盤下單。

錯誤回應為結構化物件 `{ code, message, retryable }`，`code` 為固定字串集合，
包含至少：`UnsupportedProtocol`、`WorkspaceMismatch`、`Unauthorized`、
`NotOwner`、`StaleOwner`、`DuplicateRequest`、`Validation`、`NotFound`、`Busy`。

---

## 3. 事件 envelope（`research-event-v1`）

```json
{
  "protocolVersion": "research-event-v1",
  "eventId": 123456,
  "epoch": 7,
  "entity": { "kind": "discovery_run", "id": "42" },
  "eventVersion": "discovery-event-v1",
  "committedAt": "2026-09-16T00:00:00Z",
  "payload": { }
}
```

- `eventId`：DB 內單調遞增的持久序號（跨重啟全域唯一）；今天 runner 的程序內
  `sequence` 不能冒充它，兩者並存，`payload` 內保留原 sequence。
- 事件**只在 DB commit 後**發布（沿用 commit-then-emit）。
- `stateVersion`（P03b R2）：每個 domain 寫入 transaction 遞增的持久版本（`runtime_state.mutation_seq`）；
  snapshot 與 `events.read` 都回傳。讀者規則：空頁面但版本前進、或 `ledgerGap` 標記在 snapshot 之後 →
  必須重讀 snapshot。帳本 append 失敗不會抹掉已提交結果，也不會讓只追 cursor 的讀者永久漏讀。
- 重連流程固定：先讀 snapshot（等同今天 `get_active_discovery_run` /
  `get_discovery_progress`），再以 `afterEventId` cursor 續接；漏失事件不能抹掉
  已提交結果。
- 每個狀態區塊獨立排序的 feed 行為（`src/services/discoveryFeed.ts`）不變。

---

## 4. 本機控制介面（P04a 實作，`control-endpoint-v1`）

| 項目 | 規則 |
| --- | --- |
| 綁定 | 僅 `127.0.0.1`，動態 port（`runtime/control_api.rs` `ControlServer::bind`） |
| endpoint manifest | 寫在 workspace 的本機應用資料目錄（非 OneDrive），檔名 `control-endpoint.json`：`manifestVersion`、`port`、`workspaceId`、`epoch`、`holderKind`、`instanceId`、`pid`、`serviceVersion`、`startedAt`；先寫暫存檔再原子更名；Unix 0600、Windows 承襲使用者 profile 目錄 ACL；讀者使用前必須以 `/v1/info` 核對 `instanceId`（崩潰的 service 會留下過期 manifest） |
| 認證 | 隨機控制 token（`getrandom` 32 bytes → 64 hex），檔名 `control-token`，與 manifest 同目錄、同權限、不寫進 manifest；請求以 `Authorization: Bearer <token>` 攜帶；比對 constant-time；失敗 401 |
| 請求檢查 | `Host` 必須是 `127.0.0.1` 或 `localhost`（若帶 port 須等於本 port），否則 403；缺 `Host` 400；含 `Origin` 的請求一律 403（瀏覽器來源不得直連）；以上皆先於認證與路由 |
| 傳輸 | HTTP/1.1 JSON，每連線一個請求、`Connection: close`；body 必須帶 `Content-Length`（≤ 1 MiB，不接受 chunked，違者 411／413）；request head ≤ 16 KiB（431） |
| 路由 | `GET /v1/info`（身分與協定版本）；`POST /v1/commands`（body 為 §2 envelope；成功 200 `{"result"}`，拒絕依 code 對應 400／401／404／409／503 並回 `{"error": CommandError}`）；`GET /v1/events?afterEventId=&limit=&waitMs=`（§3 `events.read` 同一頁面形狀，**long-poll**：無新事件時最多等 `waitMs`（上限 30 s），帳本增長、`stateVersion` 前進或開始關閉即返回；SSE 不採用）；`POST /v1/shutdown`（202，見 §4.1） |
| token 使用者 | Tauri backend（P04b）、MCP adapter（P16）、`service stop`／`status`；**不傳給前端 WebView**，不寫進 SQLite／logs／回應 |
| 不提供 | shell、SQL、檔案路徑、實盤端點；不綁定 `0.0.0.0`／IPv6 |

### 4.1 service 宿主生命週期（`runtime/service.rs`）

1. `run`：以 §1.2 順序取得 lease（host kind `service`）→ 建 §2 dispatcher（host sink 只喚醒 long-poll 讀者）→ 綁定 port → 發布 manifest 與 token → 服務。
2. `POST /v1/shutdown`（或 `stop` 子命令）：立即拒絕 mutating 命令（503 `Busy`、`retryable=true`，因為從未預約，同 requestId 可交給下一個 owner），讀取命令照常；對每個活著的 coordinator 發 pause，等待其在**本 epoch** 提交 Paused checkpoint 並退出（上限 60 s）；停止監聽；**先撤下 manifest／token、再釋放 lease**（避免刪到下一個 owner 剛發布的檔案）。
3. 沒有 signal handler：Ctrl+C／kill 等同崩潰，由下一個 owner 的啟動 recovery 處理（run 變 paused）；留下的 manifest 因 `/v1/info` 不回應或 `instanceId` 不符而被 `stop`／`status` 判為過期，不會被誤用。
4. 預設資料目錄 = `dirs::data_dir()/com.alphafactorforge.desktop`（與 tauri `app_data_dir` 同一解析），`--data-dir` 可覆寫（隔離工作區、測試）。

`codex app-server` 自身的 `--listen ws://` 與 `--ws-auth` 屬於 Codex 程序，不是
本控制介面；本專案不對外暴露 Codex 的端點（見
[`ai-provider-contract.md`](ai-provider-contract.md)）。

---

## 5. 儲存與相容性規則

1. 使用者 DB 位置不變（`<app_data_dir>/alphafactorforge.sqlite3`）；新 artifacts
   存在其相鄰、非 OneDrive 的本機資料目錄。
2. 新表以 0004+ migration 加入；升級前先做一致備份，失敗回滾；**舊 binary 遇到較新
   `schema_migrations` 必須拒絕寫入**（今天沒有這個檢查，P03 加入）。
3. 舊 summaries／trades 保留為「最新結果投影」；新研究結果以 attempt ID 保存完整
   不可變 artifacts（P05）。不宣稱能恢復過去已被覆寫的交易明細。
4. artifacts 先 staging → 校驗 → 原子更名 → 才提交 DB 參照；已被引用的檔案不得
   悄悄刪除。
5. 秘密不進 SQLite、artifacts、前端或 logs。

---

## 6. 實作 phase 對照

| Phase | 使用本契約的部分 | 必測情境（來自計畫 §6） |
| --- | --- | --- |
| P02（完成 2026-09-17） | §0 邊界、busy_timeout、DB path 注入、event sink 抽離 — `db::open_at`、`runtime::open_workspace`、`desktop::discovery_events`、`runtime::boundary_tests` | 既有 golden／runner 原子性不變（runner 測試檔未動） |
| P03a（完成 2026-09-17） | §1 lease（`runtime/lease.rs`、migration 0004、`db/ownership.rs`、runner epoch 檢查）、§5.2 schema 保護（`SchemaTooNew`） | 雙啟、owner crash／接手、休眠（heartbeat 停止不釋放鎖）、時鐘變動（以 `heartbeat_seq` 與讀者單調時間判定）、舊 worker 提交 — 皆有 Rust 測試 |
| P03b（完成 2026-09-17） | §2 `CommandEnvelope`／`CommandError`／白名單／reserve-then-complete 冪等（`runtime/commands.rs`、`db/runtime_ledger.rs`、migration 0005）、§3 `LedgerSink`＋`events.read`（snapshot → `afterEventId`） | 重複命令（同 requestId 三次只建一個 run、回放第一次結果）、payload 不同→`DuplicateRequest`、未完成→`Busy`、亂序／漏失（帳本依 eventId 分頁重讀）— 皆有 Rust 測試 |
| P04a（完成 2026-09-17） | §4 控制介面（`runtime/control_api.rs`、`control_client.rs`）、§4.1 service 宿主（`runtime/service.rs`、`service_main.rs`） | 關 UI 工作持續（啟動的 client 消失後 run 繼續）、重連採同一 run（`discovery.active` → `events.read` cursor → long-poll 被 runner 事件喚醒）、關閉時 drain 到 Paused checkpoint 且下一 owner 無孤兒可 resume、雙啟 exit 2、真實 binary smoke — 皆有 Rust 測試 |
| P04b | 桌面 connect 模式（`NotOwner` → 讀 manifest → 走 §4）、事件轉送到視窗、§1.5 背景切換 | 嵌入模式 native smoke 保留、桌面重連採同一 run |
| P16 | §2 命令白名單（MCP 工具對照） | 不能揭露 Test、不能改閘門 |
