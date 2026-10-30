CREATE TABLE IF NOT EXISTS universe_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    observed_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    environment TEXT NOT NULL,
    account_suffix TEXT NOT NULL,
    provider TEXT NOT NULL,
    summary TEXT NOT NULL,
    symbols_json TEXT NOT NULL,
    output_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_universe_snapshots_scope_expiry
    ON universe_snapshots(environment, account_suffix, expires_at DESC, id DESC);
