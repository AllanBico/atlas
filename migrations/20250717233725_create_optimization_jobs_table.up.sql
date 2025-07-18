-- Add up migration script here
-- In the new up.sql file

-- 1. Create the parent table for optimization jobs
CREATE TABLE optimization_jobs (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2. Add a column to `backtest_runs` to link it to a job
-- This column is nullable because a single backtest run is not part of a job.
ALTER TABLE backtest_runs
ADD COLUMN job_id BIGINT REFERENCES optimization_jobs(id) ON DELETE SET NULL;

-- 3. Create an index for faster lookups
CREATE INDEX idx_backtest_runs_job_id ON backtest_runs(job_id);