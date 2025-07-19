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

        /// The strategy to use for the backtest (e.g., "ma_crossover", "supertrend", "prob_reversion").
        #[arg(long)]
        strategy: String,
    },
    
    /// Runs a full parameter optimization job.
    Optimize,
}

// --- Main Application Entry Point ---

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from a .env file, if it exists.
    dotenvy::dotenv().ok();

    // Initialize the tracing subscriber for structured logging.
    tracing_subscriber::fmt::init();

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
            strategy,
        } => {
            // Call our new handler function
            handle_backtest(symbol, interval, start_date, end_date, strategy).await?;
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
async fn run_app() -> Result<()> {
    // --- Initialization ---
    let settings = app_config::load_settings()?;
    tracing::info!("Application settings loaded successfully");

    let _db_pool = database::connect(&settings.database).await?;
    tracing::info!("Database connection established and migrations are up-to-date");

    let _api_client = api_client::new(&settings.binance)?;
    tracing::info!("Binance API client created successfully");

    // --- Risk Manager Instantiation ---
    let mut risk_manager = match settings.simple_risk_manager {
        Some(risk_settings) => {
            let rm = SimpleRiskManager::new(risk_settings);
            tracing::info!(name = %rm.name(), "Initialized risk manager.");
            Box::new(rm) as Box<dyn RiskManager>
        }
        None => {
            anyhow::bail!("Fatal: No risk manager configured in settings. Exiting.");
        }
    };

    // --- Strategy Instantiation ---
    let mut active_strategies: Vec<Box<dyn Strategy>> = Vec::new();

    if let Some(ma_settings) = settings.strategies.ma_crossover {
        let ma_strategy = MACrossover::new(ma_settings);
        tracing::info!(name = %ma_strategy.name(), "Initialized strategy.");
        active_strategies.push(Box::new(ma_strategy));
    }

    if active_strategies.is_empty() {
        anyhow::bail!("No strategies are configured. The application will not run.");
    }
    
    // --- Simulated Executor Instantiation ---
    let mut executor = match settings.simulation {
        Some(sim_settings) => {
            let initial_capital = dec!(10_000.0); // Start with 10k USDT
            let exec = SimulatedExecutor::new(sim_settings, initial_capital);
            tracing::info!(name = %exec.name(), "Initialized executor.");
            Box::new(exec) as Box<dyn Executor>
        }
        None => {
            anyhow::bail!("Fatal: No simulation settings configured for run mode. Exiting.");
        }
    };

    // =========================================================================
    // --- SINGLE PIPELINE TEST (MANUAL) ---
    // This block demonstrates the full, end-to-end logic flow.
    // In the future, this will be replaced by the main application loop.
    // =========================================================================
    
    tracing::info!("--- Starting Single Pipeline Test ---");

    // 1. Get a strategy to test.
    let strategy = active_strategies.get_mut(0).unwrap();
    tracing::info!(strategy = %strategy.name(), "Selected strategy for test.");

    // 2. Manually create some kline data for the strategy to assess.
    //    (In a real loop, this data would come from the database or WebSocket).
    let test_klines: Vec<Kline> = vec![]; // For now, an empty vec is fine for a simple test.
                               // Our MA Crossover will just return Signal::Hold.
                               // Let's manually create a signal instead.

    // 3. Manually create a signal.
    let signal = Signal::GoLong { confidence: 0.85 };
    tracing::info!(?signal, "1. Strategy produced signal.");

    // 4. Pass the signal to the Risk Manager.
    //    We need the portfolio value, which we can get from our executor.
    let portfolio_value = executor.portfolio().cash;
    let open_position = executor.portfolio().open_positions.get(&Symbol("BTCUSDT".to_string()));
    // Create a dummy Kline for the manual test
    let dummy_kline = Kline {
        open_time: 0,
        open: dec!(0),
        high: dec!(0),
        low: dec!(0),
        close: dec!(0),
        volume: dec!(0),
        close_time: 0,
    };
    match risk_manager.evaluate(&signal, portfolio_value, &dummy_kline, open_position) {
        Ok(Some(order_request)) => {
            tracing::info!(?order_request, "2. Risk Manager approved and created OrderRequest.");

            // 5. Pass the OrderRequest to the Executor.
            match executor.execute(&order_request, dec!(50000.0), dummy_kline.open_time).await {
                Ok(execution) => {
                    tracing::info!(?execution, "3. Executor processed order and returned Execution.");
                }
                Err(e) => {
                    tracing::error!(error = %e, "Execution failed.");
                }
            }
            tracing::info!(portfolio = ?executor.portfolio(), "4. Final portfolio state.");
        }
        Ok(None) => {
            tracing::info!("2. Risk Manager decided no action was needed.");
        }
        Err(e) => {
            tracing::warn!(error = %e, "2. Risk Manager vetoed the signal.");
        }
    }

    tracing::info!("--- Single Pipeline Test Finished ---");

    Ok(())
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
    strategy_name: String,
) -> Result<()> {
    // --- 1. Initialization & Configuration ---
    let settings = app_config::load_settings()?;
    let symbol = Symbol(symbol_str);

    // Parse start and end dates
    let start_dt = Utc.datetime_from_str(&format!("{} 00:00:00", start_date), "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow::anyhow!("Failed to parse start date: {}", e))?;
    let end_dt = Utc.datetime_from_str(&format!("{} 23:59:59", end_date), "%Y-%m-%d %H:%M:%S")
        .map_err(|e| anyhow::anyhow!("Failed to parse end date: {}", e))?;

    tracing::info!(
        symbol = %symbol.0,
        interval,
        from = %start_date,
        to = %end_date,
        strategy = %strategy_name,
        "Setting up backtest."
    );
    
    // --- 2. Instantiate All Components ---
    
    // Instantiate Risk Manager
    let risk_manager = match settings.simple_risk_manager {
        Some(risk_settings) => Box::new(SimpleRiskManager::new(risk_settings)),
        None => anyhow::bail!("Cannot run backtest: simple_risk_manager settings are missing."),
    };

    // Instantiate Strategy
    let strategy: Box<dyn Strategy> = match strategy_name.as_str() {
        "ma_crossover" => {
            let settings = settings.strategies.ma_crossover.as_ref()
                .ok_or_else(|| anyhow::anyhow!("ma_crossover strategy settings are missing."))?;
            Box::new(MACrossover::new(settings.clone()))
        }
        "supertrend" => {
            let settings = settings.strategies.supertrend.as_ref()
                .ok_or_else(|| anyhow::anyhow!("supertrend strategy settings are missing."))?;
            Box::new(strategies::supertrend::SuperTrend::new(settings.clone()))
        }
        "prob_reversion" => {
            let settings = settings.strategies.prob_reversion.as_ref()
                .ok_or_else(|| anyhow::anyhow!("prob_reversion strategy settings are missing."))?;
            Box::new(strategies::prob_reversion::ProbReversion::new(settings.clone()))
        }
        _ => anyhow::bail!("Unknown strategy: {}", strategy_name),
    };

    // Instantiate Executor
    let mut executor = match settings.simulation {
        Some(sim_settings) => Box::new(SimulatedExecutor::new(sim_settings, dec!(10_000.0))),
        None => anyhow::bail!("Cannot run backtest: simulation settings are missing."),
    };

    // --- 3. Load Data ---
    let db = database::connect(&settings.database).await?;
    tracing::info!("Loading historical data for backtest...");
    let klines = db.get_klines_by_date_range(&symbol, &interval, start_dt, end_dt).await?;
    tracing::info!("Loaded {} klines for the specified date range.", klines.len());

    // --- 4. Setup and Run the Backtester ---
    let mut backtester = Backtester::new(
        symbol.clone(), // Clone symbol for later use
        interval.clone(), // Clone interval for later use
        strategy,
        risk_manager,
        executor,
    );

    // This now returns the final performance report, the trade log, and the equity curve
    let (report, trades, equity_curve) = backtester.run(klines).await?;

    // --- 5. Save the Results to the Database ---
    // Save the correct strategy settings
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
            &strategy_settings, // The strategy's parameters
            &report,            // The calculated performance report
        ).await?;
        tracing::info!(trade_count = trades.len(), "Saving individual trades to the database...");
        db.save_trades(run_id, &trades).await?;
        tracing::info!("Individual trades saved successfully.");
        tracing::info!(point_count = equity_curve.len(), "Saving equity curve to the database...");
        db.save_equity_curve(run_id, &equity_curve).await?;
        tracing::info!("Equity curve saved successfully.");
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