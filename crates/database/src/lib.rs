// In crates/database/src/lib.rs (REPLACE ENTIRE FILE)

use app_config::types::DatabaseSettings;
use sqlx::{postgres::PgPoolOptions, PgPool};
use core_types::{Kline, Symbol};
use bigdecimal::BigDecimal;
use std::str::FromStr;

pub mod error;
pub mod types;

// Re-export the most important types for easy access.
pub use error::{Error, Result};

/// A wrapper around the `sqlx` connection pool.
#[derive(Debug, Clone)]
pub struct Db(PgPool);

/// Establishes a connection pool to the PostgreSQL database and runs migrations.
///
/// # Arguments
///
/// * `settings`: The database configuration settings.
///
/// # Returns
///
/// A `Result` containing the `Db` wrapper on success, or an `Error` on failure.
pub async fn connect(settings: &DatabaseSettings) -> Result<Db> {
    // Create a connection pool.
    let pool = PgPoolOptions::new()
        .max_connections(5)
        // The `?` operator uses the `#[from]` attribute in our error enum
        // to automatically convert the `sqlx::Error` into a `database::Error`.
        .connect(&settings.url)
        .await?;

    // Run database migrations. This ensures the database schema is up-to-date.
    sqlx::migrate!("../../migrations").run(&pool).await.map_err(Error::from)?;

    Ok(Db(pool))
}

// Add the impl block for our Db wrapper struct
impl Db {
    /// Inserts a slice of `Kline` data for a specific interval into the database.
    pub async fn insert_klines(
        &self,
        symbol: &Symbol,
        interval: &str, // <-- NEW: Add interval parameter
        klines: &[Kline],
    ) -> Result<()> {
        let mut tx = self.0.begin().await.map_err(Error::OperationFailed)?;

        for kline in klines {
            // UPDATED: Added `interval` to the INSERT statement and binding.
            sqlx::query!(
                r#"
                INSERT INTO klines (symbol, interval, open_time, open, high, low, close, volume, close_time)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (symbol, interval, open_time) DO NOTHING
                "#,
                symbol.0,
                interval, // <-- NEW: Bind the interval variable
                kline.open_time,
                BigDecimal::from_str(&kline.open.to_string()).unwrap(),
                BigDecimal::from_str(&kline.high.to_string()).unwrap(),
                BigDecimal::from_str(&kline.low.to_string()).unwrap(),
                BigDecimal::from_str(&kline.close.to_string()).unwrap(),
                BigDecimal::from_str(&kline.volume.to_string()).unwrap(),
                kline.close_time
            )
            .execute(&mut *tx)
            .await
            .map_err(Error::OperationFailed)?;
        }

        tx.commit().await.map_err(Error::OperationFailed)?;

        Ok(())
    }
}