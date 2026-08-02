//! Shared in-memory result cache for sync tools — the aggregation layer
//! (Attack Surface Explorer) reuses these instead of re-scanning.
//!
//! Keyed by `tool|target` (url / domain / path). Every write bumps a generation
//! counter so the explorer can detect "underlying results changed" without a
//! background watcher.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

/// How long a cached tool result is considered fresh (session-sized).
const TTL: Duration = Duration::from_secs(30 * 60);

static GEN: AtomicU64 = AtomicU64::new(0);

fn cache() -> &'static RwLock<HashMap<String, (Instant, Value)>> {
    static CACHE: OnceLock<RwLock<HashMap<String, (Instant, Value)>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn key(tool: &str, target: &str) -> String {
    format!("{tool}|{}", target.trim().to_lowercase())
}

fn bump() {
    GEN.fetch_add(1, Ordering::Relaxed);
}

/// Monotonic counter bumped on every store — used as the explorer's cache
/// fingerprint so it recomputes only when underlying results change.
pub fn generation() -> u64 {
    GEN.load(Ordering::Relaxed)
}

/// Read-only lookup. Expired entries are dropped lazily on write paths
/// (and opportunistically here under a write lock only when the hit is stale).
pub fn get(tool: &str, url: &str) -> Option<Value> {
    let k = key(tool, url);
    let now = Instant::now();
    {
        let guard = cache().read().unwrap();
        if let Some((t, v)) = guard.get(&k) {
            if now.duration_since(*t) < TTL {
                return Some(v.clone());
            }
        } else {
            return None;
        }
    }
    // Stale hit — drop under write lock.
    let mut guard = cache().write().unwrap();
    if let Some((t, _)) = guard.get(&k) {
        if now.duration_since(*t) >= TTL {
            guard.remove(&k);
        } else if let Some((_, v)) = guard.get(&k) {
            return Some(v.clone());
        }
    }
    None
}

pub fn store<T: Serialize>(tool: &str, url: &str, value: &T) {
    if let Ok(v) = serde_json::to_value(value) {
        let mut guard = cache().write().unwrap();
        // Opportunistic TTL sweep so the map cannot grow without bound.
        let now = Instant::now();
        if guard.len() > 256 {
            guard.retain(|_, (t, _)| now.duration_since(*t) < TTL);
        }
        guard.insert(key(tool, url), (now, v));
        drop(guard);
        bump();
    }
}

/// Store a completed job's result (job-backed tools) so the explorer can reuse
/// it even after the JobManager evicts finished jobs.
/// Prefers `url`, then `domain`, then `path` as the cache target key.
pub fn store_job(tool: &str, params: &HashMap<String, String>, value: &Value) {
    let target = params
        .get("url")
        .or_else(|| params.get("domain"))
        .or_else(|| params.get("path"));
    if let Some(target) = target {
        store(tool, target, value);
    }
}
