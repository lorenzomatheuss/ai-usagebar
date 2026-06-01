//! Config file at `~/.config/torven/config.toml`.
//!
//! ## Story 1.6 — Multi-account schema
//!
//! Each per-vendor section supports `[[vendor.accounts]]` array-of-tables for
//! OpenRouter and Z.AI:
//!
//! ```toml
//! [[openrouter.accounts]]
//! name = "ClienteAcme"
//! api_key = "sk-or-..."   # optional; None means "read from Keychain" post-1.20
//! tag = "client"          # optional; client | personal | team
//! budget_usd = 200.0      # optional; non-negative if set
//! description = "..."     # optional free-form
//!
//! [[openrouter.accounts]]
//! name = "Personal"
//! tag = "personal"
//! ```
//!
//! Legacy single-key configs are migrated automatically — `[openrouter] api_key
//! = "..."` becomes a single `Account { name: "default", api_key: Some(...) }`
//! with a `tracing::warn!` so users can re-run the Settings overlay to
//! curate their accounts.
//!
//! Anthropic and OpenAI do NOT participate in `accounts` — they use OAuth
//! credential files (`~/.claude/.credentials.json`, `~/.codex/auth.json`) and
//! keep their dedicated config sub-structs.
//!
//! API keys still live in the TOML for backwards compat through Story 1.19.
//! Story 1.20 introduces the Keychain blob; consumers will then prefer
//! `account.api_key.is_some()` for inline keys and fall back to a
//! `keychain_get_blob(vendor)` lookup when it's `None`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, value};

use crate::error::{AppError, Result};
use crate::vendor::VendorId;

// =====================================================================
// Account model
// =====================================================================

/// Tag attached to an account for grouping / display. Stored as lowercase
/// strings in TOML (`tag = "client"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountTag {
    Client,
    Personal,
    Team,
}

/// A single named credential for a vendor. Multiple `Account`s per vendor are
/// supported (see [`Config::accounts`]). `api_key = None` means the key is
/// expected from the Keychain blob (Story 1.20+) or an env var.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Account {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<AccountTag>,
}

/// Deterministic account identifier used by SQLite (Story 1.13) and the
/// privacy hash for AI Insights (FR-7). Stable across renames *as long as*
/// `name` doesn't change. Lower-cased to prevent `"Personal"` and `"personal"`
/// colliding.
pub fn account_id(vendor: VendorId, name: &str) -> String {
    format!("{}-{}", vendor.slug(), name.to_lowercase())
}

// =====================================================================
// Validation
// =====================================================================

/// Validation failure modes — collected (not returned eagerly) so a Settings
/// overlay can show every problem at once.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConfigError {
    #[error("[{vendor}.accounts] '{name}': name must not be empty")]
    EmptyAccountName { vendor: String, name: String },

    #[error("[{vendor}.accounts]: duplicate account name '{name}'")]
    DuplicateAccountName { vendor: String, name: String },

    #[error(
        "[{vendor}.accounts] '{name}': api_key contains invalid characters \
         (allowed: A-Z a-z 0-9 _ - .)"
    )]
    InvalidApiKeyChars { vendor: String, name: String },

    #[error("[{vendor}.accounts] '{name}': budget_usd must be >= 0, got {got}")]
    NegativeBudget {
        vendor: String,
        name: String,
        got: f64,
    },

    #[error(
        "[display]: refresh_interval_secs must be >= 10, got {got} \
         (lower values would hammer vendor APIs)"
    )]
    RefreshIntervalTooLow { got: u32 },

    #[error(
        "[display.thresholds]: amber ({amber}) must be < critical ({critical}) \
         and both within [0.0, 1.0]"
    )]
    InvalidThresholds { amber: f64, critical: f64 },

    #[error(
        "[ai_insights]: max_cost_usd must be >= 0, got {got}"
    )]
    NegativeMaxCost { got: f64 },

    #[error("[history]: retention_days must be > 0, got {got}")]
    ZeroRetention { got: u32 },
}

