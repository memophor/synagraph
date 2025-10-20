// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Dashboard state collects metrics and history entries used by the admin UI.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Arc;
use uuid::Uuid;

use crate::repository::RepositoryBundle;

const MAX_HISTORY: usize = 200;

#[derive(Clone)]
pub struct DashboardHandle {
    inner: Arc<RwLock<DashboardData>>,
}

impl DashboardHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(DashboardData::default())),
        }
    }

    pub fn record_store(&self, tenant: Uuid, kind: &str, node_id: Uuid, created: bool) {
        let mut guard = self.inner.write();
        guard.metrics.total_stores += 1;
        guard.metrics.last_updated = Some(Utc::now());
        guard.push_history(HistoryEvent::new(
            "STORE",
            tenant,
            json!({
                "node_id": node_id,
                "kind": kind,
                "created": created,
            }),
        ));
    }

    pub fn record_lookup(&self, tenant: Uuid, node_id: Uuid, hit: bool) {
        let mut guard = self.inner.write();
        guard.metrics.total_lookups += 1;
        if hit {
            guard.metrics.cache_hits += 1;
        } else {
            guard.metrics.cache_misses += 1;
        }
        guard.metrics.last_updated = Some(Utc::now());
        guard.push_history(HistoryEvent::new(
            "LOOKUP",
            tenant,
            json!({
                "node_id": node_id,
                "hit": hit,
            }),
        ));
    }

    pub fn record_purge(&self, tenant: Uuid, detail: Value) {
        let mut guard = self.inner.write();
        guard.metrics.total_purges += 1;
        guard.metrics.last_updated = Some(Utc::now());
        guard.push_history(HistoryEvent::new("PURGE", tenant, detail));
    }

    pub fn overview(&self) -> DashboardOverview {
        let guard = self.inner.read();
        guard.metrics.compute_overview()
    }

    pub fn history(&self) -> Vec<HistoryEvent> {
        let guard = self.inner.read();
        guard.history.iter().cloned().collect()
    }

    pub fn clear_history(&self) {
        let mut guard = self.inner.write();
        guard.history.clear();
    }
}

#[derive(Default)]
struct DashboardData {
    metrics: Metrics,
    history: VecDeque<HistoryEvent>,
}

impl DashboardData {
    fn push_history(&mut self, event: HistoryEvent) {
        if self.history.len() == MAX_HISTORY {
            self.history.pop_back();
        }
        self.history.push_front(event);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub tenant_id: Uuid,
    pub detail: Value,
}

impl HistoryEvent {
    pub fn new(event_type: &str, tenant: Uuid, detail: Value) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type: event_type.to_string(),
            tenant_id: tenant,
            detail,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardOverview {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_stores: u64,
    pub total_lookups: u64,
    pub total_purges: u64,
    pub hit_rate: f64,
    pub last_updated: Option<DateTime<Utc>>,
}

#[derive(Default)]
struct Metrics {
    cache_hits: u64,
    cache_misses: u64,
    total_stores: u64,
    total_lookups: u64,
    total_purges: u64,
    last_updated: Option<DateTime<Utc>>,
}

impl Metrics {
    fn compute_overview(&self) -> DashboardOverview {
        let total = self.cache_hits + self.cache_misses;
        let hit_rate = if total == 0 {
            0.0
        } else {
            (self.cache_hits as f64 / total as f64) * 100.0
        };

        DashboardOverview {
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            total_stores: self.total_stores,
            total_lookups: self.total_lookups,
            total_purges: self.total_purges,
            hit_rate,
            last_updated: self.last_updated,
        }
    }
}

#[derive(Clone)]
pub struct AppContext {
    pub repos: RepositoryBundle,
    pub dashboard: DashboardHandle,
}

impl AppContext {
    pub fn new(repos: RepositoryBundle, dashboard: DashboardHandle) -> Self {
        Self { repos, dashboard }
    }
}
