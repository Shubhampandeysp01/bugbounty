//! Web → Security Headers & Cookies (builtin, native HTTP).
//! Probes response headers and Set-Cookie flags; flags missing hardening.
//! DELETE this file + route + frontend registry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

use super::common::{http_client, normalize_url, safe_prefix};
use super::result_cache;

#[derive(Debug, Clone, Serialize)]
pub struct HeaderCheck {
    pub name: String,
    pub present: bool,
    pub value: Option<String>,
    pub severity: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CookieCheck {
    pub name: String,
    pub raw: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub severity: String,
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct SecurityHeadersResponse {
    pub url: String,
    pub installed: bool,
    pub status: Option<u16>,
    pub final_url: Option<String>,
    pub headers: Vec<HeaderCheck>,
    pub cookies: Vec<CookieCheck>,
    pub missing_count: usize,
    pub weak_cookie_count: usize,
    pub overall: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

/// (header name lowercased, display name, severity when missing, guidance)
const SECURITY_HEADERS: &[(&str, &str, &str, &str)] = &[
    (
        "strict-transport-security",
        "Strict-Transport-Security",
        "medium",
        "Missing HSTS — browsers may allow SSL stripping / downgrade on first visit.",
    ),
    (
        "content-security-policy",
        "Content-Security-Policy",
        "medium",
        "Missing CSP — weaker XSS mitigation; consider a baseline policy.",
    ),
    (
        "x-content-type-options",
        "X-Content-Type-Options",
        "medium",
        "Missing X-Content-Type-Options — set to nosniff to block MIME sniffing.",
    ),
    (
        "x-frame-options",
        "X-Frame-Options",
        "low",
        "Missing X-Frame-Options (and no frame-ancestors in CSP) — clickjacking risk.",
    ),
    (
        "referrer-policy",
        "Referrer-Policy",
        "low",
        "Missing Referrer-Policy — full URLs may leak to third parties via Referer.",
    ),
    (
        "permissions-policy",
        "Permissions-Policy",
        "low",
        "Missing Permissions-Policy — browser features (camera, geolocation, …) not restricted.",
    ),
    (
        "cross-origin-opener-policy",
        "Cross-Origin-Opener-Policy",
        "info",
        "Missing COOP — optional isolation header for XS-Leak / Spectre hardening.",
    ),
    (
        "cross-origin-resource-policy",
        "Cross-Origin-Resource-Policy",
        "info",
        "Missing CORP — optional resource isolation header.",
    ),
];

fn header_get(map: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    map.get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn evaluate_headers(map: &reqwest::header::HeaderMap, is_https: bool) -> Vec<HeaderCheck> {
    let csp = header_get(map, "content-security-policy");
    let has_frame_ancestors = csp
        .as_deref()
        .map(|c| c.to_lowercase().contains("frame-ancestors"))
        .unwrap_or(false);

    let mut out = Vec::new();
    for &(key, display, miss_sev, miss_note) in SECURITY_HEADERS {
        let value = header_get(map, key);
        let present = value.is_some();

        // XFO is ok if CSP frame-ancestors is set.
        if key == "x-frame-options" && !present && has_frame_ancestors {
            out.push(HeaderCheck {
                name: display.into(),
                present: false,
                value: None,
                severity: "info".into(),
                note: "X-Frame-Options missing, but CSP frame-ancestors is present.".into(),
            });
            continue;
        }

        // HSTS only meaningful on HTTPS responses.
        if key == "strict-transport-security" && !is_https {
            out.push(HeaderCheck {
                name: display.into(),
                present,
                value: value.clone().map(|v| safe_prefix(&v, 200).to_string()),
                severity: "info".into(),
                note: if present {
                    "HSTS present (target is not HTTPS in this probe — unusual).".into()
                } else {
                    "HSTS not checked as a hard fail on plain HTTP targets.".into()
                },
            });
            continue;
        }

        if present {
            let val = value.unwrap_or_default();
            let (severity, note) = assess_present_header(key, &val);
            out.push(HeaderCheck {
                name: display.into(),
                present: true,
                value: Some(safe_prefix(&val, 200).to_string()),
                severity: severity.into(),
                note: note.into(),
            });
        } else {
            out.push(HeaderCheck {
                name: display.into(),
                present: false,
                value: None,
                severity: miss_sev.into(),
                note: miss_note.into(),
            });
        }
    }
    out
}

fn assess_present_header(key: &str, value: &str) -> (&'static str, String) {
    let lower = value.to_lowercase();
    match key {
        "strict-transport-security" => {
            if !lower.contains("max-age=") {
                return (
                    "low",
                    "HSTS present but max-age is missing or unreadable.".into(),
                );
            }
            let max_age = lower
                .split(';')
                .find_map(|p| {
                    let p = p.trim();
                    p.strip_prefix("max-age=")
                        .and_then(|n| n.trim().parse::<u64>().ok())
                })
                .unwrap_or(0);
            if max_age < 15_552_000 {
                // < ~180 days
                (
                    "low",
                    format!("HSTS max-age={max_age} is short; consider ≥15552000 (180 days)."),
                )
            } else {
                ("info", format!("HSTS present (max-age={max_age})."))
            }
        }
        "x-content-type-options" => {
            if lower.trim() == "nosniff" {
                ("info", "nosniff set.".into())
            } else {
                (
                    "low",
                    format!("Unexpected value `{value}` — expected nosniff."),
                )
            }
        }
        "x-frame-options" => {
            let v = lower.trim();
            if v == "deny" || v == "sameorigin" || v.starts_with("allow-from") {
                ("info", format!("Set to {value}."))
            } else {
                ("low", format!("Unusual X-Frame-Options value: {value}"))
            }
        }
        "content-security-policy" => {
            if lower.contains("unsafe-inline") || lower.contains("unsafe-eval") {
                (
                    "low",
                    "CSP present but allows unsafe-inline and/or unsafe-eval.".into(),
                )
            } else {
                ("info", "CSP present.".into())
            }
        }
        _ => ("info", format!("Present: {}", safe_prefix(value, 80))),
    }
}

fn parse_cookies(map: &reqwest::header::HeaderMap, is_https: bool) -> Vec<CookieCheck> {
    let mut out = Vec::new();
    for val in map.get_all(reqwest::header::SET_COOKIE) {
        let Ok(raw) = val.to_str() else { continue };
        let raw = raw.to_string();
        let parts: Vec<&str> = raw.split(';').map(str::trim).collect();
        let name = parts
            .first()
            .and_then(|p| p.split('=').next())
            .unwrap_or("?")
            .trim()
            .to_string();
        let mut secure = false;
        let mut http_only = false;
        let mut same_site: Option<String> = None;
        for p in parts.iter().skip(1) {
            let lower = p.to_lowercase();
            if lower == "secure" {
                secure = true;
            } else if lower == "httponly" {
                http_only = true;
            } else if let Some(rest) = lower.strip_prefix("samesite=") {
                same_site = Some(rest.trim().to_string());
            }
        }

        let mut issues = Vec::new();
        let mut sev = "info";
        if is_https && !secure {
            issues.push("missing Secure");
            sev = "medium";
        }
        if !http_only {
            issues.push("missing HttpOnly");
            if sev == "info" {
                sev = "low";
            }
        }
        match same_site.as_deref() {
            None => {
                issues.push("missing SameSite");
                if sev == "info" {
                    sev = "low";
                }
            }
            Some(s) if s.eq_ignore_ascii_case("none") && !secure => {
                issues.push("SameSite=None without Secure");
                sev = "medium";
            }
            _ => {}
        }

        let note = if issues.is_empty() {
            "Secure + HttpOnly + SameSite look good.".into()
        } else {
            format!("Weak flags: {}.", issues.join(", "))
        };

        out.push(CookieCheck {
            name,
            raw: safe_prefix(&raw, 240).to_string(),
            secure,
            http_only,
            same_site,
            severity: sev.into(),
            note,
        });
    }
    out
}

fn overall_severity(headers: &[HeaderCheck], cookies: &[CookieCheck]) -> String {
    let mut rank = 0u8;
    for h in headers {
        rank = rank.max(sev_rank(&h.severity));
    }
    for c in cookies {
        rank = rank.max(sev_rank(&c.severity));
    }
    match rank {
        5 => "critical",
        4 => "high",
        3 => "medium",
        2 => "low",
        _ => "info",
    }
    .into()
}

fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 5,
        "high" => 4,
        "medium" => 3,
        "low" => 2,
        _ => 1,
    }
}

pub async fn security_headers(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<SecurityHeadersResponse>, (StatusCode, String)> {
    let started = Instant::now();
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let target = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let is_https = target.starts_with("https://");

    let client = http_client(20).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let (status, final_url, header_map, error) = match client.get(&target).send().await {
        Ok(res) => {
            let status = res.status().as_u16();
            let final_url = res.url().to_string();
            let headers = res.headers().clone();
            // Drain body so the connection can be reused cleanly; we only need headers.
            let _ = res.bytes().await;
            (Some(status), Some(final_url), Some(headers), None)
        }
        Err(e) => (None, None, None, Some(format!("Request failed: {e}"))),
    };

    let headers = header_map
        .as_ref()
        .map(|m| evaluate_headers(m, is_https))
        .unwrap_or_default();
    let cookies = header_map
        .as_ref()
        .map(|m| parse_cookies(m, is_https))
        .unwrap_or_default();

    let missing_count = headers.iter().filter(|h| !h.present && h.severity != "info").count();
    let weak_cookie_count = cookies
        .iter()
        .filter(|c| c.severity != "info")
        .count();
    let overall = if error.is_some() {
        "error".into()
    } else {
        overall_severity(&headers, &cookies)
    };

    let resp = SecurityHeadersResponse {
        url: target.clone(),
        installed: true,
        status,
        final_url,
        headers,
        cookies,
        missing_count,
        weak_cookie_count,
        overall,
        error,
        duration_ms: started.elapsed().as_millis() as u64,
        command: "builtin (native HTTP header + Set-Cookie probe)".into(),
    };

    result_cache::store("security-headers", &target, &resp);
    Ok(Json(resp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

    fn map_from(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.append(
                HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        m
    }

    #[test]
    fn missing_headers_flagged() {
        let m = map_from(&[("server", "nginx")]);
        let checks = evaluate_headers(&m, true);
        let csp = checks.iter().find(|c| c.name == "Content-Security-Policy").unwrap();
        assert!(!csp.present);
        assert_eq!(csp.severity, "medium");
    }

    #[test]
    fn frame_ancestors_excuses_xfo() {
        let m = map_from(&[("content-security-policy", "default-src 'self'; frame-ancestors 'none'")]);
        let checks = evaluate_headers(&m, true);
        let xfo = checks.iter().find(|c| c.name == "X-Frame-Options").unwrap();
        assert!(!xfo.present);
        assert_eq!(xfo.severity, "info");
    }

    #[test]
    fn cookie_flags_parsed() {
        let m = map_from(&[("set-cookie", "sid=abc; Path=/; HttpOnly")]);
        let cookies = parse_cookies(&m, true);
        assert_eq!(cookies.len(), 1);
        assert!(!cookies[0].secure);
        assert!(cookies[0].http_only);
        assert_eq!(cookies[0].severity, "medium"); // missing Secure on HTTPS
    }
}
