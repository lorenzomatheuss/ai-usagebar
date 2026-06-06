//! Story 4.6 (Wave 4) — Monthly budget configuration + month-to-date
//! aggregation for the SwiftUI BudgetBurn gauge.
//!
//! ## Overview
//!
//! The user sets monthly USD spending limits in `~/.config/torven/config.toml`
//! under an optional `[budgets]` section:
//!
//! ```toml
//! [budgets]
//! monthly_usd_total = 100.0
//!
//! [budgets.per_vendor]
//! openrouter = 50.0
//! anthropic  = 30.0
//! openai     = 20.0
//! ```
//!
//! [`parse_budgets`] reads the file with `toml_edit` (same crate already used
//! by `config.rs` for active-account persistence — Story 4.0). The section is
//! optional: a config without `[budgets]` parses to a default
//! [`BudgetConfig`] with `monthly_usd_total = None` and an empty per-vendor
//! map. This is the v1 contract: the SwiftUI `BudgetBurn` view renders
//! `EmptyView()` when no budget is configured, so we MUST NOT treat a missing
//! section as an error.
//!
//! ## Why a separate module
//!
//! `config.rs` is already 1300+ lines and owns the multi-account schema, env
//! resolution, and active-account persistence. Bolting `[budgets]` parsing on
//! the side would muddy that file. The contract is also narrower: budgets are
//! a single-section read with no migration history and no save path (Wave 4
//! is read-only; Wave 5 will add the Settings UI). A dedicated module keeps
//! the surface obvious to future contributors.
//!
//! ## Test surface
//!
//! Four tests cover the AC-8 matrix:
//! - `test_parse_budgets_full` — every field populated
//! - `test_parse_budgets_absent` — `[budgets]` section missing
//! - `test_parse_budgets_negative` — negative value rejected
//! - `test_budget_status_monthly_reset` — UTC month-start computed correctly
//!
//! The fourth test exercises [`month_start_utc_ms`], a pure helper extracted
//! so `get_budget_status` can be tested without monkey-patching
//! `chrono::Utc::now()`.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Datelike, TimeZone, Utc};
use toml_edit::DocumentMut;

/// Parse errors for [`parse_budgets`]. Variants are intentionally narrow —
/// the SwiftUI surface only cares whether the parse succeeded; granular
/// matching is for the CLI/TUI which can log a helpful message.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BudgetConfigError {
    #[error("config.toml read error at {path}: {message}")]
    Io { path: String, message: String },

    #[error("config.toml parse error: {0}")]
    Parse(String),

    #[error(
        "[budgets]: '{field}' must be >= 0, got {got} \
         (use 0 to disable a budget, never a negative number)"
    )]
    NegativeValue { field: String, got: f64 },
}

/// Parsed `[budgets]` section. Both fields are optional in the source TOML —
/// a missing section yields the default value (`None` / empty map). The
/// `[`get_budget_status`]` FFI uses this to short-circuit the SQLite query
/// when no budget is configured.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BudgetConfig {
    /// Global monthly cap in USD. `None` means "no overall cap" — the
    /// per-vendor map may still carry caps.
    pub monthly_usd_total: Option<f64>,

    /// Per-vendor monthly caps in USD, keyed by vendor slug (e.g.
    /// `"openrouter"`). Empty when the user has no per-vendor breakdown.
    pub per_vendor: HashMap<String, f64>,
}

impl BudgetConfig {
    /// True when at least one cap (global or per-vendor) is configured. The
    /// FFI uses this as a fast path: if no budget exists, skip the SQLite
    /// month-to-date aggregation entirely.
    pub fn has_budget(&self) -> bool {
        self.monthly_usd_total.is_some() || !self.per_vendor.is_empty()
    }
}

/// Read `path` and parse the `[budgets]` section. Returns
/// `BudgetConfig::default()` when the file is missing OR the section is
/// absent — both are valid "user has no budget configured" states (per AC-1
/// and AC-5 of Story 4.6).
///
/// Errors only on actual parse failures or invalid (negative) values.
pub fn parse_budgets(path: &Path) -> Result<BudgetConfig, BudgetConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BudgetConfig::default()),
        Err(e) => {
            return Err(BudgetConfigError::Io {
                path: path.display().to_string(),
                message: e.to_string(),
            });
        }
    };
    parse_budgets_str(&text)
}

