use std::path::Path;

use chrono::Utc;
use torven_core::history::{
    AccountFilter, HistoryDb, UsageSnapshot, query_snapshots, query_snapshots_paged,
    record_snapshot, run_retention_janitor,
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
        AccountFilter::Specific("openrouter-acme"),
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

    let all = query_snapshots(
        &db,
        "openrouter",
        AccountFilter::All,
        base_ts,
        base_ts + 3_000,
    )
    .expect("query all account snapshots");
    assert_eq!(all.len(), 4);

    record_snapshot(&db, &snapshot("openrouter", None, base_ts + 4_000))
        .expect("record null-account snapshot");
    let null_only = query_snapshots(
        &db,
        "openrouter",
        AccountFilter::Null,
        base_ts,
        base_ts + 4_000,
    )
    .expect("query null-account snapshots");
    assert_eq!(null_only.len(), 1);
    assert!(null_only[0].account_id.is_none());
}

#[test]
fn test_query_pagination() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let db = open_temp_db(tempdir.path());
    let base_ts = 1_700_000_000_000;

    for i in 0..3 {
        record_snapshot(
            &db,
            &snapshot("openrouter", Some("openrouter-acme"), base_ts + i * 1_000),
        )
        .expect("record paginated snapshot");
    }

    let first_page = query_snapshots_paged(
        &db,
        "openrouter",
        AccountFilter::Specific("openrouter-acme"),
        base_ts,
        base_ts + 3_000,
        2,
        None,
    )
    .expect("first page");
    assert_eq!(first_page.items.len(), 2);
    assert_eq!(first_page.next_cursor.as_deref(), Some("2"));

    let second_page = query_snapshots_paged(
        &db,
        "openrouter",
        AccountFilter::Specific("openrouter-acme"),
        base_ts,
        base_ts + 3_000,
        2,
        first_page.next_cursor.as_deref(),
    )
    .expect("second page");
    assert_eq!(second_page.items.len(), 1);
    assert!(second_page.next_cursor.is_none());
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

    let remaining = query_snapshots(
        &db,
        "zai",
        AccountFilter::Specific("zai-recent"),
        now - 180 * day_ms,
        now,
    )
    .expect("query recent account");
    assert_eq!(remaining.len(), 1);

    let old = query_snapshots(
        &db,
        "zai",
        AccountFilter::Specific("zai-old"),
        now - 180 * day_ms,
        now,
    )
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
    let rows = query_snapshots(
        &db,
        "anthropic",
        AccountFilter::Null,
        1_699_999_999_999,
        1_700_000_000_001,
    )
    .expect("query after idempotent migrations");
    assert_eq!(rows.len(), 1);
}
