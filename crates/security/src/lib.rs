//! `security` -- encryption, secure storage, and a secret manager
//! (Development_plan Phase 16's "Encryption" / "Secure Storage" /
//! "Secret Manager" modules).
//!
//! Read [`key`]'s doc comment before using [`MasterKey::load_or_create`]
//! anywhere real -- it's honest about exactly what a local key-file
//! scheme does and doesn't protect against, which matters more than the
//! encryption algorithm choice for how much trust this actually earns.
//!
//! [`SecretManager`] is deliberately not exposed as an assistant tool
//! anywhere in this workspace -- see its own doc comment for why.

pub mod cipher;
pub mod key;
pub mod manager;
pub mod store;

pub use cipher::{decrypt, encrypt, EncryptedBlob};
pub use key::MasterKey;
pub use manager::SecretManager;
pub use store::SecureStore;
