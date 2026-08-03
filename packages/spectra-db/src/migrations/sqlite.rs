use super::error::{MigrationError, MigrationResult};
use super::{AppliedMigration, Migration, MigrationDrift, MigrationResultEntry, MigrationStatus};
use crate::sqlite::{SqliteConnection, SqliteStatement, SqliteValue, StepResult};

pub struct SqliteMigrator {
    pub(crate) connection: SqliteConnection,
    pub(crate) migrations: Vec<Migration>,
}

impl SqliteMigrator {
    pub fn new(connection: SqliteConnection, migrations: Vec<Migration>) -> Self {
        Self {
            connection,
            migrations,
        }
    }

    pub fn status(&self) -> MigrationResult<MigrationStatus> {
        let applied = read_applied(&self.connection)?;
        let mut drift = Vec::new();
        let mut pending = Vec::new();
        for (index, record) in applied.iter().enumerate() {
            if self
                .migrations
                .get(index)
                .map(|migration| migration.version)
                != Some(record.version)
            {
                drift.push(MigrationDrift {
                    version: record.version,
                    reason: "migration was applied out of order".into(),
                });
            }
        }
        for record in &applied {
            match self
                .migrations
                .iter()
                .find(|migration| migration.version == record.version)
            {
                None => drift.push(MigrationDrift {
                    version: record.version,
                    reason: "applied migration file is missing".into(),
                }),
                Some(migration) if migration.name != record.name => drift.push(MigrationDrift {
                    version: record.version,
                    reason: "migration name differs from tracking table".into(),
                }),
                Some(migration) if migration.checksum != record.checksum => {
                    drift.push(MigrationDrift {
                        version: record.version,
                        reason: "migration checksum differs from tracking table".into(),
                    })
                }
                Some(_) => {}
            }
        }
        for migration in &self.migrations {
            if !applied
                .iter()
                .any(|record| record.version == migration.version)
            {
                pending.push(migration.clone());
            }
        }
        Ok(MigrationStatus {
            applied,
            pending,
            drift,
        })
    }

    pub fn migrate(&self) -> MigrationResult<Vec<MigrationResultEntry>> {
        self.ensure_tracking_table()?;
        let status = self.status()?;
        if !status.drift.is_empty() {
            return Err(MigrationError::new(
                "DB2503_CHECKSUM_MISMATCH",
                format_drift(&status.drift),
            ));
        }
        let mut result = Vec::new();
        for migration in status.pending {
            self.connection
                .begin_immediate()
                .map_err(MigrationError::from)?;
            let operation = (|| {
                self.connection
                    .execute_batch(&migration.up_sql)
                    .map_err(MigrationError::from)?;
                let mut statement = SqliteStatement::prepare(self.connection.clone(), "INSERT INTO _spectra_migrations(version,name,checksum,applied_at) VALUES(?1,?2,?3,datetime('now'))").map_err(MigrationError::from)?;
                statement
                    .bind(1, SqliteValue::Integer(migration.version as i64))
                    .map_err(MigrationError::from)?;
                statement
                    .bind(2, SqliteValue::Text(migration.name.clone()))
                    .map_err(MigrationError::from)?;
                statement
                    .bind(3, SqliteValue::Text(migration.checksum.clone()))
                    .map_err(MigrationError::from)?;
                statement.step().map_err(MigrationError::from)?;
                statement.finalize().map_err(MigrationError::from)?;
                Ok::<(), MigrationError>(())
            })();
            match operation {
                Ok(()) => {
                    self.connection.commit().map_err(MigrationError::from)?;
                    result.push(MigrationResultEntry {
                        version: migration.version,
                        name: migration.name,
                        action: "applied".into(),
                    });
                }
                Err(error) => {
                    let _ = self.connection.rollback();
                    return Err(error);
                }
            }
        }
        Ok(result)
    }

