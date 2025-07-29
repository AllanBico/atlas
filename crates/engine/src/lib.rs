// In crates/engine/src/lib.rs

use api_client::live_connector::LiveConnector;
use core_types::{Symbol, Kline};
use database::Db;
use execution::Executor;
use futures::StreamExt;
use risk::RiskManager;
use strategies::Strategy;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use events::WsMessage;
use crate::bot::Bot;
use app_config::types::{BinanceSettings, LiveConfig};
use strategies::ma_crossover::MACrossover;
use strategies::prob_reversion::ProbReversion;
use strategies::supertrend::SuperTrend;
pub mod bot;
const KLINE_HISTORY_SIZE: usize = 2; // Same as in backtester
use anyhow;
use toml;
/// The core trading engine that orchestrates live data and decision making for a portfolio of bots.
pub struct Engine<'a> {
    /// A map of all active bot instances, keyed by their unique stream name (e.g., "btcusdt@kline_1m").
    bots: HashMap<String, Bot<'a>>,
    
    // The engine still needs these components to pass down to the bots' logic
    db: Db,
    risk_manager: Box<dyn RiskManager + Send + Sync + 'a>,
    executor: Box<dyn Executor + Send + Sync + 'a>,
    live_connector: LiveConnector,
    binance_settings: BinanceSettings,
    ws_tx: broadcast::Sender<WsMessage>,
    
    /// The shared portfolio state, wrapped in Arc<Mutex<>> for thread-safe access
    portfolio: Arc<Mutex<Portfolio>>,
}

impl<'a> Engine<'a> {
    /// Creates a new Engine and instantiates all bots based on the provided configuration.
    pub fn new(
        live_config: &LiveConfig,
        strategy_settings: &StrategySettings,
        binance_settings: BinanceSettings,
        db: Db,
        risk_manager: Box<dyn RiskManager + Send + Sync + 'a>,
        executor: Box<dyn Executor + Send + Sync + 'a>,
        ws_tx: broadcast::Sender<WsMessage>,
        binance_settings: BinanceSettings, // Pass this through
    ) -> Self {
        let mut bots = HashMap::new();

        // Iterate through the bot configurations from live.toml
        for bot_config in &live_config.bot {
            if !bot_config.enabled {
                continue; // Skip disabled bots
            }

            // --- Strategy Factory Logic ---
            // Find the correct strategy parameters from the main config
            // and instantiate the strategy trait object.
            let strategy: Box<dyn Strategy + Send + 'a> = 
                match bot_config.strategy_params.as_str() {
                    "ma_crossover" => {
                        let params = strategy_settings.ma_crossover.clone()
                            .expect("Missing ma_crossover params in main config");
                        Box::new(MACrossover::new(params))
                    },
                    "supertrend" => {
                        let params = strategy_settings.supertrend.clone()
                            .expect("Missing supertrend params in main config");
                        Box::new(SuperTrend::new(params))
                    },
                    "prob_reversion" => {
                        let params = strategy_settings.prob_reversion.clone()
                            .expect("Missing prob_reversion params in main config");
                        Box::new(ProbReversion::new(params))
                    },
                    _ => {
                        tracing::warn!(name = %bot_config.strategy_params, "Unknown strategy params key in live.toml, skipping bot.");
                        continue;
                    }
                };
            
            // Create the new bot instance
            let bot = Bot::new(
                Symbol(bot_config.symbol.clone()),
                bot_config.interval.clone(),
                strategy,
            );
            
            // Use the WebSocket stream name as the unique key
            let stream_name = format!("{}@kline_{}", bot_config.symbol.to_lowercase(), bot_config.interval);
            bots.insert(stream_name, bot);
        }

        Self {
            bots,
            db,
            risk_manager,
            executor,
            live_connector: LiveConnector::new(),
            binance_settings,
            ws_tx,
            portfolio,
        }
    }

    /// The main, long-running loop of the trading engine.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        // --- 1. Warm-up Phase (for all bots) ---
        tracing::info!("Warming up all bot instances...");
        // TODO: Implement a `get_latest_klines` DB method and warm up each bot.
        // for bot in self.bots.values_mut() {
        //     let klines = self.db.get_latest_klines(&bot.symbol, &bot.interval, KLINE_HISTORY_SIZE).await?;
        //     bot.warm_up(klines);
        // }
        tracing::info!("Engine warmup complete.");

        // --- 2. Subscribe to all streams ---
        let stream_names: Vec<String> = self.bots.keys().cloned().collect();
        if stream_names.is_empty() {
            tracing::warn!("No bots configured to run. Engine will idle.");
            // Prevent the engine from exiting
            loop { tokio::time::sleep(std::time::Duration::from_secs(60)).await; }
        }
        
        let mut combined_stream = Box::pin(self.live_connector.subscribe_to_streams(
            stream_names,
            &self.binance_settings.ws_base_url,
        ));
        tracing::info!("Engine subscribed to all streams and is now live.");

        // --- 3. The Main Data Router Loop ---
        while let Some(Ok(event)) = combined_stream.next().await {
            // Only process closed klines
            if !event.kline.is_closed {
                continue;
            }

            let stream_key = format!("{}@kline_{}", event.kline.symbol.to_lowercase(), event.kline.interval);

            if let Some(bot) = self.bots.get_mut(&stream_key) {
                // Convert the WsKline into our core Kline type
                let kline = Kline {
                    open_time: event.kline.open_time,
                    open: event.kline.open,
                    high: event.kline.high,
                    low: event.kline.low,
                    close: event.kline.close,
                    volume: event.kline.volume,
                    close_time: event.kline.close_time,
                };
                
                // Delegate all decision-making logic to the bot instance.
                if let Err(e) = bot.on_kline(
                    kline,
                    &self.risk_manager,
                    &mut self.executor,
                    &self.portfolio,
                ).await {
                    tracing::error!(bot_id = %bot.id, error = %e, "An error occurred in a bot's on_kline handler.");
                }
            } else {
                tracing::warn!(stream = %stream_key, "Received data for a stream with no configured bot.");
            }
        }
        
        anyhow::bail!("Combined kline stream unexpectedly ended.")
    }
}