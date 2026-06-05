// Integration tests for `history::query_aggregated` (Story 4.0.5, Wave 4).
//
// Mirrors the structure of `history_e2e.rs` (Story 1.13). Each test uses an
// in-memory `HistoryDb` so the tests are deterministic and free of fs side
// effects.
//
// Acceptance criteria reference:
//   * AC-7  — same-bucket events sum together
//   * AC-8  — distinct buckets emit in ASC order
//   * AC-9  — `BucketStrategy::Auto` picks Hourly/Daily/Weekly by range
//   * AC-10 — vendor filter ("" = all) and account filter modes

use torven_core::history::{
    AccountFilter, BucketStrategy, HistoryDb, UsageSnapshot, query_aggregated, record_snapshot,
};

const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 86_400_000;
const WEEK_MS: i64 = 604_800_000;
const SEVEN_DAYS_MS: i64 = WEEK_MS;
const THIRTY_DAYS_MS: i64 = 2_592_000_000;
const SIXTY_DAYS_MS: i64 = 5_184_000_000;

fn snapshot_with(
    vendor: &str,
    account_id: Option<&str>,
    ts: i64,
    cost_usd: Option<f64>,
    tokens_used: Option<i64>,
) -> UsageSnapshot {
    UsageSnapshot {
        vendor: vendor.to_string(),
        account_id: account_id.map(str::to_string),
        ts,
        cost_usd,
        tokens_used,
        pct_used: None,
        metric_kind: "usd_spent".to_string(),
        raw_payload_json: None,
    }
}

// ---------------------------------------------------------------------------
// AC-7 — three events inside one hour bucket merge into a single TimeBucket
// with summed cost, summed tokens, and request_count = 3.
// ---------------------------------------------------------------------------

#[test]
fn ac_7_three_events_same_hour_bucket_sum_correctly() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");

    // All three timestamps fall in the [3_600_000, 7_200_000) hour bucket.
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 3_600_001, Some(0.01), Some(100)),
    )
    .expect("record 1");
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 5_000_000, Some(0.02), Some(200)),
    )
    .expect("record 2");
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 7_199_999, Some(0.03), Some(300)),
    )
    .expect("record 3");

    let buckets = query_aggregated(
        &db,
        "anthropic",
        AccountFilter::All,
        0,
        10_000_000,
        BucketStrategy::Hourly,
    )
    .expect("query_aggregated");

    assert_eq!(
        buckets.len(),
        1,
        "three same-bucket events => one TimeBucket"
    );
    let b = &buckets[0];
    assert_eq!(b.bucket_start_ts, 3_600_000);
    assert_eq!(b.bucket_end_ts, 7_200_000);
    assert_eq!(b.vendor, "anthropic");
    assert_eq!(b.account_id, None, "All mode renders account_id as None");
    assert!(
        (b.cost_sum_usd - 0.06).abs() < 1e-9,
        "0.01 + 0.02 + 0.03 = 0.06, got {}",
        b.cost_sum_usd
    );
    assert_eq!(b.tokens_sum, 600, "100 + 200 + 300 = 600");
    assert_eq!(b.request_count, 3);
    assert_eq!(b.metric_kind, "usd_spent");
}

// ---------------------------------------------------------------------------
// AC-8 — two events in distinct hour buckets => 2 TimeBuckets, ordered ASC.
// ---------------------------------------------------------------------------

#[test]
fn ac_8_two_events_distinct_buckets_emit_ascending() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");

    // Bucket 1: [3_600_000, 7_200_000)  — one event
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 3_600_001, Some(0.10), Some(10)),
    )
    .expect("record 1");
    // Bucket 2: [7_200_000, 10_800_000) — one event
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 7_200_001, Some(0.20), Some(20)),
    )
    .expect("record 2");

    let buckets = query_aggregated(
        &db,
        "anthropic",
        AccountFilter::All,
        0,
        15_000_000,
        BucketStrategy::Hourly,
    )
    .expect("query_aggregated");

    assert_eq!(buckets.len(), 2);
    assert!(
        buckets[1].bucket_start_ts > buckets[0].bucket_start_ts,
        "ORDER BY bucket_start ASC: {} should be > {}",
        buckets[1].bucket_start_ts,
        buckets[0].bucket_start_ts
    );
    assert_eq!(buckets[0].bucket_start_ts, 3_600_000);
    assert_eq!(buckets[1].bucket_start_ts, 7_200_000);
}

// ---------------------------------------------------------------------------
// AC-9 — `BucketStrategy::Auto` picks Hourly / Daily / Weekly by range width.
// Verified via `bucket_end_ts - bucket_start_ts`.
// ---------------------------------------------------------------------------

#[test]
fn ac_9_auto_picks_hourly_for_7d_range() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 0, Some(0.01), Some(1)),
    )
    .expect("record");

    let buckets = query_aggregated(
        &db,
        "anthropic",
        AccountFilter::All,
        0,
        SEVEN_DAYS_MS,
        BucketStrategy::Auto,
    )
    .expect("query_aggregated");

    assert_eq!(buckets.len(), 1);
    assert_eq!(
        buckets[0].bucket_end_ts - buckets[0].bucket_start_ts,
        HOUR_MS,
        "7d range under Auto must resolve to Hourly buckets"
    );
}

