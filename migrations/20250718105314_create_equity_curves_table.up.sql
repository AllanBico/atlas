-- In the new up.sql file
CREATE TABLE equity_curves (
    id BIGSERIAL PRIMARY KEY,
    run_id BIGINT NOT NULL REFERENCES backtest_runs(id) ON DELETE CASCADE,
    timestamp TIMESTAMPTZ NOT NULL,
    equity NUMERIC(30, 15) NOT NULL
);

-- An index on the run_id and timestamp is vital for fast querying to draw the graph
CREATE INDEX idx_equity_curves_run_id_timestamp ON equity_curves(run_id, timestamp);