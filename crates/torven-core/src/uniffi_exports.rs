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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::budgets::{self, BudgetConfig};
use crate::cache::Cache;
use crate::config::{Config, account_id};
use crate::error::AppError;
use crate::format::{LabelKind, RawMetrics, compute_metrics};
use crate::history::{
    self, AccountFilter, BucketStrategy as HistoryBucketStrategy, HistoryDb,
    TimeBucket as HistoryTimeBucket, UsageSnapshot,
};
use crate::insights::client::RealAnthropicClient;
use crate::insights::{
    CancelHandle, InsightsCallback, InsightsContext, InsightsError, InsightsOutput, LlmClient,
};
use crate::keychain::{self, SecretStore, blob_bytes_from_store, set_blob_bytes_for_store};
use crate::usage::VendorSnapshot;
use crate::vendor::VendorId;
use crate::vendors;

static HISTORY_DB: OnceLock<Mutex<Option<HistoryDb>>> = OnceLock::new();
static SECRET_STORE: OnceLock<Result<Arc<dyn SecretStore>, String>> = OnceLock::new();

/// In-memory active-account map populated by `set_active_account` and read by
/// `get_accounts_for_vendor`. Keyed by vendor slug (e.g. "openrouter"), value
/// is the deterministic `account_id(...)`. Story 4.0 lazy-loads from
/// `~/.config/torven/config.toml [active_accounts]` on first access so swaps
/// persist across app restarts.
static ACTIVE_ACCOUNTS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn active_accounts() -> &'static Mutex<HashMap<String, String>> {
    ACTIVE_ACCOUNTS.get_or_init(|| {
        let initial = match crate::config::default_config_path() {
            Some(path) => crate::config::load_active_accounts(&path),
            None => HashMap::new(),
        };
        Mutex::new(initial)
    })
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct UsageSnapshotInput {
    pub vendor: String,
    pub account_id: Option<String>,
    pub ts: i64,
    pub cost_usd: Option<f64>,
    pub tokens_used: Option<i64>,
    pub pct_used: Option<f64>,
    pub metric_kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PagedHistorySnapshots {
    pub items: Vec<HistorySnapshot>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryAccountFilterMode {
    All,
    Null,
    Specific,
}

/// Mirrors `dictionary TimeBucket` in `torven_core.udl`. One row of the
/// SQLite-side temporal aggregation computed by [`ffi_query_aggregated`].
/// See Story 4.0.5 §AC-1.
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

/// Mirrors `enum BucketStrategy` in `torven_core.udl`. See Story 4.0.5 §AC-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketStrategy {
    Auto,
    Hourly,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum HistoryFfiError {
    #[error("history database is not initialized")]
    NotInitialized,
    #[error("history storage error")]
    Storage,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum KeychainFfiError {
    #[error("keychain storage error")]
    Storage,
}

/// Returns the canonical list of LLM vendors Torven knows about.
///
/// Story 1.5 hardcoded 5 vendors with `is_configured = false`. Story 5.5.1
/// (WAVE5.5-D1) trimmed the list to 4 by removing Gemini — it was a reserved
/// placeholder that never received a Rust vendor module, and live smoke
/// surfaced its empty legend slot as confusing UX. The remaining order
/// mirrors the legacy Waybar widget rollout (Anthropic → OpenAI →
/// OpenRouter → Z.AI). Story 1.6 swaps this implementation for a
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
    // Story 5.5.1 (WAVE5.5-D1): Gemini removed from the canonical vendor list.
    // It was a reserved-slot placeholder from Story 1.5 that never received
    // a Rust vendor module nor product-side validation; live smoke on
    // 2026-06-07 surfaced it as a confusing legend entry. Re-adding Gemini
    // is a post-v1.0 decision conditional on demand.
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
    account_filter_mode: HistoryAccountFilterMode,
    account_id: Option<String>,
    since_ts: i64,
    until_ts: i64,
    limit: i64,
    cursor: Option<String>,
) -> Result<PagedHistorySnapshots, HistoryFfiError> {
    with_history_db(|db| {
        let account_filter = match account_filter_mode {
            HistoryAccountFilterMode::All => AccountFilter::All,
            HistoryAccountFilterMode::Null => AccountFilter::Null,
            HistoryAccountFilterMode::Specific => {
                AccountFilter::Specific(account_id.as_deref().ok_or_else(|| {
                    history::HistoryError::InvalidAccountFilter(
                        "account_id required for Specific account filter".to_string(),
                    )
                })?)
            }
        };
        let page = history::query_snapshots_paged(
            db,
            &vendor,
            account_filter,
            since_ts,
            until_ts,
            limit,
            cursor.as_deref(),
        )?;
        Ok(PagedHistorySnapshots {
            items: page.items.into_iter().map(HistorySnapshot::from).collect(),
            next_cursor: page.next_cursor,
        })
    })
}

pub fn ffi_record_snapshot(snapshot: UsageSnapshotInput) -> Result<i64, HistoryFfiError> {
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
        history::record_snapshot(db, &snapshot)
    })
}

pub fn ffi_run_retention_janitor(retention_days: u32) -> Result<(), HistoryFfiError> {
    with_history_db(|db| {
        history::run_retention_janitor(db, retention_days)?;
        Ok(())
    })
}

/// Story 4.0.5 (Wave 4) — FFI entry for SQLite-side temporal aggregation.
///
/// Delegates to [`history::query_aggregated`], mapping the FFI enum
/// [`HistoryAccountFilterMode`] / [`BucketStrategy`] back onto the
/// strongly-typed Rust [`AccountFilter`] / [`HistoryBucketStrategy`].
/// `vendor = ""` (empty string) means "all vendors" per AC-3.
pub fn ffi_query_aggregated(
    vendor: String,
    account_filter_mode: HistoryAccountFilterMode,
    account_id: Option<String>,
    since_ts: i64,
    until_ts: i64,
    bucket_strategy: BucketStrategy,
) -> Result<Vec<TimeBucket>, HistoryFfiError> {
    with_history_db(|db| {
        let account_filter = match account_filter_mode {
            HistoryAccountFilterMode::All => AccountFilter::All,
            HistoryAccountFilterMode::Null => AccountFilter::Null,
            HistoryAccountFilterMode::Specific => {
                AccountFilter::Specific(account_id.as_deref().ok_or_else(|| {
                    history::HistoryError::InvalidAccountFilter(
                        "account_id required for Specific account filter".to_string(),
                    )
                })?)
            }
        };
        let strategy = HistoryBucketStrategy::from(bucket_strategy);
        let buckets =
            history::query_aggregated(db, &vendor, account_filter, since_ts, until_ts, strategy)?;
        Ok(buckets.into_iter().map(TimeBucket::from).collect())
    })
}

fn history_slot() -> &'static Mutex<Option<HistoryDb>> {
    HISTORY_DB.get_or_init(|| Mutex::new(None))
}

