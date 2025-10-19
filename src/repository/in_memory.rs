// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Simple in-memory repository used for early development and testing flows.

use std::collections::{HashMap, VecDeque};

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::domain::node::KnowledgeNode;

use super::{
    ArtifactCache, BusSubscription, EdgeRepository, EmbeddingRepository, EventBus, KnowledgeEdge,
    NodeEmbedding, NodeRepository, OutboxEvent, OutboxKind, OutboxRepository, UpsertOutcome,
};

#[derive(Default)]
pub struct InMemoryNodeRepository {
    inner: RwLock<HashMap<Uuid, HashMap<Uuid, KnowledgeNode>>>,
}

impl InMemoryNodeRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn tenant_map_mut<'a>(
        guard: &'a mut HashMap<Uuid, HashMap<Uuid, KnowledgeNode>>,
        tenant: Uuid,
    ) -> &'a mut HashMap<Uuid, KnowledgeNode> {
        guard.entry(tenant).or_insert_with(HashMap::new)
    }
}

#[async_trait]
impl NodeRepository for InMemoryNodeRepository {
    async fn upsert(&self, tenant: Uuid, mut node: KnowledgeNode) -> Result<UpsertOutcome> {
        let mut guard = self.inner.write().await;
        let tenant_map = Self::tenant_map_mut(&mut guard, tenant);

        node.tenant_id = tenant;
        let now = Utc::now();

        let outcome = if let Some(existing) = tenant_map.get(&node.id) {
            node.created_at = existing.created_at;
            node.updated_at = now;
            tenant_map.insert(node.id, node);
            UpsertOutcome::Updated
        } else {
            node.created_at = now;
            node.updated_at = now;
            tenant_map.insert(node.id, node);
            UpsertOutcome::Created
        };

        Ok(outcome)
    }

    async fn get(&self, tenant: Uuid, id: Uuid) -> Result<Option<KnowledgeNode>> {
        let guard = self.inner.read().await;
        Ok(guard.get(&tenant).and_then(|nodes| nodes.get(&id)).cloned())
    }

