//! Shared in-memory result cache for sync tools — the aggregation layer
//! (Attack Surface Explorer) reuses these instead of re-scanning.
//!
//! Keyed by `tool|url`. Every write bumps a generation counter so the explorer
//! can detect "underlying results changed" without a background watcher.

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

fn key(tool: &str, url: &str) -> String {
    format!("{tool}|{}", url.trim().to_lowercase())
}

fn bump() {
    GEN.fetch_add(1, Ordering::Relaxed);
}

/// Monotonic counter bumped on every store — used as the explorer's cache
/// fingerprint so it recomputes only when underlying results change.
pub fn generation() -> u64 {
    GEN.load(Ordering::Relaxed)
}

pub fn get(tool: &str, url: &str) -> Option<Value> {
    let now = Instant::now();
    let mut guard = cache().write().unwrap();
    guard.retain(|_, (t, _)| now.duration_since(*t) < TTL);
    guard.get(&key(tool, url)).map(|(_, v)| v.clone())
}

pub fn store<T: Serialize>(tool: &str, url: &str, value: &T) {
    if let Ok(v) = serde_json::to_value(value) {
        cache().write().unwrap().insert(key(tool, url), (Instant::now(), v));
        bump();
    }
}

/// Store a completed job's result (job-backed tools) so the explorer can reuse
/// it even after the JobManager evicts finished jobs.
pub fn store_job(tool: &str, params: &HashMap<String, String>, value: &Value) {
    if let Some(url) = params.get("url") {
        store(tool, url, value);
    }
}
