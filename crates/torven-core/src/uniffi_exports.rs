//! UniFFI exports — Rust implementations of every function declared in
//! `torven_core.udl`.
//!
//! This module wires the UDL declarations to actual Rust functions. UniFFI's
//! `build.rs` step generates scaffolding (under `OUT_DIR`) that this module
//! pulls in via `uniffi::include_scaffolding!`. Each free function declared
//! in the UDL must have a matching Rust function in scope here with the same
//! name and signature.
//!
//! Architecture reference: docs/architecture/torven-v1-adr.md#adr-4
//!
//! ## Adding a new FFI function
//!
//! 1. Declare it in `torven_core.udl` (follow the type conventions there).
//! 2. Implement a plain Rust function with the matching name/signature in
//!    this file.
//! 3. Rebuild — UniFFI will fail the build if the UDL and Rust sides drift.
//! 4. Regenerate the Swift bindings (see `apple/scripts/build-xcframework.sh`).
//!
//! ## Notes
//!
//! - We use UniFFI's **UDL mode** (declarative `.udl` file + `build.rs`
//!   scaffolding) rather than the alternative `proc-macro` mode. The UDL
//!   mode has more battle-testing in Firefox iOS / Mullvad / Matrix Element
//!   iOS — see ADR-4 for the trade-off analysis.
//! - The `uniffi::include_scaffolding!` macro must live at the **crate root**
//!   (`src/lib.rs`), not in this submodule, because the generated scaffolding
//!   emits a `UniFfiTag` type that the attribute macros expect to find at
//!   `crate::UniFfiTag`. The crate root re-exports `ping` so the generated
//!   code can resolve `crate::ping`.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::history::{self, HistoryDb, UsageSnapshot};

static HISTORY_DB: OnceLock<Mutex<Option<HistoryDb>>> = OnceLock::new();

/// Smoke-test function exposed via FFI.
///
/// Mirrors the `ping()` declaration in `torven_core.udl`. Returns the
/// constant string `"pong"`. Used by the AR-1 spike (Story 1.2) to validate
/// the Rust -> FFI -> Swift pipeline end-to-end before any business surface
/// is added.
pub fn ping() -> String {
    "pong".to_string()
}

/// Vendor metadata record crossed across the FFI boundary.
///
/// Mirrors the `dictionary VendorInfo` declaration in `torven_core.udl`.
/// The field names and types MUST match the UDL — UniFFI generates the Swift
/// `struct VendorInfo` from the UDL and the scaffolding emits a constructor
/// that calls `VendorInfo { id, display_name, is_configured }`. Any rename
/// here breaks codegen with a compiler error pointing at the scaffolding.
///
/// The fields are `pub` (UniFFI requires direct field access for record
/// types; there are no accessor methods generated in UDL mode).
///
/// See [`get_vendor_list`] for the function that returns these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorInfo {
    /// Stable lowercase slug. Lookup key in config + keychain account
    /// namespace. NEVER user-facing.
    pub id: String,
    /// UTF-8 human-readable label rendered in the menu-bar UI.
    pub display_name: String,
    /// `true` when credentials are present and the most recent validation
    /// passed. Story 1.5 hardcodes `false` for every vendor; Story 1.6
    /// wires it to the real config probe.
    pub is_configured: bool,
}

/// Snapshot record exposed to Swift for history reads and writes.
///
/// This mirrors the UDL `dictionary HistorySnapshot`. The public Rust history
/// API has a separate [`UsageSnapshot`] input type that can carry
/// `raw_payload_json`; the FFI intentionally omits raw payloads so crash logs
/// and Swift-side diagnostics do not accidentally expose vendor response
/// bodies.
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

#[derive(Debug, Clone, thiserror::Error)]
pub enum HistoryFfiError {
    #[error("history database is not initialized")]
    NotInitialized,
    #[error("history storage error")]
    Storage,
}

/// Returns the canonical list of LLM vendors Torven knows about.
///
/// Story 1.5 hardcodes all 5 vendors with `is_configured = false`. The order
/// mirrors the legacy Waybar widget rollout (Anthropic → OpenAI →
/// OpenRouter → Z.AI → Gemini). Story 1.6 swaps this implementation for a
/// config-driven probe.
///
/// ## FFI memory ownership (AR-3)
///
/// Each `VendorInfo` is constructed on the Rust heap and transferred to Swift
/// through UniFFI's reference-counted struct pathway. The Swift bindings emit
/// destructors that free the underlying Rust allocations when the
/// corresponding Swift `VendorInfo` goes out of scope. The Story 1.5 spike
/// (see Change Log) validates zero leaks under `leaks --atExit` after 100
/// invocations.
pub fn get_vendor_list() -> Vec<VendorInfo> {
    vec![
        VendorInfo {
            id: "anthropic".to_string(),
            display_name: "Anthropic (Claude)".to_string(),
            is_configured: false,
        },
        VendorInfo {
            id: "openai".to_string(),
            display_name: "OpenAI (Codex)".to_string(),
            is_configured: false,
        },
        VendorInfo {
            id: "openrouter".to_string(),
            display_name: "OpenRouter".to_string(),
            is_configured: false,
        },
        VendorInfo {
            id: "zai".to_string(),
            display_name: "Z.AI".to_string(),
            is_configured: false,
        },
        VendorInfo {
            id: "gemini".to_string(),
            display_name: "Google Gemini".to_string(),
            is_configured: false,
        },
    ]
}

