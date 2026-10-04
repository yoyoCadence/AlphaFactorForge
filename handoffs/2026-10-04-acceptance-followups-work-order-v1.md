# Handoff: PR #1–#140 驗收後續工作單（給接手 agent）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Source: [驗收總覽](2026-10-04-pr001-130-acceptance-index-v1.md)、[#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)
Baseline: `5b4a562`（之後若 `main` 前進，先重新確認下列行號）
Status: Open。FU-1、FU-3 的一部分與 FU-9 需要先取得維護者決定（§2）；其餘可直接開工。

## 0. 這份文件是什麼

2026-10-04 的整批驗收在 #1–#140 找到 1 項 P2、4 項 P3 與幾項低優先問題，但**沒有修改任何產品程式碼**。
這份工作單把每一項整理成可以直接接手的任務：證據、檔案與行號、修正方向、驗收條件、驗證指令，
以及開工前**需要維護者確認的決定**。原始證據仍在各 batch 的 handoff，這裡只引用、不重抄。

## 1. 接手方式（請先讀）

- 先讀 `AGENTS.md`。需要設計決定的項目（FU-1、FU-9）先走 **Mode A**：提出方案、等維護者確認，再實作。其他項目可直接 Mode B。
- **一個項目一個 PR**，從最新的 `main` 開分支（建議名稱見各項）。`main` 要求分支與最新 main 同步，所以前一個 PR 合併後再開下一個，避免重跑 CI。
- 開工時把 tasks.md 對應的 Backlog 項目移到 In Progress；完成後移到 Done，並在**原 batch handoff** 與本文件 §5 的狀態表追加 Resolution（handoff 只能追加，不改寫原文）。
- 本機環境注意（Windows、repo 在 OneDrive 下）：
  - Rust 建置用 `CARGO_TARGET_DIR` 指到 OneDrive 之外（例如 `/c/tmp/aff-target`），否則常在連結階段出現 LNK1104。
  - Playwright 本機用 `--workers=1` 與 `E2E_PORT=5199`；冷啟動的 dev server 第一次載入可能超過 30 秒，先手動啟動並載入一次再跑。
  - `python` 在這台機器是 Microsoft Store 的空殼，腳本化編輯請用 `node -e`，並以 `git diff --stat` 確認。
  - Claude Code 在這個環境呼叫 `git push`／`gh` 時要加 `env -u GITHUB_TOKEN`；Codex 依 AGENTS.md §7 的流程。
- 不要做：不改不相關的檔案；不升級契約版本卻不寫版本化說明；**不要執行 `npm audit fix`**（`docs/security-audit-npm.md` 明文禁止）。

## 2. 需要維護者確認的決定

| ID | 問題 | 選項 | 建議（審查者意見） | 影響的項目 |
| --- | --- | --- | --- | --- |
| **D1** | 帳本中「不新增事件的證據」要用什麼方式納入 binding 的回退保護？ | (a) binding 另存證據快照：已隔離家族集合、每個家族的檢定數上限；檢查時要求現況 ⊇ 快照。(b) 證據另建一條 hash chain，binding 存它的 seq／head。(c) 把隔離／升級寫成鏈上的非有效事件（新 TrialKind），沿用既有的鏈、checkpoint 與轉移機制。 | **(a)**：只改 workspace 端的 binding JSON，不動 registry schema 與匯出格式，範圍最小；集合大小受家族數限制。(c) 最一致但要改事件語意與匯出版本。 | FU-1 |
| **D2** | PR131-140-R1（檢定數升級）可以只寫成文件限制嗎？ | 只寫文件＋要求 P13 一律經過帶快照的圍欄；或與隔離一起納入 D1 的保護 | 與隔離一起處理（同一個根因、同一個 PR 的增量很小） | FU-1 |
| **D3** | AGENTS.md 的 Rust 版本要怎麼寫？ | 寫死 `1.89+`；或改寫成「見 `src-tauri/Cargo.toml` 的 `rust-version`」 | 後者，避免再次漂移 | FU-2 |
| **D4** | Vitest 的大版本升級要在哪個 PR 做？升到哪一版？ | 3.2.6 → 4.1.11（修補 advisory 的最低版本，仍跨一個 major）；或 → 5.x（npm 建議 5.0.3） | 先做四個間接依賴（不跨 major），Vitest 另開 PR 升到 **4.1.11**，理由同 SEC-002：只取修補所需的最小組合 | FU-3 |
| **D5** | CI 要不要加 `npm audit --omit=dev`？ | 加；不加 | 加（目前為 0，可守住 production 依賴）；屬於 CI 變更，需要明確同意 | FU-3 |
| **D6** | 已保存的 campaign 在被釘選的合約版本升級後要怎麼處理？ | 維持現狀（讀取失敗，清單整個報錯）；逐列標示「過期、不可再啟動」並讓其他列照常顯示；提供明確的遷移 | 逐列標示過期，仍 fail closed、不允許啟動 | FU-9 |
| **D7** | 低優先項目（FU-6 G-N1、FU-7 指標 fixture）要不要現在做？ | 現在做；延後 | 現在做，都是小改動 | FU-6、FU-7 |

