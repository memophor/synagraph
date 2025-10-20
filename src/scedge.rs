// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// This module proxies Scedge Core APIs so the dashboard can surface cache telemetry and controls.

use chrono::{DateTime, Utc};
use reqwest::StatusCode as HttpStatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const HEALTHZ_PATH: &str = "/healthz";
const METRICS_PATH: &str = "/metrics";
const LOOKUP_PATH: &str = "/lookup";
const STORE_PATH: &str = "/store";
const PURGE_PATH: &str = "/purge";

#[derive(Clone)]
pub struct ScedgeBridge {
    inner: Option<ScedgeClient>,
}

impl ScedgeBridge {
    pub fn new(base_url: Option<String>) -> Self {
        let inner = base_url.map(ScedgeClient::new);
        Self { inner }
    }

    pub fn is_configured(&self) -> bool {
        self.inner.is_some()
    }

    pub async fn status(&self) -> ScedgeStatus {
        let Some(client) = &self.inner else {
            return ScedgeStatus::disabled();
        };

        let mut status = ScedgeStatus::configured();

        match client.health().await {
            Ok(health) => {
                status.healthy = health.status.eq_ignore_ascii_case("healthy");
                status.health = Some(health);
            }
            Err(err) => {
                status.errors.push(format!("health probe failed: {}", err));
            }
        }

        match client.metrics().await {
            Ok(raw) => {
                let parsed = parse_prometheus_metrics(&raw);
                status.metrics = Some(parsed);
            }
            Err(err) => {
                status.errors.push(format!("metrics probe failed: {}", err));
            }
        }

        status
    }

    pub async fn lookup(
        &self,
        key: String,
        tenant: Option<String>,
    ) -> Result<(HttpStatusCode, Value), ScedgeError> {
        let client = self.inner.as_ref().ok_or(ScedgeError::Disabled)?;
        Ok(client.lookup(key, tenant).await?)
    }

    pub async fn store(&self, payload: Value) -> Result<(HttpStatusCode, Value), ScedgeError> {
        let client = self.inner.as_ref().ok_or(ScedgeError::Disabled)?;
        Ok(client.store(payload).await?)
    }

    pub async fn purge(&self, payload: Value) -> Result<(HttpStatusCode, Value), ScedgeError> {
        let client = self.inner.as_ref().ok_or(ScedgeError::Disabled)?;
        Ok(client.purge(payload).await?)
    }
}

#[derive(Clone)]
struct ScedgeClient {
    base_url: String,
    client: reqwest::Client,
}

impl ScedgeClient {
    fn new(base_url: String) -> Self {
        let base = base_url.trim_end_matches('/').to_owned();
        let client = reqwest::Client::builder()
            .user_agent("synagraph-dashboard/0.1")
            .build()
            .expect("reqwest client");
        Self {
            base_url: base,
            client,
        }
    }

    async fn health(&self) -> Result<ScedgeHealth, reqwest::Error> {
        self.client
            .get(format!("{}{}", self.base_url, HEALTHZ_PATH))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    async fn metrics(&self) -> Result<String, reqwest::Error> {
        self.client
            .get(format!("{}{}", self.base_url, METRICS_PATH))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await
    }

    async fn lookup(
        &self,
        key: String,
        tenant: Option<String>,
    ) -> Result<(HttpStatusCode, Value), reqwest::Error> {
        let mut req = self
            .client
            .get(format!("{}{}", self.base_url, LOOKUP_PATH))
            .query(&[("key", key.clone())]);
        if let Some(tenant) = tenant {
            req = req.query(&[("tenant", tenant)]);
        }
        let res = req.send().await?;
        let status = res.status();
        let body = decode_body(res).await?;
        Ok((status, body))
    }

    async fn store(&self, payload: Value) -> Result<(HttpStatusCode, Value), reqwest::Error> {
        let res = self
            .client
            .post(format!("{}{}", self.base_url, STORE_PATH))
            .json(&payload)
            .send()
            .await?;
        let status = res.status();
        let body = decode_body(res).await?;
        Ok((status, body))
    }

    async fn purge(&self, payload: Value) -> Result<(HttpStatusCode, Value), reqwest::Error> {
        let res = self
            .client
            .post(format!("{}{}", self.base_url, PURGE_PATH))
            .json(&payload)
            .send()
            .await?;
        let status = res.status();
        let body = decode_body(res).await?;
        Ok((status, body))
    }
}

async fn decode_body(res: reqwest::Response) -> Result<Value, reqwest::Error> {
    let text = res.text().await?;
    if text.is_empty() {
        return Ok(Value::Null);
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(json) => Ok(json),
        Err(_) => Ok(Value::String(text)),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScedgeHealth {
    pub status: String,
    pub service: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScedgeMetric {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScedgeStatus {
    pub configured: bool,
    pub healthy: bool,
    pub fetched_at: DateTime<Utc>,
    pub health: Option<ScedgeHealth>,
    pub metrics: Option<Vec<ScedgeMetric>>,
    pub errors: Vec<String>,
}

impl ScedgeStatus {
    fn disabled() -> Self {
        Self {
            configured: false,
            healthy: false,
            fetched_at: Utc::now(),
            health: None,
            metrics: None,
            errors: vec!["SCEDGE_BASE_URL not configured".into()],
        }
    }

    fn configured() -> Self {
        Self {
            configured: true,
            healthy: false,
            fetched_at: Utc::now(),
            health: None,
            metrics: None,
            errors: Vec::new(),
        }
    }
}

fn parse_prometheus_metrics(metrics: &str) -> Vec<ScedgeMetric> {
    metrics
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let value = parts.next()?;
            let value = value.parse::<f64>().ok()?;
            Some(ScedgeMetric {
                name: name.to_string(),
                value,
            })
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum ScedgeError {
    #[error("scedge bridge not configured")]
    Disabled,
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}
