//! Attack Surface Explorer — aggregation / visualization layer for WordPress.
//!
//! NOT a scanner: it reuses results already produced by the existing tools
//! (Version Check, Plugin Enum, Theme Enum, Component Intelligence, REST
//! Surface, XML-RPC Probe, User Enum, Sensitive Paths, WF Vuln Scanner, Nuclei)
//! from `result_cache` (sync tools) and the JobManager (job-backed tools).
//!
//! The aggregated view is cached and recomputed only when the underlying
//! results change (a generation counter in `result_cache` + job fingerprints).
//!
//! Registry/adapter pattern: adding a future CMS = add a source + node builder;
//! new findings register per-source, no if/else chains in the renderers.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use crate::jobs::JobStatus;
use crate::AppState;

use super::common::normalize_url;
use super::component_intel::cached_repo_meta;
use super::result_cache;
use super::wordpress_vuln_scan::lookup_component_vulns;

/// Job-backed tools (result lives in the JobManager, mirrored to result_cache).
const JOB_TOOLS: &[&str] = &["wordpress-vuln-scan", "wordpress-nuclei"];

/// Tracked sources: (tool id, human label).
const SOURCES: &[(&str, &str)] = &[
    ("wordpress-check", "Version Check"),
    ("wordpress-plugins", "Plugin Enum"),
    ("wordpress-themes", "Theme Enum"),
    ("wordpress-rest", "REST Surface"),
    ("wordpress-xmlrpc", "XML-RPC Probe"),
    ("wordpress-users", "User Enum"),
    ("wordpress-paths", "Sensitive Paths"),
    ("wordpress-vuln-scan", "WF Vuln Scanner"),
    ("wordpress-nuclei", "WP Nuclei Scan"),
];

// ─── Response types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AttackSurfaceItem {
    pub label: String,
    pub value: Option<String>,
    pub severity: String,
    pub detail: Option<String>,
    pub meta: Vec<[String; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttackSurfaceNode {
    pub id: String,
    pub label: String,
    pub category: String,
    pub status: String,
    pub count: usize,
    pub note: String,
    pub items: Vec<AttackSurfaceItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttackSurfaceSummary {
    pub core: String,
    pub plugins: usize,
    pub themes: usize,
    pub rest_routes: usize,
    pub authentication: usize,
    pub sensitive_files: usize,
    pub known_vulns: usize,
    pub total_findings: usize,
    pub overall: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MissingTool {
    pub tool: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttackSurfaceResponse {
    pub url: String,
    pub generated_at: String,
    pub nodes: Vec<AttackSurfaceNode>,
    pub summary: AttackSurfaceSummary,
    pub missing: Vec<MissingTool>,
}

// ─── Severity helpers ───────────────────────────────────────────────────────

fn sev(s: &str) -> &'static str {
    let l = s.trim().to_lowercase();
    if l == "info" || l == "informational" || l == "none" {
        "informational"
    } else if l.contains("critical") {
        "critical"
    } else if l.contains("high") {
        "high"
    } else if l.contains("medium") {
        "medium"
    } else if l.contains("low") {
        "low"
    } else {
        "informational"
    }
}

fn score_sev(s: f64) -> &'static str {
    if s >= 9.0 {
        "critical"
    } else if s >= 7.0 {
        "high"
    } else if s >= 4.0 {
        "medium"
    } else if s > 0.0 {
        "low"
    } else {
        "informational"
    }
}

fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 5,
        "high" => 4,
        "medium" => 3,
        "low" => 2,
        "informational" => 1,
        _ => 0,
    }
}

fn rank_sev(r: u8) -> &'static str {
    match r {
        5 => "critical",
        4 => "high",
        3 => "medium",
        2 => "low",
        1 => "informational",
        _ => "none",
    }
}

// ─── JSON value helpers ─────────────────────────────────────────────────────

fn str_at<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(|x| x.as_str())
}
fn bool_at(v: &Value, k: &str) -> Option<bool> {
    v.get(k).and_then(|x| x.as_bool())
}
fn usize_at(v: &Value, k: &str) -> Option<usize> {
    v.get(k).and_then(|x| x.as_u64()).map(|u| u as usize)
}
fn f64_at(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}
fn arr<'a>(v: &'a Value, k: &str) -> Vec<&'a Value> {
    v.get(k)
        .and_then(|x| x.as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}
