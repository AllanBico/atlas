// In crates/engine/src/reconciler.rs

use api_client::ApiClient;
use core_types::{Position, Side, Symbol};
use execution::types::Portfolio;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use tokio::time::interval;

/// A background task that periodically reconciles the bot's internal state
/// with the actual state reported by the exchange.
pub struct StateReconciler {
    /// The API client for communicating with Binance.
    api_client: ApiClient,
    
    /// A shared, thread-safe reference to the executor's portfolio.
    portfolio: Arc<Mutex<Portfolio>>,
}

impl StateReconciler {
    pub fn new(api_client: ApiClient, portfolio: Arc<Mutex<Portfolio>>) -> Self {
        Self { api_client, portfolio }
    }

    /// The main reconciliation loop.
    pub async fn run(&self) -> anyhow::Result<()> {
        let mut interval = interval(Duration::from_secs(60)); // Reconcile every 60 seconds
        loop {
            interval.tick().await;
            if let Err(e) = self.reconcile().await {
                tracing::error!("Failed to reconcile state: {}", e);
            }
        }
    }

    async fn reconcile(&self) -> anyhow::Result<()> {
        // Fetch the real account state from the exchange
        let account_state = self.api_client.get_account_balance().await?;

        // Lock the portfolio to update it
        let mut portfolio = self.portfolio.lock().unwrap();

        // Update cash balance
        portfolio.cash = account_state.total_wallet_balance;

        // Update positions
        let mut open_positions = HashMap::new();
        for position in account_state.positions {
            if position.position_amt != Decimal::ZERO {
                open_positions.insert(
                    Symbol(position.symbol.clone()),
                    Position {
                        symbol: Symbol(position.symbol),
                        side: if position.position_amt > Decimal::ZERO { Side::Long } else { Side::Short },
                        quantity: position.position_amt.abs(),
                        entry_price: position.entry_price,
                        leverage: position.leverage.parse().unwrap_or(1),
                        sl_price: Default::default(), // SL price is not available from this API endpoint
                        entry_time: 0,
                    },
                );
            }
        }
        portfolio.open_positions = open_positions;

        Ok(())
    }
}