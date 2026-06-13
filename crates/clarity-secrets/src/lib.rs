#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)
)]
//! Encrypted secret storage for Clarity.
//!
//! Uses ChaCha20-Poly1305 (XChaCha20-Poly1305 would be even safer for random nonces,
//! but ChaCha20-Poly1305 with random 96-bit nonces is sufficient for this use case).
//!
//! Ciphertext format: `enc2:<hex(nonce || ciphertext_and_tag)>`
//! Legacy plaintext is passed through unchanged (for migration).

use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use std::fs;
#[cfg(unix)]
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{info, warn};

/// Size of the ChaCha20-Poly1305 key in bytes.
pub const KEY_SIZE: usize = 32;
/// Size of the ChaCha20-Poly1305 nonce in bytes.
pub const NONCE_SIZE: usize = 12;
/// Size of the ChaCha20-Poly1305 authentication tag in bytes.
pub const TAG_SIZE: usize = 16;
/// Prefix identifying a value encrypted with this store.
pub const CIPHER_PREFIX: &str = "enc2:";

/// Errors returned by the secret store.
#[derive(Debug, Error)]
pub enum SecretError {
    /// An IO operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Hex decoding failed.
    #[error("Hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),
    /// Decryption failed because the ciphertext was invalid or tampered with.
    #[error("Decryption failed (invalid ciphertext or tampered data)")]
    DecryptionFailed,
    /// The ciphertext format was invalid.
    #[error("Invalid ciphertext format")]
    InvalidFormat,
    /// The key file could not be found.
    #[error("Key file not found")]
    KeyNotFound,
}

/// Encrypted secret store backed by a single key file.
#[derive(Debug, Clone)]
pub struct SecretStore {
    key: Key,
    key_path: PathBuf,
}

impl SecretStore {
    /// Load an existing key file, or create a new random key if it does not exist.
    pub fn load_or_create(key_path: impl AsRef<Path>) -> Result<Self, SecretError> {
        let key_path = key_path.as_ref().to_path_buf();
        if key_path.exists() {
            Self::load(&key_path)
        } else {
            let store = Self::generate(&key_path)?;
            store.save_key()?;
            info!("Created new secret key at {}", key_path.display());
            Ok(store)
        }
    }

    /// Load a key from an existing file (hex-encoded 32-byte key).
    pub fn load(key_path: impl AsRef<Path>) -> Result<Self, SecretError> {
        let key_path = key_path.as_ref().to_path_buf();
        let hex_key = fs::read_to_string(&key_path)?;
        let hex_key = hex_key.trim();
        if hex_key.len() != KEY_SIZE * 2 {
            return Err(SecretError::InvalidFormat);
        }
        let key_bytes = hex::decode(hex_key)?;
        let key = Key::from_slice(&key_bytes);
        Ok(Self {
            key: *key,
            key_path,
        })
    }

    /// Generate a new random key without persisting it.
    pub fn generate(key_path: impl AsRef<Path>) -> Result<Self, SecretError> {
        let key_path = key_path.as_ref().to_path_buf();
        let key = ChaCha20Poly1305::generate_key(&mut OsRng);
        Ok(Self { key, key_path })
    }