/// Strict allow-list for API key characters: alphanumeric, `_`, `-`, `.`.
/// Empty strings are rejected here too — call sites that want to permit
/// `None` should match on `Option` BEFORE invoking this check.
fn is_valid_api_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// Run every validator and return the full list of errors. Empty `Vec` means
/// the config is structurally valid.
pub fn validate_config(config: &Config) -> Vec<ConfigError> {
    let mut errs = Vec::new();

    // Per-vendor account checks.
    for (vendor, accounts) in &config.accounts {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for acct in accounts {
            let v = vendor.slug().to_string();
            if acct.name.trim().is_empty() {
                errs.push(ConfigError::EmptyAccountName {
                    vendor: v.clone(),
                    name: acct.name.clone(),
                });
            } else if !seen.insert(acct.name.as_str()) {
                errs.push(ConfigError::DuplicateAccountName {
                    vendor: v.clone(),
                    name: acct.name.clone(),
                });
            }
            if let Some(b) = acct.budget_usd {
                if b < 0.0 {
                    errs.push(ConfigError::NegativeBudget {
                        vendor: v.clone(),
                        name: acct.name.clone(),
                        got: b,
                    });
                }
            }
            if let Some(k) = acct.api_key.as_deref() {
                if !is_valid_api_key(k) {
                    errs.push(ConfigError::InvalidApiKeyChars {
                        vendor: v.clone(),
                        name: acct.name.clone(),
                    });
                }
            }
        }
    }

    // Display config.
    if config.display.refresh_interval_secs < 10 {
        errs.push(ConfigError::RefreshIntervalTooLow {
            got: config.display.refresh_interval_secs,
        });
    }
    let t = &config.display.thresholds;
    if !(0.0..=1.0).contains(&t.amber)
        || !(0.0..=1.0).contains(&t.critical)
        || t.amber >= t.critical
    {
        errs.push(ConfigError::InvalidThresholds {
            amber: t.amber,
            critical: t.critical,
        });
    }

    // AI Insights.
    if config.ai_insights.max_cost_usd < 0.0 {
        errs.push(ConfigError::NegativeMaxCost {
            got: config.ai_insights.max_cost_usd,
        });
    }

    // History.
    if config.history.retention_days == 0 {
        errs.push(ConfigError::ZeroRetention {
            got: config.history.retention_days,
        });
    }

    errs
}

// =====================================================================
// Config (top-level)
// =====================================================================

/// Resolved configuration after migration. The `RawConfig` intermediary owns
/// the deserialize logic and the legacy-shape compatibility.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(try_from = "RawConfig")]
pub struct Config {
    /// Per-vendor accounts. Empty `Vec` (or missing key) means "vendor has no
    /// credentials configured" — the fetcher will return a `Credentials` error
    /// when invoked. Only `OpenRouter` and `Zai` populate this map; Anthropic
    /// and OpenAI carry their OAuth config in their dedicated structs.
    pub accounts: HashMap<VendorId, Vec<Account>>,

    /// UI preferences (primary vendor for the menu-bar default, etc.).
    pub ui: UiConfig,

    /// Anthropic OAuth credentials.
    pub anthropic: AnthropicConfig,

    /// OpenAI Codex OAuth credentials.
    pub openai: OpenAiConfig,

    /// Per-vendor env var name fallback — preserved across migration so the
    /// existing fetcher contract (`resolve_api_key(label, env_var, inline)`)
    /// keeps working unchanged.
    pub zai_env: VendorEnvConfig,
    pub openrouter_env: VendorEnvConfig,

    /// Optional Z.AI plan tier label (display-only).
    pub zai_plan_tier: Option<String>,

    /// AI Insights / history / display preferences.
    pub ai_insights: AiInsightsConfig,
    pub history: HistoryConfig,
    pub display: DisplayConfig,
}

