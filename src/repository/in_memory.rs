// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Simple in-memory repository used for early development and testing flows.

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::domain::node::KnowledgeNode;

use super::{NodeRepository, UpsertOutcome};

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
