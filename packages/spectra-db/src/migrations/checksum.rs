use super::error::{MigrationError, MigrationResult};
use sha2::{Digest, Sha256};

pub fn normalize_sql(bytes: &[u8]) -> MigrationResult<String> {
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| MigrationError::new("DB2503_INVALID_UTF8", "migration is not valid UTF-8"))?;
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}

pub fn checksum(version: u64, name: &str, up: &str, down: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(version.to_string().as_bytes());
    digest.update([0]);
    digest.update(name.as_bytes());
    digest.update([0]);
    digest.update(up.as_bytes());
    digest.update([0]);
    digest.update(down.as_bytes());
    format!("{:x}", digest.finalize())
}
