use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{PoolResult, SimulationResponse};

/// Creates a connection pool with sensible defaults for a 4-core machine.
pub async fn create_pool(database_url: &str) -> Result<PgPool, AppError> {
    let pool = PgPoolOptions::new()
        .max_connections(16) // 4 cores × 4 connections each
        .min_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(database_url)
        .await
        .map_err(AppError::Database)?;

    Ok(pool)
}

/// Runs the migration SQL to ensure tables exist.
pub async fn run_migrations(pool: &PgPool) -> Result<(), AppError> {
    let migration_sql = include_str!("../migrations/001_create_tables.sql");
    sqlx::raw_sql(migration_sql)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;

    tracing::info!("database migrations applied");
    Ok(())
}

/// Persists a full simulation response: inserts into `result` and `pool_result` tables
/// within a single transaction.
pub async fn insert_simulation_result(
    pool: &PgPool,
    call_id: Uuid,
    response: &SimulationResponse,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(AppError::Database)?;

    let payload = serde_json::to_value(response).map_err(AppError::Serialization)?;

    sqlx::query(
        r#"
        INSERT INTO result (
            id, request_id, response_payload, block_number,
            matching_pools, candidate_pools, total_pools,
            status, result_quality
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(call_id)
    .bind(&response.request_id)
    .bind(&payload)
    .bind(response.meta.block_number as i64)
    .bind(response.meta.matching_pools as i32)
    .bind(response.meta.candidate_pools as i32)
    .bind(response.meta.total_pools as i32)
    .bind(&response.meta.status)
    .bind(&response.meta.result_quality)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    for pool_data in &response.data {
        insert_pool_result(&mut tx, call_id, pool_data).await?;
    }

    tx.commit().await.map_err(AppError::Database)?;

    tracing::debug!(call_id = %call_id, "simulation result persisted");
    Ok(())
}

async fn insert_pool_result(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    simulation_result_id: Uuid,
    pool_data: &PoolResult,
) -> Result<(), AppError> {
    let amounts_out =
        serde_json::to_value(&pool_data.amounts_out).map_err(AppError::Serialization)?;
    let gas_used =
        serde_json::to_value(&pool_data.gas_used).map_err(AppError::Serialization)?;
    let slippage_bps =
        serde_json::to_value(&pool_data.slippage_bps).map_err(AppError::Serialization)?;

    sqlx::query(
        r#"
        INSERT INTO pool_result (
            id, simulation_result_id, pool_address, pool_name,
            amounts_out, gas_used, block_number,
            slippage_bps, pool_utilization_bps, risk_score, risk_level
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(simulation_result_id)
    .bind(&pool_data.pool_address)
    .bind(&pool_data.pool_name)
    .bind(&amounts_out)
    .bind(&gas_used)
    .bind(pool_data.block_number as i64)
    .bind(&slippage_bps)
    .bind(pool_data.pool_utilization_bps)
    .bind(pool_data.execution_risk.risk_score)
    .bind(&pool_data.execution_risk.risk_level)
    .execute(&mut **tx)
    .await
    .map_err(AppError::Database)?;

    Ok(())
}
