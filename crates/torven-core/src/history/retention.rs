use chrono::Utc;

use crate::history::{HistoryDb, HistoryError};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

pub fn run_retention_janitor(db: &HistoryDb, retention_days: u32) -> Result<u64, HistoryError> {
    let cutoff_ts = Utc::now().timestamp_millis() - (retention_days as i64 * DAY_MS);
    let conn = db.connection()?;
    let deleted = conn
        .execute("DELETE FROM usage_snapshots WHERE ts < ?1", [cutoff_ts])
        .map_err(HistoryError::Sqlite)?;
    Ok(deleted as u64)
}