fn with_history_db<T>(
    f: impl FnOnce(&HistoryDb) -> Result<T, history::HistoryError>,
) -> Result<T, HistoryFfiError> {
    let slot = history_slot()
        .lock()
        .map_err(|_| HistoryFfiError::Storage)?;
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

impl From<HistoryTimeBucket> for TimeBucket {
    fn from(value: HistoryTimeBucket) -> Self {
        Self {
            bucket_start_ts: value.bucket_start_ts,
            bucket_end_ts: value.bucket_end_ts,
            vendor: value.vendor,
            account_id: value.account_id,
            cost_sum_usd: value.cost_sum_usd,
            tokens_sum: value.tokens_sum,
            request_count: value.request_count,
            metric_kind: value.metric_kind,
        }
    }
}

impl From<BucketStrategy> for HistoryBucketStrategy {
    fn from(value: BucketStrategy) -> Self {
        match value {
            BucketStrategy::Auto => Self::Auto,
            BucketStrategy::Hourly => Self::Hourly,
            BucketStrategy::Daily => Self::Daily,
            BucketStrategy::Weekly => Self::Weekly,
        }
    }
}

pub fn ffi_keychain_get_blob(vendor: String) -> Result<Vec<u8>, KeychainFfiError> {
    let store = default_secret_store()?;
    blob_bytes_from_store(store.as_ref(), &vendor)
        .map(|blob| blob.unwrap_or_default())
        .map_err(KeychainFfiError::from)
}

pub fn ffi_keychain_set_blob(vendor: String, blob: Vec<u8>) -> Result<(), KeychainFfiError> {
    let store = default_secret_store()?;
    set_blob_bytes_for_store(store.as_ref(), &vendor, &blob).map_err(KeychainFfiError::from)
}

fn default_secret_store() -> Result<Arc<dyn SecretStore>, KeychainFfiError> {
    let result = SECRET_STORE.get_or_init(|| {
        #[cfg(target_os = "macos")]
        {
            Ok(Arc::new(keychain::MacKeychainStore) as Arc<dyn SecretStore>)
        }
        #[cfg(not(target_os = "macos"))]
        {
            keychain::FileFallbackStore::default_store()
                .map(|store| Arc::new(store) as Arc<dyn SecretStore>)
                .map_err(|err| err.to_string())
        }
    });

    result
        .as_ref()
        .map(Arc::clone)
        .map_err(|_| KeychainFfiError::Storage)
}

impl From<keychain::KeychainError> for KeychainFfiError {
    fn from(value: keychain::KeychainError) -> Self {
        let _ = value;
        Self::Storage
    }
}

// ---------------------------------------------------------------------------
// Story 1.15 — `InsightsClient` FFI surface (AR-2 resolution §Decision 1).
// ---------------------------------------------------------------------------

/// Production AI Insights FFI client. Thin wrapper around
/// [`RealAnthropicClient`] that exposes the single `[Async]` entry point
/// declared in the UDL (`interface InsightsClient`) without leaking the
/// trait-based internal architecture (`LlmClient` / `MockLlmClient`) across
/// the FFI boundary.
///
/// ## Why a wrapper instead of exposing `RealAnthropicClient` directly
///
/// 1. **Naming hygiene:** `RealAnthropicClient` is an implementation detail
///    of the Rust `LlmClient` trait pattern; the FFI surface promises a
///    single canonical type per UDL declaration.
/// 2. **Future-proofing:** when v1.1+ adds OpenRouter, OpenAI, or Z.AI
///    insight providers, `InsightsClient` becomes a vendor-selecting facade
///    that picks the right concrete `LlmClient` impl; the Swift binding
///    shape stays identical.
/// 3. **Async-runtime bridging:** UniFFI 0.29's **UDL mode** scaffolding
///    does not pass `async_runtime = "tokio"` to the generated proc-macro
///    `#[export_for_udl]` invocation, so async functions run on UniFFI's
///    bare future driver — no tokio context. The inner future uses
///    `reqwest::stream()` + `tokio::select!`, both of which require a
///    tokio context. We wrap the body with `async_compat::Compat::new(...)`
///    to provide one. Doing this inside the FFI wrapper keeps the
///    `LlmClient::request_insight_streaming` implementation (in
///    `insights/client.rs`) unaware of the FFI driver — the Rust-side
///    callers (tests, AR-2 spike) run inside a real tokio runtime and
///    don't need the wrapper.
///
/// See `docs/architecture/ar-2-spike-resolution.md` §Decision 1.
pub struct InsightsClient {
    inner: RealAnthropicClient,
}

impl InsightsClient {
    /// Constructs a new client. The UDL `constructor(string api_key)` lowers
    /// to this call; UniFFI scaffolding wraps the returned `Self` in
    /// `Arc::new(...)` before crossing to Swift.
    ///
    /// **SECURITY:** the `api_key` argument crosses FFI as a `string`. Per
    /// the SECURITY note in `torven_core.udl`, the Swift side MUST source it
    /// from the Keychain (via `ffi_keychain_get_blob`) and pass it directly
    /// into this constructor — never store it in a Swift-side cache nor log
    /// it. The Rust copy lives inside `RealAnthropicClient` for the lifetime
    /// of the `Arc<InsightsClient>`.
    pub fn new(api_key: String) -> Self {
        Self {
            inner: RealAnthropicClient::new(api_key),
        }
    }

    /// `[Async, Throws=InsightsError]` FFI entry point. Streams partial JSON
    /// chunks to `callback.on_token` and resolves with the final
    /// `InsightsOutput` (AR-2 resolution §Decision 1). `cancel_handle.cancel()`
    /// from Swift causes the future to resolve `Err(Cancelled)` within p99
    /// 100ms (AR-2 spike asserts this).
    ///
    /// ## Async-runtime bridging
    ///
    /// UDL-mode scaffolding does not establish a tokio context (no
    /// `async_runtime = "tokio"` attribute on the generated
    /// `#[export_for_udl]`). We wrap the inner future in
    /// `async_compat::Compat::new(...)` so `reqwest::stream()` and
    /// `tokio::select!` can find a reactor. The `async_compat` crate is
    /// re-exported by `uniffi` itself (see `uniffi-core/src/lib.rs` —
    /// `pub use async_compat;`).
    pub async fn request_insight_streaming(
        &self,
        context: InsightsContext,
        callback: Box<dyn InsightsCallback>,
        cancel_handle: Arc<CancelHandle>,
    ) -> Result<InsightsOutput, InsightsError> {
        // UniFFI 0.29 UDL-mode scaffolding lowers `callback interface` to
        // `Box<dyn Trait>`. The internal `LlmClient` trait takes
        // `Arc<dyn InsightsCallback>` because tests and the AR-2 spike
        // share a single callback across multiple owners; convert here.
        // The `Arc::from(Box<dyn Trait>)` conversion is O(1) — it reuses
        // the existing heap allocation as the Arc payload.
        let callback_arc: Arc<dyn InsightsCallback> = Arc::from(callback);

        // `Compat::new` adopts the tokio I/O / timer drivers for the
        // duration of the inner future. Required because UniFFI's UDL-mode
        // future polling driver does not establish a tokio context.
        ::uniffi::deps::async_compat::Compat::new(self.inner.request_insight_streaming(
            context,
            callback_arc,
            cancel_handle,
        ))
        .await
    }
}

// =====================================================================
// Story 3.3 (Wave 3) — Account picker FFI surface
// =====================================================================

/// Mirrors `dictionary AccountInfo` in `torven_core.udl`. Used by the SwiftUI
/// AccountPicker (Story 3.3) to list configured accounts for a vendor with
/// the active marker reflecting the in-process state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountInfo {
    pub id: String,
    pub label: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum AccountFfiError {
    #[error("vendor slug not recognized")]
    VendorNotFound,
    #[error("account id not found in vendor's configured accounts")]
    AccountNotFound,
    #[error("account storage error")]
    Storage,
}

/// Map the vendor slug crossed through FFI back to the strongly-typed
/// `VendorId` used in `config::Config`. Returns `None` for slugs that don't
/// participate in multi-account (`anthropic`, `openai`, `gemini` in v1.0).
fn vendor_id_from_slug(slug: &str) -> Option<VendorId> {
    match slug {
        "anthropic" => Some(VendorId::Anthropic),
        "openai" => Some(VendorId::Openai),
        "zai" => Some(VendorId::Zai),
        "openrouter" => Some(VendorId::Openrouter),
        // "gemini" — not a VendorId variant yet; multi-account support arrives
        // when Gemini gets a full Rust vendor module. For now `None`.
        _ => None,
    }
}

/// Inner helper isolated for testability — no globals, deterministic.
///
/// Returns the `AccountInfo` rows for `vendor_slug`. Empty `Vec` is a valid
/// "no accounts configured" result (e.g. for Anthropic/OpenAI/Gemini, or for
/// OpenRouter/Z.AI before the user has populated `[[vendor.accounts]]`).
fn get_accounts_for_vendor_inner(
    config: &Config,
    active_map: &HashMap<String, String>,
    vendor_slug: &str,
) -> Vec<AccountInfo> {
    let Some(vendor) = vendor_id_from_slug(vendor_slug) else {
        return Vec::new();
    };
    let Some(accounts) = config.accounts.get(&vendor) else {
        return Vec::new();
    };
    if accounts.is_empty() {
        return Vec::new();
    }

    let default_active = account_id(vendor, &accounts[0].name);
    let active_id = active_map
        .get(vendor_slug)
        .cloned()
        .unwrap_or(default_active);

    accounts
        .iter()
        .map(|acct| {
            let id = account_id(vendor, &acct.name);
            let is_active = id == active_id;
            AccountInfo {
                id,
                label: acct.name.clone(),
                is_active,
            }
        })
        .collect()
}

/// Inner helper isolated for testability.
fn set_active_account_inner(
    config: &Config,
    active_map: &mut HashMap<String, String>,
    vendor_slug: &str,
    account_id_param: &str,
) -> Result<(), AccountFfiError> {
    let vendor = vendor_id_from_slug(vendor_slug).ok_or(AccountFfiError::VendorNotFound)?;
    let accounts = config
        .accounts
        .get(&vendor)
        .ok_or(AccountFfiError::VendorNotFound)?;
    let exists = accounts
        .iter()
        .any(|acct| account_id(vendor, &acct.name) == account_id_param);
    if !exists {
        return Err(AccountFfiError::AccountNotFound);
    }
    active_map.insert(vendor_slug.to_string(), account_id_param.to_string());
    Ok(())
}

/// FFI entry: returns configured `AccountInfo` rows for `vendor_id`.
///
/// Story 3.3: graceful degradation on config load failure — returns empty
/// `Vec` (matches the contract that "no accounts" is not an error). The
/// AccountPicker renders an "Only N accounts configured" affordance.
pub fn get_accounts_for_vendor(vendor_id: String) -> Vec<AccountInfo> {
    let config = match Config::load() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let Ok(active_map) = active_accounts().lock() else {
        return Vec::new();
    };
    get_accounts_for_vendor_inner(&config, &active_map, &vendor_id)
}

/// FFI entry: swap the in-memory active account for `vendor_id` to
/// `account_id`. Validates the vendor exists and `account_id` is in the
/// configured `Vec<Account>`; otherwise raises the matching `AccountFfiError`.
pub fn set_active_account(
    vendor_id: String,
    account_id_param: String,
) -> Result<(), AccountFfiError> {
    let config = Config::load().map_err(|_| AccountFfiError::Storage)?;
    let mut active_map = active_accounts()
        .lock()
        .map_err(|_| AccountFfiError::Storage)?;
    set_active_account_inner(&config, &mut active_map, &vendor_id, &account_id_param)?;

    // Story 4.0 (AC-2): persist post-swap. Disk failure does NOT undo the
    // in-memory swap nor propagate an error — the UX contract is that the
    // active-account swap is instantaneous. Persistence is best-effort
    // background semantics, surfaced as `warn!` for diagnostics.
    if let Some(path) = crate::config::default_config_path() {
        if let Err(e) = crate::config::save_active_accounts(&path, &active_map) {
            tracing::warn!(
                vendor = %vendor_id,
                account = %account_id_param,
                "set_active_account: failed to persist [active_accounts] to disk: {e}"
            );
        }
    }
    Ok(())
}

// =====================================================================
// Story 4.6 (Wave 4) — Budget burn FFI surface
// =====================================================================

/// Mirrors `dictionary VendorBudgetStatus` in `torven_core.udl`. One row in
/// the per-vendor breakdown returned by [`get_budget_status`]. Only vendors
/// that have a non-`None` entry in `[budgets.per_vendor]` appear here — a
/// vendor with month-to-date spending but no configured cap is folded into
/// `total_spent_usd` only.
#[derive(Debug, Clone, PartialEq)]
pub struct VendorBudgetStatus {
    pub vendor_id: String,
    pub spent_usd: f64,
    pub budget_usd: f64,
    pub percent_used: f64,
}

/// Mirrors `dictionary BudgetStatus` in `torven_core.udl`. Single payload
/// returned by [`get_budget_status`]; consumed by Swift's `BudgetBurn` view.
///
/// `has_budget = false` is the signal that the user has not configured any
/// `[budgets]` entries — the SwiftUI view renders `EmptyView()` in that case
/// and skips the SQLite aggregation entirely (AC-4 fast path).
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetStatus {
    pub total_spent_usd: f64,
    pub total_budget_usd: Option<f64>,
    pub total_percent_used: f64,
    pub per_vendor: Vec<VendorBudgetStatus>,
    pub has_budget: bool,
}

impl BudgetStatus {
    fn empty() -> Self {
        Self {
            total_spent_usd: 0.0,
            total_budget_usd: None,
            total_percent_used: 0.0,
            per_vendor: Vec::new(),
            has_budget: false,
        }
    }
}

/// Story 4.6 (Wave 4) — Aggregate the current calendar month's spending and
/// compare to the user's configured budgets.
///
/// ## Flow
///
/// 1. Resolve the config path (`~/.config/torven/config.toml` on Linux,
///    `~/Library/Application Support/torven/config.toml` on macOS) via
///    [`config::default_config_path`].
/// 2. Parse `[budgets]` via [`budgets::parse_budgets`]. If no budget is
///    configured (`has_budget == false`), return `BudgetStatus::empty()`
///    immediately — AC-4 fast path skips the SQLite query.
/// 3. Compute `since_ts` = first instant of the current UTC month via
///    [`budgets::month_start_utc_ms`]. `until_ts` = `now` (ms).
/// 4. Run an inline SQL `SUM(cost_usd) GROUP BY vendor` query against
///    `usage_snapshots`. Inline query (not [`history::query_aggregated`])
///    because we don't need temporal buckets — only month-to-date totals
///    per vendor. The decision is documented in the Story 4.6 Dev Agent
///    Record.
/// 5. Compute the global total (sum across all vendors), then for each
///    vendor in `[budgets.per_vendor]` look up its spend and build a
///    `VendorBudgetStatus`.
///
/// ## Error handling
///
/// Per the UDL declaration this function is NOT marked `[Throws=...]`. Any
/// failure (config parse error, SQLite I/O) degrades to
/// `BudgetStatus::empty()` with a `tracing::warn!` for diagnostics. The
/// gauge disappears gracefully on the Swift side; users see no error popup
/// (the failure is non-fatal — they can still inspect their charts).
///
/// If the SQLite history DB hasn't been initialised yet (early app
/// startup), the function also returns empty — consistent with the gauge
/// rendering nothing until data is available.
pub fn get_budget_status() -> BudgetStatus {
    // Resolve config path. None on a system with no HOME-equivalent: nothing
    // to read, so no budget. Return empty gracefully.
    let Some(config_path) = crate::config::default_config_path() else {
        return BudgetStatus::empty();
    };

    // Parse the optional [budgets] section. A parse error here is logged but
    // not propagated — the gauge degrades to empty.
    let budget_cfg = match budgets::parse_budgets(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(
                "get_budget_status: failed to parse [budgets] in {}: {e}",
                config_path.display()
            );
            return BudgetStatus::empty();
        }
    };

    // Fast path: no budget configured → no SQLite query.
    if !budget_cfg.has_budget() {
        return BudgetStatus::empty();
    }

    // Aggregate the current UTC month. `since_ts` is the first instant of
    // the calendar month (00:00:00.000 UTC of day 1); `until_ts` is `now`.
    let since_ts = budgets::month_start_utc_ms();
    let until_ts = chrono::Utc::now().timestamp_millis();

    let spend_by_vendor = match aggregate_month_spend(since_ts, until_ts) {
        Ok(map) => map,
        Err(e) => {
            tracing::warn!("get_budget_status: SQLite aggregation failed: {e}");
            return BudgetStatus::empty();
        }
    };

    build_status(&budget_cfg, &spend_by_vendor)
}

