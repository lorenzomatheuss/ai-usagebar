use std::collections::HashMap;

use torven_core::config::Config;
use torven_core::keychain::{
    AccountSecret, FileFallbackStore, SecretStore, migrate_keys_to_keychain,
};
use torven_core::vendor::VendorId;

fn account_map(accounts: Vec<AccountSecret>) -> HashMap<String, String> {
    accounts
        .into_iter()
        .map(|account| (account.account_id, account.api_key))
        .collect()
}

#[test]
fn test_file_fallback_store_set_get() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let store = FileFallbackStore::new(tempdir.path());
    let accounts = vec![
        AccountSecret {
            account_id: "openrouter-client".to_string(),
            api_key: "sk-or-client".to_string(),
        },
        AccountSecret {
            account_id: "openrouter-personal".to_string(),
            api_key: "sk-or-personal".to_string(),
        },
    ];

    store
        .set_accounts_blob("openrouter", &accounts)
        .expect("write fallback blob");
    let got = store
        .get_accounts_blob("openrouter")
        .expect("read fallback blob")
        .expect("blob exists");
    assert_eq!(got, accounts);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(store.path_for_vendor("openrouter"))
            .expect("secret file metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    store
        .delete_vendor_entry("openrouter")
        .expect("delete fallback blob");
    assert!(
        store
            .get_accounts_blob("openrouter")
            .expect("read missing blob")
            .is_none()
    );
}

#[test]
fn test_file_fallback_migration() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let store = FileFallbackStore::new(tempdir.path());
    let mut config = Config::load_from_str(
        r#"
        [[openrouter.accounts]]
        name = "Client"
        api_key = "sk-or-client"

        [[openrouter.accounts]]
        name = "Personal"
        api_key = "sk-or-personal"

        [[zai.accounts]]
        name = "Default"
        api_key = "sk-zai-default"
        "#,
    )
    .expect("parse config");

    let warnings = migrate_keys_to_keychain(&mut config, &store);
    assert_eq!(warnings.len(), 3);

    for vendor in [VendorId::Openrouter, VendorId::Zai] {
        let accounts = config.accounts.get(&vendor).expect("vendor accounts");
        assert!(
            accounts.iter().all(|account| account.api_key.is_none()),
            "{vendor:?} inline api_keys should be removed after migration"
        );
    }

    let openrouter = account_map(
        store
            .get_accounts_blob("openrouter")
            .expect("read openrouter blob")
            .expect("openrouter blob exists"),
    );
    assert_eq!(
        openrouter.get("openrouter-client").map(String::as_str),
        Some("sk-or-client")
    );
    assert_eq!(
        openrouter.get("openrouter-personal").map(String::as_str),
        Some("sk-or-personal")
    );

    let zai = account_map(
        store
            .get_accounts_blob("zai")
            .expect("read zai blob")
            .expect("zai blob exists"),
    );
    assert_eq!(
        zai.get("zai-default").map(String::as_str),
        Some("sk-zai-default")
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "macOS Keychain real integration spike; may prompt for login keychain access"]
fn test_mac_keychain_set_get_delete() {
    use torven_core::keychain::MacKeychainStore;

    let store = MacKeychainStore;
    let vendor = "torven-test-openrouter";
    let accounts = vec![AccountSecret {
        account_id: "openrouter-test".to_string(),
        api_key: "sk-test-keychain".to_string(),
    }];

    store
        .set_accounts_blob(vendor, &accounts)
        .expect("set macOS keychain blob");
    let got = store
        .get_accounts_blob(vendor)
        .expect("get macOS keychain blob")
        .expect("blob exists");
    assert_eq!(got, accounts);
    store
        .delete_vendor_entry(vendor)
        .expect("delete macOS keychain blob");
}
