// @file crates/browser-storage/src/site_policy_store.rs
// @description SQLite implementation of the per-site cookie policy exception store.
// @layer storage
// @created meerita <meerita@icloud.com>

use rusqlite::{params, OptionalExtension};

use crate::database::SqliteStorage;
use crate::error::StorageError;
use crate::stores::SitePolicyStore;

impl SitePolicyStore for SqliteStorage {
    fn set_site_policy(
        &self,
        domain: &str,
        policy: &str,
        created_at: i64,
    ) -> Result<(), StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        connection
            .execute(
                "INSERT INTO site_cookie_policies (domain, policy, created_at) \
                 VALUES (?1, ?2, ?3) \
                 ON CONFLICT(domain) DO UPDATE SET \
                   policy = excluded.policy, \
                   created_at = excluded.created_at",
                params![domain, policy, created_at],
            )
            .map(|_| ())
            .map_err(|_| StorageError::QueryFailed)
    }

    fn remove_site_policy(&self, domain: &str) -> Result<(), StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        connection
            .execute(
                "DELETE FROM site_cookie_policies WHERE domain = ?1",
                params![domain],
            )
            .map(|_| ())
            .map_err(|_| StorageError::QueryFailed)
    }

    fn site_policy(&self, domain: &str) -> Result<Option<String>, StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        connection
            .query_row(
                "SELECT policy FROM site_cookie_policies WHERE domain = ?1",
                params![domain],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| StorageError::QueryFailed)
    }

    fn all_site_policies(&self) -> Result<Vec<(String, String)>, StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        let mut statement = connection
            .prepare("SELECT domain, policy FROM site_cookie_policies")
            .map_err(|_| StorageError::QueryFailed)?;
        let policies = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|_| StorageError::QueryFailed)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| StorageError::QueryFailed)?;
        Ok(policies)
    }

    fn clear_site_policies(&self) -> Result<(), StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        connection
            .execute_batch("DELETE FROM site_cookie_policies;")
            .map_err(|_| StorageError::QueryFailed)
    }
}
