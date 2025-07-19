// In crates/strategies/src/supertrend.rs

use crate::types::SuperTrendSettings; // We will define this next
use crate::{Signal, Strategy};
use core_types::{Kline, Side};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use ta::indicators::{AverageTrueRange, ExponentialMovingAverage as Ema};
use ta::{Next, Period, DataItem};

// --- Internal State and Enums ---

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum TrendDirection {
    #[default]
    Sideways,
    Uptrend,
    Downtrend,
}

/// Holds the internal calculation state for a single bar.
#[derive(Debug, Clone, Copy, Default)]
struct StState {
    atr: f64,
    final_upper_band: f64,
    final_lower_band: f64,
    trend: TrendDirection,
    confirmed_trend: TrendDirection,
    confirmation_count: u32,
}

/// The main stateful struct for the Enhanced SuperTrend strategy.
#[derive(Debug)]
pub struct SuperTrend {
    settings: SuperTrendSettings,
    atr_indicator: AverageTrueRange,
    ema_confirm: Ema,
    // We only need to store the history of states for calculation.
    states: Vec<StState>,
    // Tracks the current position side to generate correct exit signals.
    last_signal_side: Option<Side>,
}

impl SuperTrend {
    /// Creates a new `SuperTrend` strategy instance from its settings.
    pub fn new(settings: SuperTrendSettings) -> Self {
        if settings.period < 1
            || settings.confirmation_bars < 1
            || settings.ema_confirmation_period < 1
        {
            panic!("Strategy periods must be greater than 0.");
        }
        if settings.multiplier <= 0.0 || settings.exit_multiplier <= 0.0 {
            panic!("Strategy multipliers must be positive.");
        }

        Self {
            atr_indicator: AverageTrueRange::new(settings.period as usize).unwrap(),
            ema_confirm: Ema::new(settings.ema_confirmation_period as usize).unwrap(),
            settings,
            states: Vec::new(),
            last_signal_side: None,
        }
    }
}

impl Strategy for SuperTrend {
    fn name(&self) -> &'static str {
        "EnhancedSuperTrend"
    }

    fn assess(&mut self, klines: &[Kline]) -> Signal {
        let required_bars = (self.settings.period as usize)
            .max(self.settings.ema_confirmation_period as usize);

        if klines.len() < required_bars {
            return Signal::Hold;
        }

        // --- State Calculation Loop ---
        // We recalculate the state history based on the provided klines.
        // This makes the strategy stateless between `assess` calls, which is robust.
        self.states.clear();
        let mut last_state = StState::default();
        let mut atr = self.atr_indicator.clone(); // Clone to use for this run
        
        for (i, kline) in klines.iter().enumerate() {
            let close = kline.close.to_f64().unwrap_or(0.0);
            let high = kline.high.to_f64().unwrap_or(0.0);
            let low = kline.low.to_f64().unwrap_or(0.0);
            
            // ATR requires the previous close, which is unavailable for the first kline.
            let prev_close = if i > 0 { klines[i-1].close.to_f64().unwrap_or(0.0) } else { close };
            let data_item = DataItem::builder().high(high).low(low).close(close).open(close).volume(0.0).build().unwrap();
            let current_atr = atr.next(&data_item);
            
            let hl2 = (high + low) / 2.0;

            // --- SuperTrend Core Logic (translated from Go) ---
            let basic_upper = hl2 + (self.settings.multiplier * current_atr);
            let basic_lower = hl2 - (self.settings.multiplier * current_atr);

            let mut current_state = last_state;
            current_state.atr = current_atr;

            current_state.final_upper_band = if basic_upper < last_state.final_upper_band || prev_close > last_state.final_upper_band {
                basic_upper
            } else {
                last_state.final_upper_band
            };

            current_state.final_lower_band = if basic_lower > last_state.final_lower_band || prev_close < last_state.final_lower_band {
                basic_lower
            } else {
                last_state.final_lower_band
            };

            current_state.trend = if close > current_state.final_upper_band {
                TrendDirection::Uptrend
            } else if close < current_state.final_lower_band {
                TrendDirection::Downtrend
            } else {
                last_state.trend // Maintain previous trend
            };

            // Trend confirmation logic
            if current_state.trend == last_state.confirmed_trend {
                current_state.confirmation_count += 1;
            } else {
                current_state.confirmation_count = 1;
                current_state.confirmed_trend = current_state.trend;
            }

            self.states.push(current_state);
            last_state = current_state;
        }

        // --- Signal Generation (using the latest calculated states) ---
        if self.states.len() < 2 {
            return Signal::Hold;
        }

        let current_state = self.states.last().unwrap();
        let prev_state = &self.states[self.states.len() - 2];
        let current_kline = klines.last().unwrap();
        
        // Volume Filter
        if current_kline.volume < Decimal::from_f64(self.settings.volume_threshold).unwrap_or_default() {
            return Signal::Hold;
        }

        // Confirmation Bars Filter
        if current_state.confirmation_count < self.settings.confirmation_bars {
            return Signal::Hold;
        }

        // Generate Entry Signals
        if prev_state.confirmed_trend != TrendDirection::Uptrend && current_state.confirmed_trend == TrendDirection::Uptrend {
            let ema_val: f64 = klines.iter().map(|k| k.close.to_f64().unwrap()).collect::<Vec<f64>>().as_slice().ema(self.settings.ema_confirmation_period as usize).unwrap_or(0.0);
            if current_kline.close.to_f64().unwrap() > ema_val {
                self.last_signal_side = Some(Side::Long);
                return Signal::GoLong { confidence: self.settings.confidence };
            }
        }

        if prev_state.confirmed_trend != TrendDirection::Downtrend && current_state.confirmed_trend == TrendDirection::Downtrend {
             let ema_val: f64 = klines.iter().map(|k| k.close.to_f64().unwrap()).collect::<Vec<f64>>().as_slice().ema(self.settings.ema_confirmation_period as usize).unwrap_or(0.0);
            if current_kline.close.to_f64().unwrap() < ema_val {
                self.last_signal_side = Some(Side::Short);
                return Signal::GoShort { confidence: self.settings.confidence };
            }
        }

        // Generate Tighter Exit Signals
        let hl2 = (current_kline.high + current_kline.low) / Decimal::from(2);
        let exit_atr = Decimal::from_f64(current_state.atr).unwrap_or_default();
        let exit_multiplier = Decimal::from_f64(self.settings.exit_multiplier).unwrap_or_default();
        
        let exit_upper = hl2 + (exit_multiplier * exit_atr);
        let exit_lower = hl2 - (exit_multiplier * exit_atr);

        if self.last_signal_side == Some(Side::Long) && current_kline.close < exit_lower {
            self.last_signal_side = None;
            return Signal::Close;
        }

        if self.last_signal_side == Some(Side::Short) && current_kline.close > exit_upper {
            self.last_signal_side = None;
            return Signal::Close;
        }

        Signal::Hold
    }
}

// Helper trait to easily calculate EMA on a slice of f64
trait EmaExt {
    fn ema(&self, period: usize) -> Option<f64>;
}

impl EmaExt for [f64] {
    fn ema(&self, period: usize) -> Option<f64> {
        if self.len() < period {
            return None;
        }
        let mut ema = Ema::new(period).ok()?;
        let mut last = None;
        self.iter().for_each(|v| {
            last = Some(ema.next(*v));
        });
        last
    }
}