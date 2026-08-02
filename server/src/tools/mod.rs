//! Modular tools — each file is independent.
//!
//! To DELETE a tool:
//!   1. Remove `server/src/tools/<name>.rs`
//!   2. Remove its `mod` + route below
//!   3. Remove entry from `frontend/tools/registry.js`
//!   4. Optionally `brew uninstall <binary>`
//!
//! Categories:
//!   wordpress → CMS fingerprinting & recon
//!   web       → website probe / scan / fuzz
//!   local     → local filesystem secrets & vulns

pub mod attack_surface;
pub mod common;
pub mod component_intel;
pub mod cors_check;
pub mod cve_lookup;
pub mod ffuf;
pub mod findings;
pub mod gitleaks;
pub mod httpx;
pub mod js_analysis;
pub mod katana;
pub mod nuclei;
pub mod open_redirect;
pub mod result_cache;
pub mod status;
pub mod subfinder;
pub mod trivy;
pub mod waybackurls;
pub mod wordpress;
pub mod wordpress_nuclei;
pub mod wordpress_paths;
pub mod wordpress_plugins;
pub mod wordpress_rest;
pub mod wordpress_themes;
pub mod wordpress_users;
pub mod wordpress_vuln_scan;
pub mod wordpress_xmlrpc;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;

use crate::AppState;

/// All tool routes — merge into the main app.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/tools/status", get(status::tools_status))
        // Aggregation / visualization
        .route(
            "/api/tools/attack-surface",
            get(attack_surface::attack_surface),
        )
        // WordPress
        .route("/api/tools/wordpress-check", get(wordpress::wordpress_check))
        .route("/api/tools/wordpress-users", get(wordpress_users::wordpress_users))
        .route(
            "/api/tools/wordpress-plugins",
            get(wordpress_plugins::wordpress_plugins),
        )
        .route(
            "/api/tools/wordpress-themes",
            get(wordpress_themes::wordpress_themes),
        )
        .route(
            "/api/tools/component-intel",
            get(component_intel::component_intel),
        )
        .route(
            "/api/tools/wordpress-xmlrpc",
            get(wordpress_xmlrpc::wordpress_xmlrpc),
        )
        .route("/api/tools/wordpress-paths", get(wordpress_paths::wordpress_paths))
        .route("/api/tools/wordpress-rest", get(wordpress_rest::wordpress_rest))
        .route(
            "/api/tools/wordpress-nuclei",
            get(wordpress_nuclei::wordpress_nuclei),
        )
        .route(
            "/api/tools/wordpress-vuln-scan",
            get(wordpress_vuln_scan::wordpress_vuln_scan),
        )
        .route(
            "/api/tools/wordpress-vuln-detail",
            get(wordpress_vuln_scan::wordpress_vuln_detail),
        )
        .route(
            "/api/tools/wordpress-vuln-db/status",
            get(wordpress_vuln_scan::vuln_db_status),
        )
        .route(
            "/api/tools/wordpress-vuln-db/refresh",
            get(wordpress_vuln_scan::vuln_db_refresh),
        )
        // Websites
        .route("/api/tools/httpx", get(httpx::httpx_probe))
        .route("/api/tools/nuclei", get(nuclei::nuclei_scan))
        .route("/api/tools/ffuf", get(ffuf::ffuf_fuzz))
        .route("/api/tools/cors-check", get(cors_check::cors_check))
        .route("/api/tools/open-redirect", get(open_redirect::open_redirect))
        // Recon
        .route("/api/tools/subfinder", get(subfinder::subdomain_enum))
        .route("/api/tools/waybackurls", get(waybackurls::archive_urls))
        .route("/api/tools/katana", get(katana::crawl))
        .route("/api/tools/js-analysis", get(js_analysis::js_analysis))
        // Local files
        .route("/api/tools/gitleaks", get(gitleaks::gitleaks_scan))
        .route("/api/tools/trivy", get(trivy::trivy_scan))
        // Intel
        .route("/api/tools/cve-lookup", get(cve_lookup::cve_lookup))
        .route("/api/tools/findings", get(findings::list_findings))
        .route("/api/tools/findings", post(findings::create_finding))
        .route("/api/tools/findings/{id}", get(findings::get_finding))
        .route("/api/tools/findings/{id}", put(findings::update_finding))
        .route("/api/tools/findings/{id}", delete(findings::delete_finding))
}
