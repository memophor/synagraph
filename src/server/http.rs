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
use crate::domain::capsule::{CapsuleIngestRequest, CapsuleLookupResponse};
use crate::domain::node::KnowledgeNode;
use crate::repository::UpsertOutcome;
use crate::scedge::{ScedgeError, ScedgeStatus};
use crate::state::{AppContext, DashboardOverview, HistoryEvent};
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
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
        .route("/lookup", get(api_capsule_lookup))
        .route("/ingest/capsule", post(api_capsule_store))
        .route("/capsules/purge", post(api_capsule_purge))
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

#[derive(Debug, Deserialize)]
struct CapsuleLookupQuery {
    key: String,
    tenant: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CapsuleStoreBody {
    #[serde(default)]
    tenant: Option<String>,
    #[serde(flatten)]
    capsule: CapsuleIngestRequest,
}

#[derive(Debug, Deserialize)]
struct CapsulePurgeBody {
    #[serde(default)]
    tenant: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    keys: Option<Vec<String>>,
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

async fn api_capsule_lookup(
    State(state): State<HttpState>,
    Query(query): Query<CapsuleLookupQuery>,
) -> Result<Json<CapsuleLookupResponse>, (StatusCode, Json<Value>)> {
    let tenant_id = resolve_tenant(&state.cfg, query.tenant.as_deref());
    let node = state
        .ctx
        .repos
        .nodes
        .get_by_key(tenant_id, &query.key)
        .await
        .map_err(internal_error)?
        .ok_or_else(cache_miss)?;

    let capsule = CapsuleLookupResponse::from_node(&node).map_err(internal_error)?;

    if let Some(expected) = &query.tenant {
        if capsule.artifact.policy.tenant != *expected {
            return Err(cache_miss());
        }
    }

    Ok(Json(capsule))
}

async fn api_capsule_store(
    State(state): State<HttpState>,
    Json(body): Json<CapsuleStoreBody>,
) -> (StatusCode, Json<Value>) {
    let CapsuleStoreBody { tenant, capsule } = body;

    if let Some(expected) = tenant.as_ref() {
        if capsule.artifact.policy.tenant != *expected {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "policy.tenant mismatch" })),
            );
        }
    }

    if capsule.artifact.policy.tenant.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "artifact.policy.tenant is required" })),
        );
    }

    let tenant_id = resolve_tenant(&state.cfg, tenant.as_deref());
    let existing_node = match state
        .ctx
        .repos
        .nodes
        .get_by_key(tenant_id, &capsule.key)
        .await
    {
        Ok(node) => node,
        Err(err) => return internal_error(err),
    };
    let response_capsule = capsule.clone();
    let node = match capsule.into_node(tenant_id) {
        Ok(node) => node,
        Err(err) => return internal_error(err),
    };

    let existing_capsule = existing_node
        .as_ref()
        .and_then(|node| CapsuleLookupResponse::from_node(node).ok());

    match state.ctx.repos.nodes.upsert(tenant_id, node).await {
        Ok(outcome) => {
            let status = match outcome {
                UpsertOutcome::Created => "created",
                UpsertOutcome::Updated => "updated",
            };
            if state.cfg.scedge_event_bus_enabled {
                let tenant_slug = response_capsule.artifact.policy.tenant.clone();
                let subject = state.cfg.scedge_event_bus_subject.clone();
                let new_hash = response_capsule.artifact.hash.clone();
                let event = if let (UpsertOutcome::Updated, Some(old_capsule)) =
                    (outcome, existing_capsule)
                {
                    json!({
                        "type": "SUPERSEDED_BY",
                        "tenant": tenant_slug,
                        "old_hash": old_capsule.artifact.hash,
                        "new_hash": new_hash,
                    })
                } else {
                    json!({
                        "type": "UPSERT_NODE",
                        "tenant": tenant_slug,
                        "key": response_capsule.key,
                        "hash": new_hash,
                    })
                };
                publish_graph_event(&state, &subject, event).await;
            }
            (
                StatusCode::OK,
                Json(json!({
                    "status": status,
                    "key": response_capsule.key,
                    "hash": response_capsule.artifact.hash,
                    "tenant": response_capsule.artifact.policy.tenant
                })),
            )
        }
        Err(err) => internal_error(err),
    }
}

