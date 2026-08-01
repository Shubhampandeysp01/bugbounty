//! Websites → Probe (httpx) — DELETE this file + route + frontend registry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use super::common::{normalize_url, run_cli, truncate_output};

#[derive(Debug, Serialize)]
pub struct HttpxResponse {
    pub url: String,
    pub installed: bool,
    pub findings: Vec<Value>,
    pub raw: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

pub async fn httpx_probe(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<HttpxResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let target = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // httpx: status, title, tech, server — JSONL (use short flags for v1.10+)
    let result = run_cli(
        "httpx",
        &[
            "-u",
            &target,
            "-silent",
            "-json",
            "-sc",
            "-title",
            "-td",
            "-server",
            "-cl",
            "-ip",
            "-timeout",
            "12",
            "-nc",
        ],
        60,
    )
    .await;

    if !result.installed {
        return Ok(Json(HttpxResponse {
            url: target,
            installed: false,
            findings: vec![],
            raw: String::new(),
            error: result.error,
            duration_ms: result.duration_ms,
            command: result.command,
        }));
    }

    let mut findings = Vec::new();
    for line in result.stdout.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            findings.push(v);
        }
    }

    // Banners on stderr are normal; only surface real failures
    let err = if findings.is_empty() {
        if let Some(e) = result.error.clone() {
            if e.contains("Timed out") || e.contains("not installed") || e.contains("Failed to spawn")
            {
                Some(e)
            } else {
                Some("No HTTP response parsed. Host may be down, blocked, or unreachable.".into())
            }
        } else {
            Some("No HTTP response parsed. Host may be down, blocked, or unreachable.".into())
        }
    } else {
        None
    };

    Ok(Json(HttpxResponse {
        url: target,
        installed: true,
        findings,
        raw: truncate_output(&result.stdout, 20_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command,
    }))
}