    pub fn rollback(&self, steps: usize) -> MigrationResult<Vec<MigrationResultEntry>> {
        if steps == 0 {
            return Ok(Vec::new());
        }
        let status = self.status()?;
        if !status.drift.is_empty() {
            return Err(MigrationError::new(
                "DB2503_CHECKSUM_MISMATCH",
                format_drift(&status.drift),
            ));
        }
        if steps > status.applied.len() {
            return Err(MigrationError::new(
                "DB2503_ROLLBACK_RANGE",
                "rollback exceeds applied migrations",
            ));
        }
        let mut applied = status.applied;
        applied.sort_by_key(|record| record.version);
        let selected = applied.into_iter().rev().take(steps).collect::<Vec<_>>();
        let mut result = Vec::new();
        for record in selected {
            let migration = self
                .migrations
                .iter()
                .find(|migration| migration.version == record.version)
                .ok_or_else(|| {
                    MigrationError::new(
                        "DB2503_MISSING_FILE",
                        format!("migration {} is missing", record.version),
                    )
                })?;
            self.connection
                .begin_immediate()
                .map_err(MigrationError::from)?;
            let operation = (|| {
                self.connection
                    .execute_batch(&migration.down_sql)
                    .map_err(MigrationError::from)?;
                let mut statement = SqliteStatement::prepare(
                    self.connection.clone(),
                    "DELETE FROM _spectra_migrations WHERE version=?1",
                )
                .map_err(MigrationError::from)?;
                statement
                    .bind(1, SqliteValue::Integer(record.version as i64))
                    .map_err(MigrationError::from)?;
                statement.step().map_err(MigrationError::from)?;
                statement.finalize().map_err(MigrationError::from)?;
                Ok::<(), MigrationError>(())
            })();
            match operation {
                Ok(()) => {
                    self.connection.commit().map_err(MigrationError::from)?;
                    result.push(MigrationResultEntry {
                        version: record.version,
                        name: record.name,
                        action: "rolled_back".into(),
                    });
                }
                Err(error) => {
                    let _ = self.connection.rollback();
                    return Err(error);
                }
            }
        }
        Ok(result)
    }

    fn ensure_tracking_table(&self) -> MigrationResult<()> {
        self.connection.execute_batch("CREATE TABLE IF NOT EXISTS _spectra_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at TEXT NOT NULL)").map_err(MigrationError::from)
    }
}

fn read_applied(connection: &SqliteConnection) -> MigrationResult<Vec<AppliedMigration>> {
    let mut statement = match SqliteStatement::prepare(
        connection.clone(),
        "SELECT version,name,checksum,applied_at FROM _spectra_migrations ORDER BY version",
    ) {
        Ok(statement) => statement,
        Err(error) if error.message.contains("no such table") => return Ok(Vec::new()),
        Err(error) => return Err(MigrationError::from(error)),
    };
    let mut records = Vec::new();
    while statement.step().map_err(MigrationError::from)? == StepResult::Row {
        let version = match statement.column_value(0).map_err(MigrationError::from)? {
            SqliteValue::Integer(value) if value > 0 => value as u64,
            _ => {
                return Err(MigrationError::new(
                    "DB2503_TRACKING_CORRUPT",
                    "invalid migration version in tracking table",
                ))
            }
        };
        let name = match statement.column_value(1).map_err(MigrationError::from)? {
            SqliteValue::Text(value) => value,
            _ => {
                return Err(MigrationError::new(
                    "DB2503_TRACKING_CORRUPT",
                    "invalid migration name in tracking table",
                ))
            }
        };
        let checksum = match statement.column_value(2).map_err(MigrationError::from)? {
            SqliteValue::Text(value) => value,
            _ => {
                return Err(MigrationError::new(
                    "DB2503_TRACKING_CORRUPT",
                    "invalid migration checksum in tracking table",
                ))
            }
        };
        let applied_at = match statement.column_value(3).map_err(MigrationError::from)? {
            SqliteValue::Text(value) => value,
            _ => {
                return Err(MigrationError::new(
                    "DB2503_TRACKING_CORRUPT",
                    "invalid migration timestamp in tracking table",
                ))
            }
        };
        records.push(AppliedMigration {
            version,
            name,
            checksum,
            applied_at,
        });
    }
    statement.finalize().map_err(MigrationError::from)?;
    Ok(records)
}

fn format_drift(drift: &[MigrationDrift]) -> String {
    drift
        .iter()
        .map(|entry| format!("{}: {}", entry.version, entry.reason))
        .collect::<Vec<_>>()
        .join("; ")
}
