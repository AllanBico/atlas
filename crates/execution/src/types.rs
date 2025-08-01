// In crates/execution/src/types.rs

use serde::Deserialize;

#[derive(Clone)]
#[derive(Debug, Deserialize)]
pub struct SimulationSettings {
    /// The maker fee for the exchange (e.g., 0.0002 for 0.02%).
    pub maker_fee: f64,
    
    /// The taker fee for the exchange (e.g., 0.0004 for 0.04%).
    pub taker_fee: f64,
    
    /// The simulated slippage percentage for market orders (e.g., 0.0005 for 0.05%).
    pub slippage_percent: f64,
}

use core_types::{Position, Symbol};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Represents the state of the simulated trading portfolio.
#[derive(Debug)]
#[derive(Clone)]
pub struct Portfolio {
    pub initial_capital: Decimal,
    /// The total cash balance of the portfolio (e.g., in USDT).
    pub cash: Decimal,
    
    /// A map holding the currently open positions, keyed by symbol.
    pub open_positions: HashMap<Symbol, Position>,
}

impl Portfolio {
    /// Creates a new portfolio with an initial cash balance.
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            initial_capital,
            cash: initial_capital,
            open_positions: HashMap::new(),
        }
    }
}