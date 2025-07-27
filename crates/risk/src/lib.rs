// In crates/risk/src/lib.rs (REPLACE ENTIRE FILE)

use core_types::{OrderRequest, Position, Signal, Kline};
pub mod simple_manager;

pub mod error;
pub mod types;

// Re-export public types
pub use error::{Error, Result};

/// The universal interface for a risk management module.
///
/// A `RiskManager` is responsible for evaluating a trading `Signal` against a set of
/// risk rules and, if approved, calculating the appropriate position size and creating
/// a final `OrderRequest`.
pub trait RiskManager: Sync {
    /// The name of the risk management strategy.
    fn name(&self) -> &'static str;

    /// Evaluates a signal and the current portfolio state to produce an order request.
    ///
    /// # Arguments
    ///
    /// * `signal`: The trading `Signal` produced by a strategy.
    /// * `symbol`: The symbol for which the signal was generated.
    /// * `portfolio_value`: The total value of the account.
    /// * `current_kline`: The current kline data for price information.
    /// * `open_position`: An `Option` containing the currently open position for the
    ///   signal's symbol, if one exists.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(OrderRequest))`: If the signal is approved and a new order should be placed.
    /// * `Ok(None)`: If the signal is valid but no action is required (e.g., a `Hold` signal).
    /// * `Err(Error::Vetoed)`: If the signal is rejected due to a risk rule violation.
    fn evaluate(
        &self,
        signal: &Signal,
        symbol: &core_types::Symbol,
        portfolio_value: rust_decimal::Decimal,
        current_kline: &Kline,
        open_position: Option<&Position>,
    ) -> Result<Option<OrderRequest>>;
}