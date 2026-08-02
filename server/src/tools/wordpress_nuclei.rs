//! WordPress → Nuclei WP templates — DELETE file + route + registry to remove.
//! Runs nuclei with WordPress-focused tags (uses installed nuclei binary).

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
pub struct WpNucleiResponse {
    pub url: String,
    pub installed: bool,
    pub findings: Vec<Value>,
    pub raw: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
    pub note: String,
}

/// Job-Manager contract: build the CLI argument vector from tool params.
pub fn build_args(_ctx: &CliCtx, params: &HashMap<String, String>) -> Result<Vec<String>, String> {
    let target = target_from_params(params)?;

    let severity = params
        .get("severity")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("low,medium,high,critical");

    // Focused WP tags — nuclei template tags vary; wordpress is primary
    let tags = params
        .get("tags")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "wordpress".into());

    if !tags
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == ',' || c == '-' || c == '_')
    {
        return Err("Invalid tags".into());
    }

    Ok(vec![
        "-u".into(),
        target,
        "-jsonl".into(),
        "-silent".into(),
        "-no-color".into(),
        "-severity".into(),
        severity.to_string(),
        "-tags".into(),
        tags,
        "-timeout".into(),
        "10".into(),
        "-retries".into(),
        "1".into(),
        "-rate-limit".into(),
        "40".into(),
        "-no-interactsh".into(),
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
        return serde_json::to_value(WpNucleiResponse {
            url: target,
            installed: false,
            findings: vec![],
            raw: String::new(),
            error: result.error.clone(),
            duration_ms: result.duration_ms,
            command: result.command.clone(),
            note: "Install: brew install nuclei && nuclei -update-templates".into(),
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

    let err = if findings.is_empty() {
        if result.stderr.to_lowercase().contains("no templates") {
            Some("No WP templates found. Run: nuclei -update-templates".into())
        } else {
            result.error.clone()
        }
    } else {
        None
    };

    let severity = params
        .get("severity")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("low,medium,high,critical");
    let tags = params
        .get("tags")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "wordpress".into());

    serde_json::to_value(WpNucleiResponse {
        url: target,
        installed: true,
        findings,
        raw: truncate_output(&result.stdout, 50_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command.clone(),
        note: format!("tags={tags}; severity={severity}; WordPress-focused nuclei pass"),
    })
    .map_err(|e| e.to_string())
}

pub async fn wordpress_nuclei(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(
        crate::jobs::run_sync(&state, "wordpress-nuclei", params).await?,
    ))
}
