// In app/src/main.rs (REPLACE ENTIRE FILE)

use anyhow::Result;
use clap::{Parser, Subcommand};
use chrono::{TimeZone, Utc};
use core_types::Symbol;
use core_types::Kline;
use risk::simple_manager::SimpleRiskManager;
use risk::RiskManager;
use strategies::ma_crossover::MACrossover;
use strategies::Strategy;
use std::time::Duration;
mod optimizer;
use tokio::time::sleep;
use execution::simulated::SimulatedExecutor;
use execution::Executor;
use rust_decimal_macros::dec; // For our test portfolio
use core_types::Signal;
use backtester::Backtester;
mod analyzer;
use crate::analyzer::RankedReport;
use crate::optimizer::{generate_generic_parameter_sets, load_optimizer_config, run_optimization};
use std::time::Instant;
use serde_json;
use tokio::task;
use tracing_subscriber::prelude::*;
use events::WsMessage;
use self::tracing_layer::WsBroadcastLayer;
use tokio::sync::broadcast;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
mod tracing_layer;
use engine::Engine; // Import our new Engine

// --- Command-Line Interface Definition ---

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = "A Binance Futures trading bot.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Runs the main trading bot logic in live or paper mode.
    Run,

    /// Backfills historical kline data from Binance.
    Backfill {
        /// The trading symbol to backfill (e.g., "BTCUSDT").
        #[arg(short, long)]
        symbol: String,

        /// The interval for the klines (e.g., "5m", "1h").
        #[arg(short, long)]
        interval: String,

        /// Optional start date for backfilling in YYYY-MM-DD format.
        #[arg(long)]
        start_date: Option<String>,
    },
    
    // Add this new subcommand
    /// Runs a historical backtest of a strategy.
    Backtest {
        /// The trading symbol to backtest (e.g., "BTCUSDT").
        #[arg(short, long)]
        symbol: String,

        /// The primary interval for the strategy (e.g., "5m", "1h").
        #[arg(short, long)]
        interval: String,

        /// The start date for the backtest in YYYY-MM-DD format.
        #[arg(long)]
        start_date: String,
        
        /// The end date for the backtest in YYYY-MM-DD format.
        #[arg(long)]
        end_date: String,
    },
    
    /// Runs a full parameter optimization job.
    Optimize,
}

// --- Main Application Entry Point ---

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from a .env file, if it exists.
    dotenvy::dotenv().ok();

    // --- WebSocket and Tracing Setup ---
    let (ws_tx, _) = broadcast::channel::<WsMessage>(1024);
    // Create the cache here
    let ws_cache = Arc::new(Mutex::new(VecDeque::with_capacity(200)));
    // Pass both to the layer
    let ws_layer = WsBroadcastLayer::new(ws_tx.clone(), ws_cache.clone());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_filter(tracing_subscriber::filter::Targets::new()
            .with_target("sqlx::query", tracing::Level::WARN) // Disable sqlx query debug logs
            .with_default(tracing::Level::INFO));
    tracing_subscriber::registry().with(fmt_layer).with(ws_layer).init();

    // Parse command-line arguments.
    let cli = Cli::parse();

    tracing::info!("Starting Atlas application");

    // Match on the parsed command and call the appropriate handler.
    match cli.command {
        Commands::Run => {
            run_app().await?;
        }
        Commands::Backfill {
            symbol,
            interval,
            start_date,
        } => {
            handle_backfill(symbol, interval, start_date).await?;
        }

        Commands::Backtest {
            symbol,
            interval,
            start_date,
            end_date,
        } => {
            handle_backtest(symbol, interval, start_date, end_date, ws_tx.clone()).await?;
        }
        Commands::Optimize => {
            handle_optimize().await?;
        }
    }

    tracing::info!("Atlas application has finished successfully.");

    Ok(())
}

// --- "Run" Subcommand Logic ---

