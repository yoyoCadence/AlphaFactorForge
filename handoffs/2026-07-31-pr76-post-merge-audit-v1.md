# Handoff: PR #76 合併後全專案稽核與修正順序

Date: 2026-07-31
Repo: yoyoCadence/AlphaFactorForge
Branch: main (baseline `748ff91`)
PR: #76
Status: Open — follow-up tasks recorded; execute one task per branch/PR

## Summary

PR #76 (`RUNNER-EXEC-001`) 已合併到 `main`，merge commit 為
`748ff91c4d41558178276e2ac2e3777b4f632b2b`。合併後完成一次唯讀的全專案
稽核，範圍涵蓋互動回測、Sweep、metrics、market-data admission、Rust/Tauri
runner、SQLite persistence、validation audit records、測試／mock、CI、工具鏈與
狀態文件。未發現 P0；確認多個會靜默產生錯誤資料或失真評分的 P1。

本 handoff 是下一位 agent 的跨 session 證據與裁決來源；`tasks.md` 是唯一狀態
板，`docs/improvement-backlog.md` 的 2026-07-31 addendum 是目前唯一已展開成
coding-agent 規格的 `BUG-RESULT-CONTEXT-001`。不得把以下修正回填到已合併的
PR #76，也不得將多項修正綁成同一 PR。

## Required Action / Decision

### 1. Canonical baseline 與執行規則

1. 每次只執行一個 task；先讀完整 `AGENTS.md`、
   `docs/agent-execution-protocol.md`，再讀目標 task 規格。
2. 從最新 `origin/main` 開新 branch；若目標 task 尚未在
   `docs/improvement-backlog.md` 展開成 Files / Exact plan / Non-goals /
   Validation / Acceptance，先由 Planner 補規格，不可直接腦補實作。
3. task 開始時才從 Backlog/Next 移到 In Progress；完成並驗證後移到 Done。
4. 行為或契約改變時更新 `CHANGELOG.md`；SQLite migration 僅能新增，禁止改寫
   0001–0003；contract 文件等實作落地後再同步。
5. 每個 PR 使用英文 conventional commit title、zh-TW PR body，開 draft PR 後
   停止，交給不同 session 的 reviewer。

### 2. 已裁決執行順序

1. `BUG-RESULT-CONTEXT-001` — 現在唯一的 Next，也是最高優先。
2. `METRIC-002`。
3. `PERSIST-AUDIT-001`。
4. `RUNNER-OWNERSHIP-001`。
5. `DATA-QUALITY-001`。
6. `BUG-SWEEP-CONTEXT-001`。
7. `STRATEGY-VALIDATION-001`。
8. 上述 correctness/audit gates 合併後才開始 `RUNNER-UI-001`。
9. P2/P3 reliability、performance、docs、toolchain tasks 依 `tasks.md` 排程。

### 3. BUG-RESULT-CONTEXT-001 (P1) — 舊結果錯配新策略／資料集

#### Evidence and shortest reproductions

- `alpha-factor-forge/src/components/BacktestPanel.tsx:76` 保存裸 `result`，沒有
  strategy/dataset snapshot。
- 同檔 `121-142` 切換資料集後直到新 `getCandles` 成功才清結果；失敗會永久保留
  舊 candles/result。`148-151` 的數值更新與其他 strategy handlers 不會使結果失效。
- 同檔 `194-220` 的 Run 可消費任何非空 candles；`229-246` 的 Save 以目前
  strategy 與目前 dataset id 搭配舊 result；`275-281` 的 Sweep candle path 也可
  接受舊 candles。
- `alpha-factor-forge/src/components/ResultsSection.tsx:113-130` 以 live strategy /
  dataset metadata 和舊 result 建 report。
- 重現 A：Sample Run -> 改 fastMA、fee、direction 或 mode -> 不重跑直接
  Save/Export。舊 metrics/trades 會被標成新 strategy。
- 重現 B：dataset A Run -> 選 B -> B 尚未載入或載入失敗 -> Save/Run。A 的
  result/candles 可被標成 B，annualization 還會採 B interval。
- 重現 C：Run -> Sweep Apply Best -> 不重跑直接 Save/Export。

#### Required outcome

依 `docs/improvement-backlog.md` 的 execution-ready 規格建立不可變 completed-run
artifact、單一 context equality helper、同步 invalidation 與 generation guard。
Save/Export 必須只讀 artifact snapshot；不得從 live editor 補欄位。此 task 不處理
metrics、Sweep artifact、Rust、schema 或 Runner UI。

### 4. METRIC-002 (P1) — UTC 月報酬漏掉跨月首段

#### Evidence

- `alpha-factor-forge/src/core/metrics/index.ts:167-178` 以每月第一個 equity 點當
  當月基準；`computeMetrics` 已知 `startEquity`，卻沒有傳給 monthly calculation。
- `alpha-factor-forge/src-tauri/src/discovery_core/metrics.rs:293-322` 有相同算法；現有
  cross-language parity 只證明兩端相同，無法證明算法正確。
- 手算例：Jan31=100、Feb1/Feb29=200、Mar1/Mar31=100、Apr1/Apr30=200。
  現況得到 `[0,0,0,0]`；以前月月底為基準應為 `[0,+1,-0.5,+1]`。consistency
  normalized 可由 1 降至約 0.1334，Score 因而可被高估約 0.8666（weight=1）。

#### Required outcome and risks

- 第一個月以 `startEquity`、之後各月以前一月最後 equity 為基準。
- TS/Rust 同步，新增手算 UTC 月界測試；Gate/Score effect 也要有 regression。
- 這是 persisted calculation contract 行為變更：升級 `metrics-v1`，同步所有 config、
  parity fixture、validation record snapshot 與 `CHANGELOG.md`。
