// In crates/risk/src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Trade signal was vetoed by risk manager: {reason}")]
    Vetoed { reason: String },

    #[error("Invalid risk parameters: {0}")]
    InvalidParameters(String),
}

pub type Result<T> = std::result::Result<T, Error>;