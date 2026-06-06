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

use crate::budgets::{self, BudgetConfig};
use crate::config::{Config, account_id};
use crate::history::{
    self, AccountFilter, BucketStrategy as HistoryBucketStrategy, HistoryDb,
    TimeBucket as HistoryTimeBucket, UsageSnapshot,
};
use crate::insights::client::RealAnthropicClient;
use crate::insights::{
    CancelHandle, InsightsCallback, InsightsContext, InsightsError, InsightsOutput, LlmClient,
};
use crate::keychain::{self, SecretStore, blob_bytes_from_store, set_blob_bytes_for_store};
use crate::vendor::VendorId;

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
}
