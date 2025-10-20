import { FormEvent, useCallback, useEffect, useMemo, useState } from 'react';
import {
  OverviewResponse,
  HistoryEvent,
  fetchOverview,
  fetchHistory,
  clearHistory,
  upsertNode,
  UpsertNodeResponse,
  relateNodes,
  RelateNodesResponse,
  hybridSearch,
  HybridSearchResponse,
  ingestCapsule,
  CapsuleIngestResponse,
  purgeArtifacts,
  fetchNeighbors,
  lookupNode,
  GraphNode,
  NeighborsResponse,
  triggerDecay,
  DecayResponse,
  emitTestEvent,
  EmitEventResponse,
  fetchScedgeStatus,
  scedgeLookup,
  scedgeStore,
  scedgePurge,
  ScedgeStatusResponse,
  ScedgeActionResponse,
} from './api';

type PrimaryTab = 'graph' | 'scedge' | 'upsert' | 'search' | 'edges' | 'events';

type ExplorerStatus = {
  message: string;
  tone: 'success' | 'error';
} | null;

type Flash = {
  message: string;
  tone: 'success' | 'error';
  detail?: string;
  dismissible?: boolean;
} | null;

type EventTone = 'event-success' | 'event-warning' | 'event-danger' | 'event-neutral';

const DEFAULT_PAYLOAD = `{
  "node_id": "note::synagraph::demo",
  "kind": "note",
  "payload": {
    "title": "Hello, SynaGraph",
    "summary": "Sample payload for control-plane upserts",
    "tags": ["demo", "sample"]
  }
}`;

const DEFAULT_CAPSULE = `{
  "capsule_id": "capsule::synagraph::demo",
  "tenant_id": "default",
  "artifacts": [
    {
      "kind": "note",
      "node_id": "note::synagraph::demo",
      "payload": {
        "title": "Capsule payload",
        "summary": "Wrapped CCP artifact destined for SynaGraph"
      }
    }
  ]
}`;

const DEFAULT_EVENT_DETAIL = `{
  "note": "Synthetic test event payload"
}`;

const DEFAULT_SCEDGE_LOOKUP_KEY = 'acme:analytics:report';

const DEFAULT_SCEDGE_STORE = `{
  "key": "tenant:namespace:identifier",
  "artifact": {
    "answer": "Quarterly revenue was up 23%.",
    "policy": {"tenant": "tenant", "phi": false, "pii": false},
    "provenance": [{"source": "synagraph:artifact", "hash": "sg-123"}],
    "hash": "sg-123",
    "ttl_seconds": 3600
  }
}`;

const DEFAULT_SCEDGE_PURGE = `{
  "tenant": "acme",
  "key": "acme:analytics:report"
}`;

const primaryTabs: Array<{ key: PrimaryTab; label: string }> = [
  { key: 'graph', label: 'Graph' },
  { key: 'upsert', label: 'Upsert Node' },
  { key: 'search', label: 'Search' },
  { key: 'edges', label: 'Edges' },
  { key: 'events', label: 'Events' },
];

function formatTimestamp(ts: string) {
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return ts;
  }
}

function parseJsonInput(label: string, value: string, required = false): unknown {
  const trimmed = value.trim();
  if (!trimmed) {
    if (required) {
      throw new Error(`${label} is required`);
    }
    return undefined;
  }
  try {
    return JSON.parse(trimmed);
  } catch (err) {
    throw new Error(`Invalid ${label}: ${(err as Error).message}`);
  }
}

function parseEmbedding(value: string): number[] | undefined {
  if (!value.trim()) {
    return undefined;
  }
  const parsed = parseJsonInput('embedding', value, true);
  if (!Array.isArray(parsed) || !parsed.every((entry) => typeof entry === 'number')) {
    throw new Error('Embedding must be a JSON array of numbers');
  }
  return parsed as number[];
}

function parseOptionalNumber(label: string, value: string): number | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  const num = Number(trimmed);
  if (Number.isNaN(num)) {
    throw new Error(`${label} must be numeric`);
  }
  return num;
}

function eventTone(type: string): EventTone {
  const normalized = type.toLowerCase();
  if (normalized.includes('revoke')) {
    return 'event-danger';
  }
  if (normalized.includes('superseded')) {
    return 'event-warning';
  }
  if (normalized.includes('upsert') || normalized.includes('ingest')) {
    return 'event-success';
  }
  return 'event-neutral';
}

