// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// This binary sets up configuration, telemetry, and launches the public HTTP and gRPC servers.

use std::sync::Arc;

use anyhow::Result;
use synagraph::config::AppConfig;
use synagraph::repository::postgres::PostgresNodeRepository;
use synagraph::repository::{self, NodeRepositoryHandle};
use synagraph::{server, telemetry};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    telemetry::init();

    let cfg = AppConfig::from_env()?;

    let node_repo: NodeRepositoryHandle = match cfg.database_url.clone() {
        Some(url) => {
            tracing::info!("initializing postgres repository");
            let repo = PostgresNodeRepository::connect(&url).await?;
            Arc::new(repo)
        }
        None => {
            tracing::info!("initializing in-memory repository");
            Arc::new(repository::in_memory::InMemoryNodeRepository::new())
        }
    };

    tracing::info!(service = %cfg.service_name, version = %cfg.version, "starting synagraph");

    server::run(cfg, node_repo).await
}
