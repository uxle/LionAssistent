//! Device pairing and the paired-device store (Development_plan
//! Phase 28 "Remote Control").
//!
//! A remote-control server is new attack surface this workspace
//! hasn't had before: unlike every prior phase's tools, which are
//! only reachable from a process already running as the local user,
//! [`crate::server`] listens on a socket other devices on the same
//! network can reach. Everything in this module exists to make sure
//! reaching that socket is not the same as controlling this
//! assistant.
//!
//! ## Threat model and what actually defends against it
//!
//! - **Pairing must be a local, explicit act, never a network one.**
//!   [`PairingManager::begin_pairing`] is a plain function call --
//!   there is no HTTP endpoint that starts pairing. Only *completing*
//!   an already-started pairing is reachable over the network
//!   ([`crate::server`]'s `/pair/complete` route), and only a code
//!   that a local caller already generated can complete it. A remote
//!   attacker who has never touched this machine has nothing to
//!   submit.
//! - **A 6-digit code is low-entropy on its own** (1,000,000
//!   possibilities) **and is defended by three things together, not
//!   one**: [`PAIRING_CODE_TTL`] (5 minutes -- an attacker gets one
//!   short window, not unlimited time), [`MAX_PAIRING_ATTEMPTS`] (5
//!   wrong guesses invalidates the code outright, so the real search
//!   space an attacker can explore per code is 5, not 1,000,000), and
//!   [`crate::server`]'s LAN-only restriction on that route (a
//!   1-in-200,000-per-attempt guess is a very different risk from a
//!   random host on the internet versus a device already inside the
//!   home network -- this module assumes the latter is the realistic
//!   threat, matching PRD SS3's own no-cloud-dependency, local-first
//!   framing of what this assistant is for).
//! - **The paired device token is never stored, only its hash.**
//!   [`DeviceStore`] persists `sha256(token)`, never the token itself
//!   -- the same reasoning a password database hashes rather than
//!   encrypts: nothing needs to recover the original token, only to
//!   check whether a presented one matches, so no code path exists
//!   that *could* leak it even if the database file were read
//!   directly. This is a deliberately different (and simpler) choice
//!   than `security::SecretManager`'s reversible AES-256-GCM
//!   encryption -- that crate's own docs note it exists for secrets
//!   something needs to read *back*, like an API key going out in a
//!   request; a bearer token here is only ever compared, never
//!   reused elsewhere, so a one-way hash is both sufficient and
//!   strictly safer than keeping something reversible around.
//! - **A device token is high-entropy and generated, never chosen.**
//!   `generate_device_token` (private -- see [`DeviceStore::register`])
//!   pulls 32 bytes (256 bits) from the OS CSPRNG -- the same
//!   `getrandom`-backed randomness source
//!   `security::MasterKey::generate` already uses elsewhere in this
//!   workspace, pinned to the identical `=0.2.17` version for the
//!   same edition2024 reason. 256 bits makes guessing a valid token
//!   directly infeasible regardless of how long an attacker has.

use common::{AppError, AppResult};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a pairing code remains valid after it's generated.
pub const PAIRING_CODE_TTL: Duration = Duration::from_secs(5 * 60);

/// How many wrong submissions a single pairing code tolerates before
/// it is invalidated outright, forcing a fresh `begin_pairing` call.
pub const MAX_PAIRING_ATTEMPTS: u32 = 5;

/// Bytes of OS randomness in a device token (256 bits).
const TOKEN_BYTES: usize = 32;

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    to_hex(&hasher.finalize())
}

/// 32 bytes of OS-CSPRNG randomness, hex-encoded (64 characters).
fn generate_device_token() -> String {
    use getrandom::getrandom;
    let mut bytes = [0u8; TOKEN_BYTES];
    // getrandom failing at all means the OS's own randomness source is
    // broken -- not a recoverable condition for anything security
    // sensitive, so this matches security::MasterKey::generate's own
    // choice to treat OS randomness as always available rather than
    // invent a fallback that would be weaker than what it's replacing.
    getrandom(&mut bytes).expect("OS randomness source unavailable");
    to_hex(&bytes)
}

