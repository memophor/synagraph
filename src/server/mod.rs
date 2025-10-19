// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// This module orchestrates the HTTP and gRPC servers that expose the graph engine.

mod grpc;
mod http;

use anyhow::Result;
use tokio::try_join;

use crate::config::AppConfig;

pub async fn run(cfg: AppConfig) -> Result<()> {
    let http_future = http::serve(cfg.clone());
    let grpc_future = grpc::serve(cfg.clone());

    try_join!(http_future, grpc_future)?;

    Ok(())
}
