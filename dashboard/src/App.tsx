import { FormEvent, useEffect, useMemo, useState } from 'react';
import {
  StoreRequest,
  fetchOverview,
  fetchHistory,
  clearHistory,
  storeNode,
  lookupNode,
  purgeArtifacts,
  OverviewResponse,
  HistoryEvent,
} from './api';

const DEFAULT_PAYLOAD = `{
  "title": "Hello, world!",
  "summary": "Sample payload for SynaGraph",
  "tags": ["demo", "sample"]
}`;

function formatTimestamp(ts: string) {
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return ts;
  }
}

const tabs = ['Store', 'Lookup', 'Purge'] as const;

type TabKey = (typeof tabs)[number];

export default function App() {
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    return (localStorage.getItem('synagraph-theme') as 'light' | 'dark') || 'light';
  });
  const [overview, setOverview] = useState<OverviewResponse | null>(null);
  const [history, setHistory] = useState<HistoryEvent[]>([]);
  const [activeTab, setActiveTab] = useState<TabKey>('Store');
  const [storeForm, setStoreForm] = useState<StoreRequest>({ kind: 'note', payload: DEFAULT_PAYLOAD });
  const [lookupId, setLookupId] = useState('');
  const [purgeReason, setPurgeReason] = useState('');
  const [tenantId, setTenantId] = useState<string>('');
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [statusTone, setStatusTone] = useState<'success' | 'error'>('success');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    document.body.dataset.theme = theme === 'dark' ? 'dark' : '';
    localStorage.setItem('synagraph-theme', theme);
  }, [theme]);

  const refresh = async () => {
    try {
      const [ov, hist] = await Promise.all([fetchOverview(), fetchHistory()]);
      setOverview(ov);
      setHistory(hist);
    } catch (err) {
      console.error(err);
      setStatusTone('error');
      setStatusMessage('Failed to load dashboard data');
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const handleStore = async (e: FormEvent) => {
    e.preventDefault();
    try {
      setLoading(true);
      let payload: unknown = storeForm.payload;
      if (typeof payload === 'string') {
        try {
          payload = JSON.parse(payload);
        } catch (parseError) {
          throw new Error('Invalid JSON payload');
        }
      }
      const res = await storeNode({ ...storeForm, payload, tenantId: tenantId || undefined });
      setStatusTone('success');
      setStatusMessage(res.created ? 'Node created' : 'Node updated');
      setStoreForm((prev) => ({ ...prev, nodeId: res.node_id }));
      await refresh();
    } catch (err) {
      console.error(err);
      setStatusTone('error');
      setStatusMessage(err instanceof Error ? err.message : 'Store operation failed');
    } finally {
      setLoading(false);
    }
  };

  const handleLookup = async (e: FormEvent) => {
    e.preventDefault();
    if (!lookupId) {
      setStatusTone('error');
      setStatusMessage('Provide a node ID to lookup');
      return;
    }
    try {
      setLoading(true);
      const res = await lookupNode({ tenantId: tenantId || undefined, nodeId: lookupId });
      setStatusTone(res.found ? 'success' : 'error');
      setStatusMessage(res.found ? 'Node retrieved' : 'Node not found');
      await refresh();
    } catch (err) {
      console.error(err);
      setStatusTone('error');
      setStatusMessage(err instanceof Error ? err.message : 'Lookup failed');
    } finally {
      setLoading(false);
    }
  };

  const handlePurge = async (e: FormEvent) => {
    e.preventDefault();
    try {
      setLoading(true);
      await purgeArtifacts(tenantId || undefined, purgeReason || undefined);
      setStatusTone('success');
      setStatusMessage('Purge request submitted');
      await refresh();
    } catch (err) {
      console.error(err);
      setStatusTone('error');
      setStatusMessage(err instanceof Error ? err.message : 'Purge failed');
    } finally {
      setLoading(false);
    }
  };

  const metrics = useMemo(() => {
    if (!overview) {
      return [
        { label: 'Cache Hits', value: '-', tone: 'positive' as const },
        { label: 'Cache Misses', value: '-', tone: 'negative' as const },
        { label: 'Total Stores', value: '-', tone: 'neutral' as const },
        { label: 'Hit Rate', value: '-', tone: 'neutral' as const },
      ];
    }
    return [
      { label: 'Cache Hits', value: overview.cache_hits.toString(), tone: 'positive' as const },
      { label: 'Cache Misses', value: overview.cache_misses.toString(), tone: 'negative' as const },
      { label: 'Total Stores', value: overview.total_stores.toString(), tone: 'neutral' as const },
      { label: 'Hit Rate', value: `${overview.hit_rate.toFixed(1)}%`, tone: 'neutral' as const },
    ];
  }, [overview]);

  return (
    <div className="dashboard">
      <header className="topbar">
        <div className="brand">
          <span className="brand-icon">⚡</span>
          <div>
            <div className="brand-title">SynaGraph</div>
            <div className="brand-subtitle">Knowledge Graph Operations</div>
          </div>
        </div>
        <div className="topbar-actions">
          <input
            className="search"
            placeholder="Search soon…"
            disabled
            title="Search will land in a future release"
          />
          <button
            className="theme-toggle"
            onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
            aria-label="Toggle theme"
          >
            {theme === 'dark' ? '🌙' : '☀️'}
          </button>
        </div>
      </header>

      <main className="layout">
        <section className="metrics">
          {metrics.map((metric) => (
            <article key={metric.label} className={`metric-card metric-${metric.tone}`}>
              <div className="metric-label">{metric.label}</div>
              <div className="metric-value">{metric.value}</div>
            </article>
          ))}
        </section>

        <section className="content-grid">
          <article className="panel operations">
            <div className="panel-header">
              <h2>Operations</h2>
              <button className="refresh" onClick={refresh}>Refresh</button>
            </div>
            <div className="tabs">
              {tabs.map((tab) => (
                <button
                  key={tab}
                  className={tab === activeTab ? 'tab active' : 'tab'}
                  onClick={() => setActiveTab(tab)}
                >
                  {tab}
                </button>
              ))}
            </div>

            <form
              className="operation-form"
              onSubmit={
                activeTab === 'Store'
                  ? handleStore
                  : activeTab === 'Lookup'
                  ? handleLookup
                  : handlePurge
              }
            >
              <label className="field">
                <span>Tenant ID (optional)</span>
                <input
                  value={tenantId}
                  placeholder="Defaults to service tenant"
                  onChange={(e) => setTenantId(e.target.value)}
                />
              </label>

              {activeTab === 'Store' && (
                <>
                  <label className="field">
                    <span>Kind</span>
                    <input
                      value={storeForm.kind}
                      onChange={(e) => setStoreForm((prev) => ({ ...prev, kind: e.target.value }))}
                    />
                  </label>
                  <label className="field">
                    <span>Node ID (optional)</span>
                    <input
                      value={storeForm.nodeId ?? ''}
                      onChange={(e) =>
                        setStoreForm((prev) => ({ ...prev, nodeId: e.target.value || undefined }))
                      }
                    />
                  </label>
                  <label className="field">
                    <span>Payload JSON</span>
                    <textarea
                      rows={10}
                      value={
                        typeof storeForm.payload === 'string'
                          ? storeForm.payload
                          : JSON.stringify(storeForm.payload, null, 2)
                      }
                      onChange={(e) => setStoreForm((prev) => ({ ...prev, payload: e.target.value }))}
                    />
                  </label>
                </>
              )}

              {activeTab === 'Lookup' && (
                <label className="field">
                  <span>Node ID</span>
                  <input
                    value={lookupId}
                    onChange={(e) => setLookupId(e.target.value)}
                    required
                  />
                </label>
              )}

              {activeTab === 'Purge' && (
                <label className="field">
                  <span>Reason</span>
                  <input
                    value={purgeReason}
                    onChange={(e) => setPurgeReason(e.target.value)}
                    placeholder="e.g., superseded"
                  />
                </label>
              )}

              <button className="primary" type="submit" disabled={loading}>
                {loading ? 'Working…' : activeTab === 'Store' ? 'Store Artifact' : activeTab === 'Lookup' ? 'Lookup Artifact' : 'Purge Cache'}
              </button>
            </form>

            {statusMessage && (
              <div className={`status status-${statusTone}`}>{statusMessage}</div>
            )}
          </article>

          <aside className="sidebar">
            <article className="panel health">
              <h2>System Health</h2>
              <div className="health-status">
                <span className="dot" />
                <span>Healthy</span>
              </div>
              {overview?.last_updated && (
                <div className="meta">Updated {formatTimestamp(overview.last_updated)}</div>
              )}
            </article>

            <article className="panel history">
              <div className="panel-header">
                <h2>History</h2>
                <div className="panel-actions">
                  <button onClick={refresh}>↻</button>
                  <button onClick={async () => { await clearHistory(); await refresh(); }}>🧹</button>
                </div>
              </div>
              <div className="history-list">
                {history.length === 0 && <div className="empty">No activity yet.</div>}
                {history.map((event) => (
                  <div key={`${event.timestamp}-${event.event_type}`} className="history-item">
                    <div className="history-meta">
                      <span className="history-tag">{event.event_type}</span>
                      <span className="history-time">{formatTimestamp(event.timestamp)}</span>
                    </div>
                    <pre>{JSON.stringify(event.detail, null, 2)}</pre>
                  </div>
                ))}
              </div>
            </article>
          </aside>
        </section>
      </main>
    </div>
  );
}
