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

use crate::AppState;

use super::common::{resolve_scan_path, run_cli, truncate_output};

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

pub async fn trivy_scan(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<TrivyResponse>, (StatusCode, String)> {
    let raw_path = params
        .get("path")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(".");

    let path = resolve_scan_path(&state.repo_root, raw_path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let path_str = path.to_string_lossy().to_string();

    // Filesystem scan: vulns + secrets + misconfig — JSON
    let result = run_cli(
        "trivy",
        &[
            "fs",
            "--format",
            "json",
            "--quiet",
            "--scanners",
            "vuln,secret,misconfig",
            "--severity",
            "HIGH,CRITICAL",
            &path_str,
        ],
        180,
    )
    .await;

    if !result.installed {
        return Ok(Json(TrivyResponse {
            path: path_str,
            installed: false,
            report: None,
            raw: String::new(),
            error: result.error,
            duration_ms: result.duration_ms,
            command: result.command,
        }));
    }

    let report = serde_json::from_str::<Value>(&result.stdout).ok();

    let err = if report.is_none() && !result.ok {
        result
            .error
            .or_else(|| {
                if !result.stderr.is_empty() {
                    Some(truncate_output(&result.stderr, 800))
                } else {
                    None
                }
            })
    } else {
        None
    };

    Ok(Json(TrivyResponse {
        path: path_str,
        installed: true,
        report,
        raw: truncate_output(&result.stdout, 80_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command,
    }))
}