/// A 6-digit pairing code, human-typeable on a phone.
fn generate_pairing_code() -> String {
    use getrandom::getrandom;
    let mut bytes = [0u8; 4];
    getrandom(&mut bytes).expect("OS randomness source unavailable");
    let n = u32::from_le_bytes(bytes) % 1_000_000;
    format!("{n:06}")
}

struct PendingPairing {
    code: String,
    device_label: String,
    created_at: Instant,
    attempts_remaining: u32,
}

/// Tracks in-flight pairing attempts. Deliberately in-memory, not
/// persisted: a pairing code is meaningless after [`PAIRING_CODE_TTL`]
/// regardless, and *not* surviving a restart is a feature here, not a
/// gap -- a code generated in a previous run has no business still
/// being completable after this process (and whatever local UI
/// displayed it) is gone.
pub struct PairingManager {
    pending: Mutex<Option<PendingPairing>>,
}

/// Returned by [`PairingManager::begin_pairing`] for the local caller
/// (a future settings UI, or a test) to display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCode {
    pub code: String,
    pub expires_in: Duration,
}

/// Why [`PairingManager::try_complete`] did not issue a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    /// No pairing is currently in progress -- `begin_pairing` was
    /// never called, or a previous code already resolved (matched,
    /// expired, or exhausted its attempts).
    NoPairingInProgress,
    /// The code has outlived [`PAIRING_CODE_TTL`].
    Expired,
    /// The submitted code did not match, and there are more attempts
    /// left on this pairing.
    WrongCode { attempts_remaining: u32 },
    /// The submitted code did not match, and this was the last
    /// attempt -- the pairing is now invalidated; `begin_pairing` must
    /// be called again.
    TooManyAttempts,
}

impl std::fmt::Display for PairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PairingError::NoPairingInProgress => write!(f, "no pairing is currently in progress"),
            PairingError::Expired => write!(f, "pairing code has expired"),
            PairingError::WrongCode { attempts_remaining } => {
                write!(
                    f,
                    "incorrect code ({attempts_remaining} attempts remaining)"
                )
            }
            PairingError::TooManyAttempts => {
                write!(f, "too many incorrect attempts; pairing code invalidated")
            }
        }
    }
}

impl Default for PairingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PairingManager {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(None),
        }
    }

    /// Starts a new pairing window, replacing any prior one still in
    /// progress (only one pairing is ever pending at a time -- a
    /// second `begin_pairing` call is how a local user cancels a
    /// stale code, no separate cancel method needed).
    pub fn begin_pairing(&self, device_label: &str) -> PairingCode {
        let code = generate_pairing_code();
        let mut guard = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        *guard = Some(PendingPairing {
            code: code.clone(),
            device_label: device_label.to_string(),
            created_at: Instant::now(),
            attempts_remaining: MAX_PAIRING_ATTEMPTS,
        });
        PairingCode {
            code,
            expires_in: PAIRING_CODE_TTL,
        }
    }

    /// Checks `submitted_code` against the in-progress pairing.
    ///
    /// On a match, returns the device label the pairing was started
    /// with and clears the pending pairing (a code is single-use even
    /// on success). On a mismatch, decrements the attempt counter and
    /// returns the specific reason -- callers (`server.rs`) map this
    /// to an HTTP response, never leaking *why* to more precision than
    /// a human retrying on their own phone would already know.
    pub fn try_complete(&self, submitted_code: &str) -> Result<String, PairingError> {
        let mut guard = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        let Some(pending) = guard.as_mut() else {
            return Err(PairingError::NoPairingInProgress);
        };

        if pending.created_at.elapsed() > PAIRING_CODE_TTL {
            *guard = None;
            return Err(PairingError::Expired);
        }

        // Constant-time comparison: this is a 6-digit code, not a
        // cryptographic secret, so a timing side-channel on the digits
        // themselves leaks little that guessing doesn't already -- but
        // comparing this way costs nothing and removes the question.
        let matches = constant_time_eq(pending.code.as_bytes(), submitted_code.as_bytes());

        if matches {
            let label = pending.device_label.clone();
            *guard = None;
            return Ok(label);
        }

        pending.attempts_remaining -= 1;
        if pending.attempts_remaining == 0 {
            *guard = None;
            return Err(PairingError::TooManyAttempts);
        }
        Err(PairingError::WrongCode {
            attempts_remaining: pending.attempts_remaining,
        })
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// A paired device's record, as read back from [`DeviceStore`]. The
/// token itself is never part of this type -- see the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub device_id: String,
    pub label: String,
    pub paired_at: u64,
    pub last_seen_at: Option<u64>,
}

