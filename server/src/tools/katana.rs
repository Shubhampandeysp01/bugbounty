//! Recon → Crawler (katana) — DELETE this file + route + frontend registry to remove.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::common::{normalize_url, truncate_output, CliResult};
use crate::jobs::CliCtx;
use crate::AppState;

const MAX_URLS: usize = 3000;

#[derive(Debug, Serialize)]
pub struct KatanaResponse {
    pub url: String,
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
    let target = target_from_params(params)?;
    // Optional depth (default 2 keeps it fast; JS crawling enabled).
    let depth = params
        .get("depth")
        .map(|s| s.trim().to_string())
        .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or_else(|| "2".into());
    Ok(vec![
        "-u".into(),
        target,
        "-d".into(),
        depth,
        "-jc".into(),
        "-kf".into(),
        "all".into(),
        "-silent".into(),
        "-timeout".into(),
        "10".into(),
        "-c".into(),
        "15".into(),
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
        return serde_json::to_value(KatanaResponse {
            url: target,
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
                "No URLs discovered. Site may be unreachable or a JS-heavy SPA (try the JS Analysis tool)."
                    .into()
            }),
        )
    } else {
        None
    };

    serde_json::to_value(KatanaResponse {
        url: target,
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

pub async fn crawl(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(
        crate::jobs::run_sync(&state, "katana-crawl", params).await?,
    ))
}
