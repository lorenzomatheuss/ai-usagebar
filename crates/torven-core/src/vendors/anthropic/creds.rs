//! Read and write Anthropic OAuth credentials.
//!
//! Story 5.5.2 (Wave 5.5) — `CredsSource` dual-source: legacy file
//! `~/.claude/.credentials.json` (Claude CLI standalone) **and** macOS Keychain
//! entry `"Claude Code-credentials"` (Claude Code app). Try-keychain-first,
//! fall-back-to-file ordering, both writeback-symmetric for token refresh.
//!
//! Schema spike GO verdict: `docs/architecture/spikes/claude-code-keychain-schema.md`
//! — the Keychain JSON blob mirrors the legacy file shape under the same
//! `claudeAiOauth.*` key tree, so `serde_json::from_str::<CredentialsFile>` parses
//! both sources without an adapter. Extra top-level `trustedDeviceToken` is
//! silently dropped by serde (not in struct).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cache::atomic_write;
use crate::error::{AppError, Result};

/// Canonical macOS Keychain service name written by the Claude Code app.
/// Confirmed by spike (`security find-generic-password -s "Claude Code-credentials"`).
pub const CLAUDE_CODE_KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

/// Disk shape (matches claudebar's jq paths). Also matches the Keychain JSON
/// blob written by Claude Code (spike-confirmed 2026-06-07).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    pub claude_ai_oauth: OauthCreds,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OauthCreds {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
    /// Unix epoch in **milliseconds** (claudebar:445 multiplies seconds × 1000).
    /// May arrive as a float in the wild — claudebar truncates with `%%.*`,
    /// so we accept both. Spike confirmed Claude Code's Keychain blob also
    /// uses ms epoch (not seconds) — same unit as the legacy file format.
    #[serde(rename = "expiresAt", deserialize_with = "de_ms_epoch")]
    pub expires_at_ms: i64,
    #[serde(rename = "subscriptionType", default)]
    pub subscription_type: String,
    #[serde(rename = "rateLimitTier", default)]
    pub rate_limit_tier: String,
    /// Optional `scopes` array — preserved through round-trips so we don't
    /// drop information when we write back after a refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<serde_json::Value>,
}

fn de_ms_epoch<'de, D>(d: D) -> std::result::Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Accept int or float — float values like 5000.0 are truncated.
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i)
            } else if let Some(f) = n.as_f64() {
                Ok(f as i64)
            } else {
                Err(serde::de::Error::custom("expiresAt not numeric"))
            }
        }
        _ => Err(serde::de::Error::custom("expiresAt must be a number")),
    }
}

impl OauthCreds {
    /// Plan label rendered the way claudebar does (claudebar:547-550):
    ///   "${sub_type^} [5x|20x]" (first letter capitalized, optional tier suffix).
    pub fn plan_label(&self) -> String {
        let mut name = capitalize_first(&self.subscription_type);
        if name.is_empty() {
            name = "Unknown".into();
        }
        if self.rate_limit_tier.contains("5x") {
            name.push_str(" 5x");
        } else if self.rate_limit_tier.contains("20x") {
            name.push_str(" 20x");
        }
        name
    }

    pub fn expires_at_secs(&self) -> i64 {
        self.expires_at_ms / 1000
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => {
            let mut out = String::with_capacity(s.len());
            for c in first.to_uppercase() {
                out.push(c);
            }
            out.push_str(chars.as_str());
            out
        }
        None => String::new(),
    }
}

/// Default file location: `~/.claude/.credentials.json`.
pub fn default_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| AppError::Other("HOME not set".into()))?;
    Ok(PathBuf::from(home).join(".claude/.credentials.json"))
}

// ---------------------------------------------------------------------------
// Story 5.5.2 — `CredsSource` dual-source enum (AC-1)
// ---------------------------------------------------------------------------

/// Where to read/write Anthropic OAuth credentials.
///
/// Story 5.5.2 (Wave 5.5 / WAVE5.5-D5): dual-source. Claude Code stores creds
/// in macOS Keychain entry `"Claude Code-credentials"`; legacy Claude CLI
/// standalone stores them in `~/.claude/.credentials.json`. Both sources share
/// the same JSON shape (spike-confirmed) so the same parser handles both.
#[derive(Debug, Clone)]
pub enum CredsSource {
    /// Legacy file path (Claude CLI standalone). Typically
    /// `~/.claude/.credentials.json`.
    File(PathBuf),
    /// macOS Keychain service name (Claude Code). Typically
    /// `"Claude Code-credentials"`.
    Keychain(String),
}

