<!-- SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions. -->
# Vision & Platform Principles

SynaGraph aims to become the synaptic graph substrate that powers the Knowlemesh ecosystem and adjacent products. This document captures the guiding principles and the high-level technical direction for the platform.

## Product Pillars

- **Unified Knowledge Fabric** – encode people, artifacts, and interactions as a single graph so context flows freely across applications.
- **Semantic Intelligence** – enrich graph entities with embeddings and relationships so similarity and recommendation surfaces feel native.
- **Temporal Awareness** – decay or reinforce knowledge over time, letting fresher, high-signal data stay prominent without manual curation.
- **Policy-Aware Provenance** – every node/edge carries lineage and policy metadata so compliance and governance are first-class.

## Architectural Principles

1. **Composable Building Blocks** – keep graph, vector, policy, and temporal subsystems modular so teams can evolve them independently.
2. **APIs First** – treat the protobuf contract and eventual GraphQL/REST surfaces as the authoritative source of truth, with server implementations following suit.
3. **Observability from Day One** – trace diagnostics across HTTP, gRPC, and storage layers; expose health signals that operators can automate against.
4. **Open Source Friendly** – minimize bespoke infrastructure and lean on well-supported OSS projects to accelerate community adoption.

## Storage Strategy

We do **not** plan to invent a bespoke storage engine. Existing technologies already cover our needs:

- **Graph persistence** – start with PostgreSQL (with JSONB) or a managed graph database if operational simplicity is critical. Introduce `pgvector` for embedding storage.
- **Vector search** – lean on `pgvector` initially; graduate to Qdrant/Weaviate/Milvus only when workloads demand specialized ANN performance.
- **Temporal & Policy behaviour** – implement in the service layer via scheduled jobs, score recalculations, and policy interceptors. Storage just needs indexed fields.
- **Event propagation** – optional Redis/NATS/Kafka for streaming updates; not a Phase 1 requirement.

If scale or workload characteristics ever exceed what commodity databases deliver, we will collect benchmarks before evaluating purpose-built or custom engines.

## Near-Term Milestones

1. **Phase 1a** – repository traits with in-memory adapter, gRPC upsert/read flows, basic telemetry.
2. **Phase 1b** – Postgres + pgvector integration, migration tooling, readiness probes that test storage links.
3. **Phase 2** – expand API surface (edges, queries), introduce policy enforcement and provenance eventing.
4. **Phase 3** – background workers for temporal decay, vector refresh, and graph propagation.

## Long-Term Bets

- Rich policy language for row-/edge-level access decisions.
- Knowledge “activation” pipeline that promotes content into downstream systems based on signals.
- Collaboration with the open-source community around connectors, schema recipes, and governance playbooks.

Stay focused on delivering value through the graph APIs and intelligence layers; revisit foundational choices only when the data tells us we have to.
