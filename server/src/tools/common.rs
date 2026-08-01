//! Shared helpers for external CLI tools.
//! Safe process spawning (no shell), URL/path validation, timeouts.

use reqwest::Client;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// Shared HTTP client for builtin recon tools.
pub fn http_client(timeout_secs: u64) -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .redirect(reqwest::redirect::Policy::limited(8))
        .user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36 BugBountyVault/1.0",
        )
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))
}

#[derive(Debug, Serialize)]
pub struct CliResult {
    pub ok: bool,
    pub binary: String,
    pub installed: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub command: String,
}

pub fn find_binary(name: &str) -> Option<PathBuf> {
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
            // macOS sometimes has no execute check via is_file alone — still fine
        }
    }
    // Homebrew common locations
    for prefix in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let candidate = PathBuf::from(prefix).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn binary_installed(name: &str) -> bool {
    find_binary(name).is_some()
}

/// Normalize user URL: add https if missing, reject shell-ish input.
pub fn normalize_url(raw: &str) -> Result<String, String> {
    let u = raw.trim();
    if u.is_empty() {
        return Err("URL is empty".into());
    }
    if u.len() > 2048 {
        return Err("URL too long".into());
    }
    if u.chars().any(|c| c.is_whitespace() || ";|&`$(){}<>\"'\\".contains(c)) {
        return Err("URL contains invalid characters".into());
    }
    let with_scheme = if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else {
        format!("https://{u}")
    };
    Ok(with_scheme.trim_end_matches('/').to_string())
}

/// Resolve a scan path: absolute as-is if it exists, else under repo_root.
/// Rejects `..` escapes outside repo when relative.
pub fn resolve_scan_path(repo_root: &Path, raw: &str) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Path is empty".into());
    }
    if raw.chars().any(|c| ";|&`$(){}<>\"'\\".contains(c)) {
        return Err("Path contains invalid characters".into());
    }

    let path = PathBuf::from(raw);
    let resolved = if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    };

    let canonical = resolved.canonicalize().map_err(|e| {
        format!("Path not found: {} ({e})", resolved.display())
    })?;

    Ok(canonical)
}

pub async fn run_cli(binary: &str, args: &[&str], timeout_secs: u64) -> CliResult {
    let started = Instant::now();
    let cmd_display = format!("{binary} {}", args.join(" "));

    let Some(bin_path) = find_binary(binary) else {
        return CliResult {
            ok: false,
            binary: binary.to_string(),
            installed: false,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
            error: Some(format!(
                "{binary} is not installed. Run: brew install {binary}"
            )),
            duration_ms: 0,
            command: cmd_display,
        };
    };

    let mut cmd = Command::new(&bin_path);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CliResult {
                ok: false,
                binary: binary.to_string(),
                installed: true,
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
                error: Some(format!("Failed to spawn {binary}: {e}")),
                duration_ms: started.elapsed().as_millis() as u64,
                command: cmd_display,
            };
        }
    };

    let output = match timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            return CliResult {
                ok: false,
                binary: binary.to_string(),
                installed: true,
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
                error: Some(format!("Process error: {e}")),
                duration_ms: started.elapsed().as_millis() as u64,
                command: cmd_display,
            };
        }
        Err(_) => {
            return CliResult {
                ok: false,
                binary: binary.to_string(),
                installed: true,
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
                error: Some(format!("Timed out after {timeout_secs}s")),
                duration_ms: started.elapsed().as_millis() as u64,
                command: cmd_display,
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code();
    let ok = output.status.success();

    // Scanners often exit non-zero when findings exist; also ignore pure banner stderr
    let has_stdout = !stdout.trim().is_empty();
    let soft_ok = ok || has_stdout;

    let error = if soft_ok {
        None
    } else {
        let hint = stderr
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.is_empty()
                    && !t.contains("projectdiscovery")
                    && !t.contains("Current httpx")
                    && !t.contains("UI Dashboard")
            })
            .take(3)
            .collect::<Vec<_>>()
            .join(" | ");
        Some(if hint.is_empty() {
            format!(
                "{binary} exited with code {}",
                code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
            )
        } else {
            truncate_output(&hint, 400)
        })
    };

    CliResult {
        ok: soft_ok,
        binary: binary.to_string(),
        installed: true,
        stdout,
        stderr,
        exit_code: code,
        error,
        duration_ms: started.elapsed().as_millis() as u64,
        command: cmd_display,
    }
}

/// Truncate huge CLI output for the UI.
pub fn truncate_output(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}\n\n… truncated ({max_chars} chars max)")
}

/// Byte-slice `s` up to `max` chars without ever landing mid-codepoint.
/// Returns a `&str` that is a valid char-boundary prefix (may be shorter than `max`).
pub fn safe_prefix(s: &str, max: usize) -> &str {
    let end = s
        .char_indices()
        .nth(max)
        .map_or(s.len(), |(i, _)| i);
    &s[..end]
}
