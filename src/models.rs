use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Request Models ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SimulationRequest {
    pub request_id: String,
    pub token_in: String,
    pub token_out: String,
    pub amounts: Vec<String>,
    pub pool_type: String,
}

/// Parameters supplied by the caller — the request_id (UUID) is generated
/// internally by the call function, not provided here.
#[derive(Debug, Clone)]
pub struct SimulationParams {
    pub token_in: String,
    pub token_out: String,
    pub amounts: Vec<String>,
    pub pool_type: String,
}

// ─── Response Models ──────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SimulationResponse {
    pub request_id: String,
    pub data: Vec<PoolResult>,
    pub meta: Meta,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PoolResult {
    pub pool: String,
    pub pool_name: String,
    pub pool_address: String,
    pub amounts_out: Vec<String>,
    pub gas_used: Vec<u64>,
    pub block_number: u64,
    pub slippage_bps: Vec<i32>,
    pub pool_utilization_bps: i32,
    pub execution_risk: ExecutionRisk,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecutionRisk {
    pub risk_score: i32,
    pub risk_level: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Meta {
    pub status: String,
    pub result_quality: String,
    pub block_number: u64,
    pub vm_block_number: Option<u64>,
    pub matching_pools: u32,
    pub candidate_pools: u32,
    pub total_pools: u32,
}

// ─── Database Row Models ─────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct SimulationResultRow {
    pub id: Uuid,
    pub request_id: String,
    pub response_payload: serde_json::Value,
    pub block_number: i64,
    pub matching_pools: i32,
    pub candidate_pools: i32,
    pub total_pools: i32,
    pub status: String,
    pub result_quality: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct PoolResultRow {
    pub id: Uuid,
    pub simulation_result_id: Uuid,
    pub pool_address: String,
    pub currencies: String,
    pub pool: String,
    pub amounts_out: serde_json::Value,
    pub gas_used: serde_json::Value,
    pub block_number: i64,
    pub slippage_bps: serde_json::Value,
    pub pool_utilization_bps: i32,
    pub risk_score: i32,
    pub risk_level: String,
}

// ─── API Response Types ──────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiListResponse<T: Serialize> {
    pub count: usize,
    pub data: Vec<T>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub database: String,
}

/// Query parameters for listing simulation results.
#[derive(Debug, Deserialize)]
pub struct ListResultsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub min_risk_score: Option<i32>,
}

/// Query parameters for listing pool results.
#[derive(Debug, Deserialize)]
pub struct ListPoolsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub pool_address: Option<String>,
    pub min_risk_score: Option<i32>,
}

/// One row from the risk-level summary aggregation query.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RiskLevelSummaryRow {
    pub currencies: String,
    pub pool_address: String,
    pub pool: String,
    pub extraction_date: String,
    pub risk_level: String,
    pub total_assessment_per_risk_type: i64,
}
