//! WordPress → Theme enumeration — DELETE file + route + registry to remove.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::HashMap;

use super::common::{get_with_retry, http_client, normalize_url};

#[derive(Debug, Serialize)]
pub struct WpThemeHit {
    pub slug: String,
    pub path: String,
    pub version: Option<String>,
    pub theme_name: Option<String>,
    pub evidence: String,
    pub confidence: u8,
    pub evidence_explainer: String,
}

/// Detection confidence (0–100) + human explainer per evidence source.
fn confidence_for(evidence: &str) -> (u8, &'static str) {
    match evidence {
        "style.css" => (
            90,
            "Theme style.css header with a Version — strong fingerprint",
        ),
        "html_reference" => (
            60,
            "Slug referenced in page HTML — presence inferred, verify manually",
        ),
        _ => (50, "Indirect signal — verify manually"),
    }
}

#[derive(Debug, Serialize)]
pub struct WpThemesResponse {
    pub url: String,
    pub themes: Vec<WpThemeHit>,
    pub active_guess: Option<String>,
    pub notes: Vec<String>,
    pub error: Option<String>,
}

const THEME_SLUGS: &[&str] = &[
    "twentytwentyfive",
    "twentytwentyfour",
    "twentytwentythree",
    "twentytwentytwo",
    "twentytwentyone",
    "twentytwenty",
    "twentynineteen",
    "astra",
    "hello-elementor",
    "oceanwp",
    "generatepress",
    "neve",
    "kadence",
    "blocksy",
    "flot",
    "divi",
    "avada",
    "enfold",
    "the7",
    "jupiter",
    "salient",
    "bridge",
    "betheme",
    "flatsome",
    "woodmart",
    "porto",
    "xstore",
    "shopkeeper",
    "storefront",
    "sydney",
    "zerif-lite",
    "hestia",
    "customizr",
    "spacious",
    "colormag",
    "colibri-wp",
    "go",
    "popularfx",
    "zakra",
    "ashe",
    "inspiro",
    "flavor",
    "total",
    "newspaper",
    "soledad",
    "jnews",
    "rehub-theme",
];

pub async fn wordpress_themes(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<WpThemesResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let base = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let client = http_client(8).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut themes = Vec::new();
    let mut notes = Vec::new();
    let mut active_guess = None;
    let mut error = None;

    // HTML references first — active theme almost always leaks here
    if let Ok(resp) = get_with_retry(&client, &base, 3).await {
        match resp.text().await {
            Ok(body) => {
                let from_html = extract_themes_from_html(&body);
                for slug in &from_html {
                    if active_guess.is_none() {
                        active_guess = Some(slug.clone());
                    }
                    if !themes.iter().any(|t: &WpThemeHit| t.slug == *slug) {
                        let (confidence, evidence_explainer) = confidence_for("html_reference");
                        themes.push(WpThemeHit {
                            slug: slug.clone(),
                            path: format!("/wp-content/themes/{slug}/"),
                            version: None,
                            theme_name: None,
                            evidence: "html_reference".into(),
                            confidence,
                            evidence_explainer: evidence_explainer.into(),
                        });
                    }
                }
                notes.push(format!(
                    "Found {} theme path(s) in HTML",
                    from_html.len()
                ));
            }
            Err(e) => error = Some(format!("Failed reading homepage: {e}")),
        }
    }

    // Probe style.css for version
    let mut to_probe: Vec<String> = themes.iter().map(|t| t.slug.clone()).collect();
    for s in THEME_SLUGS {
        if !to_probe.iter().any(|x| x == s) {
            to_probe.push(s.to_string());
        }
    }

    let mut failed = 0usize;
    let probe_total = to_probe.len();
    for slug in to_probe {
        let style = format!("{base}/wp-content/themes/{slug}/style.css");
        match get_with_retry(&client, &style, 3).await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.text().await {
                    if body.contains("Theme Name:") || body.contains("Version:") {
                        let version = header_field(&body, "Version");
                        let theme_name = header_field(&body, "Theme Name");
                        let (confidence, evidence_explainer) = confidence_for("style.css");
                        if let Some(existing) = themes.iter_mut().find(|t| t.slug == slug) {
                            existing.version = version;
                            existing.theme_name = theme_name;
                            existing.evidence = "style.css".into();
                            existing.confidence = confidence;
                            existing.evidence_explainer = evidence_explainer.into();
                        } else {
                            themes.push(WpThemeHit {
                                slug: slug.clone(),
                                path: format!("/wp-content/themes/{slug}/"),
                                version,
                                theme_name,
                                evidence: "style.css".into(),
                                confidence,
                                evidence_explainer: evidence_explainer.into(),
                            });
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(_) => failed += 1,
        }
    }

    if failed > 0 {
        if failed == probe_total {
            error = Some(format!(
                "All {failed} theme probes failed — target may be unreachable or blocking this tool"
            ));
        } else {
            notes.push(format!(
                "{failed} style.css probe(s) failed (transient network errors — results may be incomplete)"
            ));
        }
    }

    themes.sort_by(|a, b| a.slug.cmp(&b.slug));
    notes.push("Probed style.css for known + HTML-discovered themes".to_string());

    let resp = WpThemesResponse {
        url: base,
        themes,
        active_guess,
        notes,
        error,
    };
    super::result_cache::store("wordpress-themes", &resp.url, &resp);
    Ok(Json(resp))
}

fn header_field(css: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    for line in css.lines().take(40) {
        let t = line.trim();
        if t.to_lowercase().starts_with(&prefix.to_lowercase()) {
            let v = t[prefix.len()..].trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn extract_themes_from_html(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let marker = "/wp-content/themes/";
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
        if out.len() > 20 {
            break;
        }
    }
    out
}