impl Default for Config {
    fn default() -> Self {
        let mut accounts = HashMap::new();
        accounts.insert(VendorId::Zai, Vec::new());
        accounts.insert(VendorId::Openrouter, Vec::new());
        Self {
            accounts,
            ui: UiConfig::default(),
            anthropic: AnthropicConfig::default(),
            openai: OpenAiConfig::default(),
            zai_env: VendorEnvConfig {
                enabled: true,
                api_key_env: "ZAI_API_KEY".to_string(),
            },
            openrouter_env: VendorEnvConfig {
                enabled: true,
                api_key_env: "OPENROUTER_API_KEY".to_string(),
            },
            zai_plan_tier: None,
            ai_insights: AiInsightsConfig::default(),
            history: HistoryConfig::default(),
            display: DisplayConfig::default(),
        }
    }
}

/// UI dispatch preferences. Currently just `primary` — which vendor the menu
/// bar app shows by default, and which TUI tab is initially selected.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct UiConfig {
    pub primary: Option<VendorId>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AnthropicConfig {
    pub enabled: bool,
    /// Override the credentials file path (defaults to `~/.claude/.credentials.json`).
    pub credentials_path: Option<PathBuf>,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            credentials_path: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct OpenAiConfig {
    pub enabled: bool,
    /// Override the Codex auth file path (defaults to `~/.codex/auth.json`).
    pub codex_auth_path: Option<PathBuf>,
    /// Optional admin key env var name for the API-key-only fallback path.
    pub admin_key_env: String,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            codex_auth_path: None,
            admin_key_env: "OPENAI_ADMIN_KEY".to_string(),
        }
    }
}

/// Vendor-level (not per-account) env settings: `enabled` toggle and which
/// env var name the resolver should read for the inline-key fallback.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct VendorEnvConfig {
    pub enabled: bool,
    pub api_key_env: String,
}

impl Default for VendorEnvConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key_env: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AiInsightsConfig {
    pub enabled: bool,
    pub prompt_version: String,
    pub max_cost_usd: f64,
    pub rate_limit_per_minute: u32,
}

impl Default for AiInsightsConfig {
    fn default() -> Self {
        Self {
            enabled: false, // off until Story 1.15 ships the worker
            prompt_version: "v1".to_string(),
            max_cost_usd: 0.05,
            rate_limit_per_minute: 20,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct HistoryConfig {
    pub retention_days: u32,
    pub db_path: Option<PathBuf>,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            retention_days: 90,
            db_path: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct DisplayConfig {
    pub refresh_interval_secs: u32,
    pub thresholds: ThresholdConfig,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            refresh_interval_secs: 30,
            thresholds: ThresholdConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ThresholdConfig {
    pub amber: f64,
    pub critical: f64,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            amber: 0.75,
            critical: 0.95,
        }
    }
}

// =====================================================================
// RawConfig — accepts BOTH the legacy and new TOML schemas
// =====================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    ui: UiConfig,
    anthropic: AnthropicConfig,
    openai: OpenAiConfig,
    zai: RawVendorSection,
    openrouter: RawVendorSection,
    ai_insights: AiInsightsConfig,
    history: HistoryConfig,
    display: DisplayConfig,
}

/// Per-vendor section that accepts BOTH the legacy `api_key = "..."` shape AND
/// the new `[[vendor.accounts]]` array. Migration logic in
/// `TryFrom<RawConfig> for Config` resolves them.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawVendorSection {
    enabled: Option<bool>,
    /// New-shape: array-of-tables.
    accounts: Vec<Account>,
    /// Legacy: vendor-level inline key.
    api_key: Option<String>,
    /// Vendor-level env var name (preserved across migration).
    api_key_env: Option<String>,
    /// Legacy Z.AI only (display tier).
    plan_tier: Option<String>,
}

impl TryFrom<RawConfig> for Config {
    type Error = AppError;

