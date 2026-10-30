CREATE TABLE IF NOT EXISTS strategy_signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runtime_runs(id),
    tick_id INTEGER NOT NULL REFERENCES runtime_ticks(id),
    observed_at TEXT NOT NULL,
    strategy_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    strength TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS decision_outputs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runtime_runs(id),
    tick_id INTEGER NOT NULL REFERENCES runtime_ticks(id),
    observed_at TEXT NOT NULL,
    provider TEXT NOT NULL,
    raw_output_json TEXT NOT NULL,
    status TEXT NOT NULL,
    error TEXT
);

CREATE TABLE IF NOT EXISTS decision_proposals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runtime_runs(id),
    tick_id INTEGER NOT NULL REFERENCES runtime_ticks(id),
    decision_output_id INTEGER NOT NULL REFERENCES decision_outputs(id),
    observed_at TEXT NOT NULL,
    decision_type TEXT NOT NULL,
    symbol TEXT NOT NULL,
    quantity INTEGER,
    order_type TEXT,
    limit_price TEXT,
    confidence TEXT NOT NULL,
    time_horizon TEXT,
    reason TEXT NOT NULL,
    risk_notes_json TEXT NOT NULL,
    status TEXT NOT NULL,
    rejection_reason TEXT
);

CREATE TABLE IF NOT EXISTS risk_evaluations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runtime_runs(id),
    tick_id INTEGER NOT NULL REFERENCES runtime_ticks(id),
    decision_proposal_id INTEGER NOT NULL REFERENCES decision_proposals(id),
    observed_at TEXT NOT NULL,
    status TEXT NOT NULL,
    order_notional TEXT,
    reasons_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS risk_blocks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    risk_evaluation_id INTEGER NOT NULL REFERENCES risk_evaluations(id),
    reason TEXT NOT NULL,
    detail_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS execution_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runtime_runs(id),
    tick_id INTEGER NOT NULL REFERENCES runtime_ticks(id),
    decision_proposal_id INTEGER NOT NULL REFERENCES decision_proposals(id),
    created_at TEXT NOT NULL,
    status TEXT NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    order_type TEXT NOT NULL,
    limit_price TEXT,
    order_notional TEXT
);

CREATE INDEX IF NOT EXISTS idx_strategy_signals_tick
    ON strategy_signals(run_id, tick_id);

CREATE INDEX IF NOT EXISTS idx_decision_proposals_tick
    ON decision_proposals(run_id, tick_id);

CREATE INDEX IF NOT EXISTS idx_risk_evaluations_tick
    ON risk_evaluations(run_id, tick_id);

CREATE INDEX IF NOT EXISTS idx_execution_requests_status
    ON execution_requests(status);
