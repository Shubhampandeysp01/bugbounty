//! Tool executor — the single execution path for every job-backed tool.
//!
//! Contract: a tool defines only *how to execute* (`build_args` / core fn) and
//! *how to parse output* (`parse_output` / serialize). Everything else
//! (spawn, logs, cancel, status, notifications) is handled by the Job Manager.
//!
//! `CliCtx` gives CLI tools access to app state (e.g. `repo_root` for wordlists
//! / scan paths) and a job-scoped scratch directory (e.g. ffuf's JSON output
//! file) shared between argument building and output parsing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::{Job, JobEvent, JobManager, JobStatus};
use crate::tools::common::{run_cli_stream, CliResult};
use crate::AppState;

/// Shared context for a CLI tool run.
pub struct CliCtx<'a> {
    pub state: &'a AppState,
    pub scratch: PathBuf,
}

/// Outcome of running a tool. `param` marks validation failures (→ HTTP 400
/// on the legacy endpoints); `cancelled` marks user-initiated cancellation.
type Outcome = Result<serde_json::Value, (bool, bool, String)>;

type BuildArgs = fn(&CliCtx, &HashMap<String, String>) -> Result<Vec<String>, String>;
type ParseOutput = fn(&CliCtx, &HashMap<String, String>, &CliResult) -> Result<serde_json::Value, String>;

/// Tools the Job Manager knows how to run. Sync request/response tools stay on
/// their direct endpoints for now (migrate here later for full uniformity).
pub fn is_known_tool(tool: &str) -> bool {
    matches!(
        tool,
        "subfinder-enum"
            | "waybackurls-mine"
            | "katana-crawl"
            | "httpx-probe"
            | "nuclei-scan"
            | "wordpress-nuclei"
            | "ffuf-fuzz"
            | "gitleaks-scan"
            | "trivy-scan"
            | "js-analysis"
            | "cors-check"
            | "wordpress-vuln-scan"
    )
}

/// Main entrypoint — runs the job to completion and publishes terminal events.
pub async fn execute(mgr: Arc<JobManager>, state: Arc<AppState>, job: Arc<Job>) {
    job.mark_started();
    job.set_status(JobStatus::Running);
    mgr.emit(JobEvent::Started { job: job.view() });

    let outcome = run_tool(&mgr, &state, &job).await;

    job.mark_finished();
    // Builtins don't poll the flag mid-run; if the user cancelled, report the
    // terminal state as Cancelled once the run finishes so the UI stays honest.
    let outcome = if job.is_cancelled() && outcome.is_ok() {
        Err((false, true, "Cancelled".into()))
    } else {
        outcome
    };
    match outcome {
        Ok(value) => {
            crate::tools::result_cache::store_job(&job.tool, &job.params, &value);
            job.set_result(value);
            job.set_status(JobStatus::Succeeded);
            mgr.emit(JobEvent::Completed { job: job.view() });
        }
        Err((_, true, _)) => {
            job.set_status(JobStatus::Cancelled);
            mgr.emit(JobEvent::Cancelled { job: job.view() });
        }
        Err((param, false, msg)) => {
            job.set_param_error(param);
            job.set_error(msg.clone());
            job.set_status(JobStatus::Failed);
            job.append_log(&format!("Failed: {msg}"));
            mgr.emit(JobEvent::Failed { job: job.view() });
        }
    }
    info_end(&job);
}

fn info_end(job: &Job) {
    tracing::info!("Job {} ended: {}", job.id, job.status().as_str());
}

/// Job-scoped scratch dir so tools can write intermediate files without
/// colliding across concurrent jobs.
fn scratch_dir(job: &Job) -> PathBuf {
    let base = std::env::temp_dir().join(format!("vault-job-{}", job.id));
    let _ = std::fs::create_dir_all(&base);
    base
}

