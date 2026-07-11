# AGENTS.md

This file is the shared collaboration contract for Codex, Claude Code, and human contributors.

---

## 0. Project Context

- **Project name:** AlphaFactorForge
- **Project goal:** Build AlphaFactorForge, an Automated Indicator Discovery Workstation / 自動因子鍛造與驗證工作站. The product goal is to automatically design new indicators and strategy hypotheses, then validate them through reproducible technical-indicator backtesting, strict anti-overfitting discipline, audit-friendly result storage, and local-first desktop workflows. Current local work should start from the unpacked archive contents and prepare the project for incremental local development; do not treat the archived app as fully production-ready.
- **Target users:** Crypto strategy researchers, discretionary/systematic traders, and developers who need local, reproducible backtests, strategy iteration, and eventually AI-assisted Strategy DSL exploration without exposing keys or sensitive data to the frontend.
- **Tech stack:** Vite + React 18 + TypeScript, Vitest, Tauri v2, Rust 1.77+, SQLite via `rusqlite`, `@tauri-apps/api` v2, Web Worker for light frontend jobs, and a legacy single-file PWA/HTML prototype (`AlphaFactorForge.dc.html`) to be ported into the Tauri app.
- **High-risk areas** (auth / DB schema / payments / deployment / etc.): Backtest correctness and no future-data leakage; execution model assumptions; candle/dataset hashing and field mapping; SQLite migration and result persistence; Tauri invoke argument naming; long-running discovery jobs staying off the UI thread; AI API key storage; AI-generated Strategy DSL validation; preventing AI from reaching manual code mode; Tauri icon/build prerequisites.
- **Architecture constraints:** Local-first Tauri desktop app is the target architecture, not a pure frontend PWA or serverless proxy. SQLite is the primary database; localStorage is only for non-sensitive UI preferences. API keys must never be stored in frontend code, localStorage, or SQLite and must be managed by the Rust backend through OS keychain/secure storage. `src/core/*` should remain pure TypeScript with no React/DOM/IO dependencies. Frontend persistence and backend access must go through typed `tauri-client` wrappers. Heavy Strategy Discovery belongs in the Tauri backend job runner; the Web Worker is only for light interactive backtests, short sweeps, or indicator precompute. AI may only produce validated JSON Strategy DSL; code mode remains manual-only. Validation/Test discipline must be preserved: Test data must not drive generation, tuning, ranking, or prompts.
- **Verification commands:** After unpacking/establishing the `alpha-factor-forge/` source tree: `cd alpha-factor-forge && npm install`, `npm test`, `npm run typecheck`, `npm run build`, `cd src-tauri && cargo check`, optional `cargo clippy`, and `cargo tauri dev` after local Tauri prerequisites and app icons are present.

---

## 0.1 Current Technical State

- **Main entry points:** The archive has been unpacked. The Tauri scaffold lives in `alpha-factor-forge/`; its entry points are `alpha-factor-forge/src/main.tsx`, `alpha-factor-forge/src/tauri-client/*`, `alpha-factor-forge/src/workers/backtest.worker.ts`, `alpha-factor-forge/src-tauri/src/main.rs`, and `alpha-factor-forge/src-tauri/migrations/0001_init.sql`. The legacy prototype UI is `AlphaFactorForge.dc.html`. Historical handoff files are now in root `HISTORY.md` and `CONVERSATION_HISTORY.md`.
- **Storage / data model:** Current schema source of truth is `alpha-factor-forge/src-tauri/migrations/0001_init.sql`, which creates `datasets`, `candles`, `strategy_def`, `backtest_summary`, `trades`, `discovery_runs`, `discovery_jobs`, `ai_generations`, and `app_settings`. Phase A uses datasets/candles/strategy/result tables; discovery and AI tables are schema-only until later phases. API keys are explicitly excluded from DB storage.
- **Test coverage:** Vitest covers indicators, Strategy DSL validation, backtest determinism, and the ported services; Playwright drives the React UI via the `?mock=1` seam; Rust `cargo test` covers repositories + file-command helpers. `npm install` / `npm test` / `npm run typecheck` / `npm run build` all pass. For the current test counts and slice status, see the "Current Snapshot" in the root `tasks.md` (the single source of truth for status) rather than any number quoted here.
- **Deployment / cache notes:** Target app identifier is `com.alphafactorforge.desktop`. SQLite is created in the OS app-data directory on first Tauri startup. Tauri build/dev requires generated icon files under `alpha-factor-forge/src-tauri/icons/*`; multi-size icons have been generated. Native Tauri has been verified locally (`cargo check` and `cargo tauri dev` pass). The workspace is a valid Git repository developed via PRs; CI runs typecheck / test / build / cargo-check (incl. `cargo test`) / e2e on every PR.

---

## 1. Execution Modes

Agents must operate in one of two modes:

### Mode A: Planning / Architecture
- Analyze the request
- Propose structure and changes
- Outline risks and next steps
- **DO NOT modify files yet**

### Mode B: Implementation
- Apply changes strictly based on the agreed plan
- Avoid introducing new design decisions mid-implementation

If the mode is unclear, default to **Mode A first**.

For clear low-risk tasks such as typo fixes, focused tests, or small documentation updates, agents may proceed in **Mode B** directly while still summarizing the change afterward.

---

