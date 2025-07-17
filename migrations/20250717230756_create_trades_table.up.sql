-- Add up migration script here
-- In the new up.sql file
CREATE TABLE trades (
    id BIGSERIAL PRIMARY KEY,
    run_id BIGINT NOT NULL REFERENCES backtest_runs(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL, -- "Long" or "Short"
    entry_time TIMESTAMPTZ NOT NULL,
    exit_time TIMESTAMPTZ NOT NULL,
    entry_price NUMERIC(30, 15) NOT NULL,
    exit_price NUMERIC(30, 15) NOT NULL,
    quantity NUMERIC(30, 15) NOT NULL,
    pnl NUMERIC(30, 15) NOT NULL,
    fees NUMERIC(30, 15) NOT NULL,
    signal_confidence DOUBLE PRECISION NOT NULL,
    leverage INTEGER NOT NULL
);

-- An index on the foreign key is crucial for performance
CREATE INDEX idx_trades_run_id ON trades(run_id);