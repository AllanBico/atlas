// In crates/database/src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    // Replace the Placeholder with a specific error
    #[error("Failed to connect to the database")]
    ConnectionError(#[from] sqlx::Error),
    #[error("Database migration failed: {0}")]
    MigrateError(#[from] sqlx::migrate::MigrateError),
    #[error("Database operation failed")]
    OperationFailed(sqlx::Error),
}

pub type Result<T> = std::result::Result<T, Error>;