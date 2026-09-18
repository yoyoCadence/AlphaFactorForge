# AI provider contract v1（`ai-invocation-v1`）— Codex／ChatGPT 訂閱

> 由 P00（契約與相容性預檢，2026-09-16）登記。上游規劃：
> [`plans/active-plan.md`](plans/active-plan.md) §4.2–§4.4；既有邊界：
> `AGENTS.md` §0（AI 只能產生經驗證的 JSON Strategy DSL；code mode 永遠人工）、
> [`../STRATEGY_DISCOVERY.md`](../STRATEGY_DISCOVERY.md) §3。本機驗證證據：
> [`autonomous-research-capability-registry.md`](autonomous-research-capability-registry.md) §2。

**狀態：契約已定案，尚未實作。** 目前 `src-tauri/src/commands/ai_commands.rs` 與
`secret_commands.rs` 仍回傳 `AppError::NotImplemented`，且是以「API key 存
keychain」為前提寫的 stub；本契約把第一個 provider 改為 **Codex 訂閱**，API key
路徑保留給日後其他 provider，不在此版啟用。

---

## 1. 產品決策（凍結）

| 項目 | 決策 |
| --- | --- |
| 第一個 provider | Codex／ChatGPT 訂閱，經本機 `codex app-server`（stdio） |
| 登入 | 官方 ChatGPT 登入流程，憑證由 Codex 自己管理（`CODEX_HOME/auth.json`）；**不**把訂閱轉成 API key、不複製認證檔、不讀取 token 內容 |
| 自動化邊界 | AI 只提案 JSON DSL；不能批准自己、執行 SQL、取得保留資料、修改閘門或改寫產品程式 |
| 額度政策 | 不自動購買額度、不切換付費 API、不輪替帳號 |
| 資料外送 | 預設只送必要的**開發資料摘要**；原始行情是否可送第三方依來源授權逐一決定 |

---

## 2. 本機已驗證的協定能力（codex-cli 0.140.0，2026-09-16）

以下由 P00 以 stdio 握手實測（`initialize` → `initialized` → 不耗模型額度的讀取
命令），細節見 registry §2：

| 能力 | 方法 | 狀態 |
| --- | --- | --- |
| 初始化 | `initialize` / `initialized` | 可用 |
| 帳戶與登入模式 | `account/read`（回 `type: chatgpt`、`planType`） | 可用 |
| 額度視窗 | `account/rateLimits/read`（primary 5h／secondary 7d 視窗、`usedPercent`、`resetsAt`、credits） | 可用 |
| 模型列表 | `model/list`（`includeHidden` 可選；本機只回傳 1 個模型） | 可用，**名單與全域設定的預設模型不一致**（見 §6） |
| 生效設定 | `config/read`（含 `mcp_servers`、`sandbox_mode`、`approval_policy`、`web_search`） | 可用 |
| 對話與 turn | `thread/start`（`ephemeral`、`sandbox`、`approvalPolicy`、`config` 覆寫、`baseInstructions`／`developerInstructions`、`model`）、`turn/start`（`outputSchema`、`sandboxPolicy`、`effort`）、`turn/interrupt` | schema 存在；**尚未以真實 turn 驗證**（留給 P15 的一次有界生成） |
| 結構化輸出 | `turn/start.outputSchema`（JSON Schema 約束最終訊息） | schema 存在，未實測 |
| 登入流程 | `account/login/start`／`cancel`、`account/logout`、通知 `account/login/completed` | schema 存在，未實測 |
| 結束狀態 | `turn/completed` 的 `TurnStatus ∈ {completed, interrupted, failed, inProgress}` | schema 存在 |
| Server→client 請求 | `item/commandExecution/requestApproval`、`item/fileChange/requestApproval`、`item/tool/call`… | 存在；本契約**一律拒絕**（見 §3） |

`codex mcp-server`（Codex 作為 MCP server）與 `codex app-server` 皆為 experimental
子命令；`codex doctor` 顯示 0.154.0 可更新。P15 開始前必須重新執行 registry 的
版本／能力檢查，**不假設本文列出的方法在較新版本仍同名**。

---

## 3. 受限生成環境（每次呼叫）

| 項目 | 要求 | 對應機制 |
| --- | --- | --- |
| 工作目錄 | 專用空目錄（非 repo、非使用者 workspace），只含允許的摘要檔 | `thread/start.cwd` |
| 沙箱 | `read-only`；網路關閉 | `thread/start.sandbox = "read-only"`、`turn/start.sandboxPolicy = { type: "readOnly", networkAccess: false }` |
| 審批 | 不允許任何審批互動；收到任何 `*/requestApproval`、`item/tool/call`、`mcpServer/elicitation/request` 即回拒絕並中止 turn | `approvalPolicy = "never"` + adapter 端拒絕 |
| 工具 | 禁用 shell、外部工具、網路搜尋、子 agent、全域記憶、無關 MCP／plugins | `thread/start.config` 覆寫 `mcp_servers = {}`、`web_search` 關閉；**覆寫是否確實生效須於 P15 以 `config/read`＋實際 turn 驗證** |
| 隔離不可驗證時 | **禁止 unattended 模式**；只允許有人在場的單次生成 | registry 阻擋項 |
| 不改全域設定 | 不寫 `~/.codex/config.toml`；只用 per-thread 覆寫或 `-c` 參數 | — |
| 模型 | 由設定畫面從 `model/list` 實際結果選；campaign 凍結實際模型 ID | — |

