// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// This binary sets up configuration, telemetry, and launches the public HTTP and gRPC servers.

use std::sync::Arc;

use anyhow::Result;
use synagraph::config::AppConfig;
use synagraph::repository::in_memory::{
    InMemoryBus, InMemoryCache, InMemoryEdgeRepository, InMemoryEmbeddingRepository,
    InMemoryNodeRepository, InMemoryOutboxRepository,
};
use synagraph::repository::postgres::{
    PostgresEdgeRepository, PostgresEmbeddingRepository, PostgresNodeRepository,
    PostgresOutboxRepository,
};
use synagraph::repository::RepositoryBundle;
use synagraph::scedge::ScedgeBridge;
use synagraph::state::{AppContext, DashboardHandle};
use synagraph::{server, telemetry};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    telemetry::init();

    let cfg = AppConfig::from_env()?;

    let repos = match cfg.database_url.clone() {
        Some(url) => {
            tracing::info!("initializing postgres repositories");
            let node_repo = PostgresNodeRepository::connect(&url).await?;
            let pool = node_repo.pool();
            RepositoryBundle::new(
                Arc::new(node_repo),
                Arc::new(PostgresEdgeRepository::new(pool.clone())),
                Arc::new(PostgresEmbeddingRepository::new(pool.clone())),
                Arc::new(PostgresOutboxRepository::new(pool)),
                Arc::new(InMemoryCache::default()),
                Arc::new(InMemoryBus::default()),
            )
        }
        None => {
            tracing::info!("initializing in-memory repositories");
            RepositoryBundle::new(
                Arc::new(InMemoryNodeRepository::new()),
                Arc::new(InMemoryEdgeRepository::new()),
                Arc::new(InMemoryEmbeddingRepository::new()),
                Arc::new(InMemoryOutboxRepository::new()),
                Arc::new(InMemoryCache::default()),
                Arc::new(InMemoryBus::default()),
            )
        }
    };

    let dashboard = DashboardHandle::new();
    let scedge = ScedgeBridge::new(cfg.scedge_base_url.clone());
    let ctx = AppContext::new(repos, dashboard, scedge);

    tracing::info!(service = %cfg.service_name, version = %cfg.version, "starting synagraph");

    server::run(cfg, ctx).await
}
