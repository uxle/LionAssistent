//! "Secure Storage" (Development_plan Phase 16): a small SQLite table
//! holding [`crate::EncryptedBlob`]s by name -- the same persistence
//! approach (`rusqlite`, pinned `=0.31.0` for this workspace's rustc
//! floor) every other stateful crate here already uses. This module
//! only stores and retrieves opaque encrypted bytes; it never sees
//! plaintext -- that's [`crate::manager::SecretManager`]'s job, one
//! layer up.

use common::AppResult;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use crate::cipher::EncryptedBlob;

fn db_err(e: rusqlite::Error) -> common::AppError {
    common::AppError::Module {
        module: "security",
        message: e.to_string(),
    }
}

/// Wraps the connection in a `Mutex`, the same fix `permission-manager`
/// and `search::memory_search` both already applied to a plain
/// `rusqlite::Connection` (`Send` but not `Sync` on its own).
pub struct SecureStore {
    conn: Mutex<Connection>,
}

impl SecureStore {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).map_err(db_err)?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory().map_err(db_err)?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn init(conn: &Connection) -> AppResult<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS secrets (
                name TEXT PRIMARY KEY,
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn put(&self, name: &str, blob: &EncryptedBlob) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .expect("secure store connection lock poisoned");
        conn.execute(
            "INSERT INTO secrets (name, nonce, ciphertext, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET nonce = excluded.nonce, ciphertext = excluded.ciphertext",
            rusqlite::params![name, blob.nonce, blob.ciphertext, common::now().to_string()],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn get(&self, name: &str) -> AppResult<Option<EncryptedBlob>> {
        let conn = self
            .conn
            .lock()
            .expect("secure store connection lock poisoned");
        let result = conn.query_row(
            "SELECT nonce, ciphertext FROM secrets WHERE name = ?1",
            [name],
            |row| {
                Ok(EncryptedBlob {
                    nonce: row.get(0)?,
                    ciphertext: row.get(1)?,
                })
            },
        );
        match result {
            Ok(blob) => Ok(Some(blob)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(db_err(e)),
        }
    }

    pub fn delete(&self, name: &str) -> AppResult<bool> {
        let conn = self
            .conn
            .lock()
            .expect("secure store connection lock poisoned");
        let affected = conn
            .execute("DELETE FROM secrets WHERE name = ?1", [name])
            .map_err(db_err)?;
        Ok(affected > 0)
    }

    /// Names only, never anything about the values -- callers that only
    /// need "does a secret named X exist" or "what secrets are saved"
    /// never need decryption access to get that answer.
    pub fn list_names(&self) -> AppResult<Vec<String>> {
        let conn = self
            .conn
            .lock()
            .expect("secure store connection lock poisoned");
        let mut stmt = conn
            .prepare("SELECT name FROM secrets ORDER BY name")
            .map_err(db_err)?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(db_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(tag: u8) -> EncryptedBlob {
        EncryptedBlob {
            nonce: vec![tag; 12],
            ciphertext: vec![tag; 16],
        }
    }

    #[test]
    fn put_then_get_round_trips() {
        let store = SecureStore::in_memory().unwrap();
        store.put("api_key", &blob(1)).unwrap();
        let retrieved = store.get("api_key").unwrap().unwrap();
        assert_eq!(retrieved.nonce, blob(1).nonce);
        assert_eq!(retrieved.ciphertext, blob(1).ciphertext);
    }

    #[test]
    fn get_of_an_unknown_name_is_none_not_an_error() {
        let store = SecureStore::in_memory().unwrap();
        assert!(store.get("never_stored").unwrap().is_none());
    }

    #[test]
    fn put_overwrites_an_existing_name() {
        let store = SecureStore::in_memory().unwrap();
        store.put("api_key", &blob(1)).unwrap();
        store.put("api_key", &blob(2)).unwrap();
        let retrieved = store.get("api_key").unwrap().unwrap();
        assert_eq!(retrieved.nonce, blob(2).nonce);
    }

    #[test]
    fn delete_removes_the_entry_and_reports_it_existed() {
        let store = SecureStore::in_memory().unwrap();
        store.put("api_key", &blob(1)).unwrap();
        assert!(store.delete("api_key").unwrap());
        assert!(store.get("api_key").unwrap().is_none());
    }

    #[test]
    fn delete_of_an_unknown_name_reports_false_not_an_error() {
        let store = SecureStore::in_memory().unwrap();
        assert!(!store.delete("never_stored").unwrap());
    }

    #[test]
    fn list_names_returns_every_name_sorted_never_values() {
        let store = SecureStore::in_memory().unwrap();
        store.put("zebra_key", &blob(1)).unwrap();
        store.put("alpha_key", &blob(2)).unwrap();
        assert_eq!(store.list_names().unwrap(), vec!["alpha_key", "zebra_key"]);
    }

    #[test]
    fn open_on_disk_persists_across_reopening() {
        let dir = std::env::temp_dir().join("security-store-test-persist");
        std::fs::remove_dir_all(&dir).ok();
        let path = dir.join("secrets.db");

        {
            let store = SecureStore::open(&path).unwrap();
            store.put("api_key", &blob(9)).unwrap();
        }
        {
            let store = SecureStore::open(&path).unwrap();
            assert!(store.get("api_key").unwrap().is_some());
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
