# Handoff: P00 契約與相容性預檢

Date: 2026-09-16
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（疊加於 `docs/alphabtc-capability-transfer`，因 `docs/plans/active-plan.md` 尚未在 `main`）
PR: 尚未建立（GitHub 認證失效，見 Verification）
Status: P00 完成；下一個可執行 phase 為 P01（Results Explorer），須另行明確授權

## Summary

執行 `docs/plans/active-plan.md` §5 的第一個 phase。依計畫規則「每次只執行一個 phase」與
「未指定 phase 時選第一個未完成且依賴滿足的 phase」，本次只做 P00：凍結 runtime／AI／
市場三份契約、核對本機 Codex 能力與必要依賴、登記現況與產品決策，並產出可用／不可用能力
與阻擋原因的 registry。**未啟動研究、未匯入正式資料、未新增程式／migration／依賴。**

## 執行前驗證（Plan 與 codebase 對照）

- Plan §5 列出的既有路徑全部存在；「planned new」的 `service_main.rs`、`runtime/`、
  `providers/`、`research/`、`paper/`、`mcp/`、`ResultsExplorer.tsx`、`ActivityCenter.tsx`
  皆確認不存在，與 Plan 一致。
- Plan §2 缺口逐項核實：`ai_commands.rs`／`secret_commands.rs` 回傳 `NotImplemented`；
  `TauriDiscoveryEventSink` 持 `AppHandle`（`discovery_runner/mod.rs:145`）；
  `db::initialize(&AppHandle)`；`barsPerYear('1d') = 365`；`db/mod.rs` 未設 `busy_timeout`；
  `apply_migrations` 不拒絕未知較新版本；JSON DSL validator 無 runtime 消費者。
- 與 `AGENTS.md` 無衝突：純 `src/core`／`discovery_core` 邊界、金鑰不進前端／SQLite、
  AI 只產 JSON DSL 皆由契約重申。AGENTS.md §0.1「discovery and AI tables are schema-only」
  對 discovery 已過時（runner 已合併），Plan §2 已註明不得據此忽略；本次未改 AGENTS.md。
- 與 `tasks.md` 既有排序：原文要求 ABC-01 在 Results Explorer 完成後才 promote；Plan 是
  維護者明確重排（P01 仍是 Results Explorer，P02＝ABC-02a 可跟隨），已在 tasks.md 註明。
- 小幅落差（不改 Objective／Scope／AC）：使用者指示提到 `task.md` 與「GitLab MR」，實際
  是根目錄 `tasks.md` 與 GitHub remote；照實際處理。

## 修改檔案

| 檔案 | 變更 |
| --- | --- |
| `docs/research-runtime-contract.md` | 新增：`research-command-v1`／`research-event-v1`／`ownership-lease-v1` |
| `docs/ai-provider-contract.md` | 新增：`ai-invocation-v1`，Codex app-server adapter 契約 |
| `docs/market-contract.md` | 新增：`market-instrument-v1`／`market-provenance-v1`／`market-snapshot-v1` |
| `docs/autonomous-research-capability-registry.md` | 新增：活的能力登記簿（可用／可規劃／阻擋／未驗證） |
| `tasks.md` | ABC 段落新增 P00–P22 phase 對照表與排序註記；Done 新增 P00 |
| `README.md` | 三語 Roadmap 各加一段：計畫、P00 完成、第一 provider 改為訂閱、未交付項目 |
| `STRATEGY_DISCOVERY.md` | §3 加一段補充：第一 provider 改為 Codex 訂閱，指向契約與 registry |
| `handoffs/2026-09-16-p00-contract-precheck-v1.md` | 本文件 |

Migration／版本影響：無。CHANGELOG 未加（無 runtime 變更，與先前純文件提交一致）。

## 本機預檢證據（不耗模型額度）

| 檢查 | 結果 |
| --- | --- |
| `codex --version` | `codex-cli 0.140.0`（`codex doctor`：0.154.0 可更新） |
| `codex login status` | Logged in using ChatGPT |
| app-server stdio 握手 | `initialize` OK；`account/read` → `chatgpt`／`plus`；`account/rateLimits/read` → 5h／7d 視窗各 68%、無 credits；`model/list` → 只有 `gpt-5.5`；`config/read` → 預設 `gpt-6-astra`、`workspace-write`、`on-request`、2 個 MCP server |
| app-server 啟動日誌 | `failed to load models cache: missing field base_instructions`（版本漂移訊號，登記為 P15 前置） |
| `codex app-server generate-json-schema` | 261 個 schema 檔產生於 scratchpad；**未入 repo**（P15 再決定是否 vendored） |
| 工具鏈 | Rust／Cargo 1.96.0、Node 24.14.1、npm 11.12.1、Tauri CLI 2.11.3 |
| crates.io | `cargo search` 可達：`fs4 1.1.0`、`keyring 4.2.0` |
| 資料來源 host | Binance 封存 200、`api/v3/ping` 200、Tiingo 301、FinMind 422（空查詢）、TWSE 302 |

未做：任何 `thread/start`／`turn/start`、任何資料下載、任何 DB 讀寫。

## 阻擋項（解除前不得啟用）

1. **AI unattended 模式**：生成環境隔離未驗證（全域 2 個 MCP server、per-thread 覆寫未測；
   獨立 `CODEX_HOME` 會失去登入且禁止複製 auth）；模型清單與設定不一致；子命令 experimental。
   P15 以一次有界真實生成解除或維持；不得改成付費 API 或 GUI 自動化。
2. **Tiingo（P09）**：無帳戶／token。
3. **ETF calendar／年化／配息／分割語意（P08）**：資料與 metrics 語意都缺。
4. **JSON DSL 執行（P11）**：無 evaluator；白名單 24 個指標只有 11 個＋價格來源兩核心皆有。

## Verification

純文件 phase（Plan §6：「純文件 phase 不啟動研究或完整測試」）。未重跑 vitest／Rust／e2e；
無程式變更，既有 854 vitest＋155 Rust＋62 e2e 的狀態不受影響。已做：Markdown 連結與
路徑逐一核對（所有引用檔案存在）、`git diff` 檢查只含預期檔案、CRLF 保持（tasks.md／
README.md 只有新增行）。

GitHub：`git fetch origin` 與 `gh auth status` 皆 401（環境變數 `GITHUB_TOKEN` 失效；
`gh` keyring 亦無登入）。依 Plan §6：「GitHub 暫時不可用不阻擋本機分析；不得因此假設遠端已
同步」。**本分支未 push、PR 未建立**；恢復認證後需先 fetch 並比對 `origin/main`（本機記錄為
`e3fc79f`）再 push，且 base 分支 `docs/alphabtc-capability-transfer` 也尚未 push。

## 下一階段前置條件

- P01（Results Explorer）：依賴 P00 已滿足；需明確授權；範圍只讀既有 records／summaries／
  trades，不改計算。
- P02（Runtime 解耦）：依賴 P00 已滿足；以 `research-runtime-contract.md` §0／§6 為邊界。
- P15 開始前必須重跑 registry §2 的 Codex 版本／能力檢查並更新 registry。

## Resolution (added when acted on)

（待補：push／PR 編號、review 結果。）