## 3. 需要人工執行的驗收（不是缺陷，但目前不能當成已驗證）

| ID | 內容 | 步驟 | 通過條件 |
| --- | --- | --- | --- |
| **O1** | P01 Results Explorer 的 native 重啟／重開 | 以 `cargo tauri dev` 開啟、跑一次回測並保存、關閉程式、重新開啟，在 Results Explorer 重新打開該筆紀錄 | 摘要、交易明細、驗證紀錄都能讀回；缺漏的歷史明細如實顯示 |
| **O2** | P09 Tiingo 真實帳戶 | 依 `docs/market-source-tiingo-v1.md` §1–4：在 Windows 認證管理員新增一般認證 `com.alphafactorforge.desktop/tiingo`（使用者名稱 `tiingo`，密碼為 token）→ `alpha-factor-forge-service tiingo-status` → `fetch-tiingo --settings ... --from ... --to ... --data-dir <隔離目錄>` | 有資料資格時 exit 0；範本設定（成本為 null）預期得到 `DEGRADED`／exit 5；token 不出現在任何輸出或檔案 |
| **O3** | #131 campaign UI 的 native smoke（由服務建立 snapshot） | 停止桌面與服務 → `alpha-factor-forge-service fetch --instrument crypto:binance:BTCUSDT --interval 1h --from 2025-01-01 --to 2026-01-01 --cost-profile <版本>`（沒有 cost profile 的 snapshot 會是 degraded，campaign 預覽會拒絕）→ 開啟桌面 →「研究 Campaign」展開 → 選 snapshot、填樣本政策與理由 → 後端預覽 → 凍結 → 啟動該 instrument → 重新開啟判定 | 預覽顯示 resolved 與正確 bar 數；凍結 ID 與預覽相同；run 啟動；判定顯示 ELIGIBLE／NOT_ELIGIBLE、原因、精度與凍結成本，**沒有 PASS** |
| **O4** | 服務 `control-token` 檔的權限 | 決定是否支援把 `AFF_DATA_DIR` 指到共用位置 | 若支援，需另開任務設定 Windows ACL；若不支援，在文件中寫明只支援使用者 AppData |

## 4. 工作項目

### FU-1（P2）帳本中不新增事件的證據，要受 workspace binding 的回退保護

- 對應 Backlog：`PR126-130-A-R1`（P2）＋ `PR131-140-R1`（P3）。建議分支：`fix/ledger-evidence-rollback`。
- 需要的決定：**D1、D2**（先 Mode A 提案）。
- 證據與重現：
  - [A-R1 反例 patch](2026-10-04-pr126-130-acceptance-regressions.patch)：`review_a_restored_registry_releases_an_import_quarantine_unseen`（import 偵測到的隔離在還原舊檔後消失）。
  - [PR131-140-R1 反例 patch](2026-10-04-pr131-140-acceptance-regressions.patch)：`review_a_restored_registry_drops_a_protocol_upgrade_unseen`（檢定數 2 → 1）。
  - 兩者在 `5b4a562` 上都是 **1 failed**，失敗在最後的行為斷言。
