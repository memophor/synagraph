// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// This gRPC service exposes the platform contract and will evolve with persistence and policy logic.

use std::net::SocketAddr;

use anyhow::{Context, Result};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::domain::node::KnowledgeNode;
use crate::pb::synagraph::v1::graph_service_server::{GraphService, GraphServiceServer};
use crate::pb::synagraph::v1::{PingRequest, PingResponse, UpsertNodeRequest, UpsertNodeResponse};
use crate::repository::UpsertOutcome;
use crate::state::AppContext;

pub async fn serve(cfg: AppConfig, ctx: AppContext) -> Result<()> {
    let addr: SocketAddr = cfg.grpc_addr;
    let svc = GraphServiceImpl::new(cfg.clone(), ctx);

    tracing::info!(%addr, "grpc server listening");

    tonic::transport::Server::builder()
        .add_service(GraphServiceServer::new(svc))
        .serve(addr)
        .await
        .context("grpc server error")
}

#[derive(Clone)]
struct GraphServiceImpl {
    service_name: String,
    version: String,
    ctx: AppContext,
    default_tenant: Uuid,
}

impl GraphServiceImpl {
    fn new(cfg: AppConfig, ctx: AppContext) -> Self {
        Self {
            service_name: cfg.service_name,
            version: cfg.version,
            ctx,
            default_tenant: cfg.default_tenant_id,
        }
    }
}

#[tonic::async_trait]
impl GraphService for GraphServiceImpl {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        let message = request.into_inner().message;
        tracing::debug!(service = %self.service_name, "received ping request");
        let reply = PingResponse {
            message: if message.is_empty() {
                "pong".to_string()
            } else {
                format!("pong: {}", message)
            },
            version: self.version.clone(),
        };

        Ok(Response::new(reply))
    }

    async fn upsert_node(
        &self,
        request: Request<UpsertNodeRequest>,
    ) -> Result<Response<UpsertNodeResponse>, Status> {
        let payload = request.into_inner();
        tracing::debug!(service = %self.service_name, kind = %payload.kind, "processing upsert_node");
        let json_payload = parse_payload(&payload.payload_json)?;

        let tenant_id = self.default_tenant;
        let mut node = KnowledgeNode::new(tenant_id, payload.kind, json_payload);
        let node_id = if payload.node_id.is_empty() {
            node.id
        } else {
            Uuid::parse_str(&payload.node_id)
                .map_err(|_| Status::invalid_argument("node_id must be a UUID"))?
        };
        node.id = node_id;

        let outcome = self
            .ctx
            .repos
            .nodes
            .upsert(tenant_id, node.clone())
            .await
            .map_err(|err| {
                tracing::error!(?err, "node upsert failed");
                Status::internal("failed to persist node")
            })?;

        self.ctx.dashboard.record_store(
            tenant_id,
            &node.kind,
            node.id,
            matches!(outcome, UpsertOutcome::Created),
        );

        let response = UpsertNodeResponse {
            node_id: node_id.to_string(),
            created: matches!(outcome, UpsertOutcome::Created),
        };

        Ok(Response::new(response))
    }
}

fn parse_payload(raw: &str) -> Result<serde_json::Value, Status> {
    if raw.trim().is_empty() {
        return Ok(serde_json::Value::Null);
    }

    serde_json::from_str(raw)
        .map_err(|err| Status::invalid_argument(format!("payload_json is not valid JSON: {}", err)))
}

#[cfg(test)]
mod tests {
    use super::{parse_payload, GraphServiceImpl};
    use crate::config::AppConfig;
    use crate::pb::synagraph::v1::graph_service_server::GraphService;
    use crate::pb::synagraph::v1::UpsertNodeRequest;
    use crate::repository::in_memory::{
        InMemoryBus, InMemoryCache, InMemoryEdgeRepository, InMemoryEmbeddingRepository,
        InMemoryNodeRepository, InMemoryOutboxRepository,
    };
    use crate::repository::RepositoryBundle;
    use crate::state::{AppContext, DashboardHandle};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tonic::Request;
    use uuid::Uuid;

    #[test]
    fn parses_valid_json() {
        let payload = parse_payload("{\"foo\":1}").expect("valid json");
        assert_eq!(payload["foo"], 1);
    }

    #[test]
    fn empty_payload_defaults_to_null() {
        let payload = parse_payload("   ").expect("empty json");
        assert!(payload.is_null());
    }

    #[test]
    fn invalid_json_errors() {
        let err = parse_payload("not-json").unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn upsert_node_persists_and_updates_records() {
        let tenant = Uuid::new_v4();
        let cfg = AppConfig {
            http_addr: "127.0.0.1:0".parse().unwrap(),
            grpc_addr: "127.0.0.1:0".parse().unwrap(),
            service_name: "synagraph".into(),
            version: "0.1.0-test".into(),
            database_url: None,
            default_tenant_id: tenant,
            scedge_base_url: None,
            scedge_event_bus_enabled: false,
            scedge_event_bus_subject: "scedge:events".into(),
            tenant_slugs: HashMap::new(),
        };

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
        let ctx = AppContext::new(repos.clone(), dashboard, scedge);
        let service = GraphServiceImpl::new(cfg.clone(), ctx.clone());

        let response = service
            .upsert_node(Request::new(UpsertNodeRequest {
                node_id: String::new(),
                kind: "note".into(),
                payload_json: "{\"title\":\"first\"}".into(),
            }))
            .await
            .expect("upsert succeeds")
            .into_inner();

        assert!(response.created);
        let node_id = Uuid::parse_str(&response.node_id).expect("valid uuid");

        let stored = ctx
            .repos
            .nodes
            .get(tenant, node_id)
            .await
            .expect("get succeeds");
        assert!(stored.is_some());

        let response_update = service
            .upsert_node(Request::new(UpsertNodeRequest {
                node_id: response.node_id.clone(),
                kind: "note".into(),
                payload_json: "{\"title\":\"updated\"}".into(),
            }))
            .await
            .expect("upsert succeeds")
            .into_inner();

        assert!(!response_update.created);
        let stored_updated = ctx
            .repos
            .nodes
            .get(tenant, node_id)
            .await
            .expect("get succeeds")
            .expect("node exists");
        assert_eq!(stored_updated.payload_json["title"], "updated");
    }
}
