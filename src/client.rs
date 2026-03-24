use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use crate::alerts::{self, Alert};
use crate::config::{AlertConfig, RetryConfig};
use crate::errors::AppError;
use crate::metrics;
use crate::models::{SimulationParams, SimulationRequest, SimulationResponse};

/// Shared state passed into each spawned task.
pub struct SimulationClient {
    http: Client,
    db_pool: PgPool,
    target_url: String,
    alert_config: AlertConfig,
    retry_config: RetryConfig,
}

impl SimulationClient {
    pub fn new(
        target_url: String,
        db_pool: PgPool,
        alert_config: AlertConfig,
        retry_config: RetryConfig,
    ) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(16)
            .build()
            .expect("failed to build HTTP client");

        Self {
            http,
            db_pool,
            target_url,
            alert_config,
            retry_config,
        }
    }

    /// POSTs to the simulator with exponential backoff retries.
    async fn send_with_retry(
        &self,
        request_body: &SimulationRequest,
    ) -> Result<SimulationResponse, AppError> {
        let max_attempts = self.retry_config.max_retries + 1;
        let mut backoff_ms = self.retry_config.initial_backoff_ms;

        for attempt in 1..=max_attempts {
            let result = self
                .http
                .post(&self.target_url)
                .json(request_body)
                .send()
                .await;

            match result {
                Ok(resp) => match resp.error_for_status() {
                    Ok(resp) => return resp.json::<SimulationResponse>().await.map_err(Into::into),
                    Err(e) if attempt < max_attempts && is_retryable_status(&e) => {
                        tracing::warn!(
                            attempt,
                            max_attempts,
                            error = %e,
                            backoff_ms,
                            "retryable HTTP error, backing off"
                        );
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2;
                    }
                    Err(e) => return Err(e.into()),
                },
                Err(e) if attempt < max_attempts && !e.is_builder() => {
                    tracing::warn!(
                        attempt,
                        max_attempts,
                        error = %e,
                        backoff_ms,
                        "retryable request error, backing off"
                    );
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= 2;
                }
                Err(e) => return Err(e.into()),
            }
        }

        unreachable!("loop always returns")
    }

    /// Executes a single simulation call:
    /// 1. Generates a UUID call_id
    /// 2. POSTs to the remote server (with retries)
    /// 3. Records Prometheus metrics for every pool result
    /// 4. Evaluates alerting thresholds
    /// 5. Persists the response in Postgres keyed by call_id
    ///
    /// Returns (call_id, fired_alerts) on success.
    pub async fn execute_simulation(
        &self,
        params: &SimulationParams,
        pair_label: &str,
    ) -> Result<(Uuid, Vec<Alert>), AppError> {
        let call_id = Uuid::new_v4();
        let request_id = call_id.to_string();
        let start = Instant::now();

        let request_body = SimulationRequest {
            request_id: request_id.clone(),
            token_in: params.token_in.clone(),
            token_out: params.token_out.clone(),
            amounts: params.amounts.clone(),
        };

        tracing::info!(
            call_id = %call_id,
            pair = %pair_label,
            token_in = %params.token_in,
            token_out = %params.token_out,
            "sending simulation request"
        );

        let response = self.send_with_retry(&request_body).await?;

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        metrics::record_simulation_duration_ms(pair_label, elapsed_ms);
        metrics::record_simulation_success(pair_label);

        tracing::info!(
            call_id = %call_id,
            pair = %pair_label,
            pools_found = response.data.len(),
            block = response.meta.block_number,
            elapsed_ms = elapsed_ms,
            "response received"
        );

        // ── Record per-pool metrics ──────────────────────────────
        metrics::set_block_number(response.meta.block_number);
        metrics::set_matching_pools(response.meta.matching_pools);
        metrics::set_candidate_pools(response.meta.candidate_pools);

        for pool in &response.data {
            metrics::record_risk_score(
                &pool.pool_address,
                pool.execution_risk.risk_score,
            );
            for &bps in &pool.slippage_bps {
                metrics::record_slippage_bps(&pool.pool_address, bps);
            }
            for &gas in &pool.gas_used {
                metrics::record_gas_used(&pool.pool_address, gas);
            }
            metrics::record_pool_utilization_bps(
                &pool.pool_address,
                pool.pool_utilization_bps,
            );
            metrics::record_risk_level(&pool.pool_name, &pool.execution_risk.risk_level);
        }

        // ── Evaluate alerts ──────────────────────────────────────
        let fired = alerts::evaluate_response(&self.alert_config, &response);

        if !fired.is_empty() {
            if let Some(ref url) = self.alert_config.webhook_url {
                alerts::deliver_webhook(&self.http, url, &fired).await;
            }
        }

        // ── Persist ──────────────────────────────────────────────
        crate::db::insert_simulation_result(&self.db_pool, call_id, &response).await?;

        Ok((call_id, fired))
    }
}

/// Dispatches all simulation params in parallel using `tokio::spawn` across
/// all available worker threads.
pub async fn run_all_simulations(
    client: Arc<SimulationClient>,
    params_list: Vec<(String, SimulationParams)>,
) -> Vec<Result<(Uuid, Vec<Alert>), AppError>> {
    let handles: Vec<_> = params_list
        .into_iter()
        .map(|(label, params)| {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client.execute_simulation(&params, &label).await
            })
        })
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(join_err) => {
                tracing::error!(error = %join_err, "task panicked");
                results.push(Err(AppError::TaskPanic(join_err.to_string())));
            }
        }
    }

    results
}

/// Returns true for HTTP status codes worth retrying (429, 5xx).
fn is_retryable_status(e: &reqwest::Error) -> bool {
    e.status().is_some_and(|s| s == 429 || s.is_server_error())
}
