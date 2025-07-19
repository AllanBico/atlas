// In app/src/optimizer.rs

use serde::Deserialize;
use strategies::types::{MACrossoverSettings, SuperTrendSettings, ProbReversionSettings};
use std::fs;
use anyhow::{Context, Result};
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
use chrono::Utc;
use chrono::TimeZone;
use std::any::Any;
use toml::Value;

// --- Structs for deserializing optimizer.toml ---

#[derive(Deserialize, Debug)]
pub struct OptimizerConfig {
    pub job: JobSettings,
    
    // Using `flatten` tells serde to collect all other top-level tables
    // from the TOML file into this HashMap. The key will be the table name
    // (e.g., "ma_crossover_params") and the value will be the raw TOML table.
    #[serde(flatten)]
    pub strategy_params: std::collections::HashMap<String, toml::Value>,
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
    Range { cle: u32, end: u32, step: Option<u32> },
}

// #[derive(Deserialize, Debug)]
// pub struct MaCrossoverParams {
//     m5_fast_period: ParamValue,
//     m5_slow_period: ParamValue,
//     h1_fast_period: ParamValue,
//     h1_slow_period: ParamValue,
//     confidence: f64, // Keep confidence fixed for now
// }

// --- Public API for the Optimizer Module ---

pub fn load_optimizer_config() -> Result<OptimizerConfig> {
    let content = fs::read_to_string("config/optimizer.toml")
        .context("Failed to read config/optimizer.toml")?;
    toml::from_str(&content).context("Failed to parse optimizer.toml")
}

pub fn generate_generic_parameter_sets(config: &OptimizerConfig) -> anyhow::Result<Vec<Box<dyn Any + Send + Sync>>> {
    // 1. Dynamically find the correct parameter table to use.
    let strategy_key = format!("{}_params", config.job.strategy_to_optimize);
    
    let params_value = config.strategy_params
        .get(&strategy_key)
        .ok_or_else(|| anyhow::anyhow!(
            "Parameter table '{}' not found in optimizer.toml. Available tables are: {:?}",
            strategy_key,
            config.strategy_params.keys()
        ))?;

    let params_table = params_value.as_table().ok_or_else(|| anyhow::anyhow!("'{}' must be a TOML table.", strategy_key))?;

    // Helper to expand a ParamValue (int or float) into a Vec of numbers
    fn expand_value(value: &Value) -> Vec<Value> {
        if let Some(table) = value.as_table() {
            if let (Some(start), Some(end)) = (table.get("start"), table.get("end")) {
                let step = table.get("step").and_then(|v| v.as_float()).unwrap_or(1.0);
                let start = start.as_float().unwrap();
                let end = end.as_float().unwrap();
                let mut vals = vec![];
                let mut v = start;
                while v <= end + 1e-8 {
                    vals.push(Value::Float(v));
                    v += step;
                }
                return vals;
            }
        }
        vec![value.clone()]
    }

    // Build all combinations
    let mut keys = vec![];
    let mut value_lists = vec![];
    for (k, v) in params_table.iter() {
        keys.push(k.clone());
        value_lists.push(expand_value(v));
    }
    let mut final_sets = vec![];
    let mut final_tables = vec![];
    let mut indices = vec![0; value_lists.len()];
    loop {
        let mut table = toml::map::Map::new();
        for (i, k) in keys.iter().enumerate() {
            table.insert(k.clone(), value_lists[i][indices[i]].clone());
        }
        final_tables.push(table);
        // Increment indices
        let mut idx = value_lists.len();
        while idx > 0 {
            idx -= 1;
            indices[idx] += 1;
            if indices[idx] < value_lists[idx].len() {
                break;
            } else {
                indices[idx] = 0;
            }
        }
        if idx == 0 && indices[0] == 0 {
            break;
        }
    }
    // The key part that makes it generic is the `match` statement at the end:
    for final_table in final_tables {
        match config.job.strategy_to_optimize.as_str() {
            "ma_crossover" => {
                let settings: MACrossoverSettings = Value::Table(final_table).try_into()?;
                final_sets.push(Box::new(settings) as Box<dyn Any + Send + Sync>);
            },
            "supertrend" => {
                let settings: SuperTrendSettings = Value::Table(final_table).try_into()?;
                final_sets.push(Box::new(settings) as Box<dyn Any + Send + Sync>);
            },
            "prob_reversion" => {
                let settings: ProbReversionSettings = Value::Table(final_table).try_into()?;
                final_sets.push(Box::new(settings) as Box<dyn Any + Send + Sync>);
            }
            _ => anyhow::bail!("Unknown strategy '{}' in optimizer config", config.job.strategy_to_optimize),
        }
    }
    Ok(final_sets)
}