/// Parse the `[budgets]` section from an in-memory TOML string. Exposed for
/// testability — call sites that already read the file (e.g. the FFI layer
/// reusing the same string for several sections) avoid the double-read.
pub fn parse_budgets_str(text: &str) -> Result<BudgetConfig, BudgetConfigError> {
    if text.trim().is_empty() {
        return Ok(BudgetConfig::default());
    }
    let doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| BudgetConfigError::Parse(e.to_string()))?;

    let Some(section) = doc.get("budgets").and_then(|i| i.as_table()) else {
        // No `[budgets]` section — user has no budget configured. Per AC-1 /
        // AC-5 this is NOT an error; the SwiftUI gauge renders EmptyView.
        return Ok(BudgetConfig::default());
    };

    let monthly_usd_total = match section.get("monthly_usd_total") {
        Some(item) => {
            let v = item.as_float().ok_or_else(|| {
                BudgetConfigError::Parse(
                    "[budgets].monthly_usd_total must be a float (e.g. 100.0)".to_string(),
                )
            })?;
            if v < 0.0 {
                return Err(BudgetConfigError::NegativeValue {
                    field: "monthly_usd_total".to_string(),
                    got: v,
                });
            }
            Some(v)
        }
        None => None,
    };

    let mut per_vendor: HashMap<String, f64> = HashMap::new();
    if let Some(table) = section.get("per_vendor").and_then(|i| i.as_table()) {
        for (vendor, item) in table.iter() {
            let v = item.as_float().ok_or_else(|| {
                BudgetConfigError::Parse(format!("[budgets.per_vendor].{vendor} must be a float"))
            })?;
            if v < 0.0 {
                return Err(BudgetConfigError::NegativeValue {
                    field: format!("per_vendor.{vendor}"),
                    got: v,
                });
            }
            per_vendor.insert(vendor.to_string(), v);
        }
    }

    Ok(BudgetConfig {
        monthly_usd_total,
        per_vendor,
    })
}

/// Compute the Unix-millisecond timestamp of the first instant of the
/// current calendar month in UTC (i.e. `YYYY-MM-01T00:00:00.000Z`).
///
/// Extracted so [`get_budget_status`] in `uniffi_exports.rs` can call this
/// and tests can feed any reference instant via [`month_start_utc_ms_at`]
/// without monkey-patching `chrono::Utc::now()`.
///
/// AC-7 documents the timezone choice: the month boundary is UTC, not local
/// time. Users in GMT-3 will observe up to a 24-hour discrepancy in the
/// reset compared to their wall clock; Wave 5 may add a timezone preference.
pub fn month_start_utc_ms() -> i64 {
    month_start_utc_ms_at(Utc::now())
}

