// In crates/engine/src/lib.rs

use api_client::live_connector::LiveConnector;
use core_types::{Kline, Symbol, OrderRequest, Signal, Side};
use database::Db;
use execution::Executor;
use futures::StreamExt;
use risk::RiskManager;
use strategies::Strategy;
use std::collections::VecDeque;
use rust_decimal_macros::dec;
use rust_decimal::Decimal;
use tokio::sync::broadcast;
use events::WsMessage;
use app_config::types::BinanceSettings;

const KLINE_HISTORY_SIZE: usize = 2; // Same as in backtester

/// The core trading engine that orchestrates live data and decision making.
pub struct Engine<'a> {
    symbol: Symbol,
    interval: String,
    db: Db,
    strategy: Box<dyn Strategy + Send + 'a>,
    risk_manager: Box<dyn RiskManager + Send + 'a>,
    executor: Box<dyn Executor + Send + 'a>,
    live_connector: LiveConnector,
    // The in-memory "hot" cache of recent klines
    klines: VecDeque<Kline>,
    ws_tx: broadcast::Sender<WsMessage>,
    binance_settings: BinanceSettings, // <-- Add this
}

impl<'a> Engine<'a> {
    pub fn new(
        symbol: Symbol,
        interval: String,
        db: Db,
        strategy: Box<dyn Strategy + Send + 'a>,
        risk_manager: Box<dyn RiskManager + Send + 'a>,
        executor: Box<dyn Executor + Send + 'a>,
        ws_tx: broadcast::Sender<WsMessage>,
        binance_settings: BinanceSettings, // <-- Add this
    ) -> Self {
        Self {
            symbol,
            interval,
            db,
            strategy,
            risk_manager,
            executor,
            live_connector: LiveConnector::new(),
            klines: VecDeque::with_capacity(KLINE_HISTORY_SIZE + 1),
            ws_tx,
            binance_settings, // <-- Store it
        }
    }

    /// The main, long-running loop of the trading engine.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        // --- 1. Warm-up Phase ---
        tracing::info!("Warming up engine: loading initial historical data...");
        // TODO: The get_klines_by_date_range is not suitable for "last N bars".
        // We will need a new DB method: `get_latest_klines(symbol, interval, limit)`
        // For now, we will proceed with an empty history.
        // let initial_klines = self.db.get_latest_klines(&self.symbol, &self.interval, KLINE_HISTORY_SIZE).await?;
        // self.klines.extend(initial_klines);
        tracing::info!("Engine warmup complete. History size: {}", self.klines.len());

        // --- 2. Live Trading Loop ---
        let mut kline_stream = Box::pin(self.live_connector.subscribe_to_klines(
            &self.symbol,
            &self.interval,
            &self.binance_settings.ws_base_url, // <-- Pass the configured URL
        ));
        tracing::info!("Subscribed to live kline stream. Engine is now live.");

        while let Some(Ok(kline)) = kline_stream.next().await {
            // Add new kline and maintain history size
            self.klines.push_back(kline.clone());
            if self.klines.len() > KLINE_HISTORY_SIZE {
                self.klines.pop_front();
            }

            tracing::debug!(close = %kline.close, "New kline received.");

            if self.klines.len() < KLINE_HISTORY_SIZE {
                continue; // Wait until we have a full history before trading
            }

            let current_kline = kline;
            let history_slice: Vec<_> = self.klines.iter().cloned().collect();

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
                        time = current_kline.open_time,
                        sl_price = %open_position.sl_price,
                        trigger_price = if open_position.side == Side::Long { format!("{}", current_kline.low) } else { format!("{}", current_kline.high) },
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

                    let execution_result = self.executor.execute(&close_order, open_position.sl_price, current_kline.open_time).await;
                    if let Ok((execution, Some(closed_pos))) = execution_result {
                        tracing::info!(?execution, "Stop-loss order executed and position closed.");
                    } else if let Ok((execution, None)) = execution_result {
                        tracing::warn!(?execution, "Stop-loss order executed but no closed position returned.");
                    } else if let Err(e) = execution_result {
                        tracing::error!(error = %e, "Failed to execute stop-loss order.");
                    }
                    continue;
                }
            }

            // --- 2. Assess Strategy for New Signals ---
            let signal = self.strategy.assess(&history_slice);
            if matches!(signal, Signal::Hold) {
                continue;
            }
            // Broadcast the signal (placeholder, needs WsMessage variant)
            // let _ = self.ws_tx.send(WsMessage::SignalGenerated(signal.clone()));
            tracing::info!(?signal, "Strategy generated a signal.");

            // --- 3. Evaluate Signal with Risk Manager ---
            let portfolio_value = self.executor.portfolio().cash;
            let open_position = self.executor.portfolio().open_positions.get(&self.symbol);
            // The risk manager needs the previous bar's close for calculation.
            let calculation_kline = &history_slice[history_slice.len() - 2];
            let order_request_result = self.risk_manager.evaluate(
                &signal,
                portfolio_value,
                &self.symbol, // Pass the symbol
                calculation_kline, // Pass the kline with the price data
                open_position,
            );

            // --- 4. Execute Approved Order ---
            match order_request_result {
                Ok(Some(order_request)) => {
                    tracing::info!(?signal, "Strategy signal approved by risk manager.");
                    // Execute at the current bar's open price.
                    let execution_result = self.executor.execute(&order_request, current_kline.open, current_kline.open_time).await;
                    match execution_result {
                        Ok((execution, Some(closed_pos))) => {
                            tracing::info!(?execution, "Order executed and position closed.");
                        }
                        Ok((execution, None)) => {
                            tracing::info!(?execution, "Order executed (entry or no position closed).");
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

        anyhow::bail!("Kline stream unexpectedly ended.")
    }
}