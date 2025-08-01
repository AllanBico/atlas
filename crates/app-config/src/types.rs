// In crates/app-config/src/types.rs

use serde::Deserialize;
// Import the settings struct from our strategies crate
use strategies::types::{MACrossoverSettings, ProbReversionSettings, SuperTrendSettings};
use risk::types::SimpleRiskSettings;
// use execution::types::SimulationSettings; // Removed to break cyclic dependency

#[derive(Deserialize, Debug)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize, Debug)]
pub struct Settings {
    /// The application's general settings.
    pub app: AppSettings,
    /// Settings for the Binance API.
    pub binance: BinanceSettings,
    /// Settings for the database connection.
    pub database: DatabaseSettings,
    pub server: ServerSettings,
    // Add this new optional field for strategy configurations
    #[serde(default)]
    pub strategies: StrategySettings,

    // pub simulation: Option<SimulationSettings>, // Removed to break cyclic dependency

    pub simple_risk_manager: Option<SimpleRiskSettings>,
}

#[derive(Deserialize, Debug)]
pub struct AppSettings {
    /// The environment the application is running in (e.g., "development", "production").
    pub environment: String,
    /// The log level for the application.
    pub log_level: String,

    pub optimizer_cores: u32,
    #[serde(default)] // This makes the field optional, defaulting to `false`
    pub live_trading_enabled: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BinanceSettings {
    /// The API key for Binance.
    pub api_key: String,
    /// The secret key for Binance.
    pub secret_key: String,
    pub rest_base_url: String, // <-- It gets loaded into this field
    pub ws_base_url: String,
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

#[derive(Deserialize, Debug, Clone)]
pub struct LiveConfig {
    // The `serde(default)` allows the file to be empty without crashing.
    #[serde(default)]
    pub bot: Vec<BotConfig>,
}

/// Represents the configuration for a single trading bot instance.
#[derive(Deserialize, Debug, Clone)]
pub struct BotConfig {
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    pub symbol: String,
    pub interval: String,
    pub strategy_name: String,
    pub strategy_params: String, // The key to look up in StrategySettings
}

// Helper for serde to default `enabled` to true if missing.
fn default_as_true() -> bool {
    true
}