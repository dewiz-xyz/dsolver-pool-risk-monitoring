use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{
    ApiListResponse, HealthResponse, ListPoolsQuery, ListResultsQuery,
    PoolResultRow, RiskLevelSummaryRow, SimulationResultRow,
};

/// Shared state injected into every handler.
#[derive(Clone)]
pub struct ApiState {
    pub db: PgPool,
    pub prom: PrometheusHandle,
    pub api_key: Option<String>,
    pub risk_score_threshold: i32,
    pub rate_limit_rps: u64,
    /// Sliding-window counter: packed as (epoch_second << 32 | count).
    rate_counter: Arc<AtomicU64>,
}

impl ApiState {
    pub fn new(
        db: PgPool,
        prom: PrometheusHandle,
        api_key: Option<String>,
        risk_score_threshold: i32,
        rate_limit_rps: u64,
    ) -> Self {
        Self {
            db,
            prom,
            api_key,
            risk_score_threshold,
            rate_limit_rps,
            rate_counter: Arc::new(AtomicU64::new(0)),
        }
    }
}

pub fn router(state: Arc<ApiState>) -> Router {
    // Public routes (no auth required)
    let public = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(prometheus_metrics));

    // Protected routes
    let protected = Router::new()
        .route("/api/v1/results", get(list_results))
        .route("/api/v1/results/{id}", get(get_result))
        .route("/api/v1/results/{id}/pools", get(get_pools_for_result))
        .route("/api/v1/pools", get(list_pools))
        .route("/api/v1/pools/high-risk", get(high_risk_pools))
        .route("/api/v1/pools/risk-summary", get(risk_level_summary))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth,
        ));

    public
        .merge(protected)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit,
        ))
        .with_state(state)
}

// ─── Rate Limit Middleware ───────────────────────────────────────

async fn rate_limit(
    State(state): State<Arc<ApiState>>,
    request: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;

    loop {
        let current = state.rate_counter.load(Ordering::Relaxed);
        let stored_sec = (current >> 32) as u32;
        let count = current as u32;

        let (new_sec, new_count) = if stored_sec == now_secs {
            if u64::from(count) >= state.rate_limit_rps {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            (now_secs, count + 1)
        } else {
            (now_secs, 1)
        };

        let new_val = (u64::from(new_sec) << 32) | u64::from(new_count);
        if state
            .rate_counter
            .compare_exchange(current, new_val, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            break;
        }
    }

    Ok(next.run(request).await)
}

// ─── API Key Auth Middleware ─────────────────────────────────────

async fn api_key_auth(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    if let Some(ref expected) = state.api_key {
        let provided = headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok());

        match provided {
            Some(key) if key == expected => {}
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
    }
    Ok(next.run(request).await)
}

// ─── Handlers ────────────────────────────────────────────────────

async fn health(State(state): State<Arc<ApiState>>) -> Json<HealthResponse> {
    let db_status = match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => "connected".to_string(),
        Err(e) => format!("error: {e}"),
    };

    let status = if db_status == "connected" {
        "ok"
    } else {
        "degraded"
    };

    Json(HealthResponse {
        status: status.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        database: db_status,
    })
}

async fn prometheus_metrics(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    state.prom.render()
}

async fn list_results(
    State(state): State<Arc<ApiState>>,
    Query(q): Query<ListResultsQuery>,
) -> Result<Json<ApiListResponse<SimulationResultRow>>, AppApiError> {
    let limit = q.limit.unwrap_or(50).min(500);
    let offset = q.offset.unwrap_or(0);

    let rows = if let Some(min_risk) = q.min_risk_score {
        sqlx::query_as::<_, SimulationResultRow>(
            r#"
            SELECT DISTINCT r.*
            FROM result r
            JOIN pool_result pr ON pr.simulation_result_id = r.id
            WHERE pr.risk_score >= $1
            ORDER BY r.created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(min_risk)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, SimulationResultRow>(
            r#"
            SELECT * FROM result
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(ApiListResponse {
        count: rows.len(),
        data: rows,
    }))
}

async fn get_result(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<SimulationResultRow>, AppApiError> {
    let row = sqlx::query_as::<_, SimulationResultRow>(
        "SELECT * FROM result WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppApiError::NotFound)?;

    Ok(Json(row))
}

async fn get_pools_for_result(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiListResponse<PoolResultRow>>, AppApiError> {
    let rows = sqlx::query_as::<_, PoolResultRow>(
        r#"
        SELECT * FROM pool_result
        WHERE simulation_result_id = $1
        ORDER BY risk_score DESC
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(ApiListResponse {
        count: rows.len(),
        data: rows,
    }))
}

async fn list_pools(
    State(state): State<Arc<ApiState>>,
    Query(q): Query<ListPoolsQuery>,
) -> Result<Json<ApiListResponse<PoolResultRow>>, AppApiError> {
    let limit = q.limit.unwrap_or(50).min(500);
    let offset = q.offset.unwrap_or(0);
    let min_risk = q.min_risk_score.unwrap_or(0);

    let rows = if let Some(ref addr) = q.pool_address {
        sqlx::query_as::<_, PoolResultRow>(
            r#"
            SELECT * FROM pool_result
            WHERE pool_address = $1 AND risk_score >= $2
            ORDER BY block_number DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(addr)
        .bind(min_risk)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, PoolResultRow>(
            r#"
            SELECT * FROM pool_result
            WHERE risk_score >= $1
            ORDER BY block_number DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(min_risk)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(ApiListResponse {
        count: rows.len(),
        data: rows,
    }))
}

/// Returns pool results with risk_score >= configurable threshold,
/// ordered by risk_score descending.
async fn high_risk_pools(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiListResponse<PoolResultRow>>, AppApiError> {
    let rows = sqlx::query_as::<_, PoolResultRow>(
        r#"
        SELECT pr.* FROM pool_result pr
        JOIN result r ON r.id = pr.simulation_result_id
        WHERE pr.risk_score >= $1
        ORDER BY pr.risk_score DESC, r.created_at DESC
        LIMIT 100
        "#,
    )
    .bind(state.risk_score_threshold)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(ApiListResponse {
        count: rows.len(),
        data: rows,
    }))
}

/// Returns aggregated risk-level counts per pool per hour,
/// derived from `calc-total-risk-levels.sql`.
async fn risk_level_summary(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiListResponse<RiskLevelSummaryRow>>, AppApiError> {
    let rows = sqlx::query_as::<_, RiskLevelSummaryRow>(
        r#"
        SELECT
            a.pool_name,
            TO_CHAR(b.created_at, 'YYYY.MM.DD.HH24') AS extraction_date,
            a.risk_level,
            COUNT(a.pool_name) AS total_assessment_per_risk_type
        FROM pool_result AS a
        JOIN result AS b ON b.id = a.simulation_result_id
        GROUP BY a.pool_name, extraction_date, a.risk_level
        ORDER BY a.pool_name, extraction_date
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(ApiListResponse {
        count: rows.len(),
        data: rows,
    }))
}

// ─── Error type for API handlers ─────────────────────────────────

enum AppApiError {
    NotFound,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for AppApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e)
    }
}

impl IntoResponse for AppApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "not found"})),
            )
                .into_response(),
            Self::Database(e) => {
                tracing::error!(error = %e, "database error in API handler");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "internal server error"})),
                )
                    .into_response()
            }
        }
    }
}
