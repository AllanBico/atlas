use crate::types::MACrossoverSettings;
use crate::{Signal, Strategy};
use core_types::Kline;
use ta::indicators::ExponentialMovingAverage as Ema;
use ta::Next; // Import the `Next` trait to use the `.next()` method on indicators.
use num_traits::ToPrimitive; // <-- Add this import for to_f64

// Enum to represent the H1 market regime.
// While not used in the simplified `assess` method yet, it's part of the complete struct.
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
    // We store the previous value to detect the exact moment of crossover.
    last_fast_ema_val: f64,
    last_slow_ema_val: f64,
}

/// The stateful struct for our Multi-Timeframe MA Crossover strategy.
/// This initial implementation focuses on a single timeframe (M5) for signal generation.
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
        // Basic validation of settings
        if settings.m5_fast_period >= settings.m5_slow_period
            || settings.h1_fast_period >= settings.h1_slow_period
        {
            panic!("Fast EMA period must be less than Slow EMA period.");
        }

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

    /// This simplified `assess` method implements the M5 crossover logic.
    /// It does not yet incorporate the H1 market regime filter.
    fn assess(&mut self, klines: &[Kline]) -> Signal {
        // 1. Ensure we have enough data to calculate the slowest indicator.
        if klines.len() < self.settings.m5_slow_period as usize {
            return Signal::Hold; // Not enough data to warm up indicators.
        }

        // 2. Lazily initialize indicators on the first valid run.
        if self.m5_indicators.fast_ema.is_none() {
            // Warm up the indicators by feeding them the historical data slice.
            let mut fast_ema = Ema::new(self.settings.m5_fast_period as usize).unwrap();
            let mut slow_ema = Ema::new(self.settings.m5_slow_period as usize).unwrap();

            for kline in klines {
                let close_f64 = kline.close.to_f64().unwrap_or(0.0);
                self.m5_indicators.last_fast_ema_val = fast_ema.next(close_f64);
                self.m5_indicators.last_slow_ema_val = slow_ema.next(close_f64);
            }
            
            self.m5_indicators.fast_ema = Some(fast_ema);
            self.m5_indicators.slow_ema = Some(slow_ema);

            // Cannot generate a signal on the warm-up bar.
            return Signal::Hold;
        }

        // 3. Update indicators with the latest kline data point.
        let fast_ema = self.m5_indicators.fast_ema.as_mut().unwrap();
        let slow_ema = self.m5_indicators.slow_ema.as_mut().unwrap();

        let current_close = klines.last().unwrap().close.to_f64().unwrap_or(0.0);
        let current_fast_ema = fast_ema.next(current_close);
        let current_slow_ema = slow_ema.next(current_close);

        // 4. The Crossover Logic
        let signal = if current_fast_ema > current_slow_ema
            && self.m5_indicators.last_fast_ema_val <= self.m5_indicators.last_slow_ema_val
        {
            // Bullish Crossover: Fast EMA just crossed ABOVE the Slow EMA.
            Signal::GoLong {
                confidence: self.settings.confidence,
            }
        } else if current_fast_ema < current_slow_ema
            && self.m5_indicators.last_fast_ema_val >= self.m5_indicators.last_slow_ema_val
        {
            // Bearish Crossover: Fast EMA just crossed BELOW the Slow EMA.
            Signal::GoShort {
                confidence: self.settings.confidence,
            }
        } else {
            // No crossover event occurred on this kline.
            Signal::Hold
        };

        // 5. Update state for the next assessment call.
        self.m5_indicators.last_fast_ema_val = current_fast_ema;
        self.m5_indicators.last_slow_ema_val = current_slow_ema;

        // TODO: The H1 regime filter will be applied here in a future phase.
        // For example:
        // if (matches!(signal, Signal::GoLong) && self.regime != MarketRegime::Bullish) ||
        //    (matches!(signal, Signal::GoShort) && self.regime != MarketRegime::Bearish) {
        //     return Signal::Hold;
        // }

        signal
    }
}