    async fn query_by_kind(
        &self,
        tenant: Uuid,
        kind: &str,
        limit: usize,
        cursor: Option<Uuid>,
    ) -> Result<Vec<KnowledgeNode>> {
        let guard = self.inner.read().await;
        let Some(nodes_map) = guard.get(&tenant) else {
            return Ok(Vec::new());
        };

        let mut nodes: Vec<KnowledgeNode> = nodes_map
            .values()
            .filter(|node| node.kind == kind)
            .cloned()
            .collect();

        nodes.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));

        if let Some(cursor_id) = cursor {
            if let Some(pos) = nodes.iter().position(|node| node.id == cursor_id) {
                nodes = nodes.into_iter().skip(pos + 1).collect();
            }
        }

        nodes.truncate(limit);
        Ok(nodes)
    }

    async fn search_similar(
        &self,
        tenant: Uuid,
        vector: &[f32],
        limit: usize,
    ) -> Result<Vec<KnowledgeNode>> {
        if vector.is_empty() {
            return Ok(Vec::new());
        }

        let guard = self.inner.read().await;
        let Some(nodes_map) = guard.get(&tenant) else {
            return Ok(Vec::new());
        };

        let mut scored: Vec<(f32, KnowledgeNode)> = nodes_map
            .values()
            .filter_map(|node| {
                node.vector.as_ref().map(|candidate| {
                    let score = candidate
                        .iter()
                        .zip(vector.iter())
                        .map(|(a, b)| a * b)
                        .sum();
                    (score, node.clone())
                })
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let results = scored
            .into_iter()
            .take(limit)
            .map(|(_, node)| node)
            .collect();
        Ok(results)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Default)]
pub struct InMemoryEdgeRepository {
    edges: RwLock<HashMap<Uuid, Vec<(Uuid, KnowledgeEdge)>>>,
}

impl InMemoryEdgeRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EdgeRepository for InMemoryEdgeRepository {
    async fn link(
        &self,
        tenant: Uuid,
        src: Uuid,
        dst: Uuid,
        rel: &str,
        weight: f32,
        props: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut guard = self.edges.write().await;
        let list = guard.entry(tenant).or_insert_with(Vec::new);
        list.push((
            src,
            KnowledgeEdge {
                id: Uuid::new_v4(),
                tenant_id: tenant,
                src,
                dst,
                rel: rel.to_string(),
                weight,
                props,
                created_at: Utc::now(),
            },
        ));
        Ok(())
    }

    async fn neighbors(
        &self,
        tenant: Uuid,
        id: Uuid,
        rel: Option<&str>,
        _hops: u8,
        limit: usize,
    ) -> Result<Vec<KnowledgeNode>> {
        let guard = self.edges.read().await;
        let Some(edges) = guard.get(&tenant) else {
            return Ok(Vec::new());
        };

        let nodes: Vec<KnowledgeNode> = edges
            .iter()
            .filter(|(src, edge)| *src == id && rel.map(|r| r == edge.rel).unwrap_or(true))
            .take(limit)
            .map(|(_, edge)| {
                KnowledgeNode::new(
                    tenant,
                    edge.rel.clone(),
                    serde_json::json!({ "target": edge.dst }),
                )
            })
            .collect();

        Ok(nodes)
    }
}

#[allow(dead_code)]
#[derive(Default)]
pub struct InMemoryEmbeddingRepository;

impl InMemoryEmbeddingRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EmbeddingRepository for InMemoryEmbeddingRepository {
    async fn upsert_embedding(&self, _tenant: Uuid, _embedding: NodeEmbedding) -> Result<()> {
        Ok(())
    }

    async fn get_embeddings(&self, _tenant: Uuid, _node_id: Uuid) -> Result<Vec<NodeEmbedding>> {
        Ok(Vec::new())
    }
}

#[allow(dead_code)]
#[derive(Default)]
pub struct InMemoryOutboxRepository {
    events: RwLock<VecDeque<OutboxEvent>>,
}

impl InMemoryOutboxRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OutboxRepository for InMemoryOutboxRepository {
    async fn enqueue(
        &self,
        tenant: Uuid,
        kind: OutboxKind,
        payload: serde_json::Value,
    ) -> Result<i64> {
        let mut guard = self.events.write().await;
        let id = guard.len() as i64 + 1;
        guard.push_back(OutboxEvent {
            id,
            tenant_id: tenant,
            kind,
            payload,
            created_at: Utc::now(),
            published_at: None,
        });
        Ok(id)
    }

    async fn claim_batch(&self, size: usize) -> Result<Vec<OutboxEvent>> {
        let mut guard = self.events.write().await;
        let mut events = Vec::new();
        for _ in 0..size.min(guard.len()) {
            if let Some(mut event) = guard.pop_front() {
                event.published_at = Some(Utc::now());
                events.push(event);
            }
        }
        Ok(events)
    }

    async fn mark_published(&self, _ids: &[i64]) -> Result<()> {
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Default)]
pub struct InMemoryCache;

#[async_trait]
impl ArtifactCache for InMemoryCache {
    async fn get(&self, _tenant: Uuid, _key: &str) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    async fn set(
        &self,
        _tenant: Uuid,
        _key: &str,
        _value: &serde_json::Value,
        _ttl_sec: u64,
    ) -> Result<()> {
        Ok(())
    }

    async fn purge(&self, _tenant: Uuid, _key: &str) -> Result<()> {
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Default)]
pub struct InMemoryBus;

#[async_trait]
impl EventBus for InMemoryBus {
    async fn publish(&self, _topic: &str, _payload: &serde_json::Value) -> Result<()> {
        Ok(())
    }

    async fn subscribe(&self, _topic: &str) -> Result<BusSubscription> {
        Ok(BusSubscription)
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryNodeRepository;
    use crate::domain::node::KnowledgeNode;
    use crate::repository::{NodeRepository, UpsertOutcome};
    use serde_json::json;
    use uuid::Uuid;

    #[tokio::test]
    async fn upsert_inserts_and_updates_nodes() {
        let repo = InMemoryNodeRepository::new();
        let tenant = Uuid::new_v4();

        let node = KnowledgeNode::new(tenant, "note", json!({"title": "hello"}));
        let outcome = repo
            .upsert(tenant, node.clone())
            .await
            .expect("upsert succeeds");
        assert!(matches!(outcome, UpsertOutcome::Created));

        let mut updated_node = node.clone();
        updated_node.payload_json = json!({"title": "updated"});
        let outcome = repo
            .upsert(tenant, updated_node.clone())
            .await
            .expect("upsert succeeds");
        assert!(matches!(outcome, UpsertOutcome::Updated));

        let fetched = repo.get(tenant, node.id).await.expect("get succeeds");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().payload_json["title"], "updated");
    }

    #[tokio::test]
    async fn query_by_kind_respects_cursor() {
        let repo = InMemoryNodeRepository::new();
        let tenant = Uuid::new_v4();

        let mut ids = Vec::new();
        for title in ["a", "b", "c"] {
            let mut node = KnowledgeNode::new(tenant, "note", json!({"title": title}));
            node.id = Uuid::new_v4();
            ids.push(node.id);
            repo.upsert(tenant, node).await.unwrap();
        }

        let first_page = repo.query_by_kind(tenant, "note", 2, None).await.unwrap();
        assert_eq!(first_page.len(), 2);

        let cursor = first_page.last().unwrap().id;
        let second_page = repo
            .query_by_kind(tenant, "note", 2, Some(cursor))
            .await
            .unwrap();
        assert_eq!(second_page.len(), 1);
    }
}
