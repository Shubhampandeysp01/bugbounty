//! Tool install status — which binaries are available on PATH.
//! The catalog here is the single source of truth for tool ids/labels.

use axum::response::Json;
use serde::Serialize;

use super::common::binary_installed;

#[derive(Debug, Serialize)]
pub struct ToolStatus {
    pub id: String,
    pub binary: String,
    pub category: String,
    pub label: String,
    pub installed: bool,
}

#[derive(Debug, Serialize)]
pub struct ToolsStatusResponse {
    pub tools: Vec<ToolStatus>,
}

/// `(id, binary, category, label)` — the authoritative tool catalog.
pub const TOOL_CATALOG: &[(&str, &str, &str, &str)] = &[
    // WordPress (builtin unless noted)
    ("wordpress-check", "", "wordpress", "WP Version Check"),
    ("wordpress-users", "", "wordpress", "User Enum"),
    ("wordpress-plugins", "", "wordpress", "Plugin Enum"),
    ("wordpress-themes", "", "wordpress", "Theme Enum"),
    ("wordpress-xmlrpc", "", "wordpress", "XML-RPC Probe"),
    ("wordpress-paths", "", "wordpress", "Sensitive Paths"),
    ("wordpress-rest", "", "wordpress", "REST Surface"),
    ("wordpress-nuclei", "nuclei", "wordpress", "WP Nuclei Scan"),
    ("wordpress-vuln-scan", "", "wordpress", "WF Vuln Scanner"),
    // Web
    ("httpx-probe", "httpx", "web", "Live Probe"),
    ("nuclei-scan", "nuclei", "web", "Vuln Scan"),
    ("ffuf-fuzz", "ffuf", "web", "Path Fuzz"),
    ("cors-check", "", "web", "CORS Check"),
    ("open-redirect", "", "web", "Open Redirect"),
    ("security-headers", "", "web", "Security Headers"),
    // Recon
    ("subfinder-enum", "subfinder", "recon", "Subdomain Enum"),
    ("waybackurls-mine", "waybackurls", "recon", "Archive URLs"),
    ("katana-crawl", "katana", "recon", "Crawler"),
    ("js-analysis", "", "recon", "JS Analysis"),
    // Local
    ("gitleaks-scan", "gitleaks", "local", "Secrets Scan"),
    ("trivy-scan", "trivy", "local", "FS Vuln Scan"),
    // Intel
    ("cve-lookup", "", "intel", "CVE Lookup"),
    ("findings-db", "", "intel", "Findings DB"),
];

/// Look up a tool's display label by id (used by the Job Manager).
pub fn tool_label(id: &str) -> Option<&'static str> {
    TOOL_CATALOG
        .iter()
        .find(|(tid, ..)| *tid == id)
        .map(|(_, _, _, label)| *label)
}

pub async fn tools_status() -> Json<ToolsStatusResponse> {
    let tools = TOOL_CATALOG
        .iter()
        .map(|(id, binary, category, label)| {
            let installed = if binary.is_empty() {
                true
            } else {
                binary_installed(binary)
            };
            ToolStatus {
                id: (*id).into(),
                binary: if binary.is_empty() {
                    "builtin".into()
                } else {
                    (*binary).into()
                },
                category: (*category).into(),
                label: (*label).into(),
                installed,
            }
        })
        .collect();

    Json(ToolsStatusResponse { tools })
}
