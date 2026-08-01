//! Recon → Crawler (katana) — DELETE this file + route + frontend registry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use super::common::{normalize_url, run_cli, truncate_output};

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

pub async fn crawl(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<KatanaResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let target = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Optional depth (default 2 keeps it fast; JS crawling enabled).
    let depth = params
        .get("depth")
        .map(|s| s.trim().to_string())
        .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or_else(|| "2".into());

    let result = run_cli(
        "katana",
        &[
            "-u",
            &target,
            "-d",
            &depth,
            "-jc",
            "-kf",
            "all",
            "-silent",
            "-timeout",
            "10",
            "-c",
            "15",
        ],
        150,
    )
    .await;

    if !result.installed {
        return Ok(Json(KatanaResponse {
            url: target,
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
                "No URLs discovered. Site may be unreachable or a JS-heavy SPA (try the JS Analysis tool)."
                    .into()
            }),
        )
    } else {
        None
    };

    Ok(Json(KatanaResponse {
        url: target,
        installed: true,
        urls,
        count,
        raw: truncate_output(&result.stdout, 40_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command,
    }))
}
