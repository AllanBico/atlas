-- Add up migration script here
-- In up.sql

-- We can't add a NOT NULL column to an existing table without a default,
-- but we also don't want a permanent default. The strategy is to add it with a
-- temporary default, update existing rows (if any), and then remove the default.
-- For our fresh start, this is simpler.

-- Drop the old primary key constraint to modify its columns.
ALTER TABLE klines DROP CONSTRAINT klines_pkey;

-- Add the new 'interval' column.
ALTER TABLE klines ADD COLUMN interval TEXT;

-- To handle existing data, you might run an UPDATE here. For our new setup, we can skip.
-- UPDATE klines SET interval = 'unknown';

-- Now enforce NOT NULL on the column.
ALTER TABLE klines ALTER COLUMN interval SET NOT NULL;

-- Create the new, more specific primary key.
ALTER TABLE klines ADD PRIMARY KEY (symbol, interval, open_time);