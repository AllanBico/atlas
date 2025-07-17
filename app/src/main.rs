// In app/src/main.rs (REPLACE ENTIRE FILE)

use anyhow::Result;
use clap::{Parser, Subcommand};
use chrono::{TimeZone, Utc};
use core_types::Symbol;
use risk::simple_manager::SimpleRiskManager;
use risk::RiskManager;
use strategies::ma_crossover::MACrossover;
use strategies::Strategy;
use std::time::Duration;
use tokio::time::sleep;

// --- Command-Line Interface Definition ---

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = "A Binance Futures trading bot.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Runs the main trading bot logic.
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

    let db_pool = database::connect(&settings.database).await?;
    tracing::info!("Database connection established and migrations are up-to-date");

    let api_client = api_client::new(&settings.binance)?;
    tracing::info!("Binance API client created successfully");

    // --- Risk Manager Instantiation ---
    let risk_manager = match settings.simple_risk_manager {
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
        tracing::warn!("No strategies are configured. The application will idle.");
    }

    // --- Placeholder for the main application loop ---
    // The main trading logic will eventually go here.
    // For now, we will just fetch the account balance as a final check.

    match api_client.get_account_balance().await {
        Ok(account_info) => {
            if let Some(usdt_balance) = account_info.assets.iter().find(|asset| asset.asset == "USDT") {
                tracing::info!(
                    "Successfully fetched account balance. Available USDT: {}",
                    usdt_balance.available_balance
                );
            } else {
                tracing::warn!("USDT balance not found in futures account assets.");
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch account balance on startup");
            return Err(e.into());
        }
    }

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
            let dt = chrono::DateTime::<chrono::Utc>::from_utc(naive, chrono::Utc);
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