fn arr_str(v: &Value, k: &str) -> Vec<String> {
    arr(v, k)
        .iter()
        .filter_map(|x| x.as_str())
        .map(str::to_string)
        .collect()
}

fn add(
    items: &mut Vec<AttackSurfaceItem>,
    label: &str,
    value: Option<String>,
    severity: &str,
    detail: Option<String>,
    meta: Vec<[String; 2]>,
) {
    items.push(AttackSurfaceItem {
        label: label.into(),
        value,
        severity: severity.into(),
        detail,
        meta,
    });
}

// ─── Source resolution ──────────────────────────────────────────────────────

fn same_url(a: &str, b: &str) -> bool {
    let na = normalize_url(a).unwrap_or_else(|_| a.trim().to_string());
    let nb = normalize_url(b).unwrap_or_else(|_| b.trim().to_string());
    na.eq_ignore_ascii_case(&nb)
}

/// Latest succeeded result for a job-backed tool matching `url`.
fn job_value(state: &Arc<AppState>, tool: &str, url: &str) -> Option<Value> {
    for v in state.jobs.list() {
        if v.tool != tool || v.status != JobStatus::Succeeded || !v.has_result {
            continue;
        }
        let pu = v.params.get("url").map(|s| s.as_str()).unwrap_or("");
        if !same_url(pu, url) {
            continue;
        }
        if let Some((_, Some(r), _, _)) = state.jobs.result(&v.id) {
            return Some(r);
        }
    }
    None
}

fn source_value(state: &Arc<AppState>, tool: &str, url: &str) -> Option<Value> {
    if JOB_TOOLS.contains(&tool) {
        if let Some(v) = job_value(state, tool, url) {
            return Some(v);
        }
    }
    result_cache::get(tool, url)
}

/// Fingerprint of everything that could affect the aggregate for `url`.
fn fingerprint(state: &Arc<AppState>, url: &str) -> String {
    let mut parts = vec![format!("gen:{}", result_cache::generation())];
    for tool in JOB_TOOLS {
        let mut sig = "none".to_string();
        for v in state.jobs.list() {
            if v.tool != *tool || !v.has_result {
                continue;
            }
            let pu = v.params.get("url").map(|s| s.as_str()).unwrap_or("");
            if !same_url(pu, url) {
                continue;
            }
            sig = format!("{}:{}:{}", v.id, v.status.as_str(), v.finished_at.clone().unwrap_or_default());
            break;
        }
        parts.push(format!("{tool}:{sig}"));
    }
    parts.join("|")
}

// ─── Node builders ──────────────────────────────────────────────────────────

fn finish_node(
    id: &str,
    label: &str,
    category: &str,
    note: &str,
    items: Vec<AttackSurfaceItem>,
) -> AttackSurfaceNode {
    let status = items
        .iter()
        .map(|i| sev_rank(&i.severity))
        .max()
        .map(rank_sev)
        .unwrap_or("none");
    AttackSurfaceNode {
        id: id.into(),
        label: label.into(),
        category: category.into(),
        status: status.into(),
        count: items.len(),
        note: note.into(),
        items,
    }
}

fn core_node(check: &Option<Value>) -> AttackSurfaceNode {
    let mut items = Vec::new();
    let mut detected = false;
    let mut version: Option<String> = None;
    if let Some(v) = check {
        detected = bool_at(v, "detected").unwrap_or(false);
        add(
            &mut items,
            "WordPress detected",
            Some(detected.to_string()),
            "informational",
            None,
            vec![],
        );
        version = str_at(v, "version").map(str::to_string);
        if let Some(ver) = &version {
            let src = str_at(v, "version_source").unwrap_or("").to_string();
            add(
                &mut items,
                "Version",
                Some(ver.clone()),
                "informational",
                if src.is_empty() { None } else { Some(format!("source: {src}")) },
                if src.is_empty() { vec![] } else { vec![["source".into(), src]] },
            );
        }
        if let Some(w) = str_at(v, "wp_json_version") {
            add(&mut items, "wp-json version", Some(w.to_string()), "informational", None, vec![]);
        }
        if let Some(g) = str_at(v, "generator_tag") {
            add(
                &mut items,
                "Generator tag",
                Some(g.to_string()),
                "informational",
                Some("HTML meta generator exposes the CMS + version".into()),
                vec![],
            );
        }
        if let Some(r) = bool_at(v, "readme_accessible") {
            add(
                &mut items,
                "readme.txt accessible",
                Some(r.to_string()),
                if r { "low" } else { "informational" },
                if r {
                    Some("Public readme.txt leaks the exact core/plugin versions".into())
                } else {
                    None
                },
                vec![],
            );
        }
    }
    let note = match (detected, &version) {
        (true, Some(ver)) => format!("WordPress {ver} detected"),
        (true, None) => "WordPress detected · version unknown".to_string(),
        _ => "Not fingerprinted".to_string(),
    };
    finish_node("core", "Core", "infrastructure", &note, items)
}

