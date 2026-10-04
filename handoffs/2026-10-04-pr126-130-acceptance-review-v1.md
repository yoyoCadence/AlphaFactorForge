# Handoff: PR #126–#130 整合驗收（往前驗收 batch A）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`（PR #141 合併後的 `origin/main`；程式碼與 `b9b63a3` 相同，只有文件差異）
Range: `af8196d..8269c7e`（PR #126–#130）
Status: 驗收完成；五個 PR 在各自承諾的範圍內可接受。一項 P2（A-R1）已用反例重現，一項 P3（A-R2）由程式碼與 Tauri 官方文件確認，另有一個設計注意事項。修正交給維護者安排；本次不改產品程式碼。

## Summary

這五個 PR 介於前兩次整批驗收（#116–#125、#131–#140）之間，之前沒有整批看過。
#126 修的是 #116–#125 驗收的 R1–R5，但修完後沒有獨立覆驗；#127 的審查發現已由 #128 帶進 main；
#129、#130 只有作者自己的驗證。

五個合併 commit 的 tree 都與各自的最終 head 相同，五個 head 的六項 CI 都通過。
#126 的修正在它宣告的情境（匯出／匯入、多次轉移、重開）中有效；
但在「把 registry 檔還原成較舊複本」這個 R4 明確要防的情境下，**import 偵測到的永久隔離會消失**，binding 也看不出來（A-R1）。
這和 [#131–#140 驗收的 R1](2026-10-04-pr131-140-acceptance-review-v1.md)（檢定數升級）是同一個根因：
binding 只錨定事件鏈，而隔離、升級這類「不新增事件的證據」不在鏈上。

## Required Action / Decision

### A-R1 — P2：import 偵測到的永久隔離，可以被「還原較舊的 registry 檔」解除，binding 看不出來

- 來源：PR #126（P12-AUDIT-R1 的修正，`trial-ledger-v1` §20 的「隔離永久有效」），與 #119／#126 的 binding（只看事件鏈）。
- 位置：
  - `src-tauri/src/research/trial_ledger_transfer.rs:1133–1148`：import 把衝突記入 `family_conflicts`，並跳過衝突的事件（不新增事件）。
  - `src-tauri/src/research/trial_ledger_binding.rs:131–165`：前綴檢查只比對 `seq` 與 chain head。
- 重現（全程使用受支援的 API，加上與 `r4_the_reopen_watermark_detects_a_later_rollback` 相同的檔案還原方式）：
  1. registry A 登記批次 `same`，記下 binding；關閉後複製 registry 檔。
  2. registry B 以同一冪等鍵、不同 strategy 登記；把 B 的匯出匯入 A。A 正確回報 `Blocked(FamilyQuarantined)`，`added_events = 0`，鏈頭不變。
  3. 刪掉 `-wal`／`-shm` 後，用步驟 1 的複本覆蓋 A。
  4. `check_binding(步驟 2 觀察到的 binding)` 回傳 `Current`；`read_admission_count` 回傳一般的 `Count`（family 1、tests 2），**隔離消失**。
- 影響：§20 說來源衝突造成的隔離是永久的，原本 R1 正是因為「匯出／匯入洗掉隔離」而列為 P1。
  這次的路徑需要檔案層級的還原，但這正是 R4 的 binding 要偵測的情境，而一般使用者「從備份還原 registry」也很常見。
  隔離消失後，該家族可以重新取得計數、被判為 `ELIGIBLE`。
  而且**沒有其他防線**：還原前做的判定，圍欄會回 `Unchanged`，因為隔離已經不存在（與 #131–#140 R1 不同，那邊的圍欄還擋得住）。
- 同一根因的已知案例：#131–#140 驗收的 R1（`family_protocol_upgrades`）。
  `registry_conflicts`、`origin_genesis` 是否也能用同樣方式回退，本次**沒有驗證**。
- 修正方向（請維護者決定）：讓不新增事件的證據也進入 binding 的保護範圍，例如：
  1. 把隔離／升級／來源衝突寫成鏈上的「證據事件」（不計入有效試驗），讓它們推進 chain head；或
  2. 讓 binding 同時保存這些證據的摘要（例如筆數加雜湊），回退時判為 `RegistryRolledBack`。
  建議與 PR131-140-R1 一起處理，並補上「還原舊檔後仍隔離」的回歸測試。
- 反例：`review_a_restored_registry_releases_an_import_quarantine_unseen`（見下方 patch）。

### A-R2 — P3：campaign 預覽是同步 Tauri 命令，會在主執行緒上載入並驗證所有 candle

- 來源：PR #130。
- 位置：`src-tauri/src/commands/campaign_commands.rs:115`（`preview_research_campaign`，沒有 `async`）→ 每個宣告的 instrument 都呼叫
  `resolve_campaign_instrument`；`src-tauri/src/research/campaign_snapshot.rs:183–185` 讀取整個 dataset 的 candle，並重算 dataset identity。
- 依據：Tauri v2 官方文件（Calling Rust from the Frontend）寫明「Commands without the _async_ keyword are executed on the main thread unless defined with _#[tauri::command(async)]_」。
  同一檔案裡會寫入的 `freeze`／`start` 都用 `async` + `spawn_blocking`，只有這個重的讀取沒有。
- 影響：預覽的成本與「bar 數 × instrument 數」成正比（一個 campaign 最多 128 個 instrument），期間 UI 會卡住，
  而且持有 DB mutex，執行中的 discovery 寫入也得等。**耗時沒有實測**；這是依程式碼與文件做出的判斷。
  AGENTS.md 把「長時間工作離開 UI thread」列為高風險項目。
- 修正方向：改成 `async` + `spawn_blocking`（與同檔案的寫入命令相同），或標成 `#[tauri::command(async)]`；DB 鎖範圍維持不變。
- 既有任務：同類問題已在 tasks.md Backlog 的 **`DB-ASYNC-001`（P2，未完成）** 中，那是 PR #76 稽核留下的項目（大型匯入、結果保存、檔案寫入都是同步且持有 DB mutex）。
  建議把這個預覽命令加進 `DB-ASYNC-001` 的範圍，不另開任務；它是該任務建立後才新增的一個實例。

## Review Notes

- **設計注意事項（#129／#130）**：已保存的 campaign 每次讀取都會用**目前**的合約常數重新凍結（`db/campaign.rs:107,158`）。
  日後只要升級任何一個被釘選的合約版本（例如 `metrics-v2` → `v3`），所有已保存的 campaign 都會無法讀取；
  而 `list_campaigns` 遇到任何一列失敗就整個報錯，UI 的已保存清單會整個打不開。
  這是刻意的 fail closed，但下一次升級合約版本前，需要先決定舊 campaign 的處理方式（例如逐列標記為過期，而不是整個清單失敗）。
- 已檢查、沒有問題的地方：
  - #129：所有 campaign 檢查都在第一次寫入前完成；admission 的候選集合必須等於入隊的 lineage（index 加 strategy hash，資料庫層拒絕不一致）；
    宣告的 snapshot 決定試驗家族；fee/slip 取自 run config 並與判定在同一筆交易保存；兩張新表只能追加。
  - #130：`campaign.freeze`／`campaign.start` 依 requestId 冪等，payload 形狀固定；前端不能提供 campaign ID 或判定狀態；
    command 參數命名有釘選測試；連線模式經 `ServiceProxy` 轉送；後端在已有 active run 時拒絕啟動。
  - #127／#128：#127 審查的三個 P2（snapshot 身分、批次身分、walk-forward 報告一致性）已在 #128 修正並有回歸測試。
  - #126：R1–R5 的原始反例與 12 項回歸測試都在 suite 中，而且通過（本次全套測試的一部分）；A-R1 是它們沒有涵蓋的路徑。

## Per-PR acceptance

| PR | 最終 head → 合併 | 結論 | 先前驗收 | CI run（六項通過） |
| --- | --- | --- | --- | --- |
| [#126](https://github.com/yoyoCadence/AlphaFactorForge/pull/126) | `0e922ef` → `b5667fb` | 本次範圍可接受；宣告的情境都有效，附 A-R1（P2）。 | 只有作者驗證（原始反例 5 failed → 9 passed） | [36572432621](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36572432621) |
| [#127](https://github.com/yoyoCadence/AlphaFactorForge/pull/127) | `35d02a9` → `226f043` | 可接受；合併時缺的三項修正由 #128 補上。 | [審查](2026-09-29-p12d2b-campaign-admission-v1.md)，3 項 P2 | [36630471840](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36630471840) |
| [#128](https://github.com/yoyoCadence/AlphaFactorForge/pull/128) | `029e7f4` → `3cb58e7` | 可接受。 | 審查者自己的修正，4 項回歸 | [36708968263](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36708968263) |
| [#129](https://github.com/yoyoCadence/AlphaFactorForge/pull/129) | `e478512` → `3a92e9f` | 本次範圍可接受（本次新審）。 | 無 | [36711799857](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36711799857) |
| [#130](https://github.com/yoyoCadence/AlphaFactorForge/pull/130) | `402b270` → `8269c7e` | 本次範圍可接受，附 A-R2（P3）。 | 無 | [36714893574](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36714893574) |

## Verification

- 五組 `git diff <head> <merge>` 都是空的；五個 head 的六項 CI 都是 SUCCESS。
- 整合 baseline 的全套測試沿用 #131–#140 驗收的結果（程式碼相同）：566 Rust（1 ignored）、1072 Vitest、82/82 Playwright，以及 typecheck／build／check／clippy。
- 反例：暫時加入 `trial_ledger_transfer.rs` 後執行 `cargo test --locked --bin alpha-factor-forge review_a_restored_registry_releases`，
  結果 **1 failed**，失敗落在最後的行為斷言（`restored copy is Current(...) with admission Count(...)`）；
  前面的「隔離已生效」「鏈頭不變」斷言都通過。之後已移除暫時測試，產品來源與 baseline 相同，`git apply --check` 確認 patch 可套用。
- A-R2 沒有寫測試或量時間：依據是程式碼與 Tauri 官方文件。

## Reproduction patch

[A-R1 反例 patch](2026-10-04-pr126-130-acceptance-regressions.patch)（適用 `5b4a562`，只新增測試）：

```powershell
git apply handoffs/2026-10-04-pr126-130-acceptance-regressions.patch
Set-Location alpha-factor-forge/src-tauri
cargo test --locked --bin alpha-factor-forge review_a_restored_registry_releases -- --nocapture
```

未修正前預期 1 failed。看完後在 repo root 執行 `git apply --reverse` 移除。

## Resolution

Pending.

## Follow-up work order（2026-10-04 追加）

本檔的發現已整理進 [驗收後續工作單](2026-10-04-acceptance-followups-work-order-v1.md)，請接手的 agent 從那裡開始：

- A-R1 → **FU-1**（與 PR131-140-R1 合併處理）；開工前需要維護者確認 **D1**（binding 保護方式）與 **D2**。
- A-R2 → **FU-5**（併入 `DB-ASYNC-001`），不需要決定。
- 已保存 campaign 遇到合約升級的設計注意事項 → **FU-9**，需要 **D6**。

### Resolution — FU-1（2026-10-04）

A-R1 已在 `fix/ledger-evidence-rollback` 修正（trial-ledger §23）：工作區綁定改為 `trial-ledger-binding-v2`，帶著隔離家族、分歧來源與家族檢定數高水位的快照；還原較舊的 registry 複本會得到新的 `registry_evidence_rolled_back`。本檔的反例已改名為 `r23_*` 納入正式測試，最終斷言直接比對該狀態碼。另加 5 項測試；573 Rust 通過；7 個突變都被抓到。詳見 [FU-1 handoff](2026-10-04-fu1-ledger-evidence-snapshot-v1.md)。

### Resolution — FU-5（2026-10-04）

A-R2 已在 `fix/campaign-preview-async` 處理（`DB-ASYNC-001` 的第一個切片）：`preview_research_campaign` 改為 `async` + `spawn_blocking`，參數名稱不變；實作抽成 `preview_campaign`，並改為**逐 instrument 取得 DB 鎖**，另一個命令最多只等一個 instrument 的驗證，而不是整個 campaign。本機量測（release build、檔案型 SQLite／WAL，暫時測試、未提交）：10 年小時線（87,600 根）單一 instrument 的解析約 **27 ms**，其中讀取 candle 約 19 ms；1–5 個 instrument 的典型 campaign 約 30–140 ms，上限 128 個約 3.4 秒。
其他同步命令若在主執行緒等同一把鎖，仍可能等到這段時間；這屬於 `DB-ASYNC-001` 其餘命令的範圍，本切片沒有宣稱它已完成。native 視窗互動驗收仍列在工作單 O3。
