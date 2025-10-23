use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpKind {
    CallHierarchy,
    References,
    Implementations,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpKey {
    pub uid: String,
    pub kind: OpKind,
}

#[derive(Debug, Default)]
struct Bucket {
    timestamps: Vec<Instant>,
}

#[derive(Debug, Clone)]
pub struct AnomalyGuard {
    inner: Arc<RwLock<HashMap<OpKey, Bucket>>>,
    window: Duration,
    threshold: usize,
}

impl AnomalyGuard {
    pub fn from_env() -> Self {
        let window_secs = std::env::var("PROBE_LSP_ANOMALY_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120);
        let threshold = std::env::var("PROBE_LSP_ANOMALY_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(8);
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            window: Duration::from_secs(window_secs),
            threshold,
        }
    }

    /// Record a zero-result outcome for the given op key, return true if the op
    /// should be quarantined (exceeded threshold within the sliding window).
    pub async fn record_zero_and_check(&self, key: OpKey) -> bool {
        let now = Instant::now();
        {
            // Prune old timestamps and push new
            let mut guard = self.inner.write().await;
            let bucket = guard.entry(key).or_default();
            bucket
                .timestamps
                .retain(|t| now.duration_since(*t) <= self.window);
            bucket.timestamps.push(now);
            if bucket.timestamps.len() >= self.threshold {
                return true;
            }
        }
        false
    }

    pub async fn counts(&self) -> usize {
        let guard = self.inner.read().await;
        guard.len()
    }
}