fn authentication_node(xmlrpc: &Option<Value>, users: &Option<Value>) -> AttackSurfaceNode {
    let mut items = Vec::new();
    if let Some(v) = xmlrpc {
        let avail = bool_at(v, "available").unwrap_or(false);
        add(
            &mut items,
            "XML-RPC endpoint",
            Some(if avail { "open".into() } else { "closed".into() }),
            if avail { "medium" } else { "informational" },
            if avail {
                Some("Legacy interface — amplification / brute-force vector".into())
            } else {
                None
            },
            vec![],
        );
        if let Some(mc) = usize_at(v, "method_count") {
            add(&mut items, "Methods exposed", Some(mc.to_string()), "informational", None, vec![]);
        }
        if let Some(m) = bool_at(v, "multicall") {
            add(
                &mut items,
                "system.multicall",
                Some(m.to_string()),
                if m { "high" } else { "informational" },
                if m { Some("Batched requests enable password-guessing amplification".into()) } else { None },
                vec![],
            );
        }
        if let Some(p) = bool_at(v, "pingback") {
            add(
                &mut items,
                "Pingback",
                Some(p.to_string()),
                if p { "high" } else { "informational" },
                if p { Some("pingback.ping → SSRF / reflection amplification".into()) } else { None },
                vec![],
            );
        }
        if let Some(sc) = bool_at(v, "system_get_capabilities") {
            add(
                &mut items,
                "system.getCapabilities",
                Some(sc.to_string()),
                if sc { "medium" } else { "informational" },
                if sc { Some("Can leak usernames to unauthenticated callers".into()) } else { None },
                vec![],
            );
        }
        for m in arr_str(v, "interesting") {
            add(&mut items, "Interesting method", Some(m), "medium", None, vec![]);
        }
    }
    if let Some(v) = users {
        let n = arr(v, "users").len();
        add(
            &mut items,
            "Users enumerated",
            Some(n.to_string()),
            if n > 0 { "medium" } else { "informational" },
            if n > 0 {
                Some("Public user enumeration feeds credential / phishing attacks".into())
            } else {
                None
            },
            vec![],
        );
        if let Some(r) = bool_at(v, "rest_users_enabled") {
            add(
                &mut items,
                "REST user endpoint open",
                Some(r.to_string()),
                if r { "medium" } else { "informational" },
                None,
                vec![],
            );
        }
        if let Some(a) = bool_at(v, "author_enum_works") {
            add(
                &mut items,
                "Author archive enumeration",
                Some(a.to_string()),
                if a { "medium" } else { "informational" },
                None,
                vec![],
            );
        }
    }
    let open = items
        .iter()
        .any(|i| i.label == "XML-RPC endpoint" && i.value.as_deref() == Some("open"));
    let note = if open {
        "XML-RPC open — extra auth surface".to_string()
    } else {
        "Authentication surfaces mapped".to_string()
    };
    finish_node("authentication", "Authentication", "authentication", &note, items)
}

fn rest_node(rest: &Option<Value>) -> AttackSurfaceNode {
    let mut items = Vec::new();
    let mut interesting = 0usize;
    if let Some(v) = rest {
        let avail = bool_at(v, "available").unwrap_or(false);
        add(
            &mut items,
            "REST API",
            Some(if avail { "enabled".into() } else { "disabled".into() }),
            "informational",
            None,
            vec![],
        );
        if let Some(rc) = usize_at(v, "route_count") {
            add(&mut items, "Route count", Some(rc.to_string()), "informational", None, vec![]);
        }
        let routes = arr_str(v, "interesting_routes");
        interesting = routes.len();
        if !routes.is_empty() {
            add(
                &mut items,
                "Interesting routes",
                Some(routes.len().to_string()),
                "high",
                Some("Routes exposing users / plugins / themes / settings / upload".into()),
                vec![],
            );
            for r in routes {
                add(&mut items, &r, Some("exposed".into()), "high", None, vec![]);
            }
        }
        for ns in arr_str(v, "namespaces") {
            add(&mut items, "Namespace", Some(ns), "informational", None, vec![]);
        }
        if let Some(w) = str_at(v, "wp_version") {
            add(&mut items, "REST-reported version", Some(w.to_string()), "informational", None, vec![]);
        }
    }
    let note = if interesting > 0 {
        format!("{interesting} interesting routes exposed")
    } else {
        "REST surface mapped".to_string()
    };
    finish_node("rest", "REST API", "rest", &note, items)
}