    fn try_from(raw: RawConfig) -> std::result::Result<Self, Self::Error> {
        let mut accounts: HashMap<VendorId, Vec<Account>> = HashMap::new();

        // Process Z.AI: migrate legacy api_key if present, else use accounts as-is.
        let zai_accounts =
            migrate_vendor_section("zai", &raw.zai.accounts, raw.zai.api_key.as_deref());
        accounts.insert(VendorId::Zai, zai_accounts);

        // OpenRouter.
        let openrouter_accounts = migrate_vendor_section(
            "openrouter",
            &raw.openrouter.accounts,
            raw.openrouter.api_key.as_deref(),
        );
        accounts.insert(VendorId::Openrouter, openrouter_accounts);

        let zai_env = VendorEnvConfig {
            enabled: raw.zai.enabled.unwrap_or(true),
            api_key_env: raw
                .zai
                .api_key_env
                .unwrap_or_else(|| "ZAI_API_KEY".to_string()),
        };

        let openrouter_env = VendorEnvConfig {
            enabled: raw.openrouter.enabled.unwrap_or(true),
            api_key_env: raw
                .openrouter
                .api_key_env
                .unwrap_or_else(|| "OPENROUTER_API_KEY".to_string()),
        };

        Ok(Config {
            accounts,
            ui: raw.ui,
            anthropic: raw.anthropic,
            openai: raw.openai,
            zai_env,
            openrouter_env,
            zai_plan_tier: raw.zai.plan_tier,
            ai_insights: raw.ai_insights,
            history: raw.history,
            display: raw.display,
        })
    }
}

/// Produce a `Vec<Account>` from the raw section, emitting a `tracing::warn!`
/// if a legacy `api_key` is being migrated.
///
/// Rules:
/// - If `accounts` is non-empty, use it as-is and IGNORE the legacy `api_key`
///   (with a warning if both were specified — accounts wins).
/// - Else if `legacy_key` is `Some`, emit a migration warning and produce
///   `vec![Account { name: "default", api_key: Some(key), ... }]`.
/// - Else return `vec![]` (vendor disabled / unconfigured).
fn migrate_vendor_section(
    vendor_label: &str,
    accounts: &[Account],
    legacy_key: Option<&str>,
) -> Vec<Account> {
    if !accounts.is_empty() {
        if legacy_key.is_some() {
            tracing::warn!(
                vendor = vendor_label,
                "Both [[{vendor_label}.accounts]] and legacy [{vendor_label}] api_key are set. \
                 The accounts array takes precedence; the top-level api_key is ignored.",
                vendor_label = vendor_label,
            );
        }
        return accounts.to_vec();
    }
    match legacy_key {
        Some(key) if !key.is_empty() => {
            tracing::warn!(
                vendor = vendor_label,
                "Config migrated from legacy single-key format. \
                 Please review the [[{vendor_label}.accounts]] section in your config.toml.",
                vendor_label = vendor_label,
            );
            vec![Account {
                name: "default".to_string(),
                api_key: Some(key.to_string()),
                description: None,
                budget_usd: None,
                tag: None,
            }]
        }
        _ => Vec::new(),
    }
}

// =====================================================================
// Config loading
// =====================================================================

impl Config {
    /// Load from `~/.config/torven/config.toml`. Returns defaults if the
    /// file doesn't exist; errors only on actual parse failures.
    pub fn load() -> Result<Self> {
        let Some(path) = default_path() else {
            return Ok(Self::default());
        };
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::load_from_str(&s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(AppError::io_at(path, e)),
        }
    }

    /// Parse a TOML string. Migration warnings are emitted via `tracing`.
    pub fn load_from_str(s: &str) -> Result<Self> {
        Ok(toml::from_str(s)?)
    }