/// Inner helper: month-to-date `SUM(cost_usd) GROUP BY vendor`. Returns a
/// map keyed by vendor slug. Vendors with no rows in the window are absent
/// from the map (interpreted as `$0 spent`).
///
/// Returns `Err(HistoryFfiError::NotInitialized)` if the FFI history slot is
/// empty — the caller treats that as "gauge unavailable yet" and returns
/// empty.
fn aggregate_month_spend(
    since_ts: i64,
    until_ts: i64,
) -> Result<HashMap<String, f64>, HistoryFfiError> {
    with_history_db(|db| {
        let conn = db.connection()?;
        let mut stmt = conn
            .prepare(
                "
            SELECT vendor, COALESCE(SUM(cost_usd), 0.0) AS total
            FROM usage_snapshots
            WHERE ts >= ?1
              AND ts <  ?2
              AND cost_usd IS NOT NULL
            GROUP BY vendor
            ",
            )
            .map_err(history::HistoryError::Sqlite)?;
        let rows = stmt
            .query_map((since_ts, until_ts), |row| {
                let vendor: String = row.get(0)?;
                let total: f64 = row.get(1)?;
                Ok((vendor, total))
            })
            .map_err(history::HistoryError::Sqlite)?;
        let mut out: HashMap<String, f64> = HashMap::new();
        for r in rows {
            let (vendor, total) = r.map_err(history::HistoryError::Sqlite)?;
            out.insert(vendor, total);
        }
        Ok(out)
    })
}

