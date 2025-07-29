// In crates/execution/src/live.rs
use crate::{Error, Executor, Result}; 
use api_client::ApiClient;
use async_trait::async_trait;
use core_types::{Execution, OrderRequest, Position};
use events::WsMessage;
use num_traits::FromPrimitive;
use tokio::sync::broadcast;

/// An executor that places real orders on the Binance exchange.
///
/// This executor interacts directly with the `ApiClient` to send signed
/// requests for setting leverage and placing market orders.
#[derive(Debug, Clone)]
pub struct LiveExecutor {
    /// The API client for communicating with Binance.
    api_client: ApiClient,
    
    /// The sender for broadcasting events to the UI.
    ws_tx: broadcast::Sender<WsMessage>,

    // Portfolio is now passed in via the execute method
    // and managed by the Engine
}

impl LiveExecutor {
    /// Creates a new `LiveExecutor`.
    ///
    /// # Arguments
    ///
    /// * `api_client`: The Binance API client
    /// * `ws_tx`: The broadcast channel for WebSocket messages
    /// * `initial_capital`: The starting cash balance for the portfolio (for trait compatibility)
    pub fn new(
        api_client: ApiClient,
        ws_tx: broadcast::Sender<WsMessage>,
    ) -> Self {
        Self {
            api_client,
            ws_tx,
        }
    }
}

#[async_trait]
impl Executor for LiveExecutor {
    fn name(&self) -> &'static str {
        "LiveExecutor"
    }

    async fn execute(
        &mut self,
        order_request: &OrderRequest,
        _current_price: rust_decimal::Decimal, // Ignored, as we get the real fill price
        _current_time: i64, // Ignored, as the exchange provides timestamps
        _portfolio: &mut crate::types::Portfolio, // Portfolio is now passed in
    ) -> Result<(Execution, Option<Position>)> {
        tracing::info!(?order_request, "Executing live order request...");

        // --- Step 1: Set Leverage ---
        // We set leverage before every trade to ensure it's correct.
        if let Err(e) = self.api_client.set_leverage(&order_request.symbol, order_request.leverage).await {
            tracing::error!(error = %e, "Failed to set leverage. Aborting trade.");
            // We return a custom, more descriptive error.
            return Err(Error::ExecutionFailed { reason: format!("Failed to set leverage: {}", e) });
        }
        tracing::info!(leverage = order_request.leverage, "Leverage set successfully.");

        // --- Step 2: Place the Market Order ---
        let order_response = match self.api_client.place_market_order(
            &order_request.symbol,
            &order_request.side,
            order_request.quantity,
        ).await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(error = %e, "Failed to place market order.");
                return Err(Error::ExecutionFailed { reason: format!("Failed to place order: {}", e) });
            }
        };
        tracing::info!(?order_response, "Market order placed and filled successfully.");

        // --- Step 3: Create the Execution Record from the REAL Fill Data ---
        // We use the `avgPrice` and `executedQty` from the exchange response, which is the source of truth.
        let execution_fee = (order_response.cum_quote / rust_decimal::Decimal::from(order_request.leverage)) * rust_decimal::Decimal::from_f64(0.0004).unwrap();
        let execution = Execution {
            symbol: order_request.symbol.clone(),
            side: order_request.side,
            price: order_response.avg_price,
            quantity: order_response.executed_qty,
            fee: execution_fee, // Approximate fee calculation for now
            source_request: order_request.clone(),
        };

        // --- Step 4: Broadcast Events ---
        let _ = self.ws_tx.send(WsMessage::TradeExecuted(execution.clone()));
        // In the future, after this trade, the State Reconciler would fetch the new portfolio
        // state and broadcast a `WsPortfolioUpdate`. For now, we can't create one.

        // --- Step 5: Return Result ---
        // For a live executor, we don't manage the closing of positions internally.
        // The exchange handles this. So we return `None` for the closed position.
        // The State Reconciler will be the one to confirm the position is gone.
        Ok((execution, None))
    }
}