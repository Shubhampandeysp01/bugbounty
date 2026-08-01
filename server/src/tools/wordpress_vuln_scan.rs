//! WordPress vulnerability scanner — Wordfence Intelligence v3.
//!
//! Flow:
//!   1. Detect core / plugins / themes (same techniques as other WP tools)
//!   2. Match versions against local Wordfence DB
//!   3. Return CVEs, CVSS, remediation
//!
//! DB:
//!   tools/data/wordfence/feed.json  (full feed, gitignored)
//!   tools/data/wordfence/meta.json
//!
//! Auth key (first match wins):
//!   WORDFENCE_API_KEY env  |  .secrets/wordfence_api_key
//!
//! DELETE: this file + routes in mod.rs + registry entry + runners render.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

use crate::AppState;

use super::common::{http_client, normalize_url, safe_prefix};

const WF_FEED_URL: &str = "https://www.wordfence.com/api/intelligence/v3/vulnerabilities";

// ─── Version compare ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct VersionRange {
    from: String,
    from_inclusive: bool,
    to: String,
    to_inclusive: bool,
}

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

fn version_cmp(a: &str, b: &str) -> Ordering {
    if a == "*" || b == "*" {
        return Ordering::Equal;
    }
    let pa = parse_parts(a);
    let pb = parse_parts(b);
    let n = pa.len().max(pb.len());
    for i in 0..n {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => continue,
            o => return o,
        }
    }
    Ordering::Equal
}

fn version_in_range(version: &str, range: &VersionRange) -> bool {
    if version.trim().is_empty() {
        return false;
    }
    // lower bound
    if range.from != "*" {
        match version_cmp(version, &range.from) {
            Ordering::Less => return false,
            Ordering::Equal if !range.from_inclusive => return false,
            _ => {}
        }
    }
    // upper bound
    if range.to != "*" {
        match version_cmp(version, &range.to) {
            Ordering::Greater => return false,
            Ordering::Equal if !range.to_inclusive => return false,
            _ => {}
        }
    }
    true
}

// ─── In-memory index ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct IndexedVuln {
    id: String,
    title: String,
    description: String,
    cve: Option<String>,
    cve_link: Option<String>,
    cvss_score: Option<f64>,
    cvss_rating: Option<String>,
    cvss_vector: Option<String>,
    cwe_id: Option<i64>,
    cwe_name: Option<String>,
    cwe_description: Option<String>,
    published: Option<String>,
    updated: Option<String>,
    researchers: Vec<String>,
    references: Vec<String>,
    software_type: String,
    slug: String,
    name: String,
    ranges: Vec<VersionRange>,
    range_labels: Vec<String>,
    patched: bool,
    patched_versions: Vec<String>,
    remediation: Option<String>,
    copyright_notice: Option<String>,
    /// Full original record for detail page
    raw: Value,
}

#[derive(Debug, Default)]
struct VulnIndex {
    /// key: "plugin:slug" | "theme:slug" | "core:wordpress"
    by_key: HashMap<String, Vec<usize>>,
    vulns: Vec<IndexedVuln>,
    updated_at: String,
    count: usize,
    bytes: u64,
}

fn index_key(software_type: &str, slug: &str) -> String {
    let t = software_type.to_lowercase();
    let s = if t == "core" {
        "wordpress".to_string()
    } else {
        slug.to_lowercase()
    };
    format!("{t}:{s}")
}