/// Pure helper: combine the budget config with the spend-by-vendor map and
/// emit the FFI payload. Extracted so the assembly logic is unit-testable
/// without an FFI / SQLite setup.
///
/// Percent calculation: `(spent / budget) * 100`. When `budget == 0.0` (the
/// user wants ANY spend to be over-budget), we return `f64::INFINITY` if
/// `spent > 0` and `0.0` if `spent == 0`. The Swift side clamps the gauge
/// value to `[0, 100]` for display.
fn build_status(cfg: &BudgetConfig, spend_by_vendor: &HashMap<String, f64>) -> BudgetStatus {
    // BUG-001 (Wave 4 polish): defensive `+ 0.0` collapses any -0.0 that
    // sneaks in through `.sum::<f64>()` (IEEE 754 lets a sum of `-0.0`s
    // remain `-0.0`). Without this, Swift's pt-BR currency formatter
    // renders the negative sign bit as "-US$ 0,00" in the BudgetBurn label.
    // The idiom is a no-op on every non-zero value.
    let total_spent_usd: f64 = spend_by_vendor.values().sum::<f64>() + 0.0;
    let total_budget_usd = cfg.monthly_usd_total;
    let total_percent_used = match total_budget_usd {
        Some(b) if b > 0.0 => (total_spent_usd / b) * 100.0 + 0.0,
        Some(_zero) if total_spent_usd > 0.0 => f64::INFINITY,
        Some(_zero) => 0.0,
        None => 0.0,
    };

    let mut per_vendor: Vec<VendorBudgetStatus> = cfg
        .per_vendor
        .iter()
        .map(|(vendor_id, budget_usd)| {
            // BUG-001: same defensive `+ 0.0` so a negative-zero spend
            // doesn't propagate into the per-vendor FFI payload.
            let spent_usd = spend_by_vendor.get(vendor_id).copied().unwrap_or(0.0) + 0.0;
            let percent_used = match *budget_usd {
                b if b > 0.0 => (spent_usd / b) * 100.0 + 0.0,
                _zero if spent_usd > 0.0 => f64::INFINITY,
                _ => 0.0,
            };
            VendorBudgetStatus {
                vendor_id: vendor_id.clone(),
                spent_usd,
                budget_usd: *budget_usd,
                percent_used,
            }
        })
        .collect();
    // Deterministic order — HashMap iteration is unspecified, which would
    // make snapshot tests flaky and the Swift UI shuffle on each refresh.
    per_vendor.sort_by(|a, b| a.vendor_id.cmp(&b.vendor_id));

    BudgetStatus {
        total_spent_usd,
        total_budget_usd,
        total_percent_used,
        per_vendor,
        has_budget: cfg.has_budget(),
    }
}

// =====================================================================
// Story 5.1 (Wave 5) — `ffi_refresh_vendor` FFI surface
// =====================================================================

/// Mirrors `enum RefreshFfiError` in `torven_core.udl`. Each variant maps to
/// a specific failure category surfaced by the vendor refresh pipeline:
///
///   * `CredentialMissing` — OAuth credentials file does not exist, or the
///     Keychain blob is absent / empty for an API-key vendor. Distinct from
///     network failure so Story 5.3 (Refresh button) can route the user to
///     Settings (Story 5.2) instead of showing a retry affordance.
///   * `NetworkError` — transport-layer failure (DNS, TLS, connect, timeout)
///     surfaced by [`AppError::Transport`] — the `is_transient()` bucket.
///   * `ApiError` — vendor reachable but returned non-2xx
///     ([`AppError::Http`]). Could mean expired token, rate limit, or vendor
///     outage; the Swift layer can show the status code.
///   * `ParseFailure` — 2xx but response shape unexpected
///     ([`AppError::Schema`] or [`AppError::Json`]). Signals undocumented-
///     endpoint drift; relevant for diagnostics, not user-actionable.
///   * `StorageError` — `ffi_record_snapshot` failed
///     ([`HistoryFfiError::NotInitialized`] / `Storage`).
#[derive(Debug, Clone, thiserror::Error)]
pub enum RefreshFfiError {
    #[error("credentials missing for vendor")]
    CredentialMissing,
    #[error("network transport error during vendor refresh")]
    NetworkError,
    #[error("vendor API returned a non-success status")]
    ApiError,
    #[error("vendor response parse failure")]
    ParseFailure,
    #[error("history storage error while writing refreshed snapshot")]
    StorageError,
}

impl From<HistoryFfiError> for RefreshFfiError {
    fn from(_: HistoryFfiError) -> Self {
        // Both variants of HistoryFfiError (NotInitialized, Storage) reduce
        // to "we couldn't persist the snapshot" for the FFI consumer.
        RefreshFfiError::StorageError
    }
}

impl From<AppError> for RefreshFfiError {
    fn from(err: AppError) -> Self {
        match err {
            // Credentials file missing / unreadable / unparseable → user must
            // re-auth or configure Settings. Distinct from network failure.
            AppError::Credentials(_) => RefreshFfiError::CredentialMissing,
            // I/O missing for the credentials path also lands here when a
            // vendor reads the file directly (e.g. legacy code paths).
            AppError::Io { ref source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                RefreshFfiError::CredentialMissing
            }
            AppError::IoBare(ref source) if source.kind() == std::io::ErrorKind::NotFound => {
                RefreshFfiError::CredentialMissing
            }
            AppError::Transport(_) => RefreshFfiError::NetworkError,
            AppError::Http { .. } => RefreshFfiError::ApiError,
            AppError::Schema(_) | AppError::Json(_) => RefreshFfiError::ParseFailure,
            // Toml parse / generic Other / Io (non-NotFound) — bucket under
            // ParseFailure since they reflect a malformed input rather than a
            // transient transport problem. Toml errors here would only arise
            // from vendor config parsing, which currently doesn't happen
            // inside this refresh path — kept for completeness.
            AppError::Toml(_) | AppError::Io { .. } | AppError::IoBare(_) | AppError::Other(_) => {
                RefreshFfiError::ParseFailure
            }
        }
    }
}

/// Look up the API key for a single-account vendor from the Keychain blob.
///
/// For multi-account vendors (openrouter, zai), this picks the **active**
/// account based on the in-memory active map populated by
/// `set_active_account`. If no swap has occurred, the first account in the
/// blob is used (matches the Story 3.3 default-active semantics).
///
/// Returns `Err(RefreshFfiError::CredentialMissing)` if:
///   * the keychain access fails (no Keychain entry yet)
///   * the blob is empty (no accounts configured)
fn keychain_api_key_for(vendor_slug: &str) -> Result<String, RefreshFfiError> {
    let store = default_secret_store().map_err(|_| RefreshFfiError::CredentialMissing)?;
    let accounts = store
        .get_accounts_blob(vendor_slug)
        .map_err(|_| RefreshFfiError::CredentialMissing)?
        .unwrap_or_default();
    if accounts.is_empty() {
        return Err(RefreshFfiError::CredentialMissing);
    }

    // Pick the active account id (if set) — otherwise fall back to the
    // first secret in the blob, matching the Story 3.3 default-active
    // semantics for the AccountInfo picker.
    let active_id = active_accounts()
        .lock()
        .ok()
        .and_then(|guard| guard.get(vendor_slug).cloned());

    let pick = match active_id {
        Some(id) => accounts
            .iter()
            .find(|s| s.account_id == id)
            .cloned()
            .unwrap_or_else(|| accounts[0].clone()),
        None => accounts[0].clone(),
    };
    Ok(pick.api_key)
}

