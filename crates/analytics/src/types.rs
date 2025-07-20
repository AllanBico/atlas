// In crates/analytics/src/types.rs

use chrono::{DateTime, Utc};
use core_types::{Side, Symbol};
use rust_decimal::Decimal;
use serde::Serialize;
use serde::Deserialize;
use std::collections::HashMap;

/// A comprehensive record of a single closed trade, from entry to exit.
#[derive(Debug, Clone, Serialize)]
pub struct Trade {
    pub symbol: Symbol,
    pub side: Side,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub pnl: Decimal,
    pub fees: Decimal,
    pub signal_confidence: f64,
    pub leverage: u8,
}

/// A struct to hold a point in the portfolio's equity curve.
#[derive(Debug, Clone, Serialize)]
pub struct EquityPoint {
    pub timestamp: DateTime<Utc>,
    pub value: Decimal,
}

// This will hold the results for our confidence-bucketed analysis
pub type ConfidenceBucketPerformance = HashMap<String, PerformanceReport>;

/// A comprehensive report of a strategy's performance over a backtest period.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceReport {
    pub run_id: i64, // Add this
    // Tier 1 Metrics
    pub net_pnl_absolute: Decimal,
    pub net_pnl_percentage: f64,
    pub max_drawdown_absolute: Decimal,
    pub max_drawdown_percentage: f64,
    pub sharpe_ratio: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub total_trades: u32,

    // Tier 2 Metrics
    pub sortino_ratio: f64,
    pub calmar_ratio: f64,
    pub avg_trade_duration_secs: f64,
    pub expectancy: Decimal,

    // Tier 3 Metrics
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub confidence_performance: ConfidenceBucketPerformance,
    pub larom: f64, // Leverage-Adjusted Return on Margin
    pub funding_pnl: Decimal,
    pub drawdown_duration_secs: i64,
}

impl PerformanceReport {
    /// Creates a new, empty report with default zero/NaN values.
    pub fn new() -> Self {
        Self::default()
    }
}