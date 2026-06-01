//! Story 1.6 — Integration tests for the multi-account Config schema.
//!
//! Covers: parse new format, migrate legacy format, toml_edit round-trip
//! (preserves user comments), validation errors, and `AccountTag` serde
//! mapping. Internal-only invariants (regex enforcement, helpers) are kept in
//! `config.rs` unit tests next to the implementation.

use std::io::Write;

use tempfile::NamedTempFile;

use torven_core::config::{
    Account, AccountTag, Config, ConfigError, account_id, save_config, validate_config,
};
use torven_core::vendor::VendorId;

fn write_toml(s: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(s.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

// ---- Parsing the new multi-account schema ----

#[test]
fn test_parse_multi_account_toml() {
    let f = write_toml(
        r#"
        [[openrouter.accounts]]
        name = "ClienteAcme"
        api_key = "sk-or-v1-aaaa"
        tag = "client"
        budget_usd = 200.0

        [[openrouter.accounts]]
        name = "Personal"
        api_key = "sk-or-v1-bbbb"
        tag = "personal"

        [[zai.accounts]]
        name = "Default"
        api_key = "sk-zai-cccc"
        "#,
    );
    let c = Config::load_from(f.path()).expect("parse should succeed");

    let or_accts = c.accounts.get(&VendorId::Openrouter).expect("openrouter");
    assert_eq!(or_accts.len(), 2);
    assert_eq!(or_accts[0].name, "ClienteAcme");
    assert_eq!(or_accts[0].api_key.as_deref(), Some("sk-or-v1-aaaa"));
    assert_eq!(or_accts[0].tag, Some(AccountTag::Client));
    assert_eq!(or_accts[0].budget_usd, Some(200.0));
    assert_eq!(or_accts[1].name, "Personal");
    assert_eq!(or_accts[1].tag, Some(AccountTag::Personal));

    let zai_accts = c.accounts.get(&VendorId::Zai).expect("zai");
    assert_eq!(zai_accts.len(), 1);
    assert_eq!(zai_accts[0].name, "Default");
}

// ---- Legacy migration ----

#[test]
fn test_legacy_migration_openrouter() {
    let toml = r#"
        [openrouter]
        enabled = true
        api_key_env = "OPENROUTER_API_KEY"
        api_key = "sk-or-legacy-1234"
        "#;
    let (c, warnings) = Config::load_from_str_with_warnings(toml).expect("parse");
    let or_accts = c.accounts.get(&VendorId::Openrouter).expect("openrouter");
    assert_eq!(or_accts.len(), 1, "single account migrated from legacy");
    assert_eq!(or_accts[0].name, "default");
    assert_eq!(or_accts[0].api_key.as_deref(), Some("sk-or-legacy-1234"));
    assert!(or_accts[0].tag.is_none());
    assert!(or_accts[0].budget_usd.is_none());

    // Vendor-level env var is preserved across migration.
    assert_eq!(c.openrouter_env.api_key_env, "OPENROUTER_API_KEY");
    assert!(c.openrouter_env.enabled);

    // A warning was emitted.
    assert!(
        warnings.iter().any(|w| w.contains("openrouter") && w.contains("migrated")),
        "expected migration warning, got {warnings:?}"
    );
}

#[test]
fn test_legacy_migration_zai_with_plan_tier() {
    let toml = r#"
        [zai]
        enabled = true
        api_key_env = "ZAI_API_KEY"
        api_key = "sk-zai-legacy"
        plan_tier = "pro"
        "#;
    let c = Config::load_from_str(toml).expect("parse");
    let zai_accts = c.accounts.get(&VendorId::Zai).expect("zai");
    assert_eq!(zai_accts.len(), 1);
    assert_eq!(zai_accts[0].name, "default");
    assert_eq!(zai_accts[0].api_key.as_deref(), Some("sk-zai-legacy"));
    // plan_tier is preserved at the vendor level.
    assert_eq!(c.zai_plan_tier.as_deref(), Some("pro"));
}

#[test]
fn test_no_accounts_no_legacy_key_yields_empty_vec() {
    // Vendor section present but no api_key and no accounts → empty Vec
    // (vendor unconfigured; fetcher will emit Credentials error when invoked).
    let toml = r#"
        [openrouter]
        enabled = true
        api_key_env = "OPENROUTER_API_KEY"
        "#;
    let c = Config::load_from_str(toml).expect("parse");
    let or_accts = c.accounts.get(&VendorId::Openrouter).expect("openrouter");
    assert_eq!(or_accts.len(), 0);
}

#[test]
fn test_accounts_takes_precedence_over_legacy_key() {
    // If BOTH the legacy `api_key` AND `[[accounts]]` are set, accounts wins
    // and a warning is emitted.
    let toml = r#"
        [openrouter]
        enabled = true
        api_key = "legacy-should-be-ignored"

        [[openrouter.accounts]]
        name = "Primary"
        api_key = "new-shape-wins"
        "#;
    let (c, warnings) = Config::load_from_str_with_warnings(toml).expect("parse");
    let or_accts = c.accounts.get(&VendorId::Openrouter).expect("openrouter");
    assert_eq!(or_accts.len(), 1);
    assert_eq!(or_accts[0].name, "Primary");
    assert_eq!(or_accts[0].api_key.as_deref(), Some("new-shape-wins"));
    assert!(
        warnings.iter().any(|w| w.contains("accounts wins")),
        "expected 'accounts wins' warning, got {warnings:?}"
    );
}

// ---- Round-trip with toml_edit ----

#[test]
fn test_config_round_trip_preserves_user_comments() {
    let f = NamedTempFile::new().unwrap();
    let original = r#"# user-written comment ABOVE display
[display]
# user-written comment INSIDE display
refresh_interval_secs = 60

# a comment between sections
[ai_insights]
enabled = false
prompt_version = "v1"
max_cost_usd = 0.10
rate_limit_per_minute = 20

[history]
retention_days = 30

[zai]
enabled = true
api_key_env = "ZAI_API_KEY"

[openrouter]
enabled = true
api_key_env = "OPENROUTER_API_KEY"
"#;
    std::fs::write(f.path(), original).unwrap();

    let mut c = Config::load_from(f.path()).expect("parse");
    // Mutate: add an account.
    c.accounts.get_mut(&VendorId::Openrouter).unwrap().push(Account {
        name: "Personal".to_string(),
        api_key: Some("sk-or-personal".to_string()),
        description: None,
        budget_usd: Some(50.0),
        tag: Some(AccountTag::Personal),
    });
    save_config(&c, f.path()).expect("save");

    let raw = std::fs::read_to_string(f.path()).unwrap();
    // User comments survive.
    assert!(
        raw.contains("# user-written comment ABOVE display"),
        "lost ABOVE comment: {raw}"
    );
    assert!(
        raw.contains("# user-written comment INSIDE display"),
        "lost INSIDE comment: {raw}"
    );
    assert!(
        raw.contains("# a comment between sections"),
        "lost between-section comment: {raw}"
    );

    // Re-parse and assert the mutation landed.
    let c2 = Config::load_from(f.path()).expect("re-parse");
    let or = c2.accounts.get(&VendorId::Openrouter).expect("openrouter");
    assert_eq!(or.len(), 1);
    assert_eq!(or[0].name, "Personal");
    assert_eq!(or[0].budget_usd, Some(50.0));
    assert_eq!(or[0].tag, Some(AccountTag::Personal));
}

#[test]
fn test_round_trip_legacy_then_multi_account() {
    // A user with a legacy `[zai] api_key = "..."` config — after a save the
    // file should have moved into the multi-account form.
    let f = NamedTempFile::new().unwrap();
    let legacy = r#"
[zai]
enabled = true
api_key_env = "ZAI_API_KEY"
api_key = "sk-zai-legacy"
"#;
    std::fs::write(f.path(), legacy).unwrap();

    let c = Config::load_from(f.path()).expect("parse legacy");
    save_config(&c, f.path()).expect("save");

    let raw = std::fs::read_to_string(f.path()).unwrap();
    // The new file uses [[zai.accounts]] form.
    assert!(
        raw.contains("[[zai.accounts]]"),
        "expected [[zai.accounts]] section, got:\n{raw}"
    );
    assert!(raw.contains("name = \"default\""));
    assert!(raw.contains("sk-zai-legacy"));

    // Re-loading produces no migration warnings — we're in the new shape now.
    let (_c2, warnings) =
        Config::load_from_str_with_warnings(&raw).expect("re-parse new shape");
    assert!(
        warnings.is_empty(),
        "expected no warnings after migration, got: {warnings:?}"
    );
}

// ---- Validation ----

#[test]
fn test_validation_budget_negative() {
    let f = write_toml(
        r#"
        [[openrouter.accounts]]
        name = "Bad"
        budget_usd = -5.0
        "#,
    );
    let c = Config::load_from(f.path()).unwrap();
    let errs = validate_config(&c);
    assert!(
        errs.iter().any(|e| matches!(e, ConfigError::NegativeBudget { got, .. } if *got == -5.0)),
        "expected NegativeBudget, got {errs:?}"
    );
}

#[test]
fn test_validation_empty_name_and_duplicate() {
    let f = write_toml(
        r#"
        [[zai.accounts]]
        name = ""

        [[zai.accounts]]
        name = "dup"

        [[zai.accounts]]
        name = "dup"
        "#,
    );
    let c = Config::load_from(f.path()).unwrap();
    let errs = validate_config(&c);
    assert!(
        errs.iter().any(|e| matches!(e, ConfigError::EmptyAccountName { .. })),
        "expected EmptyAccountName, got {errs:?}"
    );
    assert!(
        errs.iter().any(|e| matches!(e, ConfigError::DuplicateAccountName { .. })),
        "expected DuplicateAccountName, got {errs:?}"
    );
}

#[test]
fn test_validation_invalid_api_key_chars() {
    let f = write_toml(
        r#"
        [[zai.accounts]]
        name = "WithSpaces"
        api_key = "has spaces in it"

        [[zai.accounts]]
        name = "WithBang"
        api_key = "boom!"
        "#,
    );
    let c = Config::load_from(f.path()).unwrap();
    let errs = validate_config(&c);
    let count = errs
        .iter()
        .filter(|e| matches!(e, ConfigError::InvalidApiKeyChars { .. }))
        .count();
    assert_eq!(
        count, 2,
        "expected two InvalidApiKeyChars errors, got {errs:?}"
    );
}

#[test]
fn test_validation_refresh_interval_too_low() {
    let f = write_toml(
        r#"
        [display]
        refresh_interval_secs = 5
        "#,
    );
    let c = Config::load_from(f.path()).unwrap();
    let errs = validate_config(&c);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ConfigError::RefreshIntervalTooLow { got: 5 })),
        "expected RefreshIntervalTooLow, got {errs:?}"
    );
}

