use fastembed::{TextEmbedding, TextRerank};
use std::path::Path;
use std::sync::Mutex;
use tracing::{info, warn};

/// One retrievable unit of text (a section of a markdown file) plus its dense
/// embedding vector.
pub struct Chunk {
    pub path: String,
    pub title: String,
    pub text: String,
    pub embedding: Vec<f32>,
}

/// In-memory dense-embedding index over the markdown library.
///
/// Built on a background thread at startup (and rebuilt on file change) so the
/// server never blocks on model download / inference. Until `ready` is true,
/// callers should fall back to BM25-only retrieval.
pub struct EmbeddingIndex {
    chunks: Vec<Chunk>,
    model: Mutex<Option<TextEmbedding>>,
    reranker: Mutex<Option<TextRerank>>,
    pub ready: bool,
}

impl EmbeddingIndex {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            model: Mutex::new(None),
            reranker: Mutex::new(None),
            ready: false,
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Builds (or rebuilds) the index for every `*.md` file under repo_root.
    /// Blocking — call on a dedicated thread.
    ///
    /// On rebuild, the previous index stays queryable until the new one is
    /// ready (we do not flip `ready` off at the start), so chat does not fall
    /// back to BM25-only mid-reindex.
    pub fn build(&mut self, repo_root: &Path) {
        let was_ready = self.ready;
        // Only mark not-ready on the first build; rebuilds keep serving old chunks.
        if !was_ready {
            self.ready = false;
        }

        let files = crate::scan_files(repo_root);
        let mut all_texts: Vec<String> = Vec::new();
        let mut meta: Vec<(String, String)> = Vec::new(); // (path, title)

        for f in &files {
            let content = match std::fs::read_to_string(&f.path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let chunks = chunk_markdown(&content, f.relative_path.clone(), f.title.clone());
            for c in chunks {
                all_texts.push(c.text.clone());
                meta.push((c.path, c.title));
            }
        }

        if all_texts.is_empty() {
            info!("Embedding index: no chunks to embed");
            self.ready = true;
            return;
        }

        info!("Embedding index: embedding {} chunks…", all_texts.len());

        self.ensure_model(repo_root);

        let vectors = {
            let mut guard = self.model.lock().unwrap();
            let Some(model) = guard.as_mut() else {
                warn!("Embedding model unavailable; falling back to BM25 only");
                return;
            };
            // batch to bound peak memory; 16 dims x 384, small enough
            let mut out = Vec::with_capacity(all_texts.len());
            for batch in all_texts.chunks(64) {
                match model.embed(batch, Some(64)) {
                    Ok(emb) => {
                        for e in emb {
                            out.push(e.to_vec());
                        }
                    }
                    Err(e) => {
                        warn!("Embedding batch failed: {e}");
                        break;
                    }
                }
            }
            out
        };

        if vectors.len() == all_texts.len() {
            let mut new_chunks = Vec::with_capacity(all_texts.len());
            for (i, text) in all_texts.into_iter().enumerate() {
                let (path, title) = meta[i].clone();
                new_chunks.push(Chunk {
                    path,
                    title,
                    text,
                    embedding: vectors[i].clone(),
                });
            }
            self.chunks = new_chunks;
            self.ready = true;
            info!("Embedding index ready: {} chunks", self.chunks.len());
        } else if was_ready {
            // Keep the previous good index rather than wiping to BM25-only.
            warn!(
                "Embedding rebuild partial ({} vectors for {} texts); keeping previous index",
                vectors.len(),
                all_texts.len()
            );
        } else {
            warn!(
                "Embedding index partial ({} vectors for {} texts); using BM25 only",
                vectors.len(),
                all_texts.len()
            );
            self.ready = true; // empty / partial first build — BM25 fallback
        }
    }

    /// Ensures the embedding + reranker models are loaded (idempotent). Call
    /// before embedding so failures surface once and search can detect absence.
    fn ensure_model(&self, repo_root: &Path) {
        let mut guard = self.model.lock().unwrap();
        if guard.is_none() {
            // Model cache lives alongside the server data dir.
            let cache_dir = repo_root.join(".embedding_cache");
            std::fs::create_dir_all(&cache_dir).ok();
            let opts = fastembed::TextInitOptions::new(
                fastembed::EmbeddingModel::BGESmallENV15,
            )
            .with_cache_dir(cache_dir.clone())
            .with_show_download_progress(false)
            .with_intra_threads(4)
            .with_max_length(512);
            match TextEmbedding::try_new(opts) {
                Ok(m) => {
                    info!("Dense embedding model loaded (BGE small en v1.5)");
                    *guard = Some(m);
                }
                Err(e) => {
                    warn!("Failed to load embedding model: {e}");
                }
            }
        }
        drop(guard);

        let mut rerank_guard = self.reranker.lock().unwrap();
        if rerank_guard.is_none() {
            let cache_dir = repo_root.join(".embedding_cache");
            let opts = fastembed::RerankInitOptions::new(
                fastembed::RerankerModel::BGERerankerBase,
            )
            .with_cache_dir(cache_dir)
            .with_show_download_progress(false)
            .with_intra_threads(4)
            .with_max_length(512);
            match TextRerank::try_new(opts) {
                Ok(r) => {
                    info!("Reranker model loaded (bge-reranker-base)");
                    *rerank_guard = Some(r);
                }
                Err(e) => {
                    warn!("Failed to load reranker model: {e}");
                }
            }
        }
    }

    /// Embeds `query` and returns the top `limit` chunks by cosine similarity,
    /// as `(path, title, text, score)`.
    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, String, String, f32)> {
        if !self.ready || self.chunks.is_empty() {
            return Vec::new();
        }
        let mut guard = self.model.lock().unwrap();
        let model = match guard.as_mut() {
            Some(m) => m,
            None => return Vec::new(),
        };
        let q = match model.embed(vec![query], Some(1)) {
            Ok(mut v) => v.pop().map(|e| e.to_vec()).unwrap_or_default(),
            Err(e) => {
                warn!("Query embedding failed: {e}");
                return Vec::new();
            }
        };
        if q.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(usize, f32)> = self
            .chunks
            .iter()
            .enumerate()
            .map(|(i, c)| (i, cosine(&q, &c.embedding)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(limit)
            .map(|(i, score)| {
                let c = &self.chunks[i];
                (c.path.clone(), c.title.clone(), c.text.clone(), score)
            })
            .collect()
    }

    /// Cross-encoder reranking: scores `candidates` (each `(title, body)`)
    /// against `query` with the reranker model, returning them reordered by
    /// relevance (most relevant first). If the reranker isn't loaded, returns
    /// the candidates unchanged.
    pub fn rerank(
        &self,
        query: &str,
        candidates: &[(String, String, String, f32)],
    ) -> Vec<(String, String, String, f32)> {
        if candidates.is_empty() {
            return Vec::new();
        }
        let mut guard = self.reranker.lock().unwrap();
        let reranker = match guard.as_mut() {
            Some(r) => r,
            None => return candidates.to_vec(),
        };

        // Score each candidate against the query with the cross-encoder.
        let docs: Vec<String> = candidates
            .iter()
            .map(|(_, title, body, _)| {
                if title.is_empty() {
                    body.clone()
                } else {
                    format!("{title}\n{body}")
                }
            })
            .collect();

        let doc_refs: Vec<&str> = docs.iter().map(|s| s.as_str()).collect();
        let results = match reranker.rerank(query, doc_refs, false, Some(candidates.len())) {
            Ok(r) => r,
            Err(e) => {
                warn!("Rerank failed: {e}");
                return candidates.to_vec();
            }
        };

        // results are sorted by score desc; map indices back to candidates and
        // stamp the cross-encoder score (not the old RRF score) so ordering and
        // displayed scores stay consistent.
        results
            .into_iter()
            .filter_map(|r| {
                candidates
                    .get(r.index)
                    .map(|c| (c.0.clone(), c.1.clone(), c.2.clone(), r.score))
            })
            .collect()
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = (na * nb).sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

const CHUNK_CHARS: usize = 900;
const CHUNK_OVERLAP: usize = 120;

/// Heading-aware markdown chunking: splits on `#` headings and further splits
/// long sections on paragraph/sentence boundaries. Each chunk is prefixed with
/// its heading context so it reads standalone.
pub fn chunk_markdown(content: &str, path: String, title: String) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut sections: Vec<(usize, String, String)> = Vec::new(); // (level, heading, body)

    let mut current_level = 0usize;
    let mut current_heading = String::new();
    let mut current_body = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('#') {
            // determine heading level
            let mut level = 0;
            for ch in rest.chars() {
                if ch == '#' {
                    level += 1;
                } else {
                    break;
                }
            }
            let heading_text = rest[level..].trim().to_string();
            if !current_heading.is_empty() || !current_body.is_empty() {
                sections.push((current_level, current_heading.clone(), current_body.clone()));
            }
            current_level = level;
            current_heading = heading_text;
            current_body = String::new();
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    if !current_heading.is_empty() || !current_body.is_empty() {
        sections.push((current_level, current_heading.clone(), current_body.clone()));
    }

    let mut pending_headings: Vec<(usize, String)> = Vec::new();
    for (level, heading, body) in sections {
        // track ancestor headings for context
        pending_headings.retain(|(l, _)| *l < level);
        pending_headings.push((level, heading.clone()));

        let context: String = pending_headings
            .iter()
            .map(|(_, h)| format!("# {}", h))
            .collect::<Vec<_>>()
            .join("\n");

        let body = body.trim();
        if body.is_empty() {
            continue;
        }

        if body.chars().count() <= CHUNK_CHARS {
            chunks.push(make_chunk(&path, &title, &context, body));
            continue;
        }

        // Split long section into overlapping pieces at paragraph boundaries.
        let paragraphs: Vec<&str> = body.split('\n').filter(|p| !p.trim().is_empty()).collect();
        let mut current = String::new();
        for p in paragraphs {
            if current.chars().count() + p.chars().count() > CHUNK_CHARS && !current.is_empty() {
                chunks.push(make_chunk(&path, &title, &context, &current));
                // overlap: keep the tail of the previous chunk
                let tail: String = current.chars().rev().take(CHUNK_OVERLAP).collect::<Vec<_>>().into_iter().rev().collect();
                current = tail;
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(p);
        }
        if !current.trim().is_empty() {
            chunks.push(make_chunk(&path, &title, &context, &current));
        }
    }

    chunks
}

fn make_chunk(path: &str, title: &str, context: &str, body: &str) -> Chunk {
    let text = if context.is_empty() {
        body.to_string()
    } else {
        format!("{context}\n\n{body}")
    };
    Chunk {
        path: path.to_string(),
        title: title.to_string(),
        text,
        embedding: Vec::new(),
    }
}
