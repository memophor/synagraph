// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Simple in-memory repository used for early development and testing flows.

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::domain::node::KnowledgeNode;

use super::{NodeRepository, UpsertOutcome};

#[derive(Default)]
pub struct InMemoryNodeRepository {
    inner: RwLock<HashMap<Uuid, KnowledgeNode>>,
}

impl InMemoryNodeRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl NodeRepository for InMemoryNodeRepository {
    async fn upsert(&self, mut node: KnowledgeNode) -> Result<UpsertOutcome> {
        let mut guard = self.inner.write().await;
        let created = !guard.contains_key(&node.id);
        node.created = created;
        guard.insert(node.id, node.clone());

        Ok(UpsertOutcome { node, created })
    }

    async fn get(&self, id: Uuid) -> Result<Option<KnowledgeNode>> {
        let guard = self.inner.read().await;
        Ok(guard.get(&id).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryNodeRepository;
    use crate::domain::node::KnowledgeNode;
    use crate::repository::NodeRepository;
    use serde_json::json;

    #[tokio::test]
    async fn upsert_inserts_and_updates_nodes() {
        let repo = InMemoryNodeRepository::new();

        let created = repo
            .upsert(KnowledgeNode::new("note", json!({"title": "hello"})))
            .await
            .expect("upsert succeeds");
        assert!(created.created);

        let id = created.node.id;
        let mut updated_node = created.node.clone();
        updated_node.payload = json!({"title": "updated"});

        let updated = repo
            .upsert(updated_node.clone())
            .await
            .expect("upsert succeeds");
        assert!(!updated.created);
        assert_eq!(updated.node.payload["title"], "updated");

        let fetched = repo.get(id).await.expect("get succeeds");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().payload["title"], "updated");
    }
}
