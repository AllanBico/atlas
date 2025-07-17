-- Add down migration script here
-- In down.sql

-- Drop the new primary key.
ALTER TABLE klines DROP CONSTRAINT klines_pkey;

-- Remove the 'interval' column.
ALTER TABLE klines DROP COLUMN interval;

-- Re-create the original primary key.
ALTER TABLE klines ADD PRIMARY KEY (symbol, open_time);