#[test]
fn test_validation_thresholds_inverted() {
    let f = write_toml(
        r#"
        [display.thresholds]
        amber = 0.95
        critical = 0.75
        "#,
    );
    let c = Config::load_from(f.path()).unwrap();
    let errs = validate_config(&c);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ConfigError::InvalidThresholds { .. })),
        "expected InvalidThresholds, got {errs:?}"
    );
}

#[test]
fn test_validation_passes_on_default_config() {
    let c = Config::default();
    let errs = validate_config(&c);
    assert!(errs.is_empty(), "default config should validate; got {errs:?}");
}

// ---- AccountTag serde ----

#[test]
fn test_account_tag_serde_round_trip() {
    // Reuse the parse machinery: write each tag and confirm the enum maps.
    for (tag_str, tag_enum) in [
        ("client", AccountTag::Client),
        ("personal", AccountTag::Personal),
        ("team", AccountTag::Team),
    ] {
        let toml = format!(
            r#"
            [[openrouter.accounts]]
            name = "X"
            tag = "{tag_str}"
            "#
        );
        let c = Config::load_from_str(&toml).unwrap();
        let acct = &c.accounts.get(&VendorId::Openrouter).unwrap()[0];
        assert_eq!(acct.tag, Some(tag_enum));
    }
}

