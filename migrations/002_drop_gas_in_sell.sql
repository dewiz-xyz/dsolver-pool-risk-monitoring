-- Migration: drop the gas_in_sell column from pool_result
-- Safe to run against databases that still have the column (IF EXISTS guard).
-- No-op if the column was never present.

ALTER TABLE pool_result
    DROP COLUMN IF EXISTS gas_in_sell;
