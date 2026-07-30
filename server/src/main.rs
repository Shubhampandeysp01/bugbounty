use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use chrono::Utc;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use pulldown_cmark::{html, Options, Parser};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{info, warn};
use walkdir::WalkDir;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    files: Arc<RwLock<Vec<FileEntry>>>,
    index: Arc<RwLock<TantivyIndex>>,
    repo_root: PathBuf,
}

struct TantivyIndex {
    index: Index,
    writer: std::sync::Mutex<IndexWriter>,
    schema: Schema,
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
            if entry.file_type().is_file() && entry.path().extension().map_or(false, |e| e == "md") {
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
        if trimmed.starts_with("# ") {
            return Some(trimmed[2..].trim().to_string());
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
    let files = state.files.read().await;
    Json(build_tree(&files))
}

async fn get_file(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<FileResponse>, (StatusCode, String)> {
    let path = params.get("path").ok_or_else(|| {
        (StatusCode::BAD_REQUEST, "Missing 'path' parameter".to_string())
    })?;

    let full_path = state.repo_root.join(path);
    if !full_path.exists() || !full_path.is_file() {
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
    let index = state.index.read().await;
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
            format!("...{}...", snippet)
        } else {
            format!("{}...", snippet)
        }
    } else {
        let truncated: String = text.chars().take(max_len).collect();
        if text.len() > max_len {
            format!("{}...", truncated)
        } else {
            truncated
        }
    }
}

async fn get_stats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let files = state.files.read().await;
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

// ─── WordPress Check ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct WpCheckResponse {
    url: String,
    version: Option<String>,
    version_source: Option<String>,
    detected: bool,
    generator_tag: Option<String>,
    rest_api_available: bool,
    xmlrpc_available: bool,
    readme_accessible: bool,
    wp_json_version: Option<String>,
    headers: std::collections::HashMap<String, String>,
    error: Option<String>,
}

async fn wordpress_check(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<WpCheckResponse>, (StatusCode, String)> {
    let url = params.get("url").ok_or_else(|| {
        (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string())
    })?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) BugBountyVault/1.0")
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Client error: {}", e)))?;

    let mut result = WpCheckResponse {
        url: url.clone(),
        version: None,
        version_source: None,
        detected: false,
        generator_tag: None,
        rest_api_available: false,
        xmlrpc_available: false,
        readme_accessible: false,
        wp_json_version: None,
        headers: std::collections::HashMap::new(),
        error: None,
    };

    // Normalize URL
    let base_url = url.trim_end_matches('/');

    // 1. Fetch the main page and check generator meta tag
    match client.get(base_url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            result.headers.insert("status_code".to_string(), status.to_string());
            
            if let Some(server) = resp.headers().get("server").and_then(|v| v.to_str().ok()) {
                result.headers.insert("server".to_string(), server.to_string());
            }
            if let Some(powered) = resp.headers().get("x-powered-by").and_then(|v| v.to_str().ok()) {
                result.headers.insert("x-powered-by".to_string(), powered.to_string());
            }
            
            if let Ok(body) = resp.text().await {
                // Check for generator meta tag
                if let Some(start) = body.find(r#"<meta name="generator""#) {
                    let snippet = &body[start..(start + 200).min(body.len())];
                    result.generator_tag = Some(snippet.to_string());
                    result.detected = true;
                    
                    // Extract version from generator tag
                    if let Some(v_start) = snippet.find(r#"content="WordPress "#) {
                        let v_part = &snippet[v_start + 19..];
                        if let Some(v_end) = v_part.find('"') {
                            result.version = Some(v_part[..v_end].to_string());
                            result.version_source = Some("generator_meta_tag".to_string());
                        }
                    }
                }
                
                // Check for wp-json link in head
                if body.contains("/wp-json/") || body.contains("wp-json") {
                    result.rest_api_available = true;
                }
            }
        }
        Err(e) => {
            result.error = Some(format!("Failed to fetch main page: {}", e));
        }
    }

    // 2. Check /wp-json/ for version info
    let wp_json_url = format!("{}/wp-json/", base_url);
    match client.get(&wp_json_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                result.rest_api_available = true;
                if let Ok(body) = resp.text().await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(ver) = json.get("version").and_then(|v| v.as_str()) {
                            result.wp_json_version = Some(ver.to_string());
                            if result.version.is_none() {
                                result.version = Some(ver.to_string());
                                result.version_source = Some("wp_json".to_string());
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {}
    }

    // 3. Check /xmlrpc.php
    let xmlrpc_url = format!("{}/xmlrpc.php", base_url);
    match client.post(&xmlrpc_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() || resp.status().as_u16() == 405 {
                result.xmlrpc_available = true;
            }
        }
        Err(_) => {}
    }

    // 4. Check /readme.html
    let readme_url = format!("{}/readme.html", base_url);
    match client.get(&readme_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                result.readme_accessible = true;
                if result.version.is_none() {
                    if let Ok(body) = resp.text().await {
                        if let Some(start) = body.find("Version ") {
                            let v_part = &body[start + 8..(start + 20).min(body.len())];
                            let v: String = v_part.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                            if !v.is_empty() {
                                result.version = Some(v);
                                result.version_source = Some("readme_html".to_string());
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {}
    }

    if result.version.is_some() {
        result.detected = true;
    }

    Ok(Json(result))
}

// ─── File Watcher ───────────────────────────────────────────────────────────

fn start_file_watcher(state: Arc<AppState>) {
    let repo_root = state.repo_root.clone();
    let state_clone = state.clone();

    tokio::spawn(async move {
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

        // Process events in a loop using blocking recv
        loop {
            match rx.recv() {
                Ok(event) => {
                    let ev = event.unwrap();
                    match ev.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            let files = scan_files(&repo_root);
                            let count = files.len();
                            *state_clone.files.write().await = files;
                            info!("Files rescanned: {} files", count);
                        }
                        _ => {}
                    }
                }
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
    } else if repo_root.parent().map_or(false, |p| p.join("guides").exists()) {
        repo_root.parent().unwrap().to_path_buf()
    } else {
        std::env::var("BUGBOUNTY_ROOT")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let cwd = std::env::current_dir().unwrap();
                if cwd.join("guides").exists() {
                    cwd
                } else if cwd.parent().map_or(false, |p| p.join("guides").exists()) {
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

    let state = Arc::new(AppState {
        files: Arc::new(RwLock::new(files)),
        index: Arc::new(RwLock::new(tantivy_index)),
        repo_root,
    });

    // Start file watcher
    start_file_watcher(state.clone());

    // Build router
    let app = Router::new()
        .route("/api/tree", get(get_tree))
        .route("/api/file", get(get_file))
        .route("/api/search", get(search_files))
        .route("/api/stats", get(get_stats))
        .route("/api/tools/wordpress-check", get(wordpress_check))
        .layer(CorsLayer::permissive())
        .with_state(state)
        .fallback_service(ServeDir::new("frontend").append_index_html_on_directories(true));

    let addr = "0.0.0.0:3000";
    info!("Server starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