    /// Persist the key to disk (hex-encoded).
    ///
    /// On Unix the file is created with 0o600 permissions. On Windows a best-effort
    /// attempt is made to mark the file hidden, but stable Rust does not currently
    /// expose an API for setting file attributes, so the key may be stored as a
    /// regular file.
    fn save_key(&self) -> Result<(), SecretError> {
        if let Some(parent) = self.key_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let hex_key = hex::encode(self.key.as_slice());
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.key_path)?;
            file.write_all(hex_key.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&self.key_path, hex_key)?;
        }
        #[cfg(windows)]
        {
            // Best-effort: mark the key file hidden on Windows.
            if let Err(e) = set_hidden(&self.key_path) {
                warn!("Failed to mark key file as hidden: {}", e);
            }
        }
        Ok(())
    }

    /// Encrypt a plaintext string and return `enc2:<hex(nonce || ciphertext_and_tag)>`.
    pub fn encrypt(&self, plaintext: &str) -> Result<String, SecretError> {
        let cipher = ChaCha20Poly1305::new(&self.key);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| SecretError::DecryptionFailed)?;
        let mut blob = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        Ok(format!("{}{}", CIPHER_PREFIX, hex::encode(blob)))
    }

    /// Decrypt a ciphertext string.
    ///
    /// - If it starts with `enc2:`, decrypt it.
    /// - Otherwise return the input unchanged (legacy plaintext migration).
    pub fn decrypt(&self, ciphertext: &str) -> Result<String, SecretError> {
        if !ciphertext.starts_with(CIPHER_PREFIX) {
            // Legacy plaintext or already-decrypted value.
            return Ok(ciphertext.to_string());
        }
        let blob = hex::decode(&ciphertext[CIPHER_PREFIX.len()..])?;
        if blob.len() < NONCE_SIZE + TAG_SIZE {
            return Err(SecretError::InvalidFormat);
        }
        let (nonce_bytes, ct_bytes) = blob.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = ChaCha20Poly1305::new(&self.key);
        let plaintext = cipher
            .decrypt(nonce, ct_bytes)
            .map_err(|_| SecretError::DecryptionFailed)?;
        String::from_utf8(plaintext).map_err(|_| SecretError::DecryptionFailed)
    }

    /// Return true if the value looks like an encrypted ciphertext.
    pub fn is_encrypted(value: &str) -> bool {
        value.starts_with(CIPHER_PREFIX)
    }

    /// Mask a secret value for UI round-trips.
    ///
    /// - If encrypted: returns `enc2:****`.
    /// - If plaintext and longer than 8 chars: returns `abcd****wxyz`.
    /// - Otherwise: returns `****`.
    pub fn mask(value: &str) -> String {
        if value.starts_with(CIPHER_PREFIX) {
            return format!("{}****", CIPHER_PREFIX);
        }
        if value.len() <= 8 {
            "****".to_string()
        } else {
            format!("{}****{}", &value[..4], &value[value.len() - 4..])
        }
    }

    /// Path to the key file.
    pub fn key_path(&self) -> &Path {
        &self.key_path
    }
}

#[cfg(windows)]
fn set_hidden(path: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    let metadata = fs::metadata(path)?;
    let mut file_attribute = metadata.file_attributes();
    file_attribute |= 0x02; // FILE_ATTRIBUTE_HIDDEN
    // There is no stable Rust API to set file attributes; log and skip.
    let _ = file_attribute;
    warn!("Windows hidden attribute not set via stable API; key stored as plain file");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SecretStore::load_or_create(tmp.path().join("key")).unwrap();
        let plaintext = "sk-7bbc3410fa1c4bfb85165eb90c81a7b2";
        let encrypted = store.encrypt(plaintext).unwrap();
        assert!(encrypted.starts_with(CIPHER_PREFIX));
        assert_ne!(encrypted, plaintext);
        let decrypted = store.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_plaintext() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SecretStore::load_or_create(tmp.path().join("key")).unwrap();
        let plaintext = "plain-api-key";
        let decrypted = store.decrypt(plaintext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_mask() {
        assert_eq!(SecretStore::mask("enc2:deadbeef"), "enc2:****");
        assert_eq!(SecretStore::mask("short"), "****");
        assert_eq!(
            SecretStore::mask("sk-7bbc3410fa1c4bfb85165eb90c81a7b2"),
            "sk-7****a7b2"
        );
    }

    #[test]
    fn test_load_existing_key() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("key");
        let store1 = SecretStore::load_or_create(&path).unwrap();
        let encrypted = store1.encrypt("secret").unwrap();
        let store2 = SecretStore::load(&path).unwrap();
        assert_eq!(store2.decrypt(&encrypted).unwrap(), "secret");
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SecretStore::load_or_create(tmp.path().join("key")).unwrap();
        let mut encrypted = store.encrypt("secret").unwrap();
        // Flip a hex char near the end.
        let last_char = encrypted.pop().unwrap();
        let flipped = if last_char == 'a' { 'b' } else { 'a' };
        encrypted.push(flipped);
        assert!(matches!(
            store.decrypt(&encrypted),
            Err(SecretError::DecryptionFailed | SecretError::Hex(_))
        ));
    }
}
