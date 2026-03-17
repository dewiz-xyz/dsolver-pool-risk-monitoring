use crate::config::AlertConfig;
use crate::metrics;
use crate::models::{PoolResult, SimulationResponse};

/// Alert payload sent to the configured webhook (if any).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Alert {
    pub alert_type: String,
    pub severity: String,
    pub pool_address: String,
    pub pool_name: String,
    pub message: String,
    pub value: i32,
    pub threshold: i32,
    pub block_number: u64,
    pub request_id: String,
}

/// Evaluates a full simulation response against configured thresholds.
/// Returns all fired alerts. Also records metric counters per alert.
pub fn evaluate_response(
    config: &AlertConfig,
    response: &SimulationResponse,
) -> Vec<Alert> {
    let mut alerts = Vec::new();

    for pool in &response.data {
        // ── Risk score check ─────────────────────────────────────
        if pool.execution_risk.risk_score >= config.risk_score_threshold {
            let alert = Alert {
                alert_type: "high_risk_score".into(),
                severity: severity_for_risk(pool.execution_risk.risk_score),
                pool_address: pool.pool_address.clone(),
                pool_name: pool.pool_name.clone(),
                message: format!(
                    "Risk score {} >= threshold {} on pool {}",
                    pool.execution_risk.risk_score,
                    config.risk_score_threshold,
                    pool.pool_address,
                ),
                value: pool.execution_risk.risk_score,
                threshold: config.risk_score_threshold,
                block_number: pool.block_number,
                request_id: response.request_id.clone(),
            };
            metrics::record_alert_fired("high_risk_score");
            tracing::warn!(
                pool = %pool.pool_address,
                score = pool.execution_risk.risk_score,
                threshold = config.risk_score_threshold,
                "HIGH RISK SCORE alert"
            );
            alerts.push(alert);
        }

        // ── Slippage check (per amount) ──────────────────────────
        check_slippage(config, pool, response, &mut alerts);
    }

    alerts
}

fn check_slippage(
    config: &AlertConfig,
    pool: &PoolResult,
    response: &SimulationResponse,
    alerts: &mut Vec<Alert>,
) {
    for &bps in &pool.slippage_bps {
        if bps >= config.slippage_bps_threshold {
            let alert = Alert {
                alert_type: "high_slippage".into(),
                severity: severity_for_slippage(bps),
                pool_address: pool.pool_address.clone(),
                pool_name: pool.pool_name.clone(),
                message: format!(
                    "Slippage {bps} bps >= threshold {} bps on pool {}",
                    config.slippage_bps_threshold, pool.pool_address,
                ),
                value: bps,
                threshold: config.slippage_bps_threshold,
                block_number: pool.block_number,
                request_id: response.request_id.clone(),
            };
            metrics::record_alert_fired("high_slippage");
            tracing::warn!(
                pool = %pool.pool_address,
                slippage_bps = bps,
                threshold = config.slippage_bps_threshold,
                "HIGH SLIPPAGE alert"
            );
            alerts.push(alert);
        }
    }
}

/// Fire-and-forget webhook delivery. Logs errors but never blocks the loop.
pub async fn deliver_webhook(client: &reqwest::Client, url: &str, alerts: &[Alert]) {
    if alerts.is_empty() {
        return;
    }

    match client.post(url).json(alerts).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(count = alerts.len(), "alerts delivered to webhook");
        }
        Ok(resp) => {
            tracing::error!(
                status = %resp.status(),
                "webhook returned non-2xx"
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to deliver alerts to webhook");
        }
    }
}

fn severity_for_risk(score: i32) -> String {
    match score {
        90.. => "critical".into(),
        70.. => "warning".into(),
        _ => "info".into(),
    }
}

fn severity_for_slippage(bps: i32) -> String {
    match bps {
        1000.. => "critical".into(),
        500.. => "warning".into(),
        _ => "info".into(),
    }
}
