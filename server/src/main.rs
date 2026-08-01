mod rag;
mod tools;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, RwLock};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;
use tracing::{info, warn};
use walkdir::WalkDir;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    files: Arc<RwLock<Vec<FileEntry>>>,
    pub index: Arc<RwLock<TantivyIndex>>,
    pub repo_root: PathBuf,
    pub model: Arc<rag::model::ModelServer>,
    pub embeddings: Arc<RwLock<rag::embeddings::EmbeddingIndex>>,
}
pub struct TantivyIndex {
    pub index: Index,
    pub writer: std::sync::Mutex<IndexWriter>,
    pub schema: Schema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
    path: String,
    relative_path: String,
    title: String,
    category: String,
    last_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TreeNode {
    name: String,
    path: String,
    is_dir: bool,
    children: Vec<TreeNode>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

#[derive(Debug, Serialize)]
struct SearchResult {
    path: String,
    title: String,
    snippet: String,
    score: f32,
}

#[derive(Debug, Serialize)]
struct FileResponse {
    path: String,
    title: String,
    html: String,
    raw: String,
    category: String,
}


// ─── Tantivy Schema ──────────────────────────────────────────────────────────

fn build_tantivy_index(repo_root: &Path) -> tantivy::Result<(Index, std::sync::Mutex<IndexWriter>, Schema)> {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT | STORED);
    schema_builder.add_text_field("path", STRING | STORED);
    schema_builder.add_text_field("category", STRING | STORED);
    let schema = schema_builder.build();

    let index_path = repo_root.join(".search_index");
    std::fs::create_dir_all(&index_path).ok();

    let index = if index_path.join("meta.json").exists() {
        Index::open_in_dir(&index_path)?
    } else {
        Index::create_in_dir(&index_path, schema.clone())?
    };

    let writer = index.writer(50_000_000)?;

    Ok((index, std::sync::Mutex::new(writer), schema))
}

// ─── File Scanning ──────────────────────────────────────────────────────────

fn scan_files(repo_root: &Path) -> Vec<FileEntry> {
    let mut files = Vec::new();

    let dirs = vec!["guides", "references", "case-studies"];

    for dir in &dirs {
        let dir_path = repo_root.join(dir);
        if !dir_path.exists() {
            continue;
        }

        for entry in WalkDir::new(&dir_path)
            .into_iter()
            .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "md") {
                let full_path = entry.path();
                let relative = full_path
                    .strip_prefix(repo_root)
                    .unwrap_or(full_path)
                    .to_string_lossy()
                    .to_string();

                let title = extract_title(full_path).unwrap_or_else(|| {
                    full_path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                });

                let metadata = std::fs::metadata(full_path).ok();
                let last_modified = metadata
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        let dt: chrono::DateTime<Utc> = t.into();
                        dt.to_rfc3339()
                    })
                    .unwrap_or_default();

                files.push(FileEntry {
                    path: full_path.to_string_lossy().to_string(),
                    relative_path: relative,
                    title,
                    category: dir.to_string(),
                    last_modified,
                });
            }
        }
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files
}

