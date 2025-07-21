// In crates/web-server/src/types.rs

use serde::{Deserialize, Serialize};

/// Represents a paginated list of items.
/// This is a generic struct that can be used for any paginated API response.
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total_items: i64,
    pub page: u32,
    pub page_size: u32,
}

/// Represents the pagination query parameters from the URL (e.g., ?page=1&pageSize=50).
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    // `serde(default = ...)` provides a default value if the param is missing.
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    // Add this optional filter
    pub job_id: Option<i64>,
}

// Helper functions for serde defaults.
fn default_page() -> u32 { 1 }
fn default_page_size() -> u32 { 50 }

use analytics::types::{PerformanceReport, Trade}; // For future use
use chrono::{DateTime, Utc};
use core_types::{Execution, Position};
use rust_decimal::Decimal;
use std::collections::HashMap;



// --- WebSocket Message Structures ---

/// Represents a log message event to be sent to the UI.
#[derive(Debug, Clone, Serialize)]
pub struct WsLogMessage {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
}

/// Represents the full, updated state of the portfolio.
#[derive(Debug, Clone, Serialize)]
pub struct WsPortfolioUpdate {
    pub cash: Decimal,
    pub total_value: Decimal, // cash + value of open positions
    pub open_positions: HashMap<String, Position>, // Keyed by symbol string for easy JS access
}