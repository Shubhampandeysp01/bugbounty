//! Websites → Path fuzz (ffuf) — DELETE this file + route + frontend registry to remove.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::AppState;

use super::common::{normalize_url, run_cli, truncate_output};

#[derive(Debug, Serialize)]
pub struct FfufResponse {
    pub url: String,
    pub installed: bool,
    pub findings: Vec<Value>,
    pub wordlist: String,
    pub raw: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

fn default_wordlist(repo_root: &std::path::Path) -> PathBuf {
    repo_root.join("tools/wordlists/common-paths.txt")
}

pub async fn ffuf_fuzz(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<FfufResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let base = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Target must include FUZZ or we append /FUZZ
    let target = if base.contains("FUZZ") {
        base
    } else {
        format!("{base}/FUZZ")
    };

    let wl_path = default_wordlist(&state.repo_root);
    if !wl_path.is_file() {
        return Ok(Json(FfufResponse {
            url: target,
            installed: true,
            findings: vec![],
            wordlist: wl_path.display().to_string(),
            raw: String::new(),
            error: Some(format!(
                "Wordlist missing at {}. Create tools/wordlists/common-paths.txt",
                wl_path.display()
            )),
            duration_ms: 0,
            command: String::new(),
        }));
    }

    let wl = wl_path.to_string_lossy().to_string();

    // Write JSON to a temp file (more reliable than /dev/stdout for ffuf)
    let out_path = std::env::temp_dir().join(format!(
        "vault-ffuf-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let out_str = out_path.to_string_lossy().to_string();

    let result = run_cli(
        "ffuf",
        &[
            "-u",
            &target,
            "-w",
            &wl,
            "-mc",
            "200,201,204,301,302,307,401,403",
            "-t",
            "25",
            "-timeout",
            "6",
            "-of",
            "json",
            "-o",
            &out_str,
            "-s",
        ],
        120,
    )
    .await;

    if !result.installed {
        let _ = std::fs::remove_file(&out_path);
        return Ok(Json(FfufResponse {
            url: target,
            installed: false,
            findings: vec![],
            wordlist: wl,
            raw: String::new(),
            error: result.error,
            duration_ms: result.duration_ms,
            command: result.command,
        }));
    }

    let file_body = std::fs::read_to_string(&out_path).unwrap_or_default();
    let _ = std::fs::remove_file(&out_path);

    let mut findings = Vec::new();
    if let Ok(v) = serde_json::from_str::<Value>(&file_body) {
        if let Some(arr) = v.get("results").and_then(|r| r.as_array()) {
            findings = arr.clone();
        }
    }

    let err = if findings.is_empty() && !result.ok && file_body.is_empty() {
        result.error.or_else(|| {
            if !result.stderr.is_empty() {
                Some(truncate_output(&result.stderr, 800))
            } else {
                None
            }
        })
    } else {
        None
    };

    Ok(Json(FfufResponse {
        url: target,
        installed: true,
        findings,
        wordlist: wl,
        raw: truncate_output(&file_body, 40_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command,
    }))
}
