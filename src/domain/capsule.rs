use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::domain::node::KnowledgeNode;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleProvenance {
    pub source: String,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub generated_at: Option<DateTime<Utc>>,
}

impl Default for CapsuleProvenance {
    fn default() -> Self {
        Self {
            source: String::new(),
            hash: String::new(),
            version: None,
            generated_at: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsulePolicy {
    pub tenant: String,
    #[serde(default)]
    pub phi: bool,
    #[serde(default)]
    pub pii: bool,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub compliance_tags: Vec<String>,
}

impl Default for CapsulePolicy {
    fn default() -> Self {
        Self {
            tenant: String::new(),
            phi: false,
            pii: false,
            region: None,
            compliance_tags: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleArtifact {
    #[serde(default)]
    pub answer: Value,
    #[serde(default)]
    pub policy: CapsulePolicy,
    #[serde(default)]
    pub provenance: Vec<CapsuleProvenance>,
    #[serde(default)]
    pub metrics: Option<Value>,
    #[serde(default)]
    pub ttl_seconds: Option<i64>,
    pub hash: String,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleLookupResponse {
    pub key: String,
    pub artifact: CapsuleArtifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_remaining_seconds: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleIngestRequest {
    pub key: String,
    pub artifact: CapsuleArtifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

impl CapsuleArtifact {
    pub fn ensure_defaults(&mut self) {
        if self.provenance.is_empty() {
            self.provenance.push(CapsuleProvenance::default());
        }
    }
}

impl CapsuleLookupResponse {
    pub fn from_node(node: &KnowledgeNode) -> Result<Self> {
        let mut capsule: CapsuleIngestRequest =
            serde_json::from_value(node.payload_json.clone())
                .context("knowledge node payload is not a capsule")?;

        capsule.artifact.ensure_defaults();
        if capsule.artifact.policy.tenant.is_empty() {
            capsule.artifact.policy.tenant = "default".to_string();
        }
        if capsule.artifact.hash.is_empty() {
            capsule.artifact.hash = node.id.to_string();
        }

        // Derive base TTL if only expires_at is present.
        if capsule.artifact.ttl_seconds.is_none() {
            if let Some(exp) = capsule.expires_at {
                let ttl = (exp - node.updated_at).num_seconds().max(0);
                capsule.artifact.ttl_seconds = Some(ttl);
            }
        }

        let expires_at = capsule.expires_at.or_else(|| {
            capsule
                .artifact
                .ttl_seconds
                .map(|ttl| node.updated_at + Duration::seconds(ttl))
        });

        // Ensure both expires_at and ttl_seconds are populated together.
        if capsule.artifact.ttl_seconds.is_none() {
            if let Some(exp) = expires_at {
                let ttl = (exp - node.updated_at).num_seconds().max(0);
                capsule.artifact.ttl_seconds = Some(ttl);
            }
        }

        let ttl_remaining_seconds = expires_at.map(|ts| {
            let diff = ts - Utc::now();
            diff.num_seconds().max(0)
        });

        Ok(CapsuleLookupResponse {
            key: capsule.key,
            artifact: capsule.artifact,
            expires_at,
            ttl_remaining_seconds,
        })
    }
}

impl CapsuleIngestRequest {
    pub fn into_node(self, tenant_id: Uuid) -> Result<KnowledgeNode> {
        let mut capsule = self;
        capsule.artifact.ensure_defaults();
        if capsule.artifact.policy.tenant.is_empty() {
            return Err(anyhow!("capsule policy.tenant is required"));
        }
        if capsule.artifact.hash.is_empty() {
            return Err(anyhow!("capsule artifact.hash is required"));
        }

        let mut node = KnowledgeNode::new(tenant_id, "capsule", serde_json::to_value(&capsule)?);
        let stable_id = Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!("{}:{}", capsule.key, capsule.artifact.hash).as_bytes(),
        );
        node.id = stable_id;
        node.policy = Some(json!(capsule.artifact.policy));
        node.provenance = Some(json!(capsule.artifact.provenance));
        Ok(node)
    }
}
