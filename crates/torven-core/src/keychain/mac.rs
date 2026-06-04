use crate::keychain::{
    AccountSecret, KeychainError, SecretStore, decode_accounts_blob, encode_accounts_blob,
};

const ACCOUNT_NAME: &str = "accounts-blob-v1";
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

pub struct MacKeychainStore;

impl MacKeychainStore {
    fn service_name(vendor: &str) -> String {
        format!("com.torven.app.{vendor}")
    }
}

impl SecretStore for MacKeychainStore {
    fn get_accounts_blob(&self, vendor: &str) -> Result<Option<Vec<AccountSecret>>, KeychainError> {
        let service = Self::service_name(vendor);
        match security_framework::passwords::get_generic_password(&service, ACCOUNT_NAME) {
            Ok(bytes) => decode_accounts_blob(&bytes).map(Some),
            Err(err) if err.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
            Err(err) => Err(KeychainError::Store(err.to_string())),
        }
    }

    fn set_accounts_blob(
        &self,
        vendor: &str,
        accounts: &[AccountSecret],
    ) -> Result<(), KeychainError> {
        let service = Self::service_name(vendor);
        let blob = encode_accounts_blob(accounts)?;
        match security_framework::passwords::delete_generic_password(&service, ACCOUNT_NAME) {
            Ok(()) => {}
            Err(err) if err.code() == ERR_SEC_ITEM_NOT_FOUND => {}
            Err(err) => return Err(KeychainError::Store(err.to_string())),
        }
        security_framework::passwords::set_generic_password(&service, ACCOUNT_NAME, &blob)
            .map_err(|err| KeychainError::Store(err.to_string()))
    }

    fn delete_vendor_entry(&self, vendor: &str) -> Result<(), KeychainError> {
        let service = Self::service_name(vendor);
        match security_framework::passwords::delete_generic_password(&service, ACCOUNT_NAME) {
            Ok(()) => Ok(()),
            Err(err) if err.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            Err(err) => Err(KeychainError::Store(err.to_string())),
        }
    }
}