impl CredsSource {
    /// Short, log-friendly tag identifying the source (no secret content).
    pub fn tag(&self) -> &'static str {
        match self {
            CredsSource::File(_) => "claude_cli_file",
            CredsSource::Keychain(_) => "claude_code_keychain",
        }
    }
}

/// Resolve the default Anthropic credentials source.
///
/// Try Keychain first (Claude Code is the recommended setup on macOS); if the
/// Keychain entry is missing or unreadable, fall back to the legacy file path.
/// Caller is responsible for handling the case where *neither* source has
/// credentials (treat as "user must log in").
pub fn default_anthropic_creds_source() -> CredsSource {
    // Keychain is macOS-only. On other platforms we always fall through to the
    // file path.
    #[cfg(target_os = "macos")]
    {
        if read_from_keychain(CLAUDE_CODE_KEYCHAIN_SERVICE).is_ok() {
            return CredsSource::Keychain(CLAUDE_CODE_KEYCHAIN_SERVICE.to_string());
        }
    }
    // Fall back to the legacy file path even if it doesn't exist — downstream
    // callers handle ENOENT with an "actionable Credentials error" message.
    CredsSource::File(default_path().unwrap_or_default())
}

/// Dispatch source-aware read. Returns the parsed `CredentialsFile` from either
/// the file path or the Keychain entry. Distinguishes "credentials missing"
/// (user must re-auth) from generic I/O errors via `AppError::Credentials`.
pub fn read_from_source(source: &CredsSource) -> Result<CredentialsFile> {
    match source {
        CredsSource::File(p) => read_from(p),
        CredsSource::Keychain(svc) => read_from_keychain(svc),
    }
}

/// Dispatch source-aware writeback after a token refresh. Symmetric with
/// `read_from_source`.
pub fn write_back_to_source(source: &CredsSource, new_oauth: &OauthCreds) -> Result<()> {
    match source {
        CredsSource::File(p) => write_back(p, new_oauth),
        CredsSource::Keychain(svc) => write_back_to_keychain(svc, new_oauth),
    }
}

pub fn read_from(path: &Path) -> Result<CredentialsFile> {
    let raw = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            // Missing creds file is an "actionable" condition — the user
            // needs to run `claude` to log in. Surface as `Credentials` so
            // the menu-bar / TUI can suggest re-auth rather than a generic
            // IO banner. Hint at the config knob (`credentials_path`) in
            // case the user keeps creds in a non-default location.
            AppError::Credentials(format!(
                "Claude OAuth credentials not found at {}. Run `claude` to \
                 log in, or set `credentials_path` in ~/.config/torven/config.toml.",
                path.display()
            ))
        } else {
            AppError::io_at(path, e)
        }
    })?;
    serde_json::from_str(&raw).map_err(|e| {
        AppError::Credentials(format!(
            "could not parse {}: {e}. Run `claude` to re-authenticate.",
            path.display()
        ))
    })
}

/// Persist updated credentials, preserving any unknown top-level fields the
/// Claude CLI might have added. Reads the existing file, merges our updates
/// into the `claudeAiOauth` object, and atomically writes it back.
pub fn write_back(path: &Path, new_oauth: &OauthCreds) -> Result<()> {
    let mut doc: serde_json::Value = std::fs::read_to_string(path)
        .map_err(|e| AppError::io_at(path, e))
        .and_then(|s| serde_json::from_str(&s).map_err(AppError::Json))
        .unwrap_or_else(|_| serde_json::json!({}));

    let obj = match doc.as_object_mut() {
        Some(o) => o,
        None => {
            doc = serde_json::json!({});
            doc.as_object_mut().expect("just constructed object")
        }
    };
    obj.insert(
        "claudeAiOauth".into(),
        serde_json::to_value(new_oauth).map_err(AppError::Json)?,
    );

    let bytes = serde_json::to_vec_pretty(&doc).map_err(AppError::Json)?;
    atomic_write(path, &bytes)
}

