CREATE TABLE IF NOT EXISTS broker_command_journal (
    client_order_id TEXT PRIMARY KEY,
    parent_client_order_id TEXT REFERENCES broker_command_journal(client_order_id),
    decision_id TEXT,
    command_type TEXT NOT NULL,
    environment TEXT NOT NULL,
    account_key TEXT NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    limit_price TEXT,
    order_type TEXT NOT NULL,
    requested_at TEXT NOT NULL,
    request_id TEXT,
    transport_send_started_at TEXT,
    broker_order_id TEXT,
    broker_order_orgno TEXT,
    target_broker_order_id TEXT,
    target_broker_order_orgno TEXT,
    cancel_or_amend_quantity INTEGER CHECK (
        cancel_or_amend_quantity IS NULL OR cancel_or_amend_quantity > 0
    ),
    amend_price TEXT,
    exchange_code TEXT,
    last_known_state TEXT NOT NULL,
    raw_ack_code TEXT,
    reconcile_required INTEGER NOT NULL CHECK (reconcile_required IN (0, 1)),
    last_reconciled_at TEXT,
    last_reconcile_reason TEXT,
    trading_day TEXT,
    broker_observation_window_start TEXT,
    broker_observation_window_end TEXT,
    sanitized_last_request_metadata_id TEXT,
    sanitized_last_response_metadata_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_broker_command_journal_parent
    ON broker_command_journal(parent_client_order_id);

CREATE INDEX IF NOT EXISTS idx_broker_command_journal_unreconciled
    ON broker_command_journal(reconcile_required, last_known_state);