fn extract_title(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn index_files(
    writer: &std::sync::Mutex<IndexWriter>,
    schema: &Schema,
    files: &[FileEntry],
) -> tantivy::Result<()> {
    let title_field = schema.get_field("title").unwrap();
    let body_field = schema.get_field("body").unwrap();
    let path_field = schema.get_field("path").unwrap();
    let category_field = schema.get_field("category").unwrap();

    let mut writer = writer.lock().unwrap();

    // Drop documents from previous runs so restarts / rescans never duplicate
    // (the .search_index directory is persisted on disk and gitignored).
    writer.delete_all_documents()?;

    for file in files {
        let content = std::fs::read_to_string(&file.path).unwrap_or_default();
        writer.add_document(doc!(
            title_field => file.title.as_str(),
            body_field => content.as_str(),
            path_field => file.relative_path.as_str(),
            category_field => file.category.as_str(),
        ))?;
    }

    writer.commit()?;
    Ok(())
}

// ─── Tree Building ──────────────────────────────────────────────────────────

fn build_tree(files: &[FileEntry]) -> Vec<TreeNode> {
    let mut root = TreeNode {
        name: "bugbounty".to_string(),
        path: "".to_string(),
        is_dir: true,
        children: vec![],
    };

    for file in files {
        let parts: Vec<&str> = file.relative_path.split('/').collect();
        let mut current = &mut root;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                current.children.push(TreeNode {
                    name: file.title.clone(),
                    path: file.relative_path.clone(),
                    is_dir: false,
                    children: vec![],
                });
            } else {
                let dir_name = part.to_string();
                let dir_path: String = parts[..=i].join("/");
                if let Some(pos) = current
                    .children
                    .iter()
                    .position(|c| c.name == dir_name && c.is_dir)
                {
                    current = &mut current.children[pos];
                } else {
                    current.children.push(TreeNode {
                        name: dir_name.clone(),
                        path: dir_path,
                        is_dir: true,
                        children: vec![],
                    });
                    current = current.children.last_mut().unwrap();
                }
            }
        }
    }

    sort_tree(&mut root.children);
    root.children
}

fn sort_tree(nodes: &mut [TreeNode]) {
    nodes.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });
    for node in nodes.iter_mut() {
        sort_tree(&mut node.children);
    }
}

// ─── Markdown Rendering ─────────────────────────────────────────────────────

fn render_markdown(content: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(content, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    format!(r#"<div class="markdown-body">{}</div>"#, html_output)
}

// ─── API Handlers ───────────────────────────────────────────────────────────

async fn get_tree(State(state): State<Arc<AppState>>) -> Json<Vec<TreeNode>> {
    let files = state.files.read().unwrap();
    Json(build_tree(&files))
}

async fn get_file(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<FileResponse>, (StatusCode, String)> {
    let path = params.get("path").ok_or_else(|| {
        (StatusCode::BAD_REQUEST, "Missing 'path' parameter".to_string())
    })?;

    // Path-traversal guard: only serve real files that resolve inside repo_root.
    let repo_canonical = state
        .repo_root
        .canonicalize()
        .unwrap_or_else(|_| state.repo_root.clone());
    let full_path = state
        .repo_root
        .join(path)
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, "File not found".to_string()))?;

    if !full_path.starts_with(&repo_canonical) || !full_path.is_file() {
        return Err((StatusCode::NOT_FOUND, "File not found".to_string()));
    }

    let content = std::fs::read_to_string(&full_path).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Read error: {}", e))
    })?;

    let title = extract_title(&full_path).unwrap_or_else(|| {
        full_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });

    let html = render_markdown(&content);

    let relative = full_path
        .strip_prefix(&state.repo_root)
        .unwrap_or(&full_path)
        .to_string_lossy()
        .to_string();

    let category = relative.split('/').next().unwrap_or("").to_string();

    Ok(Json(FileResponse {
        path: relative,
        title,
        html,
        raw: content,
        category,
    }))
}

