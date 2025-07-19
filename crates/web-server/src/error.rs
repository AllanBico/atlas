// In crates/web-server/src/error.rs

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    DatabaseError(#[from] database::Error),

    #[error("Failed to bind server to address")]
    ServerBindError(#[from] std::io::Error),

    // Add other web-specific errors here in the future
}

pub type Result<T> = std::result::Result<T, Error>;

// This allows us to convert our custom Error into a proper HTTP response.
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Error::DatabaseError(e) => {
                // Log the full error for debugging
                tracing::error!("Database error occurred: {:?}", e);
                // Return a generic error to the client for security
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal database error occurred".to_string(),
                )
            }
            Error::ServerBindError(e) => {
                tracing::error!("Server bind error occurred: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to bind server to address".to_string(),
                )
            }
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}