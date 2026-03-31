-- Expression indexes to support filtering and sorting on the computed
-- pool / currencies columns exposed by the vw_pool_risk view.
--
-- The planner will use these when it sees the matching SPLIT_PART expression
-- in WHERE, ORDER BY, or JOIN conditions — including queries through the view,
-- because PostgreSQL inlines simple views before planning.

CREATE INDEX IF NOT EXISTS idx_pool_result_pool
    ON pool_result (SPLIT_PART(pool_name, '::', 1));

CREATE INDEX IF NOT EXISTS idx_pool_result_currencies
    ON pool_result (SPLIT_PART(pool_name, '::', 2));

CREATE INDEX idx_pool_result_pool_currencies
    ON pool_result (SPLIT_PART(pool_name, '::', 1), SPLIT_PART(pool_name, '::', 2));
