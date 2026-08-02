//! Recon → Archive URL mine (waybackurls) — DELETE this file + route + frontend registry to remove.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::common::{normalize_domain, truncate_output, CliResult};
use crate::jobs::CliCtx;
use crate::AppState;

const MAX_URLS: usize = 4000;

#[derive(Debug, Serialize)]
pub struct WaybackResponse {
    pub domain: String,
    pub installed: bool,
    pub urls: Vec<String>,
    pub count: usize,
    pub raw: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

/// Job-Manager contract: build the CLI argument vector from tool params.
pub fn build_args(_ctx: &CliCtx, params: &HashMap<String, String>) -> Result<Vec<String>, String> {
    Ok(vec![domain_from_params(params)?])
}

fn domain_from_params(params: &HashMap<String, String>) -> Result<String, String> {
    let domain = params
        .get("domain")
        .or_else(|| params.get("url"))
        .ok_or_else(|| "Missing 'domain' parameter".to_string())?;
    normalize_domain(domain)
}

/// Job-Manager contract: turn a `CliResult` into the renderer's JSON.
pub fn parse_output(
    _ctx: &CliCtx,
    params: &HashMap<String, String>,
    result: &CliResult,
) -> Result<serde_json::Value, String> {
    let d = domain_from_params(params)?;

    if !result.installed {
        return serde_json::to_value(WaybackResponse {
            domain: d,
            installed: false,
            urls: vec![],
            count: 0,
            raw: String::new(),
            error: result.error.clone(),
            duration_ms: result.duration_ms,
            command: result.command.clone(),
        })
        .map_err(|e| e.to_string());
    }

    let mut seen = HashSet::new();
    let mut urls: Vec<String> = result
        .stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| seen.insert(l.to_string()))
        .map(|l| l.to_string())
        .collect();
    urls.sort();
    let count = urls.len();
    if urls.len() > MAX_URLS {
        urls.truncate(MAX_URLS);
    }

    let err = if count == 0 {
        Some(
            result.error.clone().unwrap_or_else(|| {
                "No archived URLs for this domain (or it was never archived).".into()
            }),
        )
    } else {
        None
    };

    serde_json::to_value(WaybackResponse {
        domain: d,
        installed: true,
        urls,
        count,
        raw: truncate_output(&result.stdout, 40_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command.clone(),
    })
    .map_err(|e| e.to_string())
}

pub async fn archive_urls(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(
        crate::jobs::run_sync(&state, "waybackurls-mine", params).await?,
    ))
}
