-- Add up migration script here
-- In the new up.sql file
CREATE TABLE optimization_summaries (
    id BIGSERIAL PRIMARY KEY,
    job_id BIGINT NOT NULL UNIQUE REFERENCES optimization_jobs(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- A JSONB column to store the array of top N ranked results.
    -- This is the most flexible way to store a summary report.
    top_n_results JSONB NOT NULL
);

CREATE INDEX idx_optimization_summaries_job_id ON optimization_summaries(job_id);