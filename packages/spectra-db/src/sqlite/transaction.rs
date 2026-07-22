use super::connection::SqliteConnection;
use super::error::SqliteResult;

pub struct SqliteTransaction {
    connection: SqliteConnection,
    active: bool,
}

impl SqliteTransaction {
    pub fn begin(connection: SqliteConnection) -> SqliteResult<Self> {
        connection.begin()?;
        Ok(Self {
            connection,
            active: true,
        })
    }
    pub fn commit(mut self) -> SqliteResult<()> {
        self.connection.commit()?;
        self.active = false;
        Ok(())
    }
    pub fn rollback(mut self) -> SqliteResult<()> {
        self.connection.rollback()?;
        self.active = false;
        Ok(())
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for SqliteTransaction {
    fn drop(&mut self) {
        if self.active {
            let _ = self.connection.rollback();
        }
    }
}
