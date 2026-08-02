//! Websites → Probe (httpx) — DELETE this file + route + frontend registry to remove.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use super::common::{normalize_url, truncate_output, CliResult};
use crate::jobs::CliCtx;
use crate::AppState;

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

/// Job-Manager contract: build the CLI argument vector from tool params.
pub fn build_args(_ctx: &CliCtx, params: &HashMap<String, String>) -> Result<Vec<String>, String> {
    let target = target_from_params(params)?;
    // httpx: status, title, tech, server — JSONL (use short flags for v1.10+)
    Ok(vec![
        "-u".into(),
        target,
        "-silent".into(),
        "-json".into(),
        "-sc".into(),
        "-title".into(),
        "-td".into(),
        "-server".into(),
        "-cl".into(),
        "-ip".into(),
        "-timeout".into(),
        "12".into(),
        "-nc".into(),
    ])
}

fn target_from_params(params: &HashMap<String, String>) -> Result<String, String> {
    let url = params
        .get("url")
        .ok_or_else(|| "Missing 'url' parameter".to_string())?;
    normalize_url(url)
}

/// Job-Manager contract: turn a `CliResult` into the renderer's JSON.
pub fn parse_output(
    _ctx: &CliCtx,
    params: &HashMap<String, String>,
    result: &CliResult,
) -> Result<serde_json::Value, String> {
    let target = target_from_params(params)?;

    if !result.installed {
        return serde_json::to_value(HttpxResponse {
            url: target,
            installed: false,
            findings: vec![],
            raw: String::new(),
            error: result.error.clone(),
            duration_ms: result.duration_ms,
            command: result.command.clone(),
        })
        .map_err(|e| e.to_string());
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

    serde_json::to_value(HttpxResponse {
        url: target,
        installed: true,
        findings,
        raw: truncate_output(&result.stdout, 20_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command.clone(),
    })
    .map_err(|e| e.to_string())
}

pub async fn httpx_probe(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(
        crate::jobs::run_sync(&state, "httpx-probe", params).await?,
    ))
}
