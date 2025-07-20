// In crates/web-server/src/lib.rs (REPLACE ENTIRE FILE)

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State, Query, Path
    },
    response::IntoResponse,
    routing::get,
    Router,
    response::Json,
    Extension,
};
use futures::{sink::SinkExt, stream::StreamExt}; // for websocket send/receive
use database::{Db, BacktestRun, OptimizationJob, ApiTrade};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use types::{PaginatedResponse, PaginationParams, WsMessage};
use analytics::types::EquityPoint;
use app_config::types::ServerSettings; // Import the new settings
use tokio::net::TcpListener;

pub mod error;
pub mod types;

// WebSocket message replay cache type
type WsCache = Arc<Mutex<VecDeque<WsMessage>>>;

// Re-export our custom error type for convenience.
pub use error::{Error, Result};

/// The shared application state that is available to all API handlers.
///
/// It is wrapped in an `Arc` to allow for safe concurrent access.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub ws_tx: broadcast::Sender<WsMessage>, // For broadcasting live messages
    pub ws_cache: WsCache,                   // For replaying recent messages
}

const WS_CACHE_SIZE: usize = 200; // The maximum number of messages to keep in the replay cache.

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
    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any) // For development, allow any origin
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    // Define the API sub-router
    let api_router = Router::new()
        .route("/backtest-runs", get(get_backtest_runs_handler))
        // Add the new optimization routes
        .route("/optimizations", get(get_optimizations_handler))
        .route("/optimizations/{jobId}", get(get_optimization_details_handler))
        // Add the new backtest detail routes
        .route("/backtests/{runId}", get(get_backtest_details_handler))
        .route("/backtests/{runId}/trades", get(get_backtest_trades_handler))
        .route("/backtests/{runId}/equity-curve", get(get_backtest_equity_curve_handler));

    // The main router.
    Router::new()
        // Add the new WebSocket route here
        .route("/ws", get(ws_handler))
        .route("/health", get(health_check_handler))
        .nest("/api", api_router)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(cors)
        .with_state(app_state)
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
    // Pass the optional job_id to the database function
    let (runs, total_items) = state.db
        .get_backtest_runs_paginated(params.page, params.page_size, params.job_id)
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

/// Handler for `GET /api/optimizations`
async fn get_optimizations_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<OptimizationJob>>> {
    let (jobs, total_items) = state.db
        .get_optimization_jobs_paginated(params.page, params.page_size)
        .await?;

    let response = PaginatedResponse {
        items: jobs,
        total_items,
        page: params.page,
        page_size: params.page_size,
    };
    
    Ok(Json(response))
}

/// Handler for `GET /api/optimizations/:jobId`
async fn get_optimization_details_handler(
    State(state): State<AppState>,
    Path(job_id): Path<i64>, // Extractor for path parameters like {job_id}
) -> Result<Json<serde_json::Value>> {
    tracing::info!(job_id, "Fetching optimization details for job");
    
    match state.db.get_optimization_summary(job_id).await? {
        Some(summary) => {
            tracing::info!(job_id, "Found optimization summary");
            Ok(Json(summary))
        },
        None => {
            tracing::warn!(job_id, "Optimization job not found or has no summary");
            Err(Error::NotFound(format!("Optimization job {} not found or has no summary", job_id)))
        },
    }
}

/// Handler for `GET /api/backtests/:runId`
async fn get_backtest_details_handler(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
) -> Result<Json<analytics::types::PerformanceReport>> {
    match state.db.get_performance_report(run_id).await? {
        Some(report) => Ok(Json(report)),
        None => Err(Error::NotFound(format!("Backtest run {} not found", run_id))),
    }
}

/// Handler for `GET /api/backtests/:runId/trades`
async fn get_backtest_trades_handler(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<ApiTrade>>> {
    let (trades, total_items) = state.db
        .get_trades_for_run_paginated(run_id, params.page, params.page_size)
        .await?;
    
    let response = PaginatedResponse {
        items: trades,
        total_items,
        page: params.page,
        page_size: params.page_size,
    };
    Ok(Json(response))
}

/// Handler for `GET /api/backtests/:runId/equity-curve`
async fn get_backtest_equity_curve_handler(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
) -> Result<Json<Vec<EquityPoint>>> {
    let curve = state.db.get_equity_curve_for_run(run_id).await?;
    Ok(Json(curve))
}

/// The handler for `GET /ws`.
/// Upgrades the connection to a WebSocket and handles the real-time communication.
async fn ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// The actual WebSocket handling logic after the connection is upgraded.
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    tracing::info!("New WebSocket client connected.");

    // --- 1. The "Replay" ---
    // Get a lock on the cache and clone all historical messages to a local vector.
    let replay_msgs: Vec<_> = {
        let cache = state.ws_cache.lock().unwrap();
        cache.iter().cloned().collect()
    };
    for msg in replay_msgs {
        let json_msg = serde_json::to_string(&msg).unwrap();
        if socket.send(Message::Text(json_msg.into())).await.is_err() {
            // Client disconnected before replay was finished.
            tracing::info!("WebSocket client disconnected during replay.");
            return;
        }
    }

    // --- 2. "Going Live" ---
    // Subscribe to the broadcast channel to receive new, live messages.
    let mut rx = state.ws_tx.subscribe();

    // The main loop for this client.
    loop {
        tokio::select! {
            // Await a new message from the broadcast channel.
            Ok(msg) = rx.recv() => {
                // Serialize the message to JSON and send it.
                let json_msg = serde_json::to_string(&msg).unwrap();
                if socket.send(Message::Text(json_msg.into())).await.is_err() {
                    // Client disconnected. Break the loop.
                    tracing::info!("WebSocket client disconnected.");
                    break;
                }
            }
            // Await a message from the client (e.g., a ping or a command).
            Some(Ok(msg)) = socket.next() => {
                if let Message::Close(_) = msg {
                    // Client sent a close frame.
                    tracing::info!("WebSocket client sent close frame.");
                    break;
                }
                // We can handle incoming messages here if we add client-to-server commands.
            }
            // If both channels are closed, the select macro will terminate.
            else => {
                break;
            }
        }
    }
    tracing::info!("WebSocket client connection closed.");
}

/// The main entry point for running the web server.
///
/// This function sets up the TCP listener and serves the application router.
/// It will run forever until the process is terminated.
pub async fn run(settings: ServerSettings, db_pool: Db) -> Result<()> {
    // 1. Create the broadcast channel.
    //    The channel capacity should be large enough to handle bursts.
    let (ws_tx, _) = broadcast::channel(1024);

    // 2. Create the WebSocket replay cache.
    let ws_cache = Arc::new(Mutex::new(VecDeque::with_capacity(WS_CACHE_SIZE)));
    
    // 3. Create the AppState.
    let app_state = AppState {
        db: db_pool,
        ws_tx,
        ws_cache,
    };
    
    // 4. Create and run the router.
    let app = create_router(app_state);

    let address = format!("{}:{}", settings.host, settings.port);
    tracing::info!("Web server listening on {}", address);

    let listener = TcpListener::bind(&address).await.map_err(Error::ServerBindError)?;

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();

    Ok(())
}