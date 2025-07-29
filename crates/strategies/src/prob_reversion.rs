// In crates/strategies/src/prob_reversion.rs

use crate::types::ProbReversionSettings;
use crate::{Signal, Strategy};
use core_types::Kline;
use rust_decimal::prelude::*;
use ta::indicators::{BollingerBands, RelativeStrengthIndex as Rsi, SimpleMovingAverage as Sma};
use ta::{Next};

/// Minimal ADX calculation (Wilder's smoothing)
fn calculate_adx(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let mut tr = Vec::with_capacity(highs.len());
    let mut plus_dm = Vec::with_capacity(highs.len());
    let mut minus_dm = Vec::with_capacity(highs.len());
    for i in 0..highs.len() {
        if i == 0 {
            tr.push(0.0);
            plus_dm.push(0.0);
            minus_dm.push(0.0);
        } else {
            let high_diff = highs[i] - highs[i - 1];
            let low_diff = lows[i - 1] - lows[i];
            let up_move = if high_diff > 0.0 && high_diff > low_diff { high_diff } else { 0.0 };
            let down_move = if low_diff > 0.0 && low_diff > high_diff { low_diff } else { 0.0 };
            plus_dm.push(up_move);
            minus_dm.push(down_move);
            let tr_val = (highs[i] - lows[i])
                .max((highs[i] - closes[i - 1]).abs())
                .max((lows[i] - closes[i - 1]).abs());
            tr.push(tr_val);
        }
    }
    // Wilder's smoothing for TR, +DM, -DM
    let mut atr = vec![0.0; highs.len()];
    let mut plus_dm_smooth = vec![0.0; highs.len()];
    let mut minus_dm_smooth = vec![0.0; highs.len()];
    let mut plus_di = vec![0.0; highs.len()];
    let mut minus_di = vec![0.0; highs.len()];
    let mut dx = vec![0.0; highs.len()];
    let mut adx = vec![0.0; highs.len()];
    if highs.len() > period {
        atr[period] = tr[1..=period].iter().sum::<f64>() / period as f64;
        plus_dm_smooth[period] = plus_dm[1..=period].iter().sum::<f64>() / period as f64;
        minus_dm_smooth[period] = minus_dm[1..=period].iter().sum::<f64>() / period as f64;
        for i in period + 1..highs.len() {
            atr[i] = (atr[i - 1] * (period as f64 - 1.0) + tr[i]) / period as f64;
            plus_dm_smooth[i] = (plus_dm_smooth[i - 1] * (period as f64 - 1.0) + plus_dm[i]) / period as f64;
            minus_dm_smooth[i] = (minus_dm_smooth[i - 1] * (period as f64 - 1.0) + minus_dm[i]) / period as f64;
        }
        for i in period..highs.len() {
            plus_di[i] = if atr[i] != 0.0 { 100.0 * (plus_dm_smooth[i] / atr[i]) } else { 0.0 };
            minus_di[i] = if atr[i] != 0.0 { 100.0 * (minus_dm_smooth[i] / atr[i]) } else { 0.0 };
            let denom = plus_di[i] + minus_di[i];
            dx[i] = if denom != 0.0 { 100.0 * ((plus_di[i] - minus_di[i]).abs() / denom) } else { 0.0 };
        }
        adx[period * 2] = dx[period..=period * 2].iter().sum::<f64>() / period as f64;
        for i in period * 2 + 1..highs.len() {
            adx[i] = (adx[i - 1] * (period as f64 - 1.0) + dx[i]) / period as f64;
        }
    }
    adx
}

/// The stateful struct for the Probabilistic Reversion strategy.
#[derive(Debug)]
pub struct ProbReversion {
    settings: ProbReversionSettings,
    // Indicators from the `ta` crate
    bbands: BollingerBands,
    rsi: Rsi,
    rsi_sma: Sma,
    // Internal state for multi-stage confirmation
    prev_rsi_sma: f64,
    pending_buy_signal_close: Option<f64>,
    // Tracks current position to generate exit signals
    in_position: bool,
}