- 開始前必須展開舊版 paused runs/records 的 fail-closed compatibility 計畫；不得只改
  兩個函式後讓舊 `metrics-v1` 冒充新結果。

### 5. PERSIST-AUDIT-001 (P1) — immutable record 只做淺層驗證

#### Evidence and reproduction

- 公開 command：`alpha-factor-forge/src-tauri/src/commands/db_commands.rs:97-118`。
- validator：`alpha-factor-forge/src-tauri/src/db/repositories.rs:670-782`。
- `727-758` 只要求 JSON envelope version 等於 row version，沒有只接受
  `validation-record-v1`；`768-780` 只要求 benchmark 有 version、array、object。
- test helper `repositories.rs:1378-1390` 缺 contracts/hashes/embargo/split/metric
  snapshots/Gate/testedCombinations，benchmark 又是空陣列；`1519-1522` 仍明確
  assert accepted。
- 這與 `docs/validation-record-contract.md:15-17,32` 的 self-contained/full evidence
  契約衝突。合法 invoke 可 append 任意自洽版本與空 benchmark 的永久「證據」。

#### Required outcome

使用 strict typed DTO + explicit version dispatch；完整驗 strategy/dataset hashes、
contracts、embargo、split、metrics、四個 deterministic benchmark id、Random Entry
distribution、Gate、Score、testedCombinations 與 summary snapshot cross-check。unknown
version 與每個 required-field mutation 都必須在 transaction 前拒絕。不要在這個 PR
順便改 schema 或一般 trade invariants（後者是 `PERSIST-INVARIANT-001`）。

### 6. RUNNER-OWNERSHIP-001 (P1) — 多實例 split-brain

#### Evidence and reproduction

- `alpha-factor-forge/src-tauri/Cargo.toml` 沒有 single-instance plugin。
- `alpha-factor-forge/src-tauri/src/main.rs:23-33` 每個 process setup 都無條件呼叫
  `recover_orphans`。
- `alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs:301-315` 的 controls 是
  process-local map。
- `alpha-factor-forge/src-tauri/src/db/discovery.rs:685-699` 沒有 owner/PID/lease/
  heartbeat/staleness 判斷，直接將所有 running runs 改 paused 並 requeue jobs。
- migration 0003 的 single-active-row index 只限制資料列，不證明 process ownership。
- 重現：A 運算中啟動 B；B 將 A 的 live run recovery。若 B resume，A/B 可能競爭
  claim/commit，導致錯誤 failure 或部分 evidence。

#### Required outcome

先採官方 Tauri single-instance plugin，且必須在 setup/recovery 前註冊；第二次啟動
只 focus 主視窗。驗證 plugin 的 Rust/toolchain floor，並做雙啟動 smoke：A run 狀態不可
被 B pause/requeue。若 maintainer 要求即使 single-instance protection 失效仍防 stale
writer，再另立 owner-generation/lease/heartbeat task；不要把 schema lease 設計偷塞進
第一個 plugin PR。

### 7. DATA-QUALITY-001 (P1) — dataset identity 不等於 market-data validity

#### Evidence

- TS `src/core/hashing/index.ts:257-283` 與 Rust
  `src-tauri/src/identity.rs:186-218` 只驗 non-empty、finite、safe timestamp、sort /
  duplicate、metadata 與 hash consistency。
- epoch seconds 例如 `1704067200` 會被當 epoch milliseconds，讓 monthly Gate/Score
  落在錯誤年份/月分。
- `{open:100, high:90, low:110, close:100, volume:-1}` 可合法 hash/import；Rust
  runner 只重驗同一 identity，而 backtest SL/TP 直接使用 high/low。
- 未驗價格 > 0、volume >= 0、`low <= high`、`low <= open/close <= high`、時間範圍
  與 interval cadence。

#### Required outcome and scope boundary

在 import admission 新增 TS/Rust semantic validator；不要污染 durable identity encoding
或改寫既有 migration。already-stored invalid datasets 在 selection/run 時 fail closed 並提供
可行的重新匯入訊息，不自動重寫 candle bytes。unknown interval fallback 是現有明確測試
契約，必須由 `INTERVAL-CONTRACT-001` 先裁決，不能在本 task 靜默改掉。

### 8. BUG-SWEEP-CONTEXT-001 (P1) — stale Sweep 可穿越 Holdout 邊界

#### Evidence and reproduction

- `alpha-factor-forge/src/components/SweepSection.tsx:203-228` 只在 library reset 或
  sweep 軸/config 改變時清 `sweepResult`；holdout、holdoutPct、dataset、interval、非
  sweep strategy fields 不會使它失效。
- `270-273` 的 Apply Best 仍可使用舊 grid。
- 重現：Holdout OFF -> Run Sweep（全資料）-> 開啟 Holdout -> 不重跑直接 Apply
  Best。原定 OOS 尾段實際已參與調參。

#### Required outcome

Sweep artifact 保存 dataset id/hash、interval、精確 holdout range、sweep config、base
strategy snapshot（swept axes 做 mask，確保 Apply cell 本身不使結果失效）。context
mismatch 隱藏/停用 heatmap/apply，generation token 丟棄 late completion。測 holdout
toggle/pct、dataset、非 axis edit、late result，以及 Apply axis cell 仍合法。

### 9. STRATEGY-VALIDATION-001 (P1) — manual period 接受零／負數／小數

`StrategySection`/`NumberInput` 只保證 finite，`strategyLibrary` 也沒有要求 indicator
period 為 safe positive integer；`strategySignals` 直接把值交給 SMA/EMA/RSI/BB。
`fastMA=2.5` 或 0 可產生 undefined/NaN 後靜默成零交易並被儲存。建立一個 runtime
validator，所有 manual run/save paths 共用；UI `min/step` 只是輔助。涵蓋零、負數、
小數、非有限值與必要 cross-field constraints；不在此 task 改 discovery config。