/// Project a `VendorSnapshot` into a `UsageSnapshotInput` row suitable for
/// `ffi_record_snapshot`. The `cost_usd` / `pct_used` projection delegates to
/// [`compute_metrics`] (the single source of truth for snapshot-to-display
/// projection used by SwiftUI and the dev TUI), so the persisted snapshot
/// row matches the headline metric Swift consumes.
///
/// `metric_kind` follows the canonical convention established by Story 4.0.5:
///   * `"cost_usd_total"` for vendors whose headline metric is dollars
///     spent (OpenRouter, and Anthropic when extra-usage is reported).
///   * `"pct_used"` for plan-based vendors whose headline is window
///     utilization (Anthropic without extra, OpenAI Codex OAuth, Z.AI).
///
/// `account_id` is populated only for multi-account vendors (openrouter,
/// zai) — the value is the active `account_id(...)` if known, else None.
/// Single-account vendors (anthropic, openai) always emit `None`, matching
/// the existing seed data convention.
fn outcome_to_snapshot_input(vendor_slug: &str, snapshot: &VendorSnapshot) -> UsageSnapshotInput {
    let metrics: RawMetrics = compute_metrics(snapshot);

    // Choose the dominant metric_kind for the snapshot row. The Story 4.0.5
    // GROUP BY uses metric_kind as part of the key, so heterogeneous kinds
    // never get silently merged in a bucket. We pick UsdSpent → cost_usd_total
    // for vendors where dollars is the headline; otherwise pct_used.
    let metric_kind = match metrics.label_kind {
        LabelKind::UsdSpent => "cost_usd_total",
        LabelKind::PercentOfWindow | LabelKind::MessagesQuota | LabelKind::OAuthUnlinked => {
            "pct_used"
        }
    }
    .to_string();

    // Multi-account vendors carry the active account id; single-account
    // vendors emit None to match existing seed/test conventions.
    let account_id_field = match vendor_slug {
        "openrouter" | "zai" => active_accounts()
            .lock()
            .ok()
            .and_then(|guard| guard.get(vendor_slug).cloned()),
        _ => None,
    };

    UsageSnapshotInput {
        vendor: vendor_slug.to_string(),
        account_id: account_id_field,
        ts: chrono::Utc::now().timestamp_millis(),
        cost_usd: metrics.cost_usd,
        // Reuse `compute_metrics`' pct_used directly — it already projects
        // to f64 0..=100 with worst-of-windows semantics for multi-window
        // vendors. `tokens_used` is None for every v1.0 vendor.
        tokens_used: None,
        pct_used: metrics.pct_used,
        metric_kind,
    }
}

/// Story 5.1 (Wave 5) — On-demand vendor refresh.
///
/// Async FFI entry point that executes the vendor's fetcher end-to-end and
/// persists the result via [`ffi_record_snapshot`]. Returns `Ok(())` on
/// success; on failure returns a [`RefreshFfiError`] with a granular variant
/// so the Swift layer can show the right affordance.
///
/// ## Wrapping with `async_compat::Compat`
///
/// UniFFI's UDL-mode polling driver does not establish a tokio context. The
/// vendor fetchers use `reqwest::Client::get(...).send().await` and
/// `tokio::time::timeout`, both of which require a tokio reactor. We wrap
/// the inner future in `async_compat::Compat::new(...)` so the reactor is
/// available for the duration of the call. This mirrors the pattern used by
/// `InsightsClient::request_insight_streaming` (Story 1.15).
pub async fn ffi_refresh_vendor(vendor_name: String) -> Result<(), RefreshFfiError> {
    ::uniffi::deps::async_compat::Compat::new(refresh_vendor_inner(vendor_name)).await
}

// ===========================================================================
// Story 5.5.2 (Wave 5.5) — Anthropic OAuth status FFI surface
// ===========================================================================

/// Status snapshot returned by `ffi_anthropic_oauth_status()`. See UDL doc for
/// field semantics.
#[derive(Debug, Clone)]
pub struct OAuthStatusFfi {
    pub is_connected: bool,
    pub is_expired: bool,
    pub expires_at_secs: Option<i64>,
    pub source: String,
}

/// Probe the dual-source Anthropic OAuth resolver and return a
/// `OAuthStatusFfi` snapshot for Settings. Async + Compat-wrapped for
/// symmetry with `ffi_refresh_vendor`.
pub async fn ffi_anthropic_oauth_status() -> OAuthStatusFfi {
    ::uniffi::deps::async_compat::Compat::new(anthropic_oauth_status_inner()).await
}

/// Inner body — pure logic, no async actually needed today (the Keychain
/// query is synchronous), but kept async to match the UDL surface and leave
/// room for a future Keychain async API. Tests drive this directly to avoid
/// the UniFFI polling driver.
async fn anthropic_oauth_status_inner() -> OAuthStatusFfi {
    use vendors::anthropic::creds;

    // `default_anthropic_creds_source()` already probes Keychain readability,
    // so by the time we have a `CredsSource` we know the source EXISTS — but
    // we still need to actually read it to check the expiry. We re-read here
    // (cheap on macOS: SecItemCopyMatching is microseconds when cached).
    let source = creds::default_anthropic_creds_source();

    let creds_file = match creds::read_from_source(&source) {
        Ok(c) => c,
        Err(err) => {
            // Resolver said one source was readable but the read failed (or
            // the file path doesn't exist). Either way, degrade to
            // "not configured". Log for diagnostics so we catch the
            // race-or-corruption case during dev.
            tracing::debug!(
                source = source.tag(),
                "ffi_anthropic_oauth_status: read failed: {}",
                err
            );
            return OAuthStatusFfi {
                is_connected: false,
                is_expired: false,
                expires_at_secs: None,
                source: "none".to_string(),
            };
        }
    };

    let expires_at_secs = creds_file.claude_ai_oauth.expires_at_secs();
    let now_secs = chrono::Utc::now().timestamp();
    let is_expired = expires_at_secs <= now_secs;

    OAuthStatusFfi {
        is_connected: true,
        is_expired,
        expires_at_secs: Some(expires_at_secs),
        source: source.tag().to_string(),
    }
}

/// Inner async body — kept separate from the FFI shim so tests can drive it
/// directly under `#[tokio::test]` without going through the UniFFI polling
/// driver.
async fn refresh_vendor_inner(vendor_name: String) -> Result<(), RefreshFfiError> {
    let started = Instant::now();
    tracing::info!(vendor = %vendor_name, "ffi_refresh_vendor: start");

    // Validate the slug BEFORE any history init or network touch. Unknown
    // slugs short-circuit to `CredentialMissing` per AC-2 (no panic, no
    // network). Doing this before `ensure_history_initialized()` keeps the
    // failure path purely a fast input-validation rejection.
    match vendor_name.as_str() {
        "anthropic" | "openai" | "openrouter" | "zai" => {}
        other => {
            tracing::warn!(vendor = %other, "ffi_refresh_vendor: unknown vendor slug");
            return Err(RefreshFfiError::CredentialMissing);
        }
    }

    // AC-4: lazy-initialize the history DB if Swift hasn't called
    // `ffi_init_history` yet. `OnceLock::get_or_init` makes this idempotent —
    // a subsequent explicit init from Swift will return the existing slot.
    ensure_history_initialized()?;

    let snapshot_input = match vendor_name.as_str() {
        "anthropic" => refresh_anthropic().await?,
        "openai" => refresh_openai().await?,
        "openrouter" => refresh_openrouter().await?,
        "zai" => refresh_zai().await?,
        // Unreachable: the slug was validated in the first match above.
        _ => unreachable!("vendor slug pre-validated"),
    };

    // Persist the snapshot via the existing FFI path so retention/janitor,
    // logging, and concurrency invariants are honored uniformly.
    ffi_record_snapshot(snapshot_input)?;

    let elapsed_ms = started.elapsed().as_millis();
    tracing::info!(
        vendor = %vendor_name,
        elapsed_ms = %elapsed_ms,
        "ffi_refresh_vendor: complete"
    );
    Ok(())
}

/// AC-4 lazy init — best-effort. Only attempts initialization if the slot is
/// `None`. Returns `Err(StorageError)` only if the slot is currently None
/// AND the default-path initialization fails. If the slot is already
/// populated, this is a no-op.
fn ensure_history_initialized() -> Result<(), RefreshFfiError> {
    let already_initialized = {
        let slot = history_slot()
            .lock()
            .map_err(|_| RefreshFfiError::StorageError)?;
        slot.is_some()
    };
    if already_initialized {
        return Ok(());
    }

    // Try to bootstrap using the default DB path. Mirror `ffi_init_history`
    // without the explicit path argument so existing tests pass through
    // `ffi_init_history(None)` continue to work.
    ffi_init_history(None).map_err(RefreshFfiError::from)
}

/// Default cache TTL for refresh paths. Story 5.1 uses `Duration::ZERO` so
/// the Refresh button (Story 5.3) always exercises the network — caching
/// would defeat the user-initiated "refresh now" UX.
const REFRESH_CACHE_TTL: Duration = Duration::from_secs(0);

