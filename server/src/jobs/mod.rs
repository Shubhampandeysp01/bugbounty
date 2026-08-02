//! Job Manager — runs tools in the background with live logs, cancellation,
//! status tracking, and a typed SSE event stream.
//!
//! A "tool" only defines *how to execute* and *how to parse output* (see
//! `executor.rs`); everything else — spawning, logging, cancellation, status,
//! notifications — lives here.

mod executor;

pub use executor::CliCtx;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    response::Sse,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use futures_util::stream::Stream;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{info, warn};

use crate::AppState;

/// Maximum number of log lines kept on a finished job (ring buffer).
const LOG_RING: usize = 2000;
/// Finished jobs are evicted after this long (session-only, in-memory).
const EVICT_AFTER: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
        )
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Succeeded => "succeeded",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }
}

/// A background tool run. Log lines are kept in a bounded ring buffer so the
/// result endpoint can replay the tail after the process exits.
pub struct Job {
    pub id: String,
    pub tool: String,
    pub label: String,
    pub params: HashMap<String, String>,
    status: RwLock<JobStatus>,
    started_at: RwLock<Option<chrono::DateTime<Utc>>>,
    finished_at: RwLock<Option<chrono::DateTime<Utc>>>,
    error: RwLock<Option<String>>,
    result: RwLock<Option<Value>>,
    log: RwLock<VecDeque<String>>,
    cancel_flag: Arc<AtomicBool>,
    param_error: AtomicBool,
}

