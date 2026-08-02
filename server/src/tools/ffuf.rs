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

use crate::jobs::CliCtx;
use crate::AppState;

use super::common::{normalize_url, truncate_output, CliResult};

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

/// Where ffuf writes its JSON report — inside the job's scratch dir.
fn out_path(ctx: &CliCtx) -> PathBuf {
    ctx.scratch.join("ffuf-report.json")
}

/// Job-Manager contract: build the CLI argument vector from tool params.
pub fn build_args(ctx: &CliCtx, params: &HashMap<String, String>) -> Result<Vec<String>, String> {
    let url = params
        .get("url")
        .ok_or_else(|| "Missing 'url' parameter".to_string())?;
    let base = normalize_url(url)?;

    // Target must include FUZZ or we append /FUZZ
    let target = if base.contains("FUZZ") {
        base
    } else {
        format!("{base}/FUZZ")
    };

    let wl_path = default_wordlist(&ctx.state.repo_root);
    if !wl_path.is_file() {
        return Err(format!(
            "Wordlist missing at {}. Create tools/wordlists/common-paths.txt",
            wl_path.display()
        ));
    }

    let wl = wl_path.to_string_lossy().to_string();
    let out_str = out_path(ctx).to_string_lossy().to_string();

    Ok(vec![
        "-u".into(),
        target,
        "-w".into(),
        wl,
        "-mc".into(),
        "200,201,204,301,302,307,401,403".into(),
        "-t".into(),
        "25".into(),
        "-timeout".into(),
        "6".into(),
        "-of".into(),
        "json".into(),
        "-o".into(),
        out_str,
        "-s".into(),
    ])
}

/// Job-Manager contract: turn a `CliResult` into the renderer's JSON.
pub fn parse_output(
    ctx: &CliCtx,
    params: &HashMap<String, String>,
    result: &CliResult,
) -> Result<serde_json::Value, String> {
    let url = params
        .get("url")
        .ok_or_else(|| "Missing 'url' parameter".to_string())?;
    let base = normalize_url(url)?;
    let target = if base.contains("FUZZ") {
        base
    } else {
        format!("{base}/FUZZ")
    };
    let wl = default_wordlist(&ctx.state.repo_root).to_string_lossy().to_string();

    if !result.installed {
        return serde_json::to_value(FfufResponse {
            url: target,
            installed: false,
            findings: vec![],
            wordlist: wl,
            raw: String::new(),
            error: result.error.clone(),
            duration_ms: result.duration_ms,
            command: result.command.clone(),
        })
        .map_err(|e| e.to_string());
    }

    let file_body = std::fs::read_to_string(out_path(ctx)).unwrap_or_default();

    let mut findings = Vec::new();
    if let Ok(v) = serde_json::from_str::<Value>(&file_body) {
        if let Some(arr) = v.get("results").and_then(|r| r.as_array()) {
            findings = arr.clone();
        }
    }

    let err = if findings.is_empty() && !result.ok && file_body.is_empty() {
        result.error.clone().or_else(|| {
            if !result.stderr.is_empty() {
                Some(truncate_output(&result.stderr, 800))
            } else {
                None
            }
        })
    } else {
        None
    };

    serde_json::to_value(FfufResponse {
        url: target,
        installed: true,
        findings,
        wordlist: wl,
        raw: truncate_output(&file_body, 40_000),
        error: err,
        duration_ms: result.duration_ms,
        command: result.command.clone(),
    })
    .map_err(|e| e.to_string())
}

pub async fn ffuf_fuzz(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(
        crate::jobs::run_sync(&state, "ffuf-fuzz", params).await?,
    ))
}
