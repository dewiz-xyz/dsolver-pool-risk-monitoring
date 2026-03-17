-- Simulation results: one row per API call
CREATE TABLE IF NOT EXISTS result (
    id                UUID PRIMARY KEY,
    request_id        TEXT        NOT NULL,
    response_payload  JSONB       NOT NULL,
    block_number      BIGINT      NOT NULL,
    matching_pools    INTEGER     NOT NULL,
    candidate_pools   INTEGER     NOT NULL,
    total_pools       INTEGER     NOT NULL,
    status            TEXT        NOT NULL,
    result_quality    TEXT        NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_result_request_id   ON result (request_id);
CREATE INDEX IF NOT EXISTS idx_result_block_number ON result (block_number);
CREATE INDEX IF NOT EXISTS idx_result_created_at   ON result (created_at);

-- Individual pool results: many per simulation result
CREATE TABLE IF NOT EXISTS pool_result (
    id                    UUID PRIMARY KEY,
    simulation_result_id  UUID        NOT NULL REFERENCES result(id) ON DELETE CASCADE,
    pool_address          TEXT        NOT NULL,
    pool_name             TEXT        NOT NULL,
    amounts_out           JSONB       NOT NULL,
    gas_used              JSONB       NOT NULL,
    gas_in_sell           TEXT        NOT NULL,
    block_number          BIGINT      NOT NULL,
    slippage_bps          JSONB       NOT NULL,
    pool_utilization_bps  INTEGER     NOT NULL,
    risk_score            INTEGER     NOT NULL,
    risk_level            TEXT        NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pool_result_simulation ON pool_result (simulation_result_id);
CREATE INDEX IF NOT EXISTS idx_pool_result_address    ON pool_result (pool_address);
CREATE INDEX IF NOT EXISTS idx_pool_result_risk_score ON pool_result (risk_score);
