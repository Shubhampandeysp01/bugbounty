use crate::AppState;
use crate::rag::model::ModelServer;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures_util::stream::{self, Stream};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value as TantivyValue;
use tantivy::ReloadPolicy;
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
pub struct ChatQuery {
    pub message: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    5
}

struct RetrievedChunk {
    path: String,
    title: String,
    body: String,
    score: f32,
}

/// BM25 retrieval over the Tantivy index (title + body). Returns whole-document
/// bodies ranked by relevance.
fn retrieve_bm25(state: &Arc<AppState>, query: &str, limit: usize) -> Vec<RetrievedChunk> {
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
    let tantivy_query = match query_parser.parse_query(query) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };

    let top_docs = match searcher.search(
        &tantivy_query,
        &TopDocs::with_limit(limit).order_by_score(),
    ) {
        Ok(docs) => docs,
        Err(_) => return Vec::new(),
    };

    let mut chunks = Vec::new();
    for (score, doc_address) in top_docs {
        let doc = match searcher.doc::<tantivy::TantivyDocument>(doc_address) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let title = doc
            .get_first(title_field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let path = doc
            .get_first(path_field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body = doc
            .get_first(body_field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        chunks.push(RetrievedChunk {
            path,
            title,
            body,
            score,
        });
    }
    chunks
}

/// Reciprocal Rank Fusion: merges two ranked path lists into one ranking.
/// RRF score of an item = Σ 1/(k + rank_i), k=60.
fn rrf_merge(
    bm25: &[(String, f32)],
    dense: &[(String, f32)],
    limit: usize,
) -> Vec<(String, f32)> {
    const K: f32 = 60.0;
    let mut scores: std::collections::HashMap<String, f32> = std::collections::HashMap::new();

    for (rank, (path, _)) in bm25.iter().enumerate() {
        *scores.entry(path.clone()).or_insert(0.0) += 1.0 / (K + rank as f32 + 1.0);
    }
    for (rank, (path, _)) in dense.iter().enumerate() {
        *scores.entry(path.clone()).or_insert(0.0) += 1.0 / (K + rank as f32 + 1.0);
    }

    let mut merged: Vec<(String, f32)> = scores.into_iter().collect();
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    merged.truncate(limit);
    merged
}

/// Hybrid retrieval: BM25 (lexical) + dense embeddings (semantic), merged with
/// RRF. If the embedding index isn't ready yet, falls back to BM25 only.
fn hybrid_retrieve(state: &Arc<AppState>, query: &str, limit: usize) -> Vec<RetrievedChunk> {
    // BM25: whole-document hits.
    let bm25 = retrieve_bm25(state, query, limit.max(8));
    let bm25_ranks: Vec<(String, f32)> = bm25
        .iter()
        .map(|c| (c.path.clone(), c.score))
        .collect();

    // Dense: per-section hits (finer granularity than whole docs).
    let dense = {
        let idx = state.embeddings.read().unwrap();
        if idx.ready {
            idx.search(query, limit.max(8))
        } else {
            Vec::new()
        }
    };
    let dense_ranks: Vec<(String, f32)> = dense
        .iter()
        .map(|(path, _title, _text, score)| (path.clone(), *score))
        .collect();

    if dense_ranks.is_empty() {
        info!("Hybrid retrieval: BM25 only (embedding index not ready): {} hits", bm25_ranks.len());
        return bm25;
    }
    info!(
        "Hybrid retrieval: {} BM25 hits + {} dense hits for: {}",
        bm25_ranks.len(),
        dense_ranks.len(),
        query
    );

    let merged = rrf_merge(&bm25_ranks, &dense_ranks, limit.max(12));
    let mut bm25_by_path: std::collections::HashMap<String, RetrievedChunk> =
        std::collections::HashMap::new();
    for c in bm25 {
        bm25_by_path.entry(c.path.clone()).or_insert(c);
    }
    // Dense chunks give finer context; keep the highest-scoring chunk per path
    // (or_insert kept the first hit, which is not necessarily the best).
    let mut dense_by_path: std::collections::HashMap<String, (String, String, String, f32)> =
        std::collections::HashMap::new();
    for (path, title, text, score) in dense {
        dense_by_path
            .entry(path.clone())
            .and_modify(|existing| {
                if score > existing.3 {
                    *existing = (path.clone(), title.clone(), text.clone(), score);
                }
            })
            .or_insert((path, title, text, score));
    }

    let candidates: Vec<(String, String, String, f32)> = merged
        .into_iter()
        .filter_map(|(path, rrf_score)| {
            if let Some((p, title, text, _)) = dense_by_path.get(&path) {
                Some((p.clone(), title.clone(), text.clone(), rrf_score))
            } else {
                bm25_by_path.get(&path).map(|c| {
                    (c.path.clone(), c.title.clone(), c.body.clone(), rrf_score)
                })
            }
        })
        .collect();

    // Cross-encoder reranks the RRF candidates for a final precision boost.
    let reranked = {
        let idx = state.embeddings.read().unwrap();
        idx.rerank(query, &candidates)
    };
    let reranked: Vec<RetrievedChunk> = reranked
        .into_iter()
        .take(limit)
        .map(|(path, title, body, score)| RetrievedChunk {
            path,
            title,
            body,
            score,
        })
        .collect();
    info!(
        "Reranked {} candidates to {} for: {}",
        candidates.len(),
        reranked.len(),
        query
    );
    reranked
}

const SYSTEM_PROMPT: &str = "You are the Bug Bounty Research Vault assistant. Ground every \
    answer in the provided context; never invent details. If unsure, \
    say so. Format lists with markdown.";

/// Truncates `body` to fit within `budget` tokens using the model's own
/// tokenizer. Binary-searches the longest char prefix that fits, so the final
/// chunk never exceeds the token budget.
async fn truncate_to_budget(
    model: &ModelServer,
    body: &str,
    budget: usize,
) -> Result<String, String> {
    if body.is_empty() || budget == 0 {
        return Ok(String::new());
    }

    // Fast path: whole body fits.
    let full = model.count_tokens(body).await?;
    if full <= budget {
        return Ok(body.to_string());
    }

    // Find the longest prefix (as char index) that fits in `budget` tokens.
    let chars: Vec<char> = body.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    let mut best = 0usize;

    while lo < hi {
        let mid = (lo + hi) / 2;
        let prefix: String = chars[..mid].iter().collect();
        let n = model.count_tokens(&prefix).await?;
        if n <= budget {
            best = mid;
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    let out: String = chars[..best].iter().collect();
    Ok(out)
}

/// Builds a grounded user prompt whose context block is packed to fit the
/// model's context window: budget = ctx_size − system − question − max_tokens
/// − safety, using exact token counts from the running tokenizer.
async fn build_prompt(
    model: &ModelServer,
    query: &str,
    chunks: &[RetrievedChunk],
) -> Result<String, String> {
    let ctx_size = model.config.ctx_size();
    let max_tokens = model.config.request.max_tokens as usize;
    let safety = 128usize;

    // Measure the fixed overhead we can't avoid.
    let system_tokens = model.count_tokens(SYSTEM_PROMPT).await?;
    let question_tokens = model.count_tokens(&format!("Question: {query}")).await?;

    let mut budget = ctx_size
        .saturating_sub(system_tokens)
        .saturating_sub(question_tokens)
        .saturating_sub(max_tokens)
        .saturating_sub(safety);

    let mut context = String::new();
    for (i, chunk) in chunks.iter().enumerate() {
        if budget == 0 {
            break;
        }
        // A little room for the [i] header + separators.
        let header = format!("\n\n[{i}] File: {}\nTitle: {}\n", chunk.path, chunk.title);
        let header_tokens = model.count_tokens(&header).await?;
        if header_tokens + 16 > budget {
            break;
        }
        budget -= header_tokens + 16;

        let body = truncate_to_budget(model, &chunk.body, budget).await?;
        if body.is_empty() {
            break;
        }
        let body_tokens = model.count_tokens(&body).await?;
        budget -= body_tokens;

        context.push_str(&header);
        context.push_str(&body);
    }

    Ok(format!(
        "You are an expert bug bounty researcher's assistant. Answer the user's \
         question using ONLY the context below (retrieved from the Vault library). \
         Cite your sources inline as [0], [1], etc. If the context does not contain \
         the answer, say you don't know and suggest what to search for. Be concise.\n\n\
         === CONTEXT ===\n{}\n=== END CONTEXT ===\n\nQuestion: {}",
        context, query
    ))
}

pub async fn chat(
    State(state): State<Arc<AppState>>,
    Json(query): Json<ChatQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let message = query.message.trim();
    if message.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Message cannot be empty".to_string()));
    }

    if !state.model.is_healthy().await {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "Model server is not running. Click “Start model” in the Ask panel \
                 (or launch llama-server at {}). See server/rag/model_config.toml.",
                state.model.config.api_base
            ),
        ));
    }

    // 1. Retrieve relevant chunks (hybrid: BM25 + dense embeddings).
    let chunks = hybrid_retrieve(&state, message, query.limit);
    if chunks.is_empty() {
        return Ok(Json(json!({
            "answer": "I couldn't find anything relevant in the Vault. Try a different question, or browse the library to add more context.",
            "sources": [],
        })));
    }
    info!("RAG retrieved {} chunks for: {}", chunks.len(), message);

    // 2. Ground the model on the retrieved chunks, packed to the context budget.
    let prompt = match build_prompt(&state.model, message, &chunks).await {
        Ok(p) => p,
        Err(e) => {
            warn!("Prompt build failed: {e}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e));
        }
    };
    if let Ok(n) = state.model.count_tokens(&prompt).await {
        info!(
            "RAG prompt packed to {n} tokens (ctx {}): {}",
            state.model.config.ctx_size(),
            message
        );
    }

    let answer = match state.model.chat(SYSTEM_PROMPT, &prompt).await {
        Ok(a) => a,
        Err(e) => {
            warn!("Model chat failed: {e}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e));
        }
    };

    // 3. Return the answer + sources for citations.
    let sources: Vec<Value> = chunks
        .iter()
        .map(|c| {
            json!({
                "path": c.path,
                "title": c.title,
                "score": c.score,
            })
        })
        .collect();

    Ok(Json(json!({
        "answer": answer,
        "sources": sources,
    })))
}

/// Reports whether the local model server is up, for the Ask panel UI.
/// `managed` = we spawned it (so the UI can offer Stop); `starting` = a manual
/// start is still loading.
pub async fn model_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let ready = state.model.is_healthy().await;
    Ok(Json(json!({
        "ready": ready,
        "starting": state.model.is_starting(),
        "managed": state.model.is_managed(),
        "api_base": state.model.config.api_base,
        "model": state.model.config.model,
    })))
}

/// POST /api/chat/model/start — spawns the model server (background) so RAG
/// chat can be used. Non-blocking; poll /api/chat/status until ready.
pub async fn start_model(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let outcome = state
        .model
        .start()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(json!({
        "ok": true,
        "status": outcome,
        "ready": state.model.is_healthy().await,
        "starting": state.model.is_starting(),
    })))
}

