use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::signal;
use tracing_subscriber::EnvFilter;

use tycho_simulator_server_risk_monitoring::api::{self, ApiState};
use tycho_simulator_server_risk_monitoring::client::{run_all_simulations, SimulationClient};
use tycho_simulator_server_risk_monitoring::config::AppConfig;
use tycho_simulator_server_risk_monitoring::db;
use tycho_simulator_server_risk_monitoring::metrics;
use tycho_simulator_server_risk_monitoring::models::SimulationParams;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to build Tokio runtime");

    rt.block_on(async_main());
}

async fn async_main() {
    // ── Tracing ──────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    // ── Config ───────────────────────────────────────────────────
    dotenvy::dotenv().ok();

    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config.json".into());

    let config = AppConfig::load(&PathBuf::from(&config_path))
        .expect("failed to load config");

    tracing::info!(
        pairs = config.token_pairs.len(),
        poll_secs = config.poll_interval_secs,
        api_port = config.api_port,
        "configuration loaded"
    );

    // ── Database ─────────────────────────────────────────────────
    let db_pool = db::create_pool(&config.database_url)
        .await
        .expect("failed to connect to Postgres");

    db::run_migrations(&db_pool)
        .await
        .expect("failed to run migrations");

    // ── Prometheus ────────────────────────────────────────────────
    let prom_handle = metrics::install_prometheus();

    // ── API Server (background) ──────────────────────────────────
    let api_state = Arc::new(ApiState::new(
        db_pool.clone(),
        prom_handle,
        config.api_key.clone(),
        config.alerts.risk_score_threshold,
        config.rate_limit_rps,
    ));

    let api_router = api::router(api_state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.api_port));

    let api_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("failed to bind API listener");
        tracing::info!(%addr, "API server listening");
        axum::serve(listener, api_router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("API server error");
    });

    // ── Simulation Client ────────────────────────────────────────
    let sim_client = Arc::new(SimulationClient::new(
        config.simulation_api_url.clone(),
        db_pool,
        config.alerts.clone(),
        config.retry.clone(),
    ));

    // ── Polling Loop ─────────────────────────────────────────────
    let poll_interval = config.poll_interval_secs;
    let token_pairs = config.token_pairs.clone();

    let poll_handle = tokio::spawn(async move {
        loop {
            let params_list: Vec<(String, SimulationParams)> = token_pairs
                .iter()
                .map(|tp| (tp.label.clone(), SimulationParams::from(tp)))
                .collect();

            tracing::info!(
                total_requests = params_list.len(),
                "starting simulation cycle"
            );

            let results = run_all_simulations(
                Arc::clone(&sim_client),
                params_list,
            )
            .await;

            // ── Cycle report ─────────────────────────────────────
            let mut success_count = 0u32;
            let mut error_count = 0u32;
            let mut total_alerts = 0usize;

            for result in &results {
                match result {
                    Ok((call_id, fired)) => {
                        tracing::info!(call_id = %call_id, alerts = fired.len(), "simulation completed");
                        success_count += 1;
                        total_alerts += fired.len();
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "simulation failed");
                        metrics::record_simulation_failure("unknown");
                        error_count += 1;
                    }
                }
            }

            tracing::info!(
                success = success_count,
                errors = error_count,
                alerts = total_alerts,
                "simulation cycle finished"
            );

            // ── Single-shot mode: poll_interval == 0 ─────────────
            if poll_interval == 0 {
                tracing::info!("single-shot mode, exiting polling loop");
                break;
            }

            tracing::info!(next_in_secs = poll_interval, "sleeping until next cycle");
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
        }
    });

    // ── Wait for either task to finish ───────────────────────────
    tokio::select! {
        _ = poll_handle => {
            tracing::info!("polling loop exited");
        }
        _ = api_handle => {
            tracing::info!("API server exited");
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