fn plugins_node(state: &Arc<AppState>, plugins: &Option<Value>) -> AttackSurfaceNode {
    let mut items = Vec::new();
    let mut vuln_total = 0usize;
    if let Some(v) = plugins {
        let hits = arr(v, "plugins");
        add(
            &mut items,
            "Plugins detected",
            Some(hits.len().to_string()),
            if hits.is_empty() { "informational" } else { "low" },
            None,
            vec![],
        );
        for h in hits {
            let slug = str_at(h, "slug").unwrap_or("?").to_string();
            let version = str_at(h, "version").map(str::to_string);
            let evidence = str_at(h, "evidence").unwrap_or("").to_string();
            let conf = h.get("confidence").and_then(|x| x.as_u64()).unwrap_or(0);
            let explain = str_at(h, "evidence_explainer").unwrap_or("").to_string();
            let mut meta = vec![
                ["evidence".into(), evidence],
                ["confidence".into(), format!("{conf}/100")],
                ["evidence_explainer".into(), explain],
            ];
            let mut vuln_count = 0usize;
            if let Some(vv) = &version {
                if let Ok(finds) = lookup_component_vulns(&state.repo_root, "plugin", &slug, vv) {
                    vuln_count = finds.len();
                    if vuln_count > 0 {
                        meta.push(["vulnerabilities".into(), vuln_count.to_string()]);
                    }
                }
                if let Some(repo) = cached_repo_meta("plugin", &slug) {
                    if let Some(latest) = &repo.version {
                        let outdated = latest != vv;
                        meta.push(["latest_version".into(), latest.clone()]);
                        meta.push(["outdated".into(), if outdated { "yes" } else { "no" }.into()]);
                    }
                }
            }
            vuln_total += vuln_count;
            let sev = if vuln_count > 0 {
                "high"
            } else if version.is_some() {
                "informational"
            } else {
                "low"
            };
            add(&mut items, &slug, version, sev, None, meta);
        }
    }
    let note = if vuln_total > 0 {
        format!("{vuln_total} known vulns across detected plugins")
    } else {
        "Installed plugins + versions".to_string()
    };
    finish_node("plugins", "Plugins", "plugins", &note, items)
}

fn themes_node(state: &Arc<AppState>, themes: &Option<Value>) -> AttackSurfaceNode {
    let mut items = Vec::new();
    let mut vuln_total = 0usize;
    if let Some(v) = themes {
        if let Some(a) = str_at(v, "active_guess") {
            add(
                &mut items,
                "Active theme (guess)",
                Some(a.to_string()),
                "informational",
                Some("Inferred from page assets — verify manually".into()),
                vec![],
            );
        }
        let hits = arr(v, "themes");
        add(
            &mut items,
            "Themes detected",
            Some(hits.len().to_string()),
            if hits.is_empty() { "informational" } else { "low" },
            None,
            vec![],
        );
        for h in hits {
            let slug = str_at(h, "slug").unwrap_or("?").to_string();
            let version = str_at(h, "version").map(str::to_string);
            let evidence = str_at(h, "evidence").unwrap_or("").to_string();
            let conf = h.get("confidence").and_then(|x| x.as_u64()).unwrap_or(0);
            let explain = str_at(h, "evidence_explainer").unwrap_or("").to_string();
            let mut meta = vec![
                ["evidence".into(), evidence],
                ["confidence".into(), format!("{conf}/100")],
                ["evidence_explainer".into(), explain],
            ];
            let mut vuln_count = 0usize;
            if let Some(vv) = &version {
                if let Ok(finds) = lookup_component_vulns(&state.repo_root, "theme", &slug, vv) {
                    vuln_count = finds.len();
                    if vuln_count > 0 {
                        meta.push(["vulnerabilities".into(), vuln_count.to_string()]);
                    }
                }
                if let Some(repo) = cached_repo_meta("theme", &slug) {
                    if let Some(latest) = &repo.version {
                        let outdated = latest != vv;
                        meta.push(["latest_version".into(), latest.clone()]);
                        meta.push(["outdated".into(), if outdated { "yes" } else { "no" }.into()]);
                    }
                }
            }
            vuln_total += vuln_count;
            let sev = if vuln_count > 0 {
                "high"
            } else if version.is_some() {
                "informational"
            } else {
                "low"
            };
            add(&mut items, &slug, version, sev, None, meta);
        }
    }
    let note = if vuln_total > 0 {
        format!("{vuln_total} known vulns across detected themes")
    } else {
        "Installed themes + versions".to_string()
    };
    finish_node("themes", "Themes", "themes", &note, items)
}

