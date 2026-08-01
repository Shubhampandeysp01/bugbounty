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

use crate::AppState;

use super::common::{resolve_scan_path, run_cli, truncate_output};

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

pub async fn gitleaks_scan(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<GitleaksResponse>, (StatusCode, String)> {
    // Default: scan this repo root
    let raw_path = params
        .get("path")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(".");

    let path = resolve_scan_path(&state.repo_root, raw_path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let path_str = path.to_string_lossy().to_string();

    // report-path "-" = stdout (gitleaks 8.x); exit-code 0 so leaks don't look like crashes
    let result = run_cli(
        "gitleaks",
        &[
            "detect",
            "--source",
            &path_str,
            "--report-format",
            "json",
            "--report-path",
            "-",
            "--no-banner",
            "--exit-code",
            "0",
        ],
        120,
    )
    .await;

    if !result.installed {
        return Ok(Json(GitleaksResponse {
            path: path_str,
            installed: false,
            findings: vec![],
            raw: String::new(),
            error: result.error,
            duration_ms: result.duration_ms,
            command: result.command,
        }));
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

    Ok(Json(GitleaksResponse {
        path: path_str,
        installed: true,
        findings,
        raw: truncate_output(&result.stdout, 40_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command,
    }))
}