## 2. Scope Control Rules

Agents must strictly limit changes to the requested scope.

Do NOT:
- Refactor unrelated files "while you are here"
- Rename or restructure directories outside the task scope
- Modify styling, formatting, or naming conventions globally without instruction

If an improvement is detected outside scope:
- Propose it instead of implementing it

---

## 3. Prohibited Behaviors

Do not:
- Silently replace or rewrite major files without instruction
- Mix a feature task with broad unrelated cleanup
- Sneak in schema, auth, or deployment edits under an unrelated feature PR
- Turn the repo into multiple conflicting architectural styles

---

## 4. Change Requirements

Every substantial change must make these clear:
- What changed
- Why this change was made
- What risks remain
- What the next recommended step is

The goal is handoff clarity, not just code delivery.

---

## 5. Canonical Baseline & Editing Rules

All changes must treat the current repository content as the canonical baseline.

- Preserve existing language, structure, and major content unless explicitly instructed otherwise
- Prefer **additive edits** over rewrites
- Do NOT replace entire files unless explicitly requested
- Do NOT reorganize large sections without clear instruction

---

## 6. Handoff Friendliness

Code and documentation should be written so another agent or human can continue without relying on private memory or one-off chat context.

- Write module responsibilities clearly
- Keep comments focused and actionable
- Make placeholders explicit
- Prefer obvious extension points over clever shortcuts
- For cross-session / cross-agent context that does not belong in code — PR reviews, design decisions, task handoffs — write a tracked note under `handoffs/` (see `handoffs/README.md` for the naming convention, lifecycle, and template). Append a Resolution section when acted on; do not rely on private chat memory.

---

## 7. Branch / PR Hygiene

At the start of every task:
- Check current branch and worktree status first
- If starting from product baseline, switch to `main`, fetch, and fast-forward from `origin/main` before creating a new branch
- If already on a feature branch, confirm it is the intended branch for this task

Before opening or updating a PR:
- Fetch and fast-forward local `main` from `origin/main`
- Branch from current `main`, not from an older local checkout
- Before pushing, check the branch against `origin/main` again — if `main` moved, rebase first
- Do not re-submit duplicate generated assets or older runtime code under the same filenames

### Codex-Only PR Opening Flow

This subsection is specifically for Codex running in the Windows sandbox. Other agents and human contributors that do not see the Codex `127.0.0.1:9` proxy issue may use the normal GitHub CLI, connector, or web PR flow while still following the branch / PR hygiene rules above.

Use this exact flow when publishing Codex work from this Windows sandbox:

1. Inspect scope first:
   ```powershell
   git status -sb
   git diff --stat
   ```
2. Commit only the intended files with explicit paths. Avoid `git add -A` unless the whole worktree is confirmed in scope.
3. Fetch and rebase before pushing:
   ```powershell
   git fetch origin
   git rebase origin/main
   ```
   If the sandbox blocks `.git` metadata writes, rerun the same git command with escalation; do not change source files to work around it.
4. Re-run the relevant verification commands after the rebase.
5. Push the branch:
   ```powershell
   git push -u origin <branch-name>
   ```
6. Prefer the GitHub connector to open a draft PR. If the connector returns `403 Resource not accessible by integration`, use `gh pr create` as fallback.
7. In this sandbox, `gh auth status` may falsely report `GITHUB_TOKEN` invalid because `HTTPS_PROXY`, `HTTP_PROXY`, and `ALL_PROXY` are set to `http://127.0.0.1:9`. For PR creation fallback, clear only those proxy variables for the command and keep token values private:
   ```powershell
   $env:HTTPS_PROXY=''; $env:HTTP_PROXY=''; $env:ALL_PROXY='';
   gh auth status
   gh pr create --draft --base main --head <branch-name> --title "<title>" --body-file <body-file>
   ```
   Never print token values. If auth still fails after clearing proxies, ask the user to authenticate or use the GitHub web PR URL from `git push`.
8. PR bodies should include: summary, what changed, validation checklist, and any manual test checklist items requested from the user.

---

## 8. Task Lifecycle

Tasks must move through the following states:

**Backlog → Next → In Progress → Done**

Use `tasks.md` as the default lightweight task board unless the project explicitly uses GitHub Issues, Linear, Notion, or another tracker.

Rules:
- Do not start a task that is not in Next or In Progress
- Move task to In Progress before implementation
- Move to Done only when completed
- Do not silently skip or reorder tasks
- For tiny fixes or direct user requests, agents may complete the work first, then add or update the task record afterward

---

## 9. Task Granularity Rule

Tasks must be:
- Small enough to complete in one session
- Clear enough that no interpretation is needed
- Independent enough to not require large refactors

Avoid vague tasks like "implement system", "build feature", or "add 3D".

---

## 10. Security Baseline

### Environment variables
- Never print secret values to the terminal — only check existence:
  ```bash
  [ -n "$API_KEY" ] && echo "API_KEY is set" || echo "API_KEY is missing"
  ```
- Never use `echo $SECRET`, `printenv KEY`, or any command that outputs a value
- Never hardcode secrets in source files
- Never commit `.env` files (use `.env.example` as template)

### General
- Never use `service_role`, admin, server-only, or equivalent privileged keys on the client side
- Database, storage, and API access policies must be explicit — do not rely on default-open behavior
