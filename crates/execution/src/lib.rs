// In crates/execution/src/lib.rs (REPLACE ENTIRE FILE)

use async_trait::async_trait;
use core_types::{Execution, OrderRequest, Position};
pub mod simulated;
pub mod error;
pub mod types;
pub mod live; 
// Re-export public types
pub use error::{Error, Result};
pub use types::{SimulationSettings, Portfolio};

/// The universal interface for an execution handler.
///
/// An `Executor` is responsible for taking a validated `OrderRequest` and
/// submitting it to a target, which could be a live exchange or a simulation engine.
/// It must handle the complexities of order submission and confirmation.
#[async_trait]
pub trait Executor: Sync {
    /// The name of the executor (e.g., "LiveBinanceExecutor", "SimulatedBacktestExecutor").
    fn name(&self) -> &'static str;

    /// Executes a given order request against the provided portfolio.
    ///
    /// This method should handle the entire lifecycle of placing an order and
    /// waiting for its confirmation.
    ///
    /// # Arguments
    ///
    /// * `order_request`: A reference to the `OrderRequest` to be executed.
    /// * `current_price`: The current market price for the asset.
    /// * `current_time`: The current timestamp for the execution.
    /// * `portfolio`: A mutable reference to the portfolio to execute the order against.
    ///
    /// # Returns
    ///
    /// A `Result` containing a tuple of:
    /// - The `Execution` details on success
    /// - An optional `Position` if a position was closed
    /// - Or an `Error` if the order could not be successfully executed.
    async fn execute(
        &mut self,
        order_request: &OrderRequest,
        current_price: rust_decimal::Decimal,
        current_time: i64,
        portfolio: &mut Portfolio,
    ) -> Result<(Execution, Option<Position>)>;
}