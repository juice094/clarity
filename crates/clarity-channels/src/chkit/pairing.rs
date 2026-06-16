//! Gateway/user pairing — first-connect authentication.
//!
//! The guard generates a one-time pairing code; the first user presents it
//! (e.g. via `/bind <code>` in WeChat) and is issued a bearer token. Token
//! hashes are persisted across restarts.

use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Maximum failed pairing attempts before lockout.
const MAX_PAIR_ATTEMPTS: u32 = 5;
/// Lockout duration after too many failed pairing attempts.
const PAIR_LOCKOUT_SECS: u64 = 300; // 5 minutes
/// Maximum number of tracked client entries to bound memory usage.
const MAX_TRACKED_CLIENTS: usize = 10_000;
/// Retention period for failed-attempt entries with no activity.
const FAILED_ATTEMPT_RETENTION_SECS: u64 = 900; // 15 min
/// Minimum interval between full sweeps of the failed-attempt map.
const FAILED_ATTEMPT_SWEEP_INTERVAL_SECS: u64 = 300; // 5 min

/// Why a `generate_pairing_code_if_vacant` call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratePairingCodeError {
    /// A pairing code is already pending; redeem or wait before issuing a new one.
    Pending,
    /// Pairing is disabled on this channel.
    PairingDisabled,
}

/// Per-client failed attempt state with optional absolute lockout deadline.
#[derive(Debug, Clone, Copy)]
struct FailedAttemptState {
    count: u32,
    lockout_until: Option<Instant>,
    last_attempt: Instant,
}

/// Manages pairing state.
///
/// Bearer tokens are stored as SHA-256 hashes to prevent plaintext exposure
/// in persisted files. When a new token is generated, the plaintext is returned
/// to the client once, and only the hash is retained.
#[derive(Debug, Clone)]
pub struct PairingGuard {
    require_pairing: bool,
    pairing_code: Arc<Mutex<Option<String>>>,
    paired_tokens: Arc<Mutex<HashSet<String>>>,
    failed_attempts: Arc<Mutex<(HashMap<String, FailedAttemptState>, Instant)>>,
}

impl PairingGuard {
    /// Create a new pairing guard.
    ///
    /// If `require_pairing` is true and no tokens exist yet, a fresh pairing
    /// code is generated. Existing plaintext tokens are hashed on load.
    pub fn new(require_pairing: bool, existing_tokens: &[String]) -> Self {
        let tokens: HashSet<String> = existing_tokens
            .iter()
            .map(|t| {
                if is_token_hash(t) {
                    t.clone()
                } else {
                    hash_token(t)
                }
            })
            .collect();
        let code = if require_pairing && tokens.is_empty() {
            Some(generate_code())
        } else {
            None
        };
        Self {
            require_pairing,
            pairing_code: Arc::new(Mutex::new(code)),
            paired_tokens: Arc::new(Mutex::new(tokens)),
            failed_attempts: Arc::new(Mutex::new((HashMap::new(), Instant::now()))),
        }
    }

    /// The one-time pairing code (generated only on first startup when no tokens exist).
    pub fn pairing_code(&self) -> Option<String> {
        self.pairing_code.lock().clone()
    }

    /// Whether pairing is required at all.
    pub fn require_pairing(&self) -> bool {
        self.require_pairing
    }

    fn try_pair_blocking(&self, code: &str, client_id: &str) -> Result<Option<String>, u64> {
        let client_id = normalize_client_key(client_id);
        let now = Instant::now();

        {
            let mut guard = self.failed_attempts.lock();
            let (ref mut map, ref mut last_sweep) = *guard;

            if now.duration_since(*last_sweep).as_secs() >= FAILED_ATTEMPT_SWEEP_INTERVAL_SECS {
                prune_failed_attempts(map, now);
                *last_sweep = now;
            }

            if let Some(state) = map.get(&client_id)
                && let Some(until) = state.lockout_until
            {
                if now < until {
                    let remaining = (until - now).as_secs();
                    return Err(remaining.max(1));
                }
                map.remove(&client_id);
            }
        }

        {
            let mut pairing_code = self.pairing_code.lock();
            if let Some(ref expected) = *pairing_code
                && constant_time_eq(code.trim(), expected.trim())
            {
                {
                    let mut guard = self.failed_attempts.lock();
                    guard.0.remove(&client_id);
                }
                let token = generate_token();
                let mut tokens = self.paired_tokens.lock();
                tokens.insert(hash_token(&token));
                *pairing_code = None;
                return Ok(Some(token));
            }
        }

        {
            let mut guard = self.failed_attempts.lock();
            let (ref mut map, _) = *guard;

            if map.len() >= MAX_TRACKED_CLIENTS {
                prune_failed_attempts(map, now);
            }
            if map.len() >= MAX_TRACKED_CLIENTS {
                if let Some(lru_key) = map
                    .iter()
                    .min_by_key(|(_, s)| s.last_attempt)
                    .map(|(k, _)| k.clone())
                {
                    map.remove(&lru_key);
                }
            }

            let entry = map.entry(client_id).or_insert(FailedAttemptState {
                count: 0,
                lockout_until: None,
                last_attempt: now,
            });

            entry.last_attempt = now;
            entry.count += 1;

            if entry.count >= MAX_PAIR_ATTEMPTS {
                entry.lockout_until = Some(now + Duration::from_secs(PAIR_LOCKOUT_SECS));
            }
        }

        Ok(None)
    }

