//! Web → Open Redirect checker (builtin, native HTTP) — appends a handful of
//! known redirect parameters to the target URL with attacker-controlled values
//! and flags any 3xx response whose Location header points off-site.
//! DELETE this file + route + frontend registry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use reqwest::redirect::Policy;
use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;

use super::common::normalize_url;

const PAYLOAD: &str = "//evil.example";

const PARAMS: &[&str] = &[
    "url",
    "redirect",
    "next",
    "return",
    "returnUrl",
    "return_url",
    "dest",
    "destination",
    "target",
    "continue",
    "rurl",
    "out",
    "view",
    "to",
    "go",
    "jump",
    "u",
    "r",
];

#[derive(Debug, Serialize)]
pub struct RedirectTest {
    pub param: String,
    pub url: String,
    pub status: Option<u16>,
    pub location: Option<String>,
    pub vulnerable: bool,
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct RedirectResponse {
    pub url: String,
    pub installed: bool,
    pub tests: Vec<RedirectTest>,
    pub vulnerable: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

/// Does a Location header point somewhere attacker-controllable?
fn looks_offsite(location: &str) -> bool {
    let l = location.trim();
    l.contains("//evil.example")
        || l.contains("https://evil.example")
        || l.contains("http://evil.example")
}

pub async fn open_redirect(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<RedirectResponse>, (StatusCode, String)> {
    let started = std::time::Instant::now();
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let target = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // We must NOT follow redirects — the whole point is to read the 3xx.
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(Policy::none())
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) BugBountyVault/1.0")
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Client error: {e}")))?;

    let (base, has_query) = if target.contains('?') {
        (target.clone(), true)
    } else {
        (target.clone(), false)
    };

    let mut tests: Vec<RedirectTest> = Vec::new();
    let mut error: Option<String> = None;

    for param in PARAMS {
        let sep = if has_query { '&' } else { '?' };
        let probe_url = format!("{base}{sep}{param}={PAYLOAD}");
        match client.get(&probe_url).send().await {
            Ok(res) => {
                let status = res.status();
                let location = res
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let vuln = status.is_redirection()
                    && location.as_deref().map(looks_offsite).unwrap_or(false);
                let note = if vuln {
                    "Redirects to attacker-controlled host".into()
                } else if status.is_redirection() {
                    format!("Redirects ({status}) but Location stays on-site or is filtered")
                } else {
                    format!("HTTP {status} — no off-site redirect")
                };
                tests.push(RedirectTest {
                    param: param.to_string(),
                    url: probe_url,
                    status: Some(status.as_u16()),
                    location,
                    vulnerable: vuln,
                    note,
                });
            }
            Err(e) => {
                error = Some(format!("{param}: {e}"));
                tests.push(RedirectTest {
                    param: param.to_string(),
                    url: probe_url,
                    status: None,
                    location: None,
                    vulnerable: false,
                    note: "Request failed".into(),
                });
            }
        }
    }

    let vulnerable = tests.iter().any(|t| t.vulnerable);

    Ok(Json(RedirectResponse {
        url: target,
        installed: true,
        tests,
        vulnerable,
        error,
        duration_ms: started.elapsed().as_millis() as u64,
        command: format!("builtin (native HTTP, {} redirect params probed)", PARAMS.len()),
    }))
}
