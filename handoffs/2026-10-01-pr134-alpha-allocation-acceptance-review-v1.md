# Handoff: PR #134 alpha allocation acceptance review

Date: 2026-10-01
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e2-alpha-allocation`
PR: [#134](https://github.com/yoyoCadence/AlphaFactorForge/pull/134)
Reviewed head: `a8a7626a953fba99a819536bdb042898e5981082`
Base: `345203367ac5428a0445efd0aaadedd28c1fbd9b` (merged #133)
Status: R1 resolved on the PR branch (2026-10-01, see Resolution); awaiting re-review/merge. Pure allocation arithmetic accepted. The three P13 recommendations below were adopted by the maintainer on 2026-10-01 (see the second Resolution).

## Summary

PR 的 15 個變更檔案符合 P12e-2 範圍。整數 ppm 加總、直接建構宣告的重新驗證、等分捨去、耗盡、明細順序的內容身分及 P12e-1／P12a 的 alpha 銜接均通過檢查；沒有 runtime 呼叫者，也沒有 PASS。

發現一項 P2 合約文件問題：目前文件保證兩個 registry 各自預約「第 1 次確認」後，合併一定會因明細矛盾而停止。等分明細或相鄰份額相同時，只有金額的輸入無法提供這個保證。這不是本次純分配算術的錯誤，但會誤導 P13 的匯入／衝突實作，應在合併前釐清。

## Required Action / Decision

### R1 — P2: 金額前綴檢查無法保證偵測重複確認序號

位置：

- [合約 §9.5](../docs/research-alpha-allocation-v1.md#9-what-p13-must-do-this-module-cannot)，第 174–178 行。
- [原交接的設計選擇 1](2026-10-01-p12e2-alpha-allocation-v1.md#decisions)，第 26–32 行；PR body 也有相同保證。
- [實作](../alpha-factor-forge/src-tauri/src/discovery_core/alpha_allocation.rs)，第 276–299 行：輸入只有 `reserved: &[u64]`，只核對長度與逐筆金額。

已以這個 head 編譯的 Rust library 重現：

```rust
use alpha_factor_forge::discovery_core::alpha_allocation::{
    allocate_confirmation_alpha, AlphaAllocationDeclaration,
};

