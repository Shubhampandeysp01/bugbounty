//! WordPress → REST API surface map — DELETE file + route + registry to remove.
//! Maps namespaces, routes, and risky endpoints.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use super::common::{http_client, normalize_url};

#[derive(Debug, Serialize)]
pub struct WpRestResponse {
    pub url: String,
    pub available: bool,
    pub name: Option<String>,
    pub description: Option<String>,
    pub wp_version: Option<String>,
    pub namespaces: Vec<String>,
    pub interesting_routes: Vec<String>,
    pub route_count: usize,
    pub notes: Vec<String>,
    pub error: Option<String>,
}

const INTERESTING_KEYWORDS: &[&str] = &[
    "users",
    "plugins",
    "themes",
    "settings",
    "options",
    "media",
    "comments",
    "jwt",
    "auth",
    "login",
    "register",
    "wc/",
    "elementor",
    "wordfence",
    "yoast",
    "rankmath",
    "contact-form",
    "form",
    "upload",
    "file",
    "admin",
    "debug",
    "redirection",
    "oembed",
];

pub async fn wordpress_rest(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<WpRestResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let base = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let client = http_client(15).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut notes = Vec::new();
    let mut error = None;
    let mut available = false;
    let mut name = None;
    let mut description = None;
    let mut wp_version = None;
    let mut namespaces = Vec::new();
    let mut interesting_routes = Vec::new();
    let mut route_count = 0;

    let endpoints = [
        format!("{base}/wp-json/"),
        format!("{base}/?rest_route=/"),
        format!("{base}/index.php?rest_route=/"),
    ];

    let mut body_json: Option<Value> = None;

    for ep in &endpoints {
        match client.get(ep).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.text().await {
                    if let Ok(v) = serde_json::from_str::<Value>(&body) {
                        available = true;
                        notes.push(format!("REST root OK via {ep}"));
                        body_json = Some(v);
                        break;
                    }
                }
            }
            Ok(resp) => {
                notes.push(format!("{ep} → HTTP {}", resp.status().as_u16()));
            }
            Err(e) => {
                if error.is_none() {
                    error = Some(format!("Request failed: {e}"));
                }
            }
        }
    }

    if let Some(v) = body_json {
        name = v
            .get("name")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        description = v
            .get("description")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        wp_version = v
            .get("version") // sometimes present on older
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());

        // GMT offset / namespaces
        if let Some(ns) = v.get("namespaces").and_then(|x| x.as_array()) {
            for n in ns {
                if let Some(s) = n.as_str() {
                    namespaces.push(s.to_string());
                }
            }
        }

        if let Some(routes) = v.get("routes").and_then(|x| x.as_object()) {
            route_count = routes.len();
            for (route, _) in routes {
                let lower = route.to_lowercase();
                if INTERESTING_KEYWORDS.iter().any(|k| lower.contains(k)) {
                    interesting_routes.push(route.clone());
                }
            }
            interesting_routes.sort();
            // cap for UI
            if interesting_routes.len() > 80 {
                interesting_routes.truncate(80);
                notes.push("Interesting routes truncated to 80".into());
            }
        }

        // Site icon / home
        if let Some(home) = v.get("home").and_then(|x| x.as_str()) {
            notes.push(format!("home={home}"));
        }
        if let Some(url_field) = v.get("url").and_then(|x| x.as_str()) {
            notes.push(format!("url={url_field}"));
        }
    }

    if namespaces.iter().any(|n| n == "wp/v2") {
        notes.push("Core namespace wp/v2 present".into());
    }
    if namespaces.iter().any(|n| n.starts_with("wc/")) {
        notes.push("WooCommerce REST namespaces detected".into());
    }
    if !available {
        notes.push("REST API root not reachable — may be disabled or blocked".into());
    }

    let resp = WpRestResponse {
        url: base,
        available,
        name,
        description,
        wp_version,
        namespaces,
        interesting_routes,
        route_count,
        notes,
        error,
    };
    super::result_cache::store("wordpress-rest", &resp.url, &resp);
    Ok(Json(resp))
}
