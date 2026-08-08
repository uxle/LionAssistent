//! AES-256-GCM authenticated encryption over a [`crate::MasterKey`] --
//! "Encryption" (Development_plan Phase 16). AES-GCM (not a hand-rolled
//! scheme) via the RustCrypto project's `aes-gcm`: authenticated, so
//! tampering with an [`EncryptedBlob`] is detected on decrypt rather
//! than silently producing corrupted plaintext.

use crate::key::MasterKey;
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use common::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// A ciphertext and the (not secret, but never reused) nonce it was
/// encrypted with. Both are needed to decrypt -- storing them together
/// is normal practice for AEAD schemes; the nonce provides no
/// confidentiality on its own.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBlob {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub fn encrypt(key: &MasterKey, plaintext: &[u8]) -> AppResult<EncryptedBlob> {
    let cipher = cipher_for(key)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| AppError::Module {
            module: "security",
            message: format!("encryption failed: {e}"),
        })?;
    Ok(EncryptedBlob {
        nonce: nonce.to_vec(),
        ciphertext,
    })
}

/// Fails (rather than returning garbage) if `key` is wrong or `blob` was
/// tampered with -- that's AES-GCM's authentication tag doing its job,
/// not an edge case this function works around.
pub fn decrypt(key: &MasterKey, blob: &EncryptedBlob) -> AppResult<Vec<u8>> {
    let cipher = cipher_for(key)?;
    let nonce = Nonce::from_slice(&blob.nonce);
    cipher
        .decrypt(nonce, blob.ciphertext.as_ref())
        .map_err(|_| AppError::Module {
            module: "security",
            message: "decryption failed: wrong key or corrupted/tampered data".to_string(),
        })
}

fn cipher_for(key: &MasterKey) -> AppResult<Aes256Gcm> {
    Aes256Gcm::new_from_slice(key.as_bytes()).map_err(|e| AppError::Module {
        module: "security",
        message: format!("invalid key: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let key = MasterKey::generate();
        let blob = encrypt(&key, b"top secret api key").unwrap();
        let plaintext = decrypt(&key, &blob).unwrap();
        assert_eq!(plaintext, b"top secret api key");
    }

    #[test]
    fn ciphertext_does_not_contain_the_plaintext() {
        let key = MasterKey::generate();
        let blob = encrypt(&key, b"top secret api key").unwrap();
        assert!(!blob
            .ciphertext
            .windows(b"secret".len())
            .any(|w| w == b"secret"));
    }

    #[test]
    fn decrypting_with_the_wrong_key_fails() {
        let key = MasterKey::generate();
        let wrong_key = MasterKey::generate();
        let blob = encrypt(&key, b"top secret").unwrap();
        assert!(decrypt(&wrong_key, &blob).is_err());
    }

    #[test]
    fn tampering_with_the_ciphertext_is_detected() {
        let key = MasterKey::generate();
        let mut blob = encrypt(&key, b"top secret").unwrap();
        blob.ciphertext[0] ^= 0xFF;
        assert!(decrypt(&key, &blob).is_err());
    }

    #[test]
    fn tampering_with_the_nonce_is_detected() {
        let key = MasterKey::generate();
        let mut blob = encrypt(&key, b"top secret").unwrap();
        blob.nonce[0] ^= 0xFF;
        assert!(decrypt(&key, &blob).is_err());
    }

    #[test]
    fn two_encryptions_of_the_same_plaintext_produce_different_ciphertexts() {
        // A fresh random nonce every call -- confirms nonces aren't
        // accidentally reused, which would be a real confidentiality
        // break for GCM.
        let key = MasterKey::generate();
        let a = encrypt(&key, b"same plaintext").unwrap();
        let b = encrypt(&key, b"same plaintext").unwrap();
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let key = MasterKey::generate();
        let blob = encrypt(&key, b"").unwrap();
        assert_eq!(decrypt(&key, &blob).unwrap(), b"");
    }
}