pub fn ffi_init_history(db_path: Option<String>) -> Result<(), HistoryFfiError> {
    let path = match db_path {
        Some(path) => PathBuf::from(path),
        None => history::default_db_path().map_err(HistoryFfiError::from)?,
    };
    let db = HistoryDb::open(&path).map_err(HistoryFfiError::from)?;

    let mut slot = history_slot()
        .lock()
        .map_err(|_| HistoryFfiError::Storage)?;
    *slot = Some(db);
    Ok(())
}

pub fn ffi_query_snapshots(
    vendor: String,
    account_id: Option<String>,
    since_ts: i64,
    until_ts: i64,
) -> Result<Vec<HistorySnapshot>, HistoryFfiError> {
    with_history_db(|db| {
        let snapshots =
            history::query_snapshots(db, &vendor, account_id.as_deref(), since_ts, until_ts)?;
        Ok(snapshots.into_iter().map(HistorySnapshot::from).collect())
    })
}

pub fn ffi_record_snapshot(snapshot: HistorySnapshot) -> Result<(), HistoryFfiError> {
    with_history_db(|db| {
        let snapshot = UsageSnapshot {
            vendor: snapshot.vendor,
            account_id: snapshot.account_id,
            ts: snapshot.ts,
            cost_usd: snapshot.cost_usd,
            tokens_used: snapshot.tokens_used,
            pct_used: snapshot.pct_used,
            metric_kind: snapshot.metric_kind,
            raw_payload_json: None,
        };
        history::record_snapshot(db, &snapshot)?;
        Ok(())
    })
}

pub fn ffi_run_retention_janitor(retention_days: u32) -> Result<(), HistoryFfiError> {
    with_history_db(|db| {
        history::run_retention_janitor(db, retention_days)?;
        Ok(())
    })
}

fn history_slot() -> &'static Mutex<Option<HistoryDb>> {
    HISTORY_DB.get_or_init(|| Mutex::new(None))
}

fn with_history_db<T>(
    f: impl FnOnce(&HistoryDb) -> Result<T, history::HistoryError>,
) -> Result<T, HistoryFfiError> {
    let mut slot = history_slot()
        .lock()
        .map_err(|_| HistoryFfiError::Storage)?;
    if slot.is_none() {
        let path = history::default_db_path().map_err(HistoryFfiError::from)?;
        *slot = Some(HistoryDb::open(&path).map_err(HistoryFfiError::from)?);
    }

    let db = slot.as_ref().ok_or(HistoryFfiError::NotInitialized)?;
    f(db).map_err(HistoryFfiError::from)
}

impl From<history::HistorySnapshot> for HistorySnapshot {
    fn from(value: history::HistorySnapshot) -> Self {
        Self {
            id: value.id,
            vendor: value.vendor,
            account_id: value.account_id,
            ts: value.ts,
            cost_usd: value.cost_usd,
            tokens_used: value.tokens_used,
            pct_used: value.pct_used,
            metric_kind: value.metric_kind,
        }
    }
}

impl From<history::HistoryError> for HistoryFfiError {
    fn from(value: history::HistoryError) -> Self {
        let _ = value;
        Self::Storage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_returns_pong() {
        assert_eq!(ping(), "pong");
    }

    #[test]
    fn get_vendor_list_returns_five_vendors() {
        let vendors = get_vendor_list();
        assert_eq!(vendors.len(), 5, "Story 1.5 expects exactly 5 vendors");
    }

    #[test]
    fn get_vendor_list_has_canonical_order() {
        let vendors = get_vendor_list();
        let ids: Vec<&str> = vendors.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["anthropic", "openai", "openrouter", "zai", "gemini"],
            "Vendor order must match the legacy Waybar rollout priority"
        );
    }

    #[test]
    fn get_vendor_list_all_unconfigured_in_story_1_5() {
        // Story 1.5 hardcodes is_configured=false. Story 1.6 will replace
        // this implementation with a real config probe — when that lands,
        // this assertion should be deleted (the new test will assert that
        // `is_configured` reflects the loaded config).
        let vendors = get_vendor_list();
        assert!(
            vendors.iter().all(|v| !v.is_configured),
            "Story 1.5 hardcodes is_configured=false for all vendors"
        );
    }

    #[test]
    fn vendor_info_display_names_are_non_empty_utf8() {
        for v in get_vendor_list() {
            assert!(
                !v.display_name.is_empty(),
                "vendor {} has empty display_name",
                v.id
            );
        }
    }

    #[test]
    fn get_vendor_list_is_pure() {
        // Spec: get_vendor_list() must be a pure, side-effect-free function.
        // Repeated invocations must return identical data (memory invariance
        // also exercised by the AR-3 leaks spike).
        let a = get_vendor_list();
        let b = get_vendor_list();
        assert_eq!(a, b, "get_vendor_list must be deterministic");
    }
}