fn build_index_from_feed(feed: &Value, updated_at: String, bytes: u64) -> VulnIndex {
    let mut index = VulnIndex {
        updated_at,
        bytes,
        ..Default::default()
    };

    let obj = match feed.as_object() {
        Some(o) => o,
        None => return index,
    };

    for (_id, rec) in obj {
        let title = rec
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let description = rec
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cve = rec
            .get("cve")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let cve_link = rec
            .get("cve_link")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                cve.as_ref()
                    .map(|c| format!("https://www.cve.org/CVERecord?id={c}"))
            });
        let published = rec
            .get("published")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let updated = rec
            .get("updated")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let cvss = rec.get("cvss");
        let cvss_score = cvss.and_then(|c| c.get("score")).and_then(|v| v.as_f64());
        let cvss_rating = cvss
            .and_then(|c| c.get("rating"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let cvss_vector = cvss
            .and_then(|c| c.get("vector"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let cwe = rec.get("cwe");
        let cwe_id = cwe.and_then(|c| c.get("id")).and_then(|v| v.as_i64());
        let cwe_name = cwe
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let cwe_description = cwe
            .and_then(|c| c.get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let researchers: Vec<String> = rec
            .get("researchers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let references: Vec<String> = rec
            .get("references")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let copyright_notice = rec
            .get("copyrights")
            .and_then(|c| {
                c.get("defiant")
                    .and_then(|d| d.get("notice"))
                    .or_else(|| c.get("message"))
            })
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let rec_id = rec
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let raw_record = rec.clone();

        let software = rec
            .get("software")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for soft in software {
            let stype = soft
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("plugin")
                .to_string();
            let slug = soft
                .get("slug")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = soft
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&slug)
                .to_string();
            let remediation = soft
                .get("remediation")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let patched = soft
                .get("patched")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let patched_versions: Vec<String> = soft
                .get("patched_versions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let mut ranges = Vec::new();
            let mut range_labels = Vec::new();
            if let Some(av) = soft.get("affected_versions").and_then(|v| v.as_object()) {
                for (label, range) in av {
                    let from = range
                        .get("from_version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("*")
                        .to_string();
                    let to = range
                        .get("to_version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("*")
                        .to_string();
                    let from_inc = range
                        .get("from_inclusive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let to_inc = range
                        .get("to_inclusive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    range_labels.push(if label.is_empty() {
                        format!(
                            "{}{} … {}{}",
                            if from_inc { "[" } else { "(" },
                            from,
                            to,
                            if to_inc { "]" } else { ")" }
                        )
                    } else {
                        label.clone()
                    });
                    ranges.push(VersionRange {
                        from,
                        from_inclusive: from_inc,
                        to,
                        to_inclusive: to_inc,
                    });
                }
            }

            let key = index_key(&stype, &slug);
            let entry = IndexedVuln {
                id: rec_id.clone(),
                title: title.clone(),
                description: description.clone(),
                cve: cve.clone(),
                cve_link: cve_link.clone(),
                cvss_score,
                cvss_rating: cvss_rating.clone(),
                cvss_vector: cvss_vector.clone(),
                cwe_id,
                cwe_name: cwe_name.clone(),
                cwe_description: cwe_description.clone(),
                published: published.clone(),
                updated: updated.clone(),
                researchers: researchers.clone(),
                references: references.clone(),
                software_type: stype,
                slug,
                name,
                ranges,
                range_labels,
                patched,
                patched_versions,
                remediation,
                copyright_notice: copyright_notice.clone(),
                raw: raw_record.clone(),
            };
            let idx = index.vulns.len();
            index.vulns.push(entry);
            index.by_key.entry(key).or_default().push(idx);
        }
    }

    index.count = obj.len();
    index
}

fn data_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("tools/data/wordfence")
}

fn feed_path(repo_root: &Path) -> PathBuf {
    data_dir(repo_root).join("feed.json")
}

fn meta_path(repo_root: &Path) -> PathBuf {
    data_dir(repo_root).join("meta.json")
}

fn api_key_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".secrets/wordfence_api_key")
}

fn load_api_key(repo_root: &Path) -> Option<String> {
    if let Ok(k) = std::env::var("WORDFENCE_API_KEY") {
        let t = k.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    std::fs::read_to_string(api_key_path(repo_root))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// Process-wide cache
fn vuln_cache() -> &'static RwLock<Option<Arc<VulnIndex>>> {
    static CACHE: OnceLock<RwLock<Option<Arc<VulnIndex>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

fn load_index_from_disk(repo_root: &Path) -> Result<Arc<VulnIndex>, String> {
    let path = feed_path(repo_root);
    if !path.is_file() {
        return Err(
            "Wordfence DB not found. Click “Refresh vulnerability DB” to download.".into(),
        );
    }
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("Read feed failed: {e}"))?;
    let feed: Value =
        serde_json::from_str(&raw).map_err(|e| format!("Parse feed failed: {e}"))?;

    let updated_at = std::fs::read_to_string(meta_path(repo_root))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|m| {
            m.get("updated_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    let index = build_index_from_feed(&feed, updated_at, bytes);
    Ok(Arc::new(index))
}

fn get_or_load_index(repo_root: &Path) -> Result<Arc<VulnIndex>, String> {
    {
        let guard = vuln_cache().read().unwrap();
        if let Some(ref idx) = *guard {
            return Ok(idx.clone());
        }
    }
    let idx = load_index_from_disk(repo_root)?;
    *vuln_cache().write().unwrap() = Some(idx.clone());
    Ok(idx)
}

// ─── Detection (reuse techniques) ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct DetectedComponent {
    software_type: String, // core | plugin | theme
    slug: String,
    name: String,
    version: Option<String>,
    evidence: String,
}

async fn detect_core(client: &reqwest::Client, base: &str) -> DetectedComponent {
    let mut version = None;
    let mut evidence = "not_found".to_string();

    if let Ok(resp) = client.get(base).send().await {
        if let Ok(body) = resp.text().await {
            if let Some(start) = body.find(r#"content="WordPress "#) {
                let v_part = &body[start + 19..];
                if let Some(end) = v_part.find('"') {
                    version = Some(v_part[..end].to_string());
                    evidence = "generator_meta".into();
                }
            }
        }
    }

    if version.is_none() {
        let wp_json = format!("{base}/wp-json/");
        if let Ok(resp) = client.get(&wp_json).send().await {
            if let Ok(body) = resp.text().await {
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(ver) = json.get("version").and_then(|v| v.as_str()) {
                        version = Some(ver.to_string());
                        evidence = "wp_json".into();
                    }
                }
            }
        }
    }

    if version.is_none() {
        let readme = format!("{base}/readme.html");
        if let Ok(resp) = client.get(&readme).send().await {
            if resp.status().is_success() {
                if let Ok(body) = resp.text().await {
                    if let Some(start) = body.find("Version ") {
                        let v_part = safe_prefix(&body[start + 8..], 12);
                        let v: String = v_part
                            .chars()
                            .take_while(|c| c.is_ascii_digit() || *c == '.')
                            .collect();
                        if !v.is_empty() {
                            version = Some(v);
                            evidence = "readme_html".into();
                        }
                    }
                }
            }
        }
    }

    DetectedComponent {
        software_type: "core".into(),
        slug: "wordpress".into(),
        name: "WordPress Core".into(),
        version,
        evidence,
    }
}

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
    "updraftplus",
    "wp-super-cache",
    "w3-total-cache",
    "litespeed-cache",
    "advanced-custom-fields",
    "seo-by-rank-math",
    "all-in-one-seo-pack",
    "revslider",
    "js_composer",
    "wp-file-manager",
    "duplicator",
    "loginizer",
    "wp-mail-smtp",
    "tablepress",
    "wp-optimize",
    "buddypress",
    "bbpress",
    "wp-rocket",
    "smush",
    "polylang",
    "monsterinsights",
    "code-snippets",
    "classic-editor",
    "google-site-kit",
    "redirection",
    "autoptimize",
    "sucuri-scanner",
    "ithemes-security",
    "limit-login-attempts-reloaded",
    "essential-addons-for-elementor-lite",
    "ninja-forms",
    "formidable",
    "gravityforms",
    "memberpress",
    "learndash",
    "yoast-seo",
];

fn extract_stable_tag(readme: &str) -> Option<String> {
    for line in readme.lines() {
        if line.to_lowercase().starts_with("stable tag:") {
            let v = line.split(':').nth(1)?.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn extract_slugs_from_html(html: &str, marker: &str) -> Vec<String> {
    let mut out = Vec::new();
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
        if out.len() > 50 {
            break;
        }
    }
    out
}

async fn detect_plugins(client: &reqwest::Client, base: &str) -> Vec<DetectedComponent> {
    let mut found: HashMap<String, DetectedComponent> = HashMap::new();

    if let Ok(resp) = client.get(base).send().await {
        if let Ok(body) = resp.text().await {
            for slug in extract_slugs_from_html(&body, "/wp-content/plugins/") {
                found.entry(slug.clone()).or_insert(DetectedComponent {
                    software_type: "plugin".into(),
                    slug: slug.clone(),
                    name: slug,
                    version: None,
                    evidence: "html_reference".into(),
                });
            }
        }
    }

    for slug in PLUGIN_SLUGS {
        let readme = format!("{base}/wp-content/plugins/{slug}/readme.txt");
        if let Ok(resp) = client.get(&readme).send().await {
            if resp.status().is_success() {
                if let Ok(body) = resp.text().await {
                    if body.to_lowercase().contains("stable tag")
                        || body.contains("Contributors:")
                    {
                        let version = extract_stable_tag(&body);
                        found.insert(
                            slug.to_string(),
                            DetectedComponent {
                                software_type: "plugin".into(),
                                slug: slug.to_string(),
                                name: slug.to_string(),
                                version,
                                evidence: "readme.txt".into(),
                            },
                        );
                    }
                }
            }
        }
    }

    // fill versions for HTML-only hits
    let need_ver: Vec<String> = found
        .iter()
        .filter(|(_, c)| c.version.is_none())
        .map(|(s, _)| s.clone())
        .collect();
    for slug in need_ver {
        let readme = format!("{base}/wp-content/plugins/{slug}/readme.txt");
        if let Ok(resp) = client.get(&readme).send().await {
            if resp.status().is_success() {
                if let Ok(body) = resp.text().await {
                    if let Some(v) = extract_stable_tag(&body) {
                        if let Some(c) = found.get_mut(&slug) {
                            c.version = Some(v);
                            c.evidence = "readme.txt".into();
                        }
                    }
                }
            }
        }
    }

    let mut out: Vec<_> = found.into_values().collect();
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    out
}

async fn detect_themes(client: &reqwest::Client, base: &str) -> Vec<DetectedComponent> {
    let mut found: HashMap<String, DetectedComponent> = HashMap::new();

    if let Ok(resp) = client.get(base).send().await {
        if let Ok(body) = resp.text().await {
            for slug in extract_slugs_from_html(&body, "/wp-content/themes/") {
                found.entry(slug.clone()).or_insert(DetectedComponent {
                    software_type: "theme".into(),
                    slug: slug.clone(),
                    name: slug,
                    version: None,
                    evidence: "html_reference".into(),
                });
            }
        }
    }

    let slugs: Vec<String> = found.keys().cloned().collect();
    for slug in slugs {
        let style = format!("{base}/wp-content/themes/{slug}/style.css");
        if let Ok(resp) = client.get(&style).send().await {
            if resp.status().is_success() {
                if let Ok(body) = resp.text().await {
                    let mut version = None;
                    let mut name = None;
                    for line in body.lines().take(40) {
                        let t = line.trim();
                        if t.to_lowercase().starts_with("version:") {
                            version = Some(t[8..].trim().to_string());
                        }
                        if t.to_lowercase().starts_with("theme name:") {
                            name = Some(t[11..].trim().to_string());
                        }
                    }
                    if let Some(c) = found.get_mut(&slug) {
                        if version.is_some() {
                            c.version = version;
                            c.evidence = "style.css".into();
                        }
                        if let Some(n) = name {
                            c.name = n;
                        }
                    }
                }
            }
        }
    }

    let mut out: Vec<_> = found.into_values().collect();
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    out
}

// ─── Matching ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct VulnFinding {
    pub id: String,
    pub title: String,
    pub description: String,
    pub cve: Option<String>,
    pub cve_link: Option<String>,
    pub cvss_score: Option<f64>,
    pub cvss_rating: Option<String>,
    pub cvss_vector: Option<String>,
    pub cwe_id: Option<i64>,
    pub cwe: Option<String>,
    pub cwe_description: Option<String>,
    pub software_type: String,
    pub slug: String,
    pub name: String,
    pub detected_version: String,
    pub affected_versions: Vec<String>,
    pub patched: bool,
    pub patched_versions: Vec<String>,
    pub remediation: Option<String>,
    pub researchers: Vec<String>,
    pub references: Vec<String>,
    pub published: Option<String>,
    pub updated: Option<String>,
    pub copyright: Option<String>,
}

fn match_component(index: &VulnIndex, comp: &DetectedComponent) -> Vec<VulnFinding> {
    let Some(ref ver) = comp.version else {
        return vec![];
    };
    let key = index_key(&comp.software_type, &comp.slug);
    let Some(idxs) = index.by_key.get(&key) else {
        return vec![];
    };

    let mut out = Vec::new();
    for &i in idxs {
        let v = &index.vulns[i];
        let affected = v.ranges.iter().any(|r| version_in_range(ver, r));
        if !affected {
            continue;
        }
        out.push(VulnFinding {
            id: v.id.clone(),
            title: v.title.clone(),
            description: v.description.clone(),
            cve: v.cve.clone(),
            cve_link: v.cve_link.clone(),
            cvss_score: v.cvss_score,
            cvss_rating: v.cvss_rating.clone(),
            cvss_vector: v.cvss_vector.clone(),
            cwe_id: v.cwe_id,
            cwe: v.cwe_name.clone(),
            cwe_description: v.cwe_description.clone(),
            software_type: v.software_type.clone(),
            slug: v.slug.clone(),
            name: v.name.clone(),
            detected_version: ver.clone(),
            affected_versions: v.range_labels.clone(),
            patched: v.patched,
            patched_versions: v.patched_versions.clone(),
            remediation: v.remediation.clone(),
            researchers: v.researchers.clone(),
            references: v.references.clone(),
            published: v.published.clone(),
            updated: v.updated.clone(),
            copyright: v.copyright_notice.clone(),
        });
    }
    out
}

#[derive(Debug, Serialize)]
pub struct VulnDetailResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub cve: Option<String>,
    pub cve_link: Option<String>,
    pub cvss_score: Option<f64>,
    pub cvss_rating: Option<String>,
    pub cvss_vector: Option<String>,
    pub cwe_id: Option<i64>,
    pub cwe: Option<String>,
    pub cwe_description: Option<String>,
    pub researchers: Vec<String>,
    pub references: Vec<String>,
    pub published: Option<String>,
    pub updated: Option<String>,
    pub copyright: Option<String>,
    /// Matched software row (when opened from scan)
    pub software_type: Option<String>,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub detected_version: Option<String>,
    pub affected_versions: Vec<String>,
    pub patched: Option<bool>,
    pub patched_versions: Vec<String>,
    pub remediation: Option<String>,
    /// Full original Wordfence record (all fields)
    pub record: Value,
}

pub async fn wordpress_vuln_detail(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<VulnDetailResponse>, (StatusCode, String)> {
    let id = params
        .get("id")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'id' parameter".into()))?;

    // optional context from scan click
    let detected_version = params.get("detected_version").cloned();
    let soft_type = params.get("software_type").cloned();
    let soft_slug = params.get("slug").cloned();

    let index = get_or_load_index(&state.repo_root).map_err(|e| (StatusCode::NOT_FOUND, e))?;

    let v = index
        .vulns
        .iter()
        .find(|x| x.id == id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Vulnerability id not found: {id}")))?;

    // Prefer the software row matching scan context
    let row = index.vulns.iter().find(|x| {
        x.id == id
            && soft_type
                .as_ref()
                .map(|t| t.eq_ignore_ascii_case(&x.software_type))
                .unwrap_or(true)
            && soft_slug
                .as_ref()
                .map(|s| s.eq_ignore_ascii_case(&x.slug))
                .unwrap_or(true)
    });
    let row = row.unwrap_or(v);

    Ok(Json(VulnDetailResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        description: row.description.clone(),
        cve: row.cve.clone(),
        cve_link: row.cve_link.clone(),
        cvss_score: row.cvss_score,
        cvss_rating: row.cvss_rating.clone(),
        cvss_vector: row.cvss_vector.clone(),
        cwe_id: row.cwe_id,
        cwe: row.cwe_name.clone(),
        cwe_description: row.cwe_description.clone(),
        researchers: row.researchers.clone(),
        references: row.references.clone(),
        published: row.published.clone(),
        updated: row.updated.clone(),
        copyright: row.copyright_notice.clone(),
        software_type: Some(row.software_type.clone()),
        slug: Some(row.slug.clone()),
        name: Some(row.name.clone()),
        detected_version,
        affected_versions: row.range_labels.clone(),
        patched: Some(row.patched),
        patched_versions: row.patched_versions.clone(),
        remediation: row.remediation.clone(),
        record: row.raw.clone(),
    }))
}

// ─── API responses ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DbStatusResponse {
    pub present: bool,
    pub updated_at: Option<String>,
    pub count: Option<usize>,
    pub bytes: Option<u64>,
    pub indexed: bool,
    pub api_key_configured: bool,
    pub feed_path: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct DbRefreshResponse {
    pub ok: bool,
    pub updated_at: String,
    pub count: usize,
    pub bytes: u64,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VulnScanResponse {
    pub url: String,
    pub components: Vec<DetectedComponent>,
    pub findings: Vec<VulnFinding>,
    pub summary: ScanSummary,
    pub db: DbStatusResponse,
    pub duration_ms: u64,
    pub notes: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScanSummary {
    pub components_scanned: usize,
    pub components_with_version: usize,
    pub total_findings: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub none: usize,
}

fn rating_bucket(r: &Option<String>, score: Option<f64>) -> &'static str {
    if let Some(r) = r {
        let l = r.to_lowercase();
        if l.contains("critical") {
            return "critical";
        }
        if l.contains("high") {
            return "high";
        }
        if l.contains("medium") {
            return "medium";
        }
        if l.contains("low") {
            return "low";
        }
    }
    match score {
        Some(s) if s >= 9.0 => "critical",
        Some(s) if s >= 7.0 => "high",
        Some(s) if s >= 4.0 => "medium",
        Some(s) if s > 0.0 => "low",
        _ => "none",
    }
}

// ─── Handlers ────────────────────────────────────────────────────────────────

pub async fn vuln_db_status(
    State(state): State<Arc<AppState>>,
) -> Json<DbStatusResponse> {
    let root = &state.repo_root;
    let path = feed_path(root);
    let present = path.is_file();
    let meta = std::fs::read_to_string(meta_path(root))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());
    let indexed = vuln_cache().read().unwrap().is_some();

    Json(DbStatusResponse {
        present,
        updated_at: meta
            .as_ref()
            .and_then(|m| m.get("updated_at"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        count: meta
            .as_ref()
            .and_then(|m| m.get("count"))
            .and_then(|v| v.as_u64())
            .map(|n| n as usize),
        bytes: meta
            .as_ref()
            .and_then(|m| m.get("bytes"))
            .and_then(|v| v.as_u64())
            .or_else(|| std::fs::metadata(&path).ok().map(|m| m.len())),
        indexed,
        api_key_configured: load_api_key(root).is_some(),
        feed_path: path.display().to_string(),
        source: WF_FEED_URL.into(),
    })
}

pub async fn vuln_db_refresh(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DbRefreshResponse>, (StatusCode, String)> {
    let root = &state.repo_root;
    let started = Instant::now();

    let key = load_api_key(root).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Wordfence API key not configured. Set WORDFENCE_API_KEY or .secrets/wordfence_api_key"
                .into(),
        )
    })?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .user_agent("BugBountyVault/1.0 (Wordfence Intelligence client)")
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let resp = client
        .get(WF_FEED_URL)
        .header("Authorization", format!("Bearer {key}"))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Download failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Ok(Json(DbRefreshResponse {
            ok: false,
            updated_at: Utc::now().to_rfc3339(),
            count: 0,
            bytes: 0,
            duration_ms: started.elapsed().as_millis() as u64,
            error: Some(format!(
                "Wordfence API HTTP {status}: {}",
                safe_prefix(&body, 300)
            )),
        }));
    }

    let bytes_data = resp
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Read body failed: {e}")))?;
    let bytes = bytes_data.len() as u64;

    let feed: Value = serde_json::from_slice(&bytes_data).map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Invalid JSON from Wordfence: {e}"),
        )
    })?;

    let count = feed.as_object().map(|o| o.len()).unwrap_or(0);
    let updated_at = Utc::now().to_rfc3339();

    std::fs::create_dir_all(data_dir(root)).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Create data dir failed: {e}"),
        )
    })?;

    std::fs::write(feed_path(root), &bytes_data).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Write feed failed: {e}"),
        )
    })?;

    let meta = serde_json::json!({
        "updated_at": updated_at,
        "count": count,
        "bytes": bytes,
        "source": WF_FEED_URL,
        "auth": "bearer",
    });
    std::fs::write(
        meta_path(root),
        serde_json::to_string_pretty(&meta).unwrap_or_default(),
    )
    .ok();

    let index = Arc::new(build_index_from_feed(&feed, updated_at.clone(), bytes));
    *vuln_cache().write().unwrap() = Some(index);

    Ok(Json(DbRefreshResponse {
        ok: true,
        updated_at,
        count,
        bytes,
        duration_ms: started.elapsed().as_millis() as u64,
        error: None,
    }))
}

