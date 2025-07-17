// In crates/strategies/src/ma_crossover.rs

use crate::types::MACrossoverSettings;
use crate::{Signal, Strategy};
use core_types::Kline;
use ta::indicators::ExponentialMovingAverage as Ema;
use num_traits::cast::ToPrimitive;
use ta::Next;

// Enum to represent the H1 market regime.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum MarketRegime {
    #[default]
    Sideways,
    Bullish,
    Bearish,
}

// Struct to hold the state for a single timeframe's indicators.
#[derive(Debug, Default)]
struct TimeframeIndicators {
    fast_ema: Option<Ema>,
    slow_ema: Option<Ema>,
    last_fast_ema_val: f64,
    last_slow_ema_val: f64,
}

/// The stateful struct for our Multi-Timeframe MA Crossover strategy.
#[derive(Debug)]
pub struct MACrossover {
    /// The configuration for this strategy instance.
    settings: MACrossoverSettings,
    /// The indicators and state for the H1 timeframe (The "General").
    h1_indicators: TimeframeIndicators,
    /// The indicators and state for the M5 timeframe (The "Sergeant").
    m5_indicators: TimeframeIndicators,
    /// The current market regime determined by the H1 timeframe.
    regime: MarketRegime,
}

impl MACrossover {
    /// Creates a new `MACrossover` strategy instance from its settings.
    pub fn new(settings: MACrossoverSettings) -> Self {
        Self {
            settings,
            h1_indicators: TimeframeIndicators::default(),
            m5_indicators: TimeframeIndicators::default(),
            regime: MarketRegime::default(),
        }
    }
}

impl Strategy for MACrossover {
    fn name(&self) -> &'static str {
        "MultiTimeframeMACrossover"
    }

    fn assess(&mut self, klines: &[Kline]) -> Signal {
        // --- This implementation focuses ONLY on the M5 logic for now ---
        // --- The H1 logic will be integrated in a later phase.       ---

        // 1. Ensure we have enough data to calculate indicators.
        if klines.len() < self.settings.m5_slow_period as usize {
            return Signal::Hold; // Not enough data yet.
        }

        // 2. Initialize indicators if they haven't been already.
        if self.m5_indicators.fast_ema.is_none() {
            self.m5_indicators.fast_ema = Some(Ema::new(self.settings.m5_fast_period as usize).unwrap());
            self.m5_indicators.slow_ema = Some(Ema::new(self.settings.m5_slow_period as usize).unwrap());
        }

        // 3. Update indicators with the latest kline data.
        let fast_ema = self.m5_indicators.fast_ema.as_mut().unwrap();
        let slow_ema = self.m5_indicators.slow_ema.as_mut().unwrap();
        
        // We calculate the indicator on the 'close' price of each kline.
        let current_close: f64 = klines.last().unwrap().close.to_f64().unwrap();
        let current_fast_ema = fast_ema.next(current_close);
        let current_slow_ema = slow_ema.next(current_close);
        
        // 4. The Crossover Logic
        let signal = if current_fast_ema > current_slow_ema && self.m5_indicators.last_fast_ema_val <= self.m5_indicators.last_slow_ema_val {
            // Bullish Crossover: Fast line just crossed above the slow line.
            Signal::GoLong { confidence: self.settings.confidence }
        } else if current_fast_ema < current_slow_ema && self.m5_indicators.last_fast_ema_val >= self.m5_indicators.last_slow_ema_val {
            // Bearish Crossover: Fast line just crossed below the slow line.
            Signal::GoShort { confidence: self.settings.confidence }
        } else {
            // No crossover event on this kline.
            Signal::Hold
        };

        // 5. Update state for the next assessment.
        self.m5_indicators.last_fast_ema_val = current_fast_ema;
        self.m5_indicators.last_slow_ema_val = current_slow_ema;

        // TODO: In the future, we would check `self.regime` here and potentially
        // filter this signal. For now, we return it directly.
        
        signal
    }
}