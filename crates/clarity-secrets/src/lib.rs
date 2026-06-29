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
//!
//! # Module overview
//!
//! - [`error`] — [`SecretError`].
//! - [`format`] — constants and helpers for the `enc2:` ciphertext format.
//! - `key_file` — key file persistence and filesystem permissions (internal).
//! - [`store`] — [`SecretStore`] implementation.

pub mod error;
pub mod format;
pub(crate) mod key_file;
pub mod store;

pub use error::SecretError;
pub use format::{CIPHER_PREFIX, KEY_SIZE, NONCE_SIZE, TAG_SIZE, is_encrypted, mask};
pub use store::SecretStore;
