# Handoff: 最近 20 個 PR（#146–#165）驗收

Date: 2026-10-07（Asia/Taipei）
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr146-165-acceptance`
Reviewed baseline: `060f3d68f2dd2dd5cf9f4d016b951222de0308c8`（main，PR #165 已合併）
PR: 本次未建立或更新遠端 PR。
Status: Review complete — 無新增 P1/P2 產品缺陷；一項 P3 交接補齊，既有數值相容性與 operator acceptance 仍開放。

## Summary

維護者要求驗收最近 20 個 PR。GitHub 依建立時間遞減列出的最新 20 個為 #146–#165，全部已合併；本輪逐項讀取 merge diff、目前相關實作、原 handoff 和合約，並交叉審查前端、Rust 與 CI／研究證據。

全部 PR 在各自承諾的切片範圍內可接受，未發現需要回退或新增 P1/P2 修補的變更。新發現是 #165 的 handoff／目前快照尚未補合併結果（P3）。仍需修改的既有 P2 是 `NUMERIC-JSON-002`；原生 campaign 等 operator acceptance 與圖表 bridge 性能也尚未完成。

本次僅新增驗收文件及 task 記錄；未修改產品、測試、依賴、CI、schema 或原始研究結果，未使用任何使用者工作區／registry。審查完成不會把上述未完成事項或 P12/P13 標成 Done。

## Scope and evidence

- 起始為乾淨 `main`；`git fetch origin`、`git merge --ff-only origin/main` 後仍為上述 baseline，再建立 review 分支。
- 透過 GitHub connector 取得最新 20 個 PR 的狀態、最終 head、merge SHA 和最終 head 的 check-runs。每個 head 都有 `typecheck / test / build / cargo-check / e2e / native-smoke` 六項成功結果，共 **120/120**。
- 比較每個 merge tree 與最終 head tree，**20/20 相同**。這讓實際合併內容與所引用的受測內容一致。
- 查詢 20 個 PR 的 inline review comments，皆為空；本輪結論依據實作及驗證，沒有既有 inline review finding 可沿用。
- 逐 PR 審查並追查當前依賴路徑；CI 成功只作為驗證的一部分。

## Per-PR acceptance

下表「通過」指切片驗收；CI 連結均對應該列的最終 head，六項皆成功。

| PR | 最終 head | 驗收結論／重點 | CI |
| --- | --- | --- | --- |
| [#146](https://github.com/yoyoCadence/AlphaFactorForge/pull/146) | `676ec69` | 通過。間接 devDependencies 修補界線符合工作單，未改產品依賴；本輪完整 audit 為 0。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37200271295) |
| [#147](https://github.com/yoyoCadence/AlphaFactorForge/pull/147) | `f9fc768` | 通過。Vitest 4.1.11，保留獨立 vite-node 3.2.4；代表 fixture 能重生並與原內容一致。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37200971203) |
| [#148](https://github.com/yoyoCadence/AlphaFactorForge/pull/148) | `0b47cd1` | 通過。每 PR 的 audit 明確只守 production dependencies；定期完整 audit 仍是既有獨立工作。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37201477231) |
| [#149](https://github.com/yoyoCadence/AlphaFactorForge/pull/149) | `102693e` | 通過。Rust 最低版本以 Cargo.toml 的 `rust-version = 1.89` 為準；P12 進度／狀態 owner 說明符合當時切片。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37202367305) |
| [#150](https://github.com/yoyoCadence/AlphaFactorForge/pull/150) | `9edebca` | 通過。未知 operand 直接拒絕，避免 undefined 流成 NaN embargo；未把 blocks/code 邊界混入 params-only Rust fixture。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37202930224) |
| [#151](https://github.com/yoyoCadence/AlphaFactorForge/pull/151) | `8416dce` | 通過。保留既有案例，新增平盤、單根、週期 1、週期超出序列；Rust 逐案核對所有指標及 warm-up，未放寬容差。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37203565918) |
| [#152](https://github.com/yoyoCadence/AlphaFactorForge/pull/152) | `96d7a71` | 通過。逐列 valid/incompatible/corrupt，raw JSON／歷史決策保留；UI 禁啟動且後端在 run/trial 寫入前重新拒絕無效列。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37209757398) |
| [#153](https://github.com/yoyoCadence/AlphaFactorForge/pull/153) | `4344519` | 通過。凍結宣告／統計未改，18 個 Wilson checks 有原始輸出及重播證據；支持聲明限於宣告集合，保留 v1 失敗與 P13 runtime 待辦。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37211472263) |
| [#154](https://github.com/yoyoCadence/AlphaFactorForge/pull/154) | `a310be5` | 通過作為 audit。實際 SQLite save/read 與 artifact 測試證實浮點 identity 問題；修復仍是 NUMERIC-JSON-002，沒有更換 production parser。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37212880656) |
| [#155](https://github.com/yoyoCadence/AlphaFactorForge/pull/155) | `b2d6ecf` | 通過。重 DB／報告 I/O 移到 blocking worker，保留 transaction、error、mutex 與 writer 語義；寫入 admission 持有至 await 完成。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37214012035) |
| [#156](https://github.com/yoyoCadence/AlphaFactorForge/pull/156) | `10999a5` | 通過。八個 Phase A handler 共用 blocking DB 邊界；invoke keys 與 connect-mode migration skip 保留。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37214990026) |
| [#157](https://github.com/yoyoCadence/AlphaFactorForge/pull/157) | `b6093ab` | 通過。隔離 workspace/registry/WebView2 profile，檢查 CDP＋DB readiness、真實 SQLite invoke、錯誤 key 拒絕及 Rust event；清理與 hosted browser policy 恢復有界。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37246153488) |
| [#158](https://github.com/yoyoCadence/AlphaFactorForge/pull/158) | `f7232a8` | 通過。mock 同 hash save 更新 name/source，保留 id、definition、metadata、lifecycle；與 SQLite UPSERT 一致，拒絕路徑沒有寫入。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37297346065) |
| [#159](https://github.com/yoyoCadence/AlphaFactorForge/pull/159) | `05e893e` | 通過。stylesheet load/error、實際字型及 layout frame 完成後讀座標；原 pan/zoom 斷言保留，本輪正常 E2E 通過。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37298624576) |
| [#160](https://github.com/yoyoCadence/AlphaFactorForge/pull/160) | `647f2e5` | 通過。模組、ordered migrations、invoke/event、工具最低版本與切片限制符合實作；後續 worker 文件亦已由 #161 同步。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37300186896) |
| [#161](https://github.com/yoyoCadence/AlphaFactorForge/pull/161) | `07db8db` | 通過。每工作自有 worker，成功／失敗／取消均 terminate；jobId/context/generation 圍欄與 fresh-worker restart 保留，core 計算未改。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37302558882) |
| [#162](https://github.com/yoyoCadence/AlphaFactorForge/pull/162) | `02a0538` | 通過。四個 campaign reads async 化，保留同一 DB 邊界、field projection、DTO 與 500-snapshot limit。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37317872403) |
| [#163](https://github.com/yoyoCadence/AlphaFactorForge/pull/163) | `7402e9f` | 通過。DB read 與 artifact walk/read/checksum 均離開 polling thread；artifact I/O 不持有 DB lock，resultError 語義保留。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37469573739) |
| [#164](https://github.com/yoyoCadence/AlphaFactorForge/pull/164) | `8c46151` | 通過。embedded DB read 與 connect service round trip 都在 blocking boundary；idle null、run-not-found 與 panic/join error 保留。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37471527229) |
| [#165](https://github.com/yoyoCadence/AlphaFactorForge/pull/165) | `8de16ac` | 產品通過；交接需補齊（下述 R1）。memo 依賴完整，frame 取最新 pointer 位置，up/cancel flush、leave/unmount cancel，Replay/pan/zoom 回歸通過。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37475179586) |

## Required action / decisions

### R1 — P3：補齊 #165 的合併交接（本輪新發現）

位置：`handoffs/2026-10-06-chart-compute-memo-v1.md:6–7`，以及 `tasks.md` 的 `PERF-CHART-COMPUTE-001` Current Snapshot。

handoff 仍為「PR: opened from this branch」「Complete locally; final-head six-green CI gates merge」，沒有 Resolution；snapshot 同樣仍以待 CI 合併描述。目前 GitHub 已確認 #165 在 **2026-10-06 22:07（Asia/Taipei）** 合併，最終 head `8de16ac` 六項 CI 成功，merge `060f3d6`。

影響：接手者無法從最新 handoff／快照直接判定 task 是否仍等待發布。建議依 `handoffs/README.md` 的 append-only lifecycle，保留原驗證內容、追加 Resolution，補 PR、head、CI run、merge SHA 並更新 Status／最新 snapshot。不需要產品修補；新追蹤項目為 `PR146-165-DOC-R1`。

### 既有 P2：NUMERIC-JSON-002 仍需版本化相容性設計

位置：`alpha-factor-forge/src-tauri/src/identity.rs:119–120`、`identity/numeric_json_audit_tests.rs:48`；完整證據在 [numeric audit](../docs/numeric-json-identity-audit-v1.md)。

合法數值 `0.0036944444444444438` 在 JS 與目前預設 serde_json 解析後相差 1 ULP。對 `number-3` fixture 的同一策略，前端 hash 為 `ec10d414…af6`、目前後端為 `b317bbc5…1b1a3`，實際 `insert_verified_strategy` 因 identity mismatch 拒絕前端 hash；原 JSON 帶後端 hash 可以保存，改成 correctly-rounded parser 又會改變該既有 row 的驗證結果。

本輪完整 Rust suite 重播三個 audit regression 均通過；這些測試固定的是既有不一致與保存行為，`NUMERIC-JSON-002` 尚未完成。這是 #154 明確揭露的既有問題，不是該 audit 新引入的回歸。下一步應先做 bounded Mode A 設計：新文件正確解析、舊 identity 驗證規則及 additive re-freeze lineage；保留原 JSON/hash/artifact/trial links，避免直接全域開啟 feature 或重算歷史 identity。

### 已揭露但未完成的驗收與性能工作

- **O3／P12d 原生 campaign 驗收**：需要真實 Tauri UI 與背景 service 建立的 snapshots，完整走 author → preview → freeze → 每 instrument start → history/admission → reopen；服務 fetch 必須帶 `--cost-profile`。#157 新增基礎 invoke/event bridge 證據，本輪 mock E2E 未涵蓋該完整原生流程。原步驟見 [工作單 v1 §3](2026-10-04-acceptance-followups-work-order-v1.md)、[v2 §6](2026-10-04-acceptance-followups-work-order-v2.md)。P12 仍 In Progress。
- **O1／O2／O4**：P01 native 重開、P09 Tiingo 真實帳戶／授權覆蓋、control-token 檔案權限，均沿用既有 operator acceptance；本輪未執行，未使用使用者憑證。
- **原生 responsiveness／PERF-CHART-BRIDGE-001**：async controlled tests 證明 command future 能 yield，尚無完整 native-window 量測；#165 處理計算／pointer repaint，native chart-window 全量 candles 重複傳送仍是原有 P2 follow-up。#161 的單次 UI backtest 仍同步，已在其 handoff 揭露。
- **SEC-NPM-AUDIT-SCHEDULE-001**：本輪 full／production audit 均為 0，CI 只守 production；完整 devDependencies audit 的定期追蹤仍未建立。既有低優先工作，未新增重複 task。

## Verification

| 檢查 | 本輪結果 |
| --- | --- |
| `npm test` | **1105/1105，69 files** |
| `npm run typecheck` | 通過 |
| `npm run build` | 通過；worker 產物正常輸出 |
| `cargo test --locked` | **591 passed（170 library + 419 desktop + 2 service），1 ignored** |
| `cargo check --locked --all-targets` | 通過 |
| `npm run e2e -- --workers=1`，`E2E_PORT=5229` | **83/83**，1.9 分鐘，一次完成；既有 assertions/spec 未改 |
| `npm audit --json`／`npm audit --omit=dev --json` | 兩者 **0 advisories** |
| 三項代表 fixture scratch regeneration | indicator／backtest／gate-score 能由 vite-node 3.2.4 重生；LF-normalized bytes 一致，tracked fixture 未改 |
| GitHub 最終 head checks／merge trees | **120/120 成功；20/20 tree 相同** |

另有前端分組專項 **66/66** tests 通過，涵蓋 mock UPSERT、sweep artifact/client、實際 worker handler、chart series 與 frame throttle。#153 的凍結輸入與 #140 核對一致，report／18 Wilson checks 的既有 replay 通過；**沒有再次執行完整 final-acceptance simulation**，支持集合仍限 `{256,512,1024}` bars × φ `{0,0.3}`、宣告 generator/family/B=799。

第一次 Vitest 及背景 Vite 啟動被沙箱 `spawn EPERM` 拒絕，原操作升權後成功，非產品測試失敗。E2E 使用隱藏專屬 Vite（127.0.0.1:5229）；結束後由 Windows port owner 和 CommandLine 核實 PID `41768`，只停止該程序並確認端口已釋放。測試 log 存於被忽略的 `output/pr146-165-*.log`，audit/scratch 存於 ignored cache；未留下 tracked probe。

本輪沒有另跑本機 native desktop smoke／人工操作；#157 及各最終 head 的 native 證據引用上表 CI。此次瀏覽器驗證使用 `?mock=1`，不能將其範圍外的 operator acceptance 判為完成。

## Next recommended step

先依原 task 順序安排 `NUMERIC-JSON-002` 的版本化相容性設計與 O3/P12d 原生操作驗收；完整 runtime reveal／alpha reservation／freshness fence 保留給 P13。R1 可在一個小型文件切片中補齊，既有性能與定期 audit 工作維持原追蹤。

## Resolution

產品修補與 operator acceptance 待後續獨立切片。本輪審查完成；需要修改／補驗證的項目仍保持未完成。

### Resolution — 2026-10-07, authorized follow-ups started

維護者隨後要求修復、開 PR、確認 CI 後 merge，再往前驗收 #126–#145。
`PR146-165-DOC-R1` 已在 `fix/versioned-strategy-numeric-policy` 補齊 #165 的
Status／PR／Resolution 與 feature snapshot；原驗證內容未改。
數值修復進入 `NUMERIC-JSON-002a`，採用
[manual-strategy-numeric-policy-v1](../docs/manual-strategy-numeric-policy-v1.md)：
新手動策略定義以 hash-covered marker 明示隔離的正確解析政策，歷史定義仍按
原政策驗證、轉換採有來源連結的新版本。parent 的研究/runtime rollout 與
尚未完成的 operator acceptance 維持開放，待具體驗證結果追加。

### Resolution — 2026-10-07, manual slice locally verified

NUMERIC-JSON-002a 的程式與本機驗證完成；新手動策略的版本化數值政策、
舊策略精確解讀載入、另存新版本與拒絕錯誤來源連結，通過 **1131 Vitest /
600 Rust（1 ignored）/ 85 E2E**、typecheck/build/all-target check 及真正的
Tauri invoke + SQLite 數值 smoke。#165 的文件結案已補齊。詳見
[repair handoff](2026-10-07-manual-strategy-numeric-policy-v1.md)。
PR publication／最終 head CI／merge 仍是交付步驟；parent 數值 rollout 與
operator acceptance 並未因這個切片而完成。
