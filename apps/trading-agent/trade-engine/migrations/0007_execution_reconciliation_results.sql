DROP TRIGGER IF EXISTS trg_execution_requests_status_insert;
DROP TRIGGER IF EXISTS trg_execution_requests_status_update;

CREATE TRIGGER trg_execution_requests_status_insert
BEFORE INSERT ON execution_requests
WHEN NEW.status NOT IN (
    'requested',
    'journal_reserved',
    'accepted',
    'rejected',
    'reconciliation_needed',
    'failed',
    'filled',
    'cancelled'
)
BEGIN
    SELECT RAISE(ABORT, 'invalid execution_requests.status');
END;

CREATE TRIGGER trg_execution_requests_status_update
BEFORE UPDATE OF status ON execution_requests
WHEN NEW.status NOT IN (
    'requested',
    'journal_reserved',
    'accepted',
    'rejected',
    'reconciliation_needed',
    'failed',
    'filled',
    'cancelled'
)
BEGIN
    SELECT RAISE(ABORT, 'invalid execution_requests.status');
END;

CREATE TABLE execution_events_v7 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_request_id INTEGER NOT NULL REFERENCES execution_requests(id),
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'execution_requested',
            'broker_command_reserved',
            'order_accepted',
            'order_rejected',
            'reconciliation_needed',
            'execution_failed',
            'reconciliation_succeeded',
            'reconciliation_still_unknown',
            'manual_intervention_required',
            'order_filled',
            'order_cancelled'
        )
    ),
    observed_at TEXT NOT NULL,
    client_order_id TEXT REFERENCES broker_command_journal(client_order_id),
    metadata_json TEXT NOT NULL
);

INSERT INTO execution_events_v7 (
    id,
    execution_request_id,
    event_type,
    observed_at,
    client_order_id,
    metadata_json
)
SELECT
    id,
    execution_request_id,
    event_type,
    observed_at,
    client_order_id,
    metadata_json
FROM execution_events;

DROP TABLE execution_events;

ALTER TABLE execution_events_v7 RENAME TO execution_events;

CREATE INDEX IF NOT EXISTS idx_execution_events_request
    ON execution_events(execution_request_id, id);
