//! CVE lookup — NVD 2.0 REST API with an on-disk cache.
//!
//! Endpoints:
//!   GET /api/tools/cve-lookup?cve=CVE-2024-1234   → exact CVE lookup
//!   GET /api/tools/cve-lookup?q=linux kernel rce   → keyword search (top N)
//!
//! Cache: server/tools/data/cves/<CVE-ID>.json — looked-up records are stored
//! so repeat lookups and the findings DB are fast and work offline.
//!
//! DELETE: this file + routes in mod.rs + registry entry + runners render.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::AppState;

use super::common::http_client;

const NVD_API: &str = "https://services.nvd.nist.gov/rest/json/cves/2.0";
const CACHE_TTL: i64 = 24 * 60 * 60; // 24h before re-fetching a record

#[derive(Debug, Deserialize)]
pub struct LookupParams {
    pub cve: Option<String>,
    pub q: Option<String>,
}

/// Normalized CVE record, independent of NVD's JSON layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveRecord {
    pub cve_id: String,
    pub description: String,
    pub published: Option<String>,
    pub modified: Option<String>,
    pub cvss_v3: Option<CvssScore>,
    pub cvss_v2: Option<CvssScore>,
    pub cwes: Vec<String>,
    pub references: Vec<CveReference>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CvssScore {
    pub version: String,
    pub base_score: f64,
    pub severity: String,
    pub vector: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveReference {
    pub url: String,
    pub source: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LookupResponse {
    ok: bool,
    records: Vec<CveRecord>,
    cached: bool,
    error: Option<String>,
}

fn cache_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("tools/data/cves")
}

fn cache_path(repo_root: &Path, cve_id: &str) -> PathBuf {
    cache_dir(repo_root).join(format!("{}.json", cve_id.to_uppercase()))
}

/// Validates a CVE id shape: CVE-YYYY-NNNNN.
fn normalize_cve_id(raw: &str) -> Result<String, String> {
    let t = raw.trim().to_uppercase();
    let mut parts = t.split('-');
    if parts.next() != Some("CVE") {
        return Err("Expected format CVE-YYYY-NNNNN, e.g. CVE-2024-12345".into());
    }
    let year = parts.next().unwrap_or("");
    let num = parts.next().unwrap_or("");
    if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit()) {
        return Err("CVE year must be 4 digits".into());
    }
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return Err("CVE number must be digits".into());
    }
    Ok(t)
}

/// Returns a cached record if it's fresh enough, else None.
fn read_cache(repo_root: &Path, cve_id: &str) -> Option<CveRecord> {
    let path = cache_path(repo_root, cve_id);
    let raw = std::fs::read_to_string(&path).ok()?;
    let rec: CveRecord = serde_json::from_str(&raw).ok()?;
    // Only trust the cache if it was fetched recently.
    let mtime = std::fs::metadata(&path).ok()?.modified().ok()?;
    let age = Utc::now().timestamp() - mtime.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    if age > CACHE_TTL {
        return None;
    }
    Some(rec)
}

fn write_cache(repo_root: &Path, rec: &CveRecord) {
    if std::fs::create_dir_all(cache_dir(repo_root)).is_ok() {
        std::fs::write(cache_path(repo_root, &rec.cve_id), serde_json::to_vec_pretty(rec).unwrap()).ok();
    }
}

