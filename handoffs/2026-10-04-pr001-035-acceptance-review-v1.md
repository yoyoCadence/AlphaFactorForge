# Handoff: PR #1–#35 整合驗收（往前驗收 batch J）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`
Status: 驗收完成；35 個 PR 都可以接受。一項 P3 防禦性強化（J-R1，code 模式直譯器的函式白名單查詢會走到原型鏈；目前仍會被拒絕，沒有可利用的路徑）。

## Summary

這 35 個 PR 是專案最早期：Phase A 回測摘要保存與 App 圖示（#1）、handoff 機制（#3）、UI 移植的各個 slice
（回測管線、params／blocks／code 三種策略模式、K 線圖、Holdout、參數掃描與熱力圖、圖表標記與游標、replay、浮動與原生視窗、報告匯出、SQLite 策略庫），
以及 2026-07-07 稽核的文件與第一批修正（#32–#35）。
35 個合併 commit 的 tree 都與各自的最終 head 相同，所有 head 的 CI 都通過。只有 #2、#7、#12、#16 有 PR 留言，都是「依 review 修正完成」的紀錄。

這一段的程式碼大多已被後續 PR 重構或強化（BacktestPanel 拆分、結果與掃描綁定 context、參數驗證、對帳修正、Rust parity），
所以本次不逐一重讀舊 diff，改為檢查仍在使用、且屬於 AGENTS.md 高風險面的兩處：code 模式直譯器（#10），以及原生視窗的權限（#30）。

## Required Action / Decision

### J-R1 — P3：code 模式直譯器用 `in` 檢查函式白名單，原型鏈上的名稱會通過第一道檢查

- 來源：PR #10（Slice 4b-1，安全直譯器）。
- 位置：`alpha-factor-forge/src/services/exprInterpreter.ts:228` `if (!(name in FN_ARITY)) fail(...)`。
  `in` 會沿著原型鏈查找，所以 `constructor`、`toString`、`valueOf`、`hasOwnProperty`、`__proto__` 都會被當成「已知函式」。
- 實際行為（暫時腳本實測）：這些名稱都**仍然被拒絕**，但原因是下一行 `args.length !== FN_ARITY[name]`：
  取回的「arity」是函式或物件而不是數字，比較永遠不相等。錯誤訊息因此變成
  `"constructor" expects function Object() { [native code] } argument(s), got 1`，把原生函式的文字帶進錯誤訊息。
  變數名稱用的是 `Set`，所以 `__proto__`、`constructor` 當變數會正常回報 unknown variable；成員存取與索引在 tokenizer 就被拒絕。
- 影響：目前沒有可利用的路徑——安全性依賴的是一個「碰巧」成立的型別不符，而不是白名單本身。
  code 模式是 AGENTS.md 明訂的「只能人工使用、AI 不可觸及」的邊界，值得讓白名單本身就正確。
- 修正：改用 `Object.hasOwn(FN_ARITY, name)`（或把 `FN_ARITY` 改成 `Map`／`Object.create(null)`），並加一個測試，
  斷言 `constructor(price)`、`toString()`、`__proto__(price)` 都以 `unknown function` 被拒絕。

## 本次其他檢查

| 對象 | 結果 |
| --- | --- |
| #10 直譯器的其餘部分 | 只有 tokenizer → 遞迴下降 parser → 白名單 AST；沒有 `eval`／`new Function`（檔頭註明有單元測試掃描）；長度、節點數、深度都有上限；`prev`／`crossUp`／`crossDown` 不可巢狀，最多回看 1 根。 |
| #30 原生視窗權限 | `src-tauri/capabilities/` 只有三個檔案，全部只授權 event 的 listen／unlisten／emit-to，且各自限定視窗（`main`、`chart-popout-window`、`metrics-popout-window`）；沒有 fs、shell、http 等 plugin。 |
| CSP | `default-src 'self'; script-src 'self'`，沒有 `unsafe-eval`；`style-src` 允許 `unsafe-inline`（React inline style 所需）。 |
| #13 dataClient seam | mock 只在 `import.meta.env.DEV` 且有 `?mock` 時啟用；production bundle 不含 mock 專屬字串（已在 #131–#140 驗收以搜尋確認）。 |
| #27／#31 策略庫載入 | 載入時以 `OPERAND_IDS` 等白名單驗證規則（`strategyLibrary.ts:44`），舊版未保存規則的策略走明確的相容路徑；batch G 的 G-N1 也因此不可觸發。 |

