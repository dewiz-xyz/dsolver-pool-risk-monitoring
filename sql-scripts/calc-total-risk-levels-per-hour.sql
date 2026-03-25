select
	a.pool_name,
	TO_CHAR(b.created_at , 'YYYY.MM.DD.HH24') as extraction_date,
	a.risk_level,
	count(a.pool_name) as total_assessment_per_risk_type
FROM pool_result AS a
JOIN result AS b ON b.id = a.simulation_result_id
group by a.pool_name, extraction_date, a.risk_level
order by a.pool_name, extraction_date
-- where  = 'aerodrome_slipstreams::USDC/USDT'