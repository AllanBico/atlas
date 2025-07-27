// In crates/app-config/src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    // Replace the Placeholder with a specific error
    #[error("Failed to load configuration")]
    LoadError(#[from] config::ConfigError),
    
    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Failed to parse TOML: {0}")]
    TomlError(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, Error>;