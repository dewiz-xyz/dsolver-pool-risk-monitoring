select
	SPLIT_PART(a.pool_name, '::', 2) AS currencies,
	a.pool_address,
	SPLIT_PART(a.pool_name, '::', 1) AS pool,
    TO_CHAR(b.created_at , 'YYYY.MM.DD.HH24') as extraction_date,
    a.risk_level,
	count(a.pool_name) as total
FROM pool_result AS a
JOIN result AS b ON b.id = a.simulation_result_id
--where a.pool_name = 'aerodrome_slipstreams::USDC/USDT'
group by a.pool_address, extraction_date, a.pool_name, a.risk_level
order by extraction_date, currencies, pool, a.pool_address;