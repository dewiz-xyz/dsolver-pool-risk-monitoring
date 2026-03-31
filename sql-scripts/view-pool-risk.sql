SELECT
    SPLIT_PART(a.pool_name, '::', 1)     AS pool,
    SPLIT_PART(a.pool_name, '::', 2)     AS currencies,
    a.pool_address,
    a.amounts_out,
    a.gas_used,
    a.block_number,
    a.slippage_bps,
    a.pool_utilization_bps,
    a.simulation_result_id,
    a.risk_level,
    a.risk_score
FROM pool_result AS a
ORDER BY pool, currencies, a.pool_address;