// ---------------------------------------------------------------------------
// Story 5.5.2 — Keychain read/write (AC-2, AC-3)
// ---------------------------------------------------------------------------

// macOS Keychain backend uses the `security-framework` crate directly. We
// intentionally NOT use the more abstract `SecretStore` trait from
// `keychain::mod` because Claude Code's entry was created with a different
// service-naming convention (`"Claude Code-credentials"` is the literal
// service string; there is no `account` qualifier — `security` CLI shows
// account as empty). Mirroring that exact lookup keeps interop deterministic.

/// Read and parse the Claude Code Keychain OAuth blob.
///
/// macOS-only. On other platforms returns `AppError::Credentials` with a
/// "platform unsupported" hint so the caller (typically `read_from_source`)
/// surfaces a graceful "not configured" rather than crashing.
///
/// Failure modes:
/// - `errSecItemNotFound` (-25300) → `AppError::Credentials("…not found…")`
/// - other security-framework error → `AppError::Credentials(...)` with the
///   underlying message (treats as actionable; user must re-login via Claude
///   Code app)
/// - JSON parse failure → `AppError::Credentials("could not parse…")`
#[cfg(target_os = "macos")]
pub fn read_from_keychain(service: &str) -> Result<CredentialsFile> {
    // Claude Code uses generic-password with NO account qualifier. Pass empty
    // account to mirror `security find-generic-password -s "..." -w` lookup.
    let blob = security_framework::passwords::get_generic_password(service, "").map_err(|err| {
        AppError::Credentials(format!(
            "Claude Code OAuth credentials not found in Keychain (service: \"{service}\"). \
             Open the Claude Code app and sign in to populate the Keychain entry. \
             Underlying error: {err}"
        ))
    })?;

    let blob_str = std::str::from_utf8(&blob).map_err(|e| {
        AppError::Credentials(format!(
            "Claude Code Keychain blob is not valid UTF-8: {e}. Re-login via Claude Code."
        ))
    })?;

    serde_json::from_str::<CredentialsFile>(blob_str).map_err(|e| {
        AppError::Credentials(format!(
            "Claude Code Keychain blob (service: \"{service}\") could not be parsed: {e}. \
             Re-login via Claude Code."
        ))
    })
}

/// Non-macOS stub. Always returns `Credentials` error — the keychain entry
/// only exists on macOS (per WAVE5-D7 platform scope).
#[cfg(not(target_os = "macos"))]
pub fn read_from_keychain(_service: &str) -> Result<CredentialsFile> {
    Err(AppError::Credentials(
        "Claude Code Keychain is macOS-only. Use the legacy \
         `~/.claude/.credentials.json` file path on this platform."
            .to_string(),
    ))
}

/// Persist refreshed credentials back to the Claude Code Keychain entry.
///
/// Reads the existing blob to preserve unknown top-level fields (e.g.
/// `trustedDeviceToken`) the same way `write_back` (file) does, then
/// re-writes the `claudeAiOauth` subtree.
///
/// macOS-only; non-macOS returns the same `Credentials` error as
/// `read_from_keychain`.
#[cfg(target_os = "macos")]
pub fn write_back_to_keychain(service: &str, new_oauth: &OauthCreds) -> Result<()> {
    // Preserve unknown top-level fields (`trustedDeviceToken`, future
    // additions) by reading the existing blob and merging.
    let mut doc: serde_json::Value =
        match security_framework::passwords::get_generic_password(service, "") {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| serde_json::json!({})),
            Err(_) => serde_json::json!({}),
        };

    let obj = match doc.as_object_mut() {
        Some(o) => o,
        None => {
            doc = serde_json::json!({});
            doc.as_object_mut().expect("just constructed object")
        }
    };
    obj.insert(
        "claudeAiOauth".into(),
        serde_json::to_value(new_oauth).map_err(AppError::Json)?,
    );

    let bytes = serde_json::to_vec(&doc).map_err(AppError::Json)?;
    security_framework::passwords::set_generic_password(service, "", &bytes).map_err(|err| {
        AppError::Credentials(format!(
            "Could not write refreshed Anthropic OAuth credentials to Keychain \
             (service: \"{service}\"): {err}"
        ))
    })
}

