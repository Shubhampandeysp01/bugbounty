//! Tool install status — which binaries are available on PATH.

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

pub async fn tools_status() -> Json<ToolsStatusResponse> {
    let catalog = [
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
        // Recon
        ("subfinder-enum", "subfinder", "recon", "Subdomain Enum"),
        ("waybackurls-mine", "waybackurls", "recon", "Archive URLs"),
        ("katana-crawl", "katana", "recon", "Crawler"),
        ("js-analysis", "", "recon", "JS Analysis"),
        // Local
        ("gitleaks-scan", "gitleaks", "local", "Secrets Scan"),
        ("trivy-scan", "trivy", "local", "FS Vuln Scan"),
    ];

    let tools = catalog
        .into_iter()
        .map(|(id, binary, category, label)| {
            let installed = if binary.is_empty() {
                true
            } else {
                binary_installed(binary)
            };
            ToolStatus {
                id: id.into(),
                binary: if binary.is_empty() {
                    "builtin".into()
                } else {
                    binary.into()
                },
                category: category.into(),
                label: label.into(),
                installed,
            }
        })
        .collect();

    Json(ToolsStatusResponse { tools })
}