### 10. P2/P3 follow-ups recorded in tasks.md

- `PERSIST-INVARIANT-001`: `save_backtest_result`/validation writes 未驗 summary/trades
  跨欄位一致；可寫 trade_count 不符、非法 side、反向時間、非正價格或 range 外 trade。
- `IO-ROBUSTNESS-001`: `file_commands.rs:22-24,66-88` 使用
  `exists -> std::fs::write`，有 check-then-write truncation race；改 `create_new` retry。
- `DB-ASYNC-001`: 大型 import/save/file commands 同步且持有 DB mutex；Discovery
  command 已示範 `spawn_blocking`。
- `CI-TAURI-SMOKE-001`: CI 只有 cargo check/test 與 Vite mock E2E，沒有 native
  executable/build/invoke/event smoke。
- `TEST-MOCK-PARITY-001`: mock `saveStrategy` 每次生新 id，SQLite 依 hash UPSERT 且
  保持 id；E2E 因此看不到 same-hash native behavior。
- `PERF-001`: Sweep 最多 256 回測在 React thread 同步跑；完整既有規格在
  `docs/improvement-backlog.md:505`，不要複製或另建 ID。
- `PERF-CHART-COMPUTE-001`: hover/replay redraw 會重算完整 indicators/trade map。
- `PERF-CHART-BRIDGE-001`: 小型 view-state 變更會重新序列化全 candles 到 OS window。
- `DOC-STATE-002`: root/local README、`alpha-factor-forge/TODO.md`、
  `PHASE_A_VERIFY.md` 仍以 0001/stub 描述已完成的 0002/0003/runner。
- `TOOLCHAIN-001`: Cargo 宣稱 Rust 1.77、實際本機 1.96、CI 使用浮動 stable；MSRV
  未被證明。
- `CI-RUSTFMT-001`: repo-wide `cargo fmt --all -- --check` 目前只因
  `src/commands/db_commands.rs` formatting drift 失敗；CI 也未執行 fmt。
- `DB-MIGRATION-DIAGNOSTIC-001`: `db/mod.rs:78-84` 的 `unwrap_or(false)` 吞掉
  migration existence query 的真 SQLite error；transactional migration apply 本身正確。
- `SEC-RUST-001`: Rust advisory scan 未納入 CI，本機未安裝 `cargo audit`。

### 11. Accepted contracts — do not silently "fix"

- `INTERVAL-CONTRACT-001`: unknown/prototype-key interval fallback-to-daily 目前是明確
  `random-entry-v1`/benchmark parity fixture 行為。它會讓 `1H`/`60m`/未支援 interval
  靜默以 365 bars/year annualize，風險高，但任何改動必須先裁決 strict canonical set
  或 alias parser，然後版本化 TS/Rust contract、fixtures 與 CHANGELOG。
- `DECISION-ZERO-TRADE-001`: runner 在 Random Entry 前遇 zero candidate trades 會讓
  execution error；現有 code comment 與 `random-entry-v1` 明確接受，不得直接改為
  fabricated percentile。建議方案是 persisted Gate rejection + explicit insufficient
  evidence，但需 Random Entry/Gate/validation-record contract 協同版本化。
- 真正 crash 後將 running 轉 paused/requeued 並由使用者明確 resume，是 D5 設計；
  問題只在沒有先確認 owner 已死就 recovery。
- terminal done event 晚於最後 persisted checkpoint、Test schema 存在但 runner 不讀
  Test，都是既定契約，不是本次缺陷。
- `RUNNER-UI-001` 是 PR #76 後原定的下一個 slice，現已移回 Backlog 等待上述
  correctness gates；舊 frontend `events.ts` DTO 與 backend v1 不一致應在該 task
  處理，不另建別名 task。

## Review Notes

- 本次沒有發現 API key 寫入 frontend/localStorage/SQLite，也未在常見 tracked-file
  secret prefix 掃描中發現命中；這不是完整 secret-scanner 的替代品。
- npm audit 在 2026-07-31 對目前 dependency tree 回報 0 vulnerabilities；Rust
  advisory scan 因 `cargo audit` 未安裝而沒有完成。
- `cargo clippy --locked --all-targets` 通過，但保留四個既有 warning：兩個
  unnecessary `map_or`、一個 manual range contains、一個 clamp-like pattern。沒有在
  audit 或此 handoff 順手修。
- migration 0001–0003 的 apply 是同 transaction 寫 DDL + version row，這個 rollback
  設計正確；沒有把「無 checksum」升級成當前 correctness defect，只視為 policy/CI
  hardening。
- Tauri capabilities/CSP 未發現高信心過度授權；Phase C AI/secret commands 仍是明確
  stub，本次不擴張其 scope。

## Verification

- Confirmed PR #76 merged to `origin/main` at `748ff91` and local `main` was
  fast-forwarded before authoring the records.
- Audit started from a clean `feat/runner-exec-001`; the documentation branch
  `docs/project-audit-handoff-2026-07-31` was created from `748ff91`.
- PR #76 baseline verification recorded in `tasks.md`: typecheck, production
  build, 382 Vitest, 123 Rust tests, and 25 Playwright E2E pass.
- Audit checks: `npm audit` reports zero current advisories; clippy completes
  with four pre-existing warnings; `cargo fmt --all -- --check` identifies the
  isolated `db_commands.rs` drift; common secret-prefix scan returned no match.
- This handoff task changes planning/status documentation only. It does not
  modify application source, migrations, tests, contracts, dependencies, or
  runtime behavior; therefore `CHANGELOG.md` is intentionally unchanged.

## Resolution (added when acted on)

For each completed task, append: task id, actor/date, branch, commit, PR, files
changed, exact validation results, contract/migration decisions, residual risk,
and the corresponding `tasks.md` status transition. Preserve every section
above; do not rewrite prior evidence.

