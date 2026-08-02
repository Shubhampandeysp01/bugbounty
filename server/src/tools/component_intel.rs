//! Component Intelligence — per-card enrichment for detected WordPress
//! plugins/themes. Lazy endpoint consumed by the Plugin/Theme Enum cards.
//!
//! Flow:
//!   1. Fetch WP.org metadata (latest version, maintainer, downloads, …)
//!   2. Compare installed vs latest → outdated
//!   3. Match against the local Wordfence index (no duplicate scanning)
//!
//! Meta is cached in memory (incl. off-repo results); network errors are not
//! cached so a retry can succeed later.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use crate::AppState;

use super::common::http_client;
use super::wordpress_vuln_scan::{lookup_component_vulns, VulnFinding};

const WP_PLUGINS_API: &str = "https://api.wordpress.org/plugins/info/1.2/";
const WP_THEMES_API: &str = "https://api.wordpress.org/themes/info/1.2/";

#[derive(Debug, Clone, Default)]
pub struct RepoMeta {
    pub found: bool,
    pub name: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub download_link: Option<String>,
    pub downloads: Option<u64>,
    pub active_installs: Option<u64>,
    pub last_updated: Option<String>,
    pub requires: Option<String>,
    pub tested: Option<String>,
    pub tags: Vec<String>,
}

// Process-wide cache (session lifetime) — same pattern as the WF index.
fn repo_cache() -> &'static RwLock<HashMap<String, Arc<RepoMeta>>> {
    static CACHE: OnceLock<RwLock<HashMap<String, Arc<RepoMeta>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Read-only view of previously-fetched WP.org metadata (no network). The
/// Attack Surface Explorer uses this to flag outdated components without
/// triggering a new fetch. Returns `None` when the component was never looked up.
pub fn cached_repo_meta(comp_type: &str, slug: &str) -> Option<Arc<RepoMeta>> {
    repo_cache().read().unwrap().get(&format!("{comp_type}:{slug}")).cloned()
}

async fn fetch_meta(client: &reqwest::Client, comp_type: &str, slug: &str) -> Result<Arc<RepoMeta>, String> {
    let key = format!("{comp_type}:{slug}");
    {
        let guard = repo_cache().read().unwrap();
        if let Some(m) = guard.get(&key) {
            return Ok(m.clone());
        }
    }

    let endpoint = if comp_type == "theme" { WP_THEMES_API } else { WP_PLUGINS_API };
    let action = if comp_type == "theme" { "theme_information" } else { "plugin_information" };
    let url = format!("{endpoint}?action={action}&request[slug]={slug}");

    let meta = fetch_meta_from_wp(client, &url).await?;
    repo_cache().write().unwrap().insert(key, Arc::new(meta.clone()));
    Ok(Arc::new(meta))
}

fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Decode the HTML entities WP.org loves to use (names, authors, last_updated).
fn decode_entities(s: &str) -> String {
    let named = [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#039;", "'"),
        ("&apos;", "'"),
        ("&#39;", "'"),
        ("&#8211;", "\u{2013}"),
        ("&#8212;", "\u{2014}"),
        ("&#8216;", "\u{2018}"),
        ("&#8217;", "\u{2019}"),
        ("&#8220;", "\u{201c}"),
        ("&#8221;", "\u{201d}"),
        ("&#8230;", "\u{2026}"),
    ];
    let mut out = s.to_string();
    for (from, to) in named {
        out = out.replace(from, to);
    }
    // numeric entities &#\d+; → char
    let re = regex::Regex::new(r"&#(\d+);").expect("valid regex");
    out = re.replace_all(&out, |caps: &regex::Captures| {
        caps[1]
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map(|c| c.to_string())
            .unwrap_or_default()
    })
    .into_owned();
    out
}

async fn fetch_meta_from_wp(client: &reqwest::Client, url: &str) -> Result<RepoMeta, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("WP.org request failed: {e}"))?;
    let status = resp.status().as_u16();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("WP.org response read failed: {e}"))?;
    let json: Value = serde_json::from_str(&body).map_err(|e| format!("WP.org response not JSON: {e}"))?;

    if status != 200 || json.get("error").and_then(|v| v.as_str()).is_some() {
        return Ok(RepoMeta::default()); // off-repo / commercial slug — cached negative
    }

    let get_str = |k: &str| -> Option<String> {
        json.get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    Ok(RepoMeta {
        found: true,
        name: get_str("name").map(|s| decode_entities(&s)),
        version: get_str("version"),
        author: get_str("author").map(|s| decode_entities(&strip_html(&s))),
        homepage: get_str("homepage"),
        download_link: get_str("download_link"),
        downloads: json.get("downloaded").and_then(|v| v.as_u64()),
        active_installs: json.get("active_installs").and_then(|v| v.as_u64()),
        last_updated: get_str("last_updated"),
        requires: get_str("requires"),
        tested: get_str("tested"),
        tags: json
            .get("tags")
            .and_then(|v| v.as_object())
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default(),
    })
}

