// In crates/engine/src/lib.rs

pub mod task;
pub mod strategy_factory;

use crate::task::TradingTask;
use anyhow::Result;
use app_config::{LiveRunConfig, Settings};
use database::Db;
use events::WsMessage;
use execution::{simulated::SimulatedExecutor, SimulationSettings};
use risk::simple_manager::SimpleRiskManager;
use crate::strategy_factory::create_strategies_for_live_run;
use tokio::sync::broadcast;
use futures::future;
use core_types::Symbol;
use rust_decimal_macros::dec;

pub const KLINE_HISTORY_SIZE: usize = 200;

/// The portfolio-level orchestrator for all trading activities.
pub struct Engine {
    live_config: LiveRunConfig,
    app_config: Settings,
    db: Db,
    ws_tx: broadcast::Sender<WsMessage>,
}

impl Engine {
    pub fn new(
        live_config: LiveRunConfig,
        app_config: Settings,
        db: Db,
        ws_tx: broadcast::Sender<WsMessage>,
    ) -> Self {
        Self {
            live_config,
            app_config,
            db,
            ws_tx,
        }
    }

    /// The main run method for the orchestrator.
    /// It spawns a `TradingTask` for each configured and enabled trading pair.
    pub async fn run(&self) -> Result<()> {
        tracing::info!("Initializing Portfolio Orchestrator Engine...");

        let mut task_handles = vec![];

        // Loop through the pairs defined in live.toml
        for pair_config in &self.live_config.pair_configs {
            if !pair_config.enabled {
                tracing::warn!(symbol = %pair_config.symbol, "Skipping disabled trading pair.");
                continue;
            }

            tracing::info!(symbol = %pair_config.symbol, "Setting up trading task.");

            // --- Instantiate all components for this specific task ---

            // 1. Create the strategies for this pair
            let strategies = create_strategies_for_live_run(
                &pair_config.strategies,
                &self.app_config.strategies,
            );

            if strategies.is_empty() {
                tracing::error!(symbol = %pair_config.symbol, "No valid strategies found for pair. Skipping.");
                continue;
            }
            
            // 2. Create the Risk Manager
            let risk_manager = Box::new(SimpleRiskManager::new(
                self.app_config.simple_risk_manager.clone().unwrap(),
            ));

            // 3. Create the Executor with simulation settings
            let sim_settings = SimulationSettings {
                slippage_percent: 0.001,
                maker_fee: 0.001,
                taker_fee: 0.001,
            };
            let executor = Box::new(SimulatedExecutor::new(
                sim_settings,
                dec!(10_000.0), // initial balance
                self.ws_tx.clone(),
            ));

            // 4. Create the TradingTask
            let mut task = TradingTask::new(
                Symbol(pair_config.symbol.clone()),
                pair_config.interval.clone(),
                self.db.clone(),
                strategies,
                risk_manager,
                executor,
                self.app_config.binance.clone(),
                self.ws_tx.clone(),
            );
            
            // 5. Spawn the task to run concurrently
            let handle = tokio::spawn(async move {
                task.run().await
            });
            
            task_handles.push(handle);
        }

        if task_handles.is_empty() {
            anyhow::bail!("No trading tasks were started. Check your live.toml configuration.");
        }

        tracing::info!(count = task_handles.len(), "All trading tasks have been spawned.");

        // Wait for all tasks to complete. In a healthy system, this will run forever.
        // `join_all` will return if any of the tasks exit (e.g., due to an error).
        let results = future::join_all(task_handles).await;
        
        tracing::error!(?results, "One or more trading tasks have terminated. Shutting down.");
        
        Ok(())
    }
}