## Per-PR acceptance

全部 35 個 PR：tree 與最終 head 相同、CI 通過、本次範圍可接受。逐一的 head／merge／CI run 如下（連結格式 `https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/<id>`）：

| PR | head → merge | CI run | PR | head → merge | CI run |
| --- | --- | --- | --- | --- | --- |
| #1 | `2276ec7` → `152e0a7` | 28313908963 | #19 | `dadfb2a` → `f017635` | 28515481305 |
| #2 | `fc11abf` → `cd80091` | 28316467127 | #20 | `ea7cf84` → `d482754` | 28518456475 |
| #3 | `0bfa05d` → `7f554ff` | 28321421128 | #21 | `bf0d25e` → `524e5f0` | 28519496673 |
| #4 | `dabb696` → `496d3b8` | 28321797226 | #22 | `2973ab9` → `86a4d36` | 28520952765 |
| #5 | `f0c505e` → `ab5af06` | 28322597596 | #23 | `5ecdf97` → `8f58b1d` | 28522508810 |
| #6 | `a92bbf8` → `674ac0f` | 28323061773 | #24 | `83cf7bd` → `e8ee8de` | 28524029541 |
| #7 | `4c422e8` → `1085376` | 28325579832 | #25 | `a3d12ea` → `1400812` | 28787467683 |
| #8 | `c929903` → `128aa82` | 28326134137 | #26 | `0d9fe5a` → `87fbd82` | 28788251265 |
| #9 | `c397040` → `67e9b3a` | 28326590414 | #27 | `157f4c7` → `a36159f` | 29140899480 |
| #10 | `512fdd7` → `165c12f` | 28345092019 | #28 | `76d7c10` → `66a50f2` | 29143156393 |
| #11 | `88e8d9d` → `856e20a` | 28345422494 | #29 | `068d3f4` → `0e08046` | 29144948234 |
| #12 | `d52df5e` → `ecef132` | 28348234631 | #30 | `c5ec41b` → `68607dd` | 29154116535 |
| #13 | `da5a561` → `38d29c0` | 28360644206 | #31 | `93714d2` → `bf0b445` | 29155413574 |
| #14 | `0707589` → `20c5d6d` | 28365029074 | #32 | `a47feb9` → `61d4b52` | 29169284100 |
| #15 | `3003475` → `77e733d` | 28372850278 | #33 | `4a94866` → `97c5b02` | 29170023813 |
| #16 | `dc83430` → `4e8eadd` | 28376864218 | #34 | `acabffa` → `4407fb2` | 29170729163 |
| #17 | `7ca5b95` → `686cf1b` | 28378527215 | #35 | `b9c2a70` → `d6dca15` | 29171755158 |
| #18 | `40b7bb8` → `ec56610` | 28513711564 | | | |

## Verification

- 35 組 `git diff <head> <merge>` 都是空的；各 head 的 CI 都是 success。
- J-R1 以暫時的 vite-node 腳本實測後已刪除，工作區與 baseline 相同。
- 全套測試沿用整合 baseline 的結果（見 [#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)）。

## Resolution

Pending for J-R1 (optional hardening).

## Follow-up work order（2026-10-04 追加）

本檔的發現已整理進 [驗收後續工作單](2026-10-04-acceptance-followups-work-order-v1.md)，請接手的 agent 從那裡開始：

- J-R1 → **FU-4**，不需要決定，可直接實作。

### Resolution — FU-4（2026-10-04）

J-R1 已在 `fix/expr-whitelist-own-property` 修正：`exprInterpreter.ts` 的函式白名單改用 `Object.prototype.hasOwnProperty.call(FN_ARITY, name)`（專案型別庫為 ES2020，不用 `Object.hasOwn`）。新測試斷言 `constructor`、`toString`、`valueOf`、`hasOwnProperty`、`__proto__` 都以 `unknown function` 被拒絕且訊息不含 `native code`；修正前失敗、修正後通過。