async fn search_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> Json<Vec<SearchResult>> {
    let index = state.index.read().unwrap();
    let schema = &index.schema;
    let title_field = schema.get_field("title").unwrap();
    let body_field = schema.get_field("body").unwrap();
    let path_field = schema.get_field("path").unwrap();

    let reader = index
        .index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .unwrap();

    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index.index, vec![title_field, body_field]);
    let tantivy_query = match query_parser.parse_query(&query.q) {
        Ok(q) => q,
        Err(_) => return Json(vec![]),
    };

    let top_docs = searcher
        .search(&tantivy_query, &TopDocs::with_limit(query.limit).order_by_score())
        .unwrap();

    let mut results = Vec::new();
    for (score, doc_address) in top_docs {
        let doc = searcher
            .doc::<tantivy::TantivyDocument>(doc_address)
            .unwrap();
        let title: String = doc
            .get_first(title_field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let path: String = doc
            .get_first(path_field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body: String = doc
            .get_first(body_field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let snippet = generate_snippet(&body, &query.q, 200);

        results.push(SearchResult {
            path,
            title,
            snippet,
            score,
        });
    }

    Json(results)
}

fn generate_snippet(text: &str, query: &str, max_len: usize) -> String {
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();

    if let Some(pos) = lower_text.find(&lower_query) {
        // Only index into `text` with byte math when the hit sits on a valid
        // char boundary (to_lowercase can shift byte offsets for some scripts).
        if text.is_char_boundary(pos) {
            // Use char indices to avoid splitting multi-byte characters
            let text_chars: Vec<char> = text.chars().collect();
            let query_chars: Vec<char> = query.chars().collect();

            // Find the char index of the query match
            let char_pos = text[..pos].chars().count();

            let context = 60;
            let start = char_pos.saturating_sub(context);
            let end = (char_pos + query_chars.len() + context).min(text_chars.len());

            let snippet: String = text_chars[start..end].iter().collect();

            if start > 0 {
                return format!("...{snippet}...");
            }
            return format!("{snippet}...");
        }
    }

    let truncated: String = text.chars().take(max_len).collect();
    if text.chars().count() > max_len {
        format!("{truncated}...")
    } else {
        truncated
    }
}

async fn get_stats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let files = state.files.read().unwrap();
    let total = files.len();
    let guides = files.iter().filter(|f| f.category == "guides").count();
    let references = files.iter().filter(|f| f.category == "references").count();
    let case_studies = files.iter().filter(|f| f.category == "case-studies").count();

    Json(serde_json::json!({
        "total_files": total,
        "guides": guides,
        "references": references,
        "case_studies": case_studies,
        "last_updated": Utc::now().to_rfc3339(),
    }))
}

// ─── File Watcher ───────────────────────────────────────────────────────────

fn start_file_watcher(state: Arc<AppState>) {
    let repo_root = state.repo_root.clone();
    let state_clone = state.clone();

    // The watcher must run on a dedicated OS thread, not on the tokio runtime:
    // `rx.recv()` and `index_files()`/`commit()` are blocking calls and would
    // otherwise permanently stall executor workers, wedging the whole server.
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();

        let mut watcher: RecommendedWatcher =
            Watcher::new(tx, Config::default()).expect("Failed to create file watcher");

        let dirs = vec!["guides", "references", "case-studies"];
        for dir in &dirs {
            let path = repo_root.join(dir);
            if path.exists() {
                watcher
                    .watch(&path, RecursiveMode::Recursive)
                    .expect("Failed to watch directory");
            }
        }

        info!("File watcher started");

        // Process events in a loop using blocking recv (fine on a plain thread).
        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    if matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                    ) {
                        // Debounce bursts: FSEvents emits several events per edit,
                        // so drain anything already queued and wait briefly before
                        // doing the (expensive) rescan + reindex.
                        while rx.try_recv().is_ok() {}
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        while rx.try_recv().is_ok() {}

                        let files = scan_files(&repo_root);
                        let count = files.len();

                        // Re-index so search stays in sync with on-disk content.
                        {
                            let index = state_clone.index.read().unwrap();
                            if let Err(e) = index_files(&index.writer, &index.schema, &files) {
                                warn!("Reindex failed after file change: {e}");
                            }
                        }

                        *state_clone.files.write().unwrap() = files;
                        info!("Files rescanned + reindexed: {} files", count);

                        // Rebuild dense embeddings in a fresh thread so this
                        // watcher thread (blocking recv loop) stays responsive.
                        {
                            let embeddings = state_clone.embeddings.clone();
                            let root = repo_root.clone();
                            std::thread::spawn(move || {
                                let mut idx = embeddings.write().unwrap();
                                idx.build(&root);
                            });
                        }
                    }
                }
                Ok(Err(e)) => warn!("File watcher error: {e}"),
                Err(_) => break,
            }
        }
    });
}

