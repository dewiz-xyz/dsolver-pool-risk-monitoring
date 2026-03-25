use std::path::Path;

use serde::Deserialize;

use crate::errors::AppError;

/// Top-level application configuration loaded from a JSON file.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    /// Postgres connection string.
    pub database_url: String,
    /// Tycho simulator endpoint (POST).
    pub simulation_api_url: String,
    /// Seconds between polling cycles. 0 = single-shot mode.
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Port for the Axum REST API + Prometheus /metrics endpoint.
    #[serde(default = "default_api_port")]
    pub api_port: u16,
    /// Alerting thresholds.
    #[serde(default)]
    pub alerts: AlertConfig,
    /// Retry configuration for simulation API calls.
    #[serde(default)]
    pub retry: RetryConfig,
    /// Optional API key for authenticating REST API requests.
    /// If set, clients must send `X-API-Key: <value>` header.
    pub api_key: Option<String>,
    /// Maximum requests per second per IP for rate limiting.
    #[serde(default = "default_rate_limit_rps")]
    pub rate_limit_rps: u64,
    /// Token pairs to simulate.
    pub token_pairs: Vec<TokenPairConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenPairConfig {
    /// Human-readable label, e.g. "DAI → USDC".
    pub label: String,
    /// Checksummed or lowercase ERC-20 address.
    pub token_in: String,
    /// Checksummed or lowercase ERC-20 address.
    pub token_out: String,
    /// Raw amounts as decimal strings (wei / smallest unit).
    pub amounts: Vec<String>,
    /// Pool type for alerting and metrics labeling (e.g. "blue_chip", "volatile").
    pub pool_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertConfig {
    /// Risk score threshold — any pool result >= this fires an alert.
    #[serde(default = "default_risk_score_threshold")]
    pub risk_score_threshold: i32,
    /// Slippage (bps) threshold — any individual slippage >= this fires an alert.
    #[serde(default = "default_slippage_bps_threshold")]
    pub slippage_bps_threshold: i32,
    /// Optional webhook URL for alert delivery (POST JSON payload).
    pub webhook_url: Option<String>,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            risk_score_threshold: default_risk_score_threshold(),
            slippage_bps_threshold: default_slippage_bps_threshold(),
            webhook_url: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (0 = no retries).
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Initial backoff delay in milliseconds.
    #[serde(default = "default_initial_backoff_ms")]
    pub initial_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_backoff_ms: default_initial_backoff_ms(),
        }
    }
}

fn default_max_retries() -> u32 {
    3
}
fn default_initial_backoff_ms() -> u64 {
    500
}
fn default_poll_interval() -> u64 {
    60
}
fn default_api_port() -> u16 {
    3000
}
fn default_rate_limit_rps() -> u64 {
    10
}
fn default_risk_score_threshold() -> i32 {
    70
}
fn default_slippage_bps_threshold() -> i32 {
    500
}

impl AppConfig {
    /// Load configuration from a JSON file, with env-var overrides for secrets.
    ///
    /// Priority: env var > JSON value.
    pub fn load(path: &Path) -> Result<Self, AppError> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            AppError::Config(format!("failed to read config file {}: {e}", path.display()))
        })?;

        let mut config: AppConfig = serde_json::from_str(&contents)
            .map_err(|e| AppError::Config(format!("invalid config JSON: {e}")))?;

        // Allow env-var overrides for secrets / CI environments.
        if let Ok(url) = std::env::var("DATABASE_URL") {
            config.database_url = url;
        }
        if let Ok(url) = std::env::var("SIMULATION_API_URL") {
            config.simulation_api_url = url;
        }
        if let Ok(port) = std::env::var("API_PORT") {
            config.api_port = port
                .parse()
                .map_err(|_| AppError::Config("API_PORT must be a valid u16".into()))?;
        }
        if let Ok(key) = std::env::var("API_KEY") {
            config.api_key = Some(key);
        }

        if config.token_pairs.is_empty() {
            return Err(AppError::Config(
                "token_pairs must contain at least one entry".into(),
            ));
        }

        Ok(config)
    }
}

/// Converts configured token pairs into SimulationParams.
impl From<&TokenPairConfig> for crate::models::SimulationParams {
    fn from(tp: &TokenPairConfig) -> Self {
        Self {
            token_in: tp.token_in.clone(),
            token_out: tp.token_out.clone(),
            amounts: tp.amounts.clone(),
            pool_type: tp.pool_type.clone(),
        }
    }
}
