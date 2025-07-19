// In crates/app-config/src/types.rs

use serde::Deserialize;
// Import the settings struct from our strategies crate
use strategies::types::{MACrossoverSettings, ProbReversionSettings, SuperTrendSettings};
use risk::types::SimpleRiskSettings;
use execution::types::SimulationSettings;

#[derive(Deserialize, Debug)]
pub struct Settings {
    /// The application's general settings.
    pub app: AppSettings,
    /// Settings for the Binance API.
    pub binance: BinanceSettings,
    /// Settings for the database connection.
    pub database: DatabaseSettings,
    
    // Add this new optional field for strategy configurations
    #[serde(default)]
    pub strategies: StrategySettings,

    // Add this new optional field for the simulation settings
    pub simulation: Option<SimulationSettings>,

    pub simple_risk_manager: Option<SimpleRiskSettings>,
}

#[derive(Deserialize, Debug)]
pub struct AppSettings {
    /// The environment the application is running in (e.g., "development", "production").
    pub environment: String,
    /// The log level for the application.
    pub log_level: String,

    pub optimizer_cores: u32,
}

#[derive(Deserialize, Debug)]
pub struct BinanceSettings {
    /// The API key for Binance.
    pub api_key: String,
    /// The secret key for Binance.
    pub secret_key: String,
}

#[derive(Deserialize, Debug)]
pub struct DatabaseSettings {
    /// The connection URL for the PostgreSQL database.
    pub url: String,
}

// Define the container for all strategy settings
#[derive(Deserialize, Debug, Default)]
pub struct StrategySettings {
    // Each strategy will have its own optional settings block
    pub ma_crossover: Option<MACrossoverSettings>,
    // In the future, we could add:
    // pub rsi_reversal: Option<RSIReversalSettings>,
    pub supertrend: Option<SuperTrendSettings>, 
    pub prob_reversion: Option<ProbReversionSettings>,
}