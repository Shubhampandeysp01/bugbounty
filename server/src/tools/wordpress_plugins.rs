//! WordPress → Plugin enumeration — DELETE file + route + registry to remove.
//! Probes popular plugin paths + readme.txt for versions.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::HashMap;

use super::common::{http_client, normalize_url};

#[derive(Debug, Serialize)]
pub struct WpPluginHit {
    pub slug: String,
    pub path: String,
    pub status: u16,
    pub version: Option<String>,
    pub evidence: String,
}

#[derive(Debug, Serialize)]
pub struct WpPluginsResponse {
    pub url: String,
    pub plugins: Vec<WpPluginHit>,
    pub probed: usize,
    pub notes: Vec<String>,
    pub error: Option<String>,
}

/// Curated high-value / common plugins (not exhaustive — intentional).
const PLUGIN_SLUGS: &[&str] = &[
    "akismet",
    "contact-form-7",
    "woocommerce",
    "elementor",
    "elementor-pro",
    "wordpress-seo",
    "jetpack",
    "wordfence",
    "all-in-one-wp-migration",
    "really-simple-ssl",
    "wpforms-lite",
    "duplicate-post",
    "classic-editor",
    "google-site-kit",
    "updraftplus",
    "wp-super-cache",
    "w3-total-cache",
    "litespeed-cache",
    "autoptimize",
    "redirection",
    "advanced-custom-fields",
    "advanced-custom-fields-pro",
    "yoast-seo",
    "seo-by-rank-math",
    "all-in-one-seo-pack",
    "mailchimp-for-wp",
    "ninja-forms",
    "gravityforms",
    "revslider",
    "js_composer",
    "js-composer",
    "essential-addons-for-elementor-lite",
    "wp-file-manager",
    "file-manager-advanced",
    "duplicator",
    "backupbuddy",
    "solid-security",
    "ithemes-security",
    "sucuri-scanner",
    "better-wp-security",
    "loginizer",
    "limit-login-attempts-reloaded",
    "wp-mail-smtp",
    "fluentform",
    "formidable",
    "tablepress",
    "wp-optimize",
    "broken-link-checker",
    "query-monitor",
    "debug-bar",
    "rest-api",
    "jwt-authentication-for-wp-rest-api",
    "memberpress",
    "learndash",
    "buddypress",
    "bbpress",
    "wp-rocket",
    "imagify",
    "smush",
    "ewww-image-optimizer",
    "polylang",
    "wpml-multilingual-cms",
    "translatepress-multilingual",
    "nextgen-gallery",
    "envira-gallery-lite",
    "popup-maker",
    "optinmonster",
    "monsterinsights",
    "google-analytics-for-wordpress",
    "insert-headers-and-footers",
    "code-snippets",
    "custom-css-js",
    "header-footer-code-manager",
    "svg-support",
    "safe-svg",
    "disable-comments",
    "user-role-editor",
    "members",
    "ultimate-member",
    "profile-builder",
    "wp-user-avatar",
    "theme-my-login",
    "login-customizer",
    "custom-login",
];

pub async fn wordpress_plugins(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<WpPluginsResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let base = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let client = http_client(8).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut plugins = Vec::new();
    let mut notes = Vec::new();
    let mut error = None;

    // Concurrent-ish sequential with small batches to avoid hammering
    for slug in PLUGIN_SLUGS {
        let readme = format!("{base}/wp-content/plugins/{slug}/readme.txt");
        match client.get(&readme).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if status == 200 {
                    if let Ok(body) = resp.text().await {
                        // Confirm it looks like a plugin readme
                        if body.to_lowercase().contains("stable tag")
                            || body.to_lowercase().contains("===")
                            || body.contains("Contributors:")
                        {
                            let version = extract_stable_tag(&body);
                            plugins.push(WpPluginHit {
                                slug: slug.to_string(),
                                path: format!("/wp-content/plugins/{slug}/"),
                                status,
                                version,
                                evidence: "readme.txt".into(),
                            });
                            continue;
                        }
                    }
                }
            }
            Err(e) => {
                if error.is_none() {
                    error = Some(format!("Request error (continuing): {e}"));
                }
            }
        }

        // Fallback: plugin main directory listing / main php (404 vs 403 vs 200)
        let dir = format!("{base}/wp-content/plugins/{slug}/");
        if let Ok(resp) = client.get(&dir).send().await {
            let status = resp.status().as_u16();
            if status == 200 || status == 403 {
                // 403 often means exists but listing denied
                if let Ok(body) = resp.text().await {
                    let looks_real = status == 403
                        || body.contains("Index of")
                        || body.to_lowercase().contains(slug)
                        || body.contains("Directory listing");
                    if looks_real || status == 403 {
                        // Avoid false positives: only count 403 if we also get non-generic page
                        // Many WAFs return 403 for everything — require readme OR directory listing
                        if status == 200
                            && (body.contains("Index of")
                                || body.contains("Directory listing")
                                || body.contains("<title>"))
                        {
                            plugins.push(WpPluginHit {
                                slug: slug.to_string(),
                                path: format!("/wp-content/plugins/{slug}/"),
                                status,
                                version: None,
                                evidence: if body.contains("Index of") {
                                    "directory_listing".into()
                                } else {
                                    "plugin_path_200".into()
                                },
                            });
                        }
                    }
                }
            }
        }
    }

    // Also parse homepage for /wp-content/plugins/slug/ references
    if let Ok(resp) = client.get(&base).send().await {
        if let Ok(body) = resp.text().await {
            let found_in_html = extract_plugins_from_html(&body);
            for slug in found_in_html {
                if !plugins.iter().any(|p| p.slug == slug) {
                    plugins.push(WpPluginHit {
                        slug: slug.clone(),
                        path: format!("/wp-content/plugins/{slug}/"),
                        status: 200,
                        version: None,
                        evidence: "html_reference".into(),
                    });
                }
            }
            notes.push(format!(
                "HTML parse + readme probe of {} popular plugins",
                PLUGIN_SLUGS.len()
            ));
        }
    }

    plugins.sort_by(|a, b| a.slug.cmp(&b.slug));

    Ok(Json(WpPluginsResponse {
        url: base,
        plugins,
        probed: PLUGIN_SLUGS.len(),
        notes,
        error,
    }))
}

fn extract_stable_tag(readme: &str) -> Option<String> {
    for line in readme.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("stable tag:") {
            let v = line.split(':').nth(1)?.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn extract_plugins_from_html(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let marker = "/wp-content/plugins/";
    let mut start = 0;
    while let Some(idx) = html[start..].find(marker) {
        let abs = start + idx + marker.len();
        let rest = &html[abs..];
        let slug: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if slug.len() >= 2 && !out.contains(&slug) {
            out.push(slug);
        }
        start = abs + 1;
        if out.len() > 40 {
            break;
        }
    }
    out
}