### BUG-RESULT-CONTEXT-001 — Claude Code, 2026-07-31

- Branch: `fix/backtest-result-context`，自 `origin/main` `0ebe6a8`（PR #77 合併後）
  開出。Commit `8a8015f`。PR: #78（draft，base `main`，等待另一個 session 的
  reviewer）。`tasks.md` 由 Next -> In Progress -> Done。
- Files changed: 新增 `alpha-factor-forge/src/services/runArtifact.ts` 與其
  `runArtifact.test.ts`；改 `src/components/BacktestPanel.tsx`、
  `src/components/ResultsSection.tsx`、`src/tauri-client/mockClient.ts`；
  新增 `e2e/result-context.spec.ts`，`e2e/export.spec.ts` 追加一段失效斷言；
  更新 `tasks.md`、`CHANGELOG.md` 與本 handoff。

#### 實作裁決

1. **不可變 artifact + 單一比較點。** 完成的回測存成凍結的 `CompletedRun`
   （deep strategy snapshot、durable `strategy-v2` identity、dataset
   id/hash/symbol/interval/時間範圍/bar 數、實際交易範圍與 Holdout split）。
   是否仍有效一律由 `runContextKey`/`sameRunContext` 判斷，React handler 不做
   逐欄比較，因此沒有任何呼叫點能漏掉欄位；`ParamsStrategy` 日後新增欄位會
   自動納入失效判斷。範圍由 runner 同一支 `holdoutSplitIndex` 導出。
2. **同步失效由 derived gate 保證。** render 階段就算出「completed 且 context
   仍相符」才會有 artifact，所以任何 result-affecting 編輯在接受該值的同一個
   render 內就讓 Save/Export/pop-out 消失，不依賴 handler 記得清狀態。
   `changeStrategy` 是唯一策略變更入口（mode/signals/rules/code/週期/執行與
   風險欄位/圖表快捷列/library 載入/Sweep 套用），`invalidateRun` 負責 Holdout。
3. **candle readiness 綁 dataset identity。** candles 以
   `{ datasetKey, rows }` 儲存，非空陣列不再等於可用；切換資料集在同一個同步
   更新裡清掉 readiness 與 completed run，載入失敗則保持 readiness 為 null，
   Run/Save/Export 因此維持停用。`run()` 移除「沒有 candles 就抓來直接用」的
   fallback，`ensureCandles` 也不再抓取，順帶關掉 Sweep 吃到舊 candles 的同一
   個破口（§3 證據 `275-281`）。
4. **generation + context 雙重守衛。** 單一遞增 token 服務 dataset load 與
   backtest run，另有 per-kind owner ref。每個非同步寫入都要同時滿足「仍是同類
   最新工作」與「當初的 context 仍是 live」。candles 不依賴策略，所以策略編輯
   只讓 in-flight run 失效，不會讓 in-flight load 變成孤兒（早期設計曾有此瑕疵）。
5. **策略名稱刻意維持 live。** Save/Export 的 strategy、dataset、interval、
   range、metrics、trades 全部只讀 artifact；唯一的 live 欄位是名稱，因為它既
   不影響結果也不進 `strategy-v2` identity，且 `e2e/strategy-library.spec.ts`
   既有流程就是「跑完再命名才存」。Save 在任何寫入前先驗
   `buildStrategyDef(snapshot).strategy_hash === artifact.strategyHash`。

#### Scope 例外（已取得授權）

`src/tauri-client/mockClient.ts` 不在規格的 Files likely affected 內。規格第 6
步要求 late-load/late-run 迴歸必須透過既有 `?mock=1` seam 且不得弱化測試，但
mock 的 `getCandles` 立即回傳，race 無法決定性重現。依 coding-agent prompt 先
停下回報，maintainer 於 2026-07-31 授權加入 `?mock=1&candleDelay=<ms>` 延遲
旋鈕。它位於既有 `import.meta.env.DEV` 守衛之後，production build 不存在，不
構成 product-only 行為。

#### Validation

- `npm run typecheck`、`npm test`（409，+27）、`npm run build`、
  `npm run e2e`（28，+3）全綠。本 task 未動 Rust/schema，因此未重跑 cargo。
- 手動：載入樣本 -> 執行回測 -> 改手續費 -> 指標表與 Save/Export 消失並顯示
  「已失效」提示 -> 重新執行 -> 全部恢復且 metadata 相符。

#### 殘餘風險與後續

- 目前「失效」的呈現是隱藏結果區並顯示 zh-TW 提示，而非停用按鈕；既有 e2e
  `holdout.spec.ts` 的 `col-全期 / col-樣本外` count-0 斷言仍成立，但其註解描述
  的「回到單欄」已改成「整區失效」。若日後偏好保留表格並改為 disabled 按鈕，
  屬 UI 決策而非正確性缺口。
- 切換資料集時 SweepSection 會因 candles 清空而卸載，其 heatmap 與軸設定隨之
  重置。這比舊行為安全，但完整的 sweep provenance 仍屬 `BUG-SWEEP-CONTEXT-001`。
- artifact 只覆蓋互動式回測；Runner 產生的 validation record 走另一條既有路徑。
- 下一個裁決順序是 `METRIC-002`，但 `docs/improvement-backlog.md` 尚未把它展開
  成 execution-ready 規格，因此本次未把它移進 Next。

#### Resolution — PR #78 acceptance follow-up, 2026-07-31

- Independent acceptance found one remaining evidence gap: the implementation
  handled a rejected `getCandles`, but the E2E suite only proved a delayed
  successful load and did not execute the fetch-failure branch required by the
  original `tasks.md` Next entry and the acceptance criterion.
- Maintainer authorized the focused correction. Commit `d222191` extends the
  existing DEV-only mock seam with `candleFailId=<dataset-id>` and adds an E2E
  that completes a run on dataset A, selects dataset B, deterministically
  rejects B's candle load, and proves Run, Sweep, Save, Export, metrics pop-out,
  and the old completed result remain unavailable after the failure settles.
