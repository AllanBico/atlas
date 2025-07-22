// In app/src/strategy_factory.rs

use app_config::types::StrategySettings;
use strategies::ma_crossover::MACrossover;
use strategies::supertrend::SuperTrend;
use strategies::prob_reversion::ProbReversion;
use strategies::Strategy;

/// Creates a vector of strategy instances from the application settings.
/// 
/// This factory function instantiates all configured strategies from the
/// settings file. It returns an empty vector if no strategies are configured.
pub fn create_strategies_from_settings(strategy_settings: &StrategySettings) -> Vec<Box<dyn Strategy + Send>> {
    let mut strategies: Vec<Box<dyn Strategy + Send>> = Vec::new();
    
    // Add MA Crossover strategy if configured
    if let Some(settings) = &strategy_settings.ma_crossover {
        strategies.push(Box::new(MACrossover::new(settings.clone())));
    }
    
    // Add SuperTrend strategy if configured
    if let Some(settings) = &strategy_settings.supertrend {
        strategies.push(Box::new(SuperTrend::new(settings.clone())));
    }
    
    // Add Probability Reversion strategy if configured
    if let Some(settings) = &strategy_settings.prob_reversion {
        strategies.push(Box::new(ProbReversion::new(settings.clone())));
    }
    
    strategies
}