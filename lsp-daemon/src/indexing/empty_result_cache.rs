use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmptyRelation {
    CallHierarchy,
    References,
    Implementations,
}

#[derive(Debug, Clone)]
struct Entry {
    first_seen: Instant,
    last_seen: Instant,
    seen_count: u32,
    file_mtime_secs: u64,
}

#[derive(Debug, Default)]
pub struct EmptyResultCacheInner {
    map: HashMap<(String, EmptyRelation), Entry>,
}

#[derive(Debug, Clone)]
pub struct EmptyResultCache {
    inner: Arc<RwLock<EmptyResultCacheInner>>,
    ttl: Duration,
    min_seen: u32,
}

impl EmptyResultCache {
    pub fn new(ttl: Duration, min_seen: u32) -> Self {
        Self {
            inner: Arc::new(RwLock::new(EmptyResultCacheInner { map: HashMap::new() })),
            ttl,
            min_seen,
        }
    }

    pub fn from_env() -> Self {
        let ttl_secs = std::env::var("PROBE_LSP_EMPTY_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30 * 60);
        let min_seen = std::env::var("PROBE_LSP_EMPTY_MIN_SEEN")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(2);
        Self::new(Duration::from_secs(ttl_secs), min_seen)
    }

    /// Record an empty observation for (uid, relation) with the current file mtime.
    pub async fn record_empty(&self, uid: &str, relation: EmptyRelation, file_mtime_secs: u64) {
        let mut guard = self.inner.write().await;
        let key = (uid.to_string(), relation);
        let now = Instant::now();
        let e = guard.map.entry(key).or_insert(Entry {
            first_seen: now,
            last_seen: now,
            seen_count: 0,
            file_mtime_secs,
        });
        // Reset if file changed
        if e.file_mtime_secs != file_mtime_secs {
            *e = Entry { first_seen: now, last_seen: now, seen_count: 1, file_mtime_secs };
        } else {
            e.last_seen = now;
            e.seen_count = e.seen_count.saturating_add(1);
        }
    }

    /// Return true if emptiness is stable and within TTL.
    pub async fn should_skip(&self, uid: &str, relation: EmptyRelation, file_mtime_secs: u64) -> bool {
        self.prune_expired().await;
        let guard = self.inner.read().await;
        if let Some(e) = guard.map.get(&(uid.to_string(), relation)) {
            if e.file_mtime_secs == file_mtime_secs
                && e.seen_count >= self.min_seen
                && e.last_seen.elapsed() <= self.ttl
            {
                return true;
            }
        }
        false
    }

    /// Clear any remembered state for this uid+relation.
    pub async fn clear(&self, uid: &str, relation: EmptyRelation) {
        let mut guard = self.inner.write().await;
        guard.map.remove(&(uid.to_string(), relation));
    }

    /// Prune expired entries to keep memory bounded.
    pub async fn prune_expired(&self) {
        let mut guard = self.inner.write().await;
        let ttl = self.ttl;
        guard.map.retain(|_, e| e.last_seen.elapsed() <= ttl);
    }

    /// Return true if we've met the repeat threshold (min_seen) for this uid+relation under the same mtime.
    /// Unlike should_skip, this does not consider TTL; it answers whether the state has become "stable enough" to persist.
    pub async fn is_stable(&self, uid: &str, relation: EmptyRelation, file_mtime_secs: u64) -> bool {
        let guard = self.inner.read().await;
        if let Some(e) = guard.map.get(&(uid.to_string(), relation)) {
            return e.file_mtime_secs == file_mtime_secs && e.seen_count >= self.min_seen;
        }
        false
    }
}

