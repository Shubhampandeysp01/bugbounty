//! WordPress → Sensitive path discovery — DELETE file + route + registry to remove.
//! Finds backups, debug logs, exposed config artifacts, installer leftovers.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::HashMap;

use super::common::{http_client, normalize_url};

#[derive(Debug, Serialize)]
pub struct WpPathHit {
    pub path: String,
    pub status: u16,
    pub length: usize,
    pub risk: String,
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct WpPathsResponse {
    pub url: String,
    pub findings: Vec<WpPathHit>,
    pub probed: usize,
    pub notes: Vec<String>,
    pub error: Option<String>,
}

struct PathSpec {
    path: &'static str,
    risk: &'static str,
    note: &'static str,
    /// If true, only report on 200 with non-empty / interesting body
    need_body_hint: Option<&'static str>,
}

const PATHS: &[PathSpec] = &[
    PathSpec {
        path: "/wp-config.php",
        risk: "critical",
        note: "Config file (should never be readable)",
        need_body_hint: Some("DB_"),
    },
    PathSpec {
        path: "/wp-config.php.bak",
        risk: "critical",
        note: "Config backup",
        need_body_hint: Some("DB_"),
    },
    PathSpec {
        path: "/wp-config.php.old",
        risk: "critical",
        note: "Config backup",
        need_body_hint: Some("DB_"),
    },
    PathSpec {
        path: "/wp-config.php.save",
        risk: "critical",
        note: "Config backup",
        need_body_hint: Some("DB_"),
    },
    PathSpec {
        path: "/wp-config.php~",
        risk: "critical",
        note: "Editor backup",
        need_body_hint: Some("DB_"),
    },
    PathSpec {
        path: "/wp-config.php.swp",
        risk: "critical",
        note: "Vim swap of config",
        need_body_hint: None,
    },
    PathSpec {
        path: "/.wp-config.php.swp",
        risk: "critical",
        note: "Vim swap of config",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-config.txt",
        risk: "critical",
        note: "Config as text",
        need_body_hint: Some("DB_"),
    },
    PathSpec {
        path: "/wp-config.bak",
        risk: "critical",
        note: "Config bak",
        need_body_hint: Some("DB_"),
    },
    PathSpec {
        path: "/wp-content/debug.log",
        risk: "high",
        note: "Debug log may leak paths/secrets",
        need_body_hint: None,
    },
    PathSpec {
        path: "/debug.log",
        risk: "high",
        note: "Root debug log",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-content/uploads/",
        risk: "info",
        note: "Uploads dir (listing?)",
        need_body_hint: Some("Index of"),
    },
    PathSpec {
        path: "/wp-content/backup-db/",
        risk: "high",
        note: "DB backup directory",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-content/backups/",
        risk: "high",
        note: "Backups directory",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-content/uploads/backup/",
        risk: "high",
        note: "Upload backups",
        need_body_hint: None,
    },
    PathSpec {
        path: "/backup.sql",
        risk: "critical",
        note: "SQL dump at web root",
        need_body_hint: Some("INSERT"),
    },
    PathSpec {
        path: "/database.sql",
        risk: "critical",
        note: "SQL dump",
        need_body_hint: Some("INSERT"),
    },
    PathSpec {
        path: "/dump.sql",
        risk: "critical",
        note: "SQL dump",
        need_body_hint: Some("INSERT"),
    },
    PathSpec {
        path: "/wordpress.sql",
        risk: "critical",
        note: "WP SQL dump",
        need_body_hint: Some("INSERT"),
    },
    PathSpec {
        path: "/.git/HEAD",
        risk: "high",
        note: "Git metadata exposed",
        need_body_hint: Some("ref:"),
    },
    PathSpec {
        path: "/.svn/entries",
        risk: "high",
        note: "SVN metadata",
        need_body_hint: None,
    },
    PathSpec {
        path: "/.env",
        risk: "critical",
        note: "Environment secrets",
        need_body_hint: Some("="),
    },
    PathSpec {
        path: "/wp-admin/install.php",
        risk: "medium",
        note: "Installer still reachable",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-admin/setup-config.php",
        risk: "medium",
        note: "Setup config reachable",
        need_body_hint: None,
    },
    PathSpec {
        path: "/readme.html",
        risk: "low",
        note: "Default readme (version leak)",
        need_body_hint: Some("WordPress"),
    },
    PathSpec {
        path: "/license.txt",
        risk: "info",
        note: "License file present",
        need_body_hint: Some("WordPress"),
    },
    PathSpec {
        path: "/wp-includes/version.php",
        risk: "medium",
        note: "version.php should not be directly useful",
        need_body_hint: Some("$wp_version"),
    },
    PathSpec {
        path: "/wp-json/",
        risk: "info",
        note: "REST API root",
        need_body_hint: Some("namespaces"),
    },
    PathSpec {
        path: "/?rest_route=/",
        risk: "info",
        note: "REST via query arg",
        need_body_hint: Some("namespaces"),
    },
    PathSpec {
        path: "/wp-cron.php",
        risk: "info",
        note: "Cron endpoint",
        need_body_hint: None,
    },
    PathSpec {
        path: "/xmlrpc.php",
        risk: "medium",
        note: "XML-RPC endpoint",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-login.php",
        risk: "info",
        note: "Login page",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-admin/",
        risk: "info",
        note: "Admin panel",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-content/plugins/",
        risk: "low",
        note: "Plugins directory listing?",
        need_body_hint: Some("Index of"),
    },
    PathSpec {
        path: "/wp-content/themes/",
        risk: "low",
        note: "Themes directory listing?",
        need_body_hint: Some("Index of"),
    },
    PathSpec {
        path: "/wp-content/uploads/wp-file-manager-pro/fm_backup/",
        risk: "critical",
        note: "File Manager backup path",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-content/ai1wm-backups/",
        risk: "critical",
        note: "All-in-One WP Migration backups",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-content/updraft/",
        risk: "critical",
        note: "UpdraftPlus backups",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-content/uploads/wpvividbackups/",
        risk: "critical",
        note: "WPvivid backups",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-content/backupwordpress/",
        risk: "high",
        note: "BackupWordPress dir",
        need_body_hint: None,
    },
    PathSpec {
        path: "/phpinfo.php",
        risk: "high",
        note: "phpinfo exposed",
        need_body_hint: Some("PHP Version"),
    },
    PathSpec {
        path: "/info.php",
        risk: "high",
        note: "phpinfo exposed",
        need_body_hint: Some("PHP Version"),
    },
    PathSpec {
        path: "/.user.ini",
        risk: "medium",
        note: "PHP user.ini",
        need_body_hint: None,
    },
    PathSpec {
        path: "/wp-content/mu-plugins/",
        risk: "info",
        note: "Must-use plugins dir",
        need_body_hint: Some("Index of"),
    },
];

pub async fn wordpress_paths(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<WpPathsResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let base = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let client = http_client(8).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut findings = Vec::new();
    let mut notes = Vec::new();
    let mut error = None;

    for spec in PATHS {
        let full = format!("{base}{}", spec.path);
        match client.get(&full).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                // Interesting statuses
                if !(status == 200 || status == 403 || status == 401 || status == 301 || status == 302)
                {
                    continue;
                }
                let body = resp.text().await.unwrap_or_default();
                let length = body.len();

                // Soft-404 filter: tiny or login redirect pages for sensitive files
                if status == 200 {
                    if let Some(hint) = spec.need_body_hint {
                        if !body.contains(hint) {
                            // For directory listing paths, skip without hint
                            if matches!(
                                spec.risk,
                                "critical" | "high" | "medium" | "low" | "info"
                            ) && (hint == "Index of"
                                || hint == "DB_"
                                || hint == "INSERT"
                                || hint == "ref:"
                                || hint == "PHP Version"
                                || hint == "$wp_version")
                            {
                                continue;
                            }
                            // info endpoints without exact hint still ok if length reasonable
                            if matches!(spec.risk, "critical" | "high") {
                                continue;
                            }
                        }
                    }
                    // Generic soft 404: very small HTML error pages
                    if length < 40 && !spec.path.contains("wp-cron") {
                        continue;
                    }
                }

                // 403 on sensitive backup paths still worth reporting
                if status == 403
                    && !matches!(
                        spec.path,
                        "/wp-admin/" | "/wp-login.php" | "/wp-content/plugins/" | "/wp-content/themes/"
                    )
                {
                    // report blocked-but-exists for backups
                    if matches!(spec.risk, "critical" | "high") {
                        findings.push(WpPathHit {
                            path: spec.path.to_string(),
                            status,
                            length,
                            risk: spec.risk.into(),
                            note: format!("{} (forbidden — may still exist)", spec.note),
                        });
                    }
                    continue;
                }

                if status == 200 || status == 401 {
                    findings.push(WpPathHit {
                        path: spec.path.to_string(),
                        status,
                        length,
                        risk: spec.risk.into(),
                        note: spec.note.into(),
                    });
                }
            }
            Err(e) => {
                if error.is_none() {
                    error = Some(format!("Request error: {e}"));
                }
            }
        }
    }

    // Sort by risk
    let rank = |r: &str| match r {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    };
    findings.sort_by(|a, b| rank(&a.risk).cmp(&rank(&b.risk)).then(a.path.cmp(&b.path)));

    notes.push(format!("Probed {} sensitive WordPress paths", PATHS.len()));
    notes.push(format!("{} interesting responses", findings.len()));

    let resp = WpPathsResponse {
        url: base,
        findings,
        probed: PATHS.len(),
        notes,
        error,
    };
    super::result_cache::store("wordpress-paths", &resp.url, &resp);
    Ok(Json(resp))
}
