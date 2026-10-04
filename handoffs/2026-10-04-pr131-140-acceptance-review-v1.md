# Handoff: 最近十個 PR（#131–#140）整合驗收

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr131-140-acceptance`
Reviewed baseline: `b9b63a318e2fbc4a8192cd6679697381f4e90a1c`（PR #140 合併後的 `origin/main`）
Range: `8269c7e..b9b63a3`（52 commits；90 files，+28,005／−107，其中大部分是 fixture JSON）
Status: 驗收完成；十個 PR 在各自承諾的範圍內皆可接受，沒有 merge-blocking 問題。一項 P3 缺口（R1，PR #132）已用反例重現，建議在 P13 開始用帳本計數提供 `familyTests` 之前決定處理方式；它不影響 P12e-7b。P12e-FINDING-1 仍未結、P13 仍阻擋、驗收 seed 20261117 未使用。

## Summary

依 GitHub `created desc`、不限狀態選出最近十個 PR，結果是 #131–#140，均已合併。
每個 PR 的合併 commit 與其最終 head 的 tree 完全相同，合併時沒有引入額外內容；
十個最終 head 都有六項成功的 CI jobs。

其中 #134、#135、#136、#138、#139、#140 已有獨立驗收（含覆驗）紀錄，被驗收的 head 就是最終合併的 head。
這次另外補上之前沒有獨立驗收的 #131、#132、#133、#137，並在整合 baseline 上重跑全部本機檢查。
新寫的反例測試發現 R1：沒有新增事件的檢定數升級不在 workspace binding 的保護範圍內。

本次只新增驗收文件、反例 patch 與任務紀錄，產品程式碼維持 baseline。
沒有在被驗收的 PR 上留言或提交 GitHub review；這份紀錄以只含文件的 PR 從本分支發布（維護者 2026-10-04 同意），
讓之後的 agent 知道檢查過哪些項目。以上是本機驗收結論，並非 GitHub 的 review 狀態。
本分支的第一個 commit（`5cb4a16`）原樣收錄上一個 session 留下、尚未提交的 PR #140 驗收紀錄。

## Required Action / Decision

### R1 — P3：沒有新增事件的檢定數升級，不受 workspace binding 保護

- 來源：PR #132（P12e-0，`trial-ledger-v1` §22）。
- 位置：
  - `src-tauri/src/research/trial_ledger.rs:1352–1367,1443–1451`：重播（replay）一個有效批次時，若宣告的檢定數較大，會寫入 `family_protocol_upgrades`，但不新增事件。
  - `src-tauri/src/research/trial_ledger_transfer.rs:1165–1196`：匯入時也會把來源較大的檢定數記成升級；若事件本地都已存在，就同樣沒有新事件（依程式碼推得，未另寫測試）。
  - `src-tauri/src/research/trial_ledger_binding.rs:131–165`：binding 的前綴檢查只比對 `seq` 與 chain head。
- 重現：家族先由舊版以 `testsPerTrial = 1` 登記（釘選值 1）；複製當時的 registry 檔。
  以 2 重播同一批次：家族有效值升為 2，但鏈頭不變。接著依 `r4_the_reopen_watermark_detects_a_later_rollback`
  的方式（刪 `-wal`／`-shm` 後覆蓋檔案）還原舊複本：
  `check_binding(先前觀察到的 binding)` 仍回傳 `Current`，`read_admission_count` 的 `testsPerTrial` 從 2 變回 1。
- 已確認仍然有效的防線：升級後做出的判定，其快照帶有 `testsPerTrial = 2`；
  還原後 `fence_admission` 回傳 `Blocked(Inconsistent)`（反例中的斷言，通過）。
  另外，discovery 下一次新登記一律宣告 2，會再次升級，而且那次登記有新事件。
- 影響界線：要同時具備「沒有新事件的升級」與「在同一家族下一次新登記之前，把 registry 檔還原到較舊複本」。
  前者在現行程式中只發生在：重播一個由 PR #132 之前的版本以 1 登記的批次（例如中斷後重試，或 legacy 回填碰到已登記的批次），
  或是事件全都已存在的匯入。
  我沒有在現行 runtime 路徑中構造出「新判定用到偏小的 m」；這是合約層面的缺口：
  §22 承諾 m 只升不降，§7 的 binding 是防回退的機制，但 binding 只錨定事件鏈。
- 建議（請維護者決定，不阻擋 P12e-7b，因為那是純模擬，不讀帳本）：在 P13 以帳本計數提供 `familyTests` 之前擇一：
  1. 在 §22 的「限制」寫明升級列在 binding 保護之外，並規定 P13 一律經由帶快照的圍欄使用計數；或
  2. 讓升級也受錨定，例如只在有新事件的登記中才寫入升級，或讓 binding／快照同時記錄家族檢定數。
  其他沒有事件的證據（`family_conflicts`、`registry_conflicts`、`origin_genesis`）是否也有同樣性質，
  本次**沒有驗證**，建議修正時一起檢查。
- 反例：`review_a_restored_registry_drops_a_protocol_upgrade_unseen`（見下方 patch）。

## Review Notes（不需修正的觀察）

- **O1（#131）**：campaign UI 只在 `?mock=1` 與單元測試中執行過；PR 本身列的 native／service smoke（由服務建立 snapshot、預覽、凍結、逐 instrument 啟動、重讀判定）仍未勾選，tasks.md 也記為 operator acceptance。
  另外，`fixtures/rs-core/research-campaign-declaration-v1.json` 只被 Rust 使用，TS 建構器產生的宣告從沒有經過 Rust `freeze_campaign` 的測試。
  我逐欄比對過兩邊的欄位名稱與網域，目前一致；任何不一致都會在後端預覽時 fail closed（例如 rationale 超過 1024 bytes、
  或 JS `trim()` 與 Rust `trim()` 對 U+0085、U+FEFF 的處理不同），不會產生錯誤的凍結。
  建議在用 UI 跑真實 campaign 之前，補上 native smoke 或一個共用 fixture。
- **O2（#132／#133 銜接 P13）**：`CONFIRMATION_TESTS_PER_TRIAL`（library）與 `DISCOVERY_TESTS_PER_TRIAL`（desktop）是兩個獨立常數，沒有測試把它們綁在一起；
  確認統計要求 `familyTests` 是 2 的倍數。若某家族經匯入升到奇數檢定數，P13 會被拒絕（fail closed，不會少算）。
  P13 設計帳本計數到 `familyTests` 的對應時，應一併處理。
- 不是問題的地方（已檢查）：
  - #133 的 Holm 把本批 k 個檢定放在家族 m 個檢定的前 k 名計算乘數；對任何未知的家族其他 p 值排列，這都不小於完整 Holm 的調整值（保守）。
    先 cap 再取 running max 與先取 max 再 cap 等價；p 值相同時，調整後的值與排序無關。
    `below(n)` 的拒絕門檻 `2^64 mod n` 讓抽樣沒有模數偏差。
  - #137 的 Wilson 整數判定式 `(lN − 10⁶x)²·10000 ≥ 38416·l·(10⁶ − l)·N` 與 `z = 49/25` 的 score test 等價；
    閉式 bounds 也由 Wilson 公式重新推導，結果一致；宣告的驗證順序與合約相同。
  - #140 的 R3 接受集合（36–42、64–91、≥ 100）由 `(L − ½)³` 與 `(2L)² ≤ n` 重新推導一致；
    程式碼中，seed 20261117 只出現在宣告產生腳本、宣告 fixture、只 parse 不模擬的測試，以及斷言它尚未被使用的測試中。
  - #131 的 DEV-only mock 沒有進入 production bundle（`dist/assets/*.js` 搜尋 mock 專屬字串，結果為 0）；
    重複點擊造成的並行啟動，會被後端 `start_inner` 的 active-run 檢查拒絕。

## Per-PR acceptance

「本次範圍可接受」只涵蓋該 PR 承諾的切片，不等同 P12e／P13 完成，也不代表任何統計量已校準。
本機測試在整合 baseline 上執行；各 head 的 CI 是遠端結果，沒有逐一 checkout 重跑。

| PR | 最終 head → 合併 | 本次結論與檢查重點 | 先前驗收 | CI run（六項通過） |
| --- | --- | --- | --- | --- |
| [#131](https://github.com/yoyoCadence/AlphaFactorForge/pull/131) | `161740b` → `b3f05c3` | 本次範圍可接受（本次新審）。宣告欄位與 Rust `freeze_campaign` 一致；預覽綁定 draft revision；凍結需最新且全部 resolved 的預覽並比對 ID；只顯示 ELIGIBLE／NOT_ELIGIBLE，沒有 PASS。Native smoke 未做（O1）。 | 無 | [36725457871](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36725457871) |
| [#132](https://github.com/yoyoCadence/AlphaFactorForge/pull/132) | `4c08fe0` → `cd244ef` | 本次範圍可接受，附 R1（P3）。migration 0003 只能追加；有效值 = max(釘選值, 升級)；降低會被拒絕；v3 匯出／匯入取最大值聯集，且驗證升級必須大於來源釘選值；圍欄方向正確（變大 → `Grew`，變小 → `Inconsistent`）；runner 三個登記點都改為 2。 | 無 | [36793360692](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36793360692) |
| [#133](https://github.com/yoyoCadence/AlphaFactorForge/pull/133) | `653ebc3` → `3452033` | 本次範圍可接受（本次新審）。嚴格解析、無偏的 SplitMix64 抽樣、置中單尾、`(1+e)/(B+1)`、家族 Holm 保守、alpha 以精確整數比較。之後因 P12e-FINDING-1 不再是最終方法，但仍是 v2 共用的骨架。 | 無 | [36857304318](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36857304318) |
| [#134](https://github.com/yoyoCadence/AlphaFactorForge/pull/134) | `56be681` → `b5a3438` | 可接受；整合測試通過，沒有新發現。 | [驗收＋覆驗](2026-10-01-pr134-alpha-allocation-acceptance-review-v1.md)，R1 已關閉 | [36879266778](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36879266778) |
| [#135](https://github.com/yoyoCadence/AlphaFactorForge/pull/135) | `0125e6c` → `1764c48` | 可接受；P12e-FINDING-1 維持未結。 | [驗收＋覆驗](2026-10-02-pr135-noise-simulation-acceptance-review-v1.md)，R1／R2 已關閉 | [37005074144](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37005074144) |
| [#136](https://github.com/yoyoCadence/AlphaFactorForge/pull/136) | `58e7404` → `556a397` | 可接受（只有計畫文件）。 | [驗收＋覆驗](2026-10-02-pr136-recalibration-plan-acceptance-review-v1.md)，R1 已關閉 | [37013969053](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37013969053) |
| [#137](https://github.com/yoyoCadence/AlphaFactorForge/pull/137) | `c6b339b` → `50b20e5` | 本次範圍可接受（本次新審）。Wilson 判定式與閉式 bounds 重新推導一致；宣告驗證順序與合約一致。#139 的驗收已用這個引擎完整重放 54 份報告。 | 無 | [37018251272](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37018251272) |
| [#138](https://github.com/yoyoCadence/AlphaFactorForge/pull/138) | `276765a` → `13100ce` | 可接受；整合測試通過。 | [驗收＋覆驗](2026-10-03-pr138-candidate-statistics-acceptance-review-v1.md)，R1／R2 已關閉 | [37084566037](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37084566037) |
| [#139](https://github.com/yoyoCadence/AlphaFactorForge/pull/139) | `4a21b52` → `e9e73e8` | 可接受。 | [驗收](2026-10-03-pr139-diagnostic-grid-acceptance-review-v1.md)，54／54 報告獨立重放 | [37117837930](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37117837930) |
| [#140](https://github.com/yoyoCadence/AlphaFactorForge/pull/140) | `759ecde` → `b9b63a3` | 可接受；另確認最終 seed 沒有任何執行路徑。v2 仍**沒有**統計上被接受的設定。 | [驗收](2026-10-04-pr140-frozen-v2-acceptance-review-v1.md)（本分支 `5cb4a16` 收錄） | [37126987054](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37126987054) |

## Verification

- 開始時在 `feat/p12e7-final-acceptance`，工作區有上一個 session 未提交的 PR #140 驗收紀錄；
  依慣例 stash → `main` fast-forward 到 `b9b63a3` → 建立本分支 → pop → 原樣提交（`5cb4a16`）。
- 合併 commit 與 head：十組 `git diff <head> <merge>` 都是空的。
- 遠端：十個最終 head 的 typecheck、test、build、cargo-check、native-smoke、e2e 都是 SUCCESS（run 見上表）。
- 本機（Windows，`CARGO_TARGET_DIR` 放在 OneDrive 之外）：
  - `cargo test --locked`：**168 library（1 ignored）+ 396 desktop + 2 service smoke = 566 通過**，與 #140 的 CI 相同。
  - `npm test`：**64 files、1072 tests 通過**。
  - `npm run typecheck`、`npm run build`、`cargo check --locked --all-targets`：通過。
  - `cargo clippy --locked --all-targets`：只有既有的 5 個 warning（`backtest.rs` ×2、`score.rs` ×2、`file_commands.rs` ×1），十個 PR 的新檔案沒有新增。
  - `rustfmt --check`：十個 PR 新增的 13 個 Rust 檔都乾淨。
  - Playwright（`--workers=1`、`E2E_PORT=5199`）：全套 82 項中 79 項通過；失敗的 3 項是最先執行的 campaign 案例，
    都卡在 `page.goto` 逾時。實測冷啟動的 dev server 第一次完整載入要 **55 秒**，超過測試的 30 秒上限。
    預熱 server 後單獨重跑 `campaign.spec.ts`，**4／4 通過**，所以本機合計 82／82。這是本機環境問題，CI 會自行啟動 server，不受影響。
- 反例：把測試暫時加入 `trial_ledger.rs` 後執行 `cargo test --locked --bin alpha-factor-forge review_a_restored`，
  結果 **1 failed**；失敗落在最後一個行為斷言（`restored copy is Current(...) with testsPerTrial 1`），
  在它之前的圍欄斷言通過。之後已移除暫時測試，產品來源與 baseline 一致，`git apply --check` 確認 patch 可以套用到 baseline。
- 未執行：native Tauri／service 的 campaign smoke（O1）、#139 的完整 release 重放（沿用該驗收的結果），
  也沒有使用 seed 20261117。沒有新增依賴，也沒有碰使用者真實的 registry；反例使用臨時目錄。

## Reproduction patch

[R1 反例 patch](2026-10-04-pr131-140-acceptance-regressions.patch) 適用於 `b9b63a3`，只新增一個測試，不含修正。
在 repo root 套用：

```powershell
git apply --check handoffs/2026-10-04-pr131-140-acceptance-regressions.patch
git apply handoffs/2026-10-04-pr131-140-acceptance-regressions.patch
Set-Location alpha-factor-forge/src-tauri
cargo test --locked --bin alpha-factor-forge review_a_restored -- --nocapture
```

在未修正的 baseline 上，預期 1 failed，訊息是 `restored copy is Current(...) with testsPerTrial 1`。
看完之後回到 repo root，用 `git apply --reverse handoffs/2026-10-04-pr131-140-acceptance-regressions.patch` 移除；
若已經同時修改了程式，先檢查差異，避免一起覆蓋。若決定採用建議 1（只寫文件），這個測試應改寫成鎖定「圍欄擋下判定」的行為，而不是直接納入。

## Remaining scope

- P12e-7b（以 seed 20261117 執行六格驗收一次）是下一個產品步驟，R1 不阻擋它。
- P13 仍然阻擋：要等 P12e-7b 通過，並先決定 R1 與 O2。
- 本次的 task board 只新增一個 Backlog 項目（R1）與紀錄，沒有重排既有的產品任務。

## Resolution

Pending. 處理 R1 的人請追加決定、變更 commit 與對應的回歸結果，並保留本次的原始證據。

## Follow-up work order（2026-10-04 追加）

本檔的發現已整理進 [驗收後續工作單](2026-10-04-acceptance-followups-work-order-v1.md)，請接手的 agent 從那裡開始：

- R1 → **FU-1**（與 PR126-130-A-R1 同一個根因，一起處理）；需要 **D1、D2**。
- O1（#131 native campaign smoke）→ 工作單 §3 的 **O3**，含完整步驟；注意服務 `fetch` 要帶 `--cost-profile`，否則 snapshot 為 degraded、預覽會拒絕。
- O2（兩個常數未綁定、奇數檢定數會讓確認 fail closed）留給 P13 設計時處理，未列為獨立工作項目。
