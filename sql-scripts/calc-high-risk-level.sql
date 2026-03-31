SELECT
    a.currencies,
    a.pool_address,
    a.pool,
    max(a.block_number)                     AS block_number,
    a.risk_level,
    max(a.risk_score)                       AS risk_score,
    count(*)                                AS total
FROM vw_pool_risk AS a
WHERE a.risk_score >= 1000
GROUP BY a.pool_address, a.pool, a.currencies, a.risk_level
ORDER BY a.currencies, a.pool, a.pool_address;