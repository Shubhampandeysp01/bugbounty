//! Local Files → Secrets (gitleaks) — DELETE this file + route + frontend registry to remove.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::jobs::CliCtx;
use crate::AppState;

use super::common::{resolve_scan_path, truncate_output, CliResult};

#[derive(Debug, Serialize)]
pub struct GitleaksResponse {
    pub path: String,
    pub installed: bool,
    pub findings: Vec<Value>,
    pub raw: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

fn scan_path(ctx: &CliCtx, params: &HashMap<String, String>) -> Result<String, String> {
    let raw_path = params
        .get("path")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(".");
    let path = resolve_scan_path(&ctx.state.repo_root, raw_path)?;
    Ok(path.to_string_lossy().to_string())
}

/// Job-Manager contract: build the CLI argument vector from tool params.
pub fn build_args(ctx: &CliCtx, params: &HashMap<String, String>) -> Result<Vec<String>, String> {
    let path_str = scan_path(ctx, params)?;
    // report-path "-" = stdout (gitleaks 8.x); exit-code 0 so leaks don't look like crashes
    Ok(vec![
        "detect".into(),
        "--source".into(),
        path_str,
        "--report-format".into(),
        "json".into(),
        "--report-path".into(),
        "-".into(),
        "--no-banner".into(),
        "--exit-code".into(),
        "0".into(),
    ])
}

/// Job-Manager contract: turn a `CliResult` into the renderer's JSON.
pub fn parse_output(
    ctx: &CliCtx,
    params: &HashMap<String, String>,
    result: &CliResult,
) -> Result<serde_json::Value, String> {
    let path_str = scan_path(ctx, params)?;

    if !result.installed {
        return serde_json::to_value(GitleaksResponse {
            path: path_str,
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
    let stdout = result.stdout.trim();
    if !stdout.is_empty() {
        if let Ok(v) = serde_json::from_str::<Value>(stdout) {
            if let Some(arr) = v.as_array() {
                findings = arr.clone();
            } else {
                findings.push(v);
            }
        } else {
            // sometimes NDJSON
            for line in stdout.lines() {
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    findings.push(v);
                }
            }
        }
    }

    let err = if !result.ok && findings.is_empty() {
        result.error.clone().or_else(|| {
            if !result.stderr.is_empty() {
                Some(truncate_output(&result.stderr, 800))
            } else {
                None
            }
        })
    } else {
        None
    };

    serde_json::to_value(GitleaksResponse {
        path: path_str,
        installed: true,
        findings,
        raw: truncate_output(&result.stdout, 40_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command.clone(),
    })
    .map_err(|e| e.to_string())
}

pub async fn gitleaks_scan(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(
        crate::jobs::run_sync(&state, "gitleaks-scan", params).await?,
    ))
}
