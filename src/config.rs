use std::env;
use std::net::SocketAddr;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub http_addr: SocketAddr,
    pub grpc_addr: SocketAddr,
    pub service_name: String,
    pub version: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let http_addr: SocketAddr = env::var("HTTP_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse()
            .context("invalid HTTP_ADDR")?;

        let grpc_addr: SocketAddr = env::var("GRPC_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
            .parse()
            .context("invalid GRPC_ADDR")?;

        let service_name = env::var("SERVICE_NAME").unwrap_or_else(|_| "synagraph".into());
        let version = env::var("SERVICE_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").into());

        Ok(Self {
            http_addr,
            grpc_addr,
            service_name,
            version,
        })
    }
}
