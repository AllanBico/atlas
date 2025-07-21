pub mod error;
pub mod types;
use strategies::Strategy;
use risk::RiskManager;
use execution::Executor;
use core_types::{Kline, OrderRequest, Signal, Side};
use rust_decimal_macros::dec;
use chrono::{Utc, TimeZone};
use num_traits::ToPrimitive;
mod logger;
use logger::TradeLogger;
use analytics::engine::AnalyticsEngine;
use analytics::types::{EquityPoint, PerformanceReport, Trade};

/// The main engine for running historical backtests.
pub struct Backtester {
    /// The symbol to be tested.
    pub symbol: core_types::Symbol,
    /// The timeframe interval for the test.
    pub interval: String,
    /// A single strategy instance to test.
    pub strategy: Box<dyn Strategy + Send>, // No more lifetime
    /// The risk manager instance.
    pub risk_manager: Box<dyn RiskManager + Send>,
    /// The execution simulator.
    pub executor: Box<dyn Executor + Send>,
    logger: TradeLogger,
}

const KLINE_HISTORY_SIZE: usize = 100;

impl Backtester {
    pub fn new(
        symbol: core_types::Symbol,
        interval: String,
        strategy: Box<dyn Strategy + Send>, // No more lifetime
        risk_manager: Box<dyn RiskManager + Send>,
        executor: Box<dyn Executor + Send>,
    ) -> Self {
        Self {
            symbol,
            interval,
            strategy,
            risk_manager,
            executor,
            logger: TradeLogger::new(),
        }
    }

    // Change the return type from anyhow::Result<()> to anyhow::Result<PerformanceReport>
    pub async fn run(&mut self, klines: Vec<Kline>) -> anyhow::Result<(PerformanceReport, Vec<Trade>, Vec<EquityPoint>)> {
        for i in KLINE_HISTORY_SIZE..klines.len() {
            let current_kline = &klines[i];
            let history_slice = &klines[(i - KLINE_HISTORY_SIZE)..i];

            // --- At the beginning of the loop ---
            self.logger.record_equity(
                Utc.timestamp_millis_opt(current_kline.open_time).unwrap(),
                self.executor.portfolio().cash
            );

            // --- 1. Check for Stop-Loss Trigger ---
            let position_to_check = self.executor.portfolio().open_positions.get(&self.symbol).cloned();
            if let Some(open_position) = position_to_check {
                let stop_triggered = if open_position.side == Side::Long {
                    current_kline.low <= open_position.sl_price
                } else {
                    current_kline.high >= open_position.sl_price
                };

                if stop_triggered {
                    tracing::info!(
                        time = %Utc.timestamp_millis_opt(current_kline.open_time).unwrap(),
                        sl_price = open_position.sl_price.to_f64().unwrap_or(0.0),
                        trigger_price = if open_position.side == Side::Long { current_kline.low.to_f64().unwrap_or(0.0) } else { current_kline.high.to_f64().unwrap_or(0.0) },
                        "Stop-loss triggered!"
                    );

                    let close_order = OrderRequest {
                        symbol: open_position.symbol.clone(),
                        side: if open_position.side == Side::Long { Side::Short } else { Side::Long },
                        quantity: open_position.quantity,
                        leverage: open_position.leverage,
                        sl_price: dec!(0),
                        originating_signal: Signal::Close,
                    };

                    let execution_result = self.executor.execute(&close_order, open_position.sl_price, current_kline.open_time).await;
                    if let Ok((execution, Some(closed_pos))) = execution_result {
                        self.logger.record_trade(&closed_pos, &execution, Utc.timestamp_millis_opt(current_kline.open_time).unwrap());
                        tracing::info!(?execution, "Stop-loss order executed.");
                    } else if let Ok((execution, None)) = execution_result {
                        tracing::warn!(?execution, "Stop-loss order executed but no closed position returned.");
                    } else if let Err(e) = execution_result {
                        tracing::error!(error = %e, "Failed to execute stop-loss order.");
                    }
                    continue;
                }
            }

            // --- 2. Assess Strategy for New Signals (if no SL was hit) ---
            let signal = self.strategy.assess(history_slice);
            if matches!(signal, Signal::Hold) {
                continue;
            }

            // --- 3. Evaluate Signal with Risk Manager ---
            let portfolio_value = self.executor.portfolio().cash;
            let open_position = self.executor.portfolio().open_positions.get(&self.symbol);
            let calculation_kline = &klines[i - 1];
            let order_request_result = self.risk_manager.evaluate(
                &signal,
                portfolio_value,
                &self.symbol, // Pass the symbol
                calculation_kline,
                open_position,
            );

            // --- 4. Execute Approved Order ---
            match order_request_result {
                Ok(Some(order_request)) => {
                    let execution_result = self.executor.execute(&order_request, calculation_kline.close, calculation_kline.open_time).await;
                    match execution_result {
                        Ok((execution, Some(closed_pos))) => {
                            self.logger.record_trade(&closed_pos, &execution, Utc.timestamp_millis_opt(calculation_kline.open_time).unwrap());
                            tracing::info!(?execution, "Order executed and trade logged.");
                        }
                        Ok((execution, None)) => {
                            tracing::info!(?execution, "Order executed (entry or no position closed).");
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Order execution failed.");
                        }
                    }
                }
                Ok(None) => {
                    // No action needed
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Risk manager vetoed the signal.");
                }
            }
        }
        tracing::info!(trades = ?self.logger.trades, "--- Logged Trades ---");
        tracing::info!(portfolio = ?self.executor.portfolio(), "Backtest finished. Final portfolio state:");

        // --- Analytics Calculation & Reporting ---
        let initial_capital = self.executor.portfolio().initial_capital;
        let analytics_engine = AnalyticsEngine::new();
        let report = analytics_engine.calculate(
            initial_capital,
            &self.logger.trades,
            &self.logger.equity_curve,
        );

        print_report(&report);

        // Return the calculated report and the trade log
        Ok((report, self.logger.trades.clone(), self.logger.equity_curve.clone()))
    }
}