fn run_single_backtest_and_save(
    job_id: i64,
    main_settings: &app_config::Settings,
    job_settings: &JobSettings,
    strategy_name: &str,
    param: &Box<dyn Any + Send + Sync>,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let db = database::connect(&main_settings.database).await?;
        let symbol = Symbol(job_settings.symbol.clone());
        let interval = job_settings.interval.clone();
        let risk_manager = Box::new(SimpleRiskManager::new(main_settings.simple_risk_manager.clone().unwrap()));
        let dummy_settings = execution::types::SimulationSettings {
            maker_fee: 0.0,
            taker_fee: 0.0,
            slippage_percent: 0.0,
        };
        let (dummy_ws_tx, _) = tokio::sync::broadcast::channel(1);
        let executor = Box::new(SimulatedExecutor::new(dummy_settings, dec!(10_000.0), dummy_ws_tx));

        // Instantiate the correct strategy based on strategy_name and param type
        let strategy: Box<dyn strategies::Strategy> = match strategy_name {
            "ma_crossover" => {
                let settings = param.downcast_ref::<MACrossoverSettings>().ok_or_else(|| anyhow::anyhow!("Failed to downcast to MACrossoverSettings"))?;
                Box::new(MACrossover::new(settings.clone()))
            },
            "supertrend" => {
                let settings = param.downcast_ref::<SuperTrendSettings>().ok_or_else(|| anyhow::anyhow!("Failed to downcast to SuperTrendSettings"))?;
                Box::new(strategies::supertrend::SuperTrend::new(settings.clone()))
            },
            "prob_reversion" => {
                let settings = param.downcast_ref::<ProbReversionSettings>().ok_or_else(|| anyhow::anyhow!("Failed to downcast to ProbReversionSettings"))?;
                Box::new(strategies::prob_reversion::ProbReversion::new(settings.clone()))
            },
            _ => anyhow::bail!("Unknown strategy '{}' in optimizer config", strategy_name),
        };

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
        if let Ok((report, trades, equity_curve)) = backtester.run(klines).await {
            // Save the parameters as JSON (downcast to correct type)
            match strategy_name {
                "ma_crossover" => {
                    let settings = param.downcast_ref::<MACrossoverSettings>().unwrap();
                    let run_id = db.save_backtest_report(Some(job_id), strategy_name, &symbol, &interval, start_dt, end_dt, settings, &report).await?;
                    db.save_trades(run_id, &trades).await?;
                    db.save_equity_curve(run_id, &equity_curve).await?;
                    tracing::info!(run_id, "Saved results.");
                },
                "supertrend" => {
                    let settings = param.downcast_ref::<SuperTrendSettings>().unwrap();
                    let run_id = db.save_backtest_report(Some(job_id), strategy_name, &symbol, &interval, start_dt, end_dt, settings, &report).await?;
                    db.save_trades(run_id, &trades).await?;
                    db.save_equity_curve(run_id, &equity_curve).await?;
                    tracing::info!(run_id, "Saved results.");
                },
                "prob_reversion" => {
                    let settings = param.downcast_ref::<ProbReversionSettings>().unwrap();
                    let run_id = db.save_backtest_report(Some(job_id), strategy_name, &symbol, &interval, start_dt, end_dt, settings, &report).await?;
                    db.save_trades(run_id, &trades).await?;
                    db.save_equity_curve(run_id, &equity_curve).await?;
                    tracing::info!(run_id, "Saved results.");
                },
                _ => {}
            }
        }
        Ok(())
    })
}

/// The main parallel engine for running an optimization job.
pub fn run_optimization(
    app_settings: &AppSettings,
    job_settings: &JobSettings,
    param_sets: Vec<Box<dyn Any + Send + Sync>>,
    job_id: i64,
) -> Result<i64> {
    tracing::info!(cores = app_settings.optimizer_cores, "Configuring Rayon thread pool.");
    ThreadPoolBuilder::new()
        .num_threads(app_settings.optimizer_cores as usize)
        .build_global()
        .context("Failed to build Rayon thread pool")?;
    let shared_settings = Arc::new(app_config::load_settings()?);
    let strategy_name = job_settings.strategy_to_optimize.clone();
    param_sets.par_iter().for_each_with(shared_settings, |settings, param| {
        if let Err(e) = run_single_backtest_and_save(job_id, settings, job_settings, &strategy_name, param) {
            tracing::error!(error = %e, "A single backtest run failed.");
        }
    });
    Ok(job_id)
}