fn files_node(paths: &Option<Value>) -> AttackSurfaceNode {
    let mut items = Vec::new();
    let mut exposed = 0usize;
    if let Some(v) = paths {
        let findings = arr(v, "findings");
        exposed = findings.len();
        add(
            &mut items,
            "Sensitive files exposed",
            Some(findings.len().to_string()),
            if findings.is_empty() { "informational" } else { "medium" },
            None,
            vec![],
        );
        for f in findings {
            let path = str_at(f, "path").unwrap_or("?").to_string();
            let status = f.get("status").and_then(|x| x.as_u64()).map(|s| s.to_string());
            let risk = str_at(f, "risk").unwrap_or("").to_string();
            let note = str_at(f, "note").unwrap_or("").to_string();
            let length = f.get("length").and_then(|x| x.as_u64());
            let mut meta = vec![];
            if let Some(l) = length {
                meta.push(["length".into(), l.to_string()]);
            }
            if !risk.is_empty() {
                meta.push(["risk".into(), risk.clone()]);
            }
            add(
                &mut items,
                &path,
                status,
                sev(&risk),
                if note.is_empty() { None } else { Some(note) },
                meta,
            );
        }
        if let Some(p) = usize_at(v, "probed") {
            add(&mut items, "Paths probed", Some(p.to_string()), "informational", None, vec![]);
        }
    }
    let note = if exposed > 0 {
        format!("{exposed} sensitive files reachable")
    } else {
        "No exposed sensitive files".to_string()
    };
    finish_node("files", "Sensitive Files", "files", &note, items)
}

fn headers_node(check: &Option<Value>) -> AttackSurfaceNode {
    const HEADERS: &[(&str, &str, &str)] = &[
        ("content-security-policy", "Content-Security-Policy", "medium"),
        ("strict-transport-security", "Strict-Transport-Security", "medium"),
        ("x-content-type-options", "X-Content-Type-Options", "medium"),
        ("x-frame-options", "X-Frame-Options", "low"),
        ("referrer-policy", "Referrer-Policy", "low"),
        ("permissions-policy", "Permissions-Policy", "low"),
    ];
    let mut items = Vec::new();
    let mut missing = 0usize;
    if let Some(v) = check {
        if let Some(headers) = v.get("headers").and_then(|h| h.as_object()) {
            for (lk, disp, sev_s) in HEADERS {
                let found = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(lk))
                    .and_then(|(_, x)| x.as_str())
                    .map(str::to_string);
                match found {
                    Some(val) => add(
                        &mut items,
                        disp,
                        Some("present".into()),
                        "informational",
                        Some(format!("{disp}: {val}")),
                        vec![],
                    ),
                    None => {
                        missing += 1;
                        add(
                            &mut items,
                            disp,
                            Some("missing".into()),
                            sev_s,
                            Some(format!("No {disp} header set")),
                            vec![],
                        );
                    }
                }
            }
        }
    }
    let note = if missing > 0 {
        format!("{missing} security headers missing")
    } else {
        "Security headers present".to_string()
    };
    finish_node("headers", "Security Headers", "infrastructure", &note, items)
}

