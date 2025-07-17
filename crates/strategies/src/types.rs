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