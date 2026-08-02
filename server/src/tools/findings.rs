//! Findings database — persistent JSON store for bug bounty findings.
//!
//! Storage: server/tools/data/findings/findings.json (gitignored).
//! A single JSON array of finding records; mutations rewrite the file
//! atomically (write temp + rename) so a crash never corrupts the store.
//!
//! Endpoints:
//!   GET    /api/tools/findings            → list (filters: ?q=, ?severity=, ?status=)
//!   GET    /api/tools/findings/{id}       → one record
//!   POST   /api/tools/findings            → create
//!   PUT    /api/tools/findings/{id}       → update
//!   DELETE /api/tools/findings/{id}       → delete
//!
//! DELETE: this file + routes in mod.rs + registry entry + runners render.

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub vuln_type: String,
    #[serde(default = "default_severity")]
    pub severity: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub cve_id: String,
    #[serde(default)]
    pub cvss_score: f64,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub remediation: String,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn default_severity() -> String {
    "medium".into()
}
fn default_status() -> String {
    "open".into()
}

#[derive(Debug, Deserialize)]
pub struct FindingInput {
    pub title: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub vuln_type: String,
    #[serde(default = "default_severity")]
    pub severity: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub cve_id: String,
    #[serde(default)]
    pub cvss_score: f64,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub remediation: String,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub q: Option<String>,
    pub severity: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StoreResponse {
    ok: bool,
    findings: Option<Vec<Finding>>,
    finding: Option<Finding>,
    error: Option<String>,
}

/// Process-wide store path guard — ensures serialized writes across requests.
fn store_mutex() -> &'static Mutex<()> {
    static M: Mutex<()> = Mutex::new(());
    &M
}

fn store_path(repo_root: &Path) -> PathBuf {
    repo_root.join("tools/data/findings/findings.json")
}

fn load_all(repo_root: &Path) -> Vec<Finding> {
    let path = store_path(repo_root);
    let raw = match std::fs::read_to_string(&path) {
        Ok(r) => r,
        Err(_) => return Vec::new(), // no store yet
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_all(repo_root: &Path, findings: &[Finding]) -> Result<(), String> {
    let path = store_path(repo_root);
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create findings dir: {e}"))?;
    let json = serde_json::to_vec_pretty(findings).map_err(|e| format!("Serialize failed: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| format!("Write failed: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("Rename failed: {e}"))?;
    Ok(())
}

fn now_rfc() -> String {
    Utc::now().to_rfc3339()
}

fn gen_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("fnd-{nanos:x}")
}

fn validate(input: &FindingInput) -> Result<(), String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("Title is required".into());
    }
    if title.len() > 300 {
        return Err("Title too long".into());
    }
    for (label, s) in [
        ("target", &input.target),
        ("vuln_type", &input.vuln_type),
        ("severity", &input.severity),
        ("status", &input.status),
        ("cve_id", &input.cve_id),
        ("endpoint", &input.endpoint),
        ("description", &input.description),
        ("remediation", &input.remediation),
    ] {
        if s.len() > 10000 {
            return Err(format!("{label} too long"));
        }
        if s.chars().any(|c| c.is_control() && c != '\n') {
            return Err(format!("{label} contains invalid characters"));
        }
    }
    Ok(())
}

/// Normalizes severity / status to a controlled set (lowercase).
fn normalize_severity(s: &str) -> String {
    let t = s.trim().to_lowercase();
    match t.as_str() {
        "critical" | "high" | "medium" | "low" | "info" => t,
        _ => "medium".into(),
    }
}

fn normalize_status(s: &str) -> String {
    let t = s.trim().to_lowercase();
    match t.as_str() {
        "open" | "confirmed" | "fixed" | "accepted" | "info" => t,
        _ => "open".into(),
    }
}

