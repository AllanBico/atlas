// In app/src/optimizer.rs

use serde::Deserialize;
use strategies::types::MACrossoverSettings;
use std::fs;
use anyhow::{Context, Result};
use itertools::{ iproduct};
use crate::{ SimpleRiskManager,}; // MACrossover will be imported below
use app_config::types::AppSettings;
use backtester::Backtester;
use core_types::Symbol;
use execution::simulated::SimulatedExecutor;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use rust_decimal_macros::dec;
use std::sync::Arc;
use strategies::ma_crossover::MACrossover;
use tokio::runtime::Runtime;
use chrono::Utc;
use chrono::TimeZone;

// --- Structs for deserializing optimizer.toml ---

#[derive(Deserialize, Debug)]
pub struct OptimizerConfig {
    pub job: JobSettings,
    pub ma_crossover_params: MaCrossoverParams,
}

#[derive(Deserialize, Debug)]
pub struct JobSettings {
    pub name: String,
    pub symbol: String,
    pub interval: String,
    pub start_date: String,
    pub end_date: String,
    pub strategy_to_optimize: String,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)] // Allows serde to try parsing as one variant, then the next
enum ParamValue {
    Fixed(u32),
    Range { start: u32, end: u32, step: Option<u32> },
}

#[derive(Deserialize, Debug)]
pub struct MaCrossoverParams {
    m5_fast_period: ParamValue,
    m5_slow_period: ParamValue,
    h1_fast_period: ParamValue,
    h1_slow_period: ParamValue,
    confidence: f64, // Keep confidence fixed for now
}

// --- Public API for the Optimizer Module ---

pub fn load_optimizer_config() -> Result<OptimizerConfig> {
    let content = fs::read_to_string("config/optimizer.toml")
        .context("Failed to read config/optimizer.toml")?;
    toml::from_str(&content).context("Failed to parse optimizer.toml")
}

pub fn generate_parameter_sets(config: &OptimizerConfig) -> Vec<MACrossoverSettings> {
    let params = &config.ma_crossover_params;

    // Helper to expand a ParamValue into a Vec of numbers
    let expand = |p: &ParamValue| -> Vec<u32> {
        match p {
            ParamValue::Fixed(val) => vec![*val],
            ParamValue::Range { start, end, step } => {
                (*start..=*end).step_by(step.unwrap_or(1) as usize).collect()
            }
        }
    };
    
    // Create vectors of values for each parameter range
    let m5_fasts = expand(&params.m5_fast_period);
    let m5_slows = expand(&params.m5_slow_period);
    let h1_fasts = expand(&params.h1_fast_period);
    let h1_slows = expand(&params.h1_slow_period);

    // Use itertools::iproduct! to get all combinations
    iproduct!(m5_fasts, m5_slows, h1_fasts, h1_slows)
        .filter_map(|(m5_fast, m5_slow, h1_fast, h1_slow)| {
            // Filter out invalid combinations where fast >= slow
            if m5_fast >= m5_slow || h1_fast >= h1_slow {
                return None;
            }
            Some(MACrossoverSettings {
                m5_fast_period: m5_fast,
                m5_slow_period: m5_slow,
                h1_fast_period: h1_fast,
                h1_slow_period: h1_slow,
                confidence: params.confidence,
            })
        })
        .collect()
}

fn run_single_backtest_and_save(
    job_id: i64,
    main_settings: &app_config::Settings,
    job_settings: &JobSettings,
    params: &MACrossoverSettings,
) -> Result<()> {
    // 1. Create a new, single-threaded Tokio runtime for this specific task.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    // 2. Block this thread on the async logic.
    rt.block_on(async {
        let db = database::connect(&main_settings.database).await?;
        
        let symbol = Symbol(job_settings.symbol.clone());
        let interval = job_settings.interval.clone();
        
        // ... (Instantiate strategy, risk_manager, executor) ...
        let risk_manager = Box::new(SimpleRiskManager::new(main_settings.simple_risk_manager.clone().unwrap()));
        let strategy = Box::new(MACrossover::new(params.clone()));
        let executor = Box::new(SimulatedExecutor::new(main_settings.simulation.clone().unwrap(), dec!(10_000.0)));

        // Parse start and end dates, supporting both YYYY-MM-DD and YYYY-MM-DDTHH:MM:SS
        let parse_date = |s: &str, is_start: bool| {
            if let Ok(dt) = Utc.datetime_from_str(s, "%Y-%m-%dT%H:%M:%S") {
                Ok(dt)
            } else if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                let time = if is_start { chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap() } else { chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap() };
                Ok(Utc.from_utc_datetime(&date.and_time(time)))
            } else {
                Err(anyhow::anyhow!(format!("Invalid date format: {}", s)))
            }
        };
        let start_dt = parse_date(&job_settings.start_date, true)?;
        let end_dt = parse_date(&job_settings.end_date, false)?;

        let klines = db.get_klines_by_date_range(&symbol, &interval, start_dt, end_dt).await?;
        
        let mut backtester = Backtester::new(symbol.clone(), interval.clone(), strategy, risk_manager, executor);

        if let Ok((report, trades)) = backtester.run(klines).await {
            let run_id = db.save_backtest_report(Some(job_id), &job_settings.strategy_to_optimize, &symbol, &interval, start_dt, end_dt, params, &report).await?;
            db.save_trades(run_id, &trades).await?;
            tracing::info!(run_id, "Saved results.");
        }
        
        // This Result is for the async block
        Ok(())
    }) // block_on returns the Result from the async block
}

/// The main parallel engine for running an optimization job.
pub fn run_optimization(
    app_settings: &AppSettings,
    job_settings: &JobSettings,
    param_sets: Vec<MACrossoverSettings>,
    job_id: i64,
) -> Result<i64> {
    tracing::info!(cores = app_settings.optimizer_cores, "Configuring Rayon thread pool.");
    ThreadPoolBuilder::new()
        .num_threads(app_settings.optimizer_cores as usize)
        .build_global()
        .context("Failed to build Rayon thread pool")?;

    // Create a single DB pool to be shared across all threads.
    // `sqlx::PgPool` is thread-safe (`Arc`-based) by design.
    // let db_settings = &app_config::load_settings()?.database;
    // let db_pool = database::connect(db_settings).await?;
    
    let shared_settings = Arc::new(app_config::load_settings()?);
    
    param_sets.par_iter().for_each_with(shared_settings, |settings, params| {
        // Just call our synchronous wrapper. No async mess here!
        if let Err(e) = run_single_backtest_and_save(job_id, settings, job_settings, params) {
            tracing::error!(error = %e, "A single backtest run failed.");
        }
    });

    Ok(job_id)
}