-- Add up migration script here
-- In up.sql
-- Create the klines table
CREATE TABLE klines (
    symbol TEXT NOT NULL,
    open_time BIGINT NOT NULL,
    open DECIMAL(30, 15) NOT NULL,
    high DECIMAL(30, 15) NOT NULL,
    low DECIMAL(30, 15) NOT NULL,
    close DECIMAL(30, 15) NOT NULL,
    volume DECIMAL(30, 15) NOT NULL,
    close_time BIGINT NOT NULL,
    PRIMARY KEY (symbol, open_time)
);