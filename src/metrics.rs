use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::PrometheusBuilder;

/// Installs the Prometheus recorder globally. Call once at startup.
/// Returns the `PrometheusHandle` needed to render /metrics output.
pub fn install_prometheus() -> metrics_exporter_prometheus::PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder")
}

// ─── Simulation-level metrics ────────────────────────────────────────────

pub fn record_simulation_success(pair_label: &str) {
    counter!("simulation_requests_total", "pair" => pair_label.to_string(), "status" => "success")
        .increment(1);
}

pub fn record_simulation_failure(pair_label: &str) {
    counter!("simulation_requests_total", "pair" => pair_label.to_string(), "status" => "error")
        .increment(1);
}

pub fn record_simulation_duration_ms(pair_label: &str, ms: f64) {
    histogram!("simulation_duration_ms", "pair" => pair_label.to_string()).record(ms);
}

// ─── Pool-level metrics ──────────────────────────────────────────────────

pub fn record_risk_score(pool_address: &str, score: i32) {
    histogram!("pool_risk_score", "pool" => pool_address.to_string()).record(f64::from(score));
}

pub fn record_slippage_bps(pool_address: &str, bps: i32) {
    histogram!("pool_slippage_bps", "pool" => pool_address.to_string()).record(f64::from(bps));
}

pub fn record_gas_used(pool_address: &str, gas: u64) {
    histogram!("pool_gas_used", "pool" => pool_address.to_string()).record(gas as f64);
}

pub fn record_pool_utilization_bps(pool_address: &str, bps: i32) {
    histogram!("pool_utilization_bps", "pool" => pool_address.to_string()).record(f64::from(bps));
}

// ─── Gauges for latest-cycle summary ─────────────────────────────────────

pub fn set_matching_pools(count: u32) {
    gauge!("latest_matching_pools").set(f64::from(count));
}

pub fn set_candidate_pools(count: u32) {
    gauge!("latest_candidate_pools").set(f64::from(count));
}

pub fn set_block_number(block: u64) {
    gauge!("latest_block_number").set(block as f64);
}

// ─── Alert counters ──────────────────────────────────────────────────────

pub fn record_alert_fired(alert_type: &str) {
    counter!("alerts_fired_total", "type" => alert_type.to_string()).increment(1);
}
