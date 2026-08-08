//! [`MasterKey`] -- a 256-bit key for [`crate::cipher`]'s AES-256-GCM,
//! obtainable three ways depending on what the caller actually needs:
//!
//! - [`MasterKey::generate`] -- a fresh random key.
//! - [`MasterKey::from_passphrase`] -- derived from something a user
//!   types, via Argon2 (never stored anywhere -- must be re-derived each
//!   time from the same passphrase and salt).
//! - [`MasterKey::load_or_create`] -- a key stored in a single file with
//!   `0600` permissions, generated once and reused after. This is the
//!   practical default for a headless/background service with no one to
//!   prompt for a passphrase, and it's worth being completely honest
//!   about what it does and doesn't protect against: it stops casual or
//!   accidental exposure (browsing files on a shared machine, a stray
//!   backup, a misconfigured file share) because the key and the
//!   ciphertext aren't the same thing an attacker sees at a glance --
//!   but it does **not** protect against a determined attacker with the
//!   same OS-user's filesystem access, since the key sits right next to
//!   what it protects. That's not a bug to fix later; it's the honest
//!   ceiling of *any* local key-in-a-file scheme without a hardware
//!   keystore or OS keyring behind it. DBDMS SS10 already scopes
//!   encryption this way ("only if the user explicitly chooses to store
//!   them"); real defense against a compromised account needs an OS
//!   keyring (`libsecret`/Secret Service on Linux) or a passphrase this
//!   crate never sees a byte of. Both are real, sensible extensions --
//!   see docs/ROADMAP.md's Phase 16 notes for why neither is built here
//!   (one needs a session bus this sandbox doesn't have to verify
//!   against; the other needs a UI that doesn't exist yet, Phase 2).

use argon2::Argon2;
use common::{AppError, AppResult};
use std::path::Path;
use zeroize::ZeroizeOnDrop;

pub const KEY_LEN: usize = 32;

#[derive(ZeroizeOnDrop)]
pub struct MasterKey([u8; KEY_LEN]);

impl MasterKey {
    /// A fresh, cryptographically random key.
    pub fn generate() -> Self {
        use aes_gcm::aead::{rand_core::RngCore, OsRng};
        let mut bytes = [0u8; KEY_LEN];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Derives a key from `passphrase` and `salt` via Argon2 (the
    /// Password Hashing Competition winner, not a hand-rolled KDF --
    /// DIG SS11 "avoid introducing unrelated dependencies" doesn't apply
    /// to using a proven library for exactly the thing it exists for).
    /// The same passphrase and salt always derive the same key; neither
    /// is stored by this function -- the caller decides where the salt
    /// lives (it isn't secret, unlike the passphrase, and needs to be
    /// available again next time to re-derive the same key).
    pub fn from_passphrase(passphrase: &str, salt: &[u8]) -> AppResult<Self> {
        let mut bytes = [0u8; KEY_LEN];
        Argon2::default()
            .hash_password_into(passphrase.as_bytes(), salt, &mut bytes)
            .map_err(|e| AppError::Module {
                module: "security",
                message: format!("key derivation failed: {e}"),
            })?;
        Ok(Self(bytes))
    }

    /// Reads a key from `path` if it already exists, or generates one
    /// and writes it there (creating parent directories and setting
    /// `0600` permissions) if not. See this module's doc comment for
    /// what this scheme does and doesn't protect against.
    pub fn load_or_create(path: &Path) -> AppResult<Self> {
        if path.exists() {
            let bytes = std::fs::read(path)?;
            let array: [u8; KEY_LEN] = bytes.try_into().map_err(|_| AppError::Module {
                module: "security",
                message: format!("key file at {} is not {KEY_LEN} bytes", path.display()),
            })?;
            return Ok(Self(array));
        }

        let key = Self::generate();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, key.0)?;
        Self::restrict_permissions(path)?;
        Ok(key)
    }

    #[cfg(unix)]
    fn restrict_permissions(path: &Path) -> AppResult<()> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(())
    }

    pub(crate) fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

/// Deliberately redacted: the whole point of this type is that its
/// bytes never appear anywhere they don't strictly need to, and a
/// forgotten `{:?}` in a log statement is a completely ordinary way for
/// that promise to get broken by accident.
impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterKey(REDACTED)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_different_keys_each_time() {
        let a = MasterKey::generate();
        let b = MasterKey::generate();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn from_passphrase_is_deterministic_for_the_same_inputs() {
        let salt = b"a fixed test salt";
        let a = MasterKey::from_passphrase("correct horse battery staple", salt).unwrap();
        let b = MasterKey::from_passphrase("correct horse battery staple", salt).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn from_passphrase_differs_for_different_passphrases() {
        let salt = b"a fixed test salt";
        let a = MasterKey::from_passphrase("passphrase one", salt).unwrap();
        let b = MasterKey::from_passphrase("passphrase two", salt).unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn from_passphrase_differs_for_different_salts() {
        let a = MasterKey::from_passphrase("same passphrase", b"salt one").unwrap();
        let b = MasterKey::from_passphrase("same passphrase", b"salt two").unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn load_or_create_persists_the_same_key_across_calls() {
        let dir = std::env::temp_dir().join("security-key-test-persist");
        std::fs::remove_dir_all(&dir).ok();
        let path = dir.join("master.key");

        let first = MasterKey::load_or_create(&path).unwrap();
        let second = MasterKey::load_or_create(&path).unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_or_create_writes_a_file_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join("security-key-test-perms");
        std::fs::remove_dir_all(&dir).ok();
        let path = dir.join("master.key");

        MasterKey::load_or_create(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn debug_never_prints_the_actual_key_material() {
        // The exact-match assertion is the whole test: `Debug` must
        // produce this fixed, redacted string and nothing derived from
        // the real bytes, for every key, every time.
        let key = MasterKey::generate();
        assert_eq!(format!("{key:?}"), "MasterKey(REDACTED)");
    }
}
