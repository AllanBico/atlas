// In crates/app-config/src/lib.rs

use config::{Config, File, Environment};

pub mod error;
pub mod types;

// Re-export the most important types for easy access.
pub use error::{Error, Result};
pub use types::{Settings, LiveConfig};

/// Loads the application settings from various sources.
///
/// This function orchestrates the layered configuration loading:
/// 1. Reads from a default `base.toml` file.
/// 2. Merges settings from an environment-specific file (e.g., `development.toml`).
/// 3. Merges settings from environment variables.
pub fn load_settings() -> Result<Settings> {
    // Get the current environment. Default to "development" if not set.
    let environment = std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "development".into());

    let settings = Config::builder()
        // 1. Load the base configuration file.
        .add_source(File::with_name("config/base"))
        // 2. Load the environment-specific configuration file.
        .add_source(File::with_name(&format!("config/{}", environment)).required(false))
        // 3. Load settings from environment variables (e.g., `APP_DATABASE__URL=...`).
        // The prefix is `APP`, separator is `__`.
        .add_source(Environment::with_prefix("APP").separator("__"))
        .build()?;

    // Deserialize the configuration into our `Settings` struct.
    let settings: Settings = settings.try_deserialize()?;

    Ok(settings)
}

/// Loads the live trading portfolio configuration from `live.toml`.
pub fn load_live_config() -> Result<LiveConfig> {
    let content = std::fs::read_to_string("config/live.toml")?;
    
    let config: LiveConfig = toml::from_str(&content)?;
    Ok(config)
}