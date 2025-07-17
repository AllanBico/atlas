-- Add up migration script here
-- In up.sql

-- Table to store the parameters of each backtest run.
CREATE TABLE backtest_runs (
    id BIGSERIAL PRIMARY KEY,
    strategy_name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    interval TEXT NOT NULL,
    start_date TIMESTAMPTZ NOT NULL,
    end_date TIMESTAMPTZ NOT NULL,
    parameters JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Table to store the calculated performance metrics for each backtest run.
CREATE TABLE performance_reports (
    id BIGSERIAL PRIMARY KEY,
    run_id BIGINT NOT NULL UNIQUE REFERENCES backtest_runs(id) ON DELETE CASCADE,

    -- Tier 1 Metrics
    net_pnl_absolute NUMERIC(30, 15) NOT NULL,
    net_pnl_percentage DOUBLE PRECISION NOT NULL,
    max_drawdown_absolute NUMERIC(30, 15) NOT NULL,
    max_drawdown_percentage DOUBLE PRECISION NOT NULL,
    sharpe_ratio DOUBLE PRECISION NOT NULL,
    win_rate DOUBLE PRECISION NOT NULL,
    profit_factor DOUBLE PRECISION NOT NULL,
    total_trades INTEGER NOT NULL,

    -- Tier 2 Metrics
    sortino_ratio DOUBLE PRECISION NOT NULL,
    calmar_ratio DOUBLE PRECISION NOT NULL,
    avg_trade_duration_secs BIGINT NOT NULL,
    expectancy NUMERIC(30, 15) NOT NULL,

    -- Tier 3 Metrics
    confidence_performance JSONB, -- Stored as JSON
    larom DOUBLE PRECISION NOT NULL,
    funding_pnl NUMERIC(30, 15) NOT NULL,
    drawdown_duration_secs BIGINT NOT NULL
);

-- Create an index on run_id for faster lookups.
CREATE INDEX idx_performance_reports_run_id ON performance_reports(run_id);