//! WordPress → Nuclei WP templates — DELETE file + route + registry to remove.
//! Runs nuclei with WordPress-focused tags (uses installed nuclei binary).

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use super::common::{normalize_url, run_cli, truncate_output};

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

pub async fn wordpress_nuclei(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<WpNucleiResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let target = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

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
        return Err((StatusCode::BAD_REQUEST, "Invalid tags".into()));
    }

    let args = [
        "-u",
        &target,
        "-jsonl",
        "-silent",
        "-no-color",
        "-severity",
        severity,
        "-tags",
        &tags,
        "-timeout",
        "10",
        "-retries",
        "1",
        "-rate-limit",
        "40",
        "-no-interactsh",
    ];

    let result = run_cli("nuclei", &args, 150).await;

    if !result.installed {
        return Ok(Json(WpNucleiResponse {
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
            result.error
        }
    } else {
        None
    };

    Ok(Json(WpNucleiResponse {
        url: target,
        installed: true,
        findings,
        raw: truncate_output(&result.stdout, 50_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command,
        note: format!(
            "tags={tags}; severity={severity}; WordPress-focused nuclei pass"
        ),
    }))
}