/// Helper function to print the performance report in a readable format.
fn print_report(report: &PerformanceReport) {
    println!("\n--- Backtest Performance Report ---");
    println!("-----------------------------------");
    // Tier 1
    println!("Net P&L:               ${:.2} ({:.2}%)", report.net_pnl_absolute, report.net_pnl_percentage);
    println!("Max Drawdown:          ${:.2} ({:.2}%)", report.max_drawdown_absolute, report.max_drawdown_percentage);
    println!("Sharpe Ratio:          {:.3}", report.sharpe_ratio);
    println!("Profit Factor:         {:.2}", report.profit_factor);
    println!("Win Rate:              {:.2}%", report.win_rate);
    println!("Total Trades:          {}", report.total_trades);
    println!("-----------------------------------");
    // Tier 2
    println!("Sortino Ratio:         {:.3}", report.sortino_ratio);
    println!("Calmar Ratio:          {:.3}", report.calmar_ratio);
    println!("Avg. Trade Duration:   {:.1}s", report.avg_trade_duration_secs);
    println!("Expectancy:            ${:.2}", report.expectancy);
    println!("-----------------------------------");
    // Tier 3
    println!("LAROM:                 {:.3}", report.larom);
    println!("Funding P&L:           ${:.2}", report.funding_pnl);
    println!("Max Drawdown Duration: {}s", report.drawdown_duration_secs);
    println!("-----------------------------------");

    // Confidence-Weighted Analysis
    if !report.confidence_performance.is_empty() {
        println!("Confidence-Weighted Performance:");
        let mut sorted_buckets: Vec<_> = report.confidence_performance.iter().collect();
        sorted_buckets.sort_by_key(|(k, _)| *k);

        for (bucket, sub_report) in sorted_buckets {
            println!(
                "  - Bucket '{}': Trades = {}, Win Rate = {:.1}%, P&L = ${:.2}",
                bucket,
                sub_report.total_trades,
                sub_report.win_rate,
                sub_report.net_pnl_absolute
            );
        }
        println!("-----------------------------------");
    }
}