/// The primary logic for the `run` command.
/// This function initializes all core components and starts the web server.
/// It will run indefinitely until terminated.
async fn run_app() -> Result<()> {
    // --- 1. Initialization ---
    let settings = app_config::load_settings()?;
    tracing::info!("Application settings loaded successfully.");

    let db_pool = database::connect(&settings.database).await?;
    tracing::info!("Database connection established and migrations are up-to-date.");

    // The WebSocket broadcaster is a central piece of state.
    let (ws_tx, _) = broadcast::channel::<events::WsMessage>(1024);

    // --- 2. Component Instantiation ---
    // Use a hardcoded SimulationSettings for now, as in backtest
    let dummy_settings = execution::types::SimulationSettings {
        maker_fee: 0.0,
        taker_fee: 0.0,
        slippage_percent: 0.0,
    };
    let api_client = api_client::new(&settings.binance)?;

    // Conditionally instantiate the executor based on the config flag
    let executor: Box<dyn Executor + Send> = if settings.app.live_trading_enabled {
        tracing::warn!("LIVE TRADING IS ENABLED. REAL ORDERS WILL BE PLACED.");
        Box::new(execution::live::LiveExecutor::new(
            api_client.clone(),
            ws_tx.clone(),
            dec!(1000.0), // dummy initial capital
        ))
    } else {
        Box::new(execution::simulated::SimulatedExecutor::new(
            dummy_settings.clone(),
            dec!(1000.0), // dummy initial capital
            ws_tx.clone(),
        ))
    };

    // Instantiate Risk Manager
    let risk_manager = Box::new(SimpleRiskManager::new(
        settings.simple_risk_manager.clone().unwrap(),
    ));

    // Instantiate Strategy (explicit, as in backtest)
    let strategy: Box<dyn Strategy + Send> = if let Some(settings) = settings.strategies.ma_crossover.as_ref() {
        Box::new(strategies::ma_crossover::MACrossover::new(settings.clone()))
    } else if let Some(settings) = settings.strategies.supertrend.as_ref() {
        Box::new(strategies::supertrend::SuperTrend::new(settings.clone()))
    } else if let Some(settings) = settings.strategies.prob_reversion.as_ref() {
        Box::new(strategies::prob_reversion::ProbReversion::new(settings.clone()))
    } else {
        anyhow::bail!("Cannot run: No strategies are configured in settings.");
    };

    // --- 3. Create the Trading Engine ---
    let live_config = app_config::load_live_config()?;
    
    let mut trading_engine = Engine::new(
        &live_config,
        &settings.strategies,
        settings.binance.clone(),
        db_pool.clone(),
        risk_manager,
        executor,
        ws_tx.clone(),
    );
    
    // --- 4. Launch Concurrent Tasks ---
    tracing::info!("Launching concurrent Trading Engine and Web Server tasks...");

    // Spawn the trading engine to run in its own concurrent task.
    let engine_handle = tokio::spawn(async move {
        trading_engine.run().await
    });
    
    // Run the web server in the current task.
    let server_handle = tokio::spawn(async move {
        web_server::run(settings.server, db_pool, ws_tx).await
    });

    // Use `tokio::select!` to wait for the first task to complete.
    // In a healthy state, neither should complete. If one does, it's likely an error.
    tokio::select! {
        engine_result = engine_handle => {
            tracing::error!(?engine_result, "Trading engine task has terminated unexpectedly.");
        }
        server_result = server_handle => {
            tracing::error!(?server_result, "Web server task has terminated unexpectedly.");
        }
    }

    anyhow::bail!("A critical task terminated. Shutting down.");
}

// --- "Backfill" Subcommand Logic ---

