// In crates/engine/src/bot.rs

use core_types::{Kline, Symbol, Signal, Side, OrderRequest};
use strategies::Strategy;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use risk::RiskManager;
use execution::Executor;
use execution::types::Portfolio;
use rust_decimal_macros::dec;

const KLINE_HISTORY_SIZE: usize = 2; // The number of klines to maintain for the strategy.

/// Represents a single, independent trading instance for a specific asset and strategy.
pub struct Bot<'a> {
    /// A unique identifier for this bot instance (e.g., "BTCUSDT_1m_MACrossover").
    pub id: String,
    pub symbol: Symbol,
    pub interval: String,
    
    /// The specific strategy instance for this bot.
    pub strategy: Box<dyn Strategy + Send + 'a>,
    
    /// The in-memory "hot" cache of recent klines for this bot's specific symbol and interval.
    klines: VecDeque<Kline>,
}

impl<'a> Bot<'a> {
    /// Creates a new `Bot` instance.
    pub fn new(
        symbol: Symbol,
        interval: String,
        strategy: Box<dyn Strategy + Send + 'a>,
    ) -> Self {
        let id = format!("{}_{}_{}", symbol.0, interval, strategy.name());
        tracing::info!(id = %id, "Creating new bot instance.");
        
        Self {
            id,
            symbol,
            interval,
            strategy,
            klines: VecDeque::with_capacity(KLINE_HISTORY_SIZE + 1),
        }
    }
    
    /// This is the primary logic loop for a single bot instance.
    /// It is called by the main Engine when a new kline for this bot's symbol is received.
    pub async fn on_kline(
        &mut self,
        kline: Kline,
        risk_manager: &Box<dyn RiskManager + Send + Sync + 'a>,
        executor: &mut Box<dyn Executor + Send + Sync + 'a>,
        portfolio: &Arc<Mutex<Portfolio>>,
    ) -> Result<(), anyhow::Error> {
        // Add new kline to our local cache and maintain history size
        self.klines.push_back(kline.clone());
        if self.klines.len() > KLINE_HISTORY_SIZE {
            self.klines.pop_front();
        }

        if self.klines.len() < KLINE_HISTORY_SIZE {
            return Ok(()); // Wait until we have a full history before trading
        }

        // Print each kline for debugging
        tracing::info!(id = %self.id, symbol = %self.symbol.0, kline = ?kline);
        
        // --- The full Strategy -> Risk -> Execution pipeline ---
        
        let current_kline = kline;
        let history_slice: Vec<_> = self.klines.iter().cloned().collect();

        // 1. Check for Stop-Loss Trigger
        let position_to_check = {
            let portfolio_guard = portfolio.lock().await;
            portfolio_guard.open_positions.get(&self.symbol).cloned()
        };
        
        if let Some(open_position) = position_to_check {
            let current_price = current_kline.close;
            let should_trigger_sl = match open_position.side {
                Side::Long => current_price <= open_position.sl_price,
                Side::Short => current_price >= open_position.sl_price,
            };
            
            if should_trigger_sl {
                tracing::info!(
                    bot_id = %self.id,
                    symbol = %open_position.symbol.0,
                    side = ?open_position.side,
                    current_price = %current_price,
                    sl_price = %open_position.sl_price,
                    "Stop-loss triggered! Closing position."
                );
                
                let close_order = OrderRequest {
                    symbol: open_position.symbol.clone(),
                    side: match open_position.side {
                        Side::Long => Side::Short,  // Close long with short
                        Side::Short => Side::Long,  // Close short with long
                    },
                    quantity: open_position.quantity,
                    leverage: open_position.leverage,
                    sl_price: dec!(0), // No stop-loss for closing orders
                    originating_signal: Signal::Close,
                };
                
                let mut portfolio_guard = portfolio.lock().await;
                let _ = executor.execute(
                    &close_order,
                    current_price,
                    current_kline.open_time,
                    &mut *portfolio_guard,
                ).await;
                return Ok(()); // Skip strategy evaluation after stop-loss
            }
        }

        // 2. Assess Strategy for New Signals
        let signal = self.strategy.assess(&history_slice);
        if matches!(signal, Signal::Hold) {
            return Ok(());
        }
        tracing::info!(bot_id = %self.id, ?signal, "Strategy generated a signal.");

        // 3. Evaluate Signal with Risk Manager
        let (portfolio_value, open_position) = {
            let portfolio_guard = portfolio.lock().await;
            (
                portfolio_guard.cash,
                portfolio_guard.open_positions.get(&self.symbol).cloned()
            )
        };
        
        let calculation_kline = &history_slice[history_slice.len() - 2];
        let order_request_result = risk_manager.evaluate(
            &signal,
            &self.symbol,
            portfolio_value,
            calculation_kline,
            open_position.as_ref(),
        );

        // 4. Execute Approved Order
        if let Ok(Some(order_request)) = order_request_result {
            tracing::info!(bot_id = %self.id, ?order_request, "Signal approved by risk manager.");
            let mut portfolio_guard = portfolio.lock().await;
            let _ = executor.execute(
                &order_request,
                current_kline.open,
                current_kline.open_time,
                &mut *portfolio_guard,
            ).await;
        } else if let Err(e) = order_request_result {
            tracing::warn!(bot_id = %self.id, error = %e, "Risk manager vetoed the signal.");
        }

        Ok(())
    }
}