/// SQLite-backed store of paired devices, one row per device, keyed
/// for lookup by `token_hash`. Mirrors `memory::MemoryStore`'s and
/// `rag::KnowledgeBase`'s own open/in_memory shape.
pub struct DeviceStore {
    conn: Mutex<Connection>,
}

impl DeviceStore {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        Self::from_connection(Connection::open(path).map_err(db_err)?)
    }

    pub fn in_memory() -> AppResult<Self> {
        Self::from_connection(Connection::open_in_memory().map_err(db_err)?)
    }

    fn from_connection(conn: Connection) -> AppResult<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS paired_devices (
                device_id   TEXT PRIMARY KEY,
                label       TEXT NOT NULL,
                token_hash  TEXT NOT NULL UNIQUE,
                paired_at   INTEGER NOT NULL,
                last_seen_at INTEGER
             );
             CREATE INDEX IF NOT EXISTS idx_paired_devices_token_hash
                ON paired_devices(token_hash);",
        )
        .map_err(db_err)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Registers a newly-paired device and returns its bearer token.
    /// The token is returned exactly once here -- it is not
    /// recoverable afterward, only re-issuable via a fresh pairing
    /// (see the module docs for why only the hash is kept).
    pub fn register(&self, label: &str) -> AppResult<(String, String)> {
        let device_id = generate_device_token(); // reused as an opaque id generator; same entropy source, different purpose
        let token = generate_device_token();
        let token_hash = sha256_hex(token.as_bytes());
        let now = now_unix();

        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO paired_devices (device_id, label, token_hash, paired_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, NULL)",
            rusqlite::params![device_id, label, token_hash, now],
        )
        .map_err(db_err)?;

        Ok((device_id, token))
    }

    /// Looks up the device a presented bearer token belongs to, and
    /// records this as its most recent activity. `None` covers both
    /// "never paired" and "revoked" identically -- from the caller's
    /// side those are indistinguishable, and should be: a revoked
    /// token must fail exactly like one that never existed.
    pub fn authenticate(&self, presented_token: &str) -> AppResult<Option<Device>> {
        let token_hash = sha256_hex(presented_token.as_bytes());
        let now = now_unix();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let found = conn
            .query_row(
                "SELECT device_id, label, paired_at, last_seen_at
                 FROM paired_devices WHERE token_hash = ?1",
                [&token_hash],
                |row| {
                    Ok(Device {
                        device_id: row.get(0)?,
                        label: row.get(1)?,
                        paired_at: row.get(2)?,
                        last_seen_at: row.get(3)?,
                    })
                },
            )
            .ok();

        if let Some(device) = &found {
            conn.execute(
                "UPDATE paired_devices SET last_seen_at = ?1 WHERE device_id = ?2",
                rusqlite::params![now, device.device_id],
            )
            .map_err(db_err)?;
        }

        Ok(found)
    }

    pub fn list(&self) -> AppResult<Vec<Device>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT device_id, label, paired_at, last_seen_at
                 FROM paired_devices ORDER BY paired_at",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Device {
                    device_id: row.get(0)?,
                    label: row.get(1)?,
                    paired_at: row.get(2)?,
                    last_seen_at: row.get(3)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    /// Revokes a device by id. Returns `false` (not an error) for an
    /// unknown id, the same as `MemoryStore::forget`'s precedent.
    pub fn revoke(&self, device_id: &str) -> AppResult<bool> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let affected = conn
            .execute(
                "DELETE FROM paired_devices WHERE device_id = ?1",
                [device_id],
            )
            .map_err(db_err)?;
        Ok(affected > 0)
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn db_err(e: rusqlite::Error) -> AppError {
    AppError::Module {
        module: "remote-control",
        message: format!("sqlite error: {e}"),
    }
}

