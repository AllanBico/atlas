// In crates/strategies/src/types.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MACrossoverSettings {
    // Parameters for the H1 "Strategist" (the trend filter)
    pub h1_fast_period: u32,
    pub h1_slow_period: u32,
    
    // Parameters for the M5 "Tactician" (the entry signal)
    pub m5_fast_period: u32,
    pub m5_slow_period: u32,

    // The confidence score to assign to signals from this strategy
    pub confidence: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)] // Clone is needed for the optimizer
pub struct SuperTrendSettings {
    pub period: u32,
    pub multiplier: f64,
    pub exit_multiplier: f64,
    pub volume_threshold: f64,
    pub confirmation_bars: u32,
    pub ema_confirmation_period: u32,
    pub confidence: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProbReversionSettings {
    pub bband_period: u32,
    pub bband_stddev: f64,
    pub adx_period: u32,
    pub adx_range_threshold: f64,
    pub rsi_period: u32,
    pub rsi_oversold: f64,
    pub rsi_smoothing: u32,
    pub confidence: f64,
}