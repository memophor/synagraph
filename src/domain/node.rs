// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// KnowledgeNode models the fundamental vertex payload persisted and exchanged via the API.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeNode {
    pub id: Uuid,
    pub kind: String,
    pub payload: serde_json::Value,
    pub created: bool,
}

impl KnowledgeNode {
    pub fn new(kind: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind: kind.into(),
            payload,
            created: true,
        }
    }
}
