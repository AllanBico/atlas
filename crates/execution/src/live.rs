// In crates/execution/src/live.rs

use crate::{Error, Executor, Result}; // Import our trait and errors
use crate::types::Portfolio;
use api_client::{ApiClient, OrderResponse};
use async_trait::async_trait;
use core_types::{Execution, OrderRequest, Position, Side};
use events::WsMessage;
use rust_decimal::prelude::*;
use tokio::sync::broadcast;

/// An executor that places real orders on the Binance exchange via the API.
///
/// This executor treats the exchange as the single source of truth for portfolio
/// state. It does not hold an in-memory portfolio, but queries the exchange
/// when necessary.
#[derive(Debug)]
pub struct LiveExecutor {
    /// The API client for communicating with Binance.
    api_client: ApiClient,
    
    /// The sender for broadcasting events to the UI.
    ws_tx: broadcast::Sender<WsMessage>,

    /// The portfolio holding the current state (cash, positions).
    /// This is kept in sync with the exchange by querying it.
    portfolio: Portfolio,
}

impl LiveExecutor {
    /// Creates a new `LiveExecutor`.
    pub fn new(
        api_client: ApiClient,
        ws_tx: broadcast::Sender<WsMessage>,
        initial_capital: Decimal,
    ) -> Self {
        Self {
            api_client,
            ws_tx,
            portfolio: Portfolio::new(initial_capital),
        }
    }

    /// Updates the portfolio state by querying the exchange.
    async fn update_portfolio_from_exchange(&mut self) -> Result<()> {
        // Query the exchange for account information
        let account_info = self.api_client.get_account_balance().await
            .map_err(|e| Error::ExecutionFailed {
                reason: format!("Failed to get account balance: {}", e),
            })?;
        
        // Update our portfolio's cash balance
        // For simplicity, we'll use the total available balance as our cash
        if let Some(available_balance) = account_info.total_available_balance {
            self.portfolio.cash = available_balance;
        } else {
            // If total_available_balance is None, fall back to margin_balance
            self.portfolio.cash = account_info.total_margin_balance;
        }

        // TODO: Update open positions from exchange data
        // This would require additional API calls to get open positions
        
        // Send a portfolio update to the UI
        let _ = self.ws_tx.send(WsMessage::PortfolioUpdate(events::WsPortfolioUpdate {
            cash: self.portfolio.cash,
            total_value: account_info.total_margin_balance, // Use margin balance as total value
            open_positions: std::collections::HashMap::new(), // TODO: Populate with actual positions
        }));

        Ok(())
    }
    
    /// A helper function to convert a Binance `OrderResponse` into our internal `Execution` struct.
    fn create_execution_from_response(
        &self,
        source_request: &OrderRequest,
        response: &OrderResponse,
    ) -> Result<Execution> {
        let total_cost = response.cum_quote;
        let quantity = response.executed_qty;

        if quantity == Decimal::ZERO {
            return Err(Error::ExecutionFailed {
                reason: "Exchange reported zero filled quantity.".to_string(),
            });
        }
        
        // Fee is not directly in the response, must be calculated or fetched from trade history.
        // For now, we'll approximate it using the taker fee from our (future) config.
        let fee = total_cost * Decimal::from_f64(0.0004).unwrap(); // Placeholder taker fee

        Ok(Execution {
            symbol: source_request.symbol.clone(),
            side: source_request.side,
            price: response.avg_price,
            quantity,
            fee,
            source_request: source_request.clone(),
        })
    }
}

#[async_trait]
impl Executor for LiveExecutor {
    fn name(&self) -> &'static str {
        "LiveExecutor"
    }

    fn portfolio(&mut self) -> &mut Portfolio {
        &mut self.portfolio
    }

    /// Executes a given order request on the live exchange.
    ///
    /// This method handles the entire lifecycle of placing an order and
    /// waiting for its confirmation.
    async fn execute(
        &mut self,
        order_request: &OrderRequest,
        _current_price: Decimal, // Not needed for live, market will determine price
        _current_time: i64,
    ) -> Result<(Execution, Option<Position>)> {
        
        let side_str = if order_request.side == Side::Long { "BUY" } else { "SELL" }; 

        // --- 1. Set Leverage --- 
        tracing::info!(symbol = %order_request.symbol.0, leverage = order_request.leverage, "Setting leverage..."); 
        self.api_client 
            .set_leverage(&order_request.symbol, order_request.leverage) 
            .await?; 

        // --- 2. Place the Main MARKET Order --- 
        tracing::info!(?order_request, "Placing main market order..."); 
        let main_order_response = self.api_client 
            .place_order( 
                &order_request.symbol, 
                side_str, 
                "MARKET", 
                Some(order_request.quantity), 
                None, // No price for market order 
                None, // No stop price for main order 
            ) 
            .await?; 
        tracing::info!(response = ?main_order_response, "Main order filled."); 

        // --- 3. Place the corresponding STOP_MARKET Order --- 
        let stop_side_str = if order_request.side == Side::Long { "SELL" } else { "BUY" }; 
        tracing::info!(sl_price = %order_request.sl_price, "Placing stop-loss order..."); 
        let stop_order_response = self.api_client 
            .place_order( 
                &order_request.symbol, 
                stop_side_str, 
                "STOP_MARKET", 
                Some(order_request.quantity), 
                None, 
                Some(order_request.sl_price), 
            ) 
            .await?; 
        tracing::info!(response = ?stop_order_response, "Stop-loss order placed."); 

        // --- 4. Construct the Execution record --- 
        let execution = self.create_execution_from_response(order_request, &main_order_response)?; 

        // --- 5. Broadcast events to the UI --- 
        // We broadcast the execution event first. 
        let _ = self.ws_tx.send(WsMessage::TradeExecuted(execution.clone())); 
        
        // Then, fetch the updated account state and broadcast it. 
        if let Ok(_account_info) = self.api_client.get_account_balance().await { 
            // TODO: Create a helper function to convert Binance's account info 
            // into our WsPortfolioUpdate message. 
            tracing::info!("Broadcasting updated portfolio state."); 
        }

        // The live executor does not manage positions, so it returns None.
        Ok((execution, None))
    }
}