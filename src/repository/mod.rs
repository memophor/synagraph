// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Repository abstractions provide persistence interfaces decoupled from storage backends.

pub mod in_memory;
pub mod postgres;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::domain::node::KnowledgeNode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEdge {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub src: Uuid,
    pub dst: Uuid,
    pub rel: String,
    pub weight: f32,
    pub props: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeEmbedding {
    pub node_id: Uuid,
    pub tenant_id: Uuid,
    pub model: String,
    pub dim: i32,
    pub vec: Vec<f32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutboxKind {
    Upsert,
    SupersededBy,
    RevokeCapsule,
}

impl OutboxKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Upsert => "UPSERT",
            Self::SupersededBy => "SUPERSEDED_BY",
            Self::RevokeCapsule => "REVOKE_CAPSULE",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub id: i64,
    pub tenant_id: Uuid,
    pub kind: OutboxKind,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpsertOutcome {
    Created,
    Updated,
}

#[async_trait]
pub trait NodeRepository: Send + Sync {
    async fn upsert(&self, tenant: Uuid, node: KnowledgeNode) -> Result<UpsertOutcome>;

    async fn get(&self, tenant: Uuid, id: Uuid) -> Result<Option<KnowledgeNode>>;

    async fn get_by_key(&self, tenant: Uuid, key: &str) -> Result<Option<KnowledgeNode>>;

    async fn delete_by_key(&self, tenant: Uuid, key: &str) -> Result<Option<KnowledgeNode>>;

    async fn query_by_kind(
        &self,
        tenant: Uuid,
        kind: &str,
        limit: usize,
        cursor: Option<Uuid>,
    ) -> Result<Vec<KnowledgeNode>>;

    async fn search_similar(
        &self,
        tenant: Uuid,
        vector: &[f32],
        limit: usize,
    ) -> Result<Vec<KnowledgeNode>>;

    async fn health_check(&self) -> Result<()>;
}

#[async_trait]
pub trait EdgeRepository: Send + Sync {
    async fn link(
        &self,
        tenant: Uuid,
        src: Uuid,
        dst: Uuid,
        rel: &str,
        weight: f32,
        props: Option<Value>,
    ) -> Result<()>;

    async fn neighbors(
        &self,
        tenant: Uuid,
        id: Uuid,
        rel: Option<&str>,
        hops: u8,
        limit: usize,
    ) -> Result<Vec<KnowledgeNode>>;
}

#[async_trait]
pub trait EmbeddingRepository: Send + Sync {
    async fn upsert_embedding(&self, tenant: Uuid, embedding: NodeEmbedding) -> Result<()>;

    async fn get_embeddings(&self, tenant: Uuid, node_id: Uuid) -> Result<Vec<NodeEmbedding>>;
}

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    async fn enqueue(&self, tenant: Uuid, kind: OutboxKind, payload: Value) -> Result<i64>;

    async fn claim_batch(&self, size: usize) -> Result<Vec<OutboxEvent>>;

    async fn mark_published(&self, ids: &[i64]) -> Result<()>;
}

#[async_trait]
pub trait ArtifactCache: Send + Sync {
    async fn get(&self, tenant: Uuid, key: &str) -> Result<Option<Value>>;
    async fn set(&self, tenant: Uuid, key: &str, value: &Value, ttl_sec: u64) -> Result<()>;
    async fn purge(&self, tenant: Uuid, key: &str) -> Result<()>;
}

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, topic: &str, payload: &Value) -> Result<()>;
    async fn subscribe(&self, topic: &str) -> Result<BusSubscription>;
}

pub struct BusSubscription;

impl BusSubscription {
    pub async fn try_next(&mut self) -> Result<Option<Value>> {
        Ok(None)
    }
}

pub type NodeRepositoryHandle = Arc<dyn NodeRepository>;
pub type EdgeRepositoryHandle = Arc<dyn EdgeRepository>;
pub type EmbeddingRepositoryHandle = Arc<dyn EmbeddingRepository>;
pub type OutboxRepositoryHandle = Arc<dyn OutboxRepository>;
pub type ArtifactCacheHandle = Arc<dyn ArtifactCache>;
pub type EventBusHandle = Arc<dyn EventBus>;

#[derive(Clone)]
pub struct RepositoryBundle {
    pub nodes: NodeRepositoryHandle,
    pub edges: EdgeRepositoryHandle,
    pub embeddings: EmbeddingRepositoryHandle,
    pub outbox: OutboxRepositoryHandle,
    pub cache: ArtifactCacheHandle,
    pub bus: EventBusHandle,
}

impl RepositoryBundle {
    pub fn new(
        nodes: NodeRepositoryHandle,
        edges: EdgeRepositoryHandle,
        embeddings: EmbeddingRepositoryHandle,
        outbox: OutboxRepositoryHandle,
        cache: ArtifactCacheHandle,
        bus: EventBusHandle,
    ) -> Self {
        Self {
            nodes,
            edges,
            embeddings,
            outbox,
            cache,
            bus,
        }
    }
}
