// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Axum HTTP endpoints live here, including the readiness probe consumed by downstream systems.

use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

use crate::config::AppConfig;
use crate::domain::node::KnowledgeNode;
use crate::repository::UpsertOutcome;
use crate::scedge::{ScedgeError, ScedgeStatus};
use crate::state::{AppContext, DashboardOverview, HistoryEvent};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

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
    ctx: AppContext,
}

pub async fn serve(cfg: AppConfig, ctx: AppContext) -> Result<()> {
    let addr: SocketAddr = cfg.http_addr;
    let state = HttpState {
        cfg: cfg.clone(),
        ctx,
    };

    let api_router = Router::new()
        .route("/overview", get(api_overview))
        .route("/history", get(api_history))
        .route("/history/clear", post(api_history_clear))
        .route("/operations/store", post(api_store))
        .route("/operations/lookup", post(api_lookup))
        .route("/operations/purge", post(api_purge))
        .route("/scedge/status", get(api_scedge_status))
        .route("/scedge/lookup", get(api_scedge_lookup))
        .route("/scedge/store", post(api_scedge_store))
        .route("/scedge/purge", post(api_scedge_purge));

    let spa_service = ServeDir::new("dashboard/dist")
        .not_found_service(ServeFile::new("dashboard/dist/index.html"));

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .nest("/api", api_router)
        .nest_service("/dashboard", spa_service)
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
    let HttpState { cfg, ctx } = state;
    let storage_ok = match ctx.repos.nodes.health_check().await {
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

#[derive(Debug, Deserialize)]
struct StoreRequest {
    tenant_id: Option<Uuid>,
    node_id: Option<Uuid>,
    kind: String,
    payload: Value,
}

#[derive(Debug, Serialize)]
struct StoreResponse {
    node_id: Uuid,
    created: bool,
}

#[derive(Debug, Deserialize)]
struct LookupRequest {
    tenant_id: Option<Uuid>,
    node_id: Uuid,
}

#[derive(Debug, Serialize)]
struct LookupResponse {
    found: bool,
    node: Option<crate::domain::node::KnowledgeNode>,
}

#[derive(Debug, Deserialize)]
struct PurgeRequest {
    tenant_id: Option<Uuid>,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ScedgeLookupQuery {
    key: String,
    tenant: Option<String>,
}

async fn api_overview(State(state): State<HttpState>) -> Json<DashboardOverview> {
    Json(state.ctx.dashboard.overview())
}

async fn api_history(State(state): State<HttpState>) -> Json<Vec<HistoryEvent>> {
    Json(state.ctx.dashboard.history())
}

async fn api_history_clear(State(state): State<HttpState>) -> Json<ApiMessage> {
    state.ctx.dashboard.clear_history();
    Json(ApiMessage {
        message: "history cleared".into(),
    })
}

async fn api_store(
    State(state): State<HttpState>,
    Json(req): Json<StoreRequest>,
) -> Json<StoreResponse> {
    let tenant = req.tenant_id.unwrap_or(state.cfg.default_tenant_id);
    let mut node = KnowledgeNode::new(tenant, req.kind, req.payload);
    if let Some(id) = req.node_id {
        node.id = id;
    }

    let outcome = state
        .ctx
        .repos
        .nodes
        .upsert(tenant, node.clone())
        .await
        .expect("node upsert via http");

    state.ctx.dashboard.record_store(
        tenant,
        &node.kind,
        node.id,
        matches!(outcome, UpsertOutcome::Created),
    );

    Json(StoreResponse {
        node_id: node.id,
        created: matches!(outcome, UpsertOutcome::Created),
    })
}

async fn api_lookup(
    State(state): State<HttpState>,
    Json(req): Json<LookupRequest>,
) -> Json<LookupResponse> {
    let tenant = req.tenant_id.unwrap_or(state.cfg.default_tenant_id);
    let result = state.ctx.repos.nodes.get(tenant, req.node_id).await;

    let (found, node) = match result {
        Ok(Some(node)) => {
            state.ctx.dashboard.record_lookup(tenant, req.node_id, true);
            (true, Some(node))
        }
        Ok(None) => {
            state
                .ctx
                .dashboard
                .record_lookup(tenant, req.node_id, false);
            (false, None)
        }
        Err(err) => {
            tracing::error!(?err, "lookup failed");
            state
                .ctx
                .dashboard
                .record_lookup(tenant, req.node_id, false);
            (false, None)
        }
    };

    Json(LookupResponse { found, node })
}

async fn api_purge(
    State(state): State<HttpState>,
    Json(req): Json<PurgeRequest>,
) -> Json<ApiMessage> {
    let tenant = req.tenant_id.unwrap_or(state.cfg.default_tenant_id);
    state.ctx.dashboard.record_purge(
        tenant,
        json!({
            "reason": req.reason,
        }),
    );

    Json(ApiMessage {
        message: "purge acknowledged".into(),
    })
}

async fn api_scedge_status(State(state): State<HttpState>) -> Json<ScedgeStatus> {
    Json(state.ctx.scedge.status().await)
}

async fn api_scedge_lookup(
    State(state): State<HttpState>,
    Query(query): Query<ScedgeLookupQuery>,
) -> (StatusCode, Json<Value>) {
    match state.ctx.scedge.lookup(query.key, query.tenant).await {
        Ok((status, payload)) => (map_status(status), Json(payload)),
        Err(err) => scedge_error_response(err),
    }
}

async fn api_scedge_store(
    State(state): State<HttpState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    match state.ctx.scedge.store(body).await {
        Ok((status, payload)) => (map_status(status), Json(payload)),
        Err(err) => scedge_error_response(err),
    }
}

async fn api_scedge_purge(
    State(state): State<HttpState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    match state.ctx.scedge.purge(body).await {
        Ok((status, payload)) => (map_status(status), Json(payload)),
        Err(err) => scedge_error_response(err),
    }
}

fn map_status(status: reqwest::StatusCode) -> StatusCode {
    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY)
}

fn scedge_error_response(err: ScedgeError) -> (StatusCode, Json<Value>) {
    match err {
        ScedgeError::Disabled => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "SCEDGE_BASE_URL not configured" })),
        ),
        ScedgeError::Http(source) => {
            tracing::error!(error = %source, "scedge proxy error");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": source.to_string() })),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::repository::in_memory::{
        InMemoryBus, InMemoryCache, InMemoryEdgeRepository, InMemoryEmbeddingRepository,
        InMemoryNodeRepository, InMemoryOutboxRepository,
    };
    use crate::repository::RepositoryBundle;
    use crate::state::AppContext;
    use crate::state::DashboardHandle;

    fn sample_config() -> AppConfig {
        AppConfig {
            http_addr: "127.0.0.1:0".parse().unwrap(),
            grpc_addr: "127.0.0.1:0".parse().unwrap(),
            service_name: "synagraph".into(),
            version: "0.1.0-test".into(),
            database_url: None,
            default_tenant_id: Uuid::new_v4(),
            scedge_base_url: None,
        }
    }

    fn sample_state() -> HttpState {
        let cfg = sample_config();
        let repos = RepositoryBundle::new(
            Arc::new(InMemoryNodeRepository::new()),
            Arc::new(InMemoryEdgeRepository::new()),
            Arc::new(InMemoryEmbeddingRepository::new()),
            Arc::new(InMemoryOutboxRepository::new()),
            Arc::new(InMemoryCache::default()),
            Arc::new(InMemoryBus::default()),
        );
        let dashboard = crate::state::DashboardHandle::new();
        let scedge = crate::scedge::ScedgeBridge::new(None);
        let ctx = AppContext::new(repos, dashboard, scedge);
        HttpState { cfg, ctx }
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
