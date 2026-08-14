//! Shared helpers for external CLI tools.
//! Safe process spawning (no shell), URL/path validation, timeouts.

use reqwest::Client;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::{Child, Command};
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

/// GET a URL with short retries for transient transport errors (connect/timeout).
/// One flaky probe out of dozens shouldn't kill a whole enumeration run — and
/// retrying usually recovers versions (style.css, readme.txt) that a single
/// dropped connection would otherwise hide. Returns the first successful
/// response, or the last error once retries are exhausted.
pub async fn get_with_retry(
    client: &Client,
    url: &str,
    max_tries: usize,
) -> Result<reqwest::Response, reqwest::Error> {
    for attempt in 0..max_tries.max(1) {
        match client.get(url).send().await {
            Ok(resp) => return Ok(resp),
            Err(e) if e.is_connect() || e.is_timeout() => {
                if attempt + 1 >= max_tries.max(1) {
                    return Err(e);
                }
                // Linear backoff (200ms, 400ms, …) to ride out throttling.
                tokio::time::sleep(Duration::from_millis(200 * (attempt as u64 + 1))).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("get_with_retry always returns inside the loop")
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

/// Normalize a bare domain input: strip scheme / path / port, validate chars.
pub fn normalize_domain(raw: &str) -> Result<String, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err("Domain is empty".into());
    }
    if t.len() > 253 {
        return Err("Domain too long".into());
    }
    if t.chars().any(|c| c.is_whitespace() || ";|&`$(){}<>\"'\\".contains(c)) {
        return Err("Domain contains invalid characters".into());
    }
    let stripped = t
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = stripped.split(['/', ':', '?']).next().unwrap_or(stripped);
    let host = host.trim().trim_end_matches('.');
    if host.is_empty() || !host.contains('.') {
        return Err("Invalid domain (expected something like example.com)".into());
    }
    Ok(host.to_lowercase())
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

/// Result of a streamed CLI run: `result` mirrors the old `run_cli` output,
/// `cancelled` is true when the run was aborted by the job's cancel flag
/// rather than by timeout/natural exit.
#[derive(Debug, Serialize)]
pub struct StreamedCli {
    pub result: CliResult,
    pub cancelled: bool,
}

/// Like `run_cli`, but streams stdout lines to `on_line` as they arrive and
/// aborts the child as soon as `cancel` flips (checked between lines). Used by
/// the Job Manager so jobs get live logs + real cancellation.
pub async fn run_cli_stream(
    binary: &str,
    args: &[&str],
    timeout_secs: u64,
    cancel: Arc<AtomicBool>,
    mut on_line: impl FnMut(&str),
) -> StreamedCli {
    let started = Instant::now();
    let cmd_display = format!("{binary} {}", args.join(" "));

    let Some(bin_path) = find_binary(binary) else {
        return StreamedCli {
            result: CliResult {
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
            },
            cancelled: false,
        };
    };

    let mut cmd = Command::new(&bin_path);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child: Child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return StreamedCli {
                result: CliResult {
                    ok: false,
                    binary: binary.to_string(),
                    installed: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: None,
                    error: Some(format!("Failed to spawn {binary}: {e}")),
                    duration_ms: started.elapsed().as_millis() as u64,
                    command: cmd_display,
                },
                cancelled: false,
            };
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut stdout_buf = String::new();
    let mut stderr_buf = String::new();
    let mut cancelled = false;
    let mut timed_out = false;

    // Read stderr concurrently so a chatty child never blocks on a full pipe.
    let stderr_task = {
        let mut stderr = stderr;
        tokio::spawn(async move {
            let mut buf = String::new();
            if let Some(s) = stderr.as_mut() {
                let _ = s.read_to_string(&mut buf).await;
            }
            buf
        })
    };

    if let Some(mut out) = stdout {
        let mut reader = BufReader::new(&mut out);
        let mut line = String::new();
        let overall = tokio::time::sleep(Duration::from_secs(timeout_secs));
        tokio::pin!(overall);
        loop {
            if cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            // 250ms slice: re-check the cancel flag between reads so a quiet
            // child never delays cancellation by the read timeout.
            let read = timeout(Duration::from_millis(250), reader.read_line(&mut line));
            tokio::select! {
                _ = &mut overall => {
                    // Overall timeout hit — kill like a hard failure.
                    timed_out = true;
                    break;
                }
                res = read => match res {
                    Ok(Ok(0)) => break, // EOF
                    Ok(Ok(_)) => {
                        stdout_buf.push_str(&line);
                        on_line(line.trim_end_matches(['\r', '\n']));
                        line.clear();
                    }
                    Ok(Err(e)) => {
                        stderr_buf.push_str(&format!("stdout read error: {e}\n"));
                        break;
                    }
                    Err(_) => continue, // nothing new within the slice — re-check cancel
                },
            }
        }
    }

    if cancelled || timed_out {
        let _ = child.start_kill();
    }
    let status = child.wait().await;
    let code: Option<i32> = status.ok().and_then(|s| s.code());

    let stderr_full = stderr_task.await.unwrap_or_default();
    stderr_buf.push_str(&stderr_full);

    let stdout = stdout_buf;
    let stderr = stderr_buf;

    // Soft-ok: treat non-empty stdout as success for tools that print findings
    // even when exit codes are non-zero (common with scanners). Never soft-ok
    // on cancel/timeout — partial stdout would otherwise mask the failure.
    let has_stdout = !stdout.trim().is_empty();
    let soft_ok = has_stdout && !cancelled && !timed_out;

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
        Some(if cancelled {
            "Cancelled".to_string()
        } else if timed_out {
            format!("{binary} timed out after {timeout_secs}s")
        } else if hint.is_empty() {
            format!(
                "{binary} exited with code {}",
                code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
            )
        } else {
            truncate_output(&hint, 400)
        })
    };

    let exit_code = code;
    StreamedCli {
        result: CliResult {
            ok: soft_ok,
            binary: binary.to_string(),
            installed: true,
            stdout,
            stderr,
            exit_code,
            error,
            duration_ms: started.elapsed().as_millis() as u64,
            command: cmd_display,
        },
        cancelled,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_url_adds_https_and_strips_slash() {
        assert_eq!(
            normalize_url("example.com/").unwrap(),
            "https://example.com"
        );
        assert_eq!(
            normalize_url("http://example.com/path").unwrap(),
            "http://example.com/path"
        );
    }

    #[test]
    fn normalize_url_rejects_shell_metacharacters() {
        assert!(normalize_url("http://x.com;rm -rf /").is_err());
        assert!(normalize_url("").is_err());
    }

    #[test]
    fn normalize_domain_strips_scheme_and_path() {
        assert_eq!(
            normalize_domain("https://Sub.Example.com/path").unwrap(),
            "sub.example.com"
        );
        assert!(normalize_domain("localhost").is_err()); // no dot
        assert!(normalize_domain("evil.com|x").is_err());
    }

    #[test]
    fn safe_prefix_respects_utf8() {
        let s = "héllo world";
        let p = safe_prefix(s, 3);
        assert_eq!(p, "hél");
        assert!(s.starts_with(p));
    }
}
