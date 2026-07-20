//! Encrypted secret store backed by a single key file.

use crate::{
    SecretError,
    format::{CIPHER_PREFIX, KEY_SIZE, NONCE_SIZE, TAG_SIZE},
    key_file,
};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use std::fs;
use std::path::{Path, PathBuf};

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
            let store = Self::generate(&key_path);
            store.save()?;
            tracing::info!("Created new secret key at {}", key_path.display());
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
    pub fn generate(key_path: impl AsRef<Path>) -> Self {
        let key_path = key_path.as_ref().to_path_buf();
        let key = ChaCha20Poly1305::generate_key(&mut OsRng);
        Self { key, key_path }
    }

    /// Persist the key to disk (hex-encoded).
    pub fn save(&self) -> Result<(), SecretError> {
        key_file::save_key(&self.key_path, self.key.as_slice())
    }

    /// Encrypt a plaintext string and return `enc2:<hex(nonce || ciphertext_and_tag)>`.
    pub fn encrypt(&self, plaintext: &str) -> Result<String, SecretError> {
        let cipher = ChaCha20Poly1305::new(&self.key);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| SecretError::EncryptionFailed)?;
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
        crate::format::is_encrypted(value)
    }

    /// Mask a secret value for UI round-trips.
    pub fn mask(value: &str) -> String {
        crate::format::mask(value)
    }

    /// Path to the key file.
    pub fn key_path(&self) -> &Path {
        &self.key_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::CIPHER_PREFIX;

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
    fn test_round_trip_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SecretStore::load_or_create(tmp.path().join("key")).unwrap();
        let encrypted = store.encrypt("").unwrap();
        assert!(encrypted.starts_with(CIPHER_PREFIX));
        let decrypted = store.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, "");
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
        assert_eq!(SecretStore::mask("exactly8"), "****");
        assert_eq!(SecretStore::mask("exactly9!"), "exac****ly9!");
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
    fn test_load_wrong_key_length_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("key");
        fs::write(&path, "deadbeef").unwrap();
        assert!(matches!(
            SecretStore::load(&path),
            Err(SecretError::InvalidFormat)
        ));
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

    #[test]
    fn test_invalid_enc2_format_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SecretStore::load_or_create(tmp.path().join("key")).unwrap();
        // Non-hex payload fails at hex decode.
        assert!(matches!(
            store.decrypt("enc2:short"),
            Err(SecretError::Hex(_))
        ));
        assert!(matches!(
            store.decrypt("enc2:nothex!"),
            Err(SecretError::Hex(_))
        ));
        // Valid hex but too short to contain nonce + tag.
        assert!(matches!(
            store.decrypt("enc2:0000"),
            Err(SecretError::InvalidFormat)
        ));
    }
}