pub async fn wordpress_vuln_scan(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<VulnScanResponse>, (StatusCode, String)> {
    let started = Instant::now();
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let base = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let root = &state.repo_root;
    let mut notes = Vec::new();

    let index = match get_or_load_index(root) {
        Ok(i) => i,
        Err(e) => {
            return Ok(Json(VulnScanResponse {
                url: base,
                components: vec![],
                findings: vec![],
                summary: ScanSummary {
                    components_scanned: 0,
                    components_with_version: 0,
                    total_findings: 0,
                    critical: 0,
                    high: 0,
                    medium: 0,
                    low: 0,
                    none: 0,
                },
                db: DbStatusResponse {
                    present: false,
                    updated_at: None,
                    count: None,
                    bytes: None,
                    indexed: false,
                    api_key_configured: load_api_key(root).is_some(),
                    feed_path: feed_path(root).display().to_string(),
                    source: WF_FEED_URL.into(),
                },
                duration_ms: started.elapsed().as_millis() as u64,
                notes: vec![e.clone()],
                error: Some(e),
            }));
        }
    };

    let client = http_client(12).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    notes.push("Detecting WordPress core version…".into());
    let core = detect_core(&client, &base).await;
    notes.push("Enumerating plugins…".into());
    let plugins = detect_plugins(&client, &base).await;
    notes.push("Enumerating themes…".into());
    let themes = detect_themes(&client, &base).await;

    let mut components = Vec::new();
    components.push(core);
    components.extend(plugins);
    components.extend(themes);

    let mut findings = Vec::new();
    for c in &components {
        findings.extend(match_component(&index, c));
    }

    // sort by severity score desc
    findings.sort_by(|a, b| {
        b.cvss_score
            .partial_cmp(&a.cvss_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
    });

    let mut summary = ScanSummary {
        components_scanned: components.len(),
        components_with_version: components.iter().filter(|c| c.version.is_some()).count(),
        total_findings: findings.len(),
        critical: 0,
        high: 0,
        medium: 0,
        low: 0,
        none: 0,
    };
    for f in &findings {
        match rating_bucket(&f.cvss_rating, f.cvss_score) {
            "critical" => summary.critical += 1,
            "high" => summary.high += 1,
            "medium" => summary.medium += 1,
            "low" => summary.low += 1,
            _ => summary.none += 1,
        }
    }

    notes.push(format!(
        "Matched against Wordfence DB ({} vulns, updated {})",
        index.count, index.updated_at
    ));
    if components.iter().any(|c| c.software_type == "core" && c.version.is_none()) {
        notes.push("Core version not detected — core CVEs may be incomplete.".into());
    }

    Ok(Json(VulnScanResponse {
        url: base,
        components,
        findings,
        summary,
        db: DbStatusResponse {
            present: true,
            updated_at: Some(index.updated_at.clone()),
            count: Some(index.count),
            bytes: Some(index.bytes),
            indexed: true,
            api_key_configured: load_api_key(root).is_some(),
            feed_path: feed_path(root).display().to_string(),
            source: WF_FEED_URL.into(),
        },
        duration_ms: started.elapsed().as_millis() as u64,
        notes,
        error: None,
    }))
}

// unit-ish checks without full test harness
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_inclusive() {
        let r = VersionRange {
            from: "*".into(),
            from_inclusive: true,
            to: "1.37".into(),
            to_inclusive: true,
        };
        assert!(version_in_range("1.37", &r));
        assert!(version_in_range("1.0", &r));
        assert!(!version_in_range("1.38", &r));
    }
}
