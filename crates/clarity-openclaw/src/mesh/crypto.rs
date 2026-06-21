//! Passphrase-based encryption for Claw Mesh role-context events.
//!
//! Each role can optionally be protected by a user-supplied passphrase. The
//! passphrase is combined with the role id via PBKDF2-HMAC-SHA256 to derive a
//! 256-bit ChaCha20-Poly1305 key. Events are serialized to JSON, encrypted,
//! and stored with a format that is backward-compatible with plaintext
//! `*.json` files.
//!
//! Ciphertext format: `enc3:<base64(salt || nonce || ciphertext_and_tag)>`
//! - `salt`: 16 random bytes, used for PBKDF2 key derivation.
//! - `nonce`: 12 random bytes, used for ChaCha20-Poly1305.
//! - `ciphertext_and_tag`: ChaCha20-Poly1305 output including the 16-byte tag.
//!
//! The role id is used as additional authenticated data (AAD) so a ciphertext
//! cannot be moved across roles without detection.
//!
//! ponytail: passphrase-based E2EE is the first step. Future work can add
//! device-key key exchange to remove the need for out-of-band passphrases.

use super::transport::MeshTransportError;
use clarity_contract::RoleContextId;
use ring::{
    aead::{self, Aad, CHACHA20_POLY1305, Nonce, UnboundKey},
    pbkdf2,
    rand::{SecureRandom, SystemRandom},
};
use std::num::NonZeroU32;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const TAG_LEN: usize = 16;
const PREFIX: &str = "enc3:";

/// Number of PBKDF2 iterations. 100k is the conventional minimum for
/// interactive passphrases (OWASP 2023).
const PBKDF2_ITERATIONS: NonZeroU32 = match NonZeroU32::new(100_000) {
    Some(nz) => nz,
    // Const-eval only: 100_000 is a non-zero literal.
    None => panic!("PBKDF2_ITERATIONS must be non-zero"),
};

/// Encrypt `plaintext` with a passphrase derived from `role_id` and
/// `passphrase`.
///
/// Returns a string prefixed with `enc3:`.
pub fn encrypt(
    role_id: &RoleContextId,
    passphrase: &str,
    plaintext: &str,
) -> std::result::Result<String, MeshTransportError> {
    let rng = SystemRandom::new();

    let mut salt = [0u8; SALT_LEN];
    rng.fill(&mut salt)
        .map_err(|e| MeshTransportError::Crypto(format!("rng salt: {e}")))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes)
        .map_err(|e| MeshTransportError::Crypto(format!("rng nonce: {e}")))?;

    let key = derive_key(role_id, passphrase, &salt)?;

    let mut in_out = plaintext.as_bytes().to_vec();
    let unbound_key = UnboundKey::new(&CHACHA20_POLY1305, &key)
        .map_err(|e| MeshTransportError::Crypto(format!("unbound key: {e}")))?;
    let less_safe_key = aead::LessSafeKey::new(unbound_key);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    less_safe_key
        .seal_in_place_append_tag(nonce, Aad::from(role_id.as_ref().as_bytes()), &mut in_out)
        .map_err(|e| MeshTransportError::Crypto(format!("seal: {e}")))?;

    let mut blob = Vec::with_capacity(SALT_LEN + NONCE_LEN + in_out.len());
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&in_out);

    Ok(format!("{}{}", PREFIX, base64_encode(&blob)))
}

