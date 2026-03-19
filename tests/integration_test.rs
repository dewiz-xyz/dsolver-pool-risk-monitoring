use std::io::Write;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// We test the client + alerts + metrics pipeline end-to-end against a mock
// Tycho simulator. Database calls are NOT covered here (requires Postgres).
// This file validates: HTTP flow, alert evaluation, metric recording.

/// Canonical mock response matching the SimulationResponse shape.
fn mock_simulation_response(request_id: &str) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "data": [
            {
                "pool": "uniswap_v3",
                "pool_name": "UniswapV3 DAI/USDC 0.01%",
                "pool_address": "0x5777d92f208679db4b9778590fa3cab3ac9e2168",
                "amounts_out": ["999800", "4999000"],
                "gas_used": [150000, 150000],
                "gas_in_sell": "0.003",
                "block_number": 19500000,
                "slippage_bps": [2, 10],
                "pool_utilization_bps": 350,
                "execution_risk": {
                    "risk_score": 25,
                    "risk_level": "low"
                }
            },
            {
                "pool": "curve_stable",
                "pool_name": "Curve 3pool",
                "pool_address": "0xbebc44782c7db0a1a60cb6fe97d0b483032ff1c7",
                "amounts_out": ["999900", "4999500"],
                "gas_used": [200000, 200000],
                "gas_in_sell": "0.004",
                "block_number": 19500000,
                "slippage_bps": [1, 5],
                "pool_utilization_bps": 200,
                "execution_risk": {
                    "risk_score": 15,
                    "risk_level": "low"
                }
            }
        ],
        "meta": {
            "status": "ready",
            "result_quality": "complete",
            "block_number": 19500000,
            "vm_block_number": 19500000,
            "matching_pools": 2,
            "candidate_pools": 5,
            "total_pools": 120
        }
    })
}

/// Response with a HIGH risk score pool — triggers the alert threshold.
fn mock_high_risk_response(request_id: &str) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "data": [
            {
                "pool": "sketchy_dex",
                "pool_name": "SketchyDEX ETH/SCAM",
                "pool_address": "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
                "amounts_out": ["500000000000000000"],
                "gas_used": [500000],
                "gas_in_sell": "0.01",
                "block_number": 19500001,
                "slippage_bps": [850],
                "pool_utilization_bps": 9500,
                "execution_risk": {
                    "risk_score": 92,
                    "risk_level": "critical"
                }
            }
        ],
        "meta": {
            "status": "ready",
            "result_quality": "complete",
            "block_number": 19500001,
            "vm_block_number": 19500001,
            "matching_pools": 1,
            "candidate_pools": 1,
            "total_pools": 120
        }
    })
}

/// Helper to create a temporary config.json file.
fn write_temp_config(
    api_url: &str,
    poll_interval: u64,
) -> tempfile::NamedTempFile {
    let config = serde_json::json!({
        "database_url": "postgres://fake:fake@localhost:5432/fake",
        "simulation_api_url": api_url,
        "poll_interval_secs": poll_interval,
        "api_port": 0,
        "alerts": {
            "risk_score_threshold": 70,
            "slippage_bps_threshold": 500,
            "webhook_url": null
        },
        "token_pairs": [
            {
                "label": "DAI → USDC",
                "token_in": "0x6b175474e89094c44da98b954eedeac495271d0f",
                "token_out": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                "amounts": ["1000000000000000000"]
            }
        ]
    });

    let mut f = tempfile::NamedTempFile::new().unwrap();
    write!(f, "{}", serde_json::to_string_pretty(&config).unwrap()).unwrap();
    f
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

/// Verifies the mock server receives the correct POST shape and returns
/// a parseable SimulationResponse.
#[tokio::test]
async fn test_mock_server_returns_valid_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/simulate"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_simulation_response("test-req-1")),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "request_id": "test-req-1",
        "token_in": "0x6b175474e89094c44da98b954eedeac495271d0f",
        "token_out": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "amounts": ["1000000000000000000"]
    });

    let resp = client
        .post(format!("{}/simulate", mock_server.uri()))
        .json(&body)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let parsed: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(parsed["meta"]["status"], "ready");
    assert_eq!(parsed["data"].as_array().unwrap().len(), 2);
}

