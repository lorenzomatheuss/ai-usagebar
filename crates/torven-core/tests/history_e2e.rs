use std::path::Path;

use chrono::Utc;
use torven_core::history::{
    HistoryDb, UsageSnapshot, query_snapshots, record_snapshot, run_retention_janitor,
};

fn open_temp_db(dir: &Path) -> HistoryDb {
    HistoryDb::open(&dir.join("history.db")).expect("history db should open")
}

fn snapshot(vendor: &str, account_id: Option<&str>, ts: i64) -> UsageSnapshot {
    UsageSnapshot {
        vendor: vendor.to_string(),
        account_id: account_id.map(str::to_string),
        ts,
        cost_usd: Some(0.42),
        tokens_used: Some(12_345),
        pct_used: Some(0.67),
        metric_kind: "usd_spent".to_string(),
        raw_payload_json: Some(r#"{"source":"test"}"#.to_string()),
    }
}

#[test]
fn test_record_and_query() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let db = open_temp_db(tempdir.path());
    let base_ts = 1_700_000_000_000;

    let first_id = record_snapshot(
        &db,
        &snapshot("openrouter", Some("openrouter-acme"), base_ts),
    )
    .expect("record first snapshot");
    record_snapshot(
        &db,
        &snapshot("openrouter", Some("openrouter-acme"), base_ts + 1_000),
    )
    .expect("record second snapshot");
    record_snapshot(
        &db,
        &snapshot("openrouter", Some("openrouter-acme"), base_ts + 2_000),
    )
    .expect("record third snapshot");
    record_snapshot(
        &db,
        &snapshot("openrouter", Some("openrouter-other"), base_ts + 3_000),
    )
    .expect("record other account snapshot");

    let rows = query_snapshots(
        &db,
        "openrouter",
        Some("openrouter-acme"),
        base_ts,
        base_ts + 2_000,
    )
    .expect("query snapshots");

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].id, first_id);
    assert_eq!(rows[0].vendor, "openrouter");
    assert_eq!(rows[0].account_id.as_deref(), Some("openrouter-acme"));
    assert_eq!(rows[0].cost_usd, Some(0.42));
    assert_eq!(rows[0].tokens_used, Some(12_345));
    assert_eq!(rows[0].pct_used, Some(0.67));
    assert_eq!(rows[0].metric_kind, "usd_spent");
}

#[test]
fn test_retention_janitor() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let db = open_temp_db(tempdir.path());
    let now = Utc::now().timestamp_millis();
    let day_ms = 24 * 60 * 60 * 1000;

    record_snapshot(&db, &snapshot("zai", Some("zai-old"), now - 120 * day_ms))
        .expect("record old snapshot");
    record_snapshot(&db, &snapshot("zai", Some("zai-recent"), now - 30 * day_ms))
        .expect("record recent snapshot");
    record_snapshot(&db, &snapshot("zai", Some("zai-current"), now))
        .expect("record current snapshot");

    let deleted = run_retention_janitor(&db, 90).expect("retention janitor");
    assert_eq!(deleted, 1);

    let remaining = query_snapshots(&db, "zai", Some("zai-recent"), now - 180 * day_ms, now)
        .expect("query recent account");
    assert_eq!(remaining.len(), 1);

    let old = query_snapshots(&db, "zai", Some("zai-old"), now - 180 * day_ms, now)
        .expect("query old account");
    assert!(old.is_empty());
}

#[test]
fn test_migration_idempotent() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let db_path = tempdir.path().join("history.db");

    HistoryDb::open(&db_path).expect("first migration run");
    let db = HistoryDb::open(&db_path).expect("second migration run");

    record_snapshot(&db, &snapshot("anthropic", None, 1_700_000_000_000))
        .expect("record after idempotent migrations");
    let rows = query_snapshots(&db, "anthropic", None, 1_699_999_999_999, 1_700_000_000_001)
        .expect("query after idempotent migrations");
    assert_eq!(rows.len(), 1);
}