- The rejection control is behind the same `import.meta.env.DEV` guard as
  `candleDelay`; a post-build string scan confirms neither control nor its mock
  error appears in the production bundle. No product behavior, Rust, schema,
  dependency, report schema, or metric contract changed, so `CHANGELOG.md`
  remains unchanged by this follow-up.
- Final validation after syncing with `origin/main`: `npm run typecheck`;
  `npm test` (409); `npm run build`; `npm run e2e` (29, +4 from baseline);
  `git diff --check`; production-bundle seam scan. All pass. `tasks.md` records
  the corrected E2E count and both authorized mock controls.

### BUG-SWEEP-CONTEXT-001 — Claude Code, 2026-08-15

- Branch: `fix/sweep-result-context`，自 `origin/main` `a3fe2fe`（PR #97 合併後）
  開出。`tasks.md` 由 Backlog -> In Progress -> Done。規格先以 Planner 身分展開到
  `docs/improvement-backlog.md`（本 handoff §2 第 2 條要求），再依該規格實作。
- Files changed: 新增 `alpha-factor-forge/src/services/sweepArtifact.ts` 與
  `sweepArtifact.test.ts`；改 `src/components/SweepSection.tsx`、
  `src/components/BacktestPanel.tsx`（只有 prop 接線）；新增
  `e2e/sweep-context.spec.ts`；更新 `docs/improvement-backlog.md`、`tasks.md`、
  `CHANGELOG.md` 與本 handoff。

#### 實作裁決

1. **不可變 artifact + 單一比較點。** 完成的掃描存成凍結的 `CompletedSweep`，其
   `sweep-context-v1` context 記錄 dataset id/hash/symbol/interval/時間範圍/bar
   數、實際最佳化的 bar 範圍（含 Holdout split）、正規化後的 sweep config，以及
   **移除掃描軸後**的 base strategy。是否仍有效一律由 `sameSweepContext` 判斷，
   `SweepSection` 不做逐欄比較。
2. **掃描軸遮罩是刻意的。** 點格子（或套用最佳）寫入的就是掃描軸本身，因此不得
   使自己來源的 heatmap 失效；非掃描欄位一律留在 basis 內，任何變更都會失效。
   `describeSweepBasis` 的測試以 `defaultStrategy()` 推導預期 key set，不手寫清單，
   所以 `ParamsStrategy` 日後新增欄位會自動納入。
3. **單一範圍定義。** 最佳化範圍由 `sweepRangeFromRunRange` 從 panel 的 run range
   導出（Holdout 開啟時為 `[from, splitIndex - 1]`），與 `run()` 共用同一支
   `holdoutSplitIndex`，BUG-001 的樣本內邊界維持單一來源。
4. **prop 收斂為一個 `liveContext`。** 原本的 `strat`/`interval`/
   `datasetSelected`/`holdout`/`holdoutPct` 五個獨立 prop 讓完成的 grid 沒有任何
   可比對的整體描述，正是 `runArtifact.ts` 當初要消除的漂移。
5. **保留既有硬清除。** config 編輯的 `clearSweep` 與 library 載入的 `resetSignal`
   維持不動：它們是新 gate 的嚴格子集，維持 `e2e/sweep.spec.ts` 既有斷言，也與
   panel 的 `loadSavedStrategy` 同時清 `completed` 的既有先例一致。
6. **late completion 用純函式驗證。** `sweepResultIsWritable`（generation ownership
   + live context 相符）是純函式並涵蓋四個分支的單元測試；repo 沒有 React 元件測試
   環境，且用 Playwright 逼出 20ms 視窗的 race 會變成時間相依的 flaky 測試，故不以
   e2e 覆蓋這一項。

#### Scope

未動 `src/services/paramSweep.ts`、`src/services/runArtifact.ts`、`src-tauri/`、
schema/migration、依賴，以及任何既有 `e2e/*.spec.ts`。sweep artifact 只存在於元件
state，不寫入 SQLite。`freezeDeep`/`cloneDeep` 刻意在 `sweepArtifact.ts` 內複製一份，
以免為了共用而改動已合併的 BUG-RESULT-CONTEXT-001 契約檔、或把 move-only refactor
混進 fix PR；抽出單一 `services/immutable.ts` 列為建議後續。

#### Validation

- `npm run typecheck`、`npm test`（743，+40）、`npm run build`、`npm run e2e`
  （53，+3）全綠。本 task 未動 Rust/schema，因此未重跑 cargo。
- Mutation check：暫時把 context gate 改為恆真後，`sweep-context.spec.ts` 的兩個
  gate 案例分別在 `sweep-best-marker` 斷言處失敗，證明斷言確實由 gate 支撐；
  dataset switch 案例仍通過，因為它是由 panel 在 candles 清空時卸載整個 section 所
  保護，dataset 欄位對 gate 的貢獻改由單元測試證明。已還原後重跑全綠。

#### 殘餘風險與後續

- 失效時的呈現是隱藏 heatmap 並顯示 zh-TW 提示，與 `ResultsSection` 的
  `result-stale` 一致；若日後偏好保留表格改為 disabled，屬 UI 決策而非正確性缺口。
- Holdout 百分比即使 clamp 後 split index 不變（例如 600 根的 30% 與 30.1%），仍會
  使掃描失效。這是刻意 fail closed，成本只是一次重掃。
- 掃描仍在 React thread 同步執行最多 256 次回測；worker 化與取消仍是 `PERF-001`。
- 下一個裁決順序是 `STRATEGY-VALIDATION-001`，也是 `RUNNER-UI-001` 前最後一個
  correctness gate；它尚未在 `docs/improvement-backlog.md` 展開成規格。

