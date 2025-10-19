// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// PostgreSQL-backed implementation of the NodeRepository trait.

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{PgPool, Postgres, Row};
use uuid::Uuid;

use crate::domain::node::KnowledgeNode;

use super::{
    ArtifactCache, BusSubscription, EdgeRepository, EmbeddingRepository, EventBus, NodeEmbedding,
    NodeRepository, OutboxEvent, OutboxKind, OutboxRepository, UpsertOutcome,
};

fn map_node_row(row: &PgRow) -> Result<KnowledgeNode> {
    let id: Uuid = row.try_get("id")?;
    let tenant_id: Uuid = row.try_get("tenant_id")?;
    let kind: String = row.try_get("kind")?;
    let payload_json: Value = row.try_get("payload_json")?;
    let provenance: Option<Value> = row.try_get("provenance")?;
    let policy: Option<Value> = row.try_get("policy")?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;

    Ok(KnowledgeNode {
        id,
        tenant_id,
        kind,
        payload_json,
        vector: None,
        provenance,
        policy,
        created_at,
        updated_at,
    })
}

#[derive(Clone)]
pub struct PostgresNodeRepository {
    pool: PgPool,
}

impl PostgresNodeRepository {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .context("failed to connect to postgres")?;

        Ok(Self { pool })
    }

    #[cfg(test)]
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NodeRepository for PostgresNodeRepository {
    async fn upsert(&self, tenant: Uuid, mut node: KnowledgeNode) -> Result<UpsertOutcome> {
        node.tenant_id = tenant;
        let mut conn = self.pool.acquire().await.context("acquire connection")?;
        set_tenant_on_conn(&mut conn, tenant).await?;

        let provenance = node.provenance.clone();
        let policy = node.policy.clone();

        let row = sqlx::query(
            r#"
            INSERT INTO knowledge_nodes (id, tenant_id, kind, payload_json, provenance, policy)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                kind = EXCLUDED.kind,
                payload_json = EXCLUDED.payload_json,
                provenance = EXCLUDED.provenance,
                policy = EXCLUDED.policy,
                updated_at = now()
            RETURNING (xmax = 0) AS created
        "#,
        )
        .bind(node.id)
        .bind(node.tenant_id)
        .bind(&node.kind)
        .bind(node.payload_json.clone())
        .bind(provenance)
        .bind(policy)
        .fetch_one(&mut *conn)
        .await
        .context("failed to upsert knowledge node")?;

        let created: bool = row.try_get("created")?;
        Ok(if created {
            UpsertOutcome::Created
        } else {
            UpsertOutcome::Updated
        })
    }

    async fn get(&self, tenant: Uuid, id: Uuid) -> Result<Option<KnowledgeNode>> {
        let mut conn = self.pool.acquire().await.context("acquire connection")?;
        set_tenant_on_conn(&mut conn, tenant).await?;

        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, kind, payload_json, vector, provenance, policy, created_at, updated_at
            FROM knowledge_nodes
            WHERE id = $1
        "#,
        )
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .context("failed to fetch knowledge node")?;

        match row {
            Some(row) => Ok(Some(map_node_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn query_by_kind(
        &self,
        tenant: Uuid,
        kind: &str,
        limit: usize,
        cursor: Option<Uuid>,
    ) -> Result<Vec<KnowledgeNode>> {
        let mut conn = self.pool.acquire().await.context("acquire connection")?;
        set_tenant_on_conn(&mut conn, tenant).await?;

        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, kind, payload_json, provenance, policy, created_at, updated_at
            FROM knowledge_nodes
            WHERE tenant_id = $1
              AND kind = $2
              AND ($3::uuid IS NULL OR id > $3)
            ORDER BY created_at DESC, id ASC
            LIMIT $4
        "#,
        )
        .bind(tenant)
        .bind(kind)
        .bind(cursor)
        .bind(limit as i64)
        .fetch_all(&mut *conn)
        .await
        .context("failed to query knowledge nodes by kind")?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(map_node_row(&row)?);
        }
        Ok(results)
    }

    async fn search_similar(
        &self,
        tenant: Uuid,
        vector: &[f32],
        limit: usize,
    ) -> Result<Vec<KnowledgeNode>> {
        let _ = (tenant, vector, limit); // vector search pending pgvector integration.
        Ok(Vec::new())
    }

    async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .context("postgres health check failed")
            .map(|_| ())
    }
}

