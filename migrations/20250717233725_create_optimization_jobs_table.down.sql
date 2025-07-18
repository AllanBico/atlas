-- Add down migration script here
-- In the new down.sql file
ALTER TABLE backtest_runs DROP COLUMN job_id;
DROP TABLE optimization_jobs;