async fn api_capsule_purge(
    State(state): State<HttpState>,
    Json(body): Json<CapsulePurgeBody>,
) -> (StatusCode, Json<Value>) {
    let tenant_id = resolve_tenant(&state.cfg, body.tenant.as_deref());
    let mut purged = 0_u32;
    let mut revoked: Vec<String> = Vec::new();

    let mut keys: Vec<String> = Vec::new();
    if let Some(key) = body.key {
        keys.push(key);
    }
    if let Some(list) = body.keys {
        keys.extend(list.into_iter().filter(|k| !k.is_empty()));
    }

    for key in keys {
        match state.ctx.repos.nodes.delete_by_key(tenant_id, &key).await {
            Ok(Some(node)) => {
                purged += 1;
                if state.cfg.scedge_event_bus_enabled {
                    if let Ok(capsule) = CapsuleLookupResponse::from_node(&node) {
                        let tenant_slug = capsule.artifact.policy.tenant.clone();
                        let hash = capsule.artifact.hash.clone();
                        revoked.push(hash.clone());
                        let event = json!({
                            "type": "REVOKE_CAPSULE",
                            "tenant": tenant_slug,
                            "capsule_id": capsule.key,
                            "hash": hash,
                        });
                        let subject = state.cfg.scedge_event_bus_subject.clone();
                        publish_graph_event(&state, &subject, event).await;
                    }
                }
            }
            Ok(None) => {}
            Err(err) => return internal_error(err),
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "purged": purged,
            "revoked_hashes": revoked,
        })),
    )
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

fn cache_miss() -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "cache miss" })),
    )
}

fn internal_error<E: std::fmt::Display>(err: E) -> (StatusCode, Json<Value>) {
    tracing::error!(error = %err, "capsule handler error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": err.to_string() })),
    )
}

fn resolve_tenant(cfg: &AppConfig, slug: Option<&str>) -> Uuid {
    if let Some(slug) = slug {
        if let Some(uuid) = cfg.tenant_slugs.get(slug) {
            return *uuid;
        }
    }
    cfg.default_tenant_id
}

