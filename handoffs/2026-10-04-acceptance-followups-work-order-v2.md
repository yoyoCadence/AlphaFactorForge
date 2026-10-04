# Handoff: PR #1–#140 驗收後續工作單 v2（決定已確認，可執行版）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Supersedes: [工作單 v1](2026-10-04-acceptance-followups-work-order-v1.md)（保留作歷史；本版取代其中的建議欄與被修正的細節）
Inputs: [Codex 方向確認與實作補正](2026-10-04-acceptance-followups-review-v1.md)、[驗收總覽](2026-10-04-pr001-130-acceptance-index-v1.md)
Baseline: `5b4a562`（若 `main` 前進，開工前重新確認行號）
Status: **決定已確認（D1–D7）**，依 §3 的順序執行。FU-1 與 FU-9 先以本文件的 Mode A 方案送審，核可後實作；其餘可直接實作。產品修正尚未開始。

## 0. 這一版改了什麼

1. **D1–D7 已有答案。** 維護者在 2026-10-04 的會話中，以 Codex 的方向確認作為 D1–D7 的答案，沿用既有授權；接手者**不要重問**同一選項（§2）。
2. **納入 Codex 的實作補正**：FU-1 的快照範圍、寫回時機與相容性；FU-1 反例的中間斷言例外；FU-3 拆成三個 PR 並保留 `vite-node`；FU-4 改用 ES2020 相容寫法；FU-5 驗證 DB 鎖等待；FU-6／FU-7 的測試範圍；FU-9 的逐列狀態。
3. **原始工作單的兩處錯誤已更正**（§4）：FU-4 的 `Object.hasOwn` 不符合專案的 ES2020 型別庫；A-R1 的「可觸發性」寫得太強。
4. **新增 FU-1 的 Mode A 方案**（§5.1）與 FU-9 的方案大綱（§5.9），以及 PR 切片與順序（§3）。

## 1. 接手方式

- 先讀 `AGENTS.md`。一個切片一個 PR，從最新的 `main` 開分支；`main` 要求分支與最新 main 同步，前一個 PR 合併後再開下一個。
- 開工前把 tasks.md **Next** 中對應的項目移到 In Progress；完成後移到 Done，並在原 batch handoff 與本文件 §7 追加結果（handoff 只能追加）。
- FU-1、FU-9：先把本文件的方案（必要時補充細節）送審，**取得核可後才改程式**；若方案需要擴大成 registry schema、匯出契約或整批 DB 非同步重構，另列範圍變更，不要在同一個 PR 夾帶。
- 本機環境（Windows、repo 在 OneDrive 下）：Rust 用 `CARGO_TARGET_DIR` 指到 OneDrive 之外；Playwright 用 `--workers=1`、`E2E_PORT=5199`，冷啟動先手動載入一次；`python` 是 Store 空殼，腳本化編輯用 `node -e`；Claude Code 呼叫 `git push`／`gh` 時加 `env -u GITHUB_TOKEN`（Codex 依 AGENTS.md §7）。
- 不要做：`npm audit fix`／`--force`（`docs/security-audit-npm.md` 禁止）；為了讓 audit 數字歸零而把間接依賴改成直接 devDependency；提高 parity 容差掩蓋差異；執行最終統計驗收或使用保留 seed 20261117。

## 2. 已確認的決定

