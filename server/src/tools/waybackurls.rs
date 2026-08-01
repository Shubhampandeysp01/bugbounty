//! Recon → Archive URL mine (waybackurls) — DELETE this file + route + frontend registry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use super::common::{normalize_domain, run_cli, truncate_output};

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

pub async fn archive_urls(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<WaybackResponse>, (StatusCode, String)> {
    let domain = params
        .get("domain")
        .or_else(|| params.get("url"))
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, "Missing 'domain' parameter".to_string())
        })?;
    let d = normalize_domain(domain).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let result = run_cli("waybackurls", &[&d], 120).await;

    if !result.installed {
        return Ok(Json(WaybackResponse {
            domain: d,
            installed: false,
            urls: vec![],
            count: 0,
            raw: String::new(),
            error: result.error,
            duration_ms: result.duration_ms,
            command: result.command,
        }));
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
            result.error.unwrap_or_else(|| {
                "No archived URLs for this domain (or it was never archived).".into()
            }),
        )
    } else {
        None
    };

    Ok(Json(WaybackResponse {
        domain: d,
        installed: true,
        urls,
        count,
        raw: truncate_output(&result.stdout, 40_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command,
    }))
}