/// Pure helper — compute the month-start ms for any reference instant.
///
/// Uses `with_day(1)` + zero-time then converts to `timestamp_millis()`.
/// `unwrap()` is safe because day 1 is always valid for every month, and
/// the 00:00:00 components are statically valid.
pub fn month_start_utc_ms_at(now: DateTime<Utc>) -> i64 {
    let year = now.year();
    let month = now.month();
    // `with_ymd_and_hms(y, m, 1, 0, 0, 0)` is the idiomatic chrono 0.4 path;
    // `.single()` returns `None` only for ambiguous local times (DST), which
    // never happens in UTC. Use `unwrap()` with that invariant in mind.
    Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .expect("UTC month-start is unambiguous; (y,m,1,0,0,0) is always valid")
        .timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_tmp(s: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(s.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    /// AC-8 test 1: a fully populated `[budgets]` section parses into the
    /// expected `BudgetConfig` with global cap + per-vendor map.
    #[test]
    fn test_parse_budgets_full() {
        let f = write_tmp(
            r#"
[budgets]
monthly_usd_total = 100.0

[budgets.per_vendor]
openrouter = 50.0
anthropic  = 30.0
openai     = 20.0
"#,
        );
        let cfg = parse_budgets(f.path()).expect("full budgets must parse");
        assert_eq!(cfg.monthly_usd_total, Some(100.0));
        assert_eq!(cfg.per_vendor.len(), 3);
        assert_eq!(cfg.per_vendor.get("openrouter"), Some(&50.0));
        assert_eq!(cfg.per_vendor.get("anthropic"), Some(&30.0));
        assert_eq!(cfg.per_vendor.get("openai"), Some(&20.0));
        assert!(cfg.has_budget());
    }

    /// AC-8 test 2: a config WITHOUT a `[budgets]` section yields default
    /// values (no error). Critical to avoid breaking existing configs.
    #[test]
    fn test_parse_budgets_absent() {
        let f = write_tmp(
            r#"
[anthropic]
enabled = true
"#,
        );
        let cfg = parse_budgets(f.path()).expect("absent [budgets] must default, not error");
        assert_eq!(cfg.monthly_usd_total, None);
        assert!(cfg.per_vendor.is_empty());
        assert!(!cfg.has_budget(), "no budget configured");
    }

    /// AC-8 test 3: a negative value (whether global or per-vendor) returns
    /// the `NegativeValue` error variant.
    #[test]
    fn test_parse_budgets_negative() {
        let f = write_tmp(
            r#"
[budgets]
monthly_usd_total = -10.0
"#,
        );
        let err = parse_budgets(f.path()).expect_err("negative cap must error");
        match err {
            BudgetConfigError::NegativeValue { field, got } => {
                assert_eq!(field, "monthly_usd_total");
                assert!(got < 0.0);
            }
            other => panic!("expected NegativeValue, got {other:?}"),
        }

        let f2 = write_tmp(
            r#"
[budgets]
monthly_usd_total = 50.0

[budgets.per_vendor]
openrouter = -1.0
"#,
        );
        let err2 = parse_budgets(f2.path()).expect_err("negative per-vendor cap must error");
        match err2 {
            BudgetConfigError::NegativeValue { field, .. } => {
                assert_eq!(field, "per_vendor.openrouter");
            }
            other => panic!("expected NegativeValue, got {other:?}"),
        }
    }

    /// AC-8 test 4: month-start computation. Reset boundary is the first
    /// instant of the calendar month in UTC.
    #[test]
    fn test_budget_status_monthly_reset() {
        // Reference instant: 2026-06-15T12:34:56Z (mid-June).
        let ref_now = Utc
            .with_ymd_and_hms(2026, 6, 15, 12, 34, 56)
            .single()
            .unwrap();
        let start = month_start_utc_ms_at(ref_now);

        // Expected: 2026-06-01T00:00:00Z = 1748736000 seconds = ms × 1000.
        let expected = Utc
            .with_ymd_and_hms(2026, 6, 1, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp_millis();
        assert_eq!(start, expected);

        // Edge: instant exactly at the boundary returns that same ms.
        let boundary = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).single().unwrap();
        assert_eq!(month_start_utc_ms_at(boundary), expected);

        // Edge: last second of the month still yields the same month-start
        // (NOT the next month) — this is the AC-7 invariant.
        let last_sec = Utc
            .with_ymd_and_hms(2026, 6, 30, 23, 59, 59)
            .single()
            .unwrap();
        assert_eq!(month_start_utc_ms_at(last_sec), expected);

        // Edge: January (year-boundary sanity).
        let jan_mid = Utc.with_ymd_and_hms(2027, 1, 10, 0, 0, 0).single().unwrap();
        let jan_expected = Utc
            .with_ymd_and_hms(2027, 1, 1, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp_millis();
        assert_eq!(month_start_utc_ms_at(jan_mid), jan_expected);
    }

    /// Zero is accepted — AC-2 explicitly notes "valor zero → aceito (budget
    /// de $0 = any spend triggers red)".
    #[test]
    fn test_parse_budgets_zero_accepted() {
        let f = write_tmp(
            r#"
[budgets]
monthly_usd_total = 0.0

[budgets.per_vendor]
openai = 0.0
"#,
        );
        let cfg = parse_budgets(f.path()).expect("zero is a valid budget");
        assert_eq!(cfg.monthly_usd_total, Some(0.0));
        assert_eq!(cfg.per_vendor.get("openai"), Some(&0.0));
    }

    /// `parse_budgets_str` mirrors `parse_budgets`'s contract for empty
    /// input. Important so callers that already have the file contents in
    /// memory don't trip on whitespace-only configs.
    #[test]
    fn test_parse_budgets_empty_string() {
        let cfg = parse_budgets_str("").unwrap();
        assert!(!cfg.has_budget());
        let cfg2 = parse_budgets_str("   \n  ").unwrap();
        assert!(!cfg2.has_budget());
    }

    /// `parse_budgets` on a non-existent file returns default (matches AC-5
    /// / AC-1 "section is opcional"). Tests the I/O fall-through branch.
    #[test]
    fn test_parse_budgets_missing_file() {
        let path = std::path::Path::new("/tmp/does-not-exist-torven-4.6-budgets");
        let cfg = parse_budgets(path).expect("missing file => default, not error");
        assert!(!cfg.has_budget());
    }
}
