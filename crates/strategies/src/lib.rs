// In crates/strategies/src/lib.rs (REPLACE ENTIRE FILE)

use core_types::{Kline, Signal};
pub mod ma_crossover;
pub mod error;
pub mod types;

/// The universal interface for a trading strategy.
///
/// A strategy is responsible for analyzing market data and producing a trading `Signal`.
/// It is a stateful entity, meaning it can keep track of previous data points,
/// indicator values, or its own internal state across multiple calls.
pub trait Strategy {
    /// The name of the strategy.
    fn name(&self) -> &'static str;

    fn assess(&mut self, klines: &[Kline]) -> Signal;
}