- 現況（production 程式碼）：
  - binding 只有 `{registryId, seq, chainHead}`（`trial_ledger_binding.rs:20–27`，`deny_unknown_fields`），存在工作區 `app_settings.trial_ledger_binding`；前綴檢查只比對 seq／chain（`trial_ledger_binding.rs:131–165`）。
  - 不新增事件就寫入的證據：`family_conflicts`（`trial_ledger_transfer.rs:168`，由 import 寫入）、`registry_conflicts`（`:184`）、`origin_genesis`（`:1377`）、`origin_checkpoints`／`registry_imports`（`:1360,1384`）、`family_protocol_upgrades`（登記 `trial_ledger.rs:1446`、匯入 `trial_ledger_transfer.rs:1191`）。
  - 圍欄 `fence_admission` 會擋下「檢定數變小」，但擋不住「隔離消失」（還原前的判定會變成 `Unchanged`）。
- 要做的事：
  1. 依 D1 的結論實作。若選 (a)：binding JSON 加上版本與證據快照欄位；舊格式（沒有新欄位）視為空快照、可讀，第一次採納後寫回新格式；`check_binding` 在同一筆讀取交易中比對「現況 ⊇ 快照」，不符時回傳 `RegistryRolledBack`（或新增一個明確的狀態碼）；`adopt` 成功時寫回目前的快照（沿用 R4 的寫回路徑）。替換 registry（不同 registryId）的情況也要檢查快照。
  2. 把兩份反例原樣納入正式測試（必要時只調整 helper 簽名），修正後必須通過。
  3. 另外寫測試確認 `registry_conflicts`、`origin_genesis` 是否有同樣的回退問題；若有，一併納入；若沒有，在 handoff 寫明理由。
  4. 回歸：正常的匯出／匯入替換（新 registry 帶著證據）仍然 `Current`；重複開啟不會誤判；`R1`–`R5` 既有測試全部通過。
  5. 文件：`docs/trial-ledger-v1.md` 的 §7（binding）、§20、§22 補上新規則與相容性說明。
- 不做：不改事件計數語意、不改匯出版本（除非 D1 選 (c)）。
- 驗收條件：兩份反例通過；上述回歸全部通過；`cargo test --locked` 全綠；文件與程式一致。
- 驗證：`cd alpha-factor-forge/src-tauri && CARGO_TARGET_DIR=/c/tmp/aff-target cargo test --locked`，以及 `cargo clippy --locked --all-targets`（只允許既有 5 個 warning）。

### FU-2（P3）文件中的 Rust 最低版本與 `Cargo.toml` 一致

- 對應 Backlog：`PR106-115-B-R1`。建議分支：`docs/rust-msrv-sync`。需要的決定：**D3**（只影響 AGENTS.md 的措辭）。
- 現況：`alpha-factor-forge/src-tauri/Cargo.toml:8` 是 `rust-version = "1.89"`（P03a，commit `e8f6934`）；
  `README.md:148,330,502`（三語）寫 1.77.2+；`alpha-factor-forge/README.md:39` 寫 ≥ 1.77；`AGENTS.md:12` 寫 1.77+。
- 要做的事：把上述文件改成 1.89（或依 D3 改寫為指向 `Cargo.toml`）。可順便處理 FU-8（README 進度）。
- 驗收條件：`grep -rn "1\.77" README.md AGENTS.md alpha-factor-forge/README.md` 沒有結果；文件只有這些改動。

### FU-3（P3）清除開發工具依賴的 npm audit 發現

- 對應 Backlog：`PR046-055-H-R1`。建議分支：`chore/dev-deps-audit-2026-10`（Vitest 另開 `chore/vitest-4`）。需要的決定：**D4、D5**。
- 現況（`alpha-factor-forge/`，2026-10-04）：`npm audit` 6 個、`npm audit --omit=dev` 0 個。

  | 套件 | 目前 | 由誰引入 | advisory 範圍 | 需要 |
  | --- | --- | --- | --- | --- |
  | postcss | 8.5.15 | `vite@6.4.3` | ≤ 8.5.22 | > 8.5.22 |
  | nanoid | 3.3.15 | postcss | < 3.3.18 | ≥ 3.3.18 |
  | browserslist | 4.28.4 | `@vitejs/plugin-react` → `@babel/core` → `@babel/helper-compilation-targets` | ≤ 4.28.6 | > 4.28.6 |
  | baseline-browser-mapping | 2.10.40 | browserslist | < 2.11.0 | ≥ 2.11.0 |
  | vitest／@vitest/mocker | 3.2.6 | 直接依賴 | < 4.1.11 | ≥ 4.1.11（major） |

