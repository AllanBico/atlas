use anyhow::Result;
use crate::{ma_crossover::MACrossover, supertrend::SuperTrend, prob_reversion::ProbReversion, Strategy};
use crate::types::{MACrossoverSettings, SuperTrendSettings, ProbReversionSettings};
use core_types::StrategyConfig;

pub fn create_strategies_for_live_run(
    pair_strategies: &[StrategyConfig],
) -> Result<Vec<Box<dyn Strategy + Send + Sync>>> {
    let mut active_strategies = Vec::new();

    for strat_config in pair_strategies {
        let strategy_instance: Box<dyn Strategy + Send + Sync> = match strat_config.name.as_str() {
            "ma_crossover" => {
                let settings: MACrossoverSettings = strat_config.params.clone().try_into()?;
                Box::new(MACrossover::new(settings))
            },
            "supertrend" => {
                let settings: SuperTrendSettings = strat_config.params.clone().try_into()?;
                Box::new(SuperTrend::new(settings))
            },
            "prob_reversion" => {
                let settings: ProbReversionSettings = strat_config.params.clone().try_into()?;
                Box::new(ProbReversion::new(settings))
            },
            unknown => anyhow::bail!("Attempted to create unknown strategy: {}", unknown),
        };
        active_strategies.push(strategy_instance);
    }
    
    Ok(active_strategies)
}
