# Handoff: PR #115 P12a 驗收與 P12 後續建議

Date: 2026-09-23
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12a-precision-precheck`
Reviewed commit: `9478a3d71d62fb8cc8cc9afd9026f62e467e49d1`
PR: [#115](https://github.com/yoyoCadence/AlphaFactorForge/pull/115)
Status: Needs one bounded API validation fix before acceptance; P12 remains In Progress.

## Summary

P12a 的合法輸入計算、AlphaBTC 反例、固定拒絕順序與範圍控制符合契約。尚未接入 runtime 的限制已明確揭露，不把 P12b–d 未實作當成本 PR 的缺陷。但公開 Rust evaluator 的防護有一個可重現漏洞：非法直接建構的計數可在回傳錯誤之前 panic。建議先補 R1 及回歸測試，再接受 P12a；不代表整個 P12 或研究資格流程完成。

本次只新增驗收文件及任務紀錄，沒有修改產品程式、提交 GitHub review 或合併 PR。

## Required Action / Decision

### R1 — Medium: 公開 evaluator 須在乘法前拒絕超界計數

Location: `alpha-factor-forge/src-tauri/src/discovery_core/precision.rs:209–222`; overflow at line 190.

`PrecisionPlan` 的欄位及 `evaluate_precision_plan` 都是 public。Evaluator 宣稱對不可能由 parser 產生的 plan 回傳錯誤，卻未先驗證 `prior_trials`、`planned_trials`、`tests_per_trial` 的上限。`family_tests` 先算 `(prior + planned) * testsPerTrial`，才檢查結果是否超界；三個 `u64::MAX` 的乘積超過 `u128`。

在未修改的產品程式上，以暫時 integration test 呼叫以下輸入，debug test 實際得到 `precision.rs:190:17: attempt to multiply with overflow`，而不是 `Err`：

```rust
let p = PrecisionPlan {
    alpha_ppm: 50_000,
    max_relative_standard_error_ppm: 200_000,
    prior_trials: u64::MAX,
    planned_trials: u64::MAX,
    tests_per_trial: u64::MAX,
    bootstrap_samples: 72_975,
    max_bootstrap_samples: 100_000,
};
let result = std::panic::catch_unwind(|| evaluate_precision_plan(&p));
assert!(result.is_ok()); // fails on reviewed commit
assert!(result.unwrap().is_err());
```

影響限制：JSON parser 已檢查個別上限，合法輸入不受此問題影響；目前沒有 runtime caller，因此不是現有 UI 可觸發的崩潰。問題在即將供 P12b/d 使用的公開 typed API。

要求：在乘法前檢查全部計數的契約範圍，或使非法 plan 無法經公開 API 建構；若仍保留任意計數運算，使用 checked arithmetic。補直接 API 的超界及 `u64::MAX` 回歸，確認回傳 `Err` 且不 panic，並保留合法最大值測試。無須改公式、schema 或 runtime。

## 圖片中的問題：建議決策（尚未批准實作）

### 1. 保留最嚴 Holm 門檻下的相對標準誤預檢

建議沿用 `p = alpha / m` 的規劃準則及目前三個拒絕原因。它回答抽樣解析度、Monte Carlo 精度及預算是否足夠，適合在執行前擋掉不可能判讀的計畫。20% 是 fixture 的宣告值，不能默默變成產品固定門檻；alpha 與精度限制須在讀取確認結果之前凍結。

須把 `ELIGIBLE` 解讀成「抽樣計畫可行」，不能當作策略有效、統計 power 足夠或樣本長度足夠。公式 `sqrt((1-p)/(pB))` 是獨立重抽樣下未平滑比例的 binomial 相對 SE 規劃量；對 `(1+extreme)/(B+1)` 不是精確的所有誤差描述，亦不處理 bootstrap 模型偏差。建議文件補充這個假設，不必因此改掉現有保守預檢。

本次額外以原始整數不等式驗證 36 組 family/alpha/SE 組合：回報需求值滿足不等式、少一個樣本不滿足；合法最大值亦正常拒絕而不 panic。2919 僅達解析度，72975 才符合本例 20% 精度條件。

Holm 控制 family-wise error 的性質可參照 [R 官方文件](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/p.adjust.html)。有限重抽樣 p 值與多重比較的注意事項可參照 [Phipson & Smyth 原論文](https://arxiv.org/abs/1603.05766)；這些來源不代表已驗證本專案尚未實作的 block-bootstrap。

### 2. P12b 帳本放工作區外，成為唯一權威來源

建議與 plan §4.4 的 evidence registry 邊界一致：本機應用資料目錄的獨立 SQLite registry；工作區 DB 只保存引用或查詢用投影。不要讓每個新 workspace 建立獨立的零計數家族。

- 明確定義穩定的 `familyId`、標準化商品／研究範圍、protocol 與 lineage。改 workspace 名稱、dataset hash 或重新匯入，不得自行產生「乾淨」家族；家族歸屬不明則停止資格判定。
- Append-only event 保存穩定 `trialId`、attempt/candidate reference、trial kind、批次、登記原因與來源。分類及是否計入須依事前協定，不能看結果後把失敗策略改稱 diagnostic 以減少計數。
- 首次暴露可影響選擇的結果之前，先持久化 trial 登記；確認批次預約與計數讀取在同一 registry transaction 內完成。重試同一登記需冪等；已登記試驗不因失敗／取消被刪除。
- `priorTrials` 由後端帳本推導，前端／CLI 不可傳入權威計數。跨工作區也須共用 registry 的併發控制；既有 workspace lock 不足以保護共用帳本。

### 3. 還原／重匯入採事件聯集，不取較大計數或覆蓋

以穩定事件／試驗 ID 去重合併；同 ID、不同 payload 應隔離並停止資格判定。不能只做 `max(localCount, importedCount)`，因為兩邊可能有各自不同的試驗。工作區舊備份不得覆蓋 registry；匯入的來源與完整性未知時標記未知並阻擋。

規格至少驗收：新 DB、改名／重匯入、舊備份還原、重複匯入、兩分支各新增試驗後聯集、併發預約、登記後崩潰及 registry 缺失／可辨識回退。離線系統不能保證偵測使用者把整台電腦全部歷史刪除或一致回退；不得宣稱防竄改，來源無法證明時不得沿用原資格。

### 4. 後續順序與資格閘門

沿用 P12b → P12c → P12d：先凍結帳本規格，再做 Train 內 walk-forward 與樣本可行性，最後由共用後端 admission 串起各預檢。桌面、CLI、headless service 須走同一條路。驗收應證明預檢失敗時沒有建立可執行的確認工作，而非只顯示警告。

Block-bootstrap／Holm 真正檢定、跨批次 alpha spending 與噪音偽陽性模擬目前未列成清楚的 P12a–d 交付項；建議在 P12/P13 交接前補有明確 owner 的小任務，不擅自啟動。特別要先說清楚 P12a `alphaPpm` 是此次確認所獲分配、family tests 如何對應帳本，避免把 campaign 總 alpha 在每批重新使用。這些能力完成以前，預檢通過仍不得產生正式統計確認 PASS。

## Verification

- Remote PR head 與本機 HEAD 相同；PR OPEN，非 draft。該 head 的 CI typecheck/test/build/cargo-check/native-smoke/e2e 六項全部 SUCCESS。
- 本機 `cargo test --locked --lib discovery_core::precision`：12/12 passed。
- 額外暫時測試：合法最大值及 36 組原始不等式驗證 passed；公開 evaluator 非法極大值測試 failed，重現 R1。暫時測試檔已移除；重現碼保留於上方。
- 本機 `npm.cmd test`：54 files / 973 passed（sandbox 初次 spawn EPERM，原命令於 escalation 後成功）。
- 本機 `npm.cmd run build`：passed，包含 `tsc --noEmit`。
- 本機 `cargo test --locked`：385 passed（87 library + 296 desktop/backend + 2 service smoke），doc tests 0，全部成功。
- 本機未重跑 Playwright 或原生桌面 UI；相關通過證據來自此 PR head 的遠端 CI。

## Resolution

Pending R1 fix and focused re-verification. P12b–d suggestions above are review recommendations, not completed features or authorization to implement.

### Resolution — R1 fixed (2026-09-23, same branch, PR #115)

- `evaluate_precision_plan` now re-checks **every** field against the §2
  domain before any arithmetic: `priorTrials ∈ [0, MAX]`, `plannedTrials`,
  `testsPerTrial`, `bootstrapSamples`, `maxBootstrapSamples ∈ [1, MAX]`
  (MAX = 2^53−1), plus the existing alpha/SE/budget checks.
- `family_tests` uses `checked_add`/`checked_mul` as a second guard, so even a
  direct call with raw `u64::MAX` counts returns the family-size error.
- Regression `direct_api_rejects_out_of_domain_counts_without_panicking`
  wraps the evaluator in `catch_unwind`: the reviewer's three-`u64::MAX`
  reproduction, and each count field at `MAX + 1` and `u64::MAX`, return
  `Err` without panicking; legal maxima (`bootstrapSamples = MAX`, family
  tests exactly `MAX` via either prior trials or tests per trial) are still
  accepted, and family `MAX + 1` still errors. The regression was run against
  the pre-fix production code and failed with
  `attempt to multiply with overflow`, then passed with the fix.
- Formulas, JSON contract, rejection order, fixture and runtime wiring are
  unchanged.
- Recommendation 1 accepted as documentation only: `docs/research-precision-v1.md`
  now states the binomial relative-SE planning assumption, that `ELIGIBLE` is
  sampling feasibility only and never a confirmation `PASS`, that alpha/SE are
  frozen before results are read, and that `alphaPpm` is the alpha allocated
  to this confirmation, not a reusable campaign total.
- Recommendations 2–4 (ledger outside the workspace as sole `priorTrials`
  authority, event-union restore/import, P12b → c → d order, explicit owners
  for block-bootstrap/Holm, alpha spending and noise simulation) are recorded
  as open task items in `tasks.md`; none is implemented or started here.

Verification: `cargo test --locked` **386 passed** (88 lib + 296 bin + 2
service smoke; +1 regression), precision module 13/13; clippy
`--all-targets` only the 5 pre-existing warnings; `rustfmt --check` on the
file passes. No frontend change, so Vitest/Playwright were not rerun locally;
remote CI reruns all six lanes on the pushed head.

Status: R1 fixed; awaiting re-review. P12 remains In Progress.