已知限制：本機全域 config 有 2 個 MCP server 與 29 個 feature flag（`codex doctor`）。
若 per-thread 覆寫無法清空它們，替代方案是獨立 `CODEX_HOME`——但那會失去登入
（auth 在 `CODEX_HOME` 內）且本契約禁止複製認證檔；此時 unattended 功能保持阻擋，
只能人工在場生成。

---

## 4. 狀態機

| 狀態 | 進入條件 | 離開條件 |
| --- | --- | --- |
| `Ready` | `account/read` 回 chatgpt 帳戶且 `rateLimitReachedType == null` | — |
| `WaitingQuota` | `rateLimitReachedType != null`，或 turn 因額度失敗 | 到達 `resetsAt` 後重新讀取額度；**不**自動購買、不切 API |
| `AuthRequired` | `account/read` 無帳戶、或 turn 回登入失效 | 使用者完成官方登入流程；研究排程暫停但 paper 帳戶不受影響 |
| `UnknownOutcome` | 送出 turn 後，在收到 `turn/completed` 前程序中斷／逾時／連線遺失 | 人工或下次啟動核對；**計入預算、不盲目重送** |
| `Blocked` | 隔離／版本檢查不通過 | 重新通過 registry 檢查 |

---

## 5. 預算與上限（可凍結設定，預設值）

| 項目 | 預設 | 備註 |
| --- | --- | --- |
| 單次生成提案數 | ≤ 4 | 超出部分丟棄並記錄 |
| 單日呼叫數 | ≤ 16 | 含失敗、格式錯誤、`UnknownOutcome` |
| 單日候選數 | ≤ 64 | 進入 runner 前計數 |
| 單次呼叫逾時 | 5 分鐘 | 逾時→`turn/interrupt`→`UnknownOutcome` |
| 探索比例 | 60% 新機制／40% 深化 | campaign 凍結；不依 Validation／Test 表現自動調整 |
| 自動修復 | 無 | 格式錯誤不重試，計入預算 |

---

## 6. AI invocation 紀錄（`ai-invocation-v1`，P15 落地）

每次呼叫保存：`invocationId`、`campaignId`、`requestId`、實際模型 ID、
`promptHash`、允許上下文的 hash 與範圍描述（只含 Train 區間的摘要）、原始輸出
全文、DSL 驗證結果（`src/core/strategy-dsl/validator.ts` 的錯誤列表）、
token／時間用量、最終狀態（§4）、Codex 版本。原始輸出不可變；驗證失敗仍保存。

`ai_generations` 表（0001 migration）現為 schema-only；P15 決定是延用並以
0004+ 補欄位，或新表。不在 P00 改。

**發現的相容性問題（P15 前必須處理）**：本機 `config/read` 的預設 `model` 為
`gpt-6-astra`，但 `model/list` 只回傳 `gpt-5.5`，且 app-server 啟動時記錄
`failed to load models cache: missing field base_instructions`。campaign 凍結
模型 ID 前，必須以 `model/list` 的實際回傳為準，不能信任 config 預設值。

---

## 7. 資訊邊界（與 P12／P13 共同執行）

- 允許進入 prompt：Train 區間的績效摘要、失敗分類、假說與失效條件、參數敏感度
  摘要、regime 摘要（見計畫 §4.3）。
- 禁止進入 prompt：Validation／Test 數值、圖形、失敗原因，及據此產生的候選選擇；
  已揭露 Test 的任何衍生資訊。
- 自動測試（P13）必須證明 prompt 組裝函式在給定含 Validation／Test 欄位的輸入時
  拒絕或遮蔽，而不是靠呼叫端自律。

---

## 8. DSL 可執行子集（P11 的輸入）

`INDICATOR_WHITELIST` 有 24 個名稱，但**兩個核心都已實作**的只有：
`SMA`、`EMA`、`WMA`、`RSI`、`MACD`、`ATR`、`BBANDS`、`STDDEV`、`HIGHEST`、`LOWEST`、
`ROC`，加上價格來源 `CLOSE`／`OPEN`／`HIGH`／`LOW`／`HLC3`。
`ADX`、`STOCH`、`CCI`、`MOM`、`KELTNER`、`OBV`、`VOL_SMA`、`MFI` 目前兩邊都沒有實作。
JSON DSL 目前**沒有任何 runtime 消費者**（validator 只在單元測試中被呼叫；
`exprInterpreter.ts` 是 code mode 的字串運算式，不是 JSON DSL）。

因此 AI 白名單初版 = 上述交集；每個新增指標須先完成 TS／Rust parity 才加入。
深度 8／節點 64 的既有限制保留（`src/core/strategy-dsl/schema.ts`）。
