// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Integration test for Postgres-backed node repository (requires DATABASE_URL).

use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use synagraph::domain::node::KnowledgeNode;
use synagraph::repository::postgres::{
    PostgresEdgeRepository, PostgresEmbeddingRepository, PostgresNodeRepository,
    PostgresOutboxRepository,
};
use synagraph::repository::{
    EdgeRepository, EmbeddingRepository, NodeEmbedding, NodeRepository, OutboxKind,
    OutboxRepository, UpsertOutcome,
};
use uuid::Uuid;

const MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[tokio::test]
async fn postgres_node_repository_respects_tenant_rls() -> Result<()> {
    dotenvy::dotenv().ok();
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!(
                "skipping postgres_node_repository_respects_tenant_rls (DATABASE_URL not set)"
            );
            return Ok(());
        }
    };

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    MIGRATIONS.run(&pool).await?;

    // Clean tables between runs.
    sqlx::query(
        r#"
        TRUNCATE outbox_events,
                 node_embeddings,
                 knowledge_edges,
                 knowledge_nodes,
                 tenants
        RESTART IDENTITY CASCADE
    "#,
    )
    .execute(&pool)
    .await?;

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();

    sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, $2)")
        .bind(tenant_a)
        .bind("Tenant A")
        .execute(&pool)
        .await?;

    sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, $2)")
        .bind(tenant_b)
        .bind("Tenant B")
        .execute(&pool)
        .await?;

    let repo = PostgresNodeRepository::connect(&database_url).await?;
    let pool = repo.pool();
    let edge_repo = PostgresEdgeRepository::new(pool.clone());
    let embedding_repo = PostgresEmbeddingRepository::new(pool.clone());
    let outbox_repo = PostgresOutboxRepository::new(pool.clone());

    let mut node = KnowledgeNode::new(tenant_a, "note", json!({ "title": "pg" }));
    let node_id = node.id;

    let outcome = repo.upsert(tenant_a, node.clone()).await?;
    assert!(matches!(outcome, UpsertOutcome::Created));

    let fetched = repo.get(tenant_a, node_id).await?;
    let fetched = fetched.expect("node present for tenant A");
    assert_eq!(fetched.tenant_id, tenant_a);
    assert_eq!(fetched.payload_json["title"], "pg");

    let forbidden = repo.get(tenant_b, node_id).await?;
    assert!(forbidden.is_none(), "tenant B should not see tenant A node");

    node.payload_json = json!({ "title": "pg-updated" });
    let outcome = repo.upsert(tenant_a, node.clone()).await?;
    assert!(matches!(outcome, UpsertOutcome::Updated));

    let results = repo.query_by_kind(tenant_a, "note", 10, None).await?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].payload_json["title"], "pg-updated");

    // Edge repository: link another node and ensure tenant isolation.
    let neighbor = KnowledgeNode::new(tenant_a, "note", json!({ "title": "neighbor" }));
    repo.upsert(tenant_a, neighbor.clone()).await?;

    edge_repo
        .link(tenant_a, node_id, neighbor.id, "RELATED", 1.0, None)
        .await?;

    let neighbors = edge_repo.neighbors(tenant_a, node_id, None, 1, 10).await?;
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].kind, neighbor.kind);

    let other_neighbors = edge_repo.neighbors(tenant_b, node_id, None, 1, 10).await?;
    assert!(other_neighbors.is_empty());

    // Embedding repository currently a stub; ensure calls succeed.
    embedding_repo
        .upsert_embedding(
            tenant_a,
            NodeEmbedding {
                node_id: neighbor.id,
                tenant_id: tenant_a,
                model: "test".to_string(),
                dim: 4,
                vec: vec![0.0; 4],
                created_at: Utc::now(),
            },
        )
        .await?;
    let embeddings = embedding_repo.get_embeddings(tenant_a, neighbor.id).await?;
    assert!(embeddings.is_empty(), "stub currently no-ops");

    // Outbox repository roundtrip.
    let event_id = outbox_repo
        .enqueue(tenant_a, OutboxKind::Upsert, json!({"node_id": node_id}))
        .await?;
    assert!(event_id > 0);
    let mut batch = outbox_repo.claim_batch(10).await?;
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].tenant_id, tenant_a);
    assert_eq!(batch[0].payload["node_id"], json!(node_id));
    outbox_repo
        .mark_published(&[batch.pop().unwrap().id])
        .await?;

    repo.health_check().await?;

    Ok(())
}
