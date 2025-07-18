// In app/src/analyzer.rs

use anyhow::Result;
use database::{Db, FullReport};
use serde::Serialize;

const MINIMUM_TRADES_THRESHOLD: u32 = 30;

#[derive(Debug, Serialize)]
pub struct RankedReport {
    pub score: f64,
    pub report: FullReport,
}

/// Analyzes and ranks the results of an optimization job.
pub async fn analyze_and_rank_results(db: &Db, job_id: i64) -> Result<Vec<RankedReport>> {
    tracing::info!(job_id, "Fetching and analyzing reports for optimization job...");

    let reports = db.get_reports_for_job(job_id).await?;
    let total_reports = reports.len();

    let mut ranked_reports: Vec<RankedReport> = reports
        .into_iter()
        .filter_map(|full_report| {
            // 1. Filter out runs with too few trades
            if full_report.report.total_trades < MINIMUM_TRADES_THRESHOLD {
                return None;
            }

            // 2. Calculate the score
            let score = calculate_score(&full_report.report);

            Some(RankedReport {
                score,
                report: full_report,
            })
        })
        .collect();
    
    tracing::info!(
        total_reports,
        passing_reports = ranked_reports.len(),
        "Finished scoring reports."
    );

    // 3. Sort by score in descending order (higher is better)
    ranked_reports.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    Ok(ranked_reports)
}

/// The multi-objective scoring function.
/// Higher scores are better.
fn calculate_score(report: &analytics::types::PerformanceReport) -> f64 {
    // Define weights for each metric
    const PROFIT_FACTOR_WEIGHT: f64 = 40.0;
    const SHARPE_RATIO_WEIGHT: f64 = 30.0;
    const MAX_DRAWDOWN_WEIGHT: f64 = -35.0; // Negative weight penalizes drawdown
    const CALMAR_RATIO_WEIGHT: f64 = 15.0;

    // Normalize or cap values to prevent extreme outliers from dominating the score
    let capped_profit_factor = report.profit_factor.min(5.0); // Cap at 5.0
    let capped_sharpe = report.sharpe_ratio.min(5.0);
    let normalized_drawdown = report.max_drawdown_percentage / 100.0; // Convert to 0-1 scale

    let score = (capped_profit_factor * PROFIT_FACTOR_WEIGHT)
        + (capped_sharpe * SHARPE_RATIO_WEIGHT)
        + (normalized_drawdown * MAX_DRAWDOWN_WEIGHT)
        + (report.calmar_ratio * CALMAR_RATIO_WEIGHT);

    score
}