use super::checksum::{checksum, normalize_sql};
use super::error::{MigrationError, MigrationResult};
use super::Migration;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

struct Parts {
    name: String,
    up: Option<String>,
    down: Option<String>,
}

pub fn discover(directory: impl AsRef<Path>) -> MigrationResult<Vec<Migration>> {
    let directory = directory.as_ref();
    if !directory.is_dir() {
        return Err(MigrationError::new(
            "DB2503_DIRECTORY",
            format!(
                "migration directory does not exist: {}",
                directory.display()
            ),
        ));
    }
    let mut entries: BTreeMap<u64, Parts> = BTreeMap::new();
    let read_dir = fs::read_dir(directory)
        .map_err(|error| MigrationError::new("DB2503_DIRECTORY", error.to_string()))?;
    for entry in read_dir {
        let entry =
            entry.map_err(|error| MigrationError::new("DB2503_DISCOVERY", error.to_string()))?;
        let path = entry.path();
        if !path.is_file() {
            return Err(MigrationError::new(
                "DB2503_INVALID_FILE",
                format!(
                    "migration directory contains non-file entry: {}",
                    path.display()
                ),
            ));
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let (stem, kind) = if let Some(value) = file_name.strip_suffix(".up.sql") {
            (value, true)
        } else if let Some(value) = file_name.strip_suffix(".down.sql") {
            (value, false)
        } else {
            return Err(MigrationError::new(
                "DB2503_INVALID_FILENAME",
                format!("invalid migration filename: {file_name}"),
            ));
        };
        let Some((version_text, name)) = stem.split_once('_') else {
            return Err(MigrationError::new(
                "DB2503_INVALID_FILENAME",
                format!("migration filename has no name: {file_name}"),
            ));
        };
        if version_text.is_empty()
            || version_text
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .is_none()
            || !valid_name(name)
        {
            return Err(MigrationError::new(
                "DB2503_INVALID_FILENAME",
                format!("invalid migration version or name: {file_name}"),
            ));
        }
        let version = version_text
            .parse::<u64>()
            .expect("validated migration version");
        let sql = normalize_sql(
            &fs::read(&path)
                .map_err(|error| MigrationError::new("DB2503_READ", error.to_string()))?,
        )?;
        let parts = entries.entry(version).or_insert_with(|| Parts {
            name: name.to_owned(),
            up: None,
            down: None,
        });
        if parts.name != name {
            return Err(MigrationError::new(
                "DB2503_NAME_MISMATCH",
                format!("migration {version} has inconsistent names"),
            ));
        }
        let target = if kind { &mut parts.up } else { &mut parts.down };
        if target.is_some() {
            return Err(MigrationError::new(
                "DB2503_DUPLICATE",
                format!("duplicate migration version {version}"),
            ));
        }
        *target = Some(sql);
    }
    entries
        .into_iter()
        .map(|(version, parts)| {
            let up = parts.up.ok_or_else(|| {
                MigrationError::new(
                    "DB2503_MISSING_PAIR",
                    format!("migration {version} has no up file"),
                )
            })?;
            let down = parts.down.ok_or_else(|| {
                MigrationError::new(
                    "DB2503_MISSING_PAIR",
                    format!("migration {version} has no down file"),
                )
            })?;
            Ok(Migration {
                version,
                name: parts.name.clone(),
                checksum: checksum(version, &parts.name, &up, &down),
                up_sql: up,
                down_sql: down,
            })
        })
        .collect()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}