let declaration = AlphaAllocationDeclaration {
    total_alpha_ppm: 50_000,
    schedule: vec![16_666; 3],
};
// Registry A 與 B 各自預約 confirmation 1；保留兩筆後投影成金額。
let report = allocate_confirmation_alpha(&declaration, &[16_666, 16_666]).unwrap();
assert_eq!(report.confirmation_number, Some(3));
assert_eq!(report.alpha_ppm, Some(16_666));
```

實際結果是 `ELIGIBLE`，`spentAlphaPpm = 33332`，不是錯誤。合法的第 1、2 次確認也產生完全相同的金額輸入，因此純函式無法區分這兩種歷史。相鄰金額不同的對照組（明細 `[20000,15000,10000,5000]`，歷史 `[20000,20000]`）才會拒絕。

請在這個 PR：

1. 修正合約 §9.5 與 PR 說明；原 handoff 依既有規則追加 Resolution，說明金額前綴只能檢查金額，不保證來源、預約身分、序號唯一性或歷史完整性。
2. 明訂 P13 在把紀錄投影成金額前，須驗證家族／宣告綁定、預約身分與確認序號，並偵測衝突；不能把兩筆獨立的 confirmation 1 默默當成第 1、2 次。
3. 補一個邊界測試，記錄等分情況仍會配置第 3 份的現有行為及 P13 責任。保留合法等分的分配行為；不要為了讓錯誤的文件保證成立而拒絕所有相同金額。

P13 的資料表／匯入實作留在 P13，不需納入這個 PR。

## Review Notes

### 六個設計選擇的驗收意見

- 接受「金額歷史違反明細即錯誤」；其延伸的「重複序號必然被偵測」須依 R1 修正。
- 接受不回收，包括失敗／未完成的預約與等分餘數。
- 接受耗盡回報 `NOT_ELIGIBLE` / `alpha_budget_exhausted`。
- 接受宣告固定 `scope: trial-family`。
- 接受 `allocationId` 為宣告內容身分並包含明細順序。它不包含 familyId，P13 必須以家族鍵保存宣告，不能只用 allocationId 作為預算擁有者。
- 接受不另設確認次數上限；有效明細的每份至少 1 ppm，總長度自然受總預算約束。

### 給維護者的三項 P13 建議

1. **以實際分到的 alpha 重新預檢。** Campaign 的 alpha 是探索 admission 的規劃輸入；確認宣告須使用分配器給的份額，並在揭露前以最新 fenced family count 重跑 P12a。先使用凍結的抽樣數與上限；不足即阻擋確認。若要更改確認抽樣方案，須事前明確宣告並重新凍結，保留原 campaign 與有效確認設定的稽核關係，不能在看過結果後補抽樣或改 alpha。這是建議，待維護者決定與 P13 契約明訂。
2. **獨立預約衝突先阻擋該家族的後續確認。** 同一筆 reservation 的完全相同重播可去重／重用；同一序號但不同 reservation 的獨立確認，保留全部紀錄與衝突證據並停止配置。不能重新編號、刪除失敗紀錄、換宣告或清空預算。確認身分、來源證據、原子唯一性與冪等重試由 P13 設計。
3. **先維持每家族預算，明示保證範圍。** v1 沒有跨 instrument 的整體保證；若未來需要整體控制，先訂研究層總額並在家族間分配，再實施家族內明細。一般上界是 `min(1, Σ familyAlpha)`；N 個家族總額相同時為 `min(1, N × totalAlpha)`。不要把「每個家族 5%」描述成「整體研究 5%」。

### P12e-3 的下一步建議

- 事前固定合成資料模型、seed 規則、獨立模擬次數、樣本長度、block length、bootstrap 數、完整 familyTests、alpha 明細及驗收容忍度。
- 每次模擬以「同一家族的任一候選／任一檢定／任一確認批次曾誤拒」計一次，估計整個家族明細的偽陽性率；只看單批次或候選平均會漏掉跨批次效果。
- 報告同時保留單批次與整個明細結果、抽樣不確定性及事前門檻，涵蓋獨立與序列相關噪音。門檻或 seed 不得因失敗結果而更換。
- Alpha 加總的 union bound 以各批次的 p 值／檢定有效為前提；分配算術與 Holm 本身不證明 block bootstrap 在所有資料模型下有效。P12e-3 仍是必要驗收，P12 不因此整體結案。

## Verification

獨立執行於 reviewed head：

- `cargo test --locked`：514 通過（121 library + 391 desktop + 2 service）。
- `npm test`：1010 通過，58 個檔案。第一次沙箱內啟動因 esbuild `spawn EPERM` 失敗；同一命令獲准在沙箱外重跑並通過。
- `npm run typecheck`、`npm run build`、`cargo check --locked --all-targets`：通過。
- 新 Rust 檔的 `rustfmt --check` 與 `git diff --check`：通過。
- 額外文件保證探針：2 項，1 通過、1 失敗。失敗的是「等分時兩筆獨立 confirmation 1 必須拒絕」的文件斷言，支持 R1；上述原有 514 項測試均通過。探針以 `rustc --test` 連結本次 library，臨時檔已清除。
- 同一 head 的 [CI run 36873198114](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36873198114)：typecheck、test、build、cargo-check、native-smoke、e2e 六項均成功。
- 未重跑作者所述 mutation checks 或 clippy；未在本機重跑 Playwright，遠端 e2e 已成功。

本次只新增驗收 handoff 與 task board 記錄。產品程式未修改，PR 未合併；本地驗收紀錄尚未 commit／push／張貼到 GitHub。

## Resolution (2026-10-01)

R1 was acted on in PR #134, on top of the reviewed head `a8a7626`. This
handoff and its task board line were committed unchanged first (`eb05452`).

1. **Contract and PR description corrected.** Contract §3 now states what the
   prefix check does not establish, with the `[16666, 16666, 16666]`
   counterexample; §9 item 5 no longer claims a merged duplicate always
   stops allocation; the PR description was rewritten to match. The original
   handoff has a Resolution instead of an in-place edit.
2. **P13 responsibility made explicit** (§9 item 5): verify family/declaration
   binding, reservation identity and confirmation number on the stored
   records and detect conflicts before projecting to amounts; never pass two
   independent reservations of one number on as `k` and `k + 1`. §9 item 1
   adds the note from the review that `allocationId` does not contain the
   family, so the budget is keyed by family.
3. **Boundary test added:**
   `amounts_alone_cannot_tell_a_duplicated_confirmation_from_the_next_one`
   pins the current behaviour (third share allocated for the equal split, and
   for `[20000, 20000, 10000]`) and the contrast case that is refused.
   Legitimate equal splits are still allocated; no equal amount is refused.
   The function's doc comment carries the same warning.

The three P13 recommendations are recorded in contract §9.1 and marked as
recommendations, not adopted decisions. The cross-instrument bound in §10 now
uses the review's `min(1, Σ family totals)` wording. The P12e-3 advice is not
acted on here; it is input for that slice.

Verification after the fix: 515 Rust (122 + 391 + 2), all-target check,
clippy (five existing warnings), 1010 Vitest, typecheck and build pass. No
product behaviour changed.

## Resolution (2026-10-01) — maintainer decision on the three P13 recommendations

The maintainer adopted all three recommendations of "給維護者的三項 P13 建議"
as decisions: (1) confirm with the share actually allocated and re-run P12a
before reveal, blocking the confirmation when samples or budget are
insufficient and keeping the original campaign as the audit record; (2)
deduplicate a replayed reservation, but keep the evidence and stop the
family's further confirmations when one confirmation number has different
reservations; (3) keep per-family budgets, state that there is no overall
false-positive guarantee, and declare a separate research-level budget if an
overall bound is needed.

They are recorded as decisions in contract §9.1 and on the P13 row of the
task board. The finer points of the recommendations that the decision does
not spell out stay as input for the P13 contract. The P12e-3 advice is
unchanged: input for that slice.
