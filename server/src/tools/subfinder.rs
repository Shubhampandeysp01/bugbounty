//! Recon → Subdomain enum (subfinder) — DELETE this file + route + frontend registry to remove.

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

#[derive(Debug, Serialize)]
pub struct SubfinderResponse {
    pub domain: String,
    pub installed: bool,
    pub subdomains: Vec<String>,
    pub count: usize,
    pub raw: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

/// Job-Manager contract: build the CLI argument vector from tool params.
/// Validation failures return `Err` (→ HTTP 400 on legacy endpoints).
pub fn build_args(_ctx: &CliCtx, params: &HashMap<String, String>) -> Result<Vec<String>, String> {
    let domain = domain_from_params(params)?;
    // -all pulls every passive source (needs API keys for best results, slower).
    let use_all = params.get("all").is_some_and(|v| v == "1");
    let mut args: Vec<String> = vec![
        "-d".into(),
        domain,
        "-silent".into(),
        // Cap per-source requests and overall runtime so a slow/broken source
        // doesn't make the API hang for minutes.
        "-timeout".into(),
        "10".into(),
        "-max-time".into(),
        "1".into(),
    ];
    if use_all {
        args.push("-all".into());
    }
    Ok(args)
}

fn domain_from_params(params: &HashMap<String, String>) -> Result<String, String> {
    let domain = params
        .get("domain")
        .or_else(|| params.get("url"))
        .ok_or_else(|| "Missing 'domain' parameter".to_string())?;
    normalize_domain(domain)
}

/// Job-Manager contract: turn a `CliResult` into the renderer's JSON. Must
/// stay byte-identical to the legacy handler response so renderers are
/// untouched.
pub fn parse_output(
    _ctx: &CliCtx,
    params: &HashMap<String, String>,
    result: &CliResult,
) -> Result<serde_json::Value, String> {
    let d = domain_from_params(params)?;

    if !result.installed {
        return serde_json::to_value(SubfinderResponse {
            domain: d,
            installed: false,
            subdomains: vec![],
            count: 0,
            raw: String::new(),
            error: result.error.clone(),
            duration_ms: result.duration_ms,
            command: result.command.clone(),
        })
        .map_err(|e| e.to_string());
    }

    let mut seen = HashSet::new();
    let mut subdomains: Vec<String> = result
        .stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| seen.insert(l.to_string()))
        .map(|l| l.to_string())
        .collect();
    subdomains.sort();
    let count = subdomains.len();

    let err = if subdomains.is_empty() {
        Some(
            result.error.clone().unwrap_or_else(|| {
                "No subdomains found. Add provider API keys to subfinder config for more sources."
                    .into()
            }),
        )
    } else {
        None
    };

    serde_json::to_value(SubfinderResponse {
        domain: d,
        installed: true,
        subdomains,
        count,
        raw: truncate_output(&result.stdout, 30_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command.clone(),
    })
    .map_err(|e| e.to_string())
}

pub async fn subdomain_enum(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(
        crate::jobs::run_sync(&state, "subfinder-enum", params).await?,
    ))
}
