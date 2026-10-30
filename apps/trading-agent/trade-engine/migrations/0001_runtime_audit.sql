CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS runtime_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mode TEXT NOT NULL,
    environment TEXT NOT NULL,
    account_suffix TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL,
    error TEXT
);

CREATE TABLE IF NOT EXISTS runtime_ticks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runtime_runs(id),
    mode TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL,
    error TEXT
);

CREATE TABLE IF NOT EXISTS portfolio_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runtime_runs(id),
    tick_id INTEGER NOT NULL REFERENCES runtime_ticks(id),
    observed_at TEXT NOT NULL,
    account_suffix TEXT NOT NULL,
    cash TEXT NOT NULL,
    positions_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS market_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runtime_runs(id),
    tick_id INTEGER NOT NULL REFERENCES runtime_ticks(id),
    observed_at TEXT NOT NULL,
    symbol TEXT NOT NULL,
    last_price TEXT NOT NULL
);
