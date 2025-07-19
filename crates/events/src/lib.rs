// --- WebSocket Message Structures (moved from web-server) ---

use serde::Serialize;
use chrono::{DateTime, Utc};
use core_types::{Execution, Position};
use rust_decimal::Decimal;
use std::collections::HashMap;

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

/// The top-level WebSocket message enum.
/// `tag` and `content` are used by serde for clean JSON representation.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    Log(WsLogMessage),
    PortfolioUpdate(WsPortfolioUpdate),
    TradeExecuted(Execution), // We can reuse our core `Execution` type
}
