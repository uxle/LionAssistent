//! "Secret Manager" (Development_plan Phase 16): the API something like
//! a future cloud-feature integration or plugin credential store would
//! actually call -- [`crate::MasterKey`] and [`crate::SecureStore`]
//! underneath, encryption happening transparently on the way in and out.
//!
//! Deliberately not exposed as an assistant tool anywhere in this
//! workspace. TCLIS SS4's tool categories are all about local system
//! control that's safe to describe in a tool result; a value stored here
//! (an API key, by design) becoming visible to the LLM as a tool result
//! is close to the opposite of the point of storing it encrypted in the
//! first place -- the model could echo it back, and from there it's in
//! logs, conversation history, anywhere the model's output goes. Even
//! `list_names` alone is deliberately not wired up here: unlike
//! `network`'s relationship to `updater` (Phase 14, one narrow concrete
//! consumer), nothing in this workspace yet has a concrete, scoped
//! reason to read secret *names* through a tool call. When something
//! does (a plugin asking for its own previously-granted credential,
//! say), that's a narrow, purpose-built tool for *that*, not a general
//! one added speculatively here.

use crate::cipher::{decrypt, encrypt};
use crate::key::MasterKey;
use crate::store::SecureStore;
use common::AppResult;

pub struct SecretManager {
    key: MasterKey,
    store: SecureStore,
}

impl SecretManager {
    pub fn new(key: MasterKey, store: SecureStore) -> Self {
        Self { key, store }
    }

    pub fn set(&self, name: &str, value: &str) -> AppResult<()> {
        let blob = encrypt(&self.key, value.as_bytes())?;
        self.store.put(name, &blob)
    }

    pub fn get(&self, name: &str) -> AppResult<Option<String>> {
        let Some(blob) = self.store.get(name)? else {
            return Ok(None);
        };
        let plaintext = decrypt(&self.key, &blob)?;
        String::from_utf8(plaintext)
            .map(Some)
            .map_err(|e| common::AppError::Module {
                module: "security",
                message: format!("decrypted secret {name:?} was not valid UTF-8: {e}"),
            })
    }

    pub fn delete(&self, name: &str) -> AppResult<bool> {
        self.store.delete(name)
    }

    pub fn list_names(&self) -> AppResult<Vec<String>> {
        self.store.list_names()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> SecretManager {
        SecretManager::new(MasterKey::generate(), SecureStore::in_memory().unwrap())
    }

    #[test]
    fn set_then_get_round_trips_the_plaintext_value() {
        let manager = manager();
        manager.set("openai_api_key", "sk-not-a-real-key").unwrap();
        assert_eq!(
            manager.get("openai_api_key").unwrap().as_deref(),
            Some("sk-not-a-real-key")
        );
    }

    #[test]
    fn get_of_an_unknown_name_is_none() {
        let manager = manager();
        assert!(manager.get("never_set").unwrap().is_none());
    }

    #[test]
    fn what_is_actually_stored_is_never_the_plaintext() {
        // A regression guard against ever "simplifying" this into
        // storing values unencrypted -- the raw store must not contain
        // the secret in the clear.
        let manager = manager();
        manager.set("api_key", "sk-super-secret-value").unwrap();

        let raw = manager.store.get("api_key").unwrap().unwrap();
        assert!(!raw
            .ciphertext
            .windows(b"secret".len())
            .any(|w| w == b"secret"));
    }

    #[test]
    fn delete_removes_a_secret() {
        let manager = manager();
        manager.set("api_key", "value").unwrap();
        assert!(manager.delete("api_key").unwrap());
        assert!(manager.get("api_key").unwrap().is_none());
    }

    #[test]
    fn list_names_never_exposes_values() {
        let manager = manager();
        manager.set("openai_key", "sk-secret-one").unwrap();
        manager.set("github_token", "ghp-secret-two").unwrap();

        let names = manager.list_names().unwrap();
        assert_eq!(names, vec!["github_token", "openai_key"]);
        for name in &names {
            assert!(!name.contains("secret"));
        }
    }

    #[test]
    fn a_manager_with_the_wrong_key_cannot_read_values_set_by_another() {
        let store = SecureStore::in_memory().unwrap();
        let writer = SecretManager::new(MasterKey::generate(), store);
        writer.set("api_key", "sk-secret").unwrap();

        // Simulate a second process opening the same store with a
        // different (wrong) key -- reuse the same underlying rows by
        // reading them back into a fresh in-memory store isn't possible
        // across two `SecureStore`s directly, so this constructs the
        // realistic failure mode instead: the same encrypted blob, a
        // different key.
        let blob = writer.store.get("api_key").unwrap().unwrap();
        let wrong_key = MasterKey::generate();
        assert!(crate::cipher::decrypt(&wrong_key, &blob).is_err());
    }
}
