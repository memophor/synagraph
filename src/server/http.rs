// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Axum HTTP endpoints live here, including the readiness probe consumed by downstream systems.

use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use tokio::net::TcpListener;

use crate::config::AppConfig;
use crate::repository::NodeRepositoryHandle;

#[derive(Serialize)]
struct HealthResponse {
    service: String,
    version: String,
    status: String,
}

#[derive(Serialize)]
struct ReadyResponse {
    service: String,
    version: String,
    ready: bool,
    storage_ok: bool,
}

#[derive(Clone)]
struct HttpState {
    cfg: AppConfig,
    node_repo: NodeRepositoryHandle,
}

pub async fn serve(cfg: AppConfig, node_repo: NodeRepositoryHandle) -> Result<()> {
    let addr: SocketAddr = cfg.http_addr;
    let state = HttpState { cfg, node_repo };
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .with_state(state);

    let listener = TcpListener::bind(addr)
        .await
        .context("failed to bind HTTP listener")?;

    tracing::info!(%addr, "http server listening");

    axum::serve(listener, app)
        .await
        .context("http server error")
}

async fn health_handler(State(state): State<HttpState>) -> Json<HealthResponse> {
    let cfg = state.cfg;
    Json(HealthResponse {
        service: cfg.service_name,
        version: cfg.version,
        status: "ok".to_string(),
    })
}

async fn ready_handler(State(state): State<HttpState>) -> Json<ReadyResponse> {
    let HttpState { cfg, node_repo } = state;
    let storage_ok = match node_repo.health_check().await {
        Ok(_) => true,
        Err(err) => {
            tracing::error!(?err, "repository health check failed");
            false
        }
    };
    Json(ReadyResponse {
        service: cfg.service_name,
        version: cfg.version,
        ready: storage_ok,
        storage_ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::repository::in_memory::InMemoryNodeRepository;

    fn sample_config() -> AppConfig {
        AppConfig {
            http_addr: "127.0.0.1:0".parse().unwrap(),
            grpc_addr: "127.0.0.1:0".parse().unwrap(),
            service_name: "synagraph".into(),
            version: "0.1.0-test".into(),
            database_url: None,
            default_tenant_id: Uuid::new_v4(),
        }
    }

    fn sample_state() -> HttpState {
        HttpState {
            cfg: sample_config(),
            node_repo: Arc::new(InMemoryNodeRepository::new()),
        }
    }

    #[tokio::test]
    async fn health_handler_returns_ok_status() {
        let state = sample_state();
        let Json(response) = health_handler(State(state)).await;
        assert_eq!(response.service, "synagraph");
        assert_eq!(response.status, "ok");
    }

    #[tokio::test]
    async fn ready_handler_reports_ready_true() {
        let state = sample_state();
        let Json(response) = ready_handler(State(state)).await;
        assert!(response.ready);
        assert!(response.storage_ok);
    }
}