    /// Parse a TOML string AND collect migration warnings in the returned
    /// `Vec` (in addition to `tracing::warn!`). Test-oriented surface — most
    /// callers want `load_from_str`.
    pub fn load_from_str_with_warnings(s: &str) -> Result<(Self, Vec<String>)> {
        let raw: RawConfig = toml::from_str(s)?;
        let mut warnings = Vec::new();

        if !raw.zai.accounts.is_empty() && raw.zai.api_key.is_some() {
            warnings.push(
                "zai: both [[zai.accounts]] and legacy api_key are set — accounts wins"
                    .to_string(),
            );
        } else if raw.zai.accounts.is_empty()
            && raw.zai.api_key.as_deref().is_some_and(|k| !k.is_empty())
        {
            warnings.push("zai: migrated legacy api_key to default account".to_string());
        }
        if !raw.openrouter.accounts.is_empty() && raw.openrouter.api_key.is_some() {
            warnings.push(
                "openrouter: both [[openrouter.accounts]] and legacy api_key are set — accounts wins"
                    .to_string(),
            );
        } else if raw.openrouter.accounts.is_empty()
            && raw
                .openrouter
                .api_key
                .as_deref()
                .is_some_and(|k| !k.is_empty())
        {
            warnings
                .push("openrouter: migrated legacy api_key to default account".to_string());
        }

        let config = Config::try_from(raw)?;
        Ok((config, warnings))
    }

    pub fn is_enabled(&self, id: VendorId) -> bool {
        match id {
            VendorId::Anthropic => self.anthropic.enabled,
            VendorId::Openai => self.openai.enabled,
            VendorId::Zai => self.zai_env.enabled,
            VendorId::Openrouter => self.openrouter_env.enabled,
        }
    }

    pub fn enabled_vendors(&self) -> Vec<VendorId> {
        VendorId::all()
            .iter()
            .copied()
            .filter(|id| self.is_enabled(*id))
            .collect()
    }

    /// Return the first account configured for `vendor` (the "primary" account).
    /// Used by the existing single-key call sites until the UI catches up
    /// with multi-account selection (Stories 1.20+).
    pub fn primary_account(&self, vendor: VendorId) -> Option<&Account> {
        self.accounts.get(&vendor).and_then(|v| v.first())
    }

    /// Convenience for fetchers: get the first inline `api_key` for `vendor`.
    /// Returns `None` if the vendor has no accounts or the first account has
    /// no inline key (caller falls back to env var / Keychain).
    pub fn primary_api_key(&self, vendor: VendorId) -> Option<&str> {
        self.primary_account(vendor)
            .and_then(|a| a.api_key.as_deref())
    }
}

/// Resolve an API key for a vendor: env var wins, then inline config, then
/// a clear error naming both fields. Used by Z.AI and OpenRouter vendors.
pub fn resolve_api_key(
    vendor_label: &str,
    env_var_name: &str,
    inline: Option<&str>,
) -> crate::error::Result<String> {
    if !env_var_name.is_empty() {
        if let Ok(v) = std::env::var(env_var_name) {
            if !v.is_empty() {
                return Ok(v);
            }
        }
    }
    if let Some(v) = inline {
        if !v.is_empty() {
            return Ok(v.to_string());
        }
    }
    Err(crate::error::AppError::Credentials(format!(
        "{vendor_label}: no API key. Either export {env_var_name} or set an account \
         under [[{}.accounts]] in ~/.config/torven/config.toml (chmod 600).",
        vendor_label.to_lowercase()
    )))
}

fn default_path() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("", "", "torven")?;
    Some(proj.config_dir().join("config.toml"))
}

// =====================================================================
// Save with comment preservation (toml_edit round-trip)
// =====================================================================