pub async fn set_tenant_on_conn(
    conn: &mut sqlx::pool::PoolConnection<Postgres>,
    tenant: Uuid,
) -> Result<()> {
    sqlx::query("SELECT set_config('app.current_tenant', $1, true)")
        .bind(tenant.to_string())
        .execute(conn.as_mut())
        .await
        .context("failed to set tenant context")
        .map(|_| ())
}

#[derive(Clone)]
pub struct PostgresEdgeRepository {
    pool: PgPool,
}

impl PostgresEdgeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EdgeRepository for PostgresEdgeRepository {
    async fn link(
        &self,
        tenant: Uuid,
        src: Uuid,
        dst: Uuid,
        rel: &str,
        weight: f32,
        props: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut conn = self.pool.acquire().await.context("acquire connection")?;
        set_tenant_on_conn(&mut conn, tenant).await?;

        sqlx::query(
            r#"
            INSERT INTO knowledge_edges (tenant_id, src, dst, rel, weight, props)
            VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        )
        .bind(tenant)
        .bind(src)
        .bind(dst)
        .bind(rel)
        .bind(weight)
        .bind(props)
        .execute(&mut *conn)
        .await
        .context("failed to insert edge")
        .map(|_| ())
    }

    async fn neighbors(
        &self,
        tenant: Uuid,
        id: Uuid,
        rel: Option<&str>,
        hops: u8,
        limit: usize,
    ) -> Result<Vec<KnowledgeNode>> {
        let mut conn = self.pool.acquire().await.context("acquire connection")?;
        set_tenant_on_conn(&mut conn, tenant).await?;

        let rows = sqlx::query(
            r#"
            SELECT n.id, n.tenant_id, n.kind, n.payload_json, n.provenance, n.policy, n.created_at, n.updated_at
            FROM knowledge_edges e
            JOIN knowledge_nodes n ON n.id = e.dst
            WHERE e.tenant_id = $1
              AND e.src = $2
              AND ($3::text IS NULL OR e.rel = $3)
            ORDER BY e.created_at DESC
            LIMIT $4
        "#,
        )
        .bind(tenant)
        .bind(id)
        .bind(rel)
        .bind(limit as i64)
        .fetch_all(&mut *conn)
        .await
        .context("failed to fetch neighbors")?;

        let mut nodes = Vec::with_capacity(rows.len());
        for row in rows {
            nodes.push(map_node_row(&row)?);
        }
        let _ = hops; // multi-hop traversal planned via recursive CTEs.
        Ok(nodes)
    }
}

#[derive(Clone)]
pub struct PostgresEmbeddingRepository {
    pool: PgPool,
}

impl PostgresEmbeddingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EmbeddingRepository for PostgresEmbeddingRepository {
    async fn upsert_embedding(&self, tenant: Uuid, embedding: NodeEmbedding) -> Result<()> {
        let mut conn = self.pool.acquire().await.context("acquire connection")?;
        set_tenant_on_conn(&mut conn, tenant).await?;

        sqlx::query(
            r#"
            INSERT INTO node_embeddings (node_id, tenant_id, model, dim, vec)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (node_id, model) DO UPDATE SET
                dim = EXCLUDED.dim,
                vec = EXCLUDED.vec,
                created_at = now()
        "#,
        )
        .bind(embedding.node_id)
        .bind(tenant)
        .bind(&embedding.model)
        .bind(embedding.dim)
        .bind(embedding.vec)
        .execute(&mut *conn)
        .await
        .context("failed to upsert embedding")
        .map(|_| ())
    }