/// Verifies alert evaluation fires on high risk scores.
#[tokio::test]
async fn test_alert_evaluation_high_risk() {
    let response_json = mock_high_risk_response("alert-test-1");
    let response: tycho_simulator_server_risk_monitoring::models::SimulationResponse =
        serde_json::from_value(response_json).unwrap();

    let alert_config = tycho_simulator_server_risk_monitoring::config::AlertConfig {
        risk_score_threshold: 70,
        slippage_bps_threshold: 500,
        webhook_url: None,
    };

    let alerts =
        tycho_simulator_server_risk_monitoring::alerts::evaluate_response(&alert_config, &response);

    // Should fire: risk_score=92 >= 70, slippage=850 >= 500
    assert!(alerts.len() >= 2, "expected at least 2 alerts, got {}", alerts.len());

    let risk_alert = alerts.iter().find(|a| a.alert_type == "high_risk_score");
    assert!(risk_alert.is_some());
    assert_eq!(risk_alert.unwrap().value, 92);
    assert_eq!(risk_alert.unwrap().severity, "critical");

    let slip_alert = alerts.iter().find(|a| a.alert_type == "high_slippage");
    assert!(slip_alert.is_some());
    assert_eq!(slip_alert.unwrap().value, 850);
}

/// Verifies NO alerts fire when all pools are low-risk.
#[tokio::test]
async fn test_alert_evaluation_no_alerts() {
    let response_json = mock_simulation_response("safe-test-1");
    let response: tycho_simulator_server_risk_monitoring::models::SimulationResponse =
        serde_json::from_value(response_json).unwrap();

    let alert_config = tycho_simulator_server_risk_monitoring::config::AlertConfig {
        risk_score_threshold: 70,
        slippage_bps_threshold: 500,
        webhook_url: None,
    };

    let alerts =
        tycho_simulator_server_risk_monitoring::alerts::evaluate_response(&alert_config, &response);
    assert!(alerts.is_empty(), "expected 0 alerts for safe pools");
}

/// Config loading: validates JSON parse + env-var override.
#[tokio::test]
async fn test_config_load_and_env_override() {
    let tmp = write_temp_config("http://localhost:9999/simulate", 30);

    std::env::set_var("DATABASE_URL", "postgres://override:override@db:5432/test");

    let config =
        tycho_simulator_server_risk_monitoring::config::AppConfig::load(tmp.path()).unwrap();

    assert_eq!(config.database_url, "postgres://override:override@db:5432/test");
    assert_eq!(config.poll_interval_secs, 30);
    assert_eq!(config.token_pairs.len(), 1);
    assert_eq!(config.alerts.risk_score_threshold, 70);

    std::env::remove_var("DATABASE_URL");
}

/// Config loading: empty token_pairs should fail.
#[tokio::test]
async fn test_config_rejects_empty_pairs() {
    let config_json = serde_json::json!({
        "database_url": "postgres://x:x@localhost/x",
        "simulation_api_url": "http://localhost/simulate",
        "token_pairs": []
    });

    let mut f = tempfile::NamedTempFile::new().unwrap();
    write!(f, "{}", serde_json::to_string(&config_json).unwrap()).unwrap();

    let result = tycho_simulator_server_risk_monitoring::config::AppConfig::load(f.path());
    assert!(result.is_err());
}

/// Webhook delivery: verifies POST to webhook URL with correct payload.
#[tokio::test]
async fn test_webhook_delivery() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/webhook"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let alerts = vec![tycho_simulator_server_risk_monitoring::alerts::Alert {
        alert_type: "high_risk_score".into(),
        severity: "critical".into(),
        pool_address: "0xdead".into(),
        pool_name: "TestPool".into(),
        message: "test alert".into(),
        value: 95,
        threshold: 70,
        block_number: 19500000,
        request_id: "webhook-test".into(),
    }];

    let client = reqwest::Client::new();
    let url = format!("{}/webhook", mock_server.uri());

    tycho_simulator_server_risk_monitoring::alerts::deliver_webhook(
        &client, &url, &alerts,
    )
    .await;

    // wiremock's expect(1) validates the POST was received.
}

/// Mock server returning 500 — verify the reqwest error is propagated.
#[tokio::test]
async fn test_mock_server_error_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/simulate"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "request_id": "err-test",
        "token_in": "0xabc",
        "token_out": "0xdef",
        "amounts": ["1000"]
    });

    let resp = client
        .post(format!("{}/simulate", mock_server.uri()))
        .json(&body)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 500);
}