impl ProbReversion {
    /// Creates a new `ProbReversion` strategy instance.
    pub fn new(settings: ProbReversionSettings) -> Self {
        let bband_period = settings.bband_period as usize;
        let rsi_period = settings.rsi_period as usize;
        let rsi_smoothing = settings.rsi_smoothing as usize;

        Self {
            settings: settings.clone(),
            bbands: BollingerBands::new(bband_period, settings.bband_stddev).unwrap(),
            rsi: Rsi::new(rsi_period).unwrap(),
            rsi_sma: Sma::new(rsi_smoothing).unwrap(),
            prev_rsi_sma: 0.0,
            pending_buy_signal_close: None,
            in_position: false,
        }
    }
}

impl Strategy for ProbReversion {
    fn name(&self) -> &'static str {
        "ProbabilisticReversion"
    }

    fn assess(&mut self, klines: &[Kline]) -> Signal {
        // Determine the longest lookback period required by any indicator
        let longest_lookback = (self.settings.adx_period * 2)
            .max(self.settings.bband_period)
            .max(self.settings.rsi_period + self.settings.rsi_smoothing);

        if klines.len() < longest_lookback as usize {
            return Signal::Hold;
        }

        // --- Data Preparation & Indicator Calculation ---
        let closes: Vec<f64> = klines.iter().map(|k| k.close.to_f64().unwrap_or(0.0)).collect();
        let highs: Vec<f64> = klines.iter().map(|k| k.high.to_f64().unwrap_or(0.0)).collect();
        let lows: Vec<f64> = klines.iter().map(|k| k.low.to_f64().unwrap_or(0.0)).collect();

        // ADX (using yata batch calculation)
        let adx_period = self.settings.adx_period as usize;
        let adx_values = calculate_adx(&highs, &lows, &closes, adx_period);
        let current_adx = *adx_values.last().unwrap_or(&0.0);

        // BollingerBands
        let mut bbands = self.bbands.clone();
        let mut last_bbands = None;
        for c in closes.iter() {
            last_bbands = Some(bbands.next(*c));
        }
        let current_bbands = last_bbands.unwrap();

        // RSI values
        let mut rsi = self.rsi.clone();
        let rsi_values: Vec<f64> = closes.iter().map(|c| rsi.next(*c)).collect();

        // SimpleMovingAverage for RSI
        let mut rsi_sma = self.rsi_sma.clone();
        let mut last_rsi_sma = 0.0;
        for v in rsi_values.iter() {
            last_rsi_sma = rsi_sma.next(*v);
        }
        let current_rsi_sma = last_rsi_sma;
        
        let current_kline = klines.last().unwrap();
        let current_close = closes.last().unwrap();
        let current_low = lows.last().unwrap();
        let current_rsi = *rsi_values.last().unwrap();

        // --- Logic Flow (translated from Go) ---

        // 1. Check for EXIT signal first. If in a position, check if price reverted to the mean.
        if self.in_position && *current_close >= current_bbands.average {
            self.in_position = false;
            self.pending_buy_signal_close = None;
            return Signal::Close;
        }

        // 2. Check for ENTRY CONFIRMATION.
        if let Some(setup_close) = self.pending_buy_signal_close {
            self.pending_buy_signal_close = None; // Consume the pending signal
            if *current_close > setup_close {
                self.in_position = true; // Mark that we've entered a position
                return Signal::GoLong { confidence: self.settings.confidence };
            }
        }

        // 3. Look for a NEW SETUP condition on the current bar.

        // FILTER 1: Regime Filter (is market ranging?)
        if current_adx >= self.settings.adx_range_threshold {
            self.prev_rsi_sma = current_rsi_sma;
            return Signal::Hold;
        }

        // FILTER 2: Location Filter (is price at an extreme low?)
        let is_location_met = *current_low <= current_bbands.lower;

        // FILTER 3: Momentum Filter (is selling pressure exhausted?)
        let is_momentum_met = current_rsi < self.settings.rsi_oversold && current_rsi_sma > self.prev_rsi_sma;

        // If all filters are met, set up a pending signal for the *next* bar to confirm.
        if is_location_met && is_momentum_met {
            self.pending_buy_signal_close = Some(*current_close);
        } else {
            self.pending_buy_signal_close = None;
        }

        // Update state for the next iteration
        self.prev_rsi_sma = current_rsi_sma;

        Signal::Hold
    }
}