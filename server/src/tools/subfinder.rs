//! Recon → Subdomain enum (subfinder) — DELETE this file + route + frontend registry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use super::common::{normalize_domain, run_cli, truncate_output};

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

pub async fn subdomain_enum(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<SubfinderResponse>, (StatusCode, String)> {
    let domain = params
        .get("domain")
        .or_else(|| params.get("url"))
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, "Missing 'domain' parameter".to_string())
        })?;
    let d = normalize_domain(domain).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // -all pulls every passive source (needs API keys for best results, slower).
    let use_all = params.get("all").is_some_and(|v| v == "1");

    let mut args: Vec<String> = vec![
        "-d".into(),
        d.clone(),
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
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let result = run_cli("subfinder", &arg_refs, 120).await;

    if !result.installed {
        return Ok(Json(SubfinderResponse {
            domain: d,
            installed: false,
            subdomains: vec![],
            count: 0,
            raw: String::new(),
            error: result.error,
            duration_ms: result.duration_ms,
            command: result.command,
        }));
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
            result.error.unwrap_or_else(|| {
                "No subdomains found. Add provider API keys to subfinder config for more sources."
                    .into()
            }),
        )
    } else {
        None
    };

    Ok(Json(SubfinderResponse {
        domain: d,
        installed: true,
        subdomains,
        count,
        raw: truncate_output(&result.stdout, 30_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command,
    }))
}