// ─── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bugbounty_server=info".into()),
        )
        .init();

    // Determine repo root
    let repo_root = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let repo_root = if repo_root.join("guides").exists() {
        repo_root
    } else if repo_root
        .parent()
        .is_some_and(|p| p.join("guides").exists())
    {
        repo_root.parent().unwrap().to_path_buf()
    } else {
        std::env::var("BUGBOUNTY_ROOT")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let cwd = std::env::current_dir().unwrap();
                if cwd.join("guides").exists() {
                    cwd
                } else if cwd.parent().is_some_and(|p| p.join("guides").exists()) {
                    cwd.parent().unwrap().to_path_buf()
                } else {
                    warn!("Could not find repo root, using current directory");
                    cwd
                }
            })
    };

    info!("Repo root: {:?}", repo_root);

    // Scan files
    let files = scan_files(&repo_root);
    info!("Found {} markdown files", files.len());

    // Build Tantivy index
    let (tantivy_index, writer, schema) =
        build_tantivy_index(&repo_root).expect("Failed to build search index");
    index_files(&writer, &schema, &files).expect("Failed to index files");

    let tantivy_index = TantivyIndex {
        index: tantivy_index,
        writer,
        schema,
    };

    // Load the local LLM configuration (model path + flags, user-editable).
    let model_config = match rag::config::ModelConfig::load(&repo_root) {
        Ok(cfg) => cfg,
        Err(e) => {
            warn!("RAG chat disabled: {e}");
            // A minimal placeholder config so the server still boots; the
            // chat endpoint reports it as unreachable.
            rag::config::ModelConfig {
                binary: String::new(),
                model: String::new(),
                api_base: String::from("http://127.0.0.1:8080"),
                flags: Vec::new(),
                request: rag::config::RequestSettings {
                    enable_thinking: false,
                    temperature: 0.2,
                    max_tokens: 800,
                    connect_timeout_secs: 10,
                },
            }
        }
    };

    let state = Arc::new(AppState {
        files: Arc::new(RwLock::new(files)),
        index: Arc::new(RwLock::new(tantivy_index)),
        repo_root,
        model: Arc::new(rag::model::ModelServer::new(model_config)),
        embeddings: Arc::new(RwLock::new(rag::embeddings::EmbeddingIndex::new())),
    });

    // Start file watcher
    start_file_watcher(state.clone());

    // Build the dense-embedding index on a background thread (model download +
    // inference is blocking and can take a while on first run). Until it's
    // ready, chat retrieval falls back to BM25 only.
    {
        let embeddings = state.embeddings.clone();
        let root = state.repo_root.clone();
        std::thread::spawn(move || {
            let mut idx = embeddings.write().unwrap();
            idx.build(&root);
        });
    }

    // Start (or reuse) the local llama-server for RAG chat, without blocking
    // server startup — the model load can take a few seconds.
    {
        let model = state.model.clone();
        tokio::spawn(async move {
            model.ensure_running().await;
        });
    }

    // Restrict CORS to local origins — this tool can read local files / run
    // scanners, so never let arbitrary web pages read API responses.
    let cors = CorsLayer::new().allow_origin(AllowOrigin::list([
        axum::http::header::HeaderValue::from_static("http://localhost:3000"),
        axum::http::header::HeaderValue::from_static("http://127.0.0.1:3000"),
    ]));

    // Build router (library APIs + modular tools)
    let app = Router::new()
        .route("/api/tree", get(get_tree))
        .route("/api/file", get(get_file))
        .route("/api/search", get(search_files))
        .route("/api/stats", get(get_stats))
        .route("/api/chat", post(rag::chat::chat))
        .route("/api/chat/stream", post(rag::chat::chat_stream))
        .route("/api/chat/status", get(rag::chat::model_status))
        .merge(tools::routes())
        .layer(cors)
        .with_state(state.clone())
        .fallback_service(ServeDir::new("frontend").append_index_html_on_directories(true));

    let addr = "0.0.0.0:3000";
    info!("Server starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    // Normal shutdown: the server has stopped accepting/draining requests, so
    // now stop the llama-server we spawned (no orphaned process left behind).
    state.model.shutdown();
    info!("Server stopped");
}

/// Waits for Ctrl+C (SIGINT) or SIGTERM. The model server is shut down after
/// axum finishes draining in-flight requests.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down…");
        }
        _ = terminate => {
            info!("Received SIGTERM, shutting down…");
        }
    }
}