- 要做的事：依 `docs/security-audit-npm.md` 的流程，以明確版本更新 lockfile（例如逐一 `npm update <pkg>` 後檢查實際版本，或在必要時 `npm install -D <pkg>@<version>`），**逐行審查 `package-lock.json` 的差異**，不得夾帶無關套件。
  Vitest 依 D4 另開 PR，並跑完整 typecheck／unit／build／e2e。完成後在 `docs/security-audit-npm.md` 追加一節新的處理紀錄。D5 若同意，另在 CI 加 `npm audit --omit=dev`。
- 驗收條件：`npm audit --json` 與 `npm audit --omit=dev --json` 都是 0（若 D4 決定延後 Vitest，則只剩 vitest／@vitest/mocker 並在文件寫明）；`npm run typecheck`、`npm test`、`npm run build`、`npx playwright test --workers=1` 通過；Vite dev server 仍只綁本機。

### FU-4（P3）code 模式直譯器的函式白名單改成只看自身屬性

- 對應 Backlog：`PR001-035-J-R1`。建議分支：`fix/expr-whitelist-own-property`。不需要決定。
- 現況：`alpha-factor-forge/src/services/exprInterpreter.ts:228` 用 `name in FN_ARITY`；`constructor`、`toString`、`valueOf`、`hasOwnProperty`、`__proto__` 會通過，只因下一行 arity 型別不符而被拒絕，錯誤訊息帶出 `function Object() { [native code] }`。
- 要做的事：改成 `Object.hasOwn(FN_ARITY, name)`（或改用 `Map`）；在 `src/services/exprInterpreter.test.ts` 的「security: rejects everything off-whitelist」區塊加測試：上述五個名稱以 `unknown function` 被拒絕，且錯誤訊息不含 `[native code]`。
- 驗收條件：新測試在修正前失敗、修正後通過；`npm test`、`npm run typecheck` 通過。

### FU-5（P3，併入既有 P2 `DB-ASYNC-001`）campaign 預覽移出主執行緒

- 對應 Backlog：`DB-ASYNC-001`（已加註）。建議分支：`fix/campaign-preview-async`，或作為 `DB-ASYNC-001` 的第一個切片。需要的決定：無（若要一次處理整個 `DB-ASYNC-001`，先與維護者確認範圍）。
- 現況：`src-tauri/src/commands/campaign_commands.rs:115` 的 `preview_research_campaign` 是同步命令；Tauri v2 官方文件：同步命令在主執行緒執行。它對每個宣告的 instrument 載入整個 dataset 並重算 identity（`research/campaign_snapshot.rs:183–185`），期間持有 DB mutex。
- 要做的事：改成 `async` + `tauri::async_runtime::spawn_blocking`（與同檔的 `freeze_research_campaign` 相同寫法），**參數名稱不變**（`declaration`），`campaignCommands.test.ts` 的參數釘選測試必須照常通過。
- 驗收條件：`npm test`（含 `campaignCommands.test.ts`）、`cargo check --locked --all-targets`、`cargo test --locked` 通過；native smoke（O3）時預覽期間視窗仍可互動。

### FU-6（低）`deriveEmbargoBars` 對無效左運算元要 throw

- 對應 Backlog：Low／optional。需要的決定：D7。
- 現況：`src/services/embargo.ts:59` 的 `operandLookback` 沒有 default；`l` 為 `'50'`、`'atr'`、`' maFast '` 時回傳 NaN（產品路徑不可觸發：策略庫載入以 `OPERAND_IDS` 驗證、`planValidationSplit` 會拒絕 NaN）。
- 要做的事：加 default 分支並 throw `RangeError`；加對應的單元測試。注意 `signalsSplitFixture` 的錯誤案例由 TS 保存，若新增錯誤案例需重新產生 fixture（`npm run fixtures:signals-split`）並確認 Rust 端一致。

