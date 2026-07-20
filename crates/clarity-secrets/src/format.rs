//! Ciphertext format constants and helpers.

/// Size of the ChaCha20-Poly1305 key in bytes.
pub const KEY_SIZE: usize = 32;
/// Size of the ChaCha20-Poly1305 nonce in bytes.
pub const NONCE_SIZE: usize = 12;
/// Size of the ChaCha20-Poly1305 authentication tag in bytes.
pub const TAG_SIZE: usize = 16;
/// Prefix identifying a value encrypted with this store.
pub const CIPHER_PREFIX: &str = "enc2:";

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
