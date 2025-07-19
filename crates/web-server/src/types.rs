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
}

// Helper functions for serde defaults.
fn default_page() -> u32 { 1 }
fn default_page_size() -> u32 { 50 }