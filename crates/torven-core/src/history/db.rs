use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::config::HistoryConfig;
use crate::history::HistoryError;
use crate::history::migrations::get_migrations;

pub struct HistoryDb {
    conn: Mutex<Connection>,
    path: Option<PathBuf>,
}

impl HistoryDb {
    pub fn open(path: &Path) -> Result<Self, HistoryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| HistoryError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut conn = Connection::open(path).map_err(|source| HistoryError::OpenFailed {
            path: path.to_path_buf(),
            source,
        })?;
        configure_connection(&conn)?;
        get_migrations()
            .to_latest(&mut conn)
            .map_err(|source| HistoryError::MigrationFailed {
                message: source.to_string(),
            })?;

        Ok(Self {
            conn: Mutex::new(conn),
            path: Some(path.to_path_buf()),
        })
    }

    pub fn open_in_memory() -> Result<Self, HistoryError> {
        let mut conn = Connection::open_in_memory().map_err(HistoryError::Sqlite)?;
        configure_connection(&conn)?;
        get_migrations()
            .to_latest(&mut conn)
            .map_err(|source| HistoryError::MigrationFailed {
                message: source.to_string(),
            })?;

        Ok(Self {
            conn: Mutex::new(conn),
            path: None,
        })
    }

    pub fn open_from_config(config: &HistoryConfig) -> Result<Self, HistoryError> {
        let path = config
            .db_path
            .clone()
            .map(Ok)
            .unwrap_or_else(default_db_path)?;
        Self::open(&path)
    }

    pub(crate) fn connection(&self) -> Result<MutexGuard<'_, Connection>, HistoryError> {
        self.conn
            .lock()
            .map_err(|_| HistoryError::ConnectionPoisoned)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

pub fn default_db_path() -> Result<PathBuf, HistoryError> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(HistoryError::HomeDirUnavailable)?;
    Ok(home
        .join("Library")
        .join("Application Support")
        .join("Torven")
        .join("history.db"))
}

fn configure_connection(conn: &Connection) -> Result<(), HistoryError> {
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;
        PRAGMA synchronous=NORMAL;
        ",
    )
    .map_err(HistoryError::Sqlite)?;
    Ok(())
}
