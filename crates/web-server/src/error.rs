// In crates/config/src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("A placeholder error for the config module")]
    Placeholder,
}
pub type Result<T> = std::result::Result<T, Error>;