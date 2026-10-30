CREATE TABLE execution_events_v9 (
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
            'order_cancelled',
            'order_amended',
            'follow_up_rejected'
        )
    ),
    observed_at TEXT NOT NULL,
    client_order_id TEXT REFERENCES broker_command_journal(client_order_id),
    metadata_json TEXT NOT NULL
);

INSERT INTO execution_events_v9 (
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

ALTER TABLE execution_events_v9 RENAME TO execution_events;

CREATE INDEX IF NOT EXISTS idx_execution_events_request
    ON execution_events(execution_request_id, id);
