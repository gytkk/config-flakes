CREATE TABLE IF NOT EXISTS panic_stop_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    active INTEGER NOT NULL CHECK (active IN (0, 1)),
    updated_at TEXT NOT NULL,
    operator_id TEXT,
    reason TEXT
);

INSERT OR IGNORE INTO panic_stop_state (
    id,
    active,
    updated_at,
    operator_id,
    reason
)
VALUES (
    1,
    0,
    '1970-01-01T00:00:00Z',
    NULL,
    'initial state'
);
