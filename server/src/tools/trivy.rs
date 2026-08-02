//! Local Files → Vulnerability / misconfig (trivy) — DELETE this file + route + frontend registry to remove.

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
pub struct TrivyResponse {
    pub path: String,
    pub installed: bool,
    pub report: Option<Value>,
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
    // Filesystem scan: vulns + secrets + misconfig — JSON
    Ok(vec![
        "fs".into(),
        "--format".into(),
        "json".into(),
        "--quiet".into(),
        "--scanners".into(),
        "vuln,secret,misconfig".into(),
        "--severity".into(),
        "HIGH,CRITICAL".into(),
        path_str,
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
        return serde_json::to_value(TrivyResponse {
            path: path_str,
            installed: false,
            report: None,
            raw: String::new(),
            error: result.error.clone(),
            duration_ms: result.duration_ms,
            command: result.command.clone(),
        })
        .map_err(|e| e.to_string());
    }

    let report = serde_json::from_str::<Value>(&result.stdout).ok();

    let err = if report.is_none() && !result.ok {
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

    serde_json::to_value(TrivyResponse {
        path: path_str,
        installed: true,
        report,
        raw: truncate_output(&result.stdout, 80_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command.clone(),
    })
    .map_err(|e| e.to_string())
}

pub async fn trivy_scan(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(
        crate::jobs::run_sync(&state, "trivy-scan", params).await?,
    ))
}