| ID | 決定 | 必須保留的條件 |
| --- | --- | --- |
| D1 | **(a)**：workspace binding 加入**版本化、單調**的證據快照 | 至少涵蓋：已隔離家族集合、已知衝突來源 registry 集合、每個家族曾觀察到的最高有效 `testsPerTrial`（高水位）；同一 registry 與替換 registry 都要檢查。 |
| D2 | 隔離消失與檢定數下降**一起修**，不只補文件 | 同一個 FU-1；在 P13 使用帳本計數前完成；保留 `fence_admission`。 |
| D3 | AGENTS.md 改為指向 `alpha-factor-forge/src-tauri/Cargo.toml` 的 `rust-version` | README 安裝說明可寫目前最低版本 1.89，並註明以 manifest 為準；與 FU-8 同一個文件 PR。 |
| D4 | 四個間接依賴先在相容範圍內更新；**Vitest 另開 PR 升到 4.1.11** | 4.1.11 是 GHSA-82fw-gwwq-j7x9 的最低修補版本，不直接跨到 5.x；開工時重新核對公告、audit、engines 與 peers。 |
| D5 | CI 加 `npm audit --omit=dev` | 明寫只守 production 依賴；完整 audit 的定期追蹤另列小任務，頻率與失敗政策在該 CI 切片寫清楚。 |
| D6 | 已保存 campaign **逐列顯示狀態**，正常列照常顯示，不相容列禁止啟動 | 區分「版本不相容」與「資料損壞／身分不符」；保留原 ID、原始宣告與歷史結果；後端繼續拒絕不合法啟動。 |
| D7 | FU-6、FU-7 要做，但優先度低於 FU-1、FU-5 與依賴修補 | FU-7 要改 Rust 測試為逐案執行，不能只增加案例數。不重排 tasks.md 既有的產品任務。 |

## 3. 執行順序與 PR 切片

| 順序 | 切片 | 內容 | 建議分支 | 前置 |
| --- | --- | --- | --- | --- |
| 1 | **FU-1-design** | §5.1 方案送審（只有文件；可直接在本 PR 審查，或另開文件 PR） | — | — |
| 2 | **FU-1** | 帳本證據快照實作（P2＋P3） | `fix/ledger-evidence-rollback` | FU-1-design 核可 |
| 3 | FU-4 | 直譯器白名單（小） | `fix/expr-whitelist-own-property` | — |
| 4 | FU-5 | campaign 預覽移出主執行緒 | `fix/campaign-preview-async` | — |
| 5 | FU-3a | 四個間接依賴 | `chore/dev-deps-audit-2026-10` | — |
| 6 | FU-3b | Vitest 4.1.11 | `chore/vitest-4` | FU-3a 合併 |
| 7 | FU-3c | CI 加 `npm audit --omit=dev` | `ci/npm-audit-production` | FU-3a 合併 |
| 8 | FU-2＋FU-8 | 文件：Rust 版本、README 進度 | `docs/rust-msrv-and-progress` | — |
| 9 | FU-6 | embargo 推導 throw | `fix/embargo-invalid-operand` | — |
| 10 | FU-7 | 指標 fixture 邊界案例 | `test/indicator-fixture-edges` | — |
| 11 | FU-9-design → FU-9 | 已保存 campaign 逐列狀態 | `feat/campaign-row-status` | 在下一次升級 campaign 釘選的合約版本**之前** |

- FU-1 是最高優先，但**不阻擋 P12e-7b**（那是純模擬，不讀帳本）；它必須在 **P13 使用帳本計數之前**，以及**任何帳本匯出／匯入命令上線之前**完成（見 §4 第 2 點）。
- 順序 3–10 彼此獨立，可依維護者的時間調整；FU-3b、FU-3c 需要 FU-3a 先合併。

## 4. 對原始發現的更正

1. **FU-4 的修正方式寫錯了**（Codex 指出，已核對）：`alpha-factor-forge/tsconfig.json:5` 的 `lib` 是 ES2020，`Object.hasOwn` 是 ES2022 API，照原工作單做 typecheck 會失敗。
   改用專案已有四處採用的 `Object.prototype.hasOwnProperty.call`（`core/market-data/foundation.ts:62`、`services/discoveryConfig.ts:358`、`services/validationRecord.ts:595`、`theme/theme.ts:752`）。
