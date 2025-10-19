// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Integration test for Postgres-backed node repository (requires DATABASE_URL).

use std::time::Duration;

use anyhow::Result;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use synagraph::domain::node::KnowledgeNode;
use synagraph::repository::postgres::PostgresNodeRepository;
use synagraph::repository::{NodeRepository, UpsertOutcome};
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

    repo.health_check().await?;

    Ok(())
}
