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
