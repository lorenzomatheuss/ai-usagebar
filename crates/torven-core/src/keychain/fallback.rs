use std::path::{Path, PathBuf};

use crate::keychain::{
    AccountSecret, KeychainError, SecretStore, decode_accounts_blob, encode_accounts_blob,
};

pub struct FileFallbackStore {
    root: PathBuf,
}

impl FileFallbackStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        tracing::warn!("Using file fallback store — não recomendado para produção");
        Self { root: root.into() }
    }

    pub fn default_root() -> Result<PathBuf, KeychainError> {
        let mut root = dirs::config_dir().ok_or(KeychainError::DefaultPathUnavailable)?;
        root.push("torven");
        root.push("secrets");
        Ok(root)
    }

    pub fn default_store() -> Result<Self, KeychainError> {
        Ok(Self::new(Self::default_root()?))
    }

    pub fn path_for_vendor(&self, vendor: &str) -> PathBuf {
        self.root.join(format!("{vendor}.json"))
    }

    fn ensure_root(&self) -> Result<(), KeychainError> {
        std::fs::create_dir_all(&self.root).map_err(|source| KeychainError::Io {
            path: self.root.clone(),
            source,
        })
    }
}

impl SecretStore for FileFallbackStore {
    fn get_accounts_blob(&self, vendor: &str) -> Result<Option<Vec<AccountSecret>>, KeychainError> {
        let path = self.path_for_vendor(vendor);
        match std::fs::read(&path) {
            Ok(bytes) => decode_accounts_blob(&bytes).map(Some),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(KeychainError::Io { path, source }),
        }
    }

    fn set_accounts_blob(
        &self,
        vendor: &str,
        accounts: &[AccountSecret],
    ) -> Result<(), KeychainError> {
        self.ensure_root()?;
        let path = self.path_for_vendor(vendor);
        write_secret_file(&path, &encode_accounts_blob(accounts)?)
    }

    fn delete_vendor_entry(&self, vendor: &str) -> Result<(), KeychainError> {
        let path = self.path_for_vendor(vendor);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(KeychainError::Io { path, source }),
        }
    }
}

fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), KeychainError> {
    std::fs::write(path, bytes).map_err(|source| KeychainError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(|source| KeychainError::Io {
                path: path.to_path_buf(),
                source,
            })?
            .permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms).map_err(|source| KeychainError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}