fn vulnerabilities_node(vuln: &Option<Value>, nuclei: &Option<Value>) -> AttackSurfaceNode {
    let mut items = Vec::new();
    let mut total = 0usize;
    if let Some(v) = vuln {
        let findings = arr(v, "findings");
        total += findings.len();
        for f in findings {
            let title = str_at(f, "title").unwrap_or("Unknown finding").to_string();
            let cve = str_at(f, "cve").map(str::to_string);
            let rating = str_at(f, "cvss_rating").map(str::to_string);
            let score = f64_at(f, "cvss_score");
            let s = rating
                .as_deref()
                .map(sev)
                .unwrap_or_else(|| score.map(score_sev).unwrap_or("informational"));
            let comp = str_at(f, "name")
                .map(str::to_string)
                .or_else(|| str_at(f, "slug").map(str::to_string))
                .unwrap_or_default();
            let ver = str_at(f, "detected_version").unwrap_or("").to_string();
            let patched = bool_at(f, "patched").unwrap_or(false);
            let mut meta = vec![];
            if let Some(c) = &cve {
                meta.push(["cve".into(), c.clone()]);
            }
            if let Some(sc) = score {
                meta.push(["cvss".into(), format!("{sc:.1}")]);
            }
            if !comp.is_empty() {
                meta.push(["component".into(), comp]);
            }
            if !ver.is_empty() {
                meta.push(["version".into(), ver]);
            }
            meta.push(["patched".into(), if patched { "yes" } else { "no" }.into()]);
            if let Some(r) = str_at(f, "remediation") {
                meta.push(["remediation".into(), r.to_string()]);
            }
            add(&mut items, &title, cve, s, None, meta);
        }
        if let Some(sum) = v.get("summary") {
            let crit = sum.get("critical").and_then(|x| x.as_u64()).unwrap_or(0);
            let high = sum.get("high").and_then(|x| x.as_u64()).unwrap_or(0);
            let med = sum.get("medium").and_then(|x| x.as_u64()).unwrap_or(0);
            let low = sum.get("low").and_then(|x| x.as_u64()).unwrap_or(0);
            add(
                &mut items,
                "Wordfence scan summary",
                Some(format!("critical {crit} · high {high} · medium {med} · low {low}")),
                "informational",
                None,
                vec![],
            );
        }
    }
    if let Some(v) = nuclei {
        let findings = arr(v, "findings");
        total += findings.len();
        for f in findings {
            let name = f
                .get("info")
                .and_then(|i| str_at(i, "name"))
                .unwrap_or("Nuclei finding")
                .to_string();
            let sev_s = f
                .get("info")
                .and_then(|i| str_at(i, "severity"))
                .unwrap_or("")
                .to_string();
            let s = sev(&sev_s);
            let matched = str_at(f, "matched-at").map(str::to_string);
            let tid = str_at(f, "template-id").map(str::to_string);
            let mut meta = vec![];
            if let Some(t) = &tid {
                meta.push(["template".into(), t.clone()]);
            }
            if let Some(m) = &matched {
                meta.push(["matched_at".into(), m.clone()]);
            }
            add(
                &mut items,
                &name,
                if sev_s.is_empty() { None } else { Some(sev_s.clone()) },
                s,
                matched,
                meta,
            );
        }
    }
    if items.is_empty() {
        add(&mut items, "No known vulnerabilities", None, "informational", None, vec![]);
    }
    let note = if total > 0 {
        format!("{total} findings (Wordfence + Nuclei)")
    } else {
        "No findings from available sources".to_string()
    };
    finish_node("vulnerabilities", "Vulnerabilities", "vulnerabilities", &note, items)
}

fn infrastructure_node(check: &Option<Value>) -> AttackSurfaceNode {
    const KEYS: &[&str] = &["server", "x-powered-by", "via", "x-cache", "content-type"];
    let mut items = Vec::new();
    if let Some(v) = check {
        if let Some(headers) = v.get("headers").and_then(|h| h.as_object()) {
            for k in KEYS {
                if let Some(val) = headers
                    .iter()
                    .find(|(k2, _)| k2.eq_ignore_ascii_case(k))
                    .and_then(|(_, x)| x.as_str())
                {
                    add(&mut items, k, Some(val.to_string()), "informational", None, vec![]);
                }
            }
        }
    }
    if items.is_empty() {
        add(&mut items, "Web server", Some("not fingerprinted".into()), "informational", None, vec![]);
    }
    finish_node("infrastructure", "Infrastructure", "infrastructure", "Host-level fingerprints", items)
}

// ─── Summary / missing ──────────────────────────────────────────────────────

