// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// This binary sets up configuration, telemetry, and launches the public HTTP and gRPC servers.

mod config;
mod domain;
mod pb;
mod repository;
mod server;
mod telemetry;

use anyhow::Result;
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    telemetry::init();

    let cfg = AppConfig::from_env()?;

    let node_repo: repository::NodeRepositoryHandle =
        std::sync::Arc::new(repository::in_memory::InMemoryNodeRepository::new());

    tracing::info!(service = %cfg.service_name, version = %cfg.version, "starting synagraph");

    server::run(cfg, node_repo).await
}
