-- Enforce row-level security for all tenants tables regardless of role.
BEGIN;

ALTER TABLE tenants FORCE ROW LEVEL SECURITY;
ALTER TABLE knowledge_nodes FORCE ROW LEVEL SECURITY;
ALTER TABLE knowledge_edges FORCE ROW LEVEL SECURITY;
ALTER TABLE node_embeddings FORCE ROW LEVEL SECURITY;
ALTER TABLE outbox_events FORCE ROW LEVEL SECURITY;

COMMIT;