/// GET /api/tools/findings?q=&severity=&status=
pub async fn list_findings(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Json<StoreResponse> {
    let _guard = store_mutex().lock().unwrap();
    let all = load_all(&state.repo_root);
    let q = params.q.as_deref().unwrap_or("").trim().to_lowercase();
    let sev = params.severity.as_deref().unwrap_or("").trim().to_lowercase();
    let status = params.status.as_deref().unwrap_or("").trim().to_lowercase();

    let findings: Vec<Finding> = all
        .into_iter()
        .filter(|f| {
            let matches_q = q.is_empty()
                || f.title.to_lowercase().contains(&q)
                || f.target.to_lowercase().contains(&q)
                || f.cve_id.to_lowercase().contains(&q)
                || f.vuln_type.to_lowercase().contains(&q)
                || f.tags.iter().any(|t| t.to_lowercase().contains(&q));
            let matches_sev = sev.is_empty() || f.severity.eq_ignore_ascii_case(&sev);
            let matches_status = status.is_empty() || f.status.eq_ignore_ascii_case(&status);
            matches_q && matches_sev && matches_status
        })
        // newest first
        .collect();

    Json(StoreResponse {
        ok: true,
        findings: Some(findings),
        finding: None,
        error: None,
    })
}

/// GET /api/tools/findings/{id}
pub async fn get_finding(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<StoreResponse>, (StatusCode, String)> {
    let _guard = store_mutex().lock().unwrap();
    let all = load_all(&state.repo_root);
    match all.into_iter().find(|f| f.id == id) {
        Some(f) => Ok(Json(StoreResponse {
            ok: true,
            findings: None,
            finding: Some(f),
            error: None,
        })),
        None => Err((StatusCode::NOT_FOUND, format!("Finding {id} not found"))),
    }
}

/// POST /api/tools/findings  (JSON body)
pub async fn create_finding(
    State(state): State<Arc<AppState>>,
    Json(input): Json<FindingInput>,
) -> Result<Json<StoreResponse>, (StatusCode, String)> {
    validate(&input).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let _guard = store_mutex().lock().unwrap();

    let mut all = load_all(&state.repo_root);
    let now = now_rfc();
    let finding = Finding {
        id: gen_id(),
        title: input.title.trim().to_string(),
        target: input.target.trim().to_string(),
        vuln_type: input.vuln_type.trim().to_string(),
        severity: normalize_severity(&input.severity),
        status: normalize_status(&input.status),
        cve_id: input.cve_id.trim().to_string(),
        cvss_score: input.cvss_score,
        endpoint: input.endpoint.trim().to_string(),
        description: input.description.trim().to_string(),
        remediation: input.remediation.trim().to_string(),
        references: input
            .references
            .into_iter()
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect(),
        tags: input
            .tags
            .into_iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        created_at: now.clone(),
        updated_at: now,
    };
    all.push(finding.clone());
    save_all(&state.repo_root, &all).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(StoreResponse {
        ok: true,
        findings: None,
        finding: Some(finding),
        error: None,
    }))
}

/// PUT /api/tools/findings/{id}  (JSON body)
pub async fn update_finding(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<FindingInput>,
) -> Result<Json<StoreResponse>, (StatusCode, String)> {
    validate(&input).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let _guard = store_mutex().lock().unwrap();

    let mut all = load_all(&state.repo_root);
    let idx = match all.iter().position(|f| f.id == id) {
        Some(i) => i,
        None => return Err((StatusCode::NOT_FOUND, format!("Finding {id} not found"))),
    };
    let old = &all[idx];
    let updated = Finding {
        id: old.id.clone(),
        title: input.title.trim().to_string(),
        target: input.target.trim().to_string(),
        vuln_type: input.vuln_type.trim().to_string(),
        severity: normalize_severity(&input.severity),
        status: normalize_status(&input.status),
        cve_id: input.cve_id.trim().to_string(),
        cvss_score: input.cvss_score,
        endpoint: input.endpoint.trim().to_string(),
        description: input.description.trim().to_string(),
        remediation: input.remediation.trim().to_string(),
        references: input
            .references
            .into_iter()
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect(),
        tags: input
            .tags
            .into_iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        created_at: old.created_at.clone(),
        updated_at: now_rfc(),
    };
    all[idx] = updated.clone();
    save_all(&state.repo_root, &all).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(StoreResponse {
        ok: true,
        findings: None,
        finding: Some(updated),
        error: None,
    }))
}

/// DELETE /api/tools/findings/{id}
pub async fn delete_finding(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<StoreResponse>, (StatusCode, String)> {
    let _guard = store_mutex().lock().unwrap();
    let mut all = load_all(&state.repo_root);
    let before = all.len();
    all.retain(|f| f.id != id);
    if all.len() == before {
        return Err((StatusCode::NOT_FOUND, format!("Finding {id} not found")));
    }
    save_all(&state.repo_root, &all).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(StoreResponse {
        ok: true,
        findings: None,
        finding: None,
        error: None,
    }))
}
