// In crates/database/src/lib.rs (REPLACE ENTIRE FILE)

use app_config::types::DatabaseSettings;
use sqlx::{postgres::PgPoolOptions, PgPool};
use core_types::{Kline, Symbol};
use bigdecimal::BigDecimal;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue; 
use analytics::types::PerformanceReport;
use analytics::types::Trade; // Add this
use serde::Serialize;
use analytics::types::EquityPoint;
use rust_decimal::Decimal;

// API DTO for trades - avoids circular dependency with web-server
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ApiTrade {
    pub symbol: String,
    pub side: String,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub pnl: Decimal,
    pub fees: Decimal,
    pub signal_confidence: f64,
    pub leverage: i32,
}

/// A struct to fetch the report along with its parameters
#[derive(Debug, Serialize)]
pub struct FullReport {
    pub run_id: i64,
    pub parameters: JsonValue,
    pub report: PerformanceReport,
}

pub mod error;
pub mod types;

// Re-export the most important types for easy access.
pub use error::{Error, Result};

// This type needs to be available to our `app` crate.
// pub use analyzer::RankedReport; // Re-export for convenience (REMOVED)

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

    /// Fetches klines for a given symbol, interval, and date range from the database.
    ///
    /// # Arguments
    ///
    /// * `symbol`: The symbol to fetch klines for.
    /// * `interval`: The kline interval.
    /// * `start_time`: The start of the date range.
    /// * `end_time`: The end of the date range.
    ///
    /// # Returns
    ///
    /// A `Result` containing a vector of `Kline` structs on success.
    pub async fn get_klines_by_date_range(
        &self,
        symbol: &Symbol,
        interval: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<Kline>> {
        let start_ts = start_time.timestamp_millis();
        let end_ts = end_time.timestamp_millis();

        // Manually map each row to Kline, converting BigDecimal to Decimal
        let rows = sqlx::query!(
            r#"
            SELECT open_time, open, high, low, close, volume, close_time
            FROM klines
            WHERE symbol = $1 AND interval = $2 AND open_time >= $3 AND open_time <= $4
            ORDER BY open_time ASC
            "#,
            symbol.0,
            interval,
            start_ts,
            end_ts
        )
        .fetch_all(&self.0)
        .await
        .map_err(Error::OperationFailed)?;

        let klines = rows
            .into_iter()
            .map(|row| Kline {
                open_time: row.open_time,
                open: row.open.to_string().parse().unwrap(),
                high: row.high.to_string().parse().unwrap(),
                low: row.low.to_string().parse().unwrap(),
                close: row.close.to_string().parse().unwrap(),
                volume: row.volume.to_string().parse().unwrap(),
                close_time: row.close_time,
            })
            .collect();

        Ok(klines)
    }

    /// Saves a backtest run and its corresponding performance report to the database.
    ///
    /// # Arguments
    ///
    /// * `strategy_name`: The name of the strategy that was tested.
    /// * `symbol`: The symbol the test was run on.
    /// * `interval`: The interval used for the test.
    /// * `start_date`: The start date of the test period.
    /// * `end_date`: The end date of the test period.
    /// * `parameters`: The strategy parameters, to be serialized to JSON.
    /// * `report`: The calculated `PerformanceReport`.
    ///
    /// # Returns
    ///
    /// A `Result` containing the ID of the new backtest run on success.
    pub async fn save_backtest_report<T: serde::Serialize>(
        &self,
        job_id: Option<i64>, // <-- Add this parameter
        strategy_name: &str,
        symbol: &Symbol,
        interval: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        parameters: &T,
        report: &PerformanceReport,
    ) -> Result<i64> {
        // --- 1. Start a Transaction ---
        let mut tx = self.0.begin().await.map_err(Error::OperationFailed)?;

        // --- 2. Serialize Parameters to JSON ---
        let params_json: JsonValue = serde_json::to_value(parameters)
            .map_err(|e| Error::OperationFailed(sqlx::Error::Decode(e.into())))?;

        // --- 3. Insert into `backtest_runs` and get the new ID ---
        let run_id: i64 = sqlx::query!(
            r#"
            INSERT INTO backtest_runs (job_id, strategy_name, symbol, interval, start_date, end_date, parameters)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
            job_id, // <-- Bind the new parameter
            strategy_name,
            symbol.0,
            interval,
            start_date,
            end_date,
            params_json
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::OperationFailed)?
        .id;

        // --- 4. Serialize the Confidence Performance to JSON ---
        let confidence_json: JsonValue = serde_json::to_value(&report.confidence_performance)
             .map_err(|e| Error::OperationFailed(sqlx::Error::Decode(e.into())))?;

        // --- Convert Decimal fields to BigDecimal for sqlx ---
        let net_pnl_absolute_bd = BigDecimal::from_str(&report.net_pnl_absolute.to_string()).unwrap();
        let max_drawdown_absolute_bd = BigDecimal::from_str(&report.max_drawdown_absolute.to_string()).unwrap();
        let expectancy_bd = BigDecimal::from_str(&report.expectancy.to_string()).unwrap();
        let funding_pnl_bd = BigDecimal::from_str(&report.funding_pnl.to_string()).unwrap();

        // --- 5. Insert into `performance_reports` ---
        sqlx::query!(
            r#"
            INSERT INTO performance_reports (
                run_id, net_pnl_absolute, net_pnl_percentage, max_drawdown_absolute,
                max_drawdown_percentage, sharpe_ratio, win_rate, profit_factor, total_trades,
                sortino_ratio, calmar_ratio, avg_trade_duration_secs, expectancy,
                confidence_performance, larom, funding_pnl, drawdown_duration_secs
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17
            )
            "#,
            run_id,
            net_pnl_absolute_bd,
            report.net_pnl_percentage,
            max_drawdown_absolute_bd,
            report.max_drawdown_percentage,
            report.sharpe_ratio,
            report.win_rate,
            report.profit_factor,
            report.total_trades as i32, // cast u32 to i32 for postgres
            report.sortino_ratio,
            report.calmar_ratio,
            report.avg_trade_duration_secs as i64, // cast f64 to i64
            expectancy_bd,
            confidence_json,
            report.larom,
            funding_pnl_bd,
            report.drawdown_duration_secs
        )
        .execute(&mut *tx)
        .await
        .map_err(Error::OperationFailed)?;

        // --- 6. Commit the Transaction ---
        tx.commit().await.map_err(Error::OperationFailed)?;

        Ok(run_id)
    }

    /// Efficiently bulk-inserts a slice of trades into the database.
    pub async fn save_trades(&self, run_id: i64, trades: &[Trade]) -> Result<()> {
        if trades.is_empty() {
            return Ok(());
        }
        let mut tx = self.0.begin().await.map_err(Error::OperationFailed)?;
        for trade in trades {
            sqlx::query!(
                r#"
                INSERT INTO trades (
                    run_id, symbol, side, entry_time, exit_time, entry_price,
                    exit_price, quantity, pnl, fees, signal_confidence, leverage
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                "#,
                run_id,
                trade.symbol.0,
                format!("{:?}", trade.side), // "Long" or "Short"
                trade.entry_time,
                trade.exit_time,
                BigDecimal::from_str(&trade.entry_price.to_string()).unwrap(),
                BigDecimal::from_str(&trade.exit_price.to_string()).unwrap(),
                BigDecimal::from_str(&trade.quantity.to_string()).unwrap(),
                BigDecimal::from_str(&trade.pnl.to_string()).unwrap(),
                BigDecimal::from_str(&trade.fees.to_string()).unwrap(),
                trade.signal_confidence,
                trade.leverage as i32
            )
            .execute(&mut *tx)
            .await
            .map_err(Error::OperationFailed)?;
        }
        tx.commit().await.map_err(Error::OperationFailed)?;
        Ok(())
    }

    /// Creates a new optimization job entry and returns its ID.
    pub async fn create_optimization_job(&self, name: &str) -> Result<i64> {
        let record = sqlx::query!(
            "INSERT INTO optimization_jobs (name) VALUES ($1) RETURNING id",
            name
        )
        .fetch_one(&self.0)
        .await
        .map_err(Error::OperationFailed)?;

        Ok(record.id)
    }

    /// Fetches all performance reports associated with a given optimization job ID.
    pub async fn get_reports_for_job(&self, job_id: i64) -> Result<Vec<FullReport>> {
        let records = sqlx::query!(
            r#"
            SELECT br.parameters, pr.*
            FROM performance_reports pr
            JOIN backtest_runs br ON pr.run_id = br.id
            WHERE br.job_id = $1
            "#, // pr.* includes run_id
            job_id
        )
        .fetch_all(&self.0)
        .await
        .map_err(Error::OperationFailed)?;

        let full_reports = records.into_iter().map(|r| {
            let report = PerformanceReport {
                run_id: r.run_id,
                net_pnl_absolute: r.net_pnl_absolute.to_string().parse().unwrap_or_default(),
                net_pnl_percentage: r.net_pnl_percentage,
                max_drawdown_absolute: r.max_drawdown_absolute.to_string().parse().unwrap_or_default(),
                max_drawdown_percentage: r.max_drawdown_percentage,
                sharpe_ratio: r.sharpe_ratio,
                win_rate: r.win_rate,
                profit_factor: r.profit_factor,
                total_trades: r.total_trades as u32,
                sortino_ratio: r.sortino_ratio,
                calmar_ratio: r.calmar_ratio,
                avg_trade_duration_secs: r.avg_trade_duration_secs as f64,
                expectancy: r.expectancy.to_string().parse().unwrap_or_default(),
                confidence_performance: serde_json::from_value(r.confidence_performance.unwrap_or_default()).unwrap_or_default(),
                larom: r.larom,
                funding_pnl: r.funding_pnl.to_string().parse().unwrap_or_default(),
                drawdown_duration_secs: r.drawdown_duration_secs,
            };
            FullReport { 
                run_id: r.run_id, 
                parameters: r.parameters, 
                report 
            }
        }).collect();

        Ok(full_reports)
    }

    pub async fn get_latest_job_id(&self) -> Result<i64> {
        let record = sqlx::query!("SELECT id FROM optimization_jobs ORDER BY id DESC LIMIT 1")
            .fetch_one(&self.0)
            .await
            .map_err(Error::OperationFailed)?;
        Ok(record.id)
    }

    /// Fetches a paginated list of optimization jobs from the database.
    pub async fn get_optimization_jobs_paginated(
        &self,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<OptimizationJob>, i64)> {
        let offset = (page - 1) * page_size;

        let jobs = sqlx::query_as!(
            OptimizationJob,
            "SELECT id, name, created_at FROM optimization_jobs ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            page_size as i64,
            offset as i64
        )
        .fetch_all(&self.0)
        .await
        .map_err(Error::OperationFailed)?;

        let total_count = sqlx::query!("SELECT COUNT(*) as count FROM optimization_jobs")
            .fetch_one(&self.0)
            .await
            .map_err(Error::OperationFailed)?
            .count
            .unwrap_or(0);
        
        Ok((jobs, total_count))
    }

    /// Fetches the detailed summary for a single optimization job.
    pub async fn get_optimization_summary(&self, job_id: i64) -> Result<Option<JsonValue>> {
        let record = sqlx::query!(
            "SELECT top_n_results FROM optimization_summaries WHERE job_id = $1",
            job_id
        )
        .fetch_optional(&self.0)
        .await
        .map_err(Error::OperationFailed)?;

        // `fetch_optional` returns an Option, which is perfect.
        // If the record exists, we return Some(json), otherwise None.
        Ok(record.map(|r| r.top_n_results))
    }

    /// Fetches the full performance report for a single backtest run ID.
    pub async fn get_performance_report(&self, run_id: i64) -> Result<Option<PerformanceReport>> {
        let record = sqlx::query!(
            "SELECT * FROM performance_reports WHERE run_id = $1",
            run_id
        )
        .fetch_optional(&self.0)
        .await
        .map_err(Error::OperationFailed)?;
        
        // Manual mapping from the flat DB record to our PerformanceReport struct
        Ok(record.map(|r| PerformanceReport {
            run_id: r.run_id,
            net_pnl_absolute: r.net_pnl_absolute.to_string().parse().unwrap_or_default(),
            net_pnl_percentage: r.net_pnl_percentage,
            max_drawdown_absolute: r.max_drawdown_absolute.to_string().parse().unwrap_or_default(),
            max_drawdown_percentage: r.max_drawdown_percentage,
            sharpe_ratio: r.sharpe_ratio,
            win_rate: r.win_rate,
            profit_factor: r.profit_factor,
            total_trades: r.total_trades as u32,
            sortino_ratio: r.sortino_ratio,
            calmar_ratio: r.calmar_ratio,
            avg_trade_duration_secs: r.avg_trade_duration_secs as f64,
            expectancy: r.expectancy.to_string().parse().unwrap_or_default(),
            confidence_performance: serde_json::from_value(r.confidence_performance.unwrap_or_default()).unwrap_or_default(),
            larom: r.larom,
            funding_pnl: r.funding_pnl.to_string().parse().unwrap_or_default(),
            drawdown_duration_secs: r.drawdown_duration_secs,
        }))
    }

    /// Fetches the full equity curve for a single backtest run ID.
    pub async fn get_equity_curve_for_run(&self, run_id: i64) -> Result<Vec<EquityPoint>> {
        let rows = sqlx::query!(
            "SELECT timestamp, equity FROM equity_curves WHERE run_id = $1 ORDER BY timestamp ASC",
            run_id
        )
        .fetch_all(&self.0)
        .await
        .map_err(Error::OperationFailed)?;
        
        let points = rows
            .into_iter()
            .map(|row| EquityPoint {
                timestamp: row.timestamp,
                value: row.equity.to_string().parse().unwrap_or_default(),
            })
            .collect();
        
        Ok(points)
    }

    /// Fetches a paginated list of trades for a single backtest run ID.
    pub async fn get_trades_for_run_paginated(
        &self,
        run_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<ApiTrade>, i64)> {
        let offset = (page - 1) * page_size;

        let rows = sqlx::query!(
            r#"SELECT symbol, side, entry_time, exit_time, entry_price, exit_price, quantity, pnl, fees, signal_confidence, leverage FROM trades WHERE run_id = $1 ORDER BY entry_time ASC LIMIT $2 OFFSET $3"#,
            run_id,
            page_size as i64,
            offset as i64
        )
        .fetch_all(&self.0)
        .await
        .map_err(Error::OperationFailed)?;

        let trades = rows
            .into_iter()
            .map(|row| ApiTrade {
                symbol: row.symbol,
                side: row.side,
                entry_time: row.entry_time,
                exit_time: row.exit_time,
                entry_price: row.entry_price.to_string().parse().unwrap_or_default(),
                exit_price: row.exit_price.to_string().parse().unwrap_or_default(),
                quantity: row.quantity.to_string().parse().unwrap_or_default(),
                pnl: row.pnl.to_string().parse().unwrap_or_default(),
                fees: row.fees.to_string().parse().unwrap_or_default(),
                signal_confidence: row.signal_confidence,
                leverage: row.leverage,
            })
            .collect();

        let total_count = sqlx::query!("SELECT COUNT(*) as count FROM trades WHERE run_id = $1", run_id)
            .fetch_one(&self.0)
            .await
            .map_err(Error::OperationFailed)?
            .count
            .unwrap_or(0);
        
        Ok((trades, total_count))
    }

    pub async fn save_optimization_summary<T: Serialize>(
        &self,
        job_id: i64,
        top_n_results: &[T], // Takes a slice of the ranked results
    ) -> Result<()> {
        let results_json: JsonValue = serde_json::to_value(top_n_results)
            .map_err(|e| Error::OperationFailed(sqlx::Error::Decode(e.into())))?;

        sqlx::query!(
            "INSERT INTO optimization_summaries (job_id, top_n_results) VALUES ($1, $2)",
            job_id,
            results_json
        )
        .execute(&self.0)
        .await
        .map_err(Error::OperationFailed)?;

        Ok(())
    }

    pub async fn save_equity_curve(&self, run_id: i64, equity_curve: &[EquityPoint]) -> Result<()> {
        if equity_curve.is_empty() {
            return Ok(());
        }
        let mut tx = self.0.begin().await.map_err(Error::OperationFailed)?;
        for point in equity_curve {
            sqlx::query!(
                "INSERT INTO equity_curves (run_id, timestamp, equity) VALUES ($1, $2, $3)",
                run_id,
                point.timestamp,
                BigDecimal::from_str(&point.value.to_string()).unwrap()
            )
            .execute(&mut *tx)
            .await
            .map_err(Error::OperationFailed)?;
        }
        tx.commit().await.map_err(Error::OperationFailed)?;
        Ok(())
    }

    /// Fetches a paginated list of backtest runs from the database.
    /// Also returns the total count of all runs for pagination controls.
    pub async fn get_backtest_runs_paginated(
        &self,
        page: u32,
        page_size: u32,
        job_id: Option<i64>, // <-- Add filter parameter
    ) -> Result<(Vec<BacktestRun>, i64)> {
        let offset = (page - 1) * page_size;
        
        let mut query_builder = sqlx::QueryBuilder::new(
            "SELECT br.id, br.strategy_name, br.symbol, br.interval, br.start_date, br.end_date, br.created_at, pr.net_pnl_percentage, pr.total_trades, pr.sharpe_ratio, pr.max_drawdown_percentage FROM backtest_runs br LEFT JOIN performance_reports pr ON br.id = pr.run_id WHERE 1=1 "
        );
        let mut count_builder = sqlx::QueryBuilder::new("SELECT COUNT(*) as count FROM backtest_runs WHERE 1=1 ");

        if let Some(id) = job_id {
            query_builder.push(" AND br.job_id = ").push_bind(id);
            count_builder.push(" AND job_id = ").push_bind(id);
        } else {
            // By default, show only single runs, not those part of an optimization
            query_builder.push(" AND br.job_id IS NULL ");
            count_builder.push(" AND job_id IS NULL ");
        }

        query_builder.push(" ORDER BY br.created_at DESC LIMIT ");
        query_builder.push_bind(page_size as i64);
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset as i64);
        
        // Use the new struct with query_as
        let runs: Vec<BacktestRun> = query_builder.build_query_as().fetch_all(&self.0).await.map_err(Error::OperationFailed)?;
        let total_count = count_builder.build_query_scalar::<i64>().fetch_one(&self.0).await.map_err(Error::OperationFailed)?;

        Ok((runs, total_count))
    }
}

// This struct will now hold a mix of metadata and key performance metrics.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BacktestRun {
    pub id: i64,
    pub strategy_name: String,
    pub symbol: String,
    pub interval: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    // Add fields from performance_reports (nullable)
    pub net_pnl_percentage: Option<f64>,
    pub total_trades: Option<i32>,
    pub sharpe_ratio: Option<f64>,
    pub max_drawdown_percentage: Option<f64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OptimizationJob {
    pub id: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
}