fn summarize(nodes: &[AttackSurfaceNode], check: &Option<Value>) -> AttackSurfaceSummary {
    let core = check
        .as_ref()
        .and_then(|v| str_at(v, "version").map(str::to_string))
        .unwrap_or_else(|| if check.is_some() { "unknown".into() } else { "not scanned".into() });

    let node = |id: &str| nodes.iter().find(|n| n.id == id);
    let items = |id: &str| node(id).map(|n| &n.items).cloned().unwrap_or_default();

    let plugins = items("plugins")
        .iter()
        .filter(|i| i.label != "Plugins detected")
        .count();
    let themes = items("themes")
        .iter()
        .filter(|i| i.label != "Themes detected" && i.label != "Active theme (guess)")
        .count();
    let rest_routes = items("rest")
        .iter()
        .find(|i| i.label == "Route count")
        .and_then(|i| i.value.clone())
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let authentication = items("authentication")
        .iter()
        .filter(|i| i.severity != "informational")
        .count();
    let sensitive_files = items("files")
        .iter()
        .filter(|i| i.severity != "informational" && i.label != "Paths probed")
        .count();
    let known_vulns = items("vulnerabilities")
        .iter()
        .filter(|i| i.severity != "informational" && i.label != "Wordfence scan summary")
        .count();
    let total_findings = nodes
        .iter()
        .flat_map(|n| &n.items)
        .filter(|i| i.severity != "informational")
        .count();
    let overall = nodes
        .iter()
        .map(|n| sev_rank(&n.status))
        .max()
        .map(rank_sev)
        .unwrap_or("none")
        .to_string();

    AttackSurfaceSummary {
        core,
        plugins,
        themes,
        rest_routes,
        authentication,
        sensitive_files,
        known_vulns,
        total_findings,
        overall,
    }
}

fn missing_tools(values: &[(String, String, bool)]) -> Vec<MissingTool> {
    values
        .iter()
        .filter(|(_, _, present)| !*present)
        .map(|(tool, label, _)| MissingTool {
            tool: tool.clone(),
            label: label.clone(),
        })
        .collect()
}

// ─── Handler ────────────────────────────────────────────────────────────────

pub async fn attack_surface(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<AttackSurfaceResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let base = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let key = base.to_lowercase();
    let fp = fingerprint(&state, &base);

    {
        let guard = agg_cache().read().unwrap();
        if let Some(e) = guard.get(&key) {
            if e.fp == fp {
                return Ok(Json(e.resp.clone()));
            }
        }
    }

    let resp = build(&state, &base)?;
    agg_cache().write().unwrap().insert(key, AggEntry { fp, resp: resp.clone() });
    Ok(Json(resp))
}

struct AggEntry {
    fp: String,
    resp: AttackSurfaceResponse,
}