fn pick_description(v: &Value) -> String {
    v["descriptions"]
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|d| d["lang"].as_str() == Some("en"))
                .or_else(|| arr.first())
        })
        .and_then(|d| d["value"].as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn pick_cvss(v: &Value) -> (Option<CvssScore>, Option<CvssScore>) {
    let mut v3 = None;
    let mut v2 = None;
    let metrics = v["metrics"].as_object().cloned().unwrap_or_default();
    // CVSS v4.x is available in newer NVD records.
    if let Some(arr) = metrics.get("cvssMetricV40").and_then(|a| a.as_array()) {
        if let Some(m) = arr.first() {
            let d = &m["cvssData"];
            v3 = Some(CvssScore {
                version: "4.0".into(),
                base_score: d["baseScore"].as_f64().unwrap_or(0.0),
                severity: d["baseSeverity"].as_str().unwrap_or("").to_string(),
                vector: d["vectorString"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    if v3.is_none() {
        if let Some(arr) = metrics.get("cvssMetricV31").and_then(|a| a.as_array()) {
            if let Some(m) = arr.first() {
                let d = &m["cvssData"];
                v3 = Some(CvssScore {
                    version: "3.1".into(),
                    base_score: d["baseScore"].as_f64().unwrap_or(0.0),
                    severity: d["baseSeverity"].as_str().unwrap_or("").to_string(),
                    vector: d["vectorString"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    if v3.is_none() {
        if let Some(arr) = metrics.get("cvssMetricV30").and_then(|a| a.as_array()) {
            if let Some(m) = arr.first() {
                let d = &m["cvssData"];
                v3 = Some(CvssScore {
                    version: "3.0".into(),
                    base_score: d["baseScore"].as_f64().unwrap_or(0.0),
                    severity: d["baseSeverity"].as_str().unwrap_or("").to_string(),
                    vector: d["vectorString"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    if let Some(arr) = metrics.get("cvssMetricV2").and_then(|a| a.as_array()) {
        if let Some(m) = arr.first() {
            let d = &m["cvssData"];
            v2 = Some(CvssScore {
                version: "2.0".into(),
                base_score: d["baseScore"].as_f64().unwrap_or(0.0),
                severity: d["baseSeverity"]
                    .as_str()
                    .or_else(|| d["severity"].as_str())
                    .unwrap_or("")
                    .to_string(),
                vector: d["vectorString"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    (v3, v2)
}

fn pick_cwes(v: &Value) -> Vec<String> {
    v["weaknesses"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|w| {
                    w["description"]
                        .as_array()
                        .and_then(|d| d.first())
                        .and_then(|d| d["value"].as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn pick_references(v: &Value) -> Vec<CveReference> {
    v["references"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    let url = r["url"].as_str()?.to_string();
                    Some(CveReference {
                        url,
                        source: r["source"].as_str().unwrap_or("").to_string(),
                        tags: r["tags"]
                            .as_array()
                            .map(|t| {
                                t.iter()
                                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_nvd_item(v: &Value) -> Option<CveRecord> {
    let cve_id = v["cve"]["id"].as_str()?.to_string();
    let description = pick_description(&v["cve"]);
    let (cvss_v3, cvss_v2) = pick_cvss(&v["cve"]);
    let published = v["cve"]["published"]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.to_rfc3339());
    let modified = v["cve"]["lastModified"]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.to_rfc3339());

    Some(CveRecord {
        cve_id,
        description,
        published,
        modified,
        cvss_v3,
        cvss_v2,
        cwes: pick_cwes(&v["cve"]),
        references: pick_references(&v["cve"]),
        source: "nvd".into(),
    })
}

/// Exact CVE lookup with cache.
async fn lookup_cve(repo_root: &Path, cve_id: &str) -> Result<(CveRecord, bool), String> {
    if let Some(rec) = read_cache(repo_root, cve_id) {
        return Ok((rec, true));
    }

    let client = http_client(20)?;
    let url = format!("{NVD_API}?cveId={cve_id}");
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("NVD request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("NVD returned HTTP {status}: {}", body.chars().take(200).collect::<String>()));
    }
    let parsed: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse NVD response: {e}"))?;

    let vulns = parsed["vulnerabilities"].as_array().cloned().unwrap_or_default();
    let rec = vulns
        .first()
        .and_then(parse_nvd_item)
        .ok_or_else(|| format!("CVE {cve_id} not found in NVD"))?;
    write_cache(repo_root, &rec);
    Ok((rec, false))
}

/// Keyword search against NVD — no caching (results differ per query).
async fn search_cves(repo_root: &Path, keyword: &str) -> Result<Vec<CveRecord>, String> {
    let client = http_client(20)?;
    let url = format!("{NVD_API}?keywordSearch={}&resultsPerPage=10", urlencode(keyword));
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("NVD request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("NVD returned HTTP {status}: {}", body.chars().take(200).collect::<String>()));
    }
    let parsed: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse NVD response: {e}"))?;

    let vulns = parsed["vulnerabilities"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for v in vulns {
        if let Some(rec) = parse_nvd_item(&v) {
            write_cache(repo_root, &rec);
            out.push(rec);
        }
    }
    Ok(out)
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// GET /api/tools/cve-lookup?cve=…  |  ?q=…
pub async fn cve_lookup(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LookupParams>,
) -> Result<Json<LookupResponse>, (StatusCode, String)> {
    let root = &state.repo_root;

    if let Some(cve) = params.cve {
        let cve = match normalize_cve_id(&cve) {
            Ok(c) => c,
            Err(e) => return Err((StatusCode::BAD_REQUEST, e)),
        };
        return match lookup_cve(root, &cve).await {
            Ok((rec, cached)) => Ok(Json(LookupResponse {
                ok: true,
                records: vec![rec],
                cached,
                error: None,
            })),
            Err(e) => Err((StatusCode::BAD_GATEWAY, e)),
        };
    }

    if let Some(q) = params.q {
        let q = q.trim();
        if q.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Query is empty".into()));
        }
        return match search_cves(root, q).await {
            Ok(records) => Ok(Json(LookupResponse {
                ok: true,
                records,
                cached: false,
                error: None,
            })),
            Err(e) => Err((StatusCode::BAD_GATEWAY, e)),
        };
    }

    Err((StatusCode::BAD_REQUEST, "Provide ?cve=CVE-… or ?q=keyword".into()))
}