2. **A-R1 目前在產品中無法觸發**（本次重新核對）：`import_json_lines`、`export_json_lines`、`fence_admission`、`read_admission_count` 在 production 程式碼中**沒有任何呼叫者**，只在測試中使用；帳本匯出／匯入命令尚未上線。
   因此「import 偵測到的隔離被還原舊檔洗掉」是**潛在的契約缺陷**：一旦加入匯入命令就會成真。它仍列為 P2，因為 §20 承諾隔離永久有效，而且修正必須早於匯入命令與 P13。
   相對地，PR131-140-R1（檢定數升級）在目前的產品**可以觸發**：runner 的 `register_lineage_with`、`register_missing_lineage`、`backfill` 都可能重播舊版以 1 登記的批次，形成不新增事件的升級，只是路徑很窄。
   原 batch handoff 的描述保留，這裡是更正紀錄。

## 5. 工作項目

### 5.1 FU-1（P2）帳本證據快照：Mode A 方案（送審用）

對應 Backlog：`PR126-130-A-R1`、`PR131-140-R1`。證據：[A-R1 反例](2026-10-04-pr126-130-acceptance-regressions.patch)、[PR131-140-R1 反例](2026-10-04-pr131-140-acceptance-regressions.patch)（在 `5b4a562` 上都是 1 failed）。

**(1) 資料形狀。** 工作區 `app_settings.trial_ledger_binding` 的 JSON 由
`{registryId, seq, chainHead}`（`trial_ledger_binding.rs:20–27`，`deny_unknown_fields`）擴充為版本化格式：

```json
{
  "version": "trial-ledger-binding-v2",
  "registryId": "…", "seq": 12, "chainHead": "…",
  "evidence": {
    "quarantinedFamilies": ["trial-family-v1:…"],
    "conflictedOrigins": ["<registry id>"],
    "familyTests": { "trial-family-v1:…": 2 }
  }
}
```

陣列排序、去重；`familyTests` 的值是曾觀察到的**高水位**（不是允許的上限）。大小受家族數與來源 registry 數限制。

**(2) 證據分類**（實作時要用測試或理由逐項確認）：

| 證據 | 遺失時的效果 | 是否放進快照 |
| --- | --- | --- |
| `family_conflicts`（隔離） | **放寬資格**（隔離消失） | 是：`quarantinedFamilies` |
| `family_protocol_upgrades`／有效 `testsPerTrial` | **放寬**（m 變小） | 是：`familyTests` |
| `registry_conflicts` | **放寬**（分歧來源重新能授權替換） | 是：`conflictedOrigins` |
| `origin_checkpoints`、`origin_genesis` | 預期只會讓替換證明失敗而**被拒絕**（較嚴） | 否；但要寫測試證明遺失時是拒絕 |
| `registry_imports` | 匯入紀錄，不影響資格 | 否；在 handoff 寫明理由 |

**(3) 檢查語意。** 在 `prefix_check`（`trial_ledger_binding.rs:131–165`）**同一筆 registry 讀取交易**內：先做既有的鏈頭檢查，再由 registry 表算出目前證據，要求
`saved.quarantinedFamilies ⊆ current`、`saved.conflictedOrigins ⊆ current`、每個 `saved.familyTests[f] ≤ current[f]`（`f` 不存在也算違反）。
不能只比較整份證據的 hash（正常新增證據也會改變 hash）。違反時回傳新的 `BindingCheck::RegistryEvidenceRolledBack`（代碼 `registry_evidence_rolled_back`，方便診斷；若審查者偏好沿用 `RegistryRolledBack`，在審查時決定）。
替換 registry（不同 `registryId`）在既有 origin checkpoint 檢查通過後，**同樣**要求包含快照中的證據；替換不得清空已有證據。

**(4) 寫回時機。** `check_binding` 回傳的 `Current(LedgerBinding)` 帶著**目前**證據。production 中所有寫入 binding 的地方都直接寫這個值，因此不需另外修改：