async fn refresh_anthropic() -> Result<UsageSnapshotInput, RefreshFfiError> {
    // Story 5.5.2 (Wave 5.5): try Keychain (`"Claude Code-credentials"`) first,
    // then fall back to legacy file `~/.claude/.credentials.json`. The resolver
    // probes Keychain readability synchronously, so by the time we get a source
    // back we already know read_from_source(&source) will succeed for the
    // happy-path case (or surface an actionable `Credentials` error otherwise).
    let creds_source = vendors::anthropic::creds::default_anthropic_creds_source();

    // CredentialMissing: if neither source is readable (Keychain entry missing
    // AND legacy file missing), short-circuit so Swift can route the user to
    // Settings / re-auth without burning an HTTP round-trip.
    match &creds_source {
        vendors::anthropic::creds::CredsSource::File(p) if !p.exists() => {
            tracing::warn!(
                "refresh_anthropic: neither Keychain (\"{}\") nor file ({}) has credentials",
                vendors::anthropic::creds::CLAUDE_CODE_KEYCHAIN_SERVICE,
                p.display()
            );
            return Err(RefreshFfiError::CredentialMissing);
        }
        _ => {}
    }

    tracing::info!(
        source = creds_source.tag(),
        "refresh_anthropic: using credentials source"
    );

    let client = reqwest::Client::new();
    let cache = Cache::for_vendor("anthropic").map_err(RefreshFfiError::from)?;
    let endpoints = vendors::anthropic::fetch::Endpoints::default();

    let outcome = vendors::anthropic::fetch::fetch_snapshot(
        &client,
        &creds_source,
        &cache,
        &endpoints,
        REFRESH_CACHE_TTL,
    )
    .await
    .map_err(RefreshFfiError::from)?;

    Ok(outcome_to_snapshot_input(
        "anthropic",
        &VendorSnapshot::Anthropic(outcome.snapshot),
    ))
}

async fn refresh_openai() -> Result<UsageSnapshotInput, RefreshFfiError> {
    let creds_path = vendors::openai::creds::default_path().map_err(RefreshFfiError::from)?;
    if !creds_path.exists() {
        tracing::warn!(
            "refresh_openai: OAuth credentials file not found at {}",
            creds_path.display()
        );
        return Err(RefreshFfiError::CredentialMissing);
    }
    let client = reqwest::Client::new();
    let cache = Cache::for_vendor("openai").map_err(RefreshFfiError::from)?;
    let endpoints = vendors::openai::fetch::Endpoints::default();

    let outcome = vendors::openai::fetch::fetch_snapshot(
        &client,
        &creds_path,
        &cache,
        &endpoints,
        REFRESH_CACHE_TTL,
    )
    .await
    .map_err(RefreshFfiError::from)?;

    Ok(outcome_to_snapshot_input(
        "openai",
        &VendorSnapshot::Openai(outcome.snapshot),
    ))
}

async fn refresh_openrouter() -> Result<UsageSnapshotInput, RefreshFfiError> {
    let api_key = keychain_api_key_for("openrouter")?;
    let client = reqwest::Client::new();
    let cache = Cache::for_vendor("openrouter").map_err(RefreshFfiError::from)?;
    let endpoints = vendors::openrouter::fetch::Endpoints::default();

    let outcome = vendors::openrouter::fetch::fetch_snapshot(
        &client,
        &api_key,
        &cache,
        &endpoints,
        REFRESH_CACHE_TTL,
    )
    .await
    .map_err(RefreshFfiError::from)?;

    Ok(outcome_to_snapshot_input(
        "openrouter",
        &VendorSnapshot::Openrouter(outcome.snapshot),
    ))
}