/// Decrypt a ciphertext produced by [`encrypt`].
///
/// If `ciphertext` does not start with `enc3:`, it is returned unchanged so
/// legacy plaintext events continue to work.
pub fn decrypt(
    role_id: &RoleContextId,
    passphrase: &str,
    ciphertext: &str,
) -> std::result::Result<String, MeshTransportError> {
    if !ciphertext.starts_with(PREFIX) {
        return Ok(ciphertext.to_string());
    }

    let blob = base64_decode(&ciphertext[PREFIX.len()..])?;
    if blob.len() < SALT_LEN + NONCE_LEN + TAG_LEN {
        return Err(MeshTransportError::Crypto(
            "ciphertext too short".to_string(),
        ));
    }

    let (salt, rest) = blob.split_at(SALT_LEN);
    let (nonce_bytes, ct_and_tag) = rest.split_at(NONCE_LEN);

    let key = derive_key(role_id, passphrase, salt)?;

    let unbound_key = UnboundKey::new(&CHACHA20_POLY1305, &key)
        .map_err(|e| MeshTransportError::Crypto(format!("unbound key: {e}")))?;
    let less_safe_key = aead::LessSafeKey::new(unbound_key);
    let nonce = Nonce::assume_unique_for_key(
        nonce_bytes
            .try_into()
            .map_err(|_| MeshTransportError::Crypto("invalid nonce length".to_string()))?,
    );

    let mut in_out = ct_and_tag.to_vec();
    let plaintext = less_safe_key
        .open_in_place(nonce, Aad::from(role_id.as_ref().as_bytes()), &mut in_out)
        .map_err(|_| MeshTransportError::Crypto("decryption failed".to_string()))?;

    String::from_utf8(plaintext.to_vec())
        .map_err(|_| MeshTransportError::Crypto("invalid utf-8 plaintext".to_string()))
}

/// Return true if the value looks like an encrypted ciphertext.
pub fn is_encrypted(value: &str) -> bool {
    value.starts_with(PREFIX)
}

fn derive_key(
    role_id: &RoleContextId,
    passphrase: &str,
    salt: &[u8],
) -> std::result::Result<[u8; KEY_LEN], MeshTransportError> {
    let mut key = [0u8; KEY_LEN];
    // Bind the KDF to the role id so the same passphrase for two roles yields
    // different keys. We prepend the role id to the passphrase material.
    let mut material = Vec::with_capacity(role_id.as_ref().len() + passphrase.len());
    material.extend_from_slice(role_id.as_ref().as_bytes());
    material.extend_from_slice(passphrase.as_bytes());

    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        PBKDF2_ITERATIONS,
        salt,
        &material,
        &mut key,
    );
    Ok(key)
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.encode(bytes)
}

fn base64_decode(s: &str) -> std::result::Result<Vec<u8>, MeshTransportError> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD
        .decode(s)
        .map_err(|e| MeshTransportError::Crypto(format!("base64 decode: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip() {
        let role = RoleContextId::new("operator");
        let plaintext = r#"{"event_id":"evt-1","content":"hello"}"#;
        let encrypted = encrypt(&role, "secret-password", plaintext).unwrap();
        assert!(encrypted.starts_with(PREFIX));
        assert_ne!(encrypted, plaintext);

        let decrypted = decrypt(&role, "secret-password", &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_plaintext_passthrough() {
        let role = RoleContextId::new("operator");
        let plaintext = "legacy-json-content";
        let decrypted = decrypt(&role, "any-password", plaintext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_passphrase_fails() {
        let role = RoleContextId::new("operator");
        let plaintext = "sensitive";
        let encrypted = encrypt(&role, "right-password", plaintext).unwrap();
        assert!(matches!(
            decrypt(&role, "wrong-password", &encrypted),
            Err(MeshTransportError::Crypto(_))
        ));
    }

    #[test]
    fn test_wrong_role_fails() {
        let role_a = RoleContextId::new("role-a");
        let role_b = RoleContextId::new("role-b");
        let plaintext = "sensitive";
        let encrypted = encrypt(&role_a, "password", plaintext).unwrap();
        assert!(matches!(
            decrypt(&role_b, "password", &encrypted),
            Err(MeshTransportError::Crypto(_))
        ));
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let role = RoleContextId::new("operator");
        let plaintext = "sensitive";
        let mut encrypted = encrypt(&role, "password", plaintext).unwrap();
        // Flip the last base64 character.
        let last = encrypted.pop().unwrap();
        let flipped = if last == 'A' { 'B' } else { 'A' };
        encrypted.push(flipped);
        assert!(matches!(
            decrypt(&role, "password", &encrypted),
            Err(MeshTransportError::Crypto(_))
        ));
    }
}
