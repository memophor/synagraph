// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// This module centralizes environment-driven configuration for both HTTP and gRPC endpoints.

use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;

use anyhow::{Context, Result};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub http_addr: SocketAddr,
    pub grpc_addr: SocketAddr,
    pub service_name: String,
    pub version: String,
    pub database_url: Option<String>,
    pub default_tenant_id: Uuid,
    pub scedge_base_url: Option<String>,
    pub scedge_event_bus_enabled: bool,
    pub scedge_event_bus_subject: String,
    pub tenant_slugs: HashMap<String, Uuid>,
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
        let version =
            env::var("SERVICE_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").into());
        let database_url = env::var("DATABASE_URL").ok();
        let default_tenant_id = env::var("DEFAULT_TENANT_ID")
            .ok()
            .and_then(|value| Uuid::parse_str(&value).ok())
            .unwrap_or_else(Uuid::nil);
        let scedge_base_url = env::var("SCEDGE_BASE_URL").ok();
        let scedge_event_bus_enabled = env::var("SCEDGE_EVENT_BUS_ENABLED")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(false);
        let scedge_event_bus_subject =
            env::var("SCEDGE_EVENT_BUS_SUBJECT").unwrap_or_else(|_| "scedge:events".to_string());
        let tenant_slugs = parse_slug_map(env::var("TENANT_SLUGS").ok());

        Ok(Self {
            http_addr,
            grpc_addr,
            service_name,
            version,
            database_url,
            default_tenant_id,
            scedge_base_url,
            scedge_event_bus_enabled,
            scedge_event_bus_subject,
            tenant_slugs,
        })
    }
}

fn parse_slug_map(source: Option<String>) -> HashMap<String, Uuid> {
    let mut map = HashMap::new();
    let Some(raw) = source else {
        return map;
    };

    for entry in raw.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split('=');
        let slug = match parts.next() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        let uuid_str = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        if let Ok(uuid) = Uuid::parse_str(uuid_str) {
            map.insert(slug, uuid);
        }
    }

    map
}
