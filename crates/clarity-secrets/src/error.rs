//! Error types for the secret store.

use thiserror::Error;

/// Errors returned by the secret store.
#[derive(Debug, Error)]
pub enum SecretError {
    /// An IO operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Hex decoding failed.
    #[error("Hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),
    /// Encryption failed because the AEAD operation could not complete.
    #[error("Encryption failed")]
    EncryptionFailed,
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