function toPrettyJson(value: unknown): string {
  if (value == null) {
    return '';
  }
  if (typeof value === 'string') {
    return value;
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function formatUnknown(value: unknown): string {
  if (value === null || value === undefined) {
    return '';
  }
  if (typeof value === 'string') {
    return value;
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function describeError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err ?? 'Unknown error');
}

function FlashBanner({ flash, onDismiss }: { flash: NonNullable<Flash>; onDismiss: () => void }) {
  return (
    <div className={`flash-banner flash-${flash.tone}`} role={flash.tone === 'error' ? 'alert' : 'status'}>
      <div className="flash-content">
        <span>{flash.message}</span>
        {flash.detail && <span className="flash-detail">{flash.detail}</span>}
      </div>
      {flash.dismissible !== false && (
        <button type="button" className="flash-dismiss" onClick={onDismiss} aria-label="Dismiss banner">
          ×
        </button>
      )}
    </div>
  );
}

export default function App() {
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    return (localStorage.getItem('synagraph-theme') as 'light' | 'dark') || 'light';
  });
  const [activeTab, setActiveTab] = useState<PrimaryTab>('graph');
  const [overview, setOverview] = useState<OverviewResponse | null>(null);
  const [history, setHistory] = useState<HistoryEvent[]>([]);
  const [tenantId, setTenantId] = useState('');
  const [flash, setFlash] = useState<Flash>(null);
  const [focusedAction, setFocusedAction] = useState<'decay' | 'emit' | 'capsule' | null>(null);

  const [nodeForm, setNodeForm] = useState({
    nodeId: '',
    kind: 'note',
    payload: DEFAULT_PAYLOAD,
    embedding: '',
    provenance: '',
    decayLambda: '',
  });
  const [upsertLoading, setUpsertLoading] = useState(false);
  const [nodeResult, setNodeResult] = useState<UpsertNodeResponse | null>(null);

  const [edgeForm, setEdgeForm] = useState({
    fromId: '',
    toId: '',
    kind: 'relates_to',
    weight: '',
    payload: '',
    provenance: '',
  });
  const [edgeLoading, setEdgeLoading] = useState(false);
  const [edgeResult, setEdgeResult] = useState<RelateNodesResponse | null>(null);

  const [searchForm, setSearchForm] = useState({
    queryText: '',
    queryVector: '',
    topK: '8',
    filter: '',
  });
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchResult, setSearchResult] = useState<HybridSearchResponse | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);

  const [capsuleForm, setCapsuleForm] = useState({ capsule: DEFAULT_CAPSULE, unwrap: true });
  const [capsuleLoading, setCapsuleLoading] = useState(false);
  const [capsuleResult, setCapsuleResult] = useState<CapsuleIngestResponse | null>(null);

  const [decayForm, setDecayForm] = useState({ nodeId: '', lambda: '0.25', reinforce: false });
  const [decayLoading, setDecayLoading] = useState(false);
  const [decayResult, setDecayResult] = useState<DecayResponse | null>(null);

  const [eventForm, setEventForm] = useState({ eventType: 'UPSERT_NODE', detail: DEFAULT_EVENT_DETAIL });
  const [eventLoading, setEventLoading] = useState(false);
  const [eventResult, setEventResult] = useState<EmitEventResponse | null>(null);

  const [scedgeStatus, setScedgeStatus] = useState<ScedgeStatusResponse | null>(null);
  const [scedgeStatusLoading, setScedgeStatusLoading] = useState(false);
  const [scedgeStatusError, setScedgeStatusError] = useState<string | null>(null);
  const [scedgeLookupForm, setScedgeLookupForm] = useState({
    key: DEFAULT_SCEDGE_LOOKUP_KEY,
    tenant: '',
  });
  const [scedgeLookupResult, setScedgeLookupResult] = useState<ScedgeActionResponse | null>(null);
  const [scedgeLookupLoading, setScedgeLookupLoading] = useState(false);
  const [scedgeLookupError, setScedgeLookupError] = useState<string | null>(null);
  const [scedgeStoreBody, setScedgeStoreBody] = useState(DEFAULT_SCEDGE_STORE);
  const [scedgeStoreResult, setScedgeStoreResult] = useState<ScedgeActionResponse | null>(null);
  const [scedgeStoreLoading, setScedgeStoreLoading] = useState(false);
  const [scedgeStoreError, setScedgeStoreError] = useState<string | null>(null);
  const [scedgePurgeBody, setScedgePurgeBody] = useState(DEFAULT_SCEDGE_PURGE);
  const [scedgePurgeResult, setScedgePurgeResult] = useState<ScedgeActionResponse | null>(null);
  const [scedgePurgeLoading, setScedgePurgeLoading] = useState(false);
  const [scedgePurgeError, setScedgePurgeError] = useState<string | null>(null);

  const [nodeLookupId, setNodeLookupId] = useState('');
  const [nodeDetail, setNodeDetail] = useState<GraphNode | null>(null);
  const [nodeExplorerStatus, setNodeExplorerStatus] = useState<ExplorerStatus>(null);
  const [nodeLookupLoading, setNodeLookupLoading] = useState(false);

  const [neighborLookupId, setNeighborLookupId] = useState('');
  const [neighborResult, setNeighborResult] = useState<NeighborsResponse | null>(null);
  const [neighborStatus, setNeighborStatus] = useState<ExplorerStatus>(null);
  const [neighborLookupLoading, setNeighborLookupLoading] = useState(false);

  useEffect(() => {
    document.body.dataset.theme = theme === 'dark' ? 'dark' : '';
    localStorage.setItem('synagraph-theme', theme);
  }, [theme]);

  const handleRefreshError = (err: unknown) => {
    console.error(err);
    const detail = describeError(err);
    setFlash((prev) =>
      prev?.tone === 'success'
        ? prev
        : {
            tone: 'error',
            message: 'Failed to load dashboard data. Ensure the SynaGraph API is running (e.g., `cargo run`).',
            detail,
            dismissible: true,
          },
    );
  };

  const refresh = async (options?: { silent?: boolean }) => {
    try {
      const [ov, hist] = await Promise.all([fetchOverview(), fetchHistory()]);
      setOverview(ov);
      setHistory(hist);
      if (!options?.silent) {
        setFlash((prev) => (prev && prev.tone === 'error' ? null : prev));
      }
    } catch (err) {
      if (!options?.silent) {
        handleRefreshError(err);
      }
      throw err;
    }
  };

  useEffect(() => {
    refresh().catch(() => {
      /* handled via flash */
    });
  }, []);

  const tenantOrUndefined = tenantId.trim() || undefined;

  const metrics = useMemo(() => {
    if (!overview) {
      return [
        { label: 'Cache Hits', value: '-', tone: 'positive' as const },
        { label: 'Cache Misses', value: '-', tone: 'negative' as const },
        { label: 'Nodes Upserted', value: '-', tone: 'neutral' as const },
        { label: 'Lookups', value: '-', tone: 'neutral' as const },
        { label: 'Purges', value: '-', tone: 'neutral' as const },
      ];
    }
    return [
      { label: 'Cache Hits', value: overview.cache_hits.toString(), tone: 'positive' as const },
      { label: 'Cache Misses', value: overview.cache_misses.toString(), tone: 'negative' as const },
      { label: 'Nodes Upserted', value: overview.total_stores.toString(), tone: 'neutral' as const },
      { label: 'Lookups', value: overview.total_lookups.toString(), tone: 'neutral' as const },
      { label: 'Purges', value: overview.total_purges.toString(), tone: 'neutral' as const },
    ];
  }, [overview]);

  const diagnostics = useMemo(() => {
    return {
      postgres: overview?.postgres_health ?? 'Unknown',
      pgvector: overview?.pgvector_health ?? 'Unknown',
      index: overview?.index_status ?? 'Unknown',
      dims: overview?.embedding_dims ?? null,
      decayProfiles: overview?.decay_profiles ?? [],
    };
  }, [overview]);

  const nodeEventTrail = useMemo(() => {
    if (!nodeDetail) {
      return [] as HistoryEvent[];
    }
    const id = nodeDetail.node_id;
    return history.filter((event) => {
      try {
        return JSON.stringify(event.detail).includes(id);
      } catch {
        return false;
      }
    });
  }, [history, nodeDetail]);

  const provenanceJson = useMemo(() => toPrettyJson(nodeDetail?.provenance), [nodeDetail]);

  const setSuccessFlash = (message: string) => setFlash({ tone: 'success', message, dismissible: true });
  const setErrorFlash = (message: string, detail?: string) => setFlash({ tone: 'error', message, detail, dismissible: true });

  const loadScedgeStatus = useCallback(async () => {
    try {
      setScedgeStatusLoading(true);
      const status = await fetchScedgeStatus();
      setScedgeStatus(status);
      setScedgeStatusError(status.errors.length ? status.errors.join(' • ') : null);
    } catch (err) {
      console.error(err);
      setScedgeStatus(null);
      setScedgeStatusError(err instanceof Error ? err.message : String(err));
    } finally {
      setScedgeStatusLoading(false);
    }
  }, []);

  useEffect(() => {
    if (activeTab === 'scedge' && !scedgeStatus && !scedgeStatusLoading) {
      loadScedgeStatus();
    }
  }, [activeTab, scedgeStatus, scedgeStatusLoading, loadScedgeStatus]);

  const handleUpsertSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setUpsertLoading(true);
    setFlash(null);
    try {
      const payload = parseJsonInput('payload', nodeForm.payload, true);
      const embedding = parseEmbedding(nodeForm.embedding);
      const provenance = parseJsonInput('provenance', nodeForm.provenance);
      const decayLambda = parseOptionalNumber('decay λ', nodeForm.decayLambda);
      const response = await upsertNode({
        tenantId: tenantOrUndefined,
        nodeId: nodeForm.nodeId || undefined,
        kind: nodeForm.kind,
        payload,
        embedding,
        provenance,
        decayLambda,
      });
      setNodeResult(response);
      setSuccessFlash(response.created ? `Node ${response.node_id} created` : `Node ${response.node_id} updated`);
      await refresh({ silent: true }).catch(handleRefreshError);
    } catch (err) {
      console.error(err);
      setErrorFlash(err instanceof Error ? err.message : 'Node upsert failed');
    } finally {
      setUpsertLoading(false);
    }
  };

  const handleEdgeSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setEdgeLoading(true);
    setFlash(null);
    try {
      if (!edgeForm.fromId.trim() || !edgeForm.toId.trim()) {
        throw new Error('Both source and target node IDs are required');
      }
      const weight = parseOptionalNumber('weight', edgeForm.weight);
      const payload = parseJsonInput('edge payload', edgeForm.payload);
      const provenance = parseJsonInput('edge provenance', edgeForm.provenance);
      const response = await relateNodes({
        tenantId: tenantOrUndefined,
        fromId: edgeForm.fromId.trim(),
        toId: edgeForm.toId.trim(),
        kind: edgeForm.kind,
        weight,
        payload,
        provenance,
      });
      setEdgeResult(response);
      setSuccessFlash(`Edge ${response.edge_id} ${response.created ? 'created' : 'updated'}`);
      await refresh({ silent: true }).catch(handleRefreshError);
    } catch (err) {
      console.error(err);
      setErrorFlash(err instanceof Error ? err.message : 'Edge operation failed');
    } finally {
      setEdgeLoading(false);
    }
  };

  const handleSearchSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setSearchLoading(true);
    setSearchError(null);
    setFlash(null);
    try {
      if (!searchForm.queryText.trim() && !searchForm.queryVector.trim()) {
        throw new Error('Provide hybrid search text and/or vector payload');
      }
      const queryVector = parseEmbedding(searchForm.queryVector || '');
      const filter = parseJsonInput('filter', searchForm.filter);
      const topK = parseOptionalNumber('top-k', searchForm.topK) ?? 8;
      const result = await hybridSearch({
        tenantId: tenantOrUndefined,
        queryText: searchForm.queryText.trim() || undefined,
        queryVector,
        topK,
        filter,
      });
      setSearchResult(result);
      setSuccessFlash(`Hybrid search returned ${result.results.length} result${result.results.length === 1 ? '' : 's'}`);
      await refresh({ silent: true }).catch(handleRefreshError);
    } catch (err) {
      console.error(err);
      const message = err instanceof Error ? err.message : 'Hybrid search failed';
      setSearchError(message);
      setErrorFlash(message);
    } finally {
      setSearchLoading(false);
    }
  };

  const handleCapsuleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setCapsuleLoading(true);
    setFlash(null);
    try {
      const capsule = parseJsonInput('capsule', capsuleForm.capsule, true);
      const response = await ingestCapsule({
        tenantId: tenantOrUndefined,
        capsule,
        unwrap: capsuleForm.unwrap,
      });
      setCapsuleResult(response);
      setSuccessFlash(
        `Capsule ${response.capsule_id} ingested (${response.upserted_nodes.length} node${
          response.upserted_nodes.length === 1 ? '' : 's'
        })`,
      );
      setFocusedAction('capsule');
      await refresh({ silent: true }).catch(handleRefreshError);
    } catch (err) {
      console.error(err);
      setErrorFlash(err instanceof Error ? err.message : 'Capsule ingest failed');
    } finally {
      setCapsuleLoading(false);
    }
  };

  const handleDecaySubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setDecayLoading(true);
    setFlash(null);
    try {
      const lambda = parseOptionalNumber('decay λ', decayForm.lambda);
      const response = await triggerDecay({
        tenantId: tenantOrUndefined,
        nodeId: decayForm.nodeId.trim() || undefined,
        lambda,
        reinforce: decayForm.reinforce,
      });
      setDecayResult(response);
      setSuccessFlash(response.message || 'Decay applied');
      setFocusedAction('decay');
      await refresh({ silent: true }).catch(handleRefreshError);
    } catch (err) {
      console.error(err);
      setErrorFlash(err instanceof Error ? err.message : 'Decay operation failed');
    } finally {
      setDecayLoading(false);
    }
  };

  const handleEmitEventSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setEventLoading(true);
    setFlash(null);
    try {
      const detail = parseJsonInput('event detail', eventForm.detail, true);
      const response = await emitTestEvent({
        tenantId: tenantOrUndefined,
        eventType: eventForm.eventType.trim() || 'TEST_EVENT',
        detail,
      });
      setEventResult(response);
      setSuccessFlash(response.message || 'Test event emitted');
      setFocusedAction('emit');
      await refresh({ silent: true }).catch(handleRefreshError);
    } catch (err) {
      console.error(err);
      setErrorFlash(err instanceof Error ? err.message : 'Test event failed');
    } finally {
      setEventLoading(false);
    }
  };

  const handleScedgeLookup = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!scedgeLookupForm.key.trim()) {
      setScedgeLookupError('Provide a cache key');
      return;
    }
    setScedgeLookupLoading(true);
    setScedgeLookupError(null);
    try {
      const result = await scedgeLookup({
        key: scedgeLookupForm.key.trim(),
        tenant: scedgeLookupForm.tenant.trim() || undefined,
      });
      setScedgeLookupResult(result);
    } catch (err) {
      console.error(err);
      setScedgeLookupResult(null);
      setScedgeLookupError(err instanceof Error ? err.message : 'Lookup failed');
    } finally {
      setScedgeLookupLoading(false);
    }
  };

  const handleScedgeStore = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setScedgeStoreLoading(true);
    setScedgeStoreError(null);
    try {
      const payload = parseJsonInput('Scedge store payload', scedgeStoreBody, true);
      const result = await scedgeStore(payload);
      setScedgeStoreResult(result);
      await loadScedgeStatus();
    } catch (err) {
      console.error(err);
      setScedgeStoreResult(null);
      setScedgeStoreError(err instanceof Error ? err.message : 'Store failed');
    } finally {
      setScedgeStoreLoading(false);
    }
  };

  const handleScedgePurge = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setScedgePurgeLoading(true);
    setScedgePurgeError(null);
    try {
      const payload = parseJsonInput('Scedge purge payload', scedgePurgeBody, true);
      const result = await scedgePurge(payload);
      setScedgePurgeResult(result);
      await loadScedgeStatus();
    } catch (err) {
      console.error(err);
      setScedgePurgeResult(null);
      setScedgePurgeError(err instanceof Error ? err.message : 'Purge failed');
    } finally {
      setScedgePurgeLoading(false);
    }
  };

  const handleNodeLookup = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!nodeLookupId.trim()) {
      setNodeExplorerStatus({ tone: 'error', message: 'Provide a node ID' });
      return;
    }
    setNodeLookupLoading(true);
    setNodeExplorerStatus(null);
    try {
      const node = await lookupNode({ nodeId: nodeLookupId.trim(), tenantId: tenantOrUndefined });
      setNodeDetail(node);
      setNodeExplorerStatus({ tone: 'success', message: `Fetched node ${node.node_id}` });
    } catch (err) {
      console.error(err);
      setNodeDetail(null);
      setNodeExplorerStatus({
        tone: 'error',
        message: err instanceof Error ? err.message : 'Lookup failed',
      });
    } finally {
      setNodeLookupLoading(false);
    }
  };

  const handleNeighborLookup = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!neighborLookupId.trim()) {
      setNeighborStatus({ tone: 'error', message: 'Provide a node ID' });
      return;
    }
    setNeighborLookupLoading(true);
    setNeighborStatus(null);
    try {
      const result = await fetchNeighbors({ nodeId: neighborLookupId.trim(), tenantId: tenantOrUndefined });
      setNeighborResult(result);
      setNeighborStatus({
        tone: 'success',
        message: `Retrieved ${result.neighbors.length} neighbor${result.neighbors.length === 1 ? '' : 's'}`,
      });
    } catch (err) {
      console.error(err);
      setNeighborResult(null);
      setNeighborStatus({
        tone: 'error',
        message: err instanceof Error ? err.message : 'Neighbor lookup failed',
      });
    } finally {
      setNeighborLookupLoading(false);
    }
  };

  const handlePurge = async () => {
    setFlash(null);
    try {
      const response = await purgeArtifacts(tenantOrUndefined, 'dashboard purge');
      setSuccessFlash(response.message || 'Purge request submitted');
      await refresh({ silent: true }).catch(handleRefreshError);
    } catch (err) {
      console.error(err);
      setErrorFlash(err instanceof Error ? err.message : 'Purge request failed');
    }
  };

  const scrollToTop = () => {
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  const numericTopK = useMemo(() => {
    const parsed = Number(searchForm.topK);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
  }, [searchForm.topK]);

  const renderSearchPanel = (opts: { context: 'graph' | 'search'; showFilter: boolean }) => (
    <article className="panel search-panel">
      <div className="panel-header">
        <h2>{opts.context === 'graph' ? 'Vector Search Probe' : 'Hybrid Search'}</h2>
        <div className="meta">Top-{numericTopK ?? 'k'}</div>
      </div>
      <form className="operation-form" onSubmit={handleSearchSubmit}>
        <label className="field">
          <span>Query Text</span>
          <textarea
            rows={opts.context === 'graph' ? 3 : 5}
            value={searchForm.queryText}
            onChange={(e) => setSearchForm((prev) => ({ ...prev, queryText: e.target.value }))}
            placeholder="What is the latest intent for customer-123?"
          />
        </label>
        <label className="field">
          <span>Query Vector (JSON array)</span>
          <textarea
            rows={4}
            value={searchForm.queryVector}
            placeholder="Embed externally to override"
            onChange={(e) => setSearchForm((prev) => ({ ...prev, queryVector: e.target.value }))}
          />
        </label>
        {opts.showFilter && (
          <label className="field">
            <span>Filter (JSON)</span>
            <textarea
              rows={3}
              value={searchForm.filter}
              placeholder='{"kind": "note"}'
              onChange={(e) => setSearchForm((prev) => ({ ...prev, filter: e.target.value }))}
            />
          </label>
        )}
        <label className="field">
          <span>Top-k</span>
          <input
            value={searchForm.topK}
            onChange={(e) => setSearchForm((prev) => ({ ...prev, topK: e.target.value }))}
            placeholder="8"
          />
        </label>
        <button className="primary" type="submit" disabled={searchLoading}>
          {searchLoading ? 'Searching…' : 'Run Search'}
        </button>
      </form>
      {searchError && <div className="status status-error">{searchError}</div>}
      {searchResult && (
        <ul className="result-list">
          {searchResult.results.map((result) => (
            <li key={`${result.node_id}-${result.score}`} className="result-item">
              <div className="result-body">
                <div className="result-id">{result.node_id}</div>
                {result.reason && <div className="result-reason">{result.reason}</div>}
                {result.metadata && <pre className="code-block compact">{JSON.stringify(result.metadata, null, 2)}</pre>}
              </div>
              <div className="result-score">{result.score.toFixed(3)}</div>
            </li>
          ))}
        </ul>
      )}
    </article>
  );

  const graphView = (
    <section className="graph-grid">
      <div className="graph-main">
        {renderSearchPanel({ context: 'graph', showFilter: false })}
      </div>
      <aside className="graph-aside">
        <article className="panel quick-actions">
          <div className="panel-header">
            <h2>Quick Actions</h2>
          </div>
          <div className="action-grid">
            <button
              type="button"
              onClick={() => {
                setActiveTab('upsert');
                scrollToTop();
              }}
            >
              Upsert Node
            </button>
            <button
              type="button"
              onClick={() => {
                setActiveTab('edges');
                scrollToTop();
              }}
            >
              Create Edge
            </button>
            <button
              type="button"
              onClick={() => {
                setActiveTab('events');
                setFocusedAction('decay');
                scrollToTop();
              }}
            >
              Decay / Reinforce
            </button>
            <button
              type="button"
              onClick={() => {
                setActiveTab('events');
                setFocusedAction('emit');
                scrollToTop();
              }}
            >
              Emit Test Event
            </button>
          </div>
        </article>

        <article className="panel explorer">
          <div className="panel-header">
            <h2>Graph Explorer</h2>
          </div>
          <div className="explorer-section">
            <h3 className="panel-subtitle">Node detail</h3>
            <form className="inline-form" onSubmit={handleNodeLookup}>
              <label className="field">
                <span>Node ID</span>
                <input
                  value={nodeLookupId}
                  onChange={(e) => setNodeLookupId(e.target.value)}
                  placeholder="note::synagraph::demo"
                />
              </label>
              <button className="secondary" type="submit" disabled={nodeLookupLoading}>
                {nodeLookupLoading ? 'Fetching…' : 'Fetch node'}
              </button>
            </form>
            {nodeExplorerStatus && (
              <div className={`status status-${nodeExplorerStatus.tone}`}>{nodeExplorerStatus.message}</div>
            )}
            {nodeDetail && (
              <div className="data-preview">
                <div className="preview-header">
                  <span className="pill">Node</span>
                  <span className="preview-id">{nodeDetail.node_id}</span>
                </div>
                <pre className="code-block">{JSON.stringify(nodeDetail, null, 2)}</pre>
              </div>
            )}
          </div>

          <div className="explorer-section">
            <h3 className="panel-subtitle">Neighbors</h3>
            <form className="inline-form" onSubmit={handleNeighborLookup}>
              <label className="field">
                <span>Node ID</span>
                <input
                  value={neighborLookupId}
                  onChange={(e) => setNeighborLookupId(e.target.value)}
                  placeholder="node::seed"
                />
              </label>
              <button className="secondary" type="submit" disabled={neighborLookupLoading}>
                {neighborLookupLoading ? 'Fetching…' : 'Fetch neighbors'}
              </button>
            </form>
            {neighborStatus && <div className={`status status-${neighborStatus.tone}`}>{neighborStatus.message}</div>}
            {neighborResult && (
              <div className="neighbor-summary">
                <div>
                  <span className="metric-label">Neighbors</span>
                  <div className="metric-value">{neighborResult.neighbors.length}</div>
                </div>
                <div>
                  <span className="metric-label">Edges</span>
                  <div className="metric-value">{neighborResult.edges.length}</div>
                </div>
              </div>
            )}
            {neighborResult && (
              <div className="data-preview">
                <div className="preview-header">
                  <span className="pill">Neighbors payload</span>
                </div>
                <pre className="code-block">{JSON.stringify(neighborResult, null, 2)}</pre>
              </div>
            )}
          </div>
        </article>

        <article className="panel provenance">
          <div className="panel-header">
            <h2>Provenance Trail</h2>
          </div>
          {provenanceJson ? (
            <pre className="code-block">{provenanceJson}</pre>
          ) : (
            <div className="empty">Load a node to surface provenance context.</div>
          )}
          {nodeEventTrail.length > 0 && (
            <div className="trail">
              {nodeEventTrail.map((event) => (
                <div key={`${event.timestamp}-${event.event_type}`} className="trail-item">
                  <div className="trail-meta">
                    <span className={`history-tag ${eventTone(event.event_type)}`}>{event.event_type}</span>
                    <span className="history-time">{formatTimestamp(event.timestamp)}</span>
                  </div>
                  <pre className="code-block compact">{JSON.stringify(event.detail, null, 2)}</pre>
                </div>
              ))}
            </div>
          )}
        </article>

        <article className="panel diagnostics">
          <div className="panel-header">
            <h2>System Diagnostics</h2>
          </div>
          <dl className="diagnostics-list">
            <div>
              <dt>Postgres</dt>
              <dd>{diagnostics.postgres}</dd>
            </div>
            <div>
              <dt>pgvector</dt>
              <dd>{diagnostics.pgvector}</dd>
            </div>
            <div>
              <dt>Index status</dt>
              <dd>{diagnostics.index}</dd>
            </div>
            <div>
              <dt>Embedding dims</dt>
              <dd>{diagnostics.dims ?? 'Unknown'}</dd>
            </div>
          </dl>
          <div className="decay-profiles">
            <span className="panel-subtitle">Decay λ profiles</span>
            {diagnostics.decayProfiles.length === 0 ? (
              <div className="empty">No decay profiles reported.</div>
            ) : (
              <ul>
                {diagnostics.decayProfiles.map((profile) => (
                  <li key={profile.lambda}>
                    λ {profile.lambda}: <strong>{profile.count}</strong> nodes
                  </li>
                ))}
              </ul>
            )}
          </div>
        </article>
      </aside>
    </section>
  );

  const scedgeMetrics = scedgeStatus?.metrics ?? [];
  const scedgeHealthy = scedgeStatus?.healthy ?? false;

  const scedgeView = (
    <section className="scedge-grid">
      <div className="scedge-column">
        <article className="panel scedge-status">
          <div className="panel-header">
            <h2>Scedge Health</h2>
            <button className="refresh" onClick={loadScedgeStatus} disabled={scedgeStatusLoading}>
              Refresh
            </button>
          </div>
          <div className="scedge-summary">
            <div>
              <span className="metric-label">Configured</span>
              <div className="metric-value">{scedgeStatus?.configured ? 'Yes' : 'No'}</div>
            </div>
            <div>
              <span className="metric-label">Status</span>
              <div className="metric-value">{scedgeHealthy ? 'Healthy' : 'Unavailable'}</div>
            </div>
            <div>
              <span className="metric-label">Service</span>
              <div className="metric-value">{scedgeStatus?.health?.service ?? 'scedge-core'}</div>
            </div>
            <div>
              <span className="metric-label">Version</span>
              <div className="metric-value">{scedgeStatus?.health?.version ?? '—'}</div>
            </div>
          </div>
          <div className="meta">
            Fetched {scedgeStatus?.fetched_at ? formatTimestamp(scedgeStatus.fetched_at) : '—'}
          </div>
          {scedgeStatusError && <div className="status status-error">{scedgeStatusError}</div>}
          {scedgeStatus?.errors && scedgeStatus.errors.length > 0 && !scedgeStatusError && (
            <div className="status status-error">{scedgeStatus.errors.join(' • ')}</div>
          )}
        </article>

        <article className="panel scedge-metrics">
          <div className="panel-header">
            <h2>Cache Metrics</h2>
          </div>
          {scedgeMetrics.length === 0 ? (
            <div className="empty">Metrics unavailable.</div>
          ) : (
            <div className="metrics metrics-compact">
              {scedgeMetrics.map((metric) => (
                <article key={metric.name} className="metric-card metric-neutral">
                  <div className="metric-label">{metric.name.replace('scedge_', '').replace(/_/g, ' ')}</div>
                  <div className="metric-value">{metric.value}</div>
                </article>
              ))}
            </div>
          )}
        </article>
      </div>

      <div className="scedge-column">
        <article className="panel scedge-panel">
          <div className="panel-header">
            <h2>Cache Lookup</h2>
          </div>
          <form className="operation-form" onSubmit={handleScedgeLookup}>
            <label className="field">
              <span>Cache key</span>
              <input
                value={scedgeLookupForm.key}
                onChange={(e) => setScedgeLookupForm((prev) => ({ ...prev, key: e.target.value }))}
              />
            </label>
            <label className="field">
              <span>Tenant (optional)</span>
              <input
                value={scedgeLookupForm.tenant}
                onChange={(e) => setScedgeLookupForm((prev) => ({ ...prev, tenant: e.target.value }))}
              />
            </label>
            <button className="secondary" type="submit" disabled={scedgeLookupLoading}>
              {scedgeLookupLoading ? 'Fetching…' : 'Lookup'}
            </button>
          </form>
          {scedgeLookupError && <div className="status status-error">{scedgeLookupError}</div>}
          {scedgeLookupResult && (
            <div className="data-preview">
              <div className="preview-header">
                <span className="pill">Cache lookup</span>
                <span className="preview-id">Status {scedgeLookupResult.status}</span>
              </div>
              <pre className="code-block">{formatUnknown(scedgeLookupResult.body)}</pre>
            </div>
          )}
        </article>

        <article className="panel scedge-panel">
          <div className="panel-header">
            <h2>Store Capsule</h2>
          </div>
          <form className="operation-form" onSubmit={handleScedgeStore}>
            <label className="field">
              <span>Payload JSON</span>
              <textarea
                rows={8}
                value={scedgeStoreBody}
                onChange={(e) => setScedgeStoreBody(e.target.value)}
              />
            </label>
            <button className="secondary" type="submit" disabled={scedgeStoreLoading}>
              {scedgeStoreLoading ? 'Storing…' : 'Store capsule'}
            </button>
          </form>
          {scedgeStoreError && <div className="status status-error">{scedgeStoreError}</div>}
          {scedgeStoreResult && (
            <div className="data-preview">
              <div className="preview-header">
                <span className="pill">Store response</span>
                <span className="preview-id">Status {scedgeStoreResult.status}</span>
              </div>
              <pre className="code-block">{formatUnknown(scedgeStoreResult.body)}</pre>
            </div>
          )}
        </article>

        <article className="panel scedge-panel">
          <div className="panel-header">
            <h2>Purge Cache</h2>
          </div>
          <form className="operation-form" onSubmit={handleScedgePurge}>
            <label className="field">
              <span>Payload JSON</span>
              <textarea
                rows={6}
                value={scedgePurgeBody}
                onChange={(e) => setScedgePurgeBody(e.target.value)}
              />
            </label>
            <button className="secondary" type="submit" disabled={scedgePurgeLoading}>
              {scedgePurgeLoading ? 'Purging…' : 'Purge'}
            </button>
          </form>
          {scedgePurgeError && <div className="status status-error">{scedgePurgeError}</div>}
          {scedgePurgeResult && (
            <div className="data-preview">
              <div className="preview-header">
                <span className="pill">Purge response</span>
                <span className="preview-id">Status {scedgePurgeResult.status}</span>
              </div>
              <pre className="code-block">{formatUnknown(scedgePurgeResult.body)}</pre>
            </div>
          )}
        </article>
      </div>
    </section>
  );

  const upsertView = (
    <section className="single-column">
      <article className="panel operations">
        <div className="panel-header">
          <h2>Upsert Node</h2>
          <button className="refresh" onClick={() => refresh().catch(handleRefreshError)}>
            Refresh
          </button>
        </div>
        <form className="operation-form" onSubmit={handleUpsertSubmit}>
          <label className="field">
            <span>Tenant ID (optional)</span>
            <input
              value={tenantId}
              placeholder="Defaults to service tenant"
              onChange={(e) => setTenantId(e.target.value)}
            />
          </label>
          <label className="field">
            <span>Node ID</span>
            <input
              value={nodeForm.nodeId}
              placeholder="Let SynaGraph assign if empty"
              onChange={(e) => setNodeForm((prev) => ({ ...prev, nodeId: e.target.value }))}
            />
          </label>
          <label className="field">
            <span>Kind</span>
            <input
              value={nodeForm.kind}
              onChange={(e) => setNodeForm((prev) => ({ ...prev, kind: e.target.value }))}
            />
          </label>
          <label className="field">
            <span>Payload JSON</span>
            <textarea
              rows={10}
              value={nodeForm.payload}
              onChange={(e) => setNodeForm((prev) => ({ ...prev, payload: e.target.value }))}
            />
          </label>
          <label className="field">
            <span>Embedding vector (JSON array)</span>
            <textarea
              rows={4}
              value={nodeForm.embedding}
              placeholder="[0.12, 0.87, ...]"
              onChange={(e) => setNodeForm((prev) => ({ ...prev, embedding: e.target.value }))}
            />
          </label>
          <label className="field">
            <span>Provenance (JSON)</span>
            <textarea
              rows={4}
              value={nodeForm.provenance}
              placeholder='{"source": "api", "trace_id": "..."}'
              onChange={(e) => setNodeForm((prev) => ({ ...prev, provenance: e.target.value }))}
            />
          </label>
          <label className="field">
            <span>Decay λ</span>
            <input
              value={nodeForm.decayLambda}
              placeholder="e.g., 0.125"
              onChange={(e) => setNodeForm((prev) => ({ ...prev, decayLambda: e.target.value }))}
            />
          </label>
          <button className="primary" type="submit" disabled={upsertLoading}>
            {upsertLoading ? 'Upserting…' : 'Upsert Node'}
          </button>
        </form>
        {nodeResult && (
          <div className="data-preview">
            <div className="preview-header">
              <span className="pill">Node upsert</span>
              <span className="preview-id">{nodeResult.node_id}</span>
            </div>
            <pre className="code-block">{JSON.stringify(nodeResult, null, 2)}</pre>
          </div>
        )}
      </article>
    </section>
  );

  const searchView = (
    <section className="single-column">{renderSearchPanel({ context: 'search', showFilter: true })}</section>
  );

  const edgesView = (
    <section className="single-column">
      <article className="panel operations">
        <div className="panel-header">
          <h2>Relate Nodes</h2>
          <button className="refresh" onClick={() => refresh().catch(handleRefreshError)}>
            Refresh
          </button>
        </div>
        <form className="operation-form" onSubmit={handleEdgeSubmit}>
          <label className="field">
            <span>Tenant ID (optional)</span>
            <input
              value={tenantId}
              placeholder="Defaults to service tenant"
              onChange={(e) => setTenantId(e.target.value)}
            />
          </label>
          <label className="field">
            <span>From Node</span>
            <input
              value={edgeForm.fromId}
              onChange={(e) => setEdgeForm((prev) => ({ ...prev, fromId: e.target.value }))}
              placeholder="node::origin"
            />
          </label>
          <label className="field">
            <span>To Node</span>
            <input
              value={edgeForm.toId}
              onChange={(e) => setEdgeForm((prev) => ({ ...prev, toId: e.target.value }))}
              placeholder="node::target"
            />
          </label>
          <label className="field">
            <span>Edge Kind</span>
            <input
              value={edgeForm.kind}
              onChange={(e) => setEdgeForm((prev) => ({ ...prev, kind: e.target.value }))}
              placeholder="relates_to"
            />
          </label>
          <label className="field">
            <span>Weight</span>
            <input
              value={edgeForm.weight}
              onChange={(e) => setEdgeForm((prev) => ({ ...prev, weight: e.target.value }))}
              placeholder="Optional strength"
            />
          </label>
          <label className="field">
            <span>Payload (JSON)</span>
            <textarea
              rows={4}
              value={edgeForm.payload}
              onChange={(e) => setEdgeForm((prev) => ({ ...prev, payload: e.target.value }))}
              placeholder='{"role": "derived"}'
            />
          </label>
          <label className="field">
            <span>Provenance (JSON)</span>
            <textarea
              rows={4}
              value={edgeForm.provenance}
              onChange={(e) => setEdgeForm((prev) => ({ ...prev, provenance: e.target.value }))}
              placeholder='{"source": "pipeline"}'
            />
          </label>
          <button className="primary" type="submit" disabled={edgeLoading}>
            {edgeLoading ? 'Relating…' : 'Create Edge'}
          </button>
        </form>
        {edgeResult && (
          <div className="data-preview">
            <div className="preview-header">
              <span className="pill">Edge relation</span>
              <span className="preview-id">{edgeResult.edge_id}</span>
            </div>
            <pre className="code-block">{JSON.stringify(edgeResult, null, 2)}</pre>
          </div>
        )}
      </article>
    </section>
  );

  const eventsView = (
    <section className="events-grid">
      <article className={`panel ${focusedAction === 'capsule' ? 'focused' : ''}`}>
        <div className="panel-header">
          <h2>Ingest Capsule</h2>
          <button className="refresh" onClick={() => refresh().catch(handleRefreshError)}>
            Refresh
          </button>
        </div>
        <form className="operation-form" onSubmit={handleCapsuleSubmit}>
          <label className="field">
            <span>Tenant ID (optional)</span>
            <input
              value={tenantId}
              placeholder="Defaults to service tenant"
              onChange={(e) => setTenantId(e.target.value)}
            />
          </label>
          <label className="field">
            <span>Capsule JSON</span>
            <textarea
              rows={10}
              value={capsuleForm.capsule}
              onChange={(e) => setCapsuleForm((prev) => ({ ...prev, capsule: e.target.value }))}
            />
          </label>
          <label className="field field-inline">
            <span>Unwrap CCP payload</span>
            <label className="toggle">
              <input
                type="checkbox"
                checked={capsuleForm.unwrap}
                onChange={(e) => setCapsuleForm((prev) => ({ ...prev, unwrap: e.target.checked }))}
              />
              <span>{capsuleForm.unwrap ? 'Yes' : 'No'}</span>
            </label>
          </label>
          <button className="primary" type="submit" disabled={capsuleLoading}>
            {capsuleLoading ? 'Ingesting…' : 'Ingest Capsule'}
          </button>
        </form>
        {capsuleResult && (
          <div className="data-preview">
            <div className="preview-header">
              <span className="pill">Capsule ingest</span>
              <span className="preview-id">{capsuleResult.capsule_id}</span>
            </div>
            <pre className="code-block">{JSON.stringify(capsuleResult, null, 2)}</pre>
          </div>
        )}
      </article>

      <article className={`panel ${focusedAction === 'decay' ? 'focused' : ''}`}>
        <div className="panel-header">
          <h2>Decay / Reinforce</h2>
          <button className="refresh" onClick={() => refresh().catch(handleRefreshError)}>
            Refresh
          </button>
        </div>
        <form className="operation-form" onSubmit={handleDecaySubmit}>
          <label className="field">
            <span>Tenant ID (optional)</span>
            <input
              value={tenantId}
              placeholder="Defaults to service tenant"
              onChange={(e) => setTenantId(e.target.value)}
            />
          </label>
          <label className="field">
            <span>Target Node (optional)</span>
            <input
              value={decayForm.nodeId}
              placeholder="Scope to a specific node"
              onChange={(e) => setDecayForm((prev) => ({ ...prev, nodeId: e.target.value }))}
            />
          </label>
          <label className="field">
            <span>Decay λ</span>
            <input
              value={decayForm.lambda}
              placeholder="0.25"
              onChange={(e) => setDecayForm((prev) => ({ ...prev, lambda: e.target.value }))}
            />
          </label>
          <label className="field field-inline">
            <span>Reinforce instead</span>
            <label className="toggle">
              <input
                type="checkbox"
                checked={decayForm.reinforce}
                onChange={(e) => setDecayForm((prev) => ({ ...prev, reinforce: e.target.checked }))}
              />
              <span>{decayForm.reinforce ? 'Reinforce' : 'Decay'}</span>
            </label>
          </label>
          <button className="primary" type="submit" disabled={decayLoading}>
            {decayLoading ? 'Applying…' : decayForm.reinforce ? 'Reinforce Now' : 'Decay Now'}
          </button>
        </form>
        {decayResult && (
          <div className="data-preview">
            <div className="preview-header">
              <span className="pill">Decay result</span>
            </div>
            <pre className="code-block">{JSON.stringify(decayResult, null, 2)}</pre>
          </div>
        )}
      </article>

      <article className={`panel ${focusedAction === 'emit' ? 'focused' : ''}`}>
        <div className="panel-header">
          <h2>Emit Test Event</h2>
          <button className="refresh" onClick={() => refresh().catch(handleRefreshError)}>
            Refresh
          </button>
        </div>
        <form className="operation-form" onSubmit={handleEmitEventSubmit}>
          <label className="field">
            <span>Tenant ID (optional)</span>
            <input
              value={tenantId}
              placeholder="Defaults to service tenant"
              onChange={(e) => setTenantId(e.target.value)}
            />
          </label>
          <label className="field">
            <span>Event type</span>
            <input
              value={eventForm.eventType}
              onChange={(e) => setEventForm((prev) => ({ ...prev, eventType: e.target.value }))}
              placeholder="UPSERT_NODE"
            />
          </label>
          <label className="field">
            <span>Detail (JSON)</span>
            <textarea
              rows={6}
              value={eventForm.detail}
              onChange={(e) => setEventForm((prev) => ({ ...prev, detail: e.target.value }))}
            />
          </label>
          <button className="primary" type="submit" disabled={eventLoading}>
            {eventLoading ? 'Emitting…' : 'Emit Event'}
          </button>
        </form>
        {eventResult && (
          <div className="data-preview">
            <div className="preview-header">
              <span className="pill">Event</span>
              <span className="preview-id">{eventResult.event_id}</span>
            </div>
            <pre className="code-block">{JSON.stringify(eventResult, null, 2)}</pre>
          </div>
        )}
        <button className="secondary" type="button" onClick={handlePurge}>
          Request Cache Purge
        </button>
      </article>

      <article className="panel history">
        <div className="panel-header">
          <h2>Event History</h2>
          <div className="panel-actions">
            <button onClick={() => refresh().catch(handleRefreshError)}>↻</button>
            <button
              onClick={async () => {
                await clearHistory();
                await refresh({ silent: true }).catch(handleRefreshError);
              }}
            >
              🧹
            </button>
          </div>
        </div>
        <div className="history-list">
          {history.length === 0 && <div className="empty">No activity yet.</div>}
          {history.map((event) => (
            <div key={`${event.timestamp}-${event.event_type}-${event.tenant_id}`} className="history-item">
              <div className="history-meta">
                <span className={`history-tag ${eventTone(event.event_type)}`}>{event.event_type}</span>
                <span className="history-time">{formatTimestamp(event.timestamp)}</span>
              </div>
              <div className="history-meta secondary">
                <span className="meta">Tenant: {event.tenant_id || 'default'}</span>
              </div>
              <pre>{JSON.stringify(event.detail, null, 2)}</pre>
            </div>
          ))}
        </div>
      </article>
    </section>
  );

  return (
    <div className="dashboard">
      <header className="topbar">
        <div className="brand">
          <span className="brand-icon" aria-hidden="true">
            <svg className="brand-glyph" viewBox="0 0 48 48">
              <defs>
                <linearGradient id="sgGlyphGradient" x1="6" y1="6" x2="42" y2="42" gradientUnits="userSpaceOnUse">
                  <stop offset="0%" stopColor="#1ed6c5" />
                  <stop offset="55%" stopColor="#5e7efa" />
                  <stop offset="100%" stopColor="#b65cff" />
                </linearGradient>
              </defs>
              <rect x="2" y="2" width="44" height="44" rx="14" fill="url(#sgGlyphGradient)" />
              <path
                d="M16 18 L32 18 L24 30 Z"
                stroke="rgba(255, 255, 255, 0.75)"
                strokeWidth="2.4"
                fill="none"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
              <circle cx="16" cy="18" r="4" fill="#ffffff" fillOpacity="0.92" />
              <circle cx="32" cy="18" r="4" fill="#ffffff" fillOpacity="0.88" />
              <circle cx="24" cy="30" r="4" fill="#ffffff" fillOpacity="0.9" />
            </svg>
          </span>
          <div className="brand-label">
            <div className="brand-title">SynaGraph</div>
            <div className="brand-subtitle">Graph Control Plane</div>
          </div>
        </div>
        <nav className="topbar-nav" role="tablist">
          {primaryTabs.map((tab) => (
            <button
              key={tab.key}
              type="button"
              className={tab.key === activeTab ? 'nav-tab active' : 'nav-tab'}
              onClick={() => {
                setActiveTab(tab.key);
                setFocusedAction(null);
              }}
            >
              {tab.label}
            </button>
          ))}
        </nav>
        <div className="topbar-actions">
          <input
            className="search"
            placeholder="Graph search (coming soon)"
            disabled
            title="Search will land in a future release"
          />
          <button
            className="theme-toggle"
            onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
            aria-label="Toggle theme"
          >
            <span role="img" aria-hidden="true">
              {theme === 'dark' ? '🌙' : '☀️'}
            </span>
            <span>{theme === 'dark' ? 'Dark' : 'Light'} mode</span>
          </button>
        </div>
      </header>

      <main className="layout">
        {flash && <FlashBanner flash={flash} onDismiss={() => setFlash(null)} />}

        <section id="overview" className="metrics">
          {metrics.map((metric) => (
            <article key={metric.label} className={`metric-card metric-${metric.tone}`}>
              <div className="metric-label">{metric.label}</div>
              <div className="metric-value">{metric.value}</div>
            </article>
          ))}
        </section>

        {activeTab === 'graph' && graphView}
        {activeTab === 'scedge' && scedgeView}
        {activeTab === 'upsert' && upsertView}
        {activeTab === 'search' && searchView}
        {activeTab === 'edges' && edgesView}
        {activeTab === 'events' && eventsView}
      </main>
    </div>
  );
}