/// Save a `Config` back to `path`, preserving comments and whitespace in any
/// existing file. Creates the file (and parent dirs) if missing.
///
/// Strategy: load the existing file as a `DocumentMut`, set the keys/sections
/// that the new config carries, leave everything else untouched. For the
/// `[[vendor.accounts]]` array we replace the whole sub-table (preserving
/// section-level comments above `[vendor]`, but not comments INSIDE the
/// account entries — those get rewritten).
pub fn save_config(config: &Config, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::io_at(parent, e))?;
        }
    }
    let original = std::fs::read_to_string(path).unwrap_or_default();
    let mut doc: DocumentMut = if original.trim().is_empty() {
        DocumentMut::new()
    } else {
        original
            .parse()
            .map_err(|e: toml_edit::TomlError| AppError::Other(format!("config.toml: {e}")))?
    };

    // [ui].primary
    if let Some(primary) = config.ui.primary {
        set_string(&mut doc, "ui", "primary", primary.slug())?;
    }

    // [display]
    set_int(
        &mut doc,
        "display",
        "refresh_interval_secs",
        config.display.refresh_interval_secs as i64,
    )?;
    set_float(
        &mut doc,
        "display.thresholds",
        "amber",
        config.display.thresholds.amber,
    )?;
    set_float(
        &mut doc,
        "display.thresholds",
        "critical",
        config.display.thresholds.critical,
    )?;

    // [history]
    set_int(
        &mut doc,
        "history",
        "retention_days",
        config.history.retention_days as i64,
    )?;

    // [ai_insights]
    set_bool(
        &mut doc,
        "ai_insights",
        "enabled",
        config.ai_insights.enabled,
    )?;
    set_string(
        &mut doc,
        "ai_insights",
        "prompt_version",
        &config.ai_insights.prompt_version,
    )?;
    set_float(
        &mut doc,
        "ai_insights",
        "max_cost_usd",
        config.ai_insights.max_cost_usd,
    )?;
    set_int(
        &mut doc,
        "ai_insights",
        "rate_limit_per_minute",
        config.ai_insights.rate_limit_per_minute as i64,
    )?;

    // Per-vendor env config & accounts.
    write_vendor_section(&mut doc, "zai", &config.zai_env, &config.zai_plan_tier)?;
    write_vendor_section(&mut doc, "openrouter", &config.openrouter_env, &None)?;

    write_accounts(&mut doc, "zai", &config.accounts.get(&VendorId::Zai))?;
    write_accounts(
        &mut doc,
        "openrouter",
        &config.accounts.get(&VendorId::Openrouter),
    )?;

    let bytes = doc.to_string();
    crate::cache::atomic_write(path, bytes.as_bytes())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
    Ok(())
}

fn write_vendor_section(
    doc: &mut DocumentMut,
    section: &str,
    env: &VendorEnvConfig,
    plan_tier: &Option<String>,
) -> Result<()> {
    set_bool(doc, section, "enabled", env.enabled)?;
    if !env.api_key_env.is_empty() {
        set_string(doc, section, "api_key_env", &env.api_key_env)?;
    }
    if let Some(tier) = plan_tier {
        set_string(doc, section, "plan_tier", tier)?;
    }
    Ok(())
}

fn write_accounts(
    doc: &mut DocumentMut,
    section: &str,
    accounts: &Option<&Vec<Account>>,
) -> Result<()> {
    // Drop any existing accounts array (we rewrite it from scratch — preserves
    // top-level section comments and unrelated fields, but NOT in-account
    // comments). Also drop the legacy single-key `api_key` field if present,
    // so that re-loading the file doesn't re-fire the migration warning.
    if let Some(table) = doc.get_mut(section).and_then(|i| i.as_table_mut()) {
        table.remove("accounts");
        table.remove("api_key");
    }
    let Some(accounts) = accounts else {
        return Ok(());
    };
    if accounts.is_empty() {
        return Ok(());
    }
    let mut arr = toml_edit::ArrayOfTables::new();
    for acct in accounts.iter() {
        let mut t = toml_edit::Table::new();
        t["name"] = value(acct.name.as_str());
        if let Some(k) = &acct.api_key {
            t["api_key"] = value(k.as_str());
        }
        if let Some(d) = &acct.description {
            t["description"] = value(d.as_str());
        }
        if let Some(b) = acct.budget_usd {
            t["budget_usd"] = value(b);
        }
        if let Some(tag) = acct.tag {
            t["tag"] = value(tag_to_str(tag));
        }
        arr.push(t);
    }

    // Ensure the parent section exists, then insert the array-of-tables.
    let table = doc
        .entry(section)
        .or_insert_with(toml_edit::table)
        .as_table_mut()
        .ok_or_else(|| AppError::Other(format!("config.toml: [{section}] is not a table")))?;
    table.insert("accounts", toml_edit::Item::ArrayOfTables(arr));
    Ok(())
}

