-- 0001_init — FULL. AlphaFactorForge core schema (per STRATEGY_DISCOVERY.md v3).
-- Phase A uses: datasets, candles, strategy_def, backtest_summary, trades.
-- Phase B/C tables (discovery_runs, discovery_jobs, ai_generations) are
-- created now (schema established) but their commands are stubbed until then.
-- app_settings is available; API KEYS ARE NEVER STORED HERE (use OS keychain).

-- ---------- Market data ----------
CREATE TABLE datasets (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    exchange      TEXT    NOT NULL,
    symbol        TEXT    NOT NULL,
    interval      TEXT    NOT NULL,
    start_time    INTEGER NOT NULL,           -- epoch ms
    end_time      INTEGER NOT NULL,           -- epoch ms
    candle_count  INTEGER NOT NULL DEFAULT 0,
    source        TEXT    NOT NULL,           -- csv | exchange | import
    dataset_hash  TEXT    NOT NULL,           -- hash(symbol+interval+bounds+version)
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(dataset_hash)
);

CREATE TABLE candles (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    dataset_id  INTEGER NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    timestamp   INTEGER NOT NULL,             -- epoch ms
    open        REAL    NOT NULL,
    high        REAL    NOT NULL,
    low         REAL    NOT NULL,
    close       REAL    NOT NULL,
    volume      REAL    NOT NULL,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(dataset_id, timestamp)
);
CREATE INDEX idx_candles_dataset_ts ON candles(dataset_id, timestamp);

-- ---------- Strategies ----------
CREATE TABLE strategy_def (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    name                      TEXT    NOT NULL,
    type                      TEXT    NOT NULL,   -- params|blocks|code|dsl|ai_dsl
    dsl_json                  TEXT,               -- nullable
    original_definition_json  TEXT    NOT NULL,
    param_schema_json         TEXT,               -- nullable
    source                    TEXT    NOT NULL,   -- manual|sweep|traditional|ai
    ai_prompt_hash            TEXT,               -- nullable
    strategy_hash             TEXT    NOT NULL,
    -- Phase A lifecycle ONLY: candidate | validated | rejected.
    -- (paper_live | promoted | quarantined are reserved for Phase D; the
    --  job runner must NOT set them in v1. CHECK enforces the v1 subset.)
    lifecycle                 TEXT    NOT NULL DEFAULT 'candidate'
                              CHECK (lifecycle IN ('candidate','validated','rejected')),
    parent_strategy_id        INTEGER REFERENCES strategy_def(id) ON DELETE SET NULL,
    created_at                TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at                TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(strategy_hash)
);

-- ---------- Backtest results ----------
CREATE TABLE backtest_summary (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_id            INTEGER NOT NULL REFERENCES strategy_def(id) ON DELETE CASCADE,
    dataset_id             INTEGER NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    -- test segment schema exists, but v1 ranking MUST NOT use it.
    segment                TEXT    NOT NULL CHECK (segment IN ('train','validation','test','full')),
    start_time             INTEGER NOT NULL,
    end_time               INTEGER NOT NULL,
    net_return             REAL,
    cagr                   REAL,
    max_drawdown           REAL,
    sharpe                 REAL,
    sortino                REAL,
    calmar                 REAL,
    win_rate               REAL,
    trade_count            INTEGER,
    profit_factor          REAL,
    avg_trade_return       REAL,
    median_trade_return    REAL,
    exposure               REAL,
    turnover               REAL,
    largest_win            REAL,
    largest_loss           REAL,
    consecutive_losses     INTEGER,
    gate_passed            INTEGER,             -- 0/1 (Phase B)
    score                  REAL,                -- (Phase B)
    score_breakdown_json   TEXT,                -- (Phase B)
    benchmark_result_json  TEXT,                -- (Phase B)
    created_at             TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(strategy_id, dataset_id, segment)
);
CREATE INDEX idx_summary_strategy ON backtest_summary(strategy_id);
CREATE INDEX idx_summary_dataset  ON backtest_summary(dataset_id);

CREATE TABLE trades (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    backtest_summary_id  INTEGER NOT NULL REFERENCES backtest_summary(id) ON DELETE CASCADE,
    entry_time           INTEGER NOT NULL,
    exit_time            INTEGER,
    side                 TEXT    NOT NULL,      -- LONG | SHORT
    entry_price          REAL    NOT NULL,
    exit_price           REAL,
    pnl                  REAL,
    pnl_pct              REAL,
    fee                  REAL,
    slippage             REAL,
    reason               TEXT,
    created_at           TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_trades_summary ON trades(backtest_summary_id);

-- ---------- Discovery (Phase B — schema only in Phase A) ----------
CREATE TABLE discovery_runs (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT    NOT NULL,
    status           TEXT    NOT NULL DEFAULT 'idle'
                     CHECK (status IN ('idle','running','paused','completed','failed','cancelled')),
    config_json      TEXT    NOT NULL,
    progress_json    TEXT,
    best_strategy_id INTEGER REFERENCES strategy_def(id) ON DELETE SET NULL,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    started_at       TEXT,
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    completed_at     TEXT
);

CREATE TABLE discovery_jobs (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    discovery_run_id INTEGER NOT NULL REFERENCES discovery_runs(id) ON DELETE CASCADE,
    strategy_id      INTEGER NOT NULL REFERENCES strategy_def(id) ON DELETE CASCADE,
    dataset_id       INTEGER NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    -- v1 discovery jobs run train/validation ONLY (never test).
    segment          TEXT    NOT NULL CHECK (segment IN ('train','validation')),
    status           TEXT    NOT NULL DEFAULT 'queued'
                     CHECK (status IN ('queued','running','done','failed','skipped')),
    result_id        INTEGER REFERENCES backtest_summary(id) ON DELETE SET NULL,
    error_message    TEXT,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_jobs_run ON discovery_jobs(discovery_run_id);

-- ---------- AI generations (Phase C — schema only in Phase A) ----------
CREATE TABLE ai_generations (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    discovery_run_id    INTEGER REFERENCES discovery_runs(id) ON DELETE SET NULL,
    prompt              TEXT    NOT NULL,
    prompt_hash         TEXT    NOT NULL,
    raw_response        TEXT,
    parsed_dsl_json     TEXT,
    validation_status   TEXT    CHECK (validation_status IN ('passed','failed')),
    error_message       TEXT,
    approved            INTEGER NOT NULL DEFAULT 0,
    approved_strategy_id INTEGER REFERENCES strategy_def(id) ON DELETE SET NULL,
    created_at          TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- ---------- App settings (NON-SENSITIVE UI prefs only) ----------
-- NEVER store API keys here. Keys live in the OS keychain (see secrets module).
CREATE TABLE app_settings (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