#[cfg(not(target_os = "macos"))]
pub fn write_back_to_keychain(_service: &str, _new_oauth: &OauthCreds) -> Result<()> {
    Err(AppError::Credentials(
        "Claude Code Keychain writeback is macOS-only.".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_creds(s: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(s.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn parses_canonical_shape() {
        let f = write_creds(
            r#"{"claudeAiOauth":{
                "accessToken":"AT",
                "refreshToken":"RT",
                "expiresAt": 1735000000000,
                "subscriptionType":"max",
                "rateLimitTier":"default_claude_max_5x"
            }}"#,
        );
        let creds = read_from(f.path()).unwrap();
        assert_eq!(creds.claude_ai_oauth.access_token, "AT");
        assert_eq!(creds.claude_ai_oauth.expires_at_ms, 1735000000000);
        assert_eq!(creds.claude_ai_oauth.plan_label(), "Max 5x");
    }

    #[test]
    fn accepts_float_expires_at() {
        // claudebar truncates `5000.0 → 5000`; we do the same.
        let f = write_creds(
            r#"{"claudeAiOauth":{
                "accessToken":"A","refreshToken":"R",
                "expiresAt": 5000.0,
                "subscriptionType":"pro","rateLimitTier":""
            }}"#,
        );
        let creds = read_from(f.path()).unwrap();
        assert_eq!(creds.claude_ai_oauth.expires_at_ms, 5000);
    }

    #[test]
    fn plan_label_pro_no_tier() {
        let f = write_creds(
            r#"{"claudeAiOauth":{
                "accessToken":"A","refreshToken":"R","expiresAt": 0,
                "subscriptionType":"pro","rateLimitTier":""
            }}"#,
        );
        let creds = read_from(f.path()).unwrap();
        assert_eq!(creds.claude_ai_oauth.plan_label(), "Pro");
    }

    #[test]
    fn plan_label_max_20x() {
        let f = write_creds(
            r#"{"claudeAiOauth":{
                "accessToken":"A","refreshToken":"R","expiresAt": 0,
                "subscriptionType":"max","rateLimitTier":"default_claude_max_20x"
            }}"#,
        );
        let creds = read_from(f.path()).unwrap();
        assert_eq!(creds.claude_ai_oauth.plan_label(), "Max 20x");
    }

    #[test]
    fn plan_label_empty_subscription_falls_back() {
        let f = write_creds(
            r#"{"claudeAiOauth":{
                "accessToken":"A","refreshToken":"R","expiresAt": 0,
                "subscriptionType":"","rateLimitTier":""
            }}"#,
        );
        let creds = read_from(f.path()).unwrap();
        assert_eq!(creds.claude_ai_oauth.plan_label(), "Unknown");
    }

    #[test]
    fn malformed_file_returns_credentials_error() {
        let f = write_creds("not json");
        let err = read_from(f.path()).unwrap_err();
        assert!(matches!(err, AppError::Credentials(_)));
    }

    #[test]
    fn missing_file_returns_actionable_credentials_error() {
        let path = std::env::temp_dir().join("torven-missing-claude-creds.json");
        let err = read_from(&path).unwrap_err();
        match err {
            AppError::Credentials(msg) => {
                assert!(msg.contains("Claude OAuth credentials not found"));
                assert!(msg.contains("credentials_path"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn write_back_round_trips_and_preserves_unknown_fields() {
        let f = write_creds(
            r#"{"claudeAiOauth":{
                "accessToken":"OLD","refreshToken":"OLD","expiresAt": 0,
                "subscriptionType":"pro","rateLimitTier":""
            },"someOtherField":"keep me"}"#,
        );
        let creds = read_from(f.path()).unwrap();
        let new_oauth = OauthCreds {
            access_token: "NEW".into(),
            refresh_token: "NEW_RT".into(),
            expires_at_ms: 1234,
            subscription_type: "pro".into(),
            rate_limit_tier: "".into(),
            scopes: creds.claude_ai_oauth.scopes.clone(),
        };
        write_back(f.path(), &new_oauth).unwrap();
        // Re-read & verify the unknown field survived.
        let raw = std::fs::read_to_string(f.path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["someOtherField"], "keep me");
        assert_eq!(v["claudeAiOauth"]["accessToken"], "NEW");
        assert_eq!(v["claudeAiOauth"]["expiresAt"], 1234);
    }

    // ---------------------------------------------------------------------
    // Story 5.5.2 — AC-6 — Keychain dual-source unit tests
    // ---------------------------------------------------------------------

    /// AC-6 test 1: a well-formed Claude Code blob (including the extra
    /// `trustedDeviceToken` top-level field that serde drops silently) parses
    /// directly into `CredentialsFile` via `serde_json::from_str`. This is
    /// the exact path `read_from_keychain` takes once the OS returns the blob.
    #[test]
    fn read_from_keychain_parses_valid_blob() {
        // Schema mirrors the spike's documented shape:
        // `{ "claudeAiOauth": {...}, "trustedDeviceToken": "..." }`.
        let blob = r#"{
            "claudeAiOauth": {
                "accessToken": "AT-keychain",
                "refreshToken": "RT-keychain",
                "expiresAt": 1735000000000,
                "subscriptionType": "max",
                "rateLimitTier": "default_claude_max_5x",
                "scopes": ["user:profile", "org:read"]
            },
            "trustedDeviceToken": "OPAQUE-DEVICE-TOKEN"
        }"#;

        let parsed: CredentialsFile =
            serde_json::from_str(blob).expect("blob with Claude Code shape must parse");
        assert_eq!(parsed.claude_ai_oauth.access_token, "AT-keychain");
        assert_eq!(parsed.claude_ai_oauth.refresh_token, "RT-keychain");
        assert_eq!(parsed.claude_ai_oauth.expires_at_ms, 1735000000000);
        assert_eq!(parsed.claude_ai_oauth.subscription_type, "max");
        assert_eq!(parsed.claude_ai_oauth.plan_label(), "Max 5x");
        assert!(parsed.claude_ai_oauth.scopes.is_some());
        // `trustedDeviceToken` is silently dropped — verify by re-serializing.
        let round_trip = serde_json::to_value(&parsed).unwrap();
        assert!(
            round_trip.get("trustedDeviceToken").is_none(),
            "trustedDeviceToken should not round-trip into the struct"
        );
    }

    /// AC-6 test 2: when the requested Keychain service is missing,
    /// `read_from_keychain` returns `AppError::Credentials` with an actionable
    /// "re-login via Claude Code" hint — not a panic, not a generic I/O error.
    /// Uses a service name that is statistically guaranteed not to exist on
    /// the developer / CI machine (random-tagged).
    #[cfg(target_os = "macos")]
    #[test]
    fn read_from_keychain_returns_credentials_err_when_missing() {
        let fake_service = format!(
            "torven-test-missing-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let err = read_from_keychain(&fake_service).expect_err("must error when missing");
        match err {
            AppError::Credentials(msg) => {
                assert!(
                    msg.contains("Claude Code OAuth credentials not found"),
                    "expected actionable hint, got: {msg}"
                );
            }
            other => panic!("expected Credentials error, got {other:?}"),
        }
    }

    /// AC-6 test 2 (non-macOS variant): the stub always returns
    /// `Credentials` with a "macOS-only" hint. Verifies no panic on other
    /// platforms when callers ask for the Keychain source.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn read_from_keychain_returns_credentials_err_when_missing() {
        let err = read_from_keychain("anything").expect_err("must error on non-macOS");
        match err {
            AppError::Credentials(msg) => {
                assert!(msg.contains("macOS-only"));
            }
            other => panic!("expected Credentials error, got {other:?}"),
        }
    }

    /// AC-6 test 3: `default_anthropic_creds_source()` prefers Keychain when
    /// the Keychain entry is readable. We can't write to the actual
    /// `"Claude Code-credentials"` entry on a developer machine without
    /// trashing the real OAuth state, so this test asserts the contract
    /// indirectly: if `read_from_keychain(CLAUDE_CODE_KEYCHAIN_SERVICE)`
    /// happens to succeed on this machine (developer logged into Claude Code),
    /// the function MUST return `Keychain(...)`. Otherwise it MUST return
    /// `File(...)`. Either branch validates the dispatch logic without
    /// requiring the test to mutate Keychain state.
    #[test]
    fn default_anthropic_creds_source_prefers_keychain_when_present() {
        let source = default_anthropic_creds_source();

        #[cfg(target_os = "macos")]
        {
            let kc_works = read_from_keychain(CLAUDE_CODE_KEYCHAIN_SERVICE).is_ok();
            match (&source, kc_works) {
                (CredsSource::Keychain(svc), true) => {
                    assert_eq!(svc, CLAUDE_CODE_KEYCHAIN_SERVICE);
                }
                (CredsSource::File(_), false) => {
                    // Expected fallback: Keychain entry not populated on this
                    // machine, so we got File(...) per AC-5.
                }
                (other, kc_works) => panic!(
                    "dispatch contract violated: source={other:?}, keychain_readable={kc_works}"
                ),
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            // On non-macOS, the macOS Keychain path is compiled out, so we
            // must always get File(...).
            assert!(
                matches!(source, CredsSource::File(_)),
                "non-macOS must always fall back to File source, got {source:?}"
            );
        }
    }

    /// AC-6 test 4: when the Keychain entry is unreadable (missing /
    /// non-macOS / parse failure), `default_anthropic_creds_source()` MUST
    /// fall back to `File(default_path())`. Exercised here by running on
    /// non-macOS (where `read_from_keychain` always errors) OR by asserting
    /// the source is one of the two valid variants on macOS — the latter is
    /// already covered by test 3. To exercise the fallback deterministically
    /// we test the `is_ok().is_some()` path of the resolver: if Keychain
    /// read returns `Err`, the resolver returns `File`. On a CI machine
    /// without Claude Code logged in, this is the typical case.
    #[test]
    fn default_anthropic_creds_source_falls_back_to_file_when_keychain_missing() {
        // We can deterministically test the fallback by checking that the
        // resolver, when it cannot read Keychain (typical CI machine), yields
        // `File(default_path())`. If the developer IS logged into Claude Code
        // on this machine, the resolver may return `Keychain(...)` — that's
        // also valid (AC-5 says try Keychain first). The post-condition we
        // assert is "the chosen path is consistent with what
        // read_from_keychain returns".
        let source = default_anthropic_creds_source();

        #[cfg(target_os = "macos")]
        {
            let kc_works = read_from_keychain(CLAUDE_CODE_KEYCHAIN_SERVICE).is_ok();
            if !kc_works {
                match source {
                    CredsSource::File(p) => {
                        // Should be the default path (or empty path if HOME
                        // missing — defensive against test isolation).
                        let expected = default_path().unwrap_or_default();
                        assert_eq!(p, expected, "expected default file path on fallback");
                    }
                    CredsSource::Keychain(_) => {
                        panic!(
                            "Keychain not readable but resolver returned Keychain source — \
                             dispatch broken"
                        );
                    }
                }
            }
            // If Keychain works, this test is a no-op (covered by test 3).
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Non-macOS always falls back to file.
            match source {
                CredsSource::File(p) => {
                    let expected = default_path().unwrap_or_default();
                    assert_eq!(p, expected);
                }
                other => panic!("non-macOS must fall back to File, got {other:?}"),
            }
        }
    }

    /// Bonus coverage: `CredsSource::tag()` produces stable log-safe strings
    /// (needed for the FFI status reporting in AC-7).
    #[test]
    fn creds_source_tag_returns_stable_strings() {
        assert_eq!(
            CredsSource::File(PathBuf::from("/x")).tag(),
            "claude_cli_file"
        );
        assert_eq!(
            CredsSource::Keychain("anything".into()).tag(),
            "claude_code_keychain"
        );
    }

    /// `read_from_source` correctly dispatches to file vs keychain.
    #[test]
    fn read_from_source_dispatches_to_file_path() {
        let f = write_creds(
            r#"{"claudeAiOauth":{
                "accessToken":"DispatchAT","refreshToken":"DispatchRT",
                "expiresAt": 1, "subscriptionType":"pro", "rateLimitTier":""
            }}"#,
        );
        let src = CredsSource::File(f.path().to_path_buf());
        let creds = read_from_source(&src).unwrap();
        assert_eq!(creds.claude_ai_oauth.access_token, "DispatchAT");
    }
}
