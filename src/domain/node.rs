// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// KnowledgeNode models the fundamental vertex payload persisted and exchanged via the API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeNode {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub kind: String,
    pub payload_json: Value,
    pub vector: Option<Vec<f32>>,
    pub provenance: Option<Value>,
    pub policy: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl KnowledgeNode {
    pub fn new(tenant_id: Uuid, kind: impl Into<String>, payload_json: Value) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            kind: kind.into(),
            payload_json,
            vector: None,
            provenance: None,
            policy: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::KnowledgeNode;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn new_initializes_fields() {
        let tenant = Uuid::new_v4();
        let node = KnowledgeNode::new(tenant, "test", json!({"foo": "bar"}));

        assert_eq!(node.tenant_id, tenant);
        assert_eq!(node.kind, "test");
        assert_eq!(node.payload_json["foo"], "bar");
        assert!(node.vector.is_none());
        assert!(node.provenance.is_none());
    }

    #[test]
    fn touch_updates_timestamp() {
        let tenant = Uuid::new_v4();
        let mut node = KnowledgeNode::new(tenant, "note", json!({}));
        let original = node.updated_at;
        node.touch();
        assert!(node.updated_at >= original);
    }
}
