use crate::types::{EquityPoint, PerformanceReport, Trade};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rust_decimal::prelude::*;

/// The engine responsible for calculating performance metrics from trade data.
#[derive(Default)]
pub struct AnalyticsEngine;

impl AnalyticsEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculates a full performance report from a set of trades and an equity curve.
    pub fn calculate(
        &self,
        initial_capital: Decimal,
        trades: &[Trade],
        equity_curve: &[EquityPoint],
    ) -> PerformanceReport {
        let mut report = PerformanceReport::new();
        if trades.is_empty() {
            return report; // Return a default report if there are no trades.
        }

        // --- Tier 1 Calculations ---

        // 1. Total Trades
        report.total_trades = trades.len() as u32;

        // 2. Net P&L (Absolute & Percentage)
        report.net_pnl_absolute = trades.iter().map(|t| t.pnl).sum();
        if initial_capital > dec!(0) {
            report.net_pnl_percentage = (report.net_pnl_absolute / initial_capital)
                .to_f64()
                .unwrap_or(0.0) * 100.0;
        }

        // 3. Win Rate & Profit Factor
        let winning_trades: Vec<&Trade> = trades.iter().filter(|t| t.pnl > dec!(0)).collect();
        let losing_trades: Vec<&Trade> = trades.iter().filter(|t| t.pnl < dec!(0)).collect();
        report.win_rate = (winning_trades.len() as f64 / report.total_trades as f64) * 100.0;

        let gross_profit: Decimal = winning_trades.iter().map(|t| t.pnl).sum();
        let gross_loss: Decimal = losing_trades.iter().map(|t| t.pnl).sum::<Decimal>().abs();
        report.profit_factor = if gross_loss > dec!(0) {
            (gross_profit / gross_loss).to_f64().unwrap_or(0.0)
        } else {
            f64::INFINITY // Pure profit
        };

        // 4. Max Drawdown (Absolute & Percentage)
        let mut peak_equity = initial_capital;
        let mut max_drawdown = dec!(0);
        for point in equity_curve {
            peak_equity = peak_equity.max(point.value);
            let drawdown = peak_equity - point.value;
            max_drawdown = max_drawdown.max(drawdown);
        }
        report.max_drawdown_absolute = max_drawdown;
        if peak_equity > dec!(0) {
            report.max_drawdown_percentage = (max_drawdown / peak_equity).to_f64().unwrap_or(0.0) * 100.0;
        }

        // 5. Sharpe Ratio (Simplified)
        if equity_curve.len() > 1 {
            let returns: Vec<f64> = equity_curve
                .windows(2)
                .map(|w| (w[1].value / w[0].value - dec!(1)).to_f64().unwrap_or(0.0))
                .collect();
            let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
            let std_dev = {
                let variance = returns.iter().map(|r| (*r - mean_return).powi(2)).sum::<f64>() / returns.len() as f64;
                variance.sqrt()
            };
            report.sharpe_ratio = if std_dev > 0.0 {
                mean_return / std_dev
            } else {
                0.0 // Or f64::INFINITY if mean_return > 0
            };
            // Note: This is a periodic Sharpe. To annualize, multiply by sqrt(periods per year).
        }

        // --- Tier 2 Calculations ---

        // 6. Sortino Ratio (Measures return against downside deviation)
        if equity_curve.len() > 1 {
            let returns: Vec<f64> = equity_curve
                .windows(2)
                .map(|w| (w[1].value / w[0].value - dec!(1)).to_f64().unwrap_or(0.0))
                .collect();
            
            let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
            
            // Calculate downside deviation (standard deviation of negative returns only)
            let negative_returns: Vec<f64> = returns.iter().cloned().filter(|r| *r < 0.0).collect();
            let downside_deviation = if !negative_returns.is_empty() {
                let variance = negative_returns.iter().map(|r| (*r - 0.0).powi(2)).sum::<f64>() / negative_returns.len() as f64;
                variance.sqrt()
            } else {
                0.0
            };

            report.sortino_ratio = if downside_deviation > 0.0 {
                mean_return / downside_deviation
            } else {
                f64::INFINITY // No downside risk
            };
        }

        // 7. Calmar Ratio (Annualized Return / Max Drawdown)
        // Note: Proper annualization needs the full backtest duration.
        // We will approximate for now.
        if report.max_drawdown_percentage > 0.0 {
            // Placeholder: Assume 1 year backtest for now.
            let annualized_return = report.net_pnl_percentage; 
            report.calmar_ratio = annualized_return / report.max_drawdown_percentage;
        }

        // 8. Average Trade Duration
        if !trades.is_empty() {
            let total_duration_secs: i64 = trades.iter().map(|t| (t.exit_time - t.entry_time).num_seconds()).sum();
            report.avg_trade_duration_secs = total_duration_secs as f64 / trades.len() as f64;
        }

        // 9. Expectancy (Average P&L per trade)
        if !trades.is_empty() {
            report.expectancy = report.net_pnl_absolute / Decimal::from(trades.len());
        }

        // --- Tier 3 ("Atlas") Calculations ---

        // 10. Confidence-Weighted Performance Analysis
        let mut confidence_map: std::collections::HashMap<String, Vec<&Trade>> = std::collections::HashMap::new();
        for trade in trades {
            let bucket = match (trade.signal_confidence * 100.0) as u32 {
                0..=59 => "0-59%",
                60..=69 => "60-69%",
                70..=79 => "70-79%",
                80..=89 => "80-89%",
                90..=100 => "90-100%",
                _ => "Other",
            };
            confidence_map.entry(bucket.to_string()).or_default().push(trade);
        }

        for (bucket_name, bucket_trades) in confidence_map {
            // We can't get an equity curve per bucket, so we'll calculate simpler metrics.
            // A more advanced version might generate sub-reports.
            // For now, let's just create a simplified sub-report.
            let mut sub_report = PerformanceReport::new();
            if !bucket_trades.is_empty() {
                sub_report.total_trades = bucket_trades.len() as u32;
                sub_report.net_pnl_absolute = bucket_trades.iter().map(|t| t.pnl).sum();
                let wins = bucket_trades.iter().filter(|t| t.pnl > dec!(0)).count();
                sub_report.win_rate = (wins as f64 / sub_report.total_trades as f64) * 100.0;
            }
            report.confidence_performance.insert(bucket_name, sub_report);
        }

        // 11. Leverage-Adjusted Return on Margin (LAROM)
        // This requires knowing margin used, which is complex. We will approximate it.
        // Approximation: Margin Used = Position Value / Leverage
        if !trades.is_empty() {
            let avg_leverage: f64 = trades.iter().map(|t| t.leverage as f64).sum::<f64>() / trades.len() as f64;
            let avg_margin_used: Decimal = trades.iter().map(|t| (t.entry_price * t.quantity) / Decimal::from(t.leverage)).sum::<Decimal>() / Decimal::from(trades.len());
            
            if avg_margin_used > dec!(0) && avg_leverage > 0.0 {
                report.larom = (report.net_pnl_absolute / (avg_margin_used * Decimal::from_f64(avg_leverage).unwrap_or(dec!(1))))
                    .to_f64()
                    .unwrap_or(0.0);
            }
        }
        
        // 12. Funding Rate Impact (Placeholder)
        // This requires funding data to be logged with each trade.
        // We will assume it's zero for now and build the structure.
        report.funding_pnl = dec!(0); // Placeholder

        // 13. Drawdown Duration
        let mut in_drawdown = false;
        let mut drawdown_start_time = None;
        let mut max_drawdown_duration = chrono::Duration::zero();
        let mut peak_equity_for_duration = initial_capital;

        for point in equity_curve {
            if point.value >= peak_equity_for_duration {
                // We've reached a new peak or recovered from a drawdown
                if in_drawdown {
                    let duration = point.timestamp - drawdown_start_time.unwrap();
                    if duration > max_drawdown_duration {
                        max_drawdown_duration = duration;
                    }
                    in_drawdown = false;
                }
                peak_equity_for_duration = point.value;
            } else {
                // We are currently in a drawdown
                if !in_drawdown {
                    in_drawdown = true;
                    drawdown_start_time = Some(point.timestamp);
                }
            }
        }
        report.drawdown_duration_secs = max_drawdown_duration.num_seconds();

        report
    }
} 