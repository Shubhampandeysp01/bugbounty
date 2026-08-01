//! WordPress tools — DELETE this file + route in mod.rs + frontend registry entry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;

use super::common::{normalize_url, safe_prefix};

#[derive(Debug, Serialize)]
pub struct WpCheckResponse {
    pub url: String,
    pub version: Option<String>,
    pub version_source: Option<String>,
    pub detected: bool,
    pub generator_tag: Option<String>,
    pub rest_api_available: bool,
    pub xmlrpc_available: bool,
    pub readme_accessible: bool,
    pub wp_json_version: Option<String>,
    pub headers: HashMap<String, String>,
    pub error: Option<String>,
}

pub async fn wordpress_check(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<WpCheckResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;

    let base_url = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) BugBountyVault/1.0")
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Client error: {e}")))?;

    let mut result = WpCheckResponse {
        url: base_url.clone(),
        version: None,
        version_source: None,
        detected: false,
        generator_tag: None,
        rest_api_available: false,
        xmlrpc_available: false,
        readme_accessible: false,
        wp_json_version: None,
        headers: HashMap::new(),
        error: None,
    };

    match client.get(&base_url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            result
                .headers
                .insert("status_code".to_string(), status.to_string());

            if let Some(server) = resp.headers().get("server").and_then(|v| v.to_str().ok()) {
                result.headers.insert("server".to_string(), server.to_string());
            }
            if let Some(powered) = resp
                .headers()
                .get("x-powered-by")
                .and_then(|v| v.to_str().ok())
            {
                result
                    .headers
                    .insert("x-powered-by".to_string(), powered.to_string());
            }

            if let Ok(body) = resp.text().await {
                if let Some(start) = body.find(r#"<meta name="generator""#) {
                    let snippet = safe_prefix(&body[start..], 200);
                    result.generator_tag = Some(snippet.to_string());
                    result.detected = true;

                    if let Some(v_start) = snippet.find(r#"content="WordPress "#) {
                        let v_part = &snippet[v_start + 19..];
                        if let Some(v_end) = v_part.find('"') {
                            result.version = Some(v_part[..v_end].to_string());
                            result.version_source = Some("generator_meta_tag".to_string());
                        }
                    }
                }

                if body.contains("/wp-json/") || body.contains("wp-json") {
                    result.rest_api_available = true;
                }
            }
        }
        Err(e) => {
            result.error = Some(format!("Failed to fetch main page: {e}"));
        }
    }

    let wp_json_url = format!("{base_url}/wp-json/");
    if let Ok(resp) = client.get(&wp_json_url).send().await {
        if resp.status().is_success() {
            result.rest_api_available = true;
            if let Ok(body) = resp.text().await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(ver) = json.get("version").and_then(|v| v.as_str()) {
                        result.wp_json_version = Some(ver.to_string());
                        if result.version.is_none() {
                            result.version = Some(ver.to_string());
                            result.version_source = Some("wp_json".to_string());
                        }
                    }
                }
            }
        }
    }

    let xmlrpc_url = format!("{base_url}/xmlrpc.php");
    if let Ok(resp) = client.post(&xmlrpc_url).send().await {
        if resp.status().is_success() || resp.status().as_u16() == 405 {
            result.xmlrpc_available = true;
        }
    }

    let readme_url = format!("{base_url}/readme.html");
    if let Ok(resp) = client.get(&readme_url).send().await {
        if resp.status().is_success() {
            result.readme_accessible = true;
            if result.version.is_none() {
                if let Ok(body) = resp.text().await {
                    if let Some(start) = body.find("Version ") {
                        let v_part = safe_prefix(&body[start + 8..], 12);
                        let v: String = v_part
                            .chars()
                            .take_while(|c| c.is_ascii_digit() || *c == '.')
                            .collect();
                        if !v.is_empty() {
                            result.version = Some(v);
                            result.version_source = Some("readme_html".to_string());
                        }
                    }
                }
            }
        }
    }

    if result.version.is_some() {
        result.detected = true;
    }

    Ok(Json(result))
}