| 寫入點 | 位置 | 時機 |
| --- | --- | --- |
| 開啟工作區 | `trial_ledger_workspace.rs:94–138` 的 `adopt`（由 `runtime/mod.rs:201` 呼叫） | 每次成功採納（R4 已是每次寫回） |
| 啟動 run | `db/discovery.rs:832`（入隊交易） | 寫 `register_lineage_with` 登記後 `require_current` 的結果（`discovery_runner/mod.rs:2052`） |
| 繼續 run | `db/discovery.rs:995`（resume 交易） | 寫 `register_missing_lineage` 的結果（`trial_ledger_workspace.rs:226`） |

只檢查、不寫入的呼叫：`discovery_runner/mod.rs:512`（逐候選 claim／commit 檢查）、`:2014`、`trial_ledger_workspace.rs:156`（登記前檢查）。
**保護邊界要寫進文件**：只有已持久保存的觀察受到保護；兩個寫入點之間新產生的證據，要到下一次寫入才受保護。
目前沒有匯入命令，所以 production 中不會在開啟期間新增隔離；**未來加入匯入命令時，必須在同一個 owner 交易中寫回新的 binding**（請在 FU-1 的文件中寫成規則）。
順序維持既有規則：registry 先 commit，workspace 後寫入。

**(5) 相容性。**
- 精確辨識舊版 `{registryId, seq, chainHead}`（沒有 `version`）：視為「證據未知」，鏈頭照常檢查；第一次成功採納時寫回 v2。舊版沒有留下的歷史證據**無法追溯復原**，在文件中寫明。
- 未知 `version`、缺欄位或損壞的 v2 一律拒絕（`state_invalid`），**不得退回空快照**。
- 檢查失敗時不得覆蓋最後一個有效 binding（沿用現有行為）。

**(6) 不變的部分與限制。** 不改 registry schema、事件計數語意與匯出格式；`fence_admission` 與 P13 的最終 admission 同步要求不變。
registry 與 workspace 不是單一原子交易：兩者**一起**還原到舊狀態時仍無法偵測，寫進限制。

**(7) 測試。**
- 兩份反例納入正式測試。**例外**：A-R1 反例第 47 行 `assert_eq!(after, observed, "the quarantine did not move the head")` 要改成只比較 `seq`／`chainHead` 不變，另外斷言快照已包含該家族（加入快照後，隔離本來就應該改變 binding）。兩份反例最後的「還原舊檔被擋下」「檢定數 2 → 1 被擋下」**不得弱化**。
- 增補：同一 registry 事件不變但證據增加（`Current`，快照擴大）／遺失（被擋）；衝突來源消失（被擋）；舊格式升級；未知版本與損壞 v2（拒絕）；帶完整證據的正常替換與多跳轉移（`Current`）；替換缺少證據（被擋）；重複開啟；**經過正式寫回點後**重新開啟再回退（被擋）；`origin_checkpoints`／`origin_genesis` 遺失時是拒絕而不是放寬。
- 既有 R1–R5、A1–A32 回歸全部通過。

**(8) 文件。** `docs/trial-ledger-v1.md`：§7（binding 格式、檢查與寫回規則）、§20、§22 補充，並新增一節實作紀錄（含保護邊界與限制）。

**驗收與驗證。** 兩份反例在修正前失敗、修正後通過；上述測試通過；
`cd alpha-factor-forge/src-tauri && CARGO_TARGET_DIR=/c/tmp/aff-target cargo test --locked`；`cargo clippy --locked --all-targets` 只有既有 5 個 warning。

### 5.2 FU-2＋FU-8（P3＋低）文件：Rust 版本與 README 進度

