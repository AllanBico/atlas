// In crates/app-config/src/types.rs

use serde::Deserialize;

use core_types::StrategyConfig;

// Define the container for all strategy settings
#[derive(Deserialize, Debug, Default, Clone)]
pub struct StrategySettings {
    // Each strategy will have its own optional settings block
    pub ma_crossover: Option<MACrossoverSettings>,
    // In the future, we could add:
    // pub rsi_reversal: Option<RSIReversalSettings>,
    pub supertrend: Option<SuperTrendSettings>, 
    pub prob_reversion: Option<ProbReversionSettings>,
}
use strategies::types::{MACrossoverSettings, ProbReversionSettings, SuperTrendSettings};
use risk::types::SimpleRiskSettings;
// use execution::types::SimulationSettings; // Removed to break cyclic dependency

#[derive(Deserialize, Debug, Clone)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize, Debug, Clone)]
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

#[derive(Deserialize, Debug, Clone)]
pub struct AppSettings {
    /// The environment the application is running in (e.g., "development", "production").
    pub environment: String,
    /// The log level for the application.
    pub log_level: String,

    pub optimizer_cores: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BinanceSettings {
    /// The API key for Binance.
    pub api_key: String,
    /// The secret key for Binance.
    pub secret_key: String,
    /// The REST API base URL for Binance.
    pub rest_base_url: String,
    /// The WebSocket base URL for Binance.
    pub ws_base_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DatabaseSettings {
    /// The connection URL for the PostgreSQL database.
    pub url: String,
}

// --- Structs for live.toml Configuration ---

/// The top-level configuration for a live or paper trading run.
#[derive(Deserialize, Debug, Clone)]
pub struct LiveRunConfig {
    #[serde(default)] // Makes the whole section optional
    pub portfolio_risk: PortfolioRiskSettings,
    
    #[serde(rename = "pairs")]
    pub pair_configs: Vec<PairConfig>,
}

/// Portfolio-level risk management settings.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct PortfolioRiskSettings {
    #[serde(default)]
    pub daily_drawdown_percent: f64,
    #[serde(default)]
    pub daily_loss_limit_usd: f64,
}

/// Configuration for a single trading pair/asset.
#[derive(Deserialize, Debug, Clone)]
pub struct PairConfig {
    pub symbol: String,
    pub interval: String,
    #[serde(default = "default_max_loss")]
    pub max_loss_per_asset_percent: f64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    pub strategies: Vec<StrategyConfig>,
}

/// Helper functions for serde defaults
fn default_max_loss() -> f64 { 100.0 } // Default to no limit
fn default_enabled() -> bool { true }