### STRATEGY-VALIDATION-001 — Claude Code, 2026-08-16

- Branch: `fix/strategy-period-validation`，自 `origin/main` `be4e3c6`
  （PR #98 合併後）開出。`tasks.md` 由 Backlog -> In Progress -> Done。規格先以
  Planner 身分展開到 `docs/improvement-backlog.md`，再依該規格實作。
- Files changed: 新增 `src/services/strategyValidation.ts` 與
  `strategyValidation.test.ts`；改 `src/services/backtestRunner.ts`、
  `src/services/strategyRecord.ts`、`src/services/discoveryConfig.ts`、
  `src/services/candidateEnumeration.ts`、`src/components/StrategySection.tsx`、
  `src/components/NumberInput.tsx`；重新產生
  `fixtures/rs-core/{benchmark,runner-config}-v1.json`；新增
  `e2e/strategy-validation.spec.ts`；更新 `docs/improvement-backlog.md`、
  `tasks.md`、`CHANGELOG.md` 與本 handoff。

#### 實作裁決

1. **硬性驗證只涵蓋 11 個指標欄位。** 8 個週期為 safe integer `>= 1`、
   `rsiBuy`/`rsiSell` 落在 0–100、`bbMult` > 0。判斷權威一律是既有的
   `checkNumericParam`，本模組只負責 zh-TW 文案與掛載點，並以 agreement test
   對 11 個 key 逐值比對，確保規則不可能分叉。
2. **5 個執行模型欄位刻意排除。** `feePct`/`slipPct`/`sizePct`/`slPct`/`tpPct`
   由 `toExecCostFractions` 的既有 legacy clamping 擁有，且已有committed 測試
   （`sizePct: 0` = 100%、負手續費 clamp 為 0）。套用 discovery 的 percent domain
   會與這些測試衝突並讓既存策略無法執行；上界則早已由引擎的
   `assertNormalizedFraction` 把關。另有 classification 測試確保
   hard ∪ legacy = `ParamsStrategy` 全部數值欄位，未來新增欄位無法漏分類。
3. **cross-field 規則是警告，不是錯誤。** 三條規則都不會產生 NaN，只是可疑假設；
   repo 自身的紀錄也把它們視為「合法 grid 的預期修剪」而非 malformed；且是否讀取
   `fastMA` 取決於選用訊號。若改為致命錯誤，預設 2-D 掃描的 (20,20) 格會被靜默清空。
   日後要收緊只是一行，放寬則不可逆——這一點請 maintainer 明確裁決後再改。
4. **依賴反轉是被迫的，不是偏好。** `discoveryConfig -> randomEntry ->
   backtestRunner`，所以 runner 會 import 的 validator 不可能再 import
   `discoveryConfig`：第一版實作立即產生 ESM 循環，26 個既有測試以
   `discoveryConfig.contracts is missing key "gate"` 失敗。解法是把 domain table、
   `checkNumericParam`、`DISCOVERY_VALIDITY_RULE_IDS`、`candidateValidity`
   **逐字搬入**新模組，兩個 discovery 模組改為 re-export。中立性以機械方式證明：
   `discoveryConfig.test.ts`、`candidateEnumeration.test.ts`、
   `runnerConfigFixture.test.ts` 完全未修改即通過，且重新產生的兩個 parity fixture
   只有 3 行 `generator.sourceHashes` 改變，其餘期望值 byte-identical。
5. **library 載入維持可載入。** `strategyFromDef` 未加新規則：UI 目前沒有刪除策略
   （PARITY-003 仍 deferred），若在載入時拒絕，舊的不合法列會永遠無法修復。Run 與
   Save 才是 fail-closed 邊界。

#### Validation

- `npm run typecheck`、`npm test`（773，+30）、`npm run build`、`npm run e2e`
  （56，+3）全綠。未動 Rust，故未重跑 cargo。
- Bundle：268.37 kB → 271.19 kB（gzip 87.90 → 88.82 kB）。新模組無 service 相依，
  因此沒有把 discovery graph 拉進前端 bundle。
- Fixture：`npm run fixtures:benchmarks`、`npm run fixtures:runner-config`
  重新產生；diff 僅
  `backtestRunner` / `discoveryConfig` / `candidateEnumeration` 三個 sourceHash。

#### 殘餘風險與後續

- cross-field 目前只警告（見裁決 3）。
- `embargo.ts` 仍保有自己的 usage-aware `period()` 檢查（只驗真正被讀取的週期），
  與本 gate 的 blanket 檢查是兩份不同契約，且由 parity fixture 鎖定，本次未合併。
- 這是 post-PR #76 稽核順序的最後一個 correctness gate；`RUNNER-UI-001`（第 8 項）
  自此解除封鎖，但尚未在 `docs/improvement-backlog.md` 展開成規格。

### RUNNER-UI-001a — Claude Code, 2026-08-16

- Branch: `feat/runner-event-contract`，自 `origin/main` `cbc3f42`（PR #99 合併後）
  開出。`tasks.md`：`RUNNER-UI-001` 由 Backlog 拆成 a／b 兩個 slice，a 進 Done、
  b 留在 Backlog。規格先以 Planner 身分展開到 `docs/improvement-backlog.md`。
- Files changed: 新增 `fixtures/rs-core/discovery-event-v1.json`（**手寫**）、
  `src-tauri/src/discovery_runner/event_contract_tests.rs`、
  `src/tauri-client/events.test.ts`；改寫 `src/tauri-client/events.ts`；改
  `src/tauri-client/commands.ts`、`src-tauri/src/discovery_runner/mod.rs`
  （只加一行 `#[cfg(test)] mod` 宣告）；更新
  `docs/improvement-backlog.md`、`tasks.md`、`CHANGELOG.md` 與本 handoff。

#### 為什麼先做 slice a

