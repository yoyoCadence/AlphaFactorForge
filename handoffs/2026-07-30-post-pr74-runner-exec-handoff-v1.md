# Handoff: PR #74 後續裁決與 RUNNER-EXEC-001 啟動條件

Date: 2026-07-30
Repo: yoyoCadence/AlphaFactorForge
Branch: main
PR: #74
Status: Needs task-board adjudication before RUNNER-EXEC-001 starts

## Summary

PR #74（RUNNER-STORE-001）已合併至 `main`，merge commit 為
`114c55a`。程式驗收已通過；本 handoff 記錄合併後的任務板裁決、
RUNNER-EXEC-001 的必要驗收條件，以及不應混入該 slice 的獨立後續。

## Required Action / Decision

### 1. 裁決 UI port umbrella

`tasks.md` 的 `In Progress` 仍有：

`Port the legacy AlphaFactorForge PWA UI into the React/Tauri structure
(incremental)`

其目前列出的 slices 已全部勾選，但不能直接以原名稱宣稱「完整 legacy
PWA 移植完成」，因為同一份 task board 仍明載以下缺口：

- `stoch*` 訊號及部分 operand 尚待 core STOCH indicator。
- live exchange 資料仍待 backend fetch；目前只有 import / sample seam。
- Paper-live forward test flow 仍在 Backlog。
- strategy delete 仍為 optional / deferred。

裁決建議：

1. 不要讓這個過大的 umbrella 繼續佔用 `In Progress`。
2. 將已完成的 slice 集合以範圍受限的名稱結案，例如
   `Phase A legacy UI port — completed listed slices through Slice 10`，或把它
   改成不帶 lifecycle checkbox 的歷史 epic。
3. 將仍未完成的 parity 項目保留或拆成具名 Backlog tasks；不要用
   「完整移植完成」掩蓋缺口。
4. 採最小、保留既有內容的 task-board 編輯；不要為了整理狀態重寫整段
   slice 歷史。

這符合 `AGENTS.md` 的 Task Granularity 與 lifecycle 規則：一個無法在單一
session 完成的長期 umbrella 不應永久停在 `In Progress`。

### 2. 正式啟動 RUNNER-EXEC-001

PR #74 已合併，因此 `RUNNER-EXEC-001` 是 `Next` 中目前最優先且已解除封鎖
的工作。開始實作前：

1. 從最新 `origin/main` fast-forward 後建立新分支。
2. 移除 `tasks.md` 中已過期的
   `Blocked until RUNNER-STORE-001 merges`。
3. 將 `RUNNER-EXEC-001` 從 `Next` 移至 `In Progress`。
4. 維持原 scope：fixed CPU worker pool、Tauri commands、
   pause/resume/cancel、single writer、versioned events；不含 frontend UI。
5. `RUNNER-UI-001` 繼續封鎖在 EXEC 後，不得提前混入。

### 3. RUNNER-EXEC-001 必要驗收條件

- Commands 接上 store 時移除
  `src-tauri/src/db/discovery.rs` 的 module-level
  `#![allow(dead_code)]`，並處理真正浮現的 dead-code 問題。
- `src-tauri/src/discovery_core/metrics.rs` 的 UTC timestamp
  `.expect()` 不得留在可由 runner 輸入觸發的路徑。EXEC 必須在執行邊界
  做 propagated fail-closed validation，或讓 core 回傳可傳播的錯誤。
- 新增超出 chrono 可表示範圍的 timestamp regression，確認不會 panic，
  且 job/run 會進入正確失敗狀態並留下 `error_message`。
- 完成事件只能在資料庫交易成功後送出；不能先送事件再落庫。
- cancel/fail 後的遲到 worker 結果不得寫入候選成果，也不得送出成功事件。
- 啟動時的 orphan recovery 只能將孤兒 `running` run 暫停並重排 in-flight
  jobs；不得自動恢復 CPU 工作。
- SQLite 寫入維持 single-writer ownership；不要讓 worker threads 共用可競態
  的寫入 connection。

## Independent Follow-ups (Do Not Bundle Into RUNNER-EXEC-001)

以下項目都可獨立進行，但不是 RUNNER-EXEC-001 的合併阻擋：

1. **Identity encoder consolidation**
   - `discovery_core/identity.rs` 與 binary `identity.rs` 有兩份
     `strategy-v2` canonical encoder。
   - 合併會碰到產品寫入邊界，應獨立 review。
   - 必須保留兩個使用端對同一
     `src/core/hashing/identity-v2.fixture.json` 的鎖定，或提供同等強度的
     product-boundary regression。

2. **既有 Clippy 警告**
   - `backtest.rs` 的兩個 `map_or(false, ...)`。
   - `score.rs` 的 `manual_range_contains` 與 clamp-like 警告。
   - 可建立明確的小型 cleanup task / PR；不要順手塞進 EXEC。

