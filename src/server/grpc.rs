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

pub async fn serve(cfg: AppConfig) -> Result<()> {
    let addr: SocketAddr = cfg.grpc_addr;
    let svc = GraphServiceImpl::new(cfg.clone());

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
}

impl GraphServiceImpl {
    fn new(cfg: AppConfig) -> Self {
        Self {
            service_name: cfg.service_name,
            version: cfg.version,
        }
    }
}

#[tonic::async_trait]
impl GraphService for GraphServiceImpl {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        let message = request.into_inner().message;
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

        let _node = KnowledgeNode {
            id: node_id,
            kind: payload.kind,
            payload: json_payload,
            created,
        };

        let response = UpsertNodeResponse {
            node_id: node_id.to_string(),
            created,
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
    use super::parse_payload;

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
}