本 handoff §11 明確指出 `events.ts` 的舊 DTO 應由 `RUNNER-UI-001` 處理、不另立
alias task。實際比對後發現舊 DTO 與 backend **沒有任何一個欄位相同**（舊的是
`tested`/`total`/`skipped` 與 `current: { symbol, interval, segment }`）。若把
contract 重定義與 UI 放進同一個 PR，reviewer 無法分辨「UI 對不對」與「contract
對不對」，因此依 AGENTS.md §9 拆成兩個 slice：a 只做 typed boundary，b 做面板。

#### 實作裁決

1. **契約以「一份手寫 fixture、兩邊各自斷言」鎖定。**
   `fixtures/rs-core/discovery-event-v1.json` 是這個目錄下唯一**刻意手寫**的
   fixture：其他 fixture 都是 TS 產生、Rust 驗證，因為運算由 TS 擁有；這裡方向
   相反（Rust 發送、TS 消費），任一邊產生對方的期望值都會讓 drift guard 失去意義。
   Rust 斷言 `serde_json::to_value(struct) == sample`，TS 斷言 parser 接受同一批
   sample，因此任一邊新增／改名欄位都會讓**對面**的測試失敗。
2. **兩種相反的 optional 慣例都必須被覆蓋。** `candidate`/`bestStrategyId`/
   `errorMessage` 有 `skip_serializing_if`，序列化時**key 不存在**；`score` 沒有，
   gate 未通過時是**明確的 null**。這正是手寫 DTO 最容易錯的地方，fixture 兩種
   permutation 都有 sample，TS parser 兩種都接受並統一正規化為 `null`。
3. **payload 一律解析後才進 UI。** 版本不符／缺必填 key／型別錯／未知 run status／
   整數超出 JS 安全範圍／非有限 score／optional 存在但格式錯，全部拒絕並經
   `onInvalid` 回報（不是靜默吞掉，UI 可提示使用者重新查 `get_discovery_progress`，
   DB 才是 progress 的 source of truth）。未知的**額外** key 則容許，避免 backend
   加欄位就打爛正在執行的視窗；真正的 drift 由 build 時的 fixture 測試抓，而任何
   可觀察的 payload 變更依專案慣例都必須改版本字串，改了就會被前端拒絕。
4. **throttle 重寫為可取消。** 舊實作無法取消（trailing timer 會打進已卸載的元件），
   且當 window 在 timer 執行前重新開啟時會**重複投遞同一個 payload**——progress
   看不出來，results 列表會多一列。mutation check 把舊行為重現為 `[1, 2, 2]`，
   新測試會抓到。時鐘可注入，因此不必透過 timer 內部行為測 window。
5. **補上 `get_active_discovery_run` wrapper。** 這是 startup recovery 把孤兒 run
   轉成 `paused` 後，前端唯一能重新找到它的方法，原本完全沒有 wrapper。

#### Scope

未動任何 emitted payload／event 名稱／command 名稱或簽章——本 slice 只讓前端對齊
後端，不反向修改。未動 UI、`mockClient`、`e2e/`、migration，以及 `src-tauri` 其他
檔案（`mod.rs` 只多一行 test-only 模組宣告）。

#### Validation

- `npm run typecheck`、`npm test`（806，+33）、`npm run build`、`npm run e2e`
  （56，未變）、`cargo check --locked`、`cargo test --locked`（152，+7）全綠。
- 手寫 fixture 一次就與真實 serde 輸出相符（7 個 Rust 契約測試首次執行即通過），
  這也反向證明 fixture 的 camelCase／省略 key／null score 拼寫是正確的。

#### 殘餘風險與後續

- `RUNNER-UI-001b`（面板、mock seam、e2e）尚未展開成規格，是下一個任務。
- 事件是快路徑、DB 才是 progress 的事實來源；slice b 必須在 mount 時先查
  `get_active_discovery_run`，並在丟棄事件後允許重新查詢，不可只靠事件累加。
- `sequence` 已納入型別但尚無消費者：slice b 應用它丟棄過期／重放事件。

#### Adjudication — maintainer, 2026-08-16 (recorded from the PR #99 review)

兩個在 PR #99 內文提出、留給 maintainer 的裁決已回覆，兩者皆**通過並關閉**：

1. **cross-field 僅警告，不設為 fatal — 同意。** 理由（maintainer 原話重點）：這三
   條限制的是**假設品質**，不是可計算性；手動策略可能刻意反轉參數，且相關指標未
   必被當前訊號使用。Discovery 可在搜尋空間層修剪，但不應因此禁止手動執行或保存。
   → 實作維持不變。日後若要改為致命錯誤，必須先在 backlog 重新裁決；本次結論不得
   被當成「暫時如此」。
2. **依賴反轉 — 接受。** 將共用 domain rules 移到只依賴 `strategy` 的 leaf module，
   確實解除了 `discoveryConfig -> randomEntry -> backtestRunner` 循環；舊 import
   path 透過 re-export 保持相容，既有 discovery 測試未改即通過，兩份 parity fixture
   也只變更三個 source hash。
   → 搬移方式維持不變。

裁決同時寫入 `docs/improvement-backlog.md`（STRATEGY-VALIDATION-001 的
Adjudicated scope 第 3 條與 Dependency inversion 段）與 `tasks.md` 的 Done 條目，
因此不需要再從 chat 或 PR 內文回溯。

### RUNNER-UI-001a — review follow-up, 2026-08-16

PR #100 的獨立驗收（Codex）判定 request-changes，發現 **1 個阻擋性契約缺陷**，已修正：

