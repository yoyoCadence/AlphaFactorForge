# Handoff: 驗收後續工作單的方向確認與實作補正

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
PR: [#142](https://github.com/yoyoCadence/AlphaFactorForge/pull/142)（原工作單所在 PR；本文件尚未提交／推送）
Reviewed head: `c7ba5c5`；產品 baseline: `5b4a562`
Source: [原工作單](2026-10-04-acceptance-followups-work-order-v1.md)
Status: Open — 交回原 agent 整理工作單與任務切片；產品修正尚未開始。

## Summary

維護者要求獨立確認原工作單及截圖中 D1–D7 的修正方向；評估後再要求把意見寫入 handoff，
交回原 agent 整理，接著開始修正。整體方向合理，主要需要補足 FU-1 的證據範圍與持久保存時機，
以及修正測試斷言、ES2020 相容性、DB 鎖等待和 fixture 指令驗收。

本文件是原工作單的追加評估，不取代原始發現、反例 patch 或驗收紀錄。只新增交接文件及索引，
沒有修改產品程式碼、依賴、CI 或合約；FU-1–FU-9 與 O1–O4 不因此變成 Done。

## Required Action / Decision

### D1–D7 的建議方向

| ID | 評估結論 | 原 agent 整理時應保留的條件 |
| --- | --- | --- |
| D1 | 選 (a)：workspace binding 加入版本化、單調的證據快照。 | 至少涵蓋已隔離家族集合、已知衝突來源 registry 集合、每個家族曾觀察到的最高有效 `testsPerTrial`；同一 registry 與替換 registry 都要檢查。詳見 FU-1 補正。 |
| D2 | 隔離消失與檢定數下降一起修。 | 同一根因、同一 FU-1；不能只補文件。P13 使用帳本計數前完成，且保留新鮮度圍欄。 |
| D3 | AGENTS.md 指向 `alpha-factor-forge/src-tauri/Cargo.toml` 的 `rust-version`。 | README 安裝說明可保留目前最低版本 1.89，並註明以 manifest 為準；FU-8 可合併同一文件 PR。 |
| D4 | 四個間接依賴先做相容範圍內的修補；Vitest 另開 PR 升到 4.1.11。 | 4.1.11 是相關 advisory 的最低修補版本；這次不需要直接跨到 5.x。開工時重新核對公告與 audit，檢查 peers 及 fixture 指令。 |
| D5 | 建議在 CI 加 `npm audit --omit=dev`。 | 明確標示只守 production 依賴；不涵蓋本次 devDependencies 發現。定期完整 audit 是另外的追蹤建議，工作流頻率與失敗政策需在具體 CI 切片中寫清楚。 |
| D6 | 舊 campaign 逐列顯示狀態，其他正常列照常顯示；不相容列禁止啟動。 | 區分版本不相容與資料損壞／身分驗證失敗；保留原始宣告與歷史結果，後端繼續拒絕不合法啟動。詳見 FU-9 補正。 |
| D7 | FU-6、FU-7 可以做，優先度低於 FU-1、FU-5 與依賴修補。 | FU-6 很小；FU-7 要修改產生器和逐案 Rust 測試，不能只增加案例數。這是優先度建議，沒有重排現有 tasks.md 的產品任務。 |

以上是本輪審查建議與交接方向；原 agent 整理時應記錄維護者在會話中已採用的方向，沿用既有授權，
不重問已確認的同一選項。FU-1、FU-9 的具體資料形狀、寫回時機與相容性仍應先整理成可審閱的 Mode A 方案，
再按既有授權實作。若方案需要擴大成 registry schema、匯出契約或整批 DB 非同步重構，另列範圍變更。

### FU-1：快照範圍、持久保存與測試補正

1. **快照保存語意上的下限。** 已觀察到的隔離家族與衝突來源不得消失；家族的有效檢定數不得下降。
   「最高檢定數」是曾觀察到的高水位，不是允許的最大設定。只保存整份資料的 hash 然後比相等並不足夠：
   正常新增證據也會改變 hash，應採集合包含及逐家族數值下限檢查。快照大小受家族與來源 registry 數量影響。
2. **證據分類要寫清楚。** `family_conflicts`、`registry_conflicts` 與有效檢定數屬於至少需保護的限制性證據。
   `origin_genesis`、`origin_checkpoints` 是來源證明，`registry_imports` 是匯入紀錄；逐一確認遺失時是否放寬資格、
   破壞替換證明或只造成拒絕。沿用原工作單要求的反例／理由紀錄，不把所有歷史列與時間戳一律塞進快照。
3. **讀取必須一致，寫回必須持久。** 在同一筆 registry 讀取交易內取得鏈頭、證據與比較結果。
   `trial_ledger_workspace.rs:81` 的 `require_current` 現在只讀，`adopt`（:94）才持久保存觀察結果。
   列出登記、匯入、重新開啟及使用 admission count 的呼叫路徑，明訂何時在 owner 檢查的 workspace 交易中
   保存新版 binding；尚無正式 caller 的路徑也要註明。只修改 `check_binding` 回傳值，沒有實際寫回，不能宣稱
   執行期間觀察到的所有新證據都已受到保護。仍遵守 registry 先 commit、workspace 後寫入的順序。
4. **相容性與保護邊界。** 精確辨識舊版 `{registryId, seq, chainHead}`，可在首次成功採納時建立新版快照；
   舊版 binding 沒有留下的歷史證據，不能靠這次升級追溯復原。未知版本、損壞或缺欄位的新版 binding 應拒絕，
   不能退回空快照。檢查失敗不得覆蓋最後有效 binding；跨 registry 替換也不得清空已有證據。
5. **保留新鮮度圍欄。** binding 保護已持久保存的觀察結果，不取代 `fence_admission` 與 P13 的最終 admission
   同步要求；registry 和 workspace 仍不是單一原子交易，也不能保證兩者一起還原時能偵測所有回退。
6. **修正反例中的中間斷言，保留最終斷言。**
   [隔離反例 patch](2026-10-04-pr126-130-acceptance-regressions.patch) 第 47 行是
   `assert_eq!(after, observed, "the quarantine did not move the head")`。
   新版 binding 加入快照後，隔離應使 binding 改變；改成只比較 `seq`／`chainHead` 不變，另外確認隔離證據已加入。
   原工作單「反例原樣納入、只調整 helper」應補上此例外。最後「還原舊 registry 被擋下」與另一份反例的
   「檢定數 2 → 1 被擋下」仍是驗收目標，不能弱化。

增補驗收：同一 registry 的事件不變但證據增加／遺失、衝突來源消失、舊格式升級、未知／損壞新格式、
帶完整證據的正常替換與多跳轉移、重複開啟，以及正式寫回後重新開啟的回退測試。
以原工作單的 `cargo test --locked`、clippy 與既有 R1–R5 回歸為準。

### FU-3：版本、依賴圖與 fixture 指令

- 官方 [GHSA-82fw-gwwq-j7x9](https://github.com/vitest-dev/vitest/security/advisories/GHSA-82fw-gwwq-j7x9)
  列出 Vitest／`@vitest/mocker` 的修補版本為 4.1.11；3.x 沒有預定回補。
- [Vitest 4 遷移文件](https://v4.vitest.dev/guide/migration) 要求 Vite >= 6、Node >= 20；
  本專案目前 Vite 6.4.3、CI Node 20 符合該 major 的要求，實作時仍核對目標套件的實際 engines／peers。
- Vitest 4 不再依賴 `vite-node`，但本專案 `package.json` 已直接依賴 `vite-node@3.2.4`，多個 `fixtures:*`
  指令也使用它。保留該直接依賴，驗證代表性 fixture 產生指令仍可執行、結果可重現；不要因升級 Vitest 就移除它。
  此檢查不要求執行尚未授權的最終統計驗收或使用保留 seed。
- 除原有 typecheck／unit／build／e2e／兩種 audit 外，加上 `npm ls` 檢查無效 peers；逐項審查 lockfile。
  間接依賴優先在原相容範圍更新，不為了消除 audit 數字把四個套件全部新增成直接 devDependency。
- 完整 audit 的定期追蹤可另列小任務，不作為本次依賴修補完成的新增阻擋條件。
  保留 `docs/security-audit-npm.md` 禁止 `npm audit fix`／`--force` 的規定。

### FU-4：使用 ES2020 相容的自身屬性檢查

`alpha-factor-forge/tsconfig.json:5` 的型別庫是 ES2020，`Object.hasOwn` 是 ES2022 API。
建議把 `exprInterpreter.ts:228` 的 `name in FN_ARITY` 改為專案已有多處採用的寫法：

```ts
Object.prototype.hasOwnProperty.call(FN_ARITY, name)
```

不需要為此升級全域 target／lib 或改用 Map。保留原工作單的五個原型名稱回歸測試及 `unknown function` 驗收。

### FU-5：非同步預覽也要驗證 DB 鎖等待

`async + tauri::async_runtime::spawn_blocking` 方向正確，符合
[Tauri 官方命令文件](https://v2.tauri.app/develop/calling-rust/#async-commands)。參數仍為 `declaration`。

預覽仍持有共享 DB mutex；其他同步命令若在主執行緒等待同一把鎖，仍可能造成卡頓。
加上受控慢操作驗收，在預覽未完成時驗證視窗互動，並觸發代表性的共享 DB 讀取路徑。
有鎖等待時記錄觀察結果；若需擴大修改其他命令，作為 `DB-ASYNC-001` 的明確後續切片，
不能把「預覽函式已 async」當成整個 DB 非同步任務已完成。保留 O3 native smoke。

### FU-6、FU-7、FU-8：保持小範圍且驗收實際涵蓋所有案例

- FU-6：對無效左運算元 throw `RangeError`，加 TS 單元測試。Rust embargo 模組目前只接受已驗證的 params-mode
  投影，blocks／code 推導留在 TS；若只新增 blocks 左運算元錯誤，不要強塞進 params-only 的 Rust parity fixture。
  只有實際改到共用 fixture 案例才重產生並驗證對應 Rust 消費端。
- FU-7：保留現有 seed-42 案例，增加平盤、長度 1、週期 1、週期大於長度。
  Rust 測試目前只驗證 `fixture.cases[0]`，必須改成逐案執行所有指標檢查，確認 warm-up 的 null 位置與輸出長度；
  不能只更新 `cases.len()` 與 ID 清單。使用整數或二進位可精確表示的小數輸入，沿用既有 tolerance，
  不藉提高容差掩蓋差異。此切片不解決既有 `NUMERIC-JSON-001` 的所有十進位解析問題。
- FU-8：與 FU-2 同一文件 PR 合理；README 以 tasks.md 為狀態來源，補目前 P12 進度與仍待完成的階段。

### FU-9：逐列狀態與歷史身分

- `db/campaign.rs:123,158` 的重凍結／清單目前任一列失敗就讓整體報錯。
  建議列狀態至少區分有效、版本不相容、資料損壞／身分不符，並附可顯示的原因。
- 新的清單資料形狀需同步更新 Rust、typed `tauri-client`、mock 與 UI；不合法列只供顯示，不能冒充已驗證宣告。
  單列問題不阻擋其他列，真正的資料庫查詢錯誤仍回報失敗，不轉成正常清單。
- UI 禁用啟動，後端 `get_campaign`／start 也持續拒絕；測試直接呼叫後端仍會被擋下。
- 保留原 campaign ID、原始宣告及歷史決策／結果。版本不相容不代表舊結果自動失效或已經以新合約重算。
  如果未來提供遷移，建立新宣告與新 ID，保留來源關係；自動遷移不是本次 FU-9 的實作要求。
- 在下一次升級 campaign 釘選的合約版本前完成，補有效／不相容／損壞列混合清單與禁止啟動測試。

## Review Notes

原 agent 的下一步是將上述補正整理回可執行的工作單／追加 Resolution，保留原始審查歷史；
把需要兩個 PR 的 FU-3 拆清楚，沿用 FU-2＋FU-8 的文件切片，以及 FU-5 的 `DB-ASYNC-001` 邊界。
依 AGENTS.md 及既有授權移動 tasks.md 的任務狀態；只在實作及對應驗收完成後把產品任務移至 Done。

O1–O4 繼續維持待人工驗收。本輪沒有執行 Tiingo 真實帳戶測試、native 互動驗收、最終統計驗收，
也沒有解除現有 P13 阻擋。未來每個產品修正依自己的變更範圍執行原工作單要求的檢查。

## Verification

- 前一輪評估：確認 PR #142 的 metadata／changed filenames 與本地 head 一致；讀取原工作單、兩份反例及相關程式碼。
- 前一輪評估：核對 Vitest 官方 advisory／遷移文件、npm audit 文件與 Tauri 官方非同步命令說明。
- 本輪文件整理：分支為 `review/pr001-130-acceptance`，起始 worktree 乾淨；保留原工作單，只追加本文件入口與 tasks.md 紀錄。
- 本輪沒有重跑 `npm audit`、unit／e2e／Rust 套件測試；不把原 agent 的既有測試數量冒充本輪驗證結果。
- 文件檢查：UTF-8、相對檔案連結、`git diff --check` 與變更範圍檢查；執行結果見本輪交接回覆。

## Resolution

Pending — 原 agent 完成工作單整理或各項修正時追加處理結果、PR／commit 與驗收證據。

### Resolution — 原 agent 整理完成（2026-10-04）

- 維護者在會話中以本文件回答 D1–D7；已全部採用，記錄於 [工作單 v2](2026-10-04-acceptance-followups-work-order-v2.md) §2，接手者不再重問。
- 已核對本文件中影響實作的事實，全部成立：`tsconfig.json` 的 `lib` 為 ES2020（`Object.hasOwn` 不可用，專案已有四處用 `Object.prototype.hasOwnProperty.call`）；`vite-node@3.2.4` 是直接依賴且 14 個 `fixtures:*` 指令使用它；A-R1 反例第 47 行確為 `assert_eq!(after, observed, …)`；Rust 指標測試只驗 `cases[0]`；`require_current` 只讀、`adopt` 才寫回。
- 補正都已寫進 v2：FU-1 的快照範圍、證據分類、檢查語意、三個 production 寫回點（`adopt`、`db/discovery.rs:832` 入隊、`:995` resume）、相容性與限制，以及反例中間斷言的例外；FU-3 拆成 3a／3b（Vitest 4.1.11，保留 `vite-node`）／3c；FU-4、FU-5、FU-6、FU-7、FU-9 依本文件修正。
- 整理時另外發現並更正：`import_json_lines`／`export_json_lines`／`fence_admission`／`read_admission_count` 目前沒有 production 呼叫者，所以 A-R1 在產品中暫時無法觸發；它仍是 P2 的潛在契約缺陷，必須在匯入命令與 P13 之前修正。PR131-140-R1 則可經由 runner 重播登記觸發。
- tasks.md：Next 依 v2 §3 列出切片順序；Backlog 的 J-R1、H-R1、A-R1 文字已依決定與更正同步。產品程式碼仍未修改；下一步是送審 v2 §5.1（FU-1 Mode A 方案）。