/// POST /api/chat/model/stop — kills the model server we spawned (leaves
/// externally-launched instances alone).
pub async fn stop_model(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let stopped = state.model.stop();
    Ok(Json(json!({
        "ok": true,
        "stopped": stopped,
        "ready": false,
        "starting": false,
        "managed": false,
    })))
}

/// Streaming chat: same retrieval + prompt-packing as `chat`, but the model
/// response is sent to the client token-by-token via Server-Sent Events.
///
/// Events:
///   - `sources`  → the retrieved chunks (path/title/score) for citations
///   - `delta`    → one assistant text fragment
///   - `done`     → stream finished (data: {})
pub async fn chat_stream(
    State(state): State<Arc<AppState>>,
    Json(query): Json<ChatQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let message = query.message.trim();
    if message.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Message cannot be empty".to_string()));
    }

    if !state.model.is_healthy().await {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "Model server is not running. Click “Start model” in the Ask panel \
                 (or launch llama-server at {}). See server/rag/model_config.toml.",
                state.model.config.api_base
            ),
        ));
    }

    // 1. Retrieve relevant chunks (hybrid: BM25 + dense embeddings, reranked).
    let chunks = hybrid_retrieve(&state, message, query.limit);
    if chunks.is_empty() {
        return Err((StatusCode::NOT_FOUND, "No relevant content found in the Vault".to_string()));
    }
    info!("RAG stream retrieved {} chunks for: {}", chunks.len(), message);

    // 2. Ground the model on the retrieved chunks, packed to the context budget.
    let prompt = match build_prompt(&state.model, message, &chunks).await {
        Ok(p) => p,
        Err(e) => {
            warn!("Prompt build failed: {e}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e));
        }
    };
    if let Ok(n) = state.model.count_tokens(&prompt).await {
        info!(
            "RAG stream prompt packed to {n} tokens (ctx {}): {}",
            state.model.config.ctx_size(),
            message
        );
    }

    // 3. Stream the model's tokens, with the sources event sent first.
    let sources: Vec<Value> = chunks
        .iter()
        .map(|c| {
            json!({
                "path": c.path,
                "title": c.title,
                "score": c.score,
            })
        })
        .collect();

    let (model, sys, usr) = (state.model.clone(), SYSTEM_PROMPT.to_string(), prompt);
    let stream = async_stream_sources(&sources)
        .chain(async_stream_deltas(model, sys, usr));

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Leads with a single `sources` event carrying the retrieved chunks.
fn async_stream_sources(
    sources: &[Value],
) -> impl Stream<Item = Result<Event, Infallible>> {
    let data = json!({ "sources": sources }).to_string();
    let event = Event::default().event("sources").data(data);
    stream::once(async { Ok(event) })
}

/// Follows with `delta` events per model token and a final `done` event.
fn async_stream_deltas(
    model: Arc<ModelServer>,
    system: String,
    user: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(16);

    tokio::spawn(async move {
        match model.chat_stream(&system, &user).await {
            Ok(mut receiver) => {
                while let Some(item) = receiver.recv().await {
                    match item {
                        Ok(delta) => {
                            let event = Event::default().event("delta").data(delta);
                            if tx.send(Ok(event)).await.is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            let event = Event::default().event("error").data(e);
                            let _ = tx.send(Ok(event)).await;
                            return;
                        }
                    }
                }
                let event = Event::default().event("done").data("{}");
                let _ = tx.send(Ok(event)).await;
            }
            Err(e) => {
                let event = Event::default().event("error").data(e);
                let _ = tx.send(Ok(event)).await;
            }
        }
    });

    tokio_stream::wrappers::ReceiverStream::new(rx)
}
