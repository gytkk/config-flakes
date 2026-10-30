CREATE TABLE IF NOT EXISTS ohlcv_bars (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    observed_at TEXT NOT NULL,
    source TEXT NOT NULL,
    environment TEXT NOT NULL,
    symbol TEXT NOT NULL,
    market_scope TEXT NOT NULL,
    exchange TEXT,
    currency TEXT,
    interval TEXT NOT NULL,
    trading_day TEXT NOT NULL,
    bar_start_at TEXT NOT NULL,
    bar_end_at TEXT NOT NULL,
    adjustment_policy TEXT NOT NULL,
    data_quality_json TEXT NOT NULL,
    open TEXT NOT NULL,
    high TEXT NOT NULL,
    low TEXT NOT NULL,
    close TEXT NOT NULL,
    volume TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ohlcv_bars_symbol_interval_end
    ON ohlcv_bars(symbol, interval, bar_end_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS technical_feature_audits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    observed_at TEXT NOT NULL,
    source TEXT NOT NULL,
    symbol TEXT NOT NULL,
    interval TEXT NOT NULL,
    ohlcv_bar_id INTEGER REFERENCES ohlcv_bars(id),
    feature_set TEXT NOT NULL,
    features_json TEXT NOT NULL,
    inputs_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_technical_feature_audits_symbol_interval
    ON technical_feature_audits(symbol, interval, observed_at DESC, id DESC);
