pub mod db;
pub mod export;
pub mod migrations;
pub mod retention;

use std::path::PathBuf;

pub use db::{HistoryDb, default_db_path};
pub use retention::run_retention_janitor;

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("failed to create or access history path {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to open history database {path}: {source}")]
    OpenFailed {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    #[error("history migration failed: {message}")]
    MigrationFailed { message: String },

    #[error("history database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("history database connection lock was poisoned")]
    ConnectionPoisoned,

    #[error("could not resolve home directory for default history database path")]
    HomeDirUnavailable,

    #[error("invalid history pagination cursor: {0}")]
    InvalidCursor(String),

    #[error("invalid history account filter: {0}")]
    InvalidAccountFilter(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageSnapshot {
    pub vendor: String,
    pub account_id: Option<String>,
    pub ts: i64,
    pub cost_usd: Option<f64>,
    pub tokens_used: Option<i64>,
    pub pct_used: Option<f64>,
    pub metric_kind: String,
    pub raw_payload_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistorySnapshot {
    pub id: i64,
    pub vendor: String,
    pub account_id: Option<String>,
    pub ts: i64,
    pub cost_usd: Option<f64>,
    pub tokens_used: Option<i64>,
    pub pct_used: Option<f64>,
    pub metric_kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountFilter<'a> {
    All,
    Null,
    Specific(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PagedHistorySnapshots {
    pub items: Vec<HistorySnapshot>,
    pub next_cursor: Option<String>,
}

// ---------------------------------------------------------------------------
// Story 4.0.5 (Wave 4) — temporal-bucket aggregation.
//
// SQLite-side `GROUP BY (ts / bucket_size_ms, vendor, account_id, metric_kind)`
// so Swift Charts consumers (Stories 4.3, 4.4, 4.5) receive pre-aggregated
// rows instead of streaming ~43K raw events through FFI. Same surface is
// reused by the Wave 6 evals binary.
// ---------------------------------------------------------------------------

const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 86_400_000;
const WEEK_MS: i64 = 604_800_000;
const SEVEN_DAYS_MS: i64 = 604_800_000;
const THIRTY_DAYS_MS: i64 = 2_592_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketStrategy {
    Auto,
    Hourly,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeBucket {
    pub bucket_start_ts: i64,
    pub bucket_end_ts: i64,
    pub vendor: String,
    pub account_id: Option<String>,
    pub cost_sum_usd: f64,
    pub tokens_sum: u64,
    pub request_count: u32,
    pub metric_kind: String,
}

/// Resolve `BucketStrategy` → bucket width in Unix ms.
///
/// `Auto` derives the width from `until_ts - since_ts` so the number of
/// buckets stays bounded for Swift Charts (AR-8 / Story 4.3 AC-8 — worst
/// case ~840 buckets × 5 vendors = 4200 marks at 7d/Hourly).
pub fn bucket_size_ms(strategy: BucketStrategy, since_ts: i64, until_ts: i64) -> i64 {
    match strategy {
        BucketStrategy::Hourly => HOUR_MS,
        BucketStrategy::Daily => DAY_MS,
        BucketStrategy::Weekly => WEEK_MS,
        BucketStrategy::Auto => {
            let range = until_ts.saturating_sub(since_ts);
            if range <= SEVEN_DAYS_MS {
                HOUR_MS
            } else if range <= THIRTY_DAYS_MS {
                DAY_MS
            } else {
                WEEK_MS
            }
        }
    }
}

pub fn record_snapshot(db: &HistoryDb, snapshot: &UsageSnapshot) -> Result<i64, HistoryError> {
    let conn = db.connection()?;
    conn.execute(
        "
        INSERT INTO usage_snapshots (
            vendor,
            account_id,
            ts,
            cost_usd,
            tokens_used,
            pct_used,
            metric_kind,
            raw_payload_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ",
        (
            snapshot.vendor.as_str(),
            snapshot.account_id.as_deref(),
            snapshot.ts,
            snapshot.cost_usd,
            snapshot.tokens_used,
            snapshot.pct_used,
            snapshot.metric_kind.as_str(),
            snapshot.raw_payload_json.as_deref(),
        ),
    )
    .map_err(HistoryError::Sqlite)?;
    Ok(conn.last_insert_rowid())
}

pub fn query_snapshots(
    db: &HistoryDb,
    vendor: &str,
    account_filter: AccountFilter<'_>,
    since_ts: i64,
    until_ts: i64,
) -> Result<Vec<HistorySnapshot>, HistoryError> {
    Ok(query_snapshots_paged(
        db,
        vendor,
        account_filter,
        since_ts,
        until_ts,
        i64::MAX,
        None,
    )?
    .items)
}

pub fn query_snapshots_paged(
    db: &HistoryDb,
    vendor: &str,
    account_filter: AccountFilter<'_>,
    since_ts: i64,
    until_ts: i64,
    limit: i64,
    cursor: Option<&str>,
) -> Result<PagedHistorySnapshots, HistoryError> {
    let conn = db.connection()?;
    let offset = parse_cursor(cursor)?;
    let limit = normalize_limit(limit);
    let fetch_limit = limit + 1;

    let mut items = match account_filter {
        AccountFilter::Specific(account_id) => {
            let mut stmt = conn
                .prepare(
                    "
                SELECT id, vendor, account_id, ts, cost_usd, tokens_used, pct_used, metric_kind
                FROM usage_snapshots
                WHERE vendor = ?1
                  AND account_id = ?2
                  AND ts >= ?3
                  AND ts <= ?4
                ORDER BY ts ASC, id ASC
                LIMIT ?5 OFFSET ?6
                ",
                )
                .map_err(HistoryError::Sqlite)?;
            stmt.query_map(
                (vendor, account_id, since_ts, until_ts, fetch_limit, offset),
                row_to_snapshot,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(HistoryError::Sqlite)
        }
        AccountFilter::Null => {
            let mut stmt = conn
                .prepare(
                    "
                SELECT id, vendor, account_id, ts, cost_usd, tokens_used, pct_used, metric_kind
                FROM usage_snapshots
                WHERE vendor = ?1
                  AND account_id IS NULL
                  AND ts >= ?2
                  AND ts <= ?3
                ORDER BY ts ASC, id ASC
                LIMIT ?4 OFFSET ?5
                ",
                )
                .map_err(HistoryError::Sqlite)?;
            stmt.query_map(
                (vendor, since_ts, until_ts, fetch_limit, offset),
                row_to_snapshot,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(HistoryError::Sqlite)
        }
        AccountFilter::All => {
            let mut stmt = conn
                .prepare(
                    "
                    SELECT id, vendor, account_id, ts, cost_usd, tokens_used, pct_used, metric_kind
                    FROM usage_snapshots
                    WHERE vendor = ?1
                      AND ts >= ?2
                      AND ts <= ?3
                    ORDER BY ts ASC, id ASC
                    LIMIT ?4 OFFSET ?5
                    ",
                )
                .map_err(HistoryError::Sqlite)?;
            stmt.query_map(
                (vendor, since_ts, until_ts, fetch_limit, offset),
                row_to_snapshot,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(HistoryError::Sqlite)
        }
    }?;

    let next_cursor = if items.len() > limit as usize {
        items.truncate(limit as usize);
        Some((offset + limit).to_string())
    } else {
        None
    };

    Ok(PagedHistorySnapshots { items, next_cursor })
}

/// Query usage snapshots aggregated into temporal buckets.
///
/// Story 4.0.5 (Wave 4). Mirrors the structure of [`query_snapshots_paged`]:
/// branches per [`AccountFilter`] variant so each SQL flavor stays simple
/// (no CASE-driven `GROUP BY`), and follows the same `?N` positional binding
/// convention.
///
/// Returns buckets ordered ASC by `bucket_start_ts`. Buckets with zero rows
/// are not emitted — Swift can fill gaps on the chart side if needed.
/// `metric_kind` is part of the GROUP BY key so heterogeneous `cost_usd_total`
/// / `tokens_used` / `pct_used` series never silently merge.
///
/// `vendor = ""` skips the vendor filter (returns rows for every vendor).
pub fn query_aggregated(
    db: &HistoryDb,
    vendor: &str,
    account_filter: AccountFilter<'_>,
    since_ts: i64,
    until_ts: i64,
    strategy: BucketStrategy,
) -> Result<Vec<TimeBucket>, HistoryError> {
    let conn = db.connection()?;
    let bucket_size = bucket_size_ms(strategy, since_ts, until_ts);
    if bucket_size <= 0 {
        return Ok(Vec::new());
    }

    let rows = match account_filter {
        AccountFilter::All => {
            // Aggregate across all accounts: `account_id` projected as NULL
            // and dropped from the GROUP BY so multiple accounts merge into
            // a single bucket per (bucket_start, vendor, metric_kind).
            let mut stmt = conn
                .prepare(
                    "
                    SELECT
                        (ts / ?1) * ?1 AS bucket_start,
                        vendor,
                        NULL AS bucket_account_id,
                        SUM(COALESCE(cost_usd, 0.0)) AS cost_sum,
                        SUM(COALESCE(tokens_used, 0)) AS tokens_sum,
                        COUNT(*) AS request_count,
                        metric_kind
                    FROM usage_snapshots
                    WHERE ts >= ?2 AND ts < ?3
                      AND (?4 = '' OR vendor = ?4)
                    GROUP BY bucket_start, vendor, metric_kind
                    ORDER BY bucket_start ASC
                    ",
                )
                .map_err(HistoryError::Sqlite)?;
            stmt.query_map((bucket_size, since_ts, until_ts, vendor), |row| {
                row_to_time_bucket(row, bucket_size)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(HistoryError::Sqlite)?
        }
        AccountFilter::Null => {
            // Filter to rows that have NULL account_id (e.g. single-account
            // vendors like Anthropic in v1.0). The grouped account_id is
            // always NULL — same shape as `All` but with the WHERE filter.
            let mut stmt = conn
                .prepare(
                    "
                    SELECT
                        (ts / ?1) * ?1 AS bucket_start,
                        vendor,
                        NULL AS bucket_account_id,
                        SUM(COALESCE(cost_usd, 0.0)) AS cost_sum,
                        SUM(COALESCE(tokens_used, 0)) AS tokens_sum,
                        COUNT(*) AS request_count,
                        metric_kind
                    FROM usage_snapshots
                    WHERE ts >= ?2 AND ts < ?3
                      AND account_id IS NULL
                      AND (?4 = '' OR vendor = ?4)
                    GROUP BY bucket_start, vendor, metric_kind
                    ORDER BY bucket_start ASC
                    ",
                )
                .map_err(HistoryError::Sqlite)?;
            stmt.query_map((bucket_size, since_ts, until_ts, vendor), |row| {
                row_to_time_bucket(row, bucket_size)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(HistoryError::Sqlite)?
        }
        AccountFilter::Specific(account_id) => {
            // Filter and group by the specific account_id; emitted in each
            // returned bucket.
            let mut stmt = conn
                .prepare(
                    "
                    SELECT
                        (ts / ?1) * ?1 AS bucket_start,
                        vendor,
                        account_id,
                        SUM(COALESCE(cost_usd, 0.0)) AS cost_sum,
                        SUM(COALESCE(tokens_used, 0)) AS tokens_sum,
                        COUNT(*) AS request_count,
                        metric_kind
                    FROM usage_snapshots
                    WHERE ts >= ?2 AND ts < ?3
                      AND account_id = ?4
                      AND (?5 = '' OR vendor = ?5)
                    GROUP BY bucket_start, vendor, account_id, metric_kind
                    ORDER BY bucket_start ASC
                    ",
                )
                .map_err(HistoryError::Sqlite)?;
            stmt.query_map(
                (bucket_size, since_ts, until_ts, account_id, vendor),
                |row| row_to_time_bucket(row, bucket_size),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(HistoryError::Sqlite)?
        }
    };

    Ok(rows)
}

fn row_to_time_bucket(row: &rusqlite::Row<'_>, bucket_size: i64) -> rusqlite::Result<TimeBucket> {
    let bucket_start: i64 = row.get(0)?;
    let vendor: String = row.get(1)?;
    let account_id: Option<String> = row.get(2)?;
    let cost_sum: f64 = row.get(3)?;
    // `SUM(COALESCE(tokens_used, 0))` is non-negative in practice, but the
    // `tokens_used` column is `i64?` so SQLite types the SUM as INTEGER.
    // Clamp at 0 before casting to u64 — negative inputs would otherwise
    // bit-cast to a huge positive (`as u64` is a transmute).
    let tokens_sum_i64: i64 = row.get(4)?;
    let request_count_i64: i64 = row.get(5)?;
    let metric_kind: String = row.get(6)?;
    Ok(TimeBucket {
        bucket_start_ts: bucket_start,
        bucket_end_ts: bucket_start + bucket_size,
        vendor,
        account_id,
        cost_sum_usd: cost_sum,
        tokens_sum: tokens_sum_i64.max(0) as u64,
        request_count: request_count_i64.clamp(0, i64::from(u32::MAX)) as u32,
        metric_kind,
    })
}

fn row_to_snapshot(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistorySnapshot> {
    Ok(HistorySnapshot {
        id: row.get(0)?,
        vendor: row.get(1)?,
        account_id: row.get(2)?,
        ts: row.get(3)?,
        cost_usd: row.get(4)?,
        tokens_used: row.get(5)?,
        pct_used: row.get(6)?,
        metric_kind: row.get(7)?,
    })
}

fn normalize_limit(limit: i64) -> i64 {
    if limit <= 0 { 100 } else { limit.min(1_000) }
}

fn parse_cursor(cursor: Option<&str>) -> Result<i64, HistoryError> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    cursor
        .parse::<i64>()
        .ok()
        .filter(|offset| *offset >= 0)
        .ok_or_else(|| HistoryError::InvalidCursor(cursor.to_string()))
}
