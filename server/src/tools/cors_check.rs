//! Web → CORS Misconfiguration checker (builtin, native HTTP) — sends GET
//! requests with attacker-controlled Origin headers and inspects the
//! `Access-Control-Allow-Origin` / `Access-Control-Allow-Credentials` responses.
//! DELETE this file + route + frontend registry to remove.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

use super::common::{http_client, normalize_url};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct CorsTest {
    pub name: String,
    pub origin: String,
    pub allow_origin: Option<String>,
    pub allow_credentials: bool,
    pub reflects_origin: bool,
    pub verdict: String,
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct CorsResponse {
    pub url: String,
    pub installed: bool,
    pub tests: Vec<CorsTest>,
    pub high_risk: bool,
    pub medium_risk: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

fn extract_host(base_url: &str) -> String {
    base_url
        .split_once("://")
        .map(|(_, rest)| rest.split('/').next().unwrap_or(""))
        .unwrap_or("")
        .to_string()
}

fn verdict(allow_origin: Option<&str>, credentials: bool, reflects: bool, origin: &str) -> (String, String) {
    match allow_origin {
        None => ("ok".into(), "No Access-Control-Allow-Origin header — not CORS-enabled here.".into()),
        Some(ao) if ao.trim() == "*" && !credentials => (
            "low".into(),
            "Wildcard origin `*` (no credentials) — readable by any site but cookies are not sent.".to_string(),
        ),
        Some(ao) if ao.trim() == "*" && credentials => (
            "high".into(),
            "Wildcard `*` combined with Allow-Credentials — browsers reject this, but the config is broken.".to_string(),
        ),
        Some(_) if reflects && credentials => (
            "critical".into(),
            format!("Reflects origin `{origin}` AND allows credentials — attacker-controlled site can read authenticated responses."),
        ),
        Some(_) if reflects => (
            "medium".into(),
            format!("Reflects arbitrary origin `{origin}` without credentials — data readable cross-origin, but no cookie auth."),
        ),
        Some(_) => (
            "low".into(),
            "Returned an explicit allow-origin that is not attacker-controlled.".to_string(),
        ),
    }
}

async fn run_test(
    client: &Client,
    base_url: &str,
    name: &str,
    origin: &str,
) -> Result<CorsTest, String> {
    let res = client
        .get(base_url)
        .header(reqwest::header::ORIGIN, origin)
        .send()
        .await
        .map_err(|e| format!("{name}: request failed: {e}"))?;

    let allow_origin = res
        .headers()
        .get(reqwest::header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let credentials = res
        .headers()
        .get(reqwest::header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let reflects = allow_origin
        .as_deref()
        .map(|ao| ao.trim() == origin || ao.trim() == "*")
        .unwrap_or(false);

    let (verdict, note) = verdict(allow_origin.as_deref(), credentials, reflects, origin);

    Ok(CorsTest {
        name: name.to_string(),
        origin: origin.to_string(),
        allow_origin,
        allow_credentials: credentials,
        reflects_origin: reflects,
        verdict: verdict.to_string(),
        note,
    })
}

/// Job-Manager contract: the long-running work behind `cors_check`. Takes
/// params (not `Query`) and returns an axum-style error so the Job Manager and
/// the legacy endpoint share one implementation.
pub async fn cors_check_core(
    params: &HashMap<String, String>,
) -> Result<CorsResponse, (StatusCode, String)> {
    let started = std::time::Instant::now();
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let target = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let client = http_client(15).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let host = extract_host(&target);
    let domain = host
        .split('.')
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(".");
    let www = if let Some(rest) = host.strip_prefix("www.") {
        rest.to_string()
    } else {
        format!("www.{domain}")
    };

    // Origins to probe, derived from the target host.
    let cases: Vec<(String, String)> = vec![
        ("Arbitrary origin".into(), "https://evil.com".into()),
        ("Null origin".into(), "null".into()),
        ("Trusted subdomain".into(), format!("https://{www}")),
        (
            "Prefix-match bypass".into(),
            format!("https://{domain}.evil.com"),
        ),
        (
            "Suffix-match bypass".into(),
            format!("https://evil{domain}"),
        ),
        ("Scheme swap".into(), format!("http://{host}")),
    ];

    let mut tests: Vec<CorsTest> = Vec::new();
    let mut error: Option<String> = None;
    for (name, origin) in cases {
        match run_test(&client, &target, &name, &origin).await {
            Ok(t) => tests.push(t),
            Err(e) => error = Some(e),
        }
    }

    let high_risk = tests
        .iter()
        .any(|t| t.verdict == "critical" || t.verdict == "high");
    let medium_risk = !high_risk && tests.iter().any(|t| t.verdict == "medium");

    Ok(CorsResponse {
        url: target,
        installed: true,
        tests,
        high_risk,
        medium_risk,
        error,
        duration_ms: started.elapsed().as_millis() as u64,
        command: "builtin (native HTTP, 6 origin probes)".into(),
    })
}

pub async fn cors_check(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(
        crate::jobs::run_sync(&state, "cors-check", params).await?,
    ))
}
