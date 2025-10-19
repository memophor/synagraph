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
