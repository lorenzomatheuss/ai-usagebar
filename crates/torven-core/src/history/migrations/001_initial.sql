CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS usage_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor TEXT NOT NULL,
    account_id TEXT,
    ts INTEGER NOT NULL,
    cost_usd REAL,
    tokens_used INTEGER,
    pct_used REAL,
    metric_kind TEXT NOT NULL,
    raw_payload_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_snapshots_vendor_ts ON usage_snapshots(vendor, ts);
CREATE INDEX IF NOT EXISTS idx_snapshots_account_ts ON usage_snapshots(account_id, ts) WHERE account_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_snapshots_vendor_account_ts ON usage_snapshots(vendor, account_id, ts);
CREATE INDEX IF NOT EXISTS idx_snapshots_ts ON usage_snapshots(ts);

CREATE TABLE IF NOT EXISTS insights_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts INTEGER NOT NULL,
    prompt_version TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    cost_usd REAL NOT NULL,
    latency_ms INTEGER NOT NULL,
    output_json TEXT NOT NULL
);