    /// Attempt to pair with the given code. Returns a bearer token on success.
    /// Returns `Err(lockout_seconds)` if locked out due to brute force.
    pub async fn try_pair(&self, code: &str, client_id: &str) -> Result<Option<String>, u64> {
        let this = self.clone();
        let code = code.to_string();
        let client_id = client_id.to_string();
        let handle = tokio::task::spawn_blocking(move || this.try_pair_blocking(&code, &client_id));
        handle.await.map_err(|_| 0u64)?
    }

    /// Check if a bearer token is valid (compares against stored hashes).
    pub fn is_authenticated(&self, token: &str) -> bool {
        if !self.require_pairing {
            return true;
        }
        let hashed = hash_token(token);
        let tokens = self.paired_tokens.lock();
        tokens.contains(&hashed)
    }

    /// Returns true if at least one token is paired.
    pub fn is_paired(&self) -> bool {
        let tokens = self.paired_tokens.lock();
        !tokens.is_empty()
    }

    /// Get all paired token hashes (for persisting).
    pub fn tokens(&self) -> Vec<String> {
        let tokens = self.paired_tokens.lock();
        tokens.iter().cloned().collect()
    }

    /// Revoke a paired token by plaintext.
    pub fn revoke_token(&self, token: &str) -> bool {
        let hashed = hash_token(token);
        self.revoke_token_hash(&hashed)
    }

    /// Revoke a paired token by its SHA-256 hash.
    pub fn revoke_token_hash(&self, token_hash: &str) -> bool {
        let mut tokens = self.paired_tokens.lock();
        tokens.remove(token_hash)
    }

    /// Revoke every paired token.
    pub fn revoke_all_tokens(&self) -> usize {
        let mut tokens = self.paired_tokens.lock();
        let count = tokens.len();
        tokens.clear();
        count
    }

    /// Generate a new pairing code that pairs an additional client.
    pub fn generate_new_pairing_code(&self) -> Option<String> {
        if !self.require_pairing {
            return None;
        }
        let new_code = generate_code();
        *self.pairing_code.lock() = Some(new_code.clone());
        Some(new_code)
    }

    /// Generate a new pairing code only when no code is already pending.
    pub fn generate_pairing_code_if_vacant(&self) -> Result<String, GeneratePairingCodeError> {
        if !self.require_pairing {
            return Err(GeneratePairingCodeError::PairingDisabled);
        }
        let mut slot = self.pairing_code.lock();
        if slot.is_some() {
            return Err(GeneratePairingCodeError::Pending);
        }
        let new_code = generate_code();
        *slot = Some(new_code.clone());
        Ok(new_code)
    }

    /// Get the token hash for a given plaintext token.
    pub fn token_hash(token: &str) -> String {
        hash_token(token)
    }

    /// Check if a token is paired and return its hash.
    pub fn authenticate_and_hash(&self, token: &str) -> Option<String> {
        if self.is_authenticated(token) {
            Some(Self::token_hash(token))
        } else {
            None
        }
    }
}

fn normalize_client_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

fn prune_failed_attempts(map: &mut HashMap<String, FailedAttemptState>, now: Instant) {
    map.retain(|_, state| {
        now.duration_since(state.last_attempt).as_secs() < FAILED_ATTEMPT_RETENTION_SECS
    });
}

fn generate_code() -> String {
    const UPPER_BOUND: u32 = 1_000_000;
    const REJECT_THRESHOLD: u32 = (u32::MAX / UPPER_BOUND) * UPPER_BOUND;

    loop {
        let uuid = uuid::Uuid::new_v4();
        let bytes = uuid.as_bytes();
        let raw = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if raw < REJECT_THRESHOLD {
            return format!("{:06}", raw % UPPER_BOUND);
        }
    }
}

fn generate_token() -> String {
    let bytes: [u8; 32] = rand::random();
    format!("zc_{}", hex::encode(bytes))
}

fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn is_token_hash(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Constant-time string comparison to prevent timing attacks.
#[allow(clippy::needless_bitwise_bool)]
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let len_diff = a.len() ^ b.len();
    let max_len = a.len().max(b.len());
    let mut byte_diff = 0u8;
    for i in 0..max_len {
        let x = *a.get(i).unwrap_or(&0);
        let y = *b.get(i).unwrap_or(&0);
        byte_diff |= x ^ y;
    }
    (len_diff == 0) & (byte_diff == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_guard_generates_code_when_no_tokens() {
        let guard = PairingGuard::new(true, &[]);
        assert!(guard.pairing_code().is_some());
        assert!(!guard.is_paired());
    }

    #[tokio::test]
    async fn new_guard_no_code_when_tokens_exist() {
        let guard = PairingGuard::new(true, &["zc_existing".into()]);
        assert!(guard.pairing_code().is_none());
        assert!(guard.is_paired());
    }

    #[tokio::test]
    async fn try_pair_correct_code() {
        let guard = PairingGuard::new(true, &[]);
        let code = guard.pairing_code().unwrap().to_string();
        let token = guard.try_pair(&code, "test_client").await.unwrap();
        assert!(token.is_some());
        assert!(token.unwrap().starts_with("zc_"));
        assert!(guard.is_paired());
    }

    #[tokio::test]
    async fn pair_then_authenticate() {
        let guard = PairingGuard::new(true, &[]);
        let code = guard.pairing_code().unwrap().to_string();
        let token = guard.try_pair(&code, "test_client").await.unwrap().unwrap();
        assert!(guard.is_authenticated(&token));
        assert!(!guard.is_authenticated("wrong"));
    }

    #[test]
    fn constant_time_eq_same_and_different() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "ab"));
    }

    #[test]
    fn generate_code_is_6_digits() {
        let code = generate_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }
}
