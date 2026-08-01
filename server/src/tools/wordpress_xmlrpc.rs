//! WordPress → XML-RPC deep probe — DELETE file + route + registry to remove.
//! Checks availability, listMethods, multicall, pingback surface.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::HashMap;

use super::common::{http_client, normalize_url};

#[derive(Debug, Serialize)]
pub struct WpXmlrpcResponse {
    pub url: String,
    pub endpoint: String,
    pub available: bool,
    pub methods: Vec<String>,
    pub method_count: usize,
    pub multicall: bool,
    pub pingback: bool,
    pub system_get_capabilities: bool,
    pub interesting: Vec<String>,
    pub notes: Vec<String>,
    pub error: Option<String>,
}

const LIST_METHODS: &str = r#"<?xml version="1.0"?>
<methodCall>
  <methodName>system.listMethods</methodName>
  <params></params>
</methodCall>"#;

pub async fn wordpress_xmlrpc(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<WpXmlrpcResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let base = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let endpoint = format!("{base}/xmlrpc.php");

    let client = http_client(15).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut notes = Vec::new();
    let mut methods = Vec::new();
    let mut available = false;
    let mut error = None;

    // POST system.listMethods
    match client
        .post(&endpoint)
        .header("Content-Type", "text/xml")
        .body(LIST_METHODS)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            notes.push(format!("POST xmlrpc.php → HTTP {status}"));
            if status == 200 || status == 405 {
                available = true;
            }
            if let Ok(body) = resp.text().await {
                if body.contains("faultCode") && body.contains("405") {
                    available = false;
                    notes.push("XML-RPC appears disabled (fault/405)".into());
                }
                if body.contains("<methodResponse>") || body.contains("<string>") {
                    available = true;
                    methods = parse_method_strings(&body);
                    notes.push(format!("system.listMethods returned {} methods", methods.len()));
                }
                if body.to_lowercase().contains("xml-rpc server accepts post requests only") {
                    available = true;
                    notes.push("Endpoint alive but needs POST (GET blocked)".into());
                }
            }
        }
        Err(e) => {
            error = Some(format!("Request failed: {e}"));
        }
    }

    // If listMethods failed but GET/POST shows something, still mark available from earlier checks
    if !available {
        if let Ok(resp) = client.get(&endpoint).send().await {
            let body = resp.text().await.unwrap_or_default();
            if body.to_lowercase().contains("xml-rpc") {
                available = true;
                notes.push("GET suggests XML-RPC endpoint exists".into());
            }
        }
    }

    let multicall = methods.iter().any(|m| m == "system.multicall");
    let pingback = methods.iter().any(|m| m.contains("pingback"));
    let system_get_capabilities = methods.iter().any(|m| m == "system.getCapabilities");

    let mut interesting = Vec::new();
    for m in &methods {
        if m.contains("pingback")
            || m.contains("multicall")
            || m.starts_with("wp.")
            || m.starts_with("blogger.")
            || m.contains("getUsersBlogs")
            || m.contains("getProfile")
        {
            interesting.push(m.clone());
        }
    }
    interesting.sort();
    interesting.dedup();

    if multicall {
        notes.push(
            "system.multicall enabled — amplifies brute-force / abuse potential".into(),
        );
    }
    if pingback {
        notes.push("pingback methods present — SSRF/DDoS pivot surface".into());
    }
    if !available {
        notes.push("XML-RPC looks disabled or filtered — good hardening".into());
    }

    Ok(Json(WpXmlrpcResponse {
        url: base,
        endpoint,
        available,
        method_count: methods.len(),
        methods,
        multicall,
        pingback,
        system_get_capabilities,
        interesting,
        notes,
        error,
    }))
}

fn parse_method_strings(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<string>") {
        let after = &rest[start + 8..];
        if let Some(end) = after.find("</string>") {
            let s = after[..end].trim().to_string();
            if !s.is_empty() && !out.contains(&s) {
                out.push(s);
            }
            rest = &after[end + 9..];
        } else {
            break;
        }
    }
    out
}