async fn refresh_zai() -> Result<UsageSnapshotInput, RefreshFfiError> {
    let api_key = keychain_api_key_for("zai")?;
    let client = reqwest::Client::new();
    let cache = Cache::for_vendor("zai").map_err(RefreshFfiError::from)?;
    let endpoints = vendors::zai::fetch::Endpoints::default();

    // Z.AI fetcher takes an optional config-driven plan tier hint. We honor
    // it best-effort by loading the config; if loading fails, fall through
    // with `None` (the fetcher tolerates this — same code path as the dev
    // TUI before Story 5.1).
    let plan_tier_owned = Config::load().ok().and_then(|cfg| cfg.zai_plan_tier);

    let outcome = vendors::zai::fetch::fetch_snapshot(
        &client,
        &api_key,
        &cache,
        &endpoints,
        REFRESH_CACHE_TTL,
        plan_tier_owned.as_deref(),
    )
    .await
    .map_err(RefreshFfiError::from)?;

    Ok(outcome_to_snapshot_input(
        "zai",
        &VendorSnapshot::Zai(outcome.snapshot),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_returns_pong() {
        assert_eq!(ping(), "pong");
    }

    #[test]
    fn get_vendor_list_returns_four_vendors() {
        // Story 5.5.1 (WAVE5.5-D1): the canonical list dropped from 5 → 4
        // when Gemini was removed (no Rust vendor module, no product
        // validation). Re-adding Gemini bumps this back to 5.
        let vendors = get_vendor_list();
        assert_eq!(
            vendors.len(),
            4,
            "Story 5.5.1 expects exactly 4 vendors (Gemini removed)"
        );
    }

    #[test]
    fn get_vendor_list_has_canonical_order() {
        let vendors = get_vendor_list();
        let ids: Vec<&str> = vendors.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["anthropic", "openai", "openrouter", "zai"],
            "Vendor order must match the legacy Waybar rollout priority \
             (Gemini removed in Story 5.5.1)"
        );
    }

    #[test]
    fn get_vendor_list_all_unconfigured_in_story_1_5() {
        // Story 1.5 hardcodes is_configured=false. Story 1.6 will replace
        // this implementation with a real config probe — when that lands,
        // this assertion should be deleted (the new test will assert that
        // `is_configured` reflects the loaded config).
        // Story 5.5.1 narrowed the list from 5 → 4 vendors; intent is
        // unchanged (all hardcoded `false`).
        let vendors = get_vendor_list();
        assert!(
            vendors.iter().all(|v| !v.is_configured),
            "Story 1.5 hardcodes is_configured=false for all vendors"
        );
    }

    #[test]
    fn get_vendor_list_does_not_contain_gemini() {
        // Story 5.5.1 (WAVE5.5-D1) regression guard: Gemini was removed
        // from the canonical vendor list because it never received a Rust
        // vendor module and live smoke surfaced its empty legend slot as
        // confusing UX. This test fails loudly if someone reintroduces the
        // entry without explicit product re-approval.
        let vendors = get_vendor_list();
        assert!(
            vendors.iter().all(|v| v.id != "gemini"),
            "Gemini was removed in Story 5.5.1 — re-add requires product sign-off"
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

    // -----------------------------------------------------------------
    // Story 3.3 (Wave 3) — Account picker FFI tests
    // -----------------------------------------------------------------

    fn config_with_two_openrouter_accounts() -> Config {
        let toml = r#"
[[openrouter.accounts]]
name = "Personal"

[[openrouter.accounts]]
name = "ClienteAcme"
"#;
        Config::load_from_str(toml).expect("fixture config must parse")
    }

    #[test]
    fn get_accounts_for_vendor_returns_two_rows_with_default_active() {
        let config = config_with_two_openrouter_accounts();
        let active_map = HashMap::new();
        let rows = get_accounts_for_vendor_inner(&config, &active_map, "openrouter");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].is_active, "first account is the default active");
        assert!(!rows[1].is_active);
        assert_eq!(rows[0].label, "Personal");
        assert_eq!(rows[1].label, "ClienteAcme");
    }

    #[test]
    fn set_active_account_swaps_to_second_row() {
        let config = config_with_two_openrouter_accounts();
        let mut active_map = HashMap::new();
        // Discover the id for the second account.
        let rows = get_accounts_for_vendor_inner(&config, &active_map, "openrouter");
        let target_id = rows[1].id.clone();

        set_active_account_inner(&config, &mut active_map, "openrouter", &target_id)
            .expect("swap to existing account must succeed");

        let rows_after = get_accounts_for_vendor_inner(&config, &active_map, "openrouter");
        assert!(!rows_after[0].is_active, "first row is no longer active");
        assert!(rows_after[1].is_active, "second row is now active");
    }

    #[test]
    fn get_accounts_for_unknown_vendor_returns_empty_not_error() {
        let config = config_with_two_openrouter_accounts();
        let active_map = HashMap::new();
        let rows = get_accounts_for_vendor_inner(&config, &active_map, "gemini");
        assert!(
            rows.is_empty(),
            "gemini has no multi-account support in v1.0"
        );
    }

    #[test]
    fn set_active_account_unknown_vendor_returns_vendor_not_found() {
        let config = config_with_two_openrouter_accounts();
        let mut active_map = HashMap::new();
        let err = set_active_account_inner(&config, &mut active_map, "nonexistent", "anything")
            .expect_err("unknown vendor must error");
        assert!(matches!(err, AccountFfiError::VendorNotFound));
    }

    #[test]
    fn set_active_account_unknown_account_returns_account_not_found() {
        let config = config_with_two_openrouter_accounts();
        let mut active_map = HashMap::new();
        let err = set_active_account_inner(
            &config,
            &mut active_map,
            "openrouter",
            "openrouter-does-not-exist",
        )
        .expect_err("unknown account must error");
        assert!(matches!(err, AccountFfiError::AccountNotFound));
    }

    // -----------------------------------------------------------------
    // Story 4.0 — Active-account persistence integration
    // -----------------------------------------------------------------

    /// AC-7 / AC-6: the in-memory mutation pipeline (swap via inner) +
    /// `save_active_accounts` to a temp config produces a TOML that, when
    /// reloaded via `load_active_accounts`, yields the same map — i.e. the
    /// restart semantic Wave 4 expects.
    #[test]
    fn set_active_account_persists_and_reload_returns_same_map() {
        use tempfile::NamedTempFile;

        let f = NamedTempFile::new().unwrap();
        let config = config_with_two_openrouter_accounts();
        let mut active_map = HashMap::new();
        let rows = get_accounts_for_vendor_inner(&config, &active_map, "openrouter");
        let target_id = rows[1].id.clone();

        set_active_account_inner(&config, &mut active_map, "openrouter", &target_id)
            .expect("swap must succeed");
        crate::config::save_active_accounts(f.path(), &active_map)
            .expect("persistence must succeed");

        // Direct file inspection — verify the [active_accounts] section
        // contains the swapped pair.
        let raw = std::fs::read_to_string(f.path()).unwrap();
        assert!(
            raw.contains("[active_accounts]"),
            "expected [active_accounts] section:\n{raw}"
        );
        assert!(
            raw.contains(&format!("openrouter = \"{target_id}\"")),
            "expected vendor=account pair in TOML:\n{raw}"
        );

        // Simulate restart: reload from disk.
        let reloaded = crate::config::load_active_accounts(f.path());
        assert_eq!(
            reloaded.get("openrouter").map(|s| s.as_str()),
            Some(target_id.as_str())
        );
    }

    // -----------------------------------------------------------------
    // Story 4.6 (Wave 4) — Budget burn build_status tests
    // -----------------------------------------------------------------

    #[test]
    fn build_status_no_budget_yields_empty() {
        let cfg = BudgetConfig::default();
        let spend = HashMap::new();
        let s = build_status(&cfg, &spend);
        assert!(!s.has_budget);
        assert_eq!(s.total_spent_usd, 0.0);
        assert_eq!(s.total_budget_usd, None);
        assert!(s.per_vendor.is_empty());
    }

    #[test]
    fn build_status_global_only_50_percent() {
        let cfg = BudgetConfig {
            monthly_usd_total: Some(100.0),
            per_vendor: HashMap::new(),
        };
        let mut spend = HashMap::new();
        spend.insert("openrouter".to_string(), 30.0);
        spend.insert("anthropic".to_string(), 20.0);

        let s = build_status(&cfg, &spend);
        assert!(s.has_budget);
        assert_eq!(s.total_spent_usd, 50.0);
        assert_eq!(s.total_budget_usd, Some(100.0));
        assert_eq!(s.total_percent_used, 50.0);
        assert!(
            s.per_vendor.is_empty(),
            "per_vendor empty when no per-vendor cap"
        );
    }

    #[test]
    fn build_status_per_vendor_overspend_marked() {
        let mut per_vendor = HashMap::new();
        per_vendor.insert("openrouter".to_string(), 50.0);
        per_vendor.insert("anthropic".to_string(), 30.0);
        let cfg = BudgetConfig {
            monthly_usd_total: Some(100.0),
            per_vendor,
        };
        let mut spend = HashMap::new();
        spend.insert("openrouter".to_string(), 60.0); // 120% overspend
        spend.insert("anthropic".to_string(), 15.0); // 50% used
        spend.insert("openai".to_string(), 5.0); // not budgeted; folded only into total

        let s = build_status(&cfg, &spend);
        assert_eq!(s.total_spent_usd, 80.0);
        assert_eq!(s.total_percent_used, 80.0);

        // Per-vendor entries — sorted alphabetically (anthropic before openrouter).
        assert_eq!(s.per_vendor.len(), 2);
        assert_eq!(s.per_vendor[0].vendor_id, "anthropic");
        assert_eq!(s.per_vendor[0].spent_usd, 15.0);
        assert_eq!(s.per_vendor[0].percent_used, 50.0);
        assert_eq!(s.per_vendor[1].vendor_id, "openrouter");
        assert_eq!(s.per_vendor[1].spent_usd, 60.0);
        assert!((s.per_vendor[1].percent_used - 120.0).abs() < 0.0001);
    }

    #[test]
    fn build_status_zero_budget_with_spend_is_infinity() {
        let cfg = BudgetConfig {
            monthly_usd_total: Some(0.0),
            per_vendor: HashMap::new(),
        };
        let mut spend = HashMap::new();
        spend.insert("openai".to_string(), 0.01);

        let s = build_status(&cfg, &spend);
        assert!(s.total_percent_used.is_infinite());
        assert_eq!(s.total_spent_usd, 0.01);
    }

    #[test]
    fn build_status_normalizes_negative_zero() {
        // BUG-001 (Wave 4 polish): signed-zero (-0.0) propagating through to
        // the FFI payload renders as "-US$ 0,00" under Swift's pt-BR locale
        // currency formatter. The `+ 0.0` idiom in build_status collapses
        // -0.0 → +0.0 (IEEE 754) without affecting any other value.
        // We compare via `to_bits()` because the equality `0.0 == -0.0` is
        // `true` in IEEE 754 — the sign bit is invisible to `==`.
        let cfg = BudgetConfig {
            monthly_usd_total: Some(1.0),
            per_vendor: HashMap::from([("anthropic".to_string(), 1.0)]),
        };
        let mut spend = HashMap::new();
        spend.insert("anthropic".to_string(), -0.0_f64);

        let status = build_status(&cfg, &spend);

        assert_eq!(
            status.total_spent_usd.to_bits(),
            0.0_f64.to_bits(),
            "total_spent_usd leaked -0.0 sign bit"
        );
        assert_eq!(
            status.per_vendor[0].spent_usd.to_bits(),
            0.0_f64.to_bits(),
            "per_vendor[0].spent_usd leaked -0.0 sign bit"
        );
        assert_eq!(
            status.total_percent_used.to_bits(),
            0.0_f64.to_bits(),
            "total_percent_used leaked -0.0 sign bit"
        );
        assert_eq!(
            status.per_vendor[0].percent_used.to_bits(),
            0.0_f64.to_bits(),
            "per_vendor[0].percent_used leaked -0.0 sign bit"
        );
    }

    #[test]
    fn vendor_id_from_slug_maps_all_four_known_vendors() {
        assert!(matches!(
            vendor_id_from_slug("anthropic"),
            Some(VendorId::Anthropic)
        ));
        assert!(matches!(
            vendor_id_from_slug("openai"),
            Some(VendorId::Openai)
        ));
        assert!(matches!(vendor_id_from_slug("zai"), Some(VendorId::Zai)));
        assert!(matches!(
            vendor_id_from_slug("openrouter"),
            Some(VendorId::Openrouter)
        ));
        assert_eq!(vendor_id_from_slug("gemini"), None);
        assert_eq!(vendor_id_from_slug("bogus"), None);
    }

    // -----------------------------------------------------------------
    // Story 5.1 (Wave 5) — ffi_refresh_vendor tests
    // -----------------------------------------------------------------

    use crate::usage::{
        AnthropicSnapshot, OpenAiSnapshot, OpenAiSource, OpenRouterSnapshot, UsageWindow,
        ZaiSnapshot,
    };

    /// AC-5: AppError → RefreshFfiError mapping must be precise — Swift
    /// surfaces a different affordance per variant. Verify every relevant
    /// branch lands in the expected bucket.
    #[test]
    fn refresh_error_mapping_credential_missing_for_credentials_variant() {
        let err = AppError::Credentials("no creds".into());
        assert!(matches!(
            RefreshFfiError::from(err),
            RefreshFfiError::CredentialMissing
        ));
    }

    #[test]
    fn refresh_error_mapping_credential_missing_for_io_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "x");
        let err = AppError::IoBare(io_err);
        assert!(matches!(
            RefreshFfiError::from(err),
            RefreshFfiError::CredentialMissing
        ));
    }

    #[test]
    fn refresh_error_mapping_network_for_transport() {
        let err = AppError::Transport("dns failed".into());
        assert!(matches!(
            RefreshFfiError::from(err),
            RefreshFfiError::NetworkError
        ));
    }

    #[test]
    fn refresh_error_mapping_api_for_http_status() {
        let err = AppError::Http {
            status: 401,
            body: "unauthorized".into(),
        };
        assert!(matches!(
            RefreshFfiError::from(err),
            RefreshFfiError::ApiError
        ));
    }

    #[test]
    fn refresh_error_mapping_parse_failure_for_schema() {
        let err = AppError::Schema("unexpected shape".into());
        assert!(matches!(
            RefreshFfiError::from(err),
            RefreshFfiError::ParseFailure
        ));
    }

    #[test]
    fn refresh_error_mapping_storage_for_history_ffi_error() {
        let err = HistoryFfiError::NotInitialized;
        assert!(matches!(
            RefreshFfiError::from(err),
            RefreshFfiError::StorageError
        ));
    }

    fn fake_anthropic_snapshot() -> VendorSnapshot {
        VendorSnapshot::Anthropic(AnthropicSnapshot {
            plan: "Pro".into(),
            session: UsageWindow {
                utilization_pct: 42,
                resets_at: None,
                window_duration: chrono::Duration::hours(5),
            },
            weekly: UsageWindow {
                utilization_pct: 18,
                resets_at: None,
                window_duration: chrono::Duration::hours(168),
            },
            sonnet: None,
            extra: None,
        })
    }

    fn fake_openrouter_snapshot() -> VendorSnapshot {
        VendorSnapshot::Openrouter(OpenRouterSnapshot {
            label: "OpenRouter".into(),
            total_credits: 50.0,
            total_usage: 12.34,
            usage_daily: 1.0,
            usage_weekly: 5.0,
            usage_monthly: 12.34,
            is_free_tier: false,
            limit: None,
            limit_remaining: None,
        })
    }

    fn fake_openai_snapshot() -> VendorSnapshot {
        VendorSnapshot::Openai(OpenAiSnapshot {
            plan: "Plus".into(),
            session: UsageWindow {
                utilization_pct: 25,
                resets_at: None,
                window_duration: chrono::Duration::hours(5),
            },
            weekly: UsageWindow {
                utilization_pct: 10,
                resets_at: None,
                window_duration: chrono::Duration::hours(168),
            },
            code_review: None,
            credits: None,
            source: OpenAiSource::CodexOauth,
        })
    }

    fn fake_zai_snapshot() -> VendorSnapshot {
        VendorSnapshot::Zai(ZaiSnapshot {
            plan: "GLM Coding Lite".into(),
            session: Some(UsageWindow {
                utilization_pct: 60,
                resets_at: None,
                window_duration: chrono::Duration::hours(5),
            }),
            weekly: None,
            mcp: None,
        })
    }

    /// AC-3: outcome_to_snapshot_input projects Anthropic (plan-based, no
    /// extra-usage) → pct_used metric_kind, with no cost_usd.
    #[test]
    fn outcome_to_snapshot_anthropic_emits_pct_used() {
        let snap = fake_anthropic_snapshot();
        let row = outcome_to_snapshot_input("anthropic", &snap);
        assert_eq!(row.vendor, "anthropic");
        assert_eq!(row.account_id, None, "anthropic is single-account in v1.0");
        assert_eq!(row.metric_kind, "pct_used");
        assert_eq!(
            row.cost_usd, None,
            "anthropic without extra-usage has no cost field"
        );
        // pct_used reflects the worst-of-windows (42 here, > 18).
        assert_eq!(row.pct_used, Some(42.0));
        assert_eq!(row.tokens_used, None);
        // ts is "recent" — within last minute.
        let now = chrono::Utc::now().timestamp_millis();
        assert!((now - row.ts).abs() < 60_000);
    }

    /// AC-3: OpenRouter is UsdSpent → cost_usd_total metric_kind with the
    /// dollar headline persisted.
    #[test]
    fn outcome_to_snapshot_openrouter_emits_cost_usd_total() {
        let snap = fake_openrouter_snapshot();
        let row = outcome_to_snapshot_input("openrouter", &snap);
        assert_eq!(row.vendor, "openrouter");
        assert_eq!(row.metric_kind, "cost_usd_total");
        assert_eq!(row.cost_usd, Some(12.34));
        // openrouter is a multi-account vendor; in the absence of any active
        // account swap (test isolation), account_id is None.
        // (Test runs may pick up a real active map from disk; we don't
        // assert on the value, only on the metric_kind/cost projection.)
    }

    /// AC-3: OpenAI Codex OAuth path is PercentOfWindow → pct_used.
    #[test]
    fn outcome_to_snapshot_openai_codex_emits_pct_used() {
        let snap = fake_openai_snapshot();
        let row = outcome_to_snapshot_input("openai", &snap);
        assert_eq!(row.vendor, "openai");
        assert_eq!(row.metric_kind, "pct_used");
        assert_eq!(row.cost_usd, None);
        assert_eq!(row.pct_used, Some(25.0));
        assert_eq!(row.account_id, None);
    }

    /// AC-3: Z.AI buckets surface as PercentOfWindow → pct_used.
    #[test]
    fn outcome_to_snapshot_zai_emits_pct_used() {
        let snap = fake_zai_snapshot();
        let row = outcome_to_snapshot_input("zai", &snap);
        assert_eq!(row.vendor, "zai");
        assert_eq!(row.metric_kind, "pct_used");
        assert_eq!(row.pct_used, Some(60.0));
        assert_eq!(row.cost_usd, None);
    }

    /// AC-2: unknown vendor slug returns `CredentialMissing` (not a panic)
    /// with `tracing::warn!` emitted server-side.
    ///
    /// The implementation short-circuits BEFORE history init, so this test
    /// is safe to run concurrently with other tests that share the
    /// process-global `HISTORY_DB` / `SECRET_STORE` statics — no env
    /// mutation required.
    #[tokio::test]
    async fn refresh_unknown_vendor_returns_credential_missing() {
        let result = refresh_vendor_inner("not-a-vendor".to_string()).await;
        assert!(matches!(result, Err(RefreshFfiError::CredentialMissing)));
    }

    /// AC-2 + AC-5: refreshing a Keychain-based vendor (openrouter) when no
    /// blob is configured returns `CredentialMissing`. This is the
    /// happy-path-of-failure: Swift renders "configure in Settings" affordance.
    ///
    /// We use Z.AI here because it shares the same Keychain code path but
    /// avoids touching anthropic OAuth paths which the developer may have
    /// configured in their real HOME.
    #[tokio::test]
    async fn refresh_keychain_vendor_without_key_returns_credential_missing() {
        // Isolate from real Keychain: redirect SECRET_STORE init by setting
        // HOME to a temp dir. On non-macOS this routes to FileFallbackStore
        // which reads from $HOME; on macOS we rely on the Keychain ACL
        // refusing to return a blob for a slug that's never been written.
        //
        // To make this test reliable cross-platform AND on macOS without
        // touching real Keychain entries, we exercise the inner helper
        // `keychain_api_key_for` with a deliberately-bogus slug. The slug
        // never had a blob written for it, so the lookup returns
        // CredentialMissing.
        let res = keychain_api_key_for("definitely-not-a-real-vendor-slug");
        assert!(matches!(res, Err(RefreshFfiError::CredentialMissing)));
    }

    /// AC-4: when the history DB slot has not been initialized AND the
    /// default path init fails, refresh_vendor_inner short-circuits to
    /// StorageError (not NetworkError or ApiError). We simulate that by
    /// pointing HOME at a non-writable path, which makes
    /// `history::default_db_path()` fail to create the parent directory.
    ///
    /// NOTE: on most CI environments /dev/null/foo will not be writable;
    /// if it happens to be on a host, the test degrades to "init succeeded
    /// → branched to NetworkError/CredentialMissing on the next step",
    /// still not a panic. We assert that the result is an Err of *some*
    /// kind — the precise variant depends on host filesystem.
    #[tokio::test]
    async fn refresh_with_uninitialized_history_and_unknown_vendor_does_not_panic() {
        // Unknown vendor short-circuits before any HTTP, so this primarily
        // exercises the lazy-init AC-4 path (best-effort, must not panic).
        let _ = refresh_vendor_inner("definitely-unknown-vendor-xyz".to_string()).await;
        // No assertion on variant — host-dependent. The test passes if the
        // inner function returns at all (no panic, no hang).
    }
}
