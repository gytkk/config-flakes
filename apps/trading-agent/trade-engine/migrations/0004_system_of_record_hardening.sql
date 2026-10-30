ALTER TABLE schema_migrations
    ADD COLUMN checksum TEXT;

ALTER TABLE runtime_runs
    ADD COLUMN config_hash TEXT;

ALTER TABLE runtime_runs
    ADD COLUMN shutdown_reason TEXT;

ALTER TABLE runtime_ticks
    ADD COLUMN tick_sequence INTEGER CHECK (
        tick_sequence IS NULL OR tick_sequence >= 0
    );

ALTER TABLE runtime_ticks
    ADD COLUMN lease_owner TEXT;

ALTER TABLE runtime_ticks
    ADD COLUMN lease_expires_at TEXT;

ALTER TABLE runtime_ticks
    ADD COLUMN duration_ms INTEGER CHECK (
        duration_ms IS NULL OR duration_ms >= 0
    );

ALTER TABLE runtime_ticks
    ADD COLUMN status_detail_json TEXT;

ALTER TABLE portfolio_snapshots
    ADD COLUMN open_orders_json TEXT;

ALTER TABLE portfolio_snapshots
    ADD COLUMN today_fills_json TEXT;

CREATE TABLE IF NOT EXISTS decision_inputs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runtime_runs(id),
    tick_id INTEGER NOT NULL REFERENCES runtime_ticks(id),
    strategy_signal_id INTEGER REFERENCES strategy_signals(id),
    observed_at TEXT NOT NULL,
    provider TEXT NOT NULL,
    symbol TEXT NOT NULL,
    input_summary_json TEXT NOT NULL,
    context_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

ALTER TABLE decision_outputs
    ADD COLUMN decision_input_id INTEGER REFERENCES decision_inputs(id);

ALTER TABLE risk_evaluations
    ADD COLUMN policy_snapshot_json TEXT;

ALTER TABLE execution_requests
    ADD COLUMN execution_request_id TEXT;

ALTER TABLE execution_requests
    ADD COLUMN risk_evaluation_id INTEGER REFERENCES risk_evaluations(id);

ALTER TABLE execution_requests
    ADD COLUMN client_order_id TEXT REFERENCES broker_command_journal(client_order_id);

ALTER TABLE execution_requests
    ADD COLUMN broker_order_id TEXT;

ALTER TABLE execution_requests
    ADD COLUMN broker_order_orgno TEXT;

ALTER TABLE execution_requests
    ADD COLUMN submitted_at TEXT;

ALTER TABLE execution_requests
    ADD COLUMN completed_at TEXT;

ALTER TABLE execution_requests
    ADD COLUMN rejection_reason TEXT;

ALTER TABLE execution_requests
    ADD COLUMN last_error TEXT;

ALTER TABLE broker_command_journal
    ADD COLUMN execution_request_id INTEGER REFERENCES execution_requests(id);

CREATE TABLE IF NOT EXISTS execution_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_request_id INTEGER NOT NULL REFERENCES execution_requests(id),
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'execution_requested',
            'broker_command_reserved',
            'order_accepted',
            'order_rejected',
            'reconciliation_needed',
            'execution_failed'
        )
    ),
    observed_at TEXT NOT NULL,
    client_order_id TEXT REFERENCES broker_command_journal(client_order_id),
    metadata_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS broker_command_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_order_id TEXT NOT NULL REFERENCES broker_command_journal(client_order_id),
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'command_recorded',
            'submitting',
            'accepted',
            'submitted_ack_unknown',
            'reconciled',
            'reconcile_failed',
            'terminal'
        )
    ),
    observed_at TEXT NOT NULL,
    request_id TEXT,
    metadata_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS fills (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_request_id INTEGER REFERENCES execution_requests(id),
    client_order_id TEXT REFERENCES broker_command_journal(client_order_id),
    broker_order_id TEXT,
    broker_order_orgno TEXT,
    environment TEXT NOT NULL,
    account_suffix TEXT NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    price TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    evidence_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS operator_actions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    observed_at TEXT NOT NULL,
    operator_id TEXT,
    action_type TEXT NOT NULL,
    target_type TEXT,
    target_id TEXT,
    reason TEXT,
    metadata_json TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_runtime_ticks_run_sequence
    ON runtime_ticks(run_id, tick_sequence)
    WHERE tick_sequence IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_market_snapshots_tick_symbol
    ON market_snapshots(run_id, tick_id, symbol);

CREATE INDEX IF NOT EXISTS idx_decision_inputs_tick
    ON decision_inputs(run_id, tick_id);

CREATE INDEX IF NOT EXISTS idx_decision_outputs_input
    ON decision_outputs(decision_input_id);

CREATE INDEX IF NOT EXISTS idx_execution_requests_risk
    ON execution_requests(risk_evaluation_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_execution_requests_request_id
    ON execution_requests(execution_request_id)
    WHERE execution_request_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_execution_requests_client_order_id
    ON execution_requests(client_order_id)
    WHERE client_order_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_execution_events_request
    ON execution_events(execution_request_id, id);

CREATE INDEX IF NOT EXISTS idx_broker_command_events_command
    ON broker_command_events(client_order_id, id);

CREATE INDEX IF NOT EXISTS idx_fills_client_order_id
    ON fills(client_order_id);

CREATE TRIGGER IF NOT EXISTS trg_execution_requests_status_insert
BEFORE INSERT ON execution_requests
WHEN NEW.status NOT IN (
    'requested',
    'journal_reserved',
    'accepted',
    'rejected',
    'reconciliation_needed',
    'failed'
)
BEGIN
    SELECT RAISE(ABORT, 'invalid execution_requests.status');
END;

CREATE TRIGGER IF NOT EXISTS trg_execution_requests_status_update
BEFORE UPDATE OF status ON execution_requests
WHEN NEW.status NOT IN (
    'requested',
    'journal_reserved',
    'accepted',
    'rejected',
    'reconciliation_needed',
    'failed'
)
BEGIN
    SELECT RAISE(ABORT, 'invalid execution_requests.status');
END;
