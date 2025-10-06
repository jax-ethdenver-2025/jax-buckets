mod sqlite;
mod types;

use std::ops::Deref;

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Database(SqlitePool);

pub type DatabaseConnection = sqlx::SqliteConnection;

impl Database {
    pub async fn connect(database_url: &url::Url) -> Result<Self, DatabaseSetupError> {
        if database_url.scheme() == "sqlite" {
            let db = sqlite::connect_sqlite(database_url).await?;
            sqlite::migrate_sqlite(&db).await?;
            return Ok(Database::new(db));
        }

        Err(DatabaseSetupError::UnknownDbType(
            database_url.scheme().to_string(),
        ))
    }

    pub fn new(pool: SqlitePool) -> Self {
        Self(pool)
    }
}

impl Deref for Database {
    type Target = SqlitePool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseSetupError {
    #[error("error occurred while attempting database migration: {0}")]
    MigrationFailed(sqlx::migrate::MigrateError),

    #[error("unable to perform initial connection and check of the database: {0}")]
    Unavailable(sqlx::Error),

    #[error("requested database type was not recognized: {0}")]
    UnknownDbType(String),
}
