// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// This binary sets up configuration, telemetry, and launches the public HTTP and gRPC servers.

mod config;
mod domain;
mod pb;
mod server;
mod telemetry;

use anyhow::Result;
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    telemetry::init();

    let cfg = AppConfig::from_env()?;

    tracing::info!(service = %cfg.service_name, version = %cfg.version, "starting synagraph");

    server::run(cfg).await
}