- 現況：`Cargo.toml:8` 為 `rust-version = "1.89"`；`README.md:148,330,502`（三語 1.77.2+）、`alpha-factor-forge/README.md:39`（≥ 1.77）、`AGENTS.md:12`（1.77+）。README 進度段落停在 P11（`c1391f2`）。
- 要做：AGENTS.md 改為指向 `Cargo.toml` 的 `rust-version`（D3）；README 安裝說明寫 1.89 並註明以 manifest 為準；README 進度補上 P12 的現況與仍待完成的階段，並以 tasks.md 為狀態來源（併入 `DOC-STATE-002`）。
- 驗收：`grep -rn "1\.77" README.md AGENTS.md alpha-factor-forge/README.md` 沒有結果；只有文件改動。

### 5.3 FU-3a／FU-3b／FU-3c（P3）依賴修補與 CI

| 套件 | 目前 | 由誰引入 | advisory 範圍 | 需要 | 切片 |
| --- | --- | --- | --- | --- | --- |
| postcss | 8.5.15 | `vite@6.4.3` | ≤ 8.5.22 | > 8.5.22 | FU-3a |
| nanoid | 3.3.15 | postcss | < 3.3.18 | ≥ 3.3.18 | FU-3a |
| browserslist | 4.28.4 | `@vitejs/plugin-react` → `@babel/core` → `@babel/helper-compilation-targets` | ≤ 4.28.6 | > 4.28.6 | FU-3a |
| baseline-browser-mapping | 2.10.40 | browserslist | < 2.11.0 | ≥ 2.11.0 | FU-3a |
| vitest／@vitest/mocker | 3.2.6 | 直接依賴 | < 4.1.11 | 4.1.11 | FU-3b |

- **FU-3a**：在原相容範圍內更新 lockfile（例如逐一 `npm update <pkg>` 後核對實際版本），**逐行審查 `package-lock.json`**；不把四個套件新增成直接 devDependency；`npm ls` 不得出現 invalid peer。
- **FU-3b**：Vitest 3.2.6 → 4.1.11（GHSA-82fw-gwwq-j7x9 的最低修補版本，3.x 沒有回補）。Vitest 4 要求 Vite ≥ 6、Node ≥ 20（本專案 Vite 6.4.3、CI Node 20），實作時核對目標套件的 engines／peers。
  Vitest 4 不再依賴 `vite-node`，但本專案直接依賴 `vite-node@3.2.4`（`package.json:44`），14 個 `fixtures:*` 指令都用它：**保留該直接依賴**，並執行代表性的 fixture 產生指令，確認結果可重現（例如 `npm run fixtures:indicators`、`fixtures:backtest` 後 `git diff` 只有預期的來源 hash 變化或沒有變化）。不得執行最終統計驗收或使用保留 seed。
- **FU-3c**：CI 加 `npm audit --omit=dev`，在 workflow 中註明只守 production 依賴；完整 audit 的定期追蹤另列小任務。
- 驗收：FU-3a 後 `npm audit` 只剩 vitest／@vitest/mocker；FU-3b 後兩種 audit 都為 0；每個切片都跑 `npm run typecheck`、`npm test`、`npm run build`、`npx playwright test --workers=1`、`npm ls`；Vite dev server 仍只綁本機；完成後在 `docs/security-audit-npm.md` 追加處理紀錄（保留禁止 `npm audit fix` 的規定）。

### 5.4 FU-4（P3）直譯器白名單只看自身屬性

- 位置：`alpha-factor-forge/src/services/exprInterpreter.ts:228` `if (!(name in FN_ARITY))`。
- 要做：改為 `if (!Object.prototype.hasOwnProperty.call(FN_ARITY, name))`；不升級全域 target／lib，不改用 `Map`。
- 測試：在 `src/services/exprInterpreter.test.ts` 的「security: rejects everything off-whitelist」區塊，斷言 `constructor(price)`、`toString()`、`valueOf()`、`hasOwnProperty(price)`、`__proto__(price)` 都以 `unknown function` 被拒絕，且錯誤訊息不含 `[native code]`。
- 驗收：新測試修正前失敗、修正後通過；`npm test`、`npm run typecheck` 通過。

### 5.5 FU-5（P3，`DB-ASYNC-001` 的第一個切片）campaign 預覽移出主執行緒

