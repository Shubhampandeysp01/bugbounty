//! Recon → JS Analysis (builtin, native HTTP) — mines endpoints & secrets from
//! page <script> tags or a direct .js URL. DELETE this file + route + frontend
//! registry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::common::{http_client, normalize_url, safe_prefix, truncate_output};

const MAX_SCRIPTS: usize = 30;
const MAX_SCRIPT_BYTES: usize = 2_000_000;
const MAX_PAGE_BYTES: usize = 3_000_000;

#[derive(Debug, Serialize)]
pub struct ScriptRef {
    pub url: String,
    pub size: usize,
}

#[derive(Debug, Serialize)]
pub struct Secret {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct Counts {
    pub scripts: usize,
    pub endpoints: usize,
    pub api_endpoints: usize,
    pub secrets: usize,
}

#[derive(Debug, Serialize)]
pub struct JsAnalysisResponse {
    pub url: String,
    pub installed: bool,
    pub scripts: Vec<ScriptRef>,
    pub endpoints: Vec<String>,
    pub secrets: Vec<Secret>,
    pub counts: Counts,
    pub raw: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

/// Compile-and-cache a regex by its pattern string (consts allowed here).
fn cached_re(pattern: &'static str) -> &'static Regex {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, &'static Regex>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().unwrap();
    let entry = guard
        .entry(pattern)
        .or_insert_with(|| Box::leak(Box::new(Regex::new(pattern).expect("valid regex"))));
    entry
}

const SCRIPT_SRC_RE: &str = r#"<script[^>]*\bsrc\s*=\s*["']([^"']+)["']"#;
const FULL_URL_RE: &str = r#"(?:https?:)?//[A-Za-z0-9_.\-]+(?::\d+)?(?:/[^\s"'<>\\]*)"#;
const PATH_LITERAL_RE: &str = r#"["'`](/[A-Za-z0-9_.\-{}$]*(?:/[A-Za-z0-9_.?=&%{}:\-]+)+)["'`]"#;
const WS_URL_RE: &str = r#"wss?://[A-Za-z0-9_.\-]+(?::\d+)?(?:/[^\s"'<>\\]*)?"#;

const STATIC_EXTS: &[&str] = &[
    ".css", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".woff", ".woff2", ".ttf", ".eot",
    ".map", ".mp4", ".webp",
];

async fn fetch_text(url: &str, max_bytes: usize) -> Result<String, String> {
    let client = http_client(20)?;
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    let status = res.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let too_big = res
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .map(|n| n > max_bytes)
        .unwrap_or(false);
    if too_big {
        return Err(format!("Response too large (>{} bytes)", max_bytes));
    }
    let bytes = res
        .bytes()
        .await
        .map_err(|e| format!("Read failed: {e}"))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

/// Resolve a possibly-relative script reference against the page URL.
fn resolve_url(base: &str, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw.to_string());
    }
    if let Some(rest) = raw.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    let base = base.trim_end_matches('/');
    if raw.starts_with('/') {
        // absolute path — keep the origin
        let origin = base
            .split_once("://")
            .map(|(s, rest)| format!("{s}://{}", rest.split('/').next().unwrap_or("")))
            .unwrap_or_default();
        return Some(format!("{origin}{raw}"));
    }
    Some(format!("{base}/{raw}"))
}

fn is_static_asset(u: &str) -> bool {
    let lower = u.to_lowercase();
    STATIC_EXTS.iter().any(|e| lower.ends_with(e))
}

/// Collect unique endpoint-looking strings from JS/text.
fn scan_endpoints(text: &str) -> Vec<String> {
    let url_re = cached_re(FULL_URL_RE);
    let path_re = cached_re(PATH_LITERAL_RE);
    let ws_re = cached_re(WS_URL_RE);

    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let push = |value: &str, out: &mut Vec<String>, seen: &mut std::collections::HashSet<String>| {
        let v = value.trim().trim_end_matches(['.', ',', ';', ')', ']', '}']);
        if v.len() < 3 || v.len() > 2048 {
            return;
        }
        if is_static_asset(v) {
            return;
        }
        // Require a path-ish shape (has a slash beyond the scheme) or a query.
        let has_path = v.contains('/') || v.contains('?');
        if !has_path {
            return;
        }
        if seen.insert(v.to_string()) {
            out.push(v.to_string());
        }
    };

    for cap in ws_re.captures_iter(text) {
        push(&cap[0], &mut out, &mut seen);
    }
    for cap in url_re.captures_iter(text) {
        let m = &cap[0];
        // skip protocol-relative that is actually a host only (no path)
        push(m, &mut out, &mut seen);
    }
    for cap in path_re.captures_iter(text) {
        push(&cap[1], &mut out, &mut seen);
    }

    // URLs that resolve inside the same origin are the most useful — sort so
    // `/api/...` paths float to the top.
    out.sort_by_key(|u| {
        let api = u.contains("/api/") || u.contains("/graphql") || u.contains("/wp-json");
        (api, u.len())
    });
    out
}

fn scan_secrets(text: &str) -> Vec<Secret> {
    let patterns: &[(&str, &str)] = &[
        ("AWS access key", r"\b(AKIA[0-9A-Z]{16})\b"),
        ("AWS secret", r#"(?i)\b(aws_secret_access_key)\s*[:=]\s*['"]?([A-Za-z0-9/+=]{16,40})"#),
        ("Google API key", r"\b(AIza[0-9A-Za-z_\-]{30,})\b"),
        ("OpenAI key", r"\b(sk-[A-Za-z0-9_\-]{20,})\b"),
        ("GitHub token", r"\b(ghp_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{20,}|gho_[A-Za-z0-9]{30,})\b"),
        ("Slack token", r"\b(xox[baprs]-[A-Za-z0-9\-]{10,})\b"),
        ("JWT", r"\b(eyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,})\b"),
        ("Private key", r"(-----BEGIN [A-Z ]*PRIVATE KEY-----)"),
        ("MongoDB URI", r#"\b(mongodb(?:\+srv)?://[^\s"'<>\\]+)"#),
        ("S3 bucket", r"\b(s3://[a-z0-9.\-]+)\b"),
        (
            "Generic secret",
            r#"(?i)\b(api[_-]?key|client[_-]?secret|access[_-]?token|auth[_-]?token|refresh[_-]?token|secret)\b['"]?\s*[:=]\s*['"]([^'"]{6,80})['"]"#,
        ),
        ("Password", r#"(?i)\b(password|passwd|pwd)\b['"]?\s*[:=]\s*['"]([^'"]{6,80})['"]"#),
    ];

    let mut out: Vec<Secret> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (label, pat) in patterns {
        let re = Regex::new(pat).expect("valid secret regex");
        for cap in re.captures_iter(text) {
            // last capture group is the secret value; for single-group patterns use group 1
            let value = cap
                .get(cap.len() - 1)
                .or_else(|| cap.get(1))
                .map(|m| m.as_str())
                .unwrap_or("");
            let value = value.trim();
            if value.len() < 6 {
                continue;
            }
            let key = (*label).to_string();
            if seen.insert((key.clone(), value.to_string())) {
                out.push(Secret {
                    key,
                    // Truncate so secrets never flood the UI/logs.
                    value: safe_prefix(value, 60).to_string(),
                });
            }
        }
    }
    out
}

pub async fn js_analysis(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<JsAnalysisResponse>, (StatusCode, String)> {
    let started = std::time::Instant::now();
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let target = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // A direct .js URL skips the HTML parsing step.
    let looks_like_js = target.to_lowercase().contains(".js");

    let mut scripts: Vec<ScriptRef> = Vec::new();
    let mut js_bodies: Vec<String> = Vec::new();
    let mut error: Option<String> = None;

    if looks_like_js {
        match fetch_text(&target, MAX_SCRIPT_BYTES).await {
            Ok(body) => {
                scripts.push(ScriptRef {
                    url: target.clone(),
                    size: body.len(),
                });
                js_bodies.push(body);
            }
            Err(e) => error = Some(e),
        }
    } else {
        // 1) Fetch the page
        let html = match fetch_text(&target, MAX_PAGE_BYTES).await {
            Ok(h) => h,
            Err(e) => {
                error = Some(e);
                String::new()
            }
        };
        if !html.is_empty() {
            // 2) Collect script srcs (also scan inline HTML for endpoints/secrets)
            js_bodies.push(html.clone());
            let src_re = cached_re(SCRIPT_SRC_RE);
            let mut refs: Vec<String> = Vec::new();
            for cap in src_re.captures_iter(&html) {
                if let Some(resolved) = resolve_url(&target, &cap[1]) {
                    if !refs.contains(&resolved) {
                        refs.push(resolved);
                    }
                }
            }
            // 3) Fetch each JS file (bounded)
            for src in refs.into_iter().take(MAX_SCRIPTS) {
                match fetch_text(&src, MAX_SCRIPT_BYTES).await {
                    Ok(body) => {
                        scripts.push(ScriptRef {
                            url: src,
                            size: body.len(),
                        });
                        js_bodies.push(body);
                    }
                    Err(e) => {
                        if error.is_none() {
                            error = Some(format!("{src}: {e}"));
                        }
                    }
                }
            }
        }
    }

    // 4) Scan all collected JS (+ page HTML) for endpoints and secrets
    let mut endpoints: Vec<String> = Vec::new();
    let mut secrets: Vec<Secret> = Vec::new();
    for body in &js_bodies {
        for e in scan_endpoints(body) {
            if !endpoints.contains(&e) {
                endpoints.push(e);
            }
        }
        for s in scan_secrets(body) {
            if !secrets.iter().any(|x| x.key == s.key && x.value == s.value) {
                secrets.push(s);
            }
        }
    }
    let api_endpoints = endpoints
        .iter()
        .filter(|u| u.contains("/api/") || u.contains("/graphql") || u.contains("/wp-json"))
        .count();

    // Keep the response lean.
    endpoints.truncate(300);
    secrets.truncate(40);

    let counts = Counts {
        scripts: scripts.len(),
        endpoints: endpoints.len(),
        api_endpoints,
        secrets: secrets.len(),
    };

    Ok(Json(JsAnalysisResponse {
        url: target,
        installed: true,
        scripts,
        endpoints,
        secrets,
        counts,
        raw: truncate_output(
            &js_bodies.iter().map(|b| b.as_str()).collect::<Vec<_>>().join("\n\n"),
            25_000,
        ),
        error,
        duration_ms: started.elapsed().as_millis() as u64,
        command: "builtin (native HTTP + regex)".into(),
    }))
}
