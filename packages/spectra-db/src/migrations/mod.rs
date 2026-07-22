mod checksum;
mod discovery;
mod error;
mod sqlite;

pub use checksum::{checksum, normalize_sql};
pub use discovery::discover;
pub use error::{MigrationError, MigrationResult};
pub use sqlite::SqliteMigrator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub version: u64,
    pub name: String,
    pub up_sql: String,
    pub down_sql: String,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: u64,
    pub name: String,
    pub checksum: String,
    pub applied_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationDrift {
    pub version: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStatus {
    pub applied: Vec<AppliedMigration>,
    pub pending: Vec<Migration>,
    pub drift: Vec<MigrationDrift>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationResultEntry {
    pub version: u64,
    pub name: String,
    pub action: String,
}

impl SqliteMigrator {
    pub fn from_directory(
        connection: crate::sqlite::SqliteConnection,
        directory: impl AsRef<std::path::Path>,
    ) -> MigrationResult<Self> {
        Ok(Self::new(connection, discover(directory)?))
    }
}