impl Job {
    fn status(&self) -> JobStatus {
        *self.status.read().unwrap()
    }
    fn set_status(&self, s: JobStatus) {
        *self.status.write().unwrap() = s;
    }
    fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }
    /// Clone of the cancellation flag so `run_cli_stream` observes flips.
    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        self.cancel_flag.clone()
    }
    fn set_param_error(&self, v: bool) {
        self.param_error.store(v, Ordering::SeqCst);
    }
    fn started_ms(&self) -> Option<u64> {
        let (s, f) = (
            *self.started_at.read().unwrap(),
            *self.finished_at.read().unwrap(),
        );
        match (s, f) {
            (Some(s), Some(f)) => Some(f.signed_duration_since(s).num_milliseconds().max(0) as u64),
            _ => None,
        }
    }
    fn mark_started(&self) {
        *self.started_at.write().unwrap() = Some(Utc::now());
    }
    fn mark_finished(&self) {
        *self.finished_at.write().unwrap() = Some(Utc::now());
    }
    fn set_result(&self, v: Value) {
        *self.result.write().unwrap() = Some(v);
    }
    fn set_error(&self, e: String) {
        *self.error.write().unwrap() = Some(e);
    }
    fn append_log(&self, line: &str) {
        let mut log = self.log.write().unwrap();
        log.push_back(line.to_string());
        while log.len() > LOG_RING {
            log.pop_front();
        }
    }
    fn log_snapshot(&self) -> Vec<String> {
        self.log.read().unwrap().iter().cloned().collect()
    }
    pub fn view(&self) -> JobView {
        JobView {
            id: self.id.clone(),
            tool: self.tool.clone(),
            label: self.label.clone(),
            status: self.status(),
            params: self.params.clone(),
            started_at: self
                .started_at
                .read()
                .unwrap()
                .map(|t| t.to_rfc3339()),
            finished_at: self
                .finished_at
                .read()
                .unwrap()
                .map(|t| t.to_rfc3339()),
            duration_ms: self.started_ms(),
            error: self.error.read().unwrap().clone(),
            has_result: self.result.read().unwrap().is_some(),
            has_log: !self.log.read().unwrap().is_empty(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JobView {
    pub id: String,
    pub tool: String,
    pub label: String,
    pub status: JobStatus,
    pub params: HashMap<String, String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub has_result: bool,
    pub has_log: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobEvent {
    Created { job: JobView },
    Started { job: JobView },
    Progress { job_id: String, progress: Option<f32>, message: Option<String> },
    Log { job_id: String, line: String, ts: u64 },
    Completed { job: JobView },
    Failed { job: JobView },
    Cancelled { job: JobView },
}

impl JobEvent {
    /// `(event_name, json_data)` for the SSE wire format.
    fn sse_parts(&self) -> (String, String) {
        let name = match self {
            JobEvent::Created { .. } => "job.created",
            JobEvent::Started { .. } => "job.started",
            JobEvent::Progress { .. } => "job.progress",
            JobEvent::Log { .. } => "job.log",
            JobEvent::Completed { .. } => "job.completed",
            JobEvent::Failed { .. } => "job.failed",
            JobEvent::Cancelled { .. } => "job.cancelled",
        };
        (name.to_string(), serde_json::to_string(self).unwrap())
    }
}

pub struct JobManager {
    jobs: Arc<RwLock<HashMap<String, Arc<Job>>>>,
    next_id: AtomicU64,
    events: broadcast::Sender<JobEvent>,
}

impl JobManager {
    pub fn new() -> Arc<Self> {
        let (events, _) = broadcast::channel(512);
        Arc::new(Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            events,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.events.subscribe()
    }

    fn emit(&self, evt: JobEvent) {
        // Ignore send errors (no subscribers) — this is a notification bus.
        let _ = self.events.send(evt);
    }

    /// Create a job for `tool` and spawn its executor. Returns the job view.
    pub fn submit(
        self: &Arc<Self>,
        state: Arc<AppState>,
        tool: &str,
        params: HashMap<String, String>,
    ) -> Result<JobView, (StatusCode, String)> {
        let label = crate::tools::status::tool_label(tool)
            .map(str::to_string)
            .unwrap_or_else(|| tool.to_string());
        if !executor::is_known_tool(tool) {
            return Err((StatusCode::BAD_REQUEST, format!("Unknown tool: {tool}")));
        }

        self.evict_finished();

        let id = format!("job-{:06}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let job = Arc::new(Job {
            id: id.clone(),
            tool: tool.to_string(),
            label,
            params,
            status: RwLock::new(JobStatus::Queued),
            started_at: RwLock::new(None),
            finished_at: RwLock::new(None),
            error: RwLock::new(None),
            result: RwLock::new(None),
            log: RwLock::new(VecDeque::new()),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            param_error: AtomicBool::new(false),
        });
        self.jobs.write().unwrap().insert(id.clone(), job.clone());

        let view = job.view();
        self.emit(JobEvent::Created { job: view.clone() });

        let mgr = self.clone();
        tokio::spawn(async move {
            executor::execute(mgr, state, job.clone()).await;
        });
        info!("Job {} started: {}", id, tool);
        Ok(view)
    }

    pub fn list(&self) -> Vec<JobView> {
        self.evict_finished();
        let jobs = self.jobs.read().unwrap();
        let mut views: Vec<JobView> = jobs.values().map(|j| j.view()).collect();
        views.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        views
    }

    pub fn get(&self, id: &str) -> Option<JobView> {
        self.jobs.read().unwrap().get(id).map(|j| j.view())
    }

    pub fn result(&self, id: &str) -> Option<(JobStatus, Option<Value>, Option<String>)> {
        let j = {
            let jobs = self.jobs.read().unwrap();
            jobs.get(id)?.clone()
        };
        let status = j.status();
        let result = j.result.read().unwrap().clone();
        let error = j.error.read().unwrap().clone();
        Some((status, result, error))
    }

    pub fn logs(&self, id: &str) -> Option<Vec<String>> {
        self.jobs.read().unwrap().get(id).map(|j| j.log_snapshot())
    }

    pub fn cancel(&self, id: &str) -> bool {
        let jobs = self.jobs.read().unwrap();
        if let Some(job) = jobs.get(id) {
            job.cancel_flag.store(true, Ordering::SeqCst);
            // The executor notices the flag between reads and kills the child.
            let view = job.view();
            if view.status == JobStatus::Running {
                self.emit(JobEvent::Progress {
                    job_id: id.to_string(),
                    progress: None,
                    message: Some("Cancelling…".into()),
                });
            }
            true
        } else {
            false
        }
    }

    /// Remove finished jobs older than `EVICT_AFTER` (session-only memory).
    fn evict_finished(&self) {
        let cutoff = Utc::now() - chrono::Duration::from_std(EVICT_AFTER).unwrap();
        let mut jobs = self.jobs.write().unwrap();
        jobs.retain(|_, j| {
            if !j.status().terminal() {
                return true;
            }
            match *j.finished_at.read().unwrap() {
                Some(t) => t > cutoff,
                None => true,
            }
        });
    }
}

// ─── Routes ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub tool: String,
    #[serde(default)]
    pub params: HashMap<String, String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/jobs", post(create_job).get(list_jobs))
        .route("/api/jobs/{id}", get(get_job))
        .route("/api/jobs/{id}/result", get(get_result))
        .route("/api/jobs/{id}/logs", get(get_logs))
        .route("/api/jobs/{id}/cancel", post(cancel_job))
        .route("/api/jobs/events", get(events_stream))
}

async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateJobRequest>,
) -> Result<Json<JobView>, (StatusCode, String)> {
    state.jobs.submit(state.clone(), &req.tool, req.params).map(Json)
}

async fn list_jobs(State(state): State<Arc<AppState>>) -> Json<Vec<JobView>> {
    Json(state.jobs.list())
}

async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<JobView>, (StatusCode, String)> {
    state
        .jobs
        .get(&id)
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Job not found".to_string()))
}

async fn get_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    match state.jobs.result(&id) {
        Some((_, Some(v), _)) => Ok(Json(v)),
        Some((status, _, err)) => Err((
            StatusCode::from_job_status(status),
            err.unwrap_or_else(|| format!("Job not finished ({})", status.as_str())),
        )),
        None => Err((StatusCode::NOT_FOUND, "Job not found".to_string())),
    }
}

async fn get_logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    state
        .jobs
        .logs(&id)
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Job not found".to_string()))
}

