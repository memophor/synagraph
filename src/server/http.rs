use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use tokio::net::TcpListener;

use crate::config::AppConfig;

#[derive(Serialize)]
struct HealthResponse {
    service: String,
    version: String,
    status: String,
}

pub async fn serve(cfg: AppConfig) -> Result<()> {
    let addr: SocketAddr = cfg.http_addr;
    let app = Router::new()
        .route("/health", get(health_handler))
        .with_state(cfg.clone());

    let listener = TcpListener::bind(addr)
        .await
        .context("failed to bind HTTP listener")?;

    tracing::info!(%addr, "http server listening");

    axum::serve(listener, app)
        .await
        .context("http server error")
}

async fn health_handler(State(cfg): State<AppConfig>) -> Json<HealthResponse> {
    Json(HealthResponse {
        service: cfg.service_name,
        version: cfg.version,
        status: "ok".to_string(),
    })
}
