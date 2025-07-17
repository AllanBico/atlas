// In crates/api-client/src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to build the API client: {0}")]
    ClientBuildError(String),
    #[error("API client error: {0}")]
    CustomError(String),
    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(#[from] serde_json::Error),
    #[error("API error: code {code}, msg: {msg}")]
    ApiError { code: i64, msg: String },
}

pub type Result<T> = std::result::Result<T, Error>;