fn tag_to_str(tag: AccountTag) -> &'static str {
    match tag {
        AccountTag::Client => "client",
        AccountTag::Personal => "personal",
        AccountTag::Team => "team",
    }
}

fn set_string(doc: &mut DocumentMut, section: &str, key: &str, new_value: &str) -> Result<()> {
    let table = navigate_table(doc, section)?;
    if let Some(item) = table.get_mut(key) {
        if let Some(v) = item.as_value_mut() {
            *v = toml_edit::Value::from(new_value);
            v.decor_mut().set_prefix(" ");
            return Ok(());
        }
    }
    table.insert(key, value(new_value));
    Ok(())
}

fn set_int(doc: &mut DocumentMut, section: &str, key: &str, new_value: i64) -> Result<()> {
    let table = navigate_table(doc, section)?;
    if let Some(item) = table.get_mut(key) {
        if let Some(v) = item.as_value_mut() {
            *v = toml_edit::Value::from(new_value);
            v.decor_mut().set_prefix(" ");
            return Ok(());
        }
    }
    table.insert(key, value(new_value));
    Ok(())
}

fn set_float(doc: &mut DocumentMut, section: &str, key: &str, new_value: f64) -> Result<()> {
    let table = navigate_table(doc, section)?;
    if let Some(item) = table.get_mut(key) {
        if let Some(v) = item.as_value_mut() {
            *v = toml_edit::Value::from(new_value);
            v.decor_mut().set_prefix(" ");
            return Ok(());
        }
    }
    table.insert(key, value(new_value));
    Ok(())
}

fn set_bool(doc: &mut DocumentMut, section: &str, key: &str, new_value: bool) -> Result<()> {
    let table = navigate_table(doc, section)?;
    if let Some(item) = table.get_mut(key) {
        if let Some(v) = item.as_value_mut() {
            *v = toml_edit::Value::from(new_value);
            v.decor_mut().set_prefix(" ");
            return Ok(());
        }
    }
    table.insert(key, value(new_value));
    Ok(())
}