- **必填的 `score` key 可被靜默省略。** Rust 的 `DiscoveryResultEvent.score` 是
  `Option<f64>` 且**沒有** `skip_serializing_if`，所以契約是「key 一定存在，值可為
  有限數字或明確 `null`」。但 `parseDiscoveryResultEvent` 當時用 `isAbsent()` 同時
  接受 `undefined` 與 `null`，因此一個整個遺失 `score` 的 gate-passed payload 會被
  解析成 `{ gatePassed: true, score: null }` 且不觸發 `onInvalid` —— 正好掩蓋本
  slice 要抓的 contract drift，也違反規格的「missing required key 必須拒絕」。
  missing-key matrix 當時也剛好沒有 `score` 這一列。
- 修正：`score` key 必須存在（`'score' in record`），值只接受 `null` 或有限數字；
  另外依 reviewer 建議鎖定 `gatePassed === (score != null)`（Score 只在 Gate 通過時
  計算，`validation_records` 也以 CHECK 強制同一配對），矛盾的 payload 一律丟棄。
- 測試：新增具名測試「score key 遺失（而非為 null）必須拒絕」、gate/score 互相矛盾
  的雙向測試，並把 `score`、`sequence`、`runId` 補進 missing-key matrix。
  Mutation check：還原成舊的 `isAbsent` 行為後，該具名測試確實失敗。
- 同時補上 reviewer 指出的非阻擋缺口：以 mock 掉 `@tauri-apps/api/event` 的
  listener-level 測試，證明三個頻道都**不轉發**無法解析的 payload、改為呼叫
  `onInvalid`（含「沒有提供 onInvalid 時不得拋錯」與 unlisten 行為）。
- 契約文件的措辭也一併修正：原本寫「兩種 spelling 都接受並正規化」，正是誘發此
  缺陷的說法；現在明確區分「OMITTED optional（absent 即契約）」與「ALWAYS-PRESENT
  nullable（key 必須存在）」兩類。

本次追加未動任何 Rust 檔案、payload、event 名稱或 command 簽章。

### RUNNER-UI-001b-1 — Claude Code, 2026-08-16

- Branch: `feat/discovery-run-config`，自 `origin/main` `c236b1b`（PR #100 合併後）
  開出。`tasks.md`：`RUNNER-UI-001b` 再拆為 b-1／b-2，b-1 進 Done。
- Files changed: 新增 `src/services/discoveryRunConfig.ts` 與其測試；更新
  `docs/improvement-backlog.md`（slice b 規格）、`tasks.md`、`CHANGELOG.md` 與本
  handoff。無 UI、無 mock、無 e2e、無 Rust。

#### 為什麼 b 還要再拆

啟動一次 run 必須送出完整的 `discovery-config-v1` envelope：13 個 exact key、10 個
pinned contract version、dataset identity、base preset 與 axes、完整 Gate／Score
config、明確 seed、caps。這是**契約層的決策集合**；渲染進度則是 UI 問題。兩者同一個
PR 無法審查，而且 config 的決策必須先定案，面板才有東西可以蓋在上面。

#### 實作裁決

1. **單一 admission 權威。** builder 用 `parseDiscoveryConfig`（Rust 端鏡像的同一支
   parser）驗證，因此不合法的 run 在工作區就以 path-qualified 訊息失敗，**不會產生
   任何 run row**。builder 自己不新增任何規則 —— 這也讓 STRATEGY-VALIDATION-001 的
   週期規則自動涵蓋 discovery（parser 內部會對 base preset 跑 `checkNumericParam`）。
2. **能推導的就不要問。** `contracts`／`gateConfig`／`scoreConfig` 一律從擁有它們的
   常數複製；`benchmarkCosts` 由 base strategy 推導，因為 envelope 要求兩者一致，
   多一個輸入只會製造不一致。
3. **v1 的 `maxConcurrency` 永遠是 `null`。** 由後端以**它自己的** core count 解析；
   若送出以 WebView `hardwareConcurrency` 驗證過的數字，可能本地通過、後端拒絕。
4. **`rootSeed` 與 `holdingAllowanceBars` 是必填、無隱藏預設。** seed 決定整個
   Random Entry 分布，必須是使用者看得到、留得住、能重新輸入的值；allowance 則是
   VAL-003「caller-approved，0 也要明確」的契約。
5. **strategy 深拷貝進 envelope**，之後在編輯器多打一個字，不會改變已送出的 run。

#### 實作中發現、已釘成行為的三件事（不要重新推導）

- **空 axis list 是合法的**：沒有 axes 的 base 就是「用這個策略跑一次完整驗證」的
  single-candidate run。builder 不得自行發明「至少要一個 axis」的規則；面板若要求，
  那是 UI 的產品決策。
- **candidate cap 由 `enumerateCandidates` 把關，不是 envelope admission。** 超出
  budget 的 grid 在這裡會**建置成功**，由後端在建立任何 candidate／job 之前拒絕
  （RUNNER-CONFIG-001）。相對地，per-axis 值數上限**是** envelope 規則，會在本地失敗。
  b-2 應以既有的 `axisValues` 顯示預估組合數，但**不得**新增第二套 cap 檢查。
- `randomRootSeed` 需要 clamp：`floor(1 * (MAX_U32 + 1))` 剛好超出可接受範圍一格，
  而 generator 是可注入的、可能回傳 1。這是我自己的邊界測試抓到的缺陷。

#### Validation

`npm run typecheck`、`npm test`（837，+20）、`npm run build`（bundle 維持
271.23 kB，證明 builder 尚未進入 UI 相依圖）、`npm run e2e`（56，未變）。未動 Rust
故未重跑 cargo。

#### 下一步

`RUNNER-UI-001b-2`：面板、`?mock=1` discovery seam、e2e。backlog 的「b-2 required
behaviour」是起點，開工前需展開成完整 file／step 計畫。重點提醒：mount 時必須先查
`getActiveRun()`（DB 才是 progress 的事實來源，事件只是快路徑）、用 `sequence` 丟棄
比快照舊的事件、unmount 時 `cancel()` throttle，並把 listener 已實作的
drop-and-report 呈現給使用者而不是吞掉。
