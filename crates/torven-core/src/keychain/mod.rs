pub mod fallback;
#[cfg(target_os = "macos")]
pub mod mac;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{Config, account_id, save_config};

pub use fallback::FileFallbackStore;
#[cfg(target_os = "macos")]
pub use mac::MacKeychainStore;

#[derive(Debug, thiserror::Error)]
pub enum KeychainError {
    #[error("keychain storage error: {0}")]
    Store(String),

    #[error("keychain blob JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("keychain file IO error at {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not resolve default secrets directory")]
    DefaultPathUnavailable,

    #[error("config save failed after keychain migration: {0}")]
    ConfigSave(#[from] crate::error::AppError),
}

pub trait SecretStore: Send + Sync {
    fn get_accounts_blob(&self, vendor: &str) -> Result<Option<Vec<AccountSecret>>, KeychainError>;
    fn set_accounts_blob(
        &self,
        vendor: &str,
        accounts: &[AccountSecret],
    ) -> Result<(), KeychainError>;
    fn delete_vendor_entry(&self, vendor: &str) -> Result<(), KeychainError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSecret {
    pub account_id: String,
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsBlob {
    pub version: u32,
    pub accounts: Vec<AccountSecret>,
}

impl AccountsBlob {
    pub const VERSION: u32 = 1;

    pub fn new(accounts: Vec<AccountSecret>) -> Self {
        Self {
            version: Self::VERSION,
            accounts,
        }
    }
}

pub fn encode_accounts_blob(accounts: &[AccountSecret]) -> Result<Vec<u8>, KeychainError> {
    Ok(serde_json::to_vec(&AccountsBlob::new(accounts.to_vec()))?)
}

pub fn decode_accounts_blob(bytes: &[u8]) -> Result<Vec<AccountSecret>, KeychainError> {
    let blob: AccountsBlob = serde_json::from_slice(bytes)?;
    Ok(blob.accounts)
}

pub fn migrate_keys_to_keychain(config: &mut Config, store: &dyn SecretStore) -> Vec<String> {
    let mut warnings = Vec::new();

    for (vendor, accounts) in &mut config.accounts {
        let vendor_slug = vendor.slug();
        let mut existing = match store.get_accounts_blob(vendor_slug) {
            Ok(Some(accounts)) => accounts,
            Ok(None) => Vec::new(),
            Err(err) => {
                warnings.push(format!(
                    "{vendor_slug}: keychain migration skipped; could not read store: {err}"
                ));
                continue;
            }
        };
        let mut migrated_accounts = Vec::new();

        for (index, account) in accounts.iter().enumerate() {
            let Some(api_key) = account.api_key.clone() else {
                continue;
            };
            let id = account_id(*vendor, &account.name);
            upsert_secret(
                &mut existing,
                AccountSecret {
                    account_id: id.clone(),
                    api_key,
                },
            );
            migrated_accounts.push((index, id, account.name.clone()));
        }

        if !migrated_accounts.is_empty() {
            match store.set_accounts_blob(vendor_slug, &existing) {
                Ok(()) => {
                    for (index, id, name) in migrated_accounts {
                        if let Some(account) = accounts.get_mut(index) {
                            account.api_key = None;
                        }
                        warnings.push(format!(
                            "{vendor_slug}: moved API key for account '{name}' to Keychain blob {id}"
                        ));
                    }
                }
                Err(err) => {
                    warnings.push(format!(
                        "{vendor_slug}: failed to persist Keychain blob after migration: {err}"
                    ));
                }
            }
        }
    }

    warnings
}

pub fn migrate_keys_to_keychain_and_save(
    config: &mut Config,
    config_path: &Path,
    store: &dyn SecretStore,
) -> Result<Vec<String>, KeychainError> {
    let warnings = migrate_keys_to_keychain(config, store);
    save_config(config, config_path)?;
    Ok(warnings)
}

pub fn blob_bytes_from_store(
    store: &dyn SecretStore,
    vendor: &str,
) -> Result<Option<Vec<u8>>, KeychainError> {
    let Some(accounts) = store.get_accounts_blob(vendor)? else {
        return Ok(None);
    };
    Ok(Some(encode_accounts_blob(&accounts)?))
}

pub fn set_blob_bytes_for_store(
    store: &dyn SecretStore,
    vendor: &str,
    blob: &[u8],
) -> Result<(), KeychainError> {
    let accounts = decode_accounts_blob(blob)?;
    store.set_accounts_blob(vendor, &accounts)
}

fn upsert_secret(accounts: &mut Vec<AccountSecret>, secret: AccountSecret) {
    if let Some(existing) = accounts
        .iter_mut()
        .find(|account| account.account_id == secret.account_id)
    {
        *existing = secret;
    } else {
        accounts.push(secret);
    }
}