async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if state.jobs.cancel(&id) {
        Ok(Json(serde_json::json!({ "ok": true, "job_id": id })))
    } else {
        Err((StatusCode::NOT_FOUND, "Job not found".to_string()))
    }
}

/// SSE event stream — drives the Job Center, live logs, notifications, and
/// running indicators. Typed events per the user's design.
async fn events_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>> {
    let rx = state.jobs.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |item| {
        let (name, data) = match item {
            Ok(evt) => evt.sse_parts(),
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                warn!("Job events lagged by {n}");
                (
                    "job.resync".to_string(),
                    serde_json::to_string(&serde_json::json!({
                        "message": "resync",
                        "lagged": n,
                    }))
                    .unwrap(),
                )
            }
        };
        let evt = axum::response::sse::Event::default().event(name).data(data);
        futures_util::future::ready(Some(Ok(evt)))
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

trait StatusCodeJobExt {
    fn from_job_status(s: JobStatus) -> StatusCode;
}
impl StatusCodeJobExt for StatusCode {
    fn from_job_status(s: JobStatus) -> StatusCode {
        match s {
            JobStatus::Succeeded => StatusCode::OK,
            JobStatus::Failed => StatusCode::INTERNAL_SERVER_ERROR,
            JobStatus::Cancelled => StatusCode::GONE,
            JobStatus::Running | JobStatus::Queued => StatusCode::ACCEPTED,
        }
    }
}

/// Synchronous request/response for the legacy `/api/tools/<tool>` endpoints:
/// submit a job and block until it finishes, returning the tool's result JSON.
/// Keeps old callers working while every tool runs through the Job Manager.
pub async fn run_sync(
    state: &Arc<AppState>,
    tool: &str,
    params: HashMap<String, String>,
) -> Result<serde_json::Value, (StatusCode, String)> {
    let view = state.jobs.submit(state.clone(), tool, params)?;
    let id = view.id;
    loop {
        tokio::time::sleep(Duration::from_millis(150)).await;
        match state.jobs.result(&id) {
            Some((JobStatus::Succeeded, Some(v), _)) => return Ok(v),
            Some((status, _, err)) if status.terminal() => {
                return Err((
                    StatusCode::from_job_status(status),
                    err.unwrap_or_else(|| format!("Job failed ({})", status.as_str())),
                ))
            }
            None => return Err((StatusCode::NOT_FOUND, "Job vanished".to_string())),
            _ => {}
        }
    }
}