async fn publish_graph_event(state: &HttpState, subject: &str, payload: Value) {
    if let Err(err) = state.ctx.repos.bus.publish(subject, &payload).await {
        tracing::error!(error = %err, "failed to publish scedge graph event");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use std::collections::HashMap;
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::domain::capsule::{CapsuleArtifact, CapsuleIngestRequest, CapsulePolicy};
    use crate::repository::in_memory::{
        InMemoryBus, InMemoryCache, InMemoryEdgeRepository, InMemoryEmbeddingRepository,
        InMemoryNodeRepository, InMemoryOutboxRepository,
    };
    use crate::repository::RepositoryBundle;
    use crate::state::{AppContext, DashboardHandle};
    use serde_json::json;

    fn sample_config() -> AppConfig {
        AppConfig {
            http_addr: "127.0.0.1:0".parse().unwrap(),
            grpc_addr: "127.0.0.1:0".parse().unwrap(),
            service_name: "synagraph".into(),
            version: "0.1.0-test".into(),
            database_url: None,
            default_tenant_id: Uuid::new_v4(),
            scedge_base_url: None,
            scedge_event_bus_enabled: false,
            scedge_event_bus_subject: "scedge:events".into(),
            tenant_slugs: HashMap::new(),
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
        let dashboard = DashboardHandle::new();
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

    #[tokio::test]
    async fn capsule_store_persists_payload() {
        let state = sample_state();
        let repos = state.ctx.repos.clone();
        let tenant = state.cfg.default_tenant_id;

        let payload = serde_json::from_value::<CapsuleStoreBody>(json!({
            "tenant": "acme",
            "key": "acme:analytics:report",
            "artifact": {
                "answer": "Quarterly revenue was up 23%.",
                "policy": {
                    "tenant": "acme",
                    "phi": false,
                    "pii": false,
                    "region": null,
                    "compliance_tags": []
                },
                "provenance": [
                    {"source": "synagraph:artifact", "hash": "sg-123", "version": "v1"}
                ],
                "hash": "sg-123",
                "ttl_seconds": 3600
            }
        }))
        .unwrap();

        let (status, Json(resp)) = api_capsule_store(State(state), Json(payload)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["status"], "created");

        let stored = repos
            .nodes
            .get_by_key(tenant, "acme:analytics:report")
            .await
            .unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn capsule_lookup_hits_cache() {
        let state = sample_state();
        let tenant = state.cfg.default_tenant_id;
        let repos = state.ctx.repos.clone();

        let capsule = CapsuleIngestRequest {
            key: "acme:analytics:report".into(),
            artifact: CapsuleArtifact {
                answer: json!("Quarterly revenue was up 23%."),
                policy: CapsulePolicy {
                    tenant: "acme".into(),
                    phi: false,
                    pii: false,
                    region: None,
                    compliance_tags: vec![],
                },
                provenance: vec![serde_json::from_value(json!({
                    "source": "synagraph:artifact",
                    "hash": "sg-123",
                    "version": "v1"
                }))
                .unwrap()],
                metrics: None,
                ttl_seconds: Some(3600),
                hash: "sg-123".into(),
                metadata: None,
            },
            expires_at: None,
        };

        let node = capsule.clone().into_node(tenant).unwrap();
        repos.nodes.upsert(tenant, node).await.unwrap();

        let query = CapsuleLookupQuery {
            key: "acme:analytics:report".into(),
            tenant: Some("acme".into()),
        };

        let Json(response) = api_capsule_lookup(State(state), Query(query))
            .await
            .unwrap();

        assert_eq!(response.key, "acme:analytics:report");
        assert_eq!(response.artifact.hash, "sg-123");
        assert_eq!(response.artifact.policy.tenant, "acme");
        assert!(response.ttl_remaining_seconds.is_some());
    }

    #[tokio::test]
    async fn capsule_lookup_miss_returns_404() {
        let state = sample_state();
        let query = CapsuleLookupQuery {
            key: "missing".into(),
            tenant: None,
        };

        let err = api_capsule_lookup(State(state), Query(query))
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn capsule_purge_removes_records() {
        let state = sample_state();
        let repos = state.ctx.repos.clone();
        let tenant = state.cfg.default_tenant_id;

        let capsule = CapsuleIngestRequest {
            key: "acme:analytics:report".into(),
            artifact: CapsuleArtifact {
                answer: json!("Quarterly revenue was up 23%"),
                policy: CapsulePolicy {
                    tenant: "acme".into(),
                    phi: false,
                    pii: false,
                    region: None,
                    compliance_tags: vec![],
                },
                provenance: vec![serde_json::from_value(json!({
                    "source": "synagraph:artifact",
                    "hash": "sg-123"
                }))
                .unwrap()],
                metrics: None,
                ttl_seconds: Some(3600),
                hash: "sg-123".into(),
                metadata: None,
            },
            expires_at: None,
        };

        let node = capsule.clone().into_node(tenant).unwrap();
        repos.nodes.upsert(tenant, node).await.unwrap();

        let payload = CapsulePurgeBody {
            tenant: Some("acme".into()),
            key: Some("acme:analytics:report".into()),
            keys: None,
        };

        let (status, Json(resp)) = api_capsule_purge(State(state), Json(payload)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["purged"], 1);

        let remaining = repos
            .nodes
            .get_by_key(tenant, "acme:analytics:report")
            .await
            .unwrap();
        assert!(remaining.is_none());
    }
}
