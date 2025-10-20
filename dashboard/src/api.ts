export interface StoreRequest {
  tenantId?: string;
  nodeId?: string;
  kind: string;
  payload: unknown;
}

export interface StoreResponse {
  node_id: string;
  created: boolean;
}

export interface LookupRequest {
  tenantId?: string;
  nodeId: string;
}

export interface LookupResponse {
  found: boolean;
  node: unknown | null;
}

export interface OverviewResponse {
  cache_hits: number;
  cache_misses: number;
  total_stores: number;
  total_lookups: number;
  total_purges: number;
  hit_rate: number;
  last_updated: string | null;
}

export interface HistoryEvent {
  timestamp: string;
  event_type: string;
  tenant_id: string;
  detail: unknown;
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

export const fetchOverview = () => api<OverviewResponse>('/api/overview');
export const fetchHistory = () => api<HistoryEvent[]>('/api/history');
export const clearHistory = () => api<{ message: string }>('/api/history/clear', { method: 'POST' });
export const storeNode = (body: StoreRequest) =>
  api<StoreResponse>('/api/operations/store', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: body.tenantId,
      node_id: body.nodeId,
      kind: body.kind,
      payload: body.payload,
    }),
  });
export const lookupNode = (body: LookupRequest) =>
  api<LookupResponse>('/api/operations/lookup', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: body.tenantId, node_id: body.nodeId }),
  });
export const purgeArtifacts = (tenantId?: string, reason?: string) =>
  api<{ message: string }>('/api/operations/purge', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, reason }),
  });
