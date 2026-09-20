# Handoff: PR #109 / P07 acceptance review

Date: 2026-09-20
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p07-binance-adapter`
PR: https://github.com/yoyoCadence/AlphaFactorForge/pull/109
Reviewed commit: `616288976de777cd5bc2486ae4043d43a6028be4`
Base: `503fc99b06b98ca653d3010db59e7f6694de6a15`
Status: Resolved locally; both fixes and regressions complete, awaiting PR CI and owner merge (see Resolution)

## Summary

P07 的正常取得路徑、既有測試及文件列出的真實區間均已獨立重跑。驗收仍不通過：封存來源的 availability 被回填成未經證明的歷史時間，且 CHECKSUM 回應原件未保存。兩個反例均在 HEAD 的隔離副本失敗；未修改目前分支的產品程式碼，未 push、留言、merge 或開始 P08。

## Required Action / Decision

### 1. [P1] 不要以封存期間終點充當已知 availability

位置：`alpha-factor-forge/src-tauri/src/market/ingest.rs:454`（`unit_available_at`），並在同檔 394 行傳入 accepted provenance。

函式把日／月期間終點直接寫入 `available_at`，但這只表示資料不可能更早完整，不是原件已發布或已觀測的證據。P06 的 `snapshot::component_events` 以 `availableAt <= asOf` 判斷 forward-observed，因此這個欄位會實際授予過去時間點的資格。

反例：在 2026-09-20 取得 committed 的 BTCUSDT 2024-07-15 日封存，provenance 寫成 `retrievedAt=2026-09-20…`、`availableAt=2024-07-16T00:00:00+00:00`。接著透過現有 `build_snapshot`，以該 provenance、`kind=ForwardObserved`、`as_of_ms=1721088000000` 建快照，實際回傳 `Created`、`status=Ok`，應被拒絕。CLI 目前固定建立 historical，這個反例走的是已存在的後端 snapshot API，不表示 CLI 已直接輸出 forward-observed。

契約依據：`docs/market-contract.md` §2 明定歷史重抓不能冒稱 point-in-time；`docs/market-foundation-v1.md` §4 的未知 availability 不能支撐 forward-observed。另據 [Binance 官方說明](https://github.com/binance/binance-public-data#readme)，月檔於當月第一個星期一提供，並不等於月界的午夜，而且封存可能事後修訂。

修正：沒有原件發布／觀測證據時保留未知，或使用有證據的實際取得時間作保守界線；不能把期間終點當作已知發布時間。對已寫入的錯誤 metadata 應訂明修正／隔離方式，不能繼續快取重用錯誤 availability。加入「重抓舊封存不能建立舊 cut 的 forward-observed」回歸測試，並保持 historical 正常取得。

### 2. [P2] 保存 CHECKSUM 回應原件與 ZIP 的對應關係

位置：`alpha-factor-forge/src-tauri/src/market/ingest.rs:324`（取得 `digest_bytes`）及 384 行（只保存解析後的 `publishedSha256`）。

正常路徑把 `.CHECKSUM` 回應解析成字串後丟棄；artifact store 只保存 ZIP。反例取得一個完整日檔後，DB 只有一筆 provenance，遍歷 store 找不到 fixture 的 CHECKSUM 原始 bytes。`publishedSha256` 可校驗 ZIP，但不能取代含檔名的發布回應原件，因而無法離線重播當時 CHECKSUM 檔名／格式驗證。ZIP 解析失敗的路徑也沒有把已解析的 checksum 放入 scope。

契約依據：`docs/market-contract.md` §2 `rawResponseHash` 明定「原始 bytes，含 CHECKSUM 檔時一併保存」。這不是要求新增來源或修訂偵測功能。

修正：保存 CHECKSUM 原始 bytes、來源 URL／取得證據及其與 ZIP 的明確關聯；accepted 和 rejected ZIP 都應能回溯當時依據的 CHECKSUM。追加正常與拒絕路徑的離線重播測試。

## Verification

- 工作目錄起初乾淨；`git fetch origin` 後 HEAD 與遠端 feature branch 一致，共 4 commits，base 為 P06 merge。
- `npm test`：**911 passed**。
- `npm run build`：**pass**，包含 `tsc --noEmit`。
- `cargo test --locked`：**328 passed**（lib 69 + desktop/shared 257 + service smoke 2）。
- `cargo check --locked --all-targets`：**pass，0 warning**。
- `cargo clippy --locked --all-targets`：**pass**；5 個 warning 位於未修改的 backtest/score/file_commands，P07 檔案無新增 warning。
- `E2E_PORT=5219 npx playwright test --workers=1`：**78 passed**，2.2 分鐘；測試後關閉本次 Vite server。
- GitHub 即時 PR／CI 狀態未能驗證：清除代理後 `gh pr view 109` 仍回覆 HTTP 401 Bad credentials；不把截圖的 CI 全綠當成此次查詢結果。

真實抓取使用新建的 `C:/tmp/aff-p07-acceptance-*` 隔離工作區，均指定 `--cost-profile cost-profile-v1`（缺漏案例未指定），不觸及正式工作區：

| 範圍 | 本次結果 |
| --- | --- |
| BTC 1h 2024-12-30 → 2025-01-03 | **96/96**，兩個封存分別 milliseconds／microseconds，exit 0 |
| BTC 1h 2025-01-01 → 2026-01-01 | **8760/8760**，exit 0 |
| ETH 1h 2025-01-01 → 2026-01-01 | **8760/8760**，exit 0 |
| BTC 1h 2017-07-01 → 2017-09-01，`--no-rest` | **356/1488**，缺 **1132** 根，`rangeIncomplete`，exit 5 |
| BTC 1h 2026-09-01 → 2026-09-21 | **464/465**，1 根 not due，19 日檔＋1 REST 頁，exit 0；與截圖 462/463 差異來自較晚的執行 cut |

額外反例：從 HEAD `git archive` 到 `C:/tmp/aff-p07-review-source-20260920`，只在副本加入兩個 acceptance tests；`cargo test --locked --bin alpha-factor-forge-service acceptance_review -- --nocapture` 結果 **0 passed / 2 failed**，分別證明以上兩項。可套用的測試 patch 見同目錄 `2026-09-20-pr109-p07-acceptance-regressions.patch`；只套用到隔離 checkout，勿把預期失敗測試誤認成本次基線套件退步。

本機完整輸出：`C:/tmp/aff-p07-review-{rust,vitest,build,check,clippy,e2e,regression}.log`；真實抓取報告：`C:/tmp/aff-p07-review-{live,btc,eth,gap,tail}.json`。暫存證據不作為唯一交接來源，本文件與測試 patch 保留可重現的要求。

## Next step

在 P07 分支完成兩項修正及回歸，追加本文件的 Resolution，再重新驗收。P08 保持原排程狀態。

## Resolution

2026-09-20：使用者授權接續 Claude 未完成的修正，並要求完成後更新 PR #109。以下是本次修正的最終行為；前述 review 內容保留為歷史。

- **P1 resolved**：每份 CHECKSUM／ZIP 都在回應完整到齊後記錄觀測時間，`availableAt = retrievedAt`，以 `availabilityBasis: observed-at-retrieval` 識別。封存期間終點只留在 `periodEndsAt`。舊來源缺少觀測標記或觀測時間不相符時，forward-observed admission 回報 `availability_unknown`，新來源晚於 cut 則回報 `availability_after_cut`；實際觀測後的 cut 可以正常建立快照。
- **舊資料隔離**：不覆寫 provenance／snapshot，也不增加 migration。舊 archive cache 會重新抓取，保留旧紀錄供稽核。get／get-by-id／list／dataset-status 會拒絕傳回未經觀測證明的既有 forward-observed 快照；historical 仍可讀。這些 reader 在遇到不合格 forward 快照時回傳明確錯誤，而非悄悄略過該列。
- **P2 resolved**：CHECKSUM 原文用 `text/plain` provenance 保存，帶有 URL、檔名、`vouchesFor`；每個 ZIP attempt 以 `checksumProvenanceId` 連結對應原文，並保存 `publishedSha256`。包含 mismatch 後成功的 retry、ZIP 解析拒絕、CHECKSUM 格式拒絕，均保留實際回應。Malformed CHECKSUM 不誤存為 ZIP。
- **快取驗證**：重讀 ZIP／CHECKSUM 原件，核對商品、週期、來源、URL、檔名與修訂狀態，重播 SHA-256 驗證。舊 metadata 或遺失 CHECKSUM 會重抓，完整證據才會 cache hit。既有內容定址檔若本身損壞，artifact store 仍拒絕覆寫；沒有在這次修正暗中新增修復／刪除流程。
- **範圍**：未開始 P08；未修改 DB schema、dataset hash、TS/Rust core 或 UI。新 regression 已納入 ingest 的正式測試模組，原始 acceptance patch 保留作歷史反例，無須再套用到修正後分支。

驗證：

- 在隔離的原 HEAD `6162889` 只加入本次測試：9 個原 ingest tests 通過、**7 個新測試全部失敗**，證明能攔下修正前行為。
- 修正後 `cargo test --locked`：**335 passed**（69 lib＋264 desktop/shared＋2 service smoke）；`cargo check --locked --all-targets` 通過且零 warning；clippy 通過，只有原先 5 個無關檔案的 warning。
- `npm test`：**911 passed**；`npm run build` 通過（含 TypeScript typecheck）。本次未重跑 UI／native smoke；前述驗收已跑過 78/78 Playwright。
- 真實舊快取升級：在前次驗收的隔離工作區 `C:/tmp/aff-p07-acceptance-20260920` 重跑 BTCUSDT 1h 2024-12-30→2025-01-03，原兩個月檔從舊 metadata **重新 fetched**，96/96、exit 0。立即重跑兩個月檔均 **cached**，仍 96/96、exit 0。資料庫確認新 ZIP 各自連結新 text/plain CHECKSUM，兩者均有觀測標記且 `availableAt == retrievedAt`。
- 本機輸出：`C:/tmp/aff-p07-fix-{red,rust,check,clippy,vitest,build}.log`、`C:/tmp/aff-p07-fix-{upgrade,cache}.json`。

修正已具備本機合併前驗證證據；PR 推送後以最新 commit 的 GitHub checks 為準。由使用者合併。
