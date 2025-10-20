export interface UpsertNodeRequest {
  tenantId?: string;
  nodeId?: string;
  kind: string;
  payload: unknown;
  embedding?: number[];
  provenance?: unknown;
  decayLambda?: number;
}

export interface UpsertNodeResponse {
  node_id: string;
  created: boolean;
  superseded_by?: {
    old_id: string;
    new_id: string;
  };
}

export interface LookupNodeRequest {
  tenantId?: string;
  nodeId: string;
}

export interface GraphNode {
  node_id: string;
  kind: string;
  payload: unknown;
  embedding?: number[];
  provenance?: unknown;
  decay_lambda?: number;
  created_at?: string;
  updated_at?: string;
}

export interface GraphEdge {
  edge_id: string;
  kind: string;
  from_id: string;
  to_id: string;
  weight?: number;
  payload?: unknown;
  provenance?: unknown;
  created_at?: string;
}

export interface RelateNodesRequest {
  tenantId?: string;
  fromId: string;
  toId: string;
  kind: string;
  weight?: number;
  payload?: unknown;
  provenance?: unknown;
}

export interface RelateNodesResponse {
  edge_id: string;
  created: boolean;
}

export interface HybridSearchRequest {
  tenantId?: string;
  queryText?: string;
  queryVector?: number[];
  topK?: number;
  filter?: unknown;
}

export interface HybridSearchResult {
  node_id: string;
  score: number;
  reason?: string;
  metadata?: Record<string, unknown>;
}

export interface HybridSearchResponse {
  took_ms?: number;
  top_k: number;
  results: HybridSearchResult[];
}

export interface NeighborsResponse {
  node: GraphNode | null;
  neighbors: GraphNode[];
  edges: GraphEdge[];
}

export interface CapsuleIngestRequest {
  tenantId?: string;
  capsule: unknown;
  unwrap?: boolean;
}

export interface CapsuleIngestResponse {
  capsule_id: string;
  upserted_nodes: string[];
  events: HistoryEvent[];
  message?: string;
}

export interface DecayRequest {
  tenantId?: string;
  nodeId?: string;
  lambda?: number;
  reinforce?: boolean;
}

export interface DecayResponse {
  message: string;
  decayed_nodes: number;
  reinforced?: boolean;
}

export interface EmitEventRequest {
  tenantId?: string;
  eventType: string;
  detail: unknown;
}

export interface EmitEventResponse {
  message: string;
  event_id: string;
}

export interface OverviewResponse {
  cache_hits: number;
  cache_misses: number;
  total_stores: number;
  total_lookups: number;
  total_purges: number;
  hit_rate: number;
  last_updated: string | null;
  postgres_health?: string;
  pgvector_health?: string;
  index_status?: string;
  embedding_dims?: number;
  decay_profiles?: Array<{
    lambda: number;
    count: number;
  }>;
}

export interface HistoryEvent {
  timestamp: string;
  event_type: string;
  tenant_id: string;
  detail: unknown;
}

export interface ScedgeMetric {
  name: string;
  value: number;
}

export interface ScedgeStatusResponse {
  configured: boolean;
  healthy: boolean;
  fetched_at: string;
  health?: {
    status: string;
    service: string;
    version: string;
  };
  metrics?: ScedgeMetric[];
  errors: string[];
}

export interface ScedgeActionResponse<T = unknown> {
  status: number;
  ok: boolean;
  body: T;
}

async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, {
    headers: { 'Content-Type': 'application/json', ...(init?.headers || {}) },
    ...init,
  });
  if (!res.ok) {
    throw new Error(await res.text());
  }
  return res.json() as Promise<T>;
}

async function fetchWithStatus<T = unknown>(path: string, init?: RequestInit): Promise<ScedgeActionResponse<T>> {
  const res = await fetch(path, {
    headers: { 'Content-Type': 'application/json', ...(init?.headers || {}) },
    ...init,
  });
  const text = await res.text();
  let body: unknown = null;
  if (text) {
    try {
      body = JSON.parse(text);
    } catch {
      body = text;
    }
  }
  return {
    status: res.status,
    ok: res.ok,
    body: body as T,
  };
}

export const fetchOverview = () => api<OverviewResponse>('/api/overview');
export const fetchHistory = () => api<HistoryEvent[]>('/api/history');
export const clearHistory = () => api<{ message: string }>('/api/history/clear', { method: 'POST' });

export const upsertNode = (body: UpsertNodeRequest) =>
  api<UpsertNodeResponse>('/api/nodes', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: body.tenantId,
      node_id: body.nodeId,
      kind: body.kind,
      payload: body.payload,
      embedding: body.embedding,
      provenance: body.provenance,
      decay_lambda: body.decayLambda,
    }),
  });

export const relateNodes = (body: RelateNodesRequest) =>
  api<RelateNodesResponse>('/api/edges', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: body.tenantId,
      from_id: body.fromId,
      to_id: body.toId,
      kind: body.kind,
      weight: body.weight,
      payload: body.payload,
      provenance: body.provenance,
    }),
  });

export const lookupNode = (request: LookupNodeRequest) =>
  api<GraphNode>(`/api/nodes/${encodeURIComponent(request.nodeId)}${
    request.tenantId ? `?tenant_id=${encodeURIComponent(request.tenantId)}` : ''
  }`);

export const fetchNeighbors = (request: LookupNodeRequest) =>
  api<NeighborsResponse>(`/api/neighbors/${encodeURIComponent(request.nodeId)}${
    request.tenantId ? `?tenant_id=${encodeURIComponent(request.tenantId)}` : ''
  }`);

export const hybridSearch = (body: HybridSearchRequest) =>
  api<HybridSearchResponse>('/api/search', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: body.tenantId,
      query_text: body.queryText,
      query_vector: body.queryVector,
      top_k: body.topK,
      filter: body.filter,
    }),
  });

export const ingestCapsule = (body: CapsuleIngestRequest) =>
  api<CapsuleIngestResponse>('/api/ingest/capsule', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: body.tenantId,
      capsule: body.capsule,
      unwrap: body.unwrap,
    }),
  });

export const triggerDecay = (body: DecayRequest) =>
  api<DecayResponse>('/api/operations/decay', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: body.tenantId,
      node_id: body.nodeId,
      lambda: body.lambda,
      reinforce: body.reinforce,
    }),
  });

export const emitTestEvent = (body: EmitEventRequest) =>
  api<EmitEventResponse>('/api/events/test', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: body.tenantId,
      event_type: body.eventType,
      detail: body.detail,
    }),
  });

export const purgeArtifacts = (tenantId?: string, reason?: string) =>
  api<{ message: string }>('/api/operations/purge', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, reason }),
  });

export const fetchScedgeStatus = () => api<ScedgeStatusResponse>('/api/scedge/status');

export const scedgeLookup = (params: { key: string; tenant?: string }) => {
  const search = new URLSearchParams({ key: params.key });
  if (params.tenant) {
    search.set('tenant', params.tenant);
  }
  return fetchWithStatus(`/api/scedge/lookup?${search.toString()}`);
};

export const scedgeStore = (payload: unknown) =>
  fetchWithStatus('/api/scedge/store', {
    method: 'POST',
    body: JSON.stringify(payload),
  });

export const scedgePurge = (payload: unknown) =>
  fetchWithStatus('/api/scedge/purge', {
    method: 'POST',
    body: JSON.stringify(payload),
  });