/// Resolve a dotted section path (e.g. `display.thresholds`) into a mutable
/// table reference, creating intermediate tables as needed.
fn navigate_table<'a>(
    doc: &'a mut DocumentMut,
    section: &str,
) -> Result<&'a mut toml_edit::Table> {
    let parts: Vec<&str> = section.split('.').collect();
    let mut current: &mut toml_edit::Table = doc.as_table_mut();
    for part in parts {
        let entry = current.entry(part).or_insert_with(toml_edit::table);
        current = entry
            .as_table_mut()
            .ok_or_else(|| AppError::Other(format!("config.toml: [{section}] is not a table")))?;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_toml(s: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(s.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn defaults_enable_all_vendors() {
        let c = Config::default();
        assert!(c.is_enabled(VendorId::Anthropic));
        assert!(c.is_enabled(VendorId::Openai));
        assert!(c.is_enabled(VendorId::Zai));
        assert!(c.is_enabled(VendorId::Openrouter));
        assert_eq!(c.enabled_vendors().len(), 4);
    }

    #[test]
    fn missing_file_uses_defaults() {
        let path = std::path::Path::new("/tmp/does-not-exist-torven-test-1.6");
        let c = Config::load_from(path).unwrap();
        assert!(c.is_enabled(VendorId::Anthropic));
        assert_eq!(c.accounts.get(&VendorId::Openrouter).unwrap().len(), 0);
    }

    #[test]
    fn parses_full_config() {
        let f = write_toml(
            r#"
            [anthropic]
            enabled = true

            [openai]
            enabled = false
            admin_key_env = "MY_ADMIN_KEY"

            [zai]
            enabled = true
            api_key_env = "MY_ZAI"
            plan_tier = "pro"

            [openrouter]
            enabled = false
            "#,
        );
        let c = Config::load_from(f.path()).unwrap();
        assert!(c.is_enabled(VendorId::Anthropic));
        assert!(!c.is_enabled(VendorId::Openai));
        assert!(c.is_enabled(VendorId::Zai));
        assert!(!c.is_enabled(VendorId::Openrouter));
        assert_eq!(c.openai.admin_key_env, "MY_ADMIN_KEY");
        assert_eq!(c.zai_env.api_key_env, "MY_ZAI");
        assert_eq!(c.zai_plan_tier.as_deref(), Some("pro"));
    }

    #[test]
    fn partial_config_falls_back_to_defaults() {
        let f = write_toml(
            r#"[openai]
enabled = false
"#,
        );
        let c = Config::load_from(f.path()).unwrap();
        assert!(!c.is_enabled(VendorId::Openai));
        assert!(c.is_enabled(VendorId::Anthropic));
        assert_eq!(c.openai.admin_key_env, "OPENAI_ADMIN_KEY");
    }

    #[test]
    fn malformed_toml_returns_error() {
        let f = write_toml("this is not = = valid");
        assert!(Config::load_from(f.path()).is_err());
    }

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        static M: std::sync::Mutex<()> = std::sync::Mutex::new(());
        M.lock().unwrap_or_else(|p| p.into_inner())
    }

    #[test]
    fn resolve_api_key_prefers_env_over_inline() {
        let _g = env_guard();
        let var = "TORVEN_TEST_ENV_WINS_1_6";
        unsafe { std::env::set_var(var, "from-env") };
        let got = resolve_api_key("Zai", var, Some("from-inline")).unwrap();
        unsafe { std::env::remove_var(var) };
        assert_eq!(got, "from-env");
    }

    #[test]
    fn resolve_api_key_falls_back_to_inline() {
        let _g = env_guard();
        let var = "TORVEN_TEST_INLINE_FALLBACK_1_6";
        unsafe { std::env::remove_var(var) };
        let got = resolve_api_key("Zai", var, Some("inline-key")).unwrap();
        assert_eq!(got, "inline-key");
    }

    #[test]
    fn resolve_api_key_errors_when_both_missing() {
        let _g = env_guard();
        let var = "TORVEN_TEST_BOTH_MISSING_1_6";
        unsafe { std::env::remove_var(var) };
        let err = resolve_api_key("Zai", var, None).unwrap_err();
        match err {
            crate::error::AppError::Credentials(msg) => {
                assert!(msg.contains(var), "error should name env var: {msg}");
                assert!(
                    msg.contains("accounts"),
                    "error should suggest accounts section: {msg}"
                );
            }
            other => panic!("expected Credentials error, got {other:?}"),
        }
    }

    #[test]
    fn enabled_vendors_preserves_canonical_order() {
        let c = Config::default();
        assert_eq!(
            c.enabled_vendors(),
            vec![
                VendorId::Anthropic,
                VendorId::Openai,
                VendorId::Zai,
                VendorId::Openrouter
            ]
        );
    }

    #[test]
    fn account_id_is_deterministic_and_lowercased() {
        assert_eq!(
            account_id(VendorId::Openrouter, "ClienteAcme"),
            "openrouter-clienteacme"
        );
        // Case is normalized — "Personal" and "personal" collide intentionally.
        assert_eq!(
            account_id(VendorId::Zai, "Personal"),
            account_id(VendorId::Zai, "personal")
        );
    }

    #[test]
    fn is_valid_api_key_rejects_spaces_and_specials() {
        assert!(is_valid_api_key("sk-or-v1-abc123"));
        assert!(is_valid_api_key("sk_or.v1-abc.123_XYZ")); // dots + mix allowed
        assert!(!is_valid_api_key("sk or v1")); // space
        assert!(!is_valid_api_key("sk-or-!@#")); // specials
        assert!(!is_valid_api_key("")); // empty
    }
}
