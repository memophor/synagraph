// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Repository abstractions provide persistence interfaces decoupled from storage backends.

pub mod in_memory;

use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use crate::domain::node::KnowledgeNode;

#[derive(Clone, Debug)]
pub struct UpsertOutcome {
    pub node: KnowledgeNode,
    pub created: bool,
}

#[async_trait]
pub trait NodeRepository: Send + Sync {
    async fn upsert(&self, node: KnowledgeNode) -> anyhow::Result<UpsertOutcome>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<KnowledgeNode>>;
}

pub type NodeRepositoryHandle = Arc<dyn NodeRepository>;
