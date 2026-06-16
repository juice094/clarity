//! AES/ECB encryption helpers for WeChat iLink media.

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, block_padding::Pkcs7};
use base64::Engine;

type Aes128EcbEnc = ecb::Encryptor<aes::Aes128>;
type Aes128EcbDec = ecb::Decryptor<aes::Aes128>;

pub(crate) fn aes_ecb_padded_size(plaintext_size: usize) -> usize {
    ((plaintext_size / 16) + 1) * 16
}

pub(crate) fn encrypt_aes_ecb(plaintext: &[u8], key: &[u8; 16]) -> anyhow::Result<Vec<u8>> {
    let padded_size = aes_ecb_padded_size(plaintext.len());
    let mut buffer = vec![0u8; padded_size];
    buffer[..plaintext.len()].copy_from_slice(plaintext);
    let encrypted = Aes128EcbEnc::new(&(*key).into())
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
        .map_err(|e| {
            crate::record!(
                ERROR,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Fail
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "media encrypt failed"
            );
            anyhow::Error::msg(format!("media encrypt failed: {e}"))
        })?;
    Ok(encrypted.to_vec())
}

pub(crate) fn decrypt_aes_ecb(ciphertext: &[u8], key: &[u8; 16]) -> anyhow::Result<Vec<u8>> {
    let mut buffer = ciphertext.to_vec();
    Aes128EcbDec::new(&(*key).into())
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map(|decrypted| decrypted.to_vec())
        .map_err(|e| {
            crate::record!(
                ERROR,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Fail
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "wechat: media decrypt failed"
            );
            anyhow::Error::msg(format!("media decrypt failed: {e}"))
        })
}

pub(crate) fn parse_aes_key(raw: &str) -> anyhow::Result<[u8; 16]> {
    let raw = raw.trim();
    if raw.len() == 32 && raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        let bytes = hex::decode(raw).map_err(|e| {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Reject
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "media hex aes_key invalid"
            );
            anyhow::Error::msg(format!("media hex aes_key invalid: {e}"))
        })?;
        return <[u8; 16]>::try_from(bytes.as_slice()).map_err(|_| {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Reject
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"key_kind": "hex", "expected_bytes": 16})),
                "wechat: media hex aes_key has wrong byte length"
            );
            anyhow::Error::msg("media hex aes_key must be 16 bytes")
        });
    }

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|e| {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Reject
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "media base64 aes_key invalid"
            );
            anyhow::Error::msg(format!("media base64 aes_key invalid: {e}"))
        })?;

    if decoded.len() == 16 {
        return <[u8; 16]>::try_from(decoded.as_slice()).map_err(|_| {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Reject
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"key_kind": "base64", "expected_bytes": 16})),
                "wechat: media base64 aes_key has wrong byte length"
            );
            anyhow::Error::msg("media base64 aes_key must be 16 bytes")
        });
    }

    if decoded.len() == 32 && decoded.iter().all(u8::is_ascii_hexdigit) {
        let hex_text = std::str::from_utf8(&decoded).map_err(|e| {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Reject
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "media aes_key utf8 invalid"
            );
            anyhow::Error::msg(format!("media aes_key utf8 invalid: {e}"))
        })?;
        let bytes = hex::decode(hex_text).map_err(|e| {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Reject
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "media nested hex aes_key invalid"
            );
            anyhow::Error::msg(format!("media nested hex aes_key invalid: {e}"))
        })?;
        return <[u8; 16]>::try_from(bytes.as_slice()).map_err(|_| {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Reject
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"key_kind": "nested_hex", "expected_bytes": 16})),
                "wechat: media nested hex aes_key has wrong byte length"
            );
            anyhow::Error::msg("media nested hex aes_key must be 16 bytes")
        });
    }

    anyhow::bail!(
        "media aes_key must decode to 16 raw bytes or 32 hex chars, got {} bytes",
        decoded.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aes_key_accepts_hex_and_base64() {
        let raw = *b"0123456789abcdef";
        let hex_key = hex::encode(raw);
        let base64_key = base64::engine::general_purpose::STANDARD.encode(raw);

        assert_eq!(parse_aes_key(&hex_key).unwrap(), raw);
        assert_eq!(parse_aes_key(&base64_key).unwrap(), raw);
    }

    #[test]
    fn round_trip_aes_ecb_encryption() {
        let key = [1u8; 16];
        let plaintext = b"hello world";
        let encrypted = encrypt_aes_ecb(plaintext, &key).unwrap();
        assert!(!encrypted.is_empty());
        let decrypted = decrypt_aes_ecb(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