/// Handles the logic for the `backfill` subcommand.
async fn handle_backfill(
    symbol_str: String,
    interval: String,
    start_date: Option<String>,
) -> Result<()> {
    // --- 1. Initialization ---
    let settings = app_config::load_settings()?;
    let db = database::connect(&settings.database).await?;
    let api_client = api_client::new(&settings.binance)?;
    let symbol = Symbol(symbol_str);

    tracing::info!(symbol = %symbol.0, interval, "Starting backfill process.");

    // --- 2. Determine Start Time ---
    let mut current_start_time = match start_date {
        Some(date_str) => {
            let naive = chrono::NaiveDateTime::parse_from_str(&format!("{} 00:00:00", date_str), "%Y-%m-%d %H:%M:%S")
                .map_err(|e| anyhow::anyhow!("Failed to parse start date: {}", e))?;
            let dt = chrono::Utc.from_utc_datetime(&naive);
            tracing::info!("Using provided start date: {}", dt);
            Some(dt.timestamp_millis())
        }
        None => {
            tracing::info!("No start date provided. Resuming from the last saved kline.");
            None
        }
    };

    // --- 3. The Fetch-and-Save Loop ---
    loop {
        tracing::info!(?current_start_time, "Fetching batch of klines...");
        let klines = api_client
            .get_historical_klines(&symbol, &interval, current_start_time, Some(1000))
            .await?;

        if klines.is_empty() {
            tracing::info!("Reached the end of the historical data. Backfill complete.");
            break;
        }

        let kline_count = klines.len();
        let first_kline_time = Utc.timestamp_millis_opt(klines.first().unwrap().open_time).unwrap();
        let last_kline_time = Utc.timestamp_millis_opt(klines.last().unwrap().open_time).unwrap();

        tracing::info!(
            count = kline_count,
            from = %first_kline_time,
            to = %last_kline_time,
            "Received klines. Inserting into database."
        );

        db.insert_klines(&symbol, &interval, &klines).await?;
        current_start_time = Some(klines.last().unwrap().open_time + 1);
        sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}

/// Handles the logic for the `backtest` subcommand.
async fn handle_backtest(
    symbol_str: String,
    interval: String,
    start_date: String,
    end_date: String,
    ws_tx: broadcast::Sender<WsMessage>,
) -> Result<()> {
    // --- 1. Initialization & Configuration ---
    let settings = app_config::load_settings()?;
    let symbol = Symbol(symbol_str);

    // Parse start and end dates
    let start_dt = Utc.datetime_from_str(&format!("{} 00:00:00", start_date), "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow::anyhow!("Failed to parse start date: {}", e))?;
    let end_dt = Utc.datetime_from_str(&format!("{} 23:59:59", end_date), "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow::anyhow!("Failed to parse end date: {}", e))?;

    // --- 2. Instantiate All Components ---
    let risk_manager = match settings.simple_risk_manager {
        Some(risk_settings) => Box::new(SimpleRiskManager::new(risk_settings)) as Box<dyn RiskManager + Send>,
        None => anyhow::bail!("Cannot run backtest: simple_risk_manager settings are missing."),
    };

    // Pick the first available strategy from config
    let (strategy_name, strategy): (String, Box<dyn Strategy + Send>) = if let Some(settings) = settings.strategies.ma_crossover.as_ref() {
        ("ma_crossover".to_string(), Box::new(MACrossover::new(settings.clone())))
    } else if let Some(settings) = settings.strategies.supertrend.as_ref() {
        ("supertrend".to_string(), Box::new(strategies::supertrend::SuperTrend::new(settings.clone())))
    } else if let Some(settings) = settings.strategies.prob_reversion.as_ref() {
        ("prob_reversion".to_string(), Box::new(strategies::prob_reversion::ProbReversion::new(settings.clone())))
    } else {
        anyhow::bail!("No strategy is configured in the config file.");
    };

    // In handle_backtest, replace settings.simulation usage with a placeholder
    let dummy_settings = execution::types::SimulationSettings {
        maker_fee: 0.0,
        taker_fee: 0.0,
        slippage_percent: 0.0,
    };
    let mut executor = Box::new(SimulatedExecutor::new(dummy_settings, dec!(10_000.0), ws_tx.clone())) as Box<dyn Executor + Send>;

    // --- 3. Load Data ---
    let db = database::connect(&settings.database).await?;
    tracing::info!("Loading historical data for backtest...");
    let klines = db.get_klines_by_date_range(&symbol, &interval, start_dt, end_dt).await?;
    tracing::info!("Loaded {} klines for the specified date range.", klines.len());

    // --- 4. Setup and Run the Backtester ---
    let mut backtester = Backtester::new(
        symbol.clone(),
        interval.clone(),
        strategy,
        risk_manager,
        executor,
    );

    let (report, trades, equity_curve) = backtester.run(klines).await?;

    // --- 5. Save the Results to the Database ---
    let strategy_settings_json = match strategy_name.as_str() {
        "ma_crossover" => settings.strategies.ma_crossover.as_ref().map(|s| serde_json::to_value(s).unwrap()),
        "supertrend" => settings.strategies.supertrend.as_ref().map(|s| serde_json::to_value(s).unwrap()),
        "prob_reversion" => settings.strategies.prob_reversion.as_ref().map(|s| serde_json::to_value(s).unwrap()),
        _ => None,
    };
    if let Some(strategy_settings) = strategy_settings_json {
        tracing::info!("Saving backtest report to the database...");
        let run_id = db.save_backtest_report(
            None, // job_id
            &strategy_name,
            &symbol,
            &interval,
            start_dt,
            end_dt,
            &strategy_settings,
            &report,
        ).await?;
        tracing::info!(trade_count = trades.len(), "Saving individual trades to the database...");
        db.save_trades(run_id, &trades).await?;
        tracing::info!("Individual trades saved successfully.");
        db.save_equity_curve(run_id, &equity_curve).await?;
        tracing::info!(run_id, "Backtest run and all associated data saved.");
    } else {
        tracing::warn!("Could not find strategy settings to save with the report.");
    }

    Ok(())
}

/// Handles the logic for the `optimize` subcommand.
async fn handle_optimize() -> Result<()> {
    // ... load configs and generate param_sets (this is fast) ...
    let start_time = Instant::now();
    tracing::info!("Starting optimization job...");

    let optimizer_config = load_optimizer_config()?;
    let app_settings = app_config::load_settings()?.app;
    let param_sets = generate_generic_parameter_sets(&optimizer_config)?;
    if param_sets.is_empty() {
        anyhow::bail!("No valid parameter sets were generated.");
    }
    
    tracing::info!("Starting optimization with {} parameter sets", param_sets.len());

    // Create the DB connection and job ID in the async context
    let db = database::connect(&app_config::load_settings()?.database).await?;
    let job_id = db.create_optimization_job(&optimizer_config.job.name).await?;
    tracing::info!(job_id, "Created parent optimization job.");

    // Now, move the heavy, parallel work to a blocking thread.
    task::spawn_blocking(move || {
        run_optimization(&app_settings, &optimizer_config.job, param_sets, job_id)
    }).await??;
    
    // 3. Analyze the results (this is fast, can be done on the main thread).
    let db = database::connect(&app_config::load_settings()?.database).await?;
    let ranked_results = analyzer::analyze_and_rank_results(&db, job_id).await?;

    print_optimization_report(&ranked_results);
    
    tracing::info!(duration = ?start_time.elapsed(), "Optimization job and analysis finished.");
    Ok(())
}

/// Helper function to print the final optimization summary.
fn print_optimization_report(results: &[RankedReport]) {
    println!("\n--- Optimization Job Complete ---");
    println!("---------------------------------");
    println!("Top 5 Parameter Sets by Score:");
    println!("---------------------------------");

    for (i, ranked_report) in results.iter().take(5).enumerate() {
        println!("\n[Rank {} | Score: {:.2}]", i + 1, ranked_report.score);
        println!("  - Parameters: {}", serde_json::to_string_pretty(&ranked_report.report.parameters).unwrap_or_default());
        
        let report = &ranked_report.report.report;
        println!("  - P&L: ${:.2} ({:.2}%) | Max Drawdown: {:.2}% | Sharpe: {:.2} | Trades: {}",
            report.net_pnl_absolute,
            report.net_pnl_percentage,
            report.max_drawdown_percentage,
            report.sharpe_ratio,
            report.total_trades
        );
    }
    println!("\n---------------------------------");
    
    if let Some(best) = results.first() {
        println!("Recommendation: The parameter set with the highest score is:");
        println!("  {}", serde_json::to_string_pretty(&best.report.parameters).unwrap_or_default());
    } else {
        println!("Recommendation: No parameter sets passed the minimum threshold.");
    }
}