### FU-7（低）指標 parity fixture 補上邊界案例

- 需要的決定：D7。
- 現況：`fixtures/rs-core/indicators-v1.json` 只有一個案例（48 根、固定週期）；Rust 測試寫死案例數與 id（`src-tauri/src/discovery_core/indicator_parity_tests.rs:147,150`）。
- 要做的事：在 `src/parity/indicatorFixture.ts` 增加平盤、長度 1、週期 1、週期大於長度等案例；**輸入值要能被 Rust 精確讀回**（用整數或二進位可精確表示的小數，例如 100、100.5），否則會遇到 `serde_json` 差 1 ulp、平盤段 STDDEV 出現假差異的問題（見 [batch F](2026-10-04-pr066-075-acceptance-review-v1.md)）；以 `npm run fixtures:indicators` 重新產生，並更新 Rust 測試的案例清單。
- 參考：[差分測試工具 patch](2026-10-04-pr066-075-differential-harness.patch) 可用來先確認兩邊一致。

### FU-8（低）README 進度補上 P12

- 併入既有的 `DOC-STATE-002`，可與 FU-2 同一個 PR。README 的進度段落最後更新於 P11（`c1391f2`，2026-09-22），補一句 P12 的狀態並指向 tasks.md。

### FU-9（設計）已保存 campaign 遇到合約版本升級

- 需要的決定：**D6**。在下一次升級任何被 campaign 釘選的合約版本（`metrics`、`split`、`walk-forward` 等）之前完成。
- 現況：`db/campaign.rs:107,158` 每次讀取都用目前常數重新凍結；任何一列失敗，`list_campaigns` 整個報錯，UI 的已保存清單打不開。
- 要做的事（若 D6 選「逐列標示過期」）：清單回傳每列的狀態而不是整體失敗；過期列不可啟動；`get_campaign`／`start` 仍 fail closed；加測試。

## 5. 狀態表（接手者請更新）

| ID | 對應 Backlog | 優先 | 需要決定 | 狀態 | PR | 備註 |
| --- | --- | --- | --- | --- | --- | --- |
| FU-1 | PR126-130-A-R1、PR131-140-R1 | P2 | D1、D2 | Open | — | |
| FU-2 | PR106-115-B-R1 | P3 | D3 | Open | — | |
| FU-3 | PR046-055-H-R1 | P3 | D4、D5 | Open | — | |
| FU-4 | PR001-035-J-R1 | P3 | — | Open | — | |
| FU-5 | DB-ASYNC-001 | P3（併入 P2） | — | Open | — | |
| FU-6 | Low／optional | 低 | D7 | Open | — | |
| FU-7 | Low／optional | 低 | D7 | Open | — | |
| FU-8 | DOC-STATE-002 | 低 | — | Open | — | |
| FU-9 | Low／optional | 設計 | D6 | Open | — | |
| O1–O4 | — | 人工驗收 | O4 需決定 | Open | — | |

## 6. 已經確認、不需要重做的檢查

接手時不必重做以下項目（細節見各 batch handoff）：

- 140 個 PR 的合併 tree 與最終 head 相同；每個 head 的 CI 都通過。
- TS↔Rust 隨機差分：回測 3,600 例、指標 800 例 × 16 條序列，全部一致；回測對帳不變式 3,600 例成立。
- 本機 HTTP 控制介面（loopback、Host、Origin、constant-time token）、工作區擁有權 epoch、Tiingo 憑證只走 Credential Manager、報告檔名白名單與原子建立、Tauri capabilities 與 CSP、DEV-only mock 不進 production bundle。
- 整合 baseline：566 Rust（1 ignored）、1072 Vitest、82/82 Playwright、typecheck、build、all-target check、clippy（既有 5 個 warning）。

## Resolution

Pending. 每完成一項，在 §5 更新狀態與 PR，並在對應的 batch handoff 追加 Resolution。
