use crate::KLINE_HISTORY_SIZE;
use api_client::live_connector::LiveConnector;
use app_config::types::BinanceSettings;
use core_types::{Kline, Symbol, Signal};
use database::Db;
use events::WsMessage;
use execution::Executor;
use futures::StreamExt;
use pin_project::pin_project;
use risk::RiskManager;
use strategies::Strategy;
use std::collections::VecDeque;
use tokio::sync::broadcast;

/// A self-contained task that manages all trading logic for a single asset.
pub struct TradingTask {
    symbol: Symbol,
    interval: String,
    db: Db,
    // A task can have multiple strategies
    strategies: Vec<Box<dyn Strategy + Send + Sync>>,
    risk_manager: Box<dyn RiskManager + Send + Sync>,
    executor: Box<dyn Executor + Send + Sync>,
    live_connector: LiveConnector,
    binance_settings: BinanceSettings,
    ws_tx: broadcast::Sender<WsMessage>,
    // The in-memory "hot" cache of recent klines for this specific asset
    klines: VecDeque<Kline>,
}

impl TradingTask {
    pub fn new(
        symbol: Symbol,
        interval: String,
        db: Db,
        strategies: Vec<Box<dyn Strategy + Send + Sync>>,
        risk_manager: Box<dyn RiskManager + Send + Sync>,
        executor: Box<dyn Executor + Send + Sync>,
        binance_settings: BinanceSettings,
        ws_tx: broadcast::Sender<WsMessage>,
    ) -> Self {
        Self {
            symbol,
            interval,
            db,
            strategies,
            risk_manager,
            executor,
            live_connector: LiveConnector::new(),
            binance_settings,
            ws_tx,
            klines: VecDeque::with_capacity(KLINE_HISTORY_SIZE + 1),
        }
    }

    /// The main, long-running loop for this trading task.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!(symbol = %self.symbol.0, interval = %self.interval, "Starting trading task.");
        
        // --- 1. Warm-up Phase ---
        // TODO: Implement `get_latest_klines` in the database crate.
        // let initial_klines = self.db.get_latest_klines(&self.symbol, &self.interval, KLINE_HISTORY_SIZE).await?;
        // self.klines.extend(initial_klines);
        tracing::info!(symbol = %self.symbol.0, "Task warmup complete.");

        // --- 2. Live Trading Loop ---
        let mut kline_stream = Box::pin(self.live_connector.subscribe_to_klines(
            &self.symbol,
            &self.interval,
            &self.binance_settings.ws_base_url,
        ));

        while let Some(Ok(kline)) = kline_stream.next().await {
            self.klines.push_back(kline.clone());
            if self.klines.len() > KLINE_HISTORY_SIZE {
                self.klines.pop_front();
            }

            if self.klines.len() < KLINE_HISTORY_SIZE {
                continue;
            }

            let history_slice: Vec<_> = self.klines.iter().cloned().collect();
            
            // --- The Pipeline ---
            // TODO: In a multi-strategy task, we need to decide how to combine signals.
            // For now, we'll just use the first strategy in the list.
            if let Some(strategy) = self.strategies.get_mut(0) {
                // The rest of the logic (stop-loss check, assess, evaluate, execute)
                // is identical to the loop from our old `Engine`.
                // This logic would be pasted here.
                
                let signal = strategy.assess(&history_slice);
                if !matches!(signal, Signal::Hold) {
                    tracing::info!(symbol = %self.symbol.0, ?signal, "Strategy generated a signal.");
                    // ... and so on
                }
            }
        }
        
        anyhow::bail!("Kline stream for {} ended unexpectedly.", self.symbol.0)
    }
}