#[test]
fn ac_9_auto_picks_daily_for_30d_range() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 0, Some(0.01), Some(1)),
    )
    .expect("record");

    let buckets = query_aggregated(
        &db,
        "anthropic",
        AccountFilter::All,
        0,
        THIRTY_DAYS_MS,
        BucketStrategy::Auto,
    )
    .expect("query_aggregated");

    assert_eq!(buckets.len(), 1);
    assert_eq!(
        buckets[0].bucket_end_ts - buckets[0].bucket_start_ts,
        DAY_MS,
        "30d range under Auto must resolve to Daily buckets"
    );
}

#[test]
fn ac_9_auto_picks_weekly_for_60d_range() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 0, Some(0.01), Some(1)),
    )
    .expect("record");

    let buckets = query_aggregated(
        &db,
        "anthropic",
        AccountFilter::All,
        0,
        SIXTY_DAYS_MS,
        BucketStrategy::Auto,
    )
    .expect("query_aggregated");

    assert_eq!(buckets.len(), 1);
    assert_eq!(
        buckets[0].bucket_end_ts - buckets[0].bucket_start_ts,
        WEEK_MS,
        ">30d range under Auto must resolve to Weekly buckets"
    );
}

// ---------------------------------------------------------------------------
// AC-10 — vendor filter ("" = all) and AccountFilter modes.
// ---------------------------------------------------------------------------

#[test]
fn ac_10_empty_vendor_returns_all_vendors() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 3_600_001, Some(0.01), Some(10)),
    )
    .expect("record anthropic");
    record_snapshot(
        &db,
        &snapshot_with(
            "openrouter",
            Some("openrouter-acme"),
            3_600_002,
            Some(0.02),
            Some(20),
        ),
    )
    .expect("record openrouter");

    let buckets = query_aggregated(
        &db,
        "", // empty vendor => all
        AccountFilter::All,
        0,
        10_000_000,
        BucketStrategy::Hourly,
    )
    .expect("query_aggregated");

    let vendors: Vec<&str> = buckets.iter().map(|b| b.vendor.as_str()).collect();
    assert!(vendors.contains(&"anthropic"));
    assert!(vendors.contains(&"openrouter"));
    assert_eq!(
        buckets.len(),
        2,
        "two vendors in one bucket window => 2 grouped rows (one per vendor)"
    );
}

#[test]
fn ac_10_specific_vendor_filters_others_out() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");
    record_snapshot(
        &db,
        &snapshot_with("anthropic", None, 3_600_001, Some(0.01), Some(10)),
    )
    .expect("record anthropic");
    record_snapshot(
        &db,
        &snapshot_with(
            "openrouter",
            Some("openrouter-acme"),
            3_600_002,
            Some(0.02),
            Some(20),
        ),
    )
    .expect("record openrouter");

    let buckets = query_aggregated(
        &db,
        "openrouter",
        AccountFilter::All,
        0,
        10_000_000,
        BucketStrategy::Hourly,
    )
    .expect("query_aggregated");

    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].vendor, "openrouter");
}

#[test]
fn ac_10_aggregate_across_accounts_emits_none_account_id() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");
    record_snapshot(
        &db,
        &snapshot_with(
            "openrouter",
            Some("openrouter-acme"),
            3_600_001,
            Some(0.10),
            Some(100),
        ),
    )
    .expect("record acme");
    record_snapshot(
        &db,
        &snapshot_with(
            "openrouter",
            Some("openrouter-other"),
            3_600_002,
            Some(0.20),
            Some(200),
        ),
    )
    .expect("record other");

    let buckets = query_aggregated(
        &db,
        "openrouter",
        AccountFilter::All, // AggregateAcrossAccounts in story parlance
        0,
        10_000_000,
        BucketStrategy::Hourly,
    )
    .expect("query_aggregated");

    assert_eq!(
        buckets.len(),
        1,
        "two accounts must merge into ONE bucket under All mode"
    );
    assert_eq!(buckets[0].account_id, None);
    assert!((buckets[0].cost_sum_usd - 0.30).abs() < 1e-9);
    assert_eq!(buckets[0].tokens_sum, 300);
    assert_eq!(buckets[0].request_count, 2);
}

#[test]
fn ac_10_specific_account_filters_by_account_id() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");
    record_snapshot(
        &db,
        &snapshot_with(
            "openrouter",
            Some("openrouter-acme"),
            3_600_001,
            Some(0.10),
            Some(100),
        ),
    )
    .expect("record acme");
    record_snapshot(
        &db,
        &snapshot_with(
            "openrouter",
            Some("openrouter-other"),
            3_600_002,
            Some(0.20),
            Some(200),
        ),
    )
    .expect("record other");

    let buckets = query_aggregated(
        &db,
        "openrouter",
        AccountFilter::Specific("openrouter-acme"),
        0,
        10_000_000,
        BucketStrategy::Hourly,
    )
    .expect("query_aggregated");

    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].account_id.as_deref(), Some("openrouter-acme"));
    assert!((buckets[0].cost_sum_usd - 0.10).abs() < 1e-9);
    assert_eq!(buckets[0].tokens_sum, 100);
    assert_eq!(buckets[0].request_count, 1);
}

// ---------------------------------------------------------------------------
// AC-5 — empty range returns Ok(vec![]) (not an error).
// ---------------------------------------------------------------------------

#[test]
fn empty_range_returns_empty_vec_not_error() {
    let db = HistoryDb::open_in_memory().expect("in-memory db");
    let buckets = query_aggregated(
        &db,
        "anthropic",
        AccountFilter::All,
        0,
        10_000_000,
        BucketStrategy::Hourly,
    )
    .expect("query_aggregated on empty db must succeed");
    assert!(buckets.is_empty());
}
