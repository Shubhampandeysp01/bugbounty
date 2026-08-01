//! Websites → Vuln scan (nuclei) — DELETE this file + route + frontend registry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use super::common::{normalize_url, run_cli, truncate_output};

#[derive(Debug, Serialize)]
pub struct NucleiResponse {
    pub url: String,
    pub installed: bool,
    pub findings: Vec<Value>,
    pub raw: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
    pub note: String,
}

pub async fn nuclei_scan(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<NucleiResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let target = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Optional severity filter: info,low,medium,high,critical (default medium,high,critical for speed)
    let severity = params
        .get("severity")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("medium,high,critical");

    // Optional tags e.g. cve,xss,misconfig
    let tags = params
        .get("tags")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut args: Vec<String> = vec![
        "-u".into(),
        target.clone(),
        "-jsonl".into(),
        "-silent".into(),
        "-no-color".into(),
        "-severity".into(),
        severity.to_string(),
        "-timeout".into(),
        "10".into(),
        "-retries".into(),
        "1".into(),
        "-rate-limit".into(),
        "50".into(),
    ];

    // Offline-friendly: skip interactsh callbacks by default
    args.push("-no-interactsh".into());

    if let Some(t) = tags {
        if t.chars().all(|c| c.is_ascii_alphanumeric() || c == ',' || c == '-' || c == '_') {
            args.push("-tags".into());
            args.push(t);
        }
    }

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let result = run_cli("nuclei", &arg_refs, 120).await;

    if !result.installed {
        return Ok(Json(NucleiResponse {
            url: target,
            installed: false,
            findings: vec![],
            raw: String::new(),
            error: result.error,
            duration_ms: result.duration_ms,
            command: result.command,
            note: "Install: brew install nuclei && nuclei -update-templates".into(),
        }));
    }

    let mut findings = Vec::new();
    for line in result.stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            findings.push(v);
        }
    }

    let err = if findings.is_empty() {
        // No findings is success for nuclei often — only surface real errors
        if result.stderr.to_lowercase().contains("no templates")
            || result.stderr.to_lowercase().contains("could not find template")
        {
            Some(
                "No templates found. Run: nuclei -update-templates".into(),
            )
        } else {
            result.error.filter(|_| findings.is_empty() && result.stdout.is_empty())
        }
    } else {
        None
    };

    Ok(Json(NucleiResponse {
        url: target,
        installed: true,
        findings,
        raw: truncate_output(&result.stdout, 50_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command,
        note: format!(
            "severity={severity}; no-interactsh; empty findings often means clean or templates missing"
        ),
    }))
}
