// In crates/execution/src/lib.rs (REPLACE ENTIRE FILE)

use async_trait::async_trait;
use core_types::{Execution, OrderRequest};
pub mod simulated;
pub mod error;
pub mod types;

// Re-export public types
pub use error::{Error, Result};
pub use types::SimulationSettings;

/// The universal interface for an execution handler.
///
/// An `Executor` is responsible for taking a validated `OrderRequest` and
/// submitting it to a target, which could be a live exchange or a simulation engine.
/// It must handle the complexities of order submission and confirmation.
#[async_trait]
pub trait Executor {
    /// The name of the executor (e.g., "LiveBinanceExecutor", "SimulatedBacktestExecutor").
    fn name(&self) -> &'static str;

    /// Executes a given order request.
    ///
    /// This method should handle the entire lifecycle of placing an order and
    /// waiting for its confirmation.
    ///
    /// # Arguments
    ///
    /// * `order_request`: A reference to the `OrderRequest` to be executed.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `Execution` details on success, or an `Error`
    /// if the order could not be successfully executed.
    async fn execute(
        &mut self,
        order_request: &OrderRequest,
        current_price: rust_decimal::Decimal, // <-- Add this parameter
    ) -> Result<Execution>;

    fn portfolio(&self) -> &crate::types::Portfolio {
        unimplemented!("Portfolio view is not supported by this executor.")
    }
}