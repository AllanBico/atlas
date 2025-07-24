//! This module provides a factory for creating strategy instances from configuration.

use core_types::StrategyConfig;
use app_config::types::StrategySettings;
use strategies::{
    Strategy,
    ma_crossover::MACrossover,
    supertrend::SuperTrend,
    prob_reversion::ProbReversion,
};

/// Creates strategy instances based on the `live.toml` configuration for a single pair.
pub fn create_strategies_for_live_run(
    pair_strategies: &[StrategyConfig],
    full_settings: &StrategySettings,
) -> Vec<Box<dyn Strategy + Send + Sync>> {
    let mut active_strategies = Vec::new();

    for strat_config in pair_strategies {
        // Find the full settings for this strategy from the main config
        let strategy_instance: Option<Box<dyn Strategy + Send + Sync>> = match strat_config.name.as_str() {
            "ma_crossover" => full_settings.ma_crossover.as_ref().map(|s| {
                Box::new(MACrossover::new(s.clone())) as Box<dyn Strategy + Send + Sync>
            }),
            "supertrend" => full_settings.supertrend.as_ref().map(|s| {
                Box::new(SuperTrend::new(s.clone())) as Box<dyn Strategy + Send + Sync>
            }),
            "prob_reversion" => full_settings.prob_reversion.as_ref().map(|s| {
                Box::new(ProbReversion::new(s.clone())) as Box<dyn Strategy + Send + Sync>
            }),
            _ => {
                tracing::warn!(name = %strat_config.name, "Attempted to create unknown strategy.");
                None
            }
        };

        if let Some(instance) = strategy_instance {
            active_strategies.push(instance);
        } else {
            tracing::error!(name=%strat_config.name, "Strategy is configured for a pair in live.toml but its parameters are not defined in development.toml!");
        }
    }
    
    active_strategies
}