// ─── Version compare (independent, mirrors vuln-scan semantics) ───────────

fn parse_parts(v: &str) -> Vec<i64> {
    let cleaned = v.trim().trim_start_matches('v');
    if cleaned.is_empty() || cleaned == "*" {
        return vec![];
    }
    cleaned
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .next()
        .unwrap_or("")
        .split('.')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<i64>().unwrap_or(0))
        .collect()
}

fn version_cmp(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let pa = parse_parts(a);
    let pb = parse_parts(b);
    if pa.is_empty() || pb.is_empty() {
        return None;
    }
    let n = pa.len().max(pb.len());
    for i in 0..n {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Equal => continue,
            o => return Some(o),
        }
    }
    Some(std::cmp::Ordering::Equal)
}

// ─── API response ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ComponentIntelResponse {
    pub component_type: String,
    pub slug: String,
    pub detected_version: Option<String>,
    pub on_repo: bool,
    pub name: Option<String>,
    pub latest_version: Option<String>,
    pub outdated: Option<bool>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub repo_url: Option<String>,
    pub download_link: Option<String>,
    pub downloads: Option<u64>,
    pub active_installs: Option<u64>,
    pub last_updated: Option<String>,
    pub requires: Option<String>,
    pub tested: Option<String>,
    pub tags: Vec<String>,
    pub vulnerabilities: Vec<VulnFinding>,
    pub db_note: Option<String>,
    pub notes: Vec<String>,
    pub error: Option<String>,
}

pub async fn component_intel(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ComponentIntelResponse>, (StatusCode, String)> {
    let comp_type = params.get("type").cloned().unwrap_or_default().to_lowercase();
    if comp_type != "plugin" && comp_type != "theme" {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing or invalid 'type' parameter (expected plugin|theme)".to_string(),
        ));
    }
    let slug = params
        .get("slug")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'slug' parameter".to_string()))?;
    let detected_version = params
        .get("version")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let client = http_client(10).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut notes = Vec::new();
    let mut error = None;

    let meta = match fetch_meta(&client, &comp_type, &slug).await {
        Ok(m) => m,
        Err(e) => {
            error = Some(e);
            Arc::new(RepoMeta::default())
        }
    };

    // Outdated: installed < latest
    let outdated = match (&detected_version, &meta.version) {
        (Some(inst), Some(latest)) => match version_cmp(inst, latest) {
            Some(std::cmp::Ordering::Less) => Some(true),
            Some(_) => Some(false),
            None => None,
        },
        _ => None,
    };

    let repo_url = if comp_type == "theme" {
        Some(format!("https://wordpress.org/themes/{slug}/"))
    } else {
        Some(format!("https://wordpress.org/plugins/{slug}/"))
    };

    // Wordfence lookup — direct index match, never a scan.
    let mut db_note = None;
    let vulnerabilities = match &detected_version {
        Some(v) => match lookup_component_vulns(&state.repo_root, &comp_type, &slug, v) {
            Ok(f) => f,
            Err(e) => {
                db_note = Some(e);
                Vec::new()
            }
        },
        None => {
            notes.push("Installed version unknown — vulnerability matching skipped.".into());
            Vec::new()
        }
    };
    if !meta.found {
        notes.push(format!(
            "{slug} is not on wordpress.org (commercial/custom plugin) — latest-version comparison unavailable."
        ));
    }

    Ok(Json(ComponentIntelResponse {
        component_type: comp_type,
        slug,
        detected_version,
        on_repo: meta.found,
        name: meta.name.clone(),
        latest_version: meta.version.clone(),
        outdated,
        author: meta.author.clone(),
        homepage: meta.homepage.clone(),
        repo_url,
        download_link: meta.download_link.clone(),
        downloads: meta.downloads,
        active_installs: meta.active_installs,
        last_updated: meta.last_updated.clone(),
        requires: meta.requires.clone(),
        tested: meta.tested.clone(),
        tags: meta.tags.clone(),
        vulnerabilities,
        db_note,
        notes,
        error,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_versions() {
        assert_eq!(version_cmp("1.2", "1.2.0"), Some(std::cmp::Ordering::Equal));
        assert_eq!(version_cmp("1.2.3", "1.2.4"), Some(std::cmp::Ordering::Less));
        assert_eq!(version_cmp("2.0", "1.9.9"), Some(std::cmp::Ordering::Greater));
        assert_eq!(version_cmp("v1.2.3", "1.2.3"), Some(std::cmp::Ordering::Equal));
        assert_eq!(version_cmp("1.2.3-beta", "1.2.3"), Some(std::cmp::Ordering::Equal));
        assert_eq!(version_cmp("", "1.0"), None);
        assert_eq!(version_cmp("1.0", "*"), None);
    }
}