    async fn get_embeddings(&self, tenant: Uuid, node_id: Uuid) -> Result<Vec<NodeEmbedding>> {
        let mut conn = self.pool.acquire().await.context("acquire connection")?;
        set_tenant_on_conn(&mut conn, tenant).await?;

        let rows = sqlx::query(
            r#"
            SELECT node_id, tenant_id, model, dim, vec, created_at
            FROM node_embeddings
            WHERE tenant_id = $1 AND node_id = $2
        "#,
        )
        .bind(tenant)
        .bind(node_id)
        .fetch_all(&mut *conn)
        .await
        .context("failed to fetch embeddings")?;

        let mut embeddings = Vec::with_capacity(rows.len());
        for row in rows {
            embeddings.push(NodeEmbedding {
                node_id: row.try_get("node_id")?,
                tenant_id: row.try_get("tenant_id")?,
                model: row.try_get("model")?,
                dim: row.try_get("dim")?,
                vec: row.try_get("vec")?,
                created_at: row.try_get("created_at")?,
            });
        }
        Ok(embeddings)
    }
}

#[derive(Clone)]
pub struct PostgresOutboxRepository {
    pool: PgPool,
}

impl PostgresOutboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutboxRepository for PostgresOutboxRepository {
    async fn enqueue(&self, tenant: Uuid, kind: OutboxKind, payload: Value) -> Result<i64> {
        let mut conn = self.pool.acquire().await.context("acquire connection")?;
        set_tenant_on_conn(&mut conn, tenant).await?;

        let row = sqlx::query(
            r#"
            INSERT INTO outbox_events (tenant_id, kind, payload)
            VALUES ($1, $2, $3)
            RETURNING id
        "#,
        )
        .bind(tenant)
        .bind(kind.as_str())
        .bind(payload)
        .fetch_one(&mut *conn)
        .await
        .context("failed to enqueue outbox event")?;

        let id: i64 = row.try_get("id")?;
        Ok(id)
    }

    async fn claim_batch(&self, size: usize) -> Result<Vec<OutboxEvent>> {
        let mut conn = self.pool.acquire().await.context("acquire connection")?;

        let rows = sqlx::query(
            r#"
            UPDATE outbox_events
            SET published_at = now()
            WHERE id IN (
                SELECT id
                FROM outbox_events
                WHERE published_at IS NULL
                ORDER BY created_at ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id, tenant_id, kind, payload, created_at, published_at
        "#,
        )
        .bind(size as i64)
        .fetch_all(&mut *conn)
        .await
        .context("failed to claim outbox batch")?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(OutboxEvent {
                id: row.try_get("id")?,
                tenant_id: row.try_get("tenant_id")?,
                kind: match row.try_get::<String, _>("kind")?.as_str() {
                    "UPSERT" => OutboxKind::Upsert,
                    "SUPERSEDED_BY" => OutboxKind::SupersededBy,
                    "REVOKE_CAPSULE" => OutboxKind::RevokeCapsule,
                    other => anyhow::bail!("unknown outbox kind {other}"),
                },
                payload: row.try_get("payload")?,
                created_at: row.try_get("created_at")?,
                published_at: row.try_get("published_at")?,
            });
        }
        Ok(events)
    }

    async fn mark_published(&self, ids: &[i64]) -> Result<()> {
        let mut conn = self.pool.acquire().await.context("acquire connection")?;

        sqlx::query("UPDATE outbox_events SET published_at = now() WHERE id = ANY($1)")
            .bind(ids)
            .execute(&mut *conn)
            .await
            .context("failed to mark outbox events published")
            .map(|_| ())
    }
}

#[derive(Clone, Default)]
pub struct InMemoryCache;

#[async_trait]
impl ArtifactCache for InMemoryCache {
    async fn get(&self, _tenant: Uuid, _key: &str) -> Result<Option<Value>> {
        Ok(None)
    }

    async fn set(&self, _tenant: Uuid, _key: &str, _value: &Value, _ttl_sec: u64) -> Result<()> {
        Ok(())
    }

    async fn purge(&self, _tenant: Uuid, _key: &str) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct InMemoryBus;

#[async_trait]
impl EventBus for InMemoryBus {
    async fn publish(&self, _topic: &str, _payload: &Value) -> Result<()> {
        Ok(())
    }

    async fn subscribe(&self, _topic: &str) -> Result<BusSubscription> {
        Ok(BusSubscription)
    }
}