fn agg_cache() -> &'static RwLock<HashMap<String, AggEntry>> {
    static CACHE: OnceLock<RwLock<HashMap<String, AggEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn build(state: &Arc<AppState>, base: &str) -> Result<AttackSurfaceResponse, (StatusCode, String)> {
    let mut values: HashMap<&str, Option<Value>> = HashMap::new();
    for (tool, _) in SOURCES {
        values.insert(*tool, source_value(state, tool, base));
    }

    let check = values.get("wordpress-check").unwrap();
    let nodes = vec![
        core_node(check),
        authentication_node(
            values.get("wordpress-xmlrpc").unwrap(),
            values.get("wordpress-users").unwrap(),
        ),
        rest_node(values.get("wordpress-rest").unwrap()),
        plugins_node(state, values.get("wordpress-plugins").unwrap()),
        themes_node(state, values.get("wordpress-themes").unwrap()),
        files_node(values.get("wordpress-paths").unwrap()),
        headers_node(check),
        vulnerabilities_node(
            values.get("wordpress-vuln-scan").unwrap(),
            values.get("wordpress-nuclei").unwrap(),
        ),
        infrastructure_node(check),
    ];

    let summary = summarize(&nodes, check);

    let missing: Vec<MissingTool> = missing_tools(
        &SOURCES
            .iter()
            .map(|(tool, label)| {
                let present = values.get(*tool).and_then(|v| v.as_ref()).is_some();
                ((*tool).to_string(), (*label).to_string(), present)
            })
            .collect::<Vec<_>>(),
    );

    Ok(AttackSurfaceResponse {
        url: base.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        nodes,
        summary,
        missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn opt(v: Value) -> Option<Value> {
        Some(v)
    }

    #[test]
    fn severity_mapping_and_ranking() {
        assert_eq!(sev("critical"), "critical");
        assert_eq!(sev("HIGH"), "high");
        assert_eq!(sev("Medium"), "medium");
        assert_eq!(sev("low"), "low");
        assert_eq!(sev("info"), "informational");
        assert_eq!(sev(""), "informational");
        assert_eq!(score_sev(9.4), "critical");
        assert_eq!(score_sev(7.1), "high");
        assert_eq!(score_sev(5.0), "medium");
        assert_eq!(score_sev(1.2), "low");
        assert!(sev_rank("critical") > sev_rank("high"));
        assert!(sev_rank("high") > sev_rank("informational"));
        assert_eq!(rank_sev(sev_rank("medium")), "medium");
    }

    #[test]
    fn core_node_extracts_version_and_readme() {
        let check = opt(json!({
            "detected": true,
            "version": "6.8.1",
            "version_source": "readme_html",
            "readme_accessible": true,
            "generator_tag": "WordPress 6.8.1",
            "wp_json_version": "6.8.1",
            "headers": {}
        }));
        let node = core_node(&check);
        assert_eq!(node.id, "core");
        assert_eq!(node.status, "low"); // readme accessible
        let version = node.items.iter().find(|i| i.label == "Version").unwrap();
        assert_eq!(version.value.as_deref(), Some("6.8.1"));
        assert!(node.items.iter().any(|i| i.label == "readme.txt accessible"));
    }

    #[test]
    fn authentication_node_ranks_xmlrpc_and_users() {
        let xmlrpc = opt(json!({
            "available": true,
            "multicall": true,
            "pingback": true,
            "method_count": 3,
            "interesting": ["system.getCapabilities"]
        }));
        let users = opt(json!({
            "users": [{"id": 1, "slug": "admin"}],
            "rest_users_enabled": true,
            "author_enum_works": false
        }));
        let node = authentication_node(&xmlrpc, &users);
        assert_eq!(node.status, "high"); // pingback
        assert!(node.items.iter().any(|i| i.label == "Pingback" && i.severity == "high"));
        assert!(node.items.iter().any(|i| i.label == "Users enumerated" && i.severity == "medium"));
        assert!(node.items.iter().any(|i| i.label == "Interesting method"));
    }

    #[test]
    fn rest_node_flags_interesting_routes() {
        let rest = opt(json!({
            "available": true,
            "route_count": 40,
            "interesting_routes": ["/wp/v2/users", "/wp/v2/media"],
            "namespaces": ["wp/v2"]
        }));
        let node = rest_node(&rest);
        assert_eq!(node.status, "high");
        assert_eq!(node.items.iter().filter(|i| i.severity == "high").count(), 3);
    }

    #[test]
    fn files_node_maps_risk_to_severity() {
        let paths = opt(json!({
            "findings": [
                {"path": "/wp-config.php.bak", "status": 200, "risk": "High", "length": 512},
                {"path": "/debug.log", "status": 200, "risk": "Medium", "length": 100}
            ],
            "probed": 50
        }));
        let node = files_node(&paths);
        assert_eq!(node.status, "high");
        assert_eq!(node.count, 4);
    }

    #[test]
    fn vulnerabilities_node_aggregates_wf_and_nuclei() {
        let vuln = opt(json!({
            "findings": [
                {"title": "XSS in plugin", "cve": "CVE-2024-1", "cvss_rating": "high", "cvss_score": 7.5}
            ],
            "summary": {"critical": 0, "high": 1, "medium": 0, "low": 0}
        }));
        let nuclei = opt(json!({
            "findings": [
                {"info": {"name": "WP user enum", "severity": "medium"}, "template-id": "wp-users"}
            ]
        }));
        let node = vulnerabilities_node(&vuln, &nuclei);
        assert_eq!(node.status, "high");
        assert_eq!(node.items.iter().filter(|i| i.severity != "informational").count(), 2);
        assert!(node.items.iter().any(|i| i.label == "WP user enum" && i.severity == "medium"));
    }

    #[test]
    fn headers_node_marks_missing_headers() {
        let check = opt(json!({
            "headers": {"server": "nginx", "x-frame-options": "DENY"}
        }));
        let node = headers_node(&check);
        assert_eq!(node.status, "medium"); // csp/hsts/cto missing
        let present = node.items.iter().find(|i| i.label == "X-Frame-Options").unwrap();
        assert_eq!(present.value.as_deref(), Some("present"));
        let csp = node.items.iter().find(|i| i.label == "Content-Security-Policy").unwrap();
        assert_eq!(csp.value.as_deref(), Some("missing"));
        assert_eq!(csp.severity, "medium");
    }

    #[test]
    fn empty_sources_mark_all_missing() {
        let values = SOURCES
            .iter()
            .map(|(t, l)| ((*t).to_string(), (*l).to_string(), false))
            .collect::<Vec<_>>();
        let missing = missing_tools(&values);
        assert_eq!(missing.len(), SOURCES.len());
    }
}
