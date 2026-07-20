//! Key file persistence and filesystem permissions.

use crate::SecretError;
use std::fs;
#[cfg(unix)]
use std::io::Write;
use std::path::Path;

/// Persist the key to disk (hex-encoded).
///
/// On Unix the file is created with `0o600` permissions. On other platforms the
/// file is written normally; callers should rely on filesystem permissions and
/// full-disk encryption to protect the key.
pub(crate) fn save_key(key_path: &Path, key: &[u8]) -> Result<(), SecretError> {
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let hex_key = hex::encode(key);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(key_path)?;
        file.write_all(hex_key.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        fs::write(key_path, hex_key)?;
    }
    Ok(())
}
