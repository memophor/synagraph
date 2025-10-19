<!-- SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions. -->
# Storage Architecture Plan

This document sketches the Phase 1 build-out for persistent graph storage and repository interfaces.

## Goals

- Persist knowledge nodes (and later edges) with metadata suitable for temporal scoring, provenance, and policy enforcement.
- Support both OLTP upsert/query workloads and vector similarity search.
- Provide testable repository abstractions decoupled from specific engines.

## Proposed Stack

| Concern              | Candidate Technology                       | Notes |
|----------------------|--------------------------------------------|-------|
| Primary graph store  | PostgreSQL + pgx/pgvector (or CockroachDB) | ACID transactions, SQL familiarity, support for JSONB metadata. |
| Vector index         | `pgvector` extension initially; later Qdrant/Weaviate for scale | Reuse Postgres initially to simplify ops. |
| Cache/eventual props | Redis or NATS (Phase 2+)                    | For propagation fan-out and TTL decay jobs. |

## Schema Draft (Postgres)

### Tables

```sql
CREATE TABLE knowledge_nodes (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    payload_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    vector VECTOR(1536), -- optional, aligns with embedding dimension
    provenance JSONB,
    policy JSONB
);

CREATE INDEX idx_knowledge_nodes_kind ON knowledge_nodes(kind);
CREATE INDEX idx_knowledge_nodes_updated_at ON knowledge_nodes(updated_at);
```

Edges will land in a follow-on table (`knowledge_edges`) once relationships are required.

## Repository Interfaces (Rust)

Define traits capturing storage behaviour to aid testing:

```rust
#[async_trait]
pub trait NodeRepository {
    async fn upsert(&self, node: KnowledgeNode) -> Result<UpsertOutcome>;
    async fn get(&self, id: Uuid) -> Result<Option<KnowledgeNode>>;
    async fn query_by_kind(&self, kind: &str, limit: usize) -> Result<Vec<KnowledgeNode>>;
    async fn search_similar(&self, vector: &[f32], limit: usize) -> Result<Vec<KnowledgeNode>>;
}
```

A Postgres-backed implementation would use `sqlx` or `tokio-postgres`, while tests can use an in-memory mock (HashMap) until the DB layer is ready.

## Migration Workflow

1. Introduce `sqlx-cli` or `refinery` for migrations.
2. Populate initial schema migration and ensure `cargo sqlx prepare` (if using `sqlx`) is part of CI.
3. For local dev, provide `docker-compose` to launch Postgres with `pgvector`.
4. Seed data for manual testing via scripts.

## Open Questions

- Will early adopters require multi-tenant isolation? If so, add `tenant_id` column and extend queries.
- Should vector embeddings be optional per node, or enforced? For optional, allow `NULL` vector columns.
- Decide on embedder integration point (async job vs inline) before finalizing `vector` field behaviour.

## Timeline

1. Phase 1a: Introduce repository traits, HashMap stub implementation, wiring through gRPC service for tests.
2. Phase 1b: Add Postgres integration, migrations, and swap the service to use real persistence behind feature flags.
3. Phase 1c: Add vector search support using `pgvector` and corresponding repository methods.
