# Handoff: FU-1 帳本綁定的證據快照（trial-ledger §23）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/ledger-evidence-rollback`（從 `main` `5a7344c` 開出）
PR: opened from this branch
Status: 實作完成並通過本機驗證；等待 PR CI 與合併。關閉 `PR126-130-A-R1`（P2）與 `PR131-140-R1`（P3）。

## Summary

registry 中不新增事件就寫入的限制性證據——import 偵測到的永久隔離、來源分歧、檢定數升級——以前不受工作區綁定保護：
把 registry 檔還原成較舊複本時，事件鏈不變，綁定仍判定 `Current`，隔離消失、檢定數下降。
現在綁定帶著版本化、單調的證據快照（`trial-ledger-binding-v2`），還原較舊複本會得到新的
`registry_evidence_rolled_back`。依據是 [工作單 v2](2026-10-04-acceptance-followups-work-order-v2.md) §5.1 的方案，
以及維護者 2026-10-04 的決定（D1 (a)、D2、新增專用狀態碼）。

## 變更

- `src-tauri/src/research/trial_ledger_binding.rs`
  - `LedgerEvidence { quarantinedFamilies, conflictedOrigins, familyTests }` 與 `covers()`（集合包含＋逐家族數值下限；目前已隔離的家族滿足其檢定數）。
  - `LedgerBinding` 加上 `evidence: Option<LedgerEvidence>`；`None` 只代表從工作區讀回的舊格式綁定，不能寫入。
  - 儲存格式改為手寫的版本化解析：沒有 `version` 的舊格式精確讀取；v2 必須版本相符、欄位完整、集合排序且不重複；其他一律 `registry_state_invalid`。
  - 把事件前綴判定拆成共用的 `event_prefix()`：`check_binding` 在前綴成立後於同一讀取交易算出目前證據並比較；
    `fence_admission` 直接用 `AdmissionSnapshot` 的前綴，不再組出 `LedgerBinding`，也不塞空證據（依維護者提醒）。
  - 新增 `BindingCheck::RegistryEvidenceRolledBack` → `registry_evidence_rolled_back`。
- `trial_ledger.rs`：匯出 `LedgerEvidence`、`LEDGER_BINDING_VERSION`。寫入綁定的三個 production 位置（`adopt`、入隊、resume）不需修改：它們本來就寫 `check_binding` 回傳的值。
- 測試：7 個 `r23_*`（見下）；兩份驗收反例原樣改名納入，只有 A-R1 的中間斷言依 Codex 補正改寫，兩者的最終斷言都直接比對 `RegistryEvidenceRolledBack`。
- 文件：`docs/trial-ledger-v1.md` §7.2 加一列、新增 §23 實作紀錄。

## 狀態碼的區分（維護者 2026-10-04）

| 情況 | 狀態碼 |
| --- | --- |
| registry 最大事件序號小於已保存序號 | `registry_rolled_back` |
| 事件前綴仍成立，但隔離／衝突證據消失，或檢定數低於已保存高水位 | `registry_evidence_rolled_back` |

兩者都阻擋資格流程並保留最後一個有效綁定，差別只在失敗原因。

## 仍需注意

- **降級**：寫入 v2 之後，舊版程式讀不懂綁定，會拒絕開啟工作區（fail closed）。本機驗證時，`--bin` 只重建桌面程式、
  留下舊的服務執行檔，`host::the_executable_launcher_…` 因此失敗——正好印證這一點；完整 `cargo test` 兩個 binary 一起重建後通過。
- **保護邊界**：只保護已持久保存的觀察；兩個寫回點之間新增的證據要到下一次寫回才受保護。目前沒有匯入命令；
  未來加入時必須在同一個 owner 交易中寫回綁定（§23 已寫成規則）。registry 與工作區一起還原仍無法偵測。
- `fence_admission`、`read_admission_count`、匯入／匯出目前仍沒有 production 呼叫者；它們的行為由測試鎖定。

## Verification

- `cargo test --locked`：**168 library（1 ignored）+ 403 desktop + 2 service smoke = 573 通過**（desktop 396 → 403，新增 7）。
- 7 個突變全部被抓到（每個都有測試實際 FAILED，不是編譯失敗）：快照比較恆真、忽略分歧來源、取消隔離優先、忽略升級、接受任何版本、允許無證據寫入、接受未排序集合。
- `cargo clippy --locked --all-targets`：只有既有 5 個 warning。
- rustfmt：`trial_ledger_binding.rs` 整檔乾淨（原本也乾淨）；其他三個檔案原本就有格式漂移，只格式化本次新增的測試區段。
- 沒有 TypeScript、UI、e2e 或 schema 變更。

## Resolution

Merged as PR #143（merge commit `398c226`，2026-10-04；六項 CI 通過）。
