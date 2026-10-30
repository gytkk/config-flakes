CREATE TABLE broker_command_events_v6 (
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
            'terminal',
            'safety_halted'
        )
    ),
    observed_at TEXT NOT NULL,
    request_id TEXT,
    metadata_json TEXT NOT NULL
);

INSERT INTO broker_command_events_v6 (
    id,
    client_order_id,
    event_type,
    observed_at,
    request_id,
    metadata_json
)
SELECT
    id,
    client_order_id,
    event_type,
    observed_at,
    request_id,
    metadata_json
FROM broker_command_events;

DROP TABLE broker_command_events;

ALTER TABLE broker_command_events_v6 RENAME TO broker_command_events;

CREATE INDEX IF NOT EXISTS idx_broker_command_events_command
    ON broker_command_events(client_order_id, id);
