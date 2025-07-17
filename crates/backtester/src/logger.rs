// In crates/backtester/src/logger.rs

use analytics::types::{EquityPoint, Trade};
use chrono::{DateTime, Utc};
use core_types::{Execution, Position};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use core_types::{Side, Signal};
use chrono::TimeZone;

/// A logger responsible for recording trades and equity changes during a backtest.
#[derive(Debug)]
pub struct TradeLogger {
    pub trades: Vec<Trade>,
    pub equity_curve: Vec<EquityPoint>,
}

impl TradeLogger {
    /// Creates a new, empty logger.
    pub fn new() -> Self {
        Self {
            trades: Vec::new(),
            equity_curve: Vec::new(),
        }
    }

    /// Records a point in the equity curve.
    pub fn record_equity(&mut self, timestamp: DateTime<Utc>, value: Decimal) {
        self.equity_curve.push(EquityPoint { timestamp, value });
    }

    /// Records a completed trade by combining the entry position and the closing execution.
    pub fn record_trade(&mut self, open_pos: &Position, close_exec: &Execution, exit_time: DateTime<Utc>) {
        // Calculate total fees (assuming entry fee was already deducted from cash)
        let fees = (open_pos.quantity * open_pos.entry_price * dec!(0.0004)) + close_exec.fee; // Placeholder taker fee

        // Calculate PnL
        let pnl = (close_exec.price - open_pos.entry_price)
            * open_pos.quantity
            * (if open_pos.side == Side::Long { dec!(1) } else { dec!(-1) });

        // Extract confidence from the originating signal
        let confidence = match close_exec.source_request.originating_signal {
            Signal::GoLong { confidence } | Signal::GoShort { confidence } => confidence,
            _ => 0.0, // Default for system-generated closes (e.g., SL)
        };

        let trade = Trade {
            symbol: open_pos.symbol.clone(),
            side: open_pos.side,
            entry_time: Utc.timestamp_millis_opt(open_pos.entry_time).unwrap(), // Position needs an entry_time!
            exit_time,
            entry_price: open_pos.entry_price,
            exit_price: close_exec.price,
            quantity: open_pos.quantity,
            pnl,
            fees,
            signal_confidence: confidence,
            leverage: open_pos.leverage,
        };

        self.trades.push(trade);
    }
}