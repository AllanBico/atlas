// In crates/web-server/src/lib.rs (REPLACE ENTIRE FILE)

use axum::{
    routing::get,
    Router,
    extract::{Query, State},
    response::Json,
};
use database::{Db, BacktestRun};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;
use types::{PaginatedResponse, PaginationParams};
use app_config::types::ServerSettings; // Import the new settings

pub mod error;
pub mod types;

// Re-export our custom error type for convenience.
pub use error::{Error, Result};

/// The shared application state that is available to all API handlers.
///
/// It is wrapped in an `Arc` to allow for safe concurrent access.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
}

// We will add the `create_router` and `run` functions in the next tasks.

/// Creates the main application router with all routes and middleware.
///
/// # Arguments
///
/// * `app_state`: The shared `AppState` containing resources like the DB pool.
///
/// # Returns
///
/// The configured `axum::Router`.
pub fn create_router(app_state: AppState) -> Router {
    // Define a CORS layer to allow requests from our frontend.
    // In a production environment, you would restrict the origin to your actual frontend domain.
    let cors = CorsLayer::new()
        .allow_origin(Any) // For development, allow any origin
        .allow_methods(Any)
        .allow_headers(Any);

    // Define the API sub-router
    let api_router = Router::new()
        .route("/backtest-runs", get(get_backtest_runs_handler));

    // The main router.
    Router::new()
        .route("/health", get(health_check_handler))
        // Nest the API router under the `/api` prefix
        .nest("/api", api_router)
        .layer(TraceLayer::new_for_http()) // Logs incoming requests and responses
        .layer(cors) // Allows cross-origin requests
        .with_state(app_state) // Makes the AppState available to all handlers
}

/// A simple health check handler.
/// Responds with a 200 OK and a JSON body.
async fn health_check_handler() -> &'static str {
    "OK"
}

/// The handler for `GET /api/backtest-runs`.
/// Fetches a paginated list of backtest runs from the database.
async fn get_backtest_runs_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<BacktestRun>>> {
    // Call our new database function with the parsed pagination parameters.
    let (runs, total_items) = state.db
        .get_backtest_runs_paginated(params.page, params.page_size)
        .await?;

    // Construct the paginated response object.
    let response = PaginatedResponse {
        items: runs,
        total_items,
        page: params.page,
        page_size: params.page_size,
    };
    
    Ok(Json(response))
}

/// The main entry point for running the web server.
///
/// This function sets up the TCP listener and serves the application router.
/// It will run forever until the process is terminated.
pub async fn run(settings: ServerSettings, db_pool: Db) -> Result<()> {
    let app_state = AppState { db: db_pool };
    let app = create_router(app_state);

    let address = format!("{}:{}", settings.host, settings.port);
    tracing::info!("Web server listening on {}", address);

    let listener = TcpListener::bind(&address).await.map_err(|e| {
        tracing::error!("Failed to bind to address {}: {}", address, e);
        // This is a custom error conversion, since TcpListener::bind doesn't return our error type.
        // We can add a new variant to our error enum for this.
        Error::ServerBindError(e)
    })?;

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap(); // `serve` can return an error, for now we unwrap.

    Ok(())
}