/// A tiny in-process rate limiter for the pairing-*completion*
/// endpoint specifically (separate from [`MAX_PAIRING_ATTEMPTS`],
/// which limits guesses against one code -- this limits how often
/// `/pair/complete` can be hit at all, regardless of which code is
/// active, so an attacker can't outrun a code's own attempt limit by
/// just waiting for the next one to be generated and immediately
/// spending all 5 attempts on it too). Not persisted -- a process
/// restart resetting this is an acceptable, low-stakes trade-off for
/// something this lightweight.
pub struct RateLimiter {
    window: Duration,
    max_per_window: u32,
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new(window: Duration, max_per_window: u32) -> Self {
        Self {
            window,
            max_per_window,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// `true` if `key` (typically a remote IP) is still under its
    /// limit for the current window, recording this call as a hit
    /// either way -- a caller that is over the limit still "uses up"
    /// nothing further, but the check itself is cheap enough not to
    /// matter.
    pub fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut guard = self.hits.lock().unwrap_or_else(|p| p.into_inner());
        let entry = guard.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < self.window);
        if entry.len() as u32 >= self.max_per_window {
            return false;
        }
        entry.push(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_code_completes_pairing_and_returns_the_label() {
        let manager = PairingManager::new();
        let code = manager.begin_pairing("Lion's Phone");
        let label = manager.try_complete(&code.code).unwrap();
        assert_eq!(label, "Lion's Phone");
    }

    #[test]
    fn wrong_code_decrements_attempts_and_does_not_complete() {
        let manager = PairingManager::new();
        let code = manager.begin_pairing("Phone");
        let wrong = if code.code == "000000" {
            "000001"
        } else {
            "000000"
        };
        let err = manager.try_complete(wrong).unwrap_err();
        assert_eq!(
            err,
            PairingError::WrongCode {
                attempts_remaining: 4
            }
        );
    }

    #[test]
    fn five_wrong_attempts_invalidates_the_code_even_for_the_right_one_after() {
        let manager = PairingManager::new();
        let code = manager.begin_pairing("Phone");
        let wrong = if code.code == "000000" {
            "000001"
        } else {
            "000000"
        };
        for _ in 0..4 {
            assert!(matches!(
                manager.try_complete(wrong),
                Err(PairingError::WrongCode { .. })
            ));
        }
        // 5th wrong attempt exhausts it.
        assert_eq!(
            manager.try_complete(wrong).unwrap_err(),
            PairingError::TooManyAttempts
        );
        // Even the *correct* code no longer works: pairing was invalidated.
        assert_eq!(
            manager.try_complete(&code.code).unwrap_err(),
            PairingError::NoPairingInProgress
        );
    }

    #[test]
    fn completing_with_no_pairing_in_progress_is_a_clear_error() {
        let manager = PairingManager::new();
        assert_eq!(
            manager.try_complete("123456").unwrap_err(),
            PairingError::NoPairingInProgress
        );
    }

    #[test]
    fn a_code_is_single_use_even_when_correct() {
        let manager = PairingManager::new();
        let code = manager.begin_pairing("Phone");
        manager.try_complete(&code.code).unwrap();
        assert_eq!(
            manager.try_complete(&code.code).unwrap_err(),
            PairingError::NoPairingInProgress
        );
    }

    #[test]
    fn starting_a_new_pairing_invalidates_any_prior_pending_code() {
        let manager = PairingManager::new();
        let first = manager.begin_pairing("Phone A");
        let _second = manager.begin_pairing("Phone B");
        // The first code no longer completes anything, even correctly.
        let err = manager.try_complete(&first.code);
        assert!(err.is_err());
    }

    #[test]
    fn generated_pairing_codes_are_always_six_digits() {
        for _ in 0..50 {
            let code = generate_pairing_code();
            assert_eq!(code.len(), 6, "code {code:?} was not 6 digits");
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn device_tokens_are_high_entropy_and_never_repeat_in_a_reasonable_sample() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let t = generate_device_token();
            assert_eq!(
                t.len(),
                TOKEN_BYTES * 2,
                "expected 64 hex chars for 32 bytes"
            );
            assert!(seen.insert(t), "device token collided across 200 samples");
        }
    }

    #[test]
    fn constant_time_eq_matches_regular_equality_semantics() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }

    // --- DeviceStore ---

    #[test]
    fn register_then_authenticate_round_trips() {
        let store = DeviceStore::in_memory().unwrap();
        let (device_id, token) = store.register("Lion's Phone").unwrap();
        let device = store.authenticate(&token).unwrap().unwrap();
        assert_eq!(device.device_id, device_id);
        assert_eq!(device.label, "Lion's Phone");
    }

    #[test]
    fn wrong_token_does_not_authenticate() {
        let store = DeviceStore::in_memory().unwrap();
        store.register("Phone").unwrap();
        assert!(store.authenticate("not-a-real-token").unwrap().is_none());
    }

    #[test]
    fn revoked_device_token_stops_authenticating_immediately() {
        let store = DeviceStore::in_memory().unwrap();
        let (device_id, token) = store.register("Phone").unwrap();
        assert!(store.authenticate(&token).unwrap().is_some());
        assert!(store.revoke(&device_id).unwrap());
        assert!(
            store.authenticate(&token).unwrap().is_none(),
            "a revoked token must fail exactly like one that never existed"
        );
    }

    #[test]
    fn revoking_an_unknown_device_id_returns_false_not_an_error() {
        let store = DeviceStore::in_memory().unwrap();
        assert!(!store.revoke("never-existed").unwrap());
    }

    #[test]
    fn the_raw_token_is_never_present_in_the_stored_row() {
        let store = DeviceStore::in_memory().unwrap();
        let (_id, token) = store.register("Phone").unwrap();
        let conn = store.conn.lock().unwrap();
        let stored_hash: String = conn
            .query_row("SELECT token_hash FROM paired_devices LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_ne!(
            stored_hash, token,
            "the raw token must never be what's stored"
        );
        assert_eq!(stored_hash, sha256_hex(token.as_bytes()));
    }

    #[test]
    fn authenticate_updates_last_seen_at() {
        let store = DeviceStore::in_memory().unwrap();
        let (_id, token) = store.register("Phone").unwrap();
        let before = store.list().unwrap()[0].last_seen_at;
        assert!(before.is_none(), "never authenticated yet");
        store.authenticate(&token).unwrap();
        let after = store.list().unwrap()[0].last_seen_at;
        assert!(after.is_some());
    }

    #[test]
    fn list_returns_every_paired_device_in_pairing_order() {
        let store = DeviceStore::in_memory().unwrap();
        store.register("First").unwrap();
        store.register("Second").unwrap();
        let devices = store.list().unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].label, "First");
        assert_eq!(devices[1].label, "Second");
    }

    #[test]
    fn persistence_across_reopening_the_same_file() {
        let dir = std::env::temp_dir().join(format!(
            "remote-control-test-{}-{:?}",
            std::process::id(),
            Instant::now()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("devices.db");

        let token = {
            let store = DeviceStore::open(&path).unwrap();
            let (_id, token) = store.register("Persistent Phone").unwrap();
            token
        };
        {
            let store = DeviceStore::open(&path).unwrap();
            let device = store.authenticate(&token).unwrap().unwrap();
            assert_eq!(device.label, "Persistent Phone");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    // --- RateLimiter ---

    #[test]
    fn rate_limiter_allows_up_to_the_configured_maximum() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 3);
        assert!(limiter.check("1.2.3.4"));
        assert!(limiter.check("1.2.3.4"));
        assert!(limiter.check("1.2.3.4"));
        assert!(
            !limiter.check("1.2.3.4"),
            "4th hit within the window must be refused"
        );
    }

    #[test]
    fn rate_limiter_tracks_keys_independently() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 1);
        assert!(limiter.check("device-a"));
        assert!(
            limiter.check("device-b"),
            "a different key must have its own budget"
        );
        assert!(!limiter.check("device-a"));
    }
}