// ---- account_id helper ----

#[test]
fn test_account_id_is_deterministic_across_calls() {
    let id1 = account_id(VendorId::Openrouter, "ClienteAcme");
    let id2 = account_id(VendorId::Openrouter, "ClienteAcme");
    assert_eq!(id1, id2);
    assert_eq!(id1, "openrouter-clienteacme");
}

// ---- primary_account / primary_api_key helpers ----

#[test]
fn test_primary_account_returns_first_or_none() {
    let toml = r#"
        [[openrouter.accounts]]
        name = "First"
        api_key = "key-1"

        [[openrouter.accounts]]
        name = "Second"
        api_key = "key-2"
        "#;
    let c = Config::load_from_str(toml).unwrap();
    let primary = c
        .primary_account(VendorId::Openrouter)
        .expect("primary present");
    assert_eq!(primary.name, "First");
    assert_eq!(c.primary_api_key(VendorId::Openrouter), Some("key-1"));

    // Zai has no accounts → None.
    assert!(c.primary_account(VendorId::Zai).is_none());
    assert!(c.primary_api_key(VendorId::Zai).is_none());
}

// ---- Default config sanity ----

#[test]
fn test_default_config_has_empty_account_vecs_for_keyed_vendors() {
    let c = Config::default();
    assert_eq!(c.accounts.get(&VendorId::Openrouter).unwrap().len(), 0);
    assert_eq!(c.accounts.get(&VendorId::Zai).unwrap().len(), 0);
}

// ---- config.example.toml parses cleanly ----

#[test]
fn test_example_config_parses_and_validates() {
    // Find the repo-root config.example.toml regardless of where `cargo test`
    // was invoked from (some test runners cd into the crate dir).
    let mut cursor = std::env::current_dir().unwrap();
    let example = loop {
        let candidate = cursor.join("config.example.toml");
        if candidate.exists() {
            break candidate;
        }
        if !cursor.pop() {
            panic!("config.example.toml not found in any parent directory");
        }
    };
    let c = Config::load_from(&example).expect("config.example.toml must parse");
    let errs = validate_config(&c);
    assert!(
        errs.is_empty(),
        "config.example.toml must pass validation, got: {errs:?}"
    );
    // Sanity: the example explicitly enables OpenRouter & Z.AI.
    assert!(c.is_enabled(VendorId::Openrouter));
    assert!(c.is_enabled(VendorId::Zai));
}
