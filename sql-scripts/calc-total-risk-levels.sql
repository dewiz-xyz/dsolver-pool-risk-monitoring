select
    a.currencies,
    a.pool_address,
    a.pool,
    max(b.id::text)::uuid                AS id,
    max(a.amounts_out::text)::jsonb      AS amounts_out,
    max(a.gas_used::text)::jsonb         AS gas_used,
    max(a.block_number)                  AS block_number,
    max(a.slippage_bps::text)::jsonb     AS slippage_bps,
    max(a.pool_utilization_bps)          AS pool_utilization_bps,
    max(a.simulation_result_id::text)::uuid AS simulation_result_id,
    a.risk_level,
    max(a.risk_score)                    AS risk_score,
    count(a.pool_name)                   AS total
FROM vw_pool_risk vpr  AS a
JOIN result AS b ON b.id = simulation_result_id
WHERE a.risk_score >= 1000
GROUP BY a.pool_address, a.pool_name, a.risk_level
order by a.currencies, a.pool, a.pool_address