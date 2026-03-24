select
	a.simulation_result_id,
	a.pool_name,
	a.risk_level, 
	a.risk_score,
	a.block_number,
	b.result_quality,
	a.slippage_bps,
	a.pool_utilization_bps,
	b.created_at
FROM pool_result AS a
JOIN result AS b ON b.id = a.simulation_result_id
where a.pool_name = 'aerodrome_slipstreams::USDC/USDT'
ORDER BY a.pool_name, b.created_at, a.simulation_result_id,  a.risk_level desc
limit 1000;