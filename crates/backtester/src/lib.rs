pub mod error;
pub mod types;
use strategies::Strategy;
use risk::RiskManager;
use execution::Executor;
use core_types::{Kline, OrderRequest, Signal, Side};
use rust_decimal_macros::dec;
use anyhow::Result;
use chrono::{Utc, TimeZone};
use num_traits::ToPrimitive;

/// The main engine for running historical backtests.
pub struct Backtester<'a> {
    /// The symbol to be tested.
    pub symbol: core_types::Symbol,
    /// The timeframe interval for the test.
    pub interval: String,
    /// A single strategy instance to test.
    pub strategy: Box<dyn Strategy + 'a>,
    /// The risk manager instance.
    pub risk_manager: Box<dyn RiskManager + 'a>,
    /// The execution simulator.
    pub executor: Box<dyn Executor + 'a>,
}

const KLINE_HISTORY_SIZE: usize = 100;

impl<'a> Backtester<'a> {
    pub async fn run(&mut self, klines: Vec<Kline>) -> Result<()> {
        // --- Main Backtesting Loop ---
        for i in KLINE_HISTORY_SIZE..klines.len() {
            let current_kline = &klines[i];
            let history_slice = &klines[(i - KLINE_HISTORY_SIZE)..i];

            // --- 1. Check for Stop-Loss Trigger ---
            let position_to_check = self.executor.portfolio().open_positions.get(&self.symbol).cloned();
            if let Some(open_position) = position_to_check {
                let stop_triggered = if open_position.side == Side::Long {
                    current_kline.low <= open_position.sl_price
                } else {
                    current_kline.high >= open_position.sl_price
                };

                if stop_triggered {
                    tracing::info!(
                        time = %Utc.timestamp_millis_opt(current_kline.open_time).unwrap(),
                        sl_price = open_position.sl_price.to_f64().unwrap_or(0.0),
                        trigger_price = if open_position.side == Side::Long { current_kline.low.to_f64().unwrap_or(0.0) } else { current_kline.high.to_f64().unwrap_or(0.0) },
                        "Stop-loss triggered!"
                    );

                    let close_order = OrderRequest {
                        symbol: open_position.symbol.clone(),
                        side: if open_position.side == Side::Long { Side::Short } else { Side::Long },
                        quantity: open_position.quantity,
                        leverage: open_position.leverage,
                        sl_price: dec!(0),
                        originating_signal: Signal::Close,
                    };

                    let execution_result = self.executor.execute(&close_order, open_position.sl_price).await;
                    if let Ok(execution) = execution_result {
                        tracing::info!(?execution, "Stop-loss order executed.");
                    } else if let Err(e) = execution_result {
                        tracing::error!(error = %e, "Failed to execute stop-loss order.");
                    }
                    continue;
                }
            }

            // --- 2. Assess Strategy for New Signals (if no SL was hit) ---
            let signal = self.strategy.assess(history_slice);
            if matches!(signal, Signal::Hold) {
                continue;
            }

            // --- 3. Evaluate Signal with Risk Manager ---
            let portfolio_value = self.executor.portfolio().cash;
            let open_position = self.executor.portfolio().open_positions.get(&self.symbol);
            let calculation_kline = &klines[i - 1];
            let order_request_result = self.risk_manager.evaluate(
                &signal,
                portfolio_value,
                calculation_kline,
                open_position,
            );

            // --- 4. Execute Approved Order ---
            match order_request_result {
                Ok(Some(order_request)) => {
                    let execution_result = self.executor.execute(&order_request, calculation_kline.close).await;
                    match execution_result {
                        Ok(execution) => {
                            tracing::info!(?execution, "Order executed.");
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Order execution failed.");
                        }
                    }
                }
                Ok(None) => {
                    // No action needed
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Risk manager vetoed the signal.");
                }
            }
        }
        Ok(())
    }
}