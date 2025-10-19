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
use crate::repository::NodeRepositoryHandle;

pub async fn serve(cfg: AppConfig, node_repo: NodeRepositoryHandle) -> Result<()> {
    let addr: SocketAddr = cfg.grpc_addr;
    let svc = GraphServiceImpl::new(cfg.clone(), node_repo);

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
    node_repo: NodeRepositoryHandle,
}

impl GraphServiceImpl {
    fn new(cfg: AppConfig, node_repo: NodeRepositoryHandle) -> Self {
        Self {
            service_name: cfg.service_name,
            version: cfg.version,
            node_repo,
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

        let (node_id, created) = if payload.node_id.is_empty() {
            (Uuid::new_v4(), true)
        } else {
            (
                Uuid::parse_str(&payload.node_id)
                    .map_err(|_| Status::invalid_argument("node_id must be a UUID"))?,
                false,
            )
        };

        let node = KnowledgeNode {
            id: node_id,
            kind: payload.kind,
            payload: json_payload,
            created,
        };

        let outcome = self.node_repo.upsert(node).await.map_err(|err| {
            tracing::error!(?err, "node upsert failed");
            Status::internal("failed to persist node")
        })?;

        let response = UpsertNodeResponse {
            node_id: outcome.node.id.to_string(),
            created: outcome.created,
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
    use crate::repository::in_memory::InMemoryNodeRepository;
    use crate::repository::NodeRepository;
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
        let cfg = AppConfig {
            http_addr: "127.0.0.1:0".parse().unwrap(),
            grpc_addr: "127.0.0.1:0".parse().unwrap(),
            service_name: "synagraph".into(),
            version: "0.1.0-test".into(),
        };

        let repo = Arc::new(InMemoryNodeRepository::new());
        let service = GraphServiceImpl::new(cfg, repo.clone());

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

        let stored = repo.get(node_id).await.expect("get succeeds");
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
        let stored_updated = repo
            .get(node_id)
            .await
            .expect("get succeeds")
            .expect("node exists");
        assert_eq!(stored_updated.payload["title"], "updated");
    }
}
