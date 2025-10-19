-- SynaGraph Phase-1 schema (PostgreSQL + pgvector)

BEGIN;

CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS vector;

-- Tenants
CREATE TABLE IF NOT EXISTS tenants (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name TEXT NOT NULL UNIQUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Provide default current tenant GUC (app.current_tenant)
DO $$
BEGIN
  PERFORM 1 FROM pg_settings WHERE name = 'app.current_tenant';
  IF NOT FOUND THEN
    PERFORM set_config('app.current_tenant', '00000000-0000-0000-0000-000000000000', true);
  END IF;
END $$;

-- Nodes
CREATE TABLE IF NOT EXISTS knowledge_nodes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  payload_json JSONB NOT NULL,
  vector VECTOR(1536),
  provenance JSONB,
  policy JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_nodes_tenant_kind ON knowledge_nodes(tenant_id, kind);
CREATE INDEX IF NOT EXISTS idx_nodes_updated_at ON knowledge_nodes(updated_at);
CREATE INDEX IF NOT EXISTS idx_nodes_payload_gin ON knowledge_nodes USING GIN (payload_json jsonb_path_ops);
CREATE INDEX IF NOT EXISTS idx_nodes_provhash ON knowledge_nodes ((provenance->>'hash'));

-- Update trigger
CREATE OR REPLACE FUNCTION set_updated_at() RETURNS trigger AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_nodes_updated ON knowledge_nodes;
CREATE TRIGGER trg_nodes_updated BEFORE UPDATE ON knowledge_nodes
FOR EACH ROW EXECUTE PROCEDURE set_updated_at();

-- Edges
CREATE TABLE IF NOT EXISTS knowledge_edges (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  src UUID NOT NULL REFERENCES knowledge_nodes(id) ON DELETE CASCADE,
  dst UUID NOT NULL REFERENCES knowledge_nodes(id) ON DELETE CASCADE,
  rel TEXT NOT NULL,
  weight REAL NOT NULL DEFAULT 1.0,
  props JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_edges_tenant_src ON knowledge_edges(tenant_id, src);
CREATE INDEX IF NOT EXISTS idx_edges_tenant_dst ON knowledge_edges(tenant_id, dst);
CREATE INDEX IF NOT EXISTS idx_edges_tenant_rel ON knowledge_edges(tenant_id, rel);

-- Embeddings (normalized, optional)
CREATE TABLE IF NOT EXISTS node_embeddings (
  node_id UUID NOT NULL REFERENCES knowledge_nodes(id) ON DELETE CASCADE,
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  model TEXT NOT NULL,
  dim INT NOT NULL,
  vec VECTOR(1536) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (node_id, model)
);
CREATE INDEX IF NOT EXISTS idx_emb_tenant_model ON node_embeddings(tenant_id, model);

-- Outbox for events
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'outbox_event_kind') THEN
    CREATE TYPE outbox_event_kind AS ENUM ('UPSERT', 'SUPERSEDED_BY', 'REVOKE_CAPSULE');
  END IF;
END $$;

CREATE TABLE IF NOT EXISTS outbox_events (
  id BIGSERIAL PRIMARY KEY,
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  kind outbox_event_kind NOT NULL,
  payload JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  published_at TIMESTAMPTZ
);

CREATE OR REPLACE FUNCTION emit_upsert_event(p_tenant UUID, p_node UUID, p_hash TEXT) RETURNS VOID AS $$
BEGIN
  INSERT INTO outbox_events (tenant_id, kind, payload)
  VALUES (p_tenant, 'UPSERT', jsonb_build_object('node_id', p_node, 'provenance_hash', p_hash));
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION emit_supersede_event(p_tenant UUID, p_old UUID, p_new UUID, p_hash TEXT) RETURNS VOID AS $$
BEGIN
  INSERT INTO outbox_events (tenant_id, kind, payload)
  VALUES (p_tenant, 'SUPERSEDED_BY',
          jsonb_build_object('old_id', p_old, 'new_id', p_new, 'provenance_hash', p_hash));
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION emit_revoke_capsule_event(p_tenant UUID, p_capsule TEXT) RETURNS VOID AS $$
BEGIN
  INSERT INTO outbox_events (tenant_id, kind, payload)
  VALUES (p_tenant, 'REVOKE_CAPSULE', jsonb_build_object('capsule_id', p_capsule));
END;
$$ LANGUAGE plpgsql;

-- RLS helpers & policies
CREATE OR REPLACE FUNCTION app_current_tenant() RETURNS UUID AS $$
BEGIN
  RETURN current_setting('app.current_tenant', true)::uuid;
EXCEPTION WHEN others THEN
  RETURN '00000000-0000-0000-0000-000000000000'::uuid;
END;
$$ LANGUAGE plpgsql STABLE;

ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE knowledge_nodes ENABLE ROW LEVEL SECURITY;
ALTER TABLE knowledge_edges ENABLE ROW LEVEL SECURITY;
ALTER TABLE node_embeddings ENABLE ROW LEVEL SECURITY;
ALTER TABLE outbox_events ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenants_self ON tenants
USING (id = app_current_tenant());

CREATE POLICY nodes_tenant_isolation ON knowledge_nodes
USING (tenant_id = app_current_tenant())
WITH CHECK (tenant_id = app_current_tenant());

CREATE POLICY edges_tenant_isolation ON knowledge_edges
USING (tenant_id = app_current_tenant())
WITH CHECK (tenant_id = app_current_tenant());

CREATE POLICY emb_tenant_isolation ON node_embeddings
USING (tenant_id = app_current_tenant())
WITH CHECK (tenant_id = app_current_tenant());

CREATE POLICY outbox_tenant_isolation ON outbox_events
USING (tenant_id = app_current_tenant())
WITH CHECK (tenant_id = app_current_tenant());

COMMIT;
