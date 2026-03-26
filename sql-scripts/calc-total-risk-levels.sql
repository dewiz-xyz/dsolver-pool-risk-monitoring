select
	SPLIT_PART(a.pool_name, '::', 2) AS currencies,
	a.pool_address,
	SPLIT_PART(a.pool_name, '::', 1) AS pool,
    a.risk_level,
	count(a.pool_name) as total
FROM pool_result AS a
JOIN result AS b ON b.id = a.simulation_result_id
--where a.pool_name = 'aerodrome_slipstreams::USDC/USDT'
group by a.pool_address, a.pool_name, a.risk_level
order by currencies, pool, a.pool_address;