- 位置：`src-tauri/src/commands/campaign_commands.rs:115`（同步命令；Tauri v2 文件：同步命令在主執行緒執行）；每個 instrument 都載入並重算整個 dataset（`research/campaign_snapshot.rs:183–185`），期間持有共享 DB mutex。
- 要做：改成 `async` + `tauri::async_runtime::spawn_blocking`（同檔 `freeze_research_campaign` 的寫法），參數名稱仍為 `declaration`。
- 驗收：`campaignCommands.test.ts` 的參數釘選照常通過；`cargo check --locked --all-targets`、`cargo test --locked`、`npm test` 通過。
  加上受控慢操作驗收：預覽未完成時視窗仍可互動，並觸發一個代表性的共享 DB 讀取路徑；**若出現鎖等待，記錄觀察結果**，擴大到其他命令時作為 `DB-ASYNC-001` 的明確後續切片。
  不得把「預覽已 async」寫成 `DB-ASYNC-001` 已完成。O3 native smoke 保留。

### 5.6 FU-6（低）`deriveEmbargoBars` 對無效左運算元 throw

- 位置：`src/services/embargo.ts:59` 的 `operandLookback` 沒有 default。
- 要做：加 default 分支並 throw `RangeError`；加 TS 單元測試（`'50'`、`'atr'`、`' maFast '`）。
- Rust embargo 只接受已驗證的 params 投影，blocks／code 推導只在 TS：**不要**把 blocks 左運算元錯誤塞進 params-only 的 Rust parity fixture。只有真正改到共用 fixture 案例時，才重新產生並驗證 Rust 端。

### 5.7 FU-7（低）指標 parity fixture 邊界案例

- 要做：在 `src/parity/indicatorFixture.ts` 保留現有 seed-42 案例，新增平盤、長度 1、週期 1、週期大於長度；輸入使用整數或二進位可精確表示的小數（例如 100、100.5），避免 `serde_json` 差 1 ulp 造成假差異；`npm run fixtures:indicators` 重新產生。
- **Rust 測試必須改成逐案執行所有指標檢查**（目前只驗 `fixture.cases[0]`，`indicator_parity_tests.rs:147,150`），確認 warm-up null 位置與輸出長度；不能只更新 `cases.len()` 與 ID 清單。沿用既有容差，不提高。
- 不在此切片處理 `NUMERIC-JSON-001`。可用[差分測試 patch](2026-10-04-pr066-075-differential-harness.patch) 先確認兩邊一致。

### 5.8 （保留編號，無內容）

FU-8 已併入 5.2。

### 5.9 FU-9（設計）已保存 campaign 的逐列狀態：Mode A 大綱

- 現況：`db/campaign.rs:123,158` 的重凍結任一列失敗，`list_campaigns` 就整體報錯，UI 清單打不開。
- 方案大綱（送審後實作）：
  - 清單每列回傳 `status`：`valid`／`incompatible`／`corrupt`，加上可顯示的 `reason`。
  - 分類需要結構化錯誤：目前 `CampaignError` 只是字串。建議讓 `freeze_campaign` 的錯誤帶種類（例如 `ContractVersion` 對應「`contractVersion` 不支援」「`contracts.X must be Y`」→ `incompatible`；JSON 解析失敗、ID 不符與其他驗證失敗 → `corrupt`），不要比對錯誤字串。
  - 非 `valid` 列只回傳原始宣告供顯示，並清楚標示未驗證；**不能冒充已驗證宣告**。真正的資料庫查詢錯誤仍整體回報失敗。
  - 同步更新 Rust 型別、typed `tauri-client`、mock 與 UI（禁用「啟動」並顯示原因）；`get_campaign`／`start_stored_campaign_for_request` 仍然拒絕。
  - 保留原 campaign ID、原始宣告與歷史判定；版本不相容不代表舊結果失效。若未來提供遷移，要建立新宣告與新 ID 並保留來源關係（不在本次範圍）。
- 測試：有效／不相容／損壞混合的清單；直接呼叫後端啟動不相容列被拒絕；UI 禁用按鈕（Playwright，mock）。
- 時機：在下一次升級任何 campaign 釘選的合約版本之前完成。

## 6. 人工驗收（不變，步驟見 [v1 §3](2026-10-04-acceptance-followups-work-order-v1.md#3-需要人工執行的驗收不是缺陷但目前不能當成已驗證)）

O1 P01 native 重開；O2 P09 Tiingo 真實帳戶；O3 #131 native campaign smoke（服務 `fetch` 要帶 `--cost-profile`）；O4 `control-token` 檔權限（需決定是否支援共用 `AFF_DATA_DIR`）。
O1–O4 本次都沒有執行，不能當成已驗證。

## 7. 狀態表（接手者請更新）

| 順序 | 切片 | 對應 Backlog | 狀態 | PR | 備註 |
| --- | --- | --- | --- | --- | --- |
| 1 | FU-1-design | PR126-130-A-R1、PR131-140-R1 | **Done**（2026-10-04 核可；狀態碼採新增 `registry_evidence_rolled_back`） | — | 維護者另提醒：圍欄不得組出帶空證據的綁定 |
| 2 | FU-1 | 同上 | **Done** | #143（`398c226`） | [handoff](2026-10-04-fu1-ledger-evidence-snapshot-v1.md) |
| 3 | FU-4 | PR001-035-J-R1 | **Done** | #144（`b395121`） | |
| 4 | FU-5 | DB-ASYNC-001 | **Done** | #145（`ce3ca94`） | 逐 instrument 鎖；10 年小時線約 27 ms／instrument（release） |
| 5 | FU-3a | PR046-055-H-R1 | **Done** | #146（`0eb8cd9`） | audit 只剩 vitest／@vitest/mocker |
| 6 | FU-3b | PR046-055-H-R1 | **Done** | #147（`c710b94`） | 兩種 audit 都為 0 |
| 7 | FU-3c | PR046-055-H-R1 | **Done** | #148（`c13cd2d`） | 定期完整 audit 另列 `SEC-NPM-AUDIT-SCHEDULE-001` |
| 8 | FU-2＋FU-8 | PR106-115-B-R1、DOC-STATE-002 | **Done** | #149（`aa7a011`） | DOC-STATE-002 的其他文件仍開放 |
| 9 | FU-6 | Low | **Done**（待 PR 合併） | 由 `fix/embargo-invalid-operand` 開出 | fixture 只更新來源 hash |
| 10 | FU-7 | Low | Ready | — | |
| 11 | FU-9 | Low／design | Design first | — | 下次升級釘選合約版本前 |
| — | O1–O4 | — | 待人工 | — | |

## 8. 已確認、不需要重做的檢查

140 個 PR 的合併 tree 與最終 head 相同、CI 全綠；TS↔Rust 隨機差分（回測 3,600、指標 800 × 16）全部一致；回測對帳不變式 3,600 例成立；
控制介面、擁有權 epoch、Tiingo 憑證、報告檔名、Tauri capabilities／CSP、DEV-only mock 都已檢查；整合 baseline 566 Rust、1072 Vitest、82/82 Playwright。細節見[驗收總覽](2026-10-04-pr001-130-acceptance-index-v1.md)。

## Resolution

Pending. 每完成一個切片，在 §7 更新狀態與 PR，並在對應的 batch handoff 追加 Resolution。

### Resolution — FU-1（2026-10-04）

§5.1 方案經維護者核可（回退使用新增的 `RegistryEvidenceRolledBack`；`fence_admission` 改用共用的事件前綴判定，不從 `AdmissionSnapshot` 組出空證據綁定），已實作。詳見 [FU-1 handoff](2026-10-04-fu1-ledger-evidence-snapshot-v1.md)。
