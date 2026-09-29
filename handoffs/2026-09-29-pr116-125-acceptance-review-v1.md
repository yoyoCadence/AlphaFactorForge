# Handoff: 最近十個 PR（#116–#125）整合驗收

Date: 2026-09-29
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr116-125-acceptance`
Reviewed baseline: `af8196de1532028b13023184a95a88b3337f9ff0`（PR #125 合併後的 `origin/main`）
Status: 驗收完成；R1–R5 待修復。整組尚不能無條件通過。

## Summary

依 GitHub `created desc`、不限狀態選取最近十個 PR，結果是 #116–#125，均已合併。
檢查各 PR 的 scope、變更與現行契約，並在整合版本上重跑測試與建置。
10 個 PR 的最終 head 均有六個成功的 CI jobs；本機既有 975 項 Vitest、455 項 Rust 測試也全部通過。
但新增的五項反例測試全部失敗，確認帳本搬移、來源分歧、切分身分與水位保存仍有缺口。

本次只有驗收文件、任務紀錄與可套用的測試 patch；執行反例時暫加的 Rust 測試已移回 patch，產品程式碼維持 baseline。
未發布 GitHub 評論、review、commit 或 PR。以下是此次本機驗收結論，並非 GitHub 的 review 狀態。

## Required Action / Decision

### R1 — P1：永久隔離狀態會被匯出／匯入洗掉

- 來源：PR #118；PR #116 的 §8.1 匯出契約也缺少衝突紀錄。
- 位置：`src-tauri/src/research/trial_ledger_transfer.rs:42–74,112–256`；`docs/trial-ledger-v1.md:353–393`。
- `ExportRecord` 只有 family、batch、event、receipt、originCheckpoint，沒有 `family_conflicts` 或 `registry_conflicts`。
  `export_json_lines` 也沒有拒絕匯出已隔離家族。
- 重現：A 登記一筆試驗；B 以相同冪等鍵、不同 strategy 登記，再將 B 匯入 A。
  A 正確回傳 `Blocked(FamilyQuarantined)`；把 A 的正常匯出檔匯入新 C 後，C 卻回傳 `Count`，有效試驗數為 1。
  全程使用受支援的 register/export/import API，沒有修改資料庫 trigger 或竄改匯出 bytes。
- 影響：備份或換機可以解除原本宣告永久的隔離，遺失已知衝突證據；後續 admission 會取得本來不應可用的計數。
  本次未宣稱現行產品已因此產生 qualification PASS，P12d admission 尚未接線。
- 修復要求：讓完整 transfer 保留並聯集兩類衝突證據，或在無法安全表示時拒絕此類匯出／還原；同步修訂版本化 wire contract。
  加入 A→C→D 多次轉移仍隔離、重複匯入冪等、其他正常家族仍可使用的案例。
- 反例：`review_quarantine_survives_export_into_replacement`。

### R2 — P1：已知分歧的來源仍可授權替換 registry

- 來源：PR #119 的 binding 檢查，與 PR #118 的 conflict 寫入整合不完整。
- 位置：`src-tauri/src/research/trial_ledger_binding.rs:150–175`；`trial_ledger_transfer.rs:1029–1095`。
- `check_binding` 只核對既有 checkpoint 的 chain 與 event 是否存在，未查 `registry_conflicts`。
  importer 只阻止本次有衝突的 checkpoint 寫入，既有 checkpoint 保留，既有 conflict 也未作為未來匯入的持久阻擋條件。
- 重現：先匯入來源 A 的正常歷史並保存 A 的 workspace binding；再匯入自洽但分歧的同 registryId 歷史。
  importer 正確記錄來源分歧，但用原 binding 呼叫 `check_binding` 仍回傳 `Current`。
  反例沿用既有來源分歧測試的完整重新計鏈方法，模擬同一 registry 備份分叉。
- 影響：§8.2「記錄 registry conflict 後不可再以該來源接受替換」未生效；曾經合法的 checkpoint 繼續授權已被標記不可信的來源。
- 修復要求：在同一 binding 讀取交易中查核來源衝突，永久拒絕該來源的替換授權；後續匯入也必須尊重已存在的來源衝突。
  加入「先接受、後分歧、再重播正常匯出／重開」仍拒絕的測試。
- 反例：`review_divergent_origin_cannot_authorize_replacement`。

### R3 — P2：`splitHash` 只包含版本字串，不包含實際切分輸入

- 來源：PR #120；PR #123 的 v3 walk-forward 整合沿用此事件身分。
- 位置：`src-tauri/src/discovery_runner/mod.rs:1775–1778`；legacy 回填亦見 `research/trial_ledger_workspace.rs:239–242`。
- hash 只編入 `config.contracts.split`、`config.contracts.embargo`，遺漏 `holdingAllowanceBars` 及實際 derived embargo／windows。
  v3 的 fold 宣告也不在 trial event payload 中；attempt 的 `configHash` 沒有進入事件身分。
- 重現：使用相同 candidate 與 dataset，只把 `holdingAllowanceBars` 加 1。
  兩個合法 v3 設定會得到不同的 derived embargo，登記函式以相同 request key 重送時卻成功重播同一個 event ID。
- 影響：已保存的 ledger split identity 不能識別實際切分；以五個身分欄位核對 reproduction 時，這一欄不能證明切分相同。
  **界線：**正常 command envelope 另有 payload 冪等檢查；不同 requestId 的現行 variant 仍會增加計數。
  反例直接測 register-before-enqueue 邊界，不宣稱一般 UI 已能用同 requestId 繞過 command 層。
- 修復要求：hash 由已驗證資料長度與 candidate 推導的切分／embargo，以及適用的 fold 宣告；new run 與 legacy recovery 使用一致契約。
  舊事件不可原地重寫；無法證明完整切分身分者不得作為免費 reproduction 依據。
- 反例：`review_changed_embargo_cannot_replay_registered_trial`。

### R4 — P2：已綁定工作區重新開啟時，不保存新觀察到的水位

- 來源：PR #120。
- 位置：`src-tauri/src/research/trial_ledger_workspace.rs:97–121`。
- `check_binding` 能回傳新的 head，但 `write_workspace_binding` 被包在 `if saved.is_none()` 內。
  已綁定的工作區只檢查，不更新；接受替換 registry 時也不立即保存新的 registryId。
- 重現：工作區初次綁定 seq=0；其他工作區加入一個事件；重新 `adopt` 後 `require_current` 回傳 seq=1，
  `app_settings.trial_ledger_binding` 卻仍為 seq=0。
- 影響：開啟時已觀察到的新增歷史沒有成為持久的回退界線，之後退回較舊但仍涵蓋原 binding 的 registry 可通過檢查。
  這與 §7.2 正常開啟「更新為目前鏈頭」、§7.3 替換成功後「改綁新 registry」不符。
- 修復要求：在 owner 控制的工作區交易中保存每次成功採納的 current binding；僅 legacy backfill 保持首次執行。
  加入 reopen 後回退檢出，以及替換後保存新 registryId 的回歸。
- 反例：`review_reopen_persists_newly_observed_registry_head`。

### R5 — P2：空 registry 的合法搬移無法恢復既有工作區

- 來源：PR #118–#119；PR #120 的開啟路徑使之成為工作區啟動失敗。
- 位置：`src-tauri/src/research/trial_ledger_binding.rs:150–175`；`trial_ledger_transfer.rs:430–435,1015–1027`。
- 合法空 binding 使用 seq=0／genesis hash；匯出 header 和 `registry_imports` 能記錄它，
  但 `origin_checkpoints` 只允許 seq≥1 且要求對應 event，空帳本自然沒有該列。
- 重現：A 首次開啟後尚無事件；保存 seq=0 binding；把 A 的完整正常匯出匯入 B。
  B 確認匯入成功，卻對 A 的原 binding 回傳 `RegistryReplaced`。
- 影響：沒有執行 discovery 的工作區，在換機或依匯出恢復帳本後仍無法開啟；這是合法狀態的可用性缺陷。
- 修復要求：定義並保存可驗證的 genesis／空來源匯入證據，讓 binding 檢查能採納它；
  不可直接無條件信任任何 caller 提供的 seq=0。另測空来源經多次 export/import 的可恢復性。
- 反例：`review_empty_export_can_restore_empty_workspace_binding`。

## Per-PR acceptance

「本次範圍可接受」只涵蓋該 PR 承諾的切片，不等同完整 P12、admission 或真實交易資格已完成。
本機測試針對整合 baseline；以下各 head 的 CI 是遠端結果，沒有聲稱逐個 checkout 重跑所有測試。

| PR | 最終 head | 本次結論與檢查重點 | CI run（六項全通過） |
| --- | --- | --- | --- |
| [#116](https://github.com/yoyoCadence/AlphaFactorForge/pull/116) | `41626c5` | 規格需追加修正：R1 的衝突轉移與 R5 的空來源恢復沒有完整定義；先前 R1–R5 Resolution 仍保留其歷史結論。 | [35866640218](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/35866640218) |
| [#117](https://github.com/yoyoCadence/AlphaFactorForge/pull/117) | `53a65be` | 本次範圍可接受：登記冪等性、protocol pin、append-only、當前計數；先前 benchmark attestation／pre-pin replay／Windows volume 修正仍在。 | [35875128834](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/35875128834) |
| [#118](https://github.com/yoyoCadence/AlphaFactorForge/pull/118) | `aaa44c5` | 需修復 R1，並與 binding 一起修 R2／R5。正常 union／去重／來源鏈／收據驗證通過，但不足以保留隔離狀態。 | [35879485911](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/35879485911) |
| [#119](https://github.com/yoyoCadence/AlphaFactorForge/pull/119) | `a700b07` | 需修復 R2／R5。missing／rollback／同 seq 分歧及一般非空替換檢查通過。 | [35935309674](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/35935309674) |
| [#120](https://github.com/yoyoCadence/AlphaFactorForge/pull/120) | `514daa7` | 需修復 R3／R4。migration、terminal attempt 補鏈、登記後入隊、claim、pre-P05 resume、service smoke 既有測試通過。 | [36008048000](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36008048000) |
| [#121](https://github.com/yoyoCadence/AlphaFactorForge/pull/121) | `96978d0` | 本次範圍可接受：96／97 bars 邊界、u128 長度計算、2–128 folds、Train containment、不足時無部分 folds。 | [36012507865](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36012507865) |
| [#122](https://github.com/yoyoCadence/AlphaFactorForge/pull/122) | `9fbb94a` | 本次範圍可接受：固定 params／DSL candidate、成本、prefix signal construction、outer holdout／later-fold 擾動隔離、完整 metrics／trades。 | [36063998213](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36063998213) |
| [#123](https://github.com/yoyoCadence/AlphaFactorForge/pull/123) | `a23c12f` | 本次新增計算／持久化範圍可接受：v3 嚴格設定、start／resume preflight、缺少 evidence 不完成 attempt、v1/v2 相容；沿用的 trial split 身分受 R3 影響。 | [36068695205](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36068695205) |
| [#124](https://github.com/yoyoCadence/AlphaFactorForge/pull/124) | `5146b02` | 本次範圍可接受：所有必要欄位與 nullable delisting、範圍／protocol 驗證、排序穩定性、每個 binding／policy 的 ID sensitivity、不可變 accessor。 | [36330444920](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36330444920) |
| [#125](https://github.com/yoyoCadence/AlphaFactorForge/pull/125) | `0bb1926` | 本次範圍可接受：精確 snapshot、全部 candle hash、dataset 範圍／minimum bars、listing／calendar、demo／degraded 拒絕、來源修訂與 snapshot content identity。 | [36564094600](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36564094600) |

## Verification

- 開始時工作區乾淨；原分支 `feat/p12d2a-authoritative-snapshot`。fetch 後將 `main` fast-forward 到 `af8196d`，再建立驗收分支。
- 對照範圍 `a56f12e..af8196d`：56 files，9,846 insertions／73 deletions；依各 PR 邊界追查新增行。
- `npm test -- --reporter=dot`：54 files、975 tests 通過。
- `npm run typecheck`、`npm run build`：通過。
- `cargo test --locked --quiet`：101 library + 352 desktop + 2 service smoke = **455 通過**。
- `cargo check --locked --all-targets`：通過。
- 五項反例暫時加入後，`cargo test --locked --bin alpha-factor-forge review_ -- --nocapture`：
  **新增五項全部失敗**，同名 filter 選中的既有四項通過；失敗皆落在預期行為斷言，沒有編譯或 setup 失敗。
- 暫時測試已反向套回，產品來源與 baseline 一致；回歸 patch 保留於下列連結。
- 遠端六個 jobs：typecheck、test、build、cargo-check、native-smoke、e2e，已逐 PR 查證其最終 head。
- 本機 Vitest／build 首次因 sandbox 的 esbuild `spawn EPERM` 失敗，提升該命令權限後通過；不是產品回歸。
- 本機未另跑 Playwright、封裝版 Tauri UI、真實 campaign 或 Windows 長期運轉；UI／native smoke 依上表 CI。
  未加入依賴，也未使用使用者真實 registry；反例使用獨立 temporary workspace／registry。

## Reproduction patch

[五項反例測試 patch](2026-09-29-pr116-125-acceptance-regressions.patch) 適用於 `af8196d`，
只增加測試，不含修復。於 repo root 套用，再進 Rust 目錄執行：

```powershell
git apply --check handoffs/2026-09-29-pr116-125-acceptance-regressions.patch
git apply handoffs/2026-09-29-pr116-125-acceptance-regressions.patch
Set-Location alpha-factor-forge/src-tauri
cargo test --locked --bin alpha-factor-forge review_ -- --nocapture
```

未修復 baseline 上預期為 5 failed、4 passed。完成觀察後回 repo root，以
`git apply --reverse handoffs/2026-09-29-pr116-125-acceptance-regressions.patch` 移除暫時測試；
若已同時修復程式，先檢查差異，避免把新工作一起覆蓋。

## Review Notes / Remaining scope

- P12d admission、ledger freshness transaction、未知歷史資格阻擋、campaign/run persistence、數值成本設定與 P12e/P13 confirmation
  已明確留待後續。沒有把它們誤列為 #124／#125 少做功能。
- 非有效 benchmark 缺乏 portable provenance 時拒絕跨 registry import，是 #118 明示的保守限制；本次不當成新回歸。
- `testsPerTrial = 1` 是 §19 明示的暫定 pin，不在此次擅自改動；未來使用不同 multiplicity 仍需要版本化處理。
- 次要文件落差：runtime 的目錄為 `com.alphafactorforge.trial-ledger`、debug seam 為 `AFF_TEST_TRIAL_REGISTRY_DIR`，
  但 §2.1 與未使用的 `default_registry_dir()` 仍寫 `com.alphafactorforge.evidence`／`AFF_REGISTRY_DIR`。
  後续 registry 修復應同步說明真正使用的路徑，避免操作人員備份錯檔；本次未把這個靜態觀察計入五項已重現缺陷。
- 建議下一步依 R1、R2、R3、R4、R5 順序拆成小修復，將對應測試納入正式 suite，再接 P12d admission。
  此次更新的 task board 只新增 Backlog 修復項，沒有擅自重排原產品任務。

## Resolution

Pending. 修復者請追加變更 commit、對應回歸結果與驗收結論；保留本次原始證據。