3. **RS-CORE-001 indicator edge fixture**
   - 仍待補 constant RSI、period >= series length、ROC zero base。
   - 應由 TypeScript reference 產生期望值，提交 fixture diff，並由 Rust
     parity 讀同一份 fixture；適合獨立 test-only PR。

4. **REGIME-001**
   - 維持 Backlog；目前不應插隊或混入 runner。

## Review Notes

- PR #74 最終程式、migration、文件及 tests 已驗收通過。
- 本機 `cargo test --locked`：107/107 通過。
- PR CI：5/5 通過。
- PR #74 正文仍保留較早的測試數量及「run 沒有失敗原因欄位」敘述，
  與最終程式已不一致。這不是程式阻擋，但為了 audit trail，建議更新 PR
  正文或補一則最終更正留言。
- `tasks.md` 目前仍寫著 RUNNER-EXEC-001 被 RUNNER-STORE-001 阻擋；這是
  合併後必須清掉的 stale status。

## Verification

- Confirmed local branch: `main`.
- Confirmed clean worktree before writing this handoff.
- Confirmed PR #74 merged: merge commit `114c55a`.
- Inspected `tasks.md`, `db/discovery.rs`, `discovery_core/metrics.rs`,
  both Rust identity implementations, the parity handoff, and the committed
  indicator fixture coverage.
- No source code, migration, task status, PR metadata, or GitHub comments were
  changed as part of this review handoff.

## Resolution (added when acted on)

Append the selected UI umbrella disposition, RUNNER-EXEC-001 branch/commit,
task-board changes, and verification results here. Preserve the original
handoff text.

## Resolution

Date: 2026-07-30. Actor: Claude. Branch: `docs/post-pr74-board-adjudication`.
Original handoff text above is preserved unchanged.

### 1. UI port umbrella disposition — closed under a scope-limited name

Adopted the handoff's recommendation, minimally:

- The entry moved from `In Progress` to `Done`, retitled
  **"Legacy PWA UI port — the listed slices, through Slice 10 (closed
  2026-07-30)"**. Every slice line moved VERBATIM; the diff shows the block
  removed from one section and re-added to the other with no wording changes.
- A scope note was added stating that this closes the slice list only, NOT
  full legacy parity, and pointing at where the gaps now live.
- The named gaps became explicit Backlog-adjacent tasks under a new
  `### Legacy-parity gaps` heading in `Next`, so closing the umbrella cannot
  bury them:
  - **PARITY-001** — core STOCH indicator, then the deferred `stoch*` params
    signals and blocks operands.
  - **PARITY-002** — backend candle fetch command (the webview CSP is
    `default-src 'self'`, so live exchange fetch cannot come from the
    frontend; today only file/JSON import and the sample seam exist).
  - **PARITY-003** — strategy delete in the library.
  - Paper-live forward test, walk-forward, and alerts were already tracked
    under Deferred/Optional and were left where they are rather than
    duplicated.

### 2. RUNNER-EXEC-001 started

- Branched from `origin/main` at merge commit `114c55a`.
- The stale `Blocked until RUNNER-STORE-001 merges` text is gone; the entry
  moved `Next` → `In Progress` and records the unblocking merge.
- `RUNNER-UI-001` took its place in `Next`, explicitly blocked behind EXEC.
- Scope unchanged: fixed CPU worker pool, Tauri commands,
  pause/resume/cancel, single writer, versioned events; no frontend UI.

### 3. Acceptance conditions recorded on the task entry

All seven conditions from §3 of this handoff are now written into the
`RUNNER-EXEC-001` entry itself, so they travel with the task rather than
living only here.

One correction to the handoff's framing of the `metrics.rs` condition: the
code comment there assumes "RUNNER-CONFIG must guarantee chrono-representable
timestamps", and that assumption does **not** hold. RUNNER-CONFIG validates
the run envelope; these timestamps come from dataset candles, which it never
inspects. EXEC therefore cannot treat this as already satisfied upstream — it
must validate at the execution boundary or make the core return a propagated
error. The task entry says so explicitly.

### 4. PR #74 audit trail

Corrected by an appended comment rather than by rewriting the merged body,
consistent with this repository's append-only record discipline (and with the
earlier PR #73 finding against editing a prior record in place). The comment
records the three stale claims: review-round count, `cargo test` 96 → **107**,
and the since-removed statement that `discovery_runs` has no failure-reason
column — migration 0003 added `error_message` and `fail_discovery_run` writes
it in the same transaction.

### 5. Independent follow-ups — left unbundled

Identity encoder consolidation, the four pre-existing clippy warnings, the
RS-CORE-001 indicator edge fixtures, and REGIME-001 were all left out of both
this change and RUNNER-EXEC-001, per §Independent Follow-ups.

### Verification

Task-board and documentation only; no source, migration, or test changed.
`git diff` confirms the umbrella block moved without content edits, and
`tasks.md` now has exactly one `## Done` heading with all 43 slice entries
intact.
