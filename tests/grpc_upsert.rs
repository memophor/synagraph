// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Integration test: spins up the gRPC server and exercises UpsertNode through a tonic client.

use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::Duration;

use synagraph::config::AppConfig;
use synagraph::pb::synagraph::v1::graph_service_client::GraphServiceClient;
use synagraph::pb::synagraph::v1::UpsertNodeRequest;
use synagraph::repository::in_memory::{
    InMemoryBus, InMemoryCache, InMemoryEdgeRepository, InMemoryEmbeddingRepository,
    InMemoryNodeRepository, InMemoryOutboxRepository,
};
use synagraph::repository::RepositoryBundle;
use synagraph::scedge::ScedgeBridge;
use synagraph::server;
use synagraph::state::{AppContext, DashboardHandle};
use tokio::net::TcpStream;
use tokio::time::sleep;
use tonic::transport::Channel;
use uuid::Uuid;

async fn start_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind temp port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);

    let grpc_addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], port));
    let default_tenant = Uuid::new_v4();
    let cfg = AppConfig {
        http_addr: "127.0.0.1:0".parse().unwrap(),
        grpc_addr,
        service_name: "synagraph-test".into(),
        version: "0.1.0-test".into(),
        database_url: None,
        default_tenant_id: default_tenant,
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
    let scedge = ScedgeBridge::new(None);
    let ctx = AppContext::new(repos, dashboard, scedge);

    tokio::spawn(async move {
        server::run(cfg, ctx).await.expect("server exits cleanly");
    });

    // Poll until the server is ready to accept connections or timeout.
    for _ in 0..10 {
        if TcpStream::connect(grpc_addr).await.is_ok() {
            return grpc_addr;
        }
        sleep(Duration::from_millis(50)).await;
    }

    panic!("grpc server failed to start in time");
}

async fn connect(addr: SocketAddr) -> GraphServiceClient<Channel> {
    GraphServiceClient::connect(format!("http://{}", addr))
        .await
        .expect("client connects")
}

#[tokio::test]
async fn upsert_node_roundtrip_via_grpc() {
    let addr = start_server().await;
    let mut client = connect(addr).await;

    let response = client
        .upsert_node(UpsertNodeRequest {
            node_id: String::new(),
            kind: "note".into(),
            payload_json: "{\"title\":\"grpc-test\"}".into(),
        })
        .await
        .expect("upsert succeeds")
        .into_inner();

    assert!(response.created);
    let node_id = Uuid::parse_str(&response.node_id).expect("valid uuid");

    let response_update = client
        .upsert_node(UpsertNodeRequest {
            node_id: node_id.to_string(),
            kind: "note".into(),
            payload_json: "{\"title\":\"grpc-test-updated\"}".into(),
        })
        .await
        .expect("upsert succeeds")
        .into_inner();

    assert!(!response_update.created);
}