async fn run_tool(mgr: &JobManager, state: &AppState, job: &Job) -> Outcome {
    let ctx = CliCtx {
        state,
        scratch: scratch_dir(job),
    };
    let outcome = match job.tool.as_str() {
        "subfinder-enum" => run_cli_tool(mgr, &ctx, job, "subfinder", 120, crate::tools::subfinder::build_args, crate::tools::subfinder::parse_output).await,
        "waybackurls-mine" => run_cli_tool(mgr, &ctx, job, "waybackurls", 120, crate::tools::waybackurls::build_args, crate::tools::waybackurls::parse_output).await,
        "katana-crawl" => run_cli_tool(mgr, &ctx, job, "katana", 150, crate::tools::katana::build_args, crate::tools::katana::parse_output).await,
        "httpx-probe" => run_cli_tool(mgr, &ctx, job, "httpx", 60, crate::tools::httpx::build_args, crate::tools::httpx::parse_output).await,
        "nuclei-scan" => run_cli_tool(mgr, &ctx, job, "nuclei", 120, crate::tools::nuclei::build_args, crate::tools::nuclei::parse_output).await,
        "wordpress-nuclei" => run_cli_tool(mgr, &ctx, job, "nuclei", 150, crate::tools::wordpress_nuclei::build_args, crate::tools::wordpress_nuclei::parse_output).await,
        "ffuf-fuzz" => run_cli_tool(mgr, &ctx, job, "ffuf", 120, crate::tools::ffuf::build_args, crate::tools::ffuf::parse_output).await,
        "gitleaks-scan" => run_cli_tool(mgr, &ctx, job, "gitleaks", 120, crate::tools::gitleaks::build_args, crate::tools::gitleaks::parse_output).await,
        "trivy-scan" => run_cli_tool(mgr, &ctx, job, "trivy", 180, crate::tools::trivy::build_args, crate::tools::trivy::parse_output).await,
        "js-analysis" => {
            job.append_log("Running JS analysis…");
            let resp = crate::tools::js_analysis::js_analysis_core(&job.params).await;
            builtin_outcome(resp).map(|v| log_builtin(job, &v))
        }
        "cors-check" => {
            job.append_log("Running CORS probes…");
            let resp = crate::tools::cors_check::cors_check_core(&job.params).await;
            builtin_outcome(resp).map(|v| log_builtin(job, &v))
        }
        "wordpress-vuln-scan" => {
            job.append_log("Running Wordfence vulnerability scan…");
            let resp = crate::tools::wordpress_vuln_scan::wordpress_vuln_scan_core(state, &job.params).await;
            builtin_outcome(resp).map(|v| log_builtin(job, &v))
        }
        other => Err((true, false, format!("Unknown tool: {other}"))),
    };
    let _ = std::fs::remove_dir_all(&ctx.scratch);
    outcome
}

/// Replay a builtin's `notes` into the job log after it completes.
fn log_builtin(job: &Job, value: &serde_json::Value) -> serde_json::Value {
    if let Some(notes) = value.get("notes").and_then(|n| n.as_array()) {
        for n in notes {
            if let Some(s) = n.as_str() {
                job.append_log(s);
            }
        }
    }
    value.clone()
}

/// Convert a builtin core result (which returns axum `(StatusCode, String)`
/// errors) into the executor `Outcome`.
fn builtin_outcome<T: serde::Serialize>(
    resp: Result<T, (axum::http::StatusCode, String)>,
) -> Outcome {
    match resp {
        Ok(v) => serde_json::to_value(v).map_err(|e| (false, false, e.to_string())),
        Err((status, msg)) => Err((status == axum::http::StatusCode::BAD_REQUEST, false, msg)),
    }
}

/// Runs a CLI tool via `run_cli_stream`, streaming each stdout line into the
/// job log + SSE bus, honoring cancellation, then parsing the captured output.
async fn run_cli_tool(
    mgr: &JobManager,
    ctx: &CliCtx<'_>,
    job: &Job,
    binary: &str,
    timeout_secs: u64,
    build_args: BuildArgs,
    parse_output: ParseOutput,
) -> Outcome {
    let args = match build_args(ctx, &job.params) {
        Ok(a) => a,
        Err(e) => return Err((true, false, e)),
    };
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    job.append_log(&format!("$ {} {}", binary, args.join(" ")));

    let cancel = job.cancel_handle();
    let streamed = run_cli_stream(binary, &arg_refs, timeout_secs, cancel, |line| {
        job.append_log(line);
        mgr.emit(JobEvent::Log {
            job_id: job.id.clone(),
            line: line.to_string(),
            ts: chrono::Utc::now().timestamp_millis() as u64,
        });
    })
    .await;

    if streamed.cancelled || job.is_cancelled() {
        return Err((false, true, "Cancelled".into()));
    }
    parse_output(ctx, &job.params, &streamed.result).map_err(|e| (false, false, e))
}
