# AlphaFactorForge Workspace

**Automated Indicator Discovery Workstation**  
中文定位：**自動因子鍛造與驗證工作站**

AlphaFactorForge started as a Claude Design single-file PWA for crypto market data, strategy backtesting, and paper trading. The product focus is now sharper: automatically design new indicators and strategy hypotheses, then validate them rigorously before anything is trusted. This workspace contains both the working prototype and the Phase A Tauri desktop scaffold that will become the local-first app.

Current direction: keep the existing Web UI concepts, move durable storage and long-running work into Tauri/Rust, use SQLite as the main database, and keep AI/API-key handling out of the frontend.

## Current Status

- The archive `區塊鏈交易策略PWA.zip` has been unpacked into this workspace.
- `AlphaFactorForge.dc.html` is the legacy PWA prototype and can be inspected in a browser.
- `alpha-factor-forge/` is the Tauri v2 + React + TypeScript + Rust + SQLite Phase A scaffold.
- TypeScript baseline has been verified in `alpha-factor-forge/`:
  - `npm install` completed.
  - `npm test` passed: 3 files, 25 tests.
  - `npm run typecheck` passed after narrow strict-mode fixes.
  - `npm run build` passed and produced `alpha-factor-forge/dist/`.
- Native Tauri verification is not complete because `rustc` and `cargo` were not available on PATH during the latest check.
- The folder currently is not a valid Git repository; `git status` fails even though a `.git` directory exists.
- `npm install` reported 5 dependency vulnerabilities. Do not run `npm audit fix --force` casually; it may introduce breaking upgrades.

## Workspace Contents

- `AlphaFactorForge.dc.html`, `Canvas.dc.html`, `support.js`, `manifest.webmanifest`: legacy PWA prototype and runtime files.
- `alpha-factor-forge/`: Tauri desktop scaffold.
- `STRATEGY_DISCOVERY.md`: Strategy Discovery Engine v3 design, with Tauri Desktop architecture finalized.
- `STRATEGY_GUIDE.md`: strategy editor guide for params, rule blocks, and code mode.
- `HISTORY.md`: handoff summary from previous agent work.
- `CONVERSATION_HISTORY.md`: fuller chronological conversation history.
- `tasks.md`: the single active task board. The former `task.md` content has been merged there.
- `AGENTS.md`: shared collaboration contract and project context.
- `screenshots/`, `uploads/`: prototype screenshots and supporting images.

## Architecture

### Legacy PWA Prototype

The current prototype in `AlphaFactorForge.dc.html` is browser-only:

- Canvas candlestick chart with zoom, pan, hover OHLC tooltip, MA/EMA/BB/RSI/VOL overlays, and buy/sell markers.
- Market data fallback across Binance, OKX, and Coinbase.
- Data export/import for reproducible frozen datasets.
- Three strategy editor modes: params, rule blocks, and manual JavaScript expression code mode.
- Backtesting with fees, slippage, position sizing, fill mode, long/short/both direction, stop-loss/take-profit, Bar Magnifier, holdout comparison, parameter sweep heatmap, report export, and Bar Replay.
- Paper trading simulation in the browser.
- localStorage persistence for prototype strategy/paper state.

This prototype is useful reference material, but it is not the target long-term architecture.

### Tauri Desktop Target

The target app is `alpha-factor-forge/`:

- Frontend: Vite, React 18, TypeScript.
- Core logic: pure TS modules under `src/core/*`, with no React/DOM/IO dependency.
- Bridge: frontend calls backend through typed `src/tauri-client/*` wrappers.
- Backend: Tauri v2, Rust 1.77+, `rusqlite` with bundled SQLite.
- Storage: SQLite managed by Rust/Tauri commands.
- Worker: frontend Web Worker only for light interactive backtests, short sweeps, or indicator precompute.
- Heavy jobs: Strategy Discovery belongs in the Rust backend job runner.
- AI: backend-managed keychain/secure storage only; frontend must never store or read API keys.

Current SQLite tables are defined by `alpha-factor-forge/src-tauri/migrations/0001_init.sql`:

- `datasets`
- `candles`
- `strategy_def`
- `backtest_summary`
- `trades`
- `discovery_runs`
- `discovery_jobs`
- `ai_generations`
- `app_settings`

## Non-Negotiable Boundaries

- API keys never go into frontend code, localStorage, SQLite, or plain config files.
- Frontend does not directly call AI APIs.
- AI may only produce validated JSON Strategy DSL.
- Manual code mode is for humans only; AI must never use code mode.
- Test data must not drive generation, tuning, ranking, or AI prompts.
- Long-running Strategy Discovery does not run on the UI thread.
- `localStorage` is acceptable only for non-sensitive UI preferences during the Tauri migration.

## Roadmap

### Phase A: Tauri Foundation

- Verify local Rust/Tauri prerequisites.
- Add required Tauri icons.
- Run `cd alpha-factor-forge/src-tauri && cargo check`.
- Launch `cargo tauri dev` and confirm SQLite initializes in OS app data.
- Complete `backtest_summary` / `trades` persistence.
- Port the PWA UI into React/Tauri structure without direct frontend persistence.

### Phase B: Discovery And Validation

- Train/Validation/Test split with embargo.
- Gate + Score.
- Benchmarks: Buy & Hold, SMA, RSI, Bollinger, Random Entry.
- `strategy_hash` + `dataset_hash` duplicate skip.
- Rust backend discovery queue with pause/resume/cancel/checkpoint.
- Tauri event protocol for progress/result/done.
- Results Explorer that ranks Validation only and hides Test.
- Lifecycle minimum: `candidate -> validated -> rejected`.

### Phase C: Minimal AI Strategy Lab

- Store AI keys through OS keychain/secure storage.
- Backend AI connection test.
- Generate JSON Strategy DSL only.
- Validate DSL through whitelist validator.
- Require manual approval before queueing AI strategies.

### Phase D: Deferred Automation

- paper live / promoted / quarantined lifecycle.
- Hidden Test one-time reveal flow.
- Clustering and family refinement.
- Meme/low-liquidity risk filters.
- Fully automatic walk-forward.
- Full closed-loop AI automation.

Phase D is explicitly out of the first implementation pass.

## Known Issues And Open Questions

Legacy PWA items to verify before or during port:

- RSI panel may occasionally fail to refresh after symbol/interval changes.
- MA period wiring should be verified in chart drawing against strategy state.
- Bar Replay UI exists, but signal-to-bar alignment needs another pass.
- `manifest.webmanifest` exists; Service Worker is not implemented.
- Optional product features remain: walk-forward analysis, multi-asset portfolio backtesting, alerts/webhooks.

Tauri scaffold items:

- `save_backtest_result` and `get_backtest_results` currently return NotImplemented.
- `export_report` is a stub.
- AI, secrets, and discovery commands are stubs.
- App icons are placeholders; `cargo tauri dev` will need generated icon assets.
- Rust/Cargo were not available on PATH during the latest local check.

## Verification

Frontend/core baseline:

```bash
cd alpha-factor-forge
npm install
npm test
npm run typecheck
npm run build
```

Native/Tauri baseline:

```bash
cd alpha-factor-forge/src-tauri
cargo check
```

Run the desktop app only after Rust/Tauri prerequisites and icons are ready:

```bash
cd alpha-factor-forge
cargo tauri dev
```

Browser inspection during the latest setup used:

- Scaffold shell: `http://127.0.0.1:5173/`
- Legacy prototype: `http://127.0.0.1:5174/AlphaFactorForge.dc.html`
