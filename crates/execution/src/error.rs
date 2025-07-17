// In crates/execution/src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Execution failed: {reason}")]
    ExecutionFailed { reason: String },
    
    // We can add more specific variants later, e.g., for different exchange rejection reasons.
}

pub type Result<T> = std::result::Result<T, Error>;