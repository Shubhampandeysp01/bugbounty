use crate::rag::config::ModelConfig;
use futures_util::StreamExt;
use serde_json::json;
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Manages the llama-server subprocess and forwards OpenAI-compatible chat
/// completions to it. If an instance is already listening on `api_base`
/// (e.g. launched manually from the config file), it reuses it instead of
/// spawning a second one — and will NOT kill a reused instance on shutdown.
#[derive(Clone)]
pub struct ModelServer {
    pub config: ModelConfig,
    client: reqwest::Client,
    /// The llama-server process we spawned, if any. Only this is killed on
    /// shutdown; reused instances are left alone.
    spawned: Arc<Mutex<Option<Child>>>,
}

impl ModelServer {
    pub fn new(config: ModelConfig) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(config.request.connect_timeout_secs))
            .timeout(Duration::from_secs(300))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            config,
            client,
            spawned: Arc::new(Mutex::new(None)),
        }
    }

    /// Returns true if a model server is already answering on `api_base`.
    pub async fn is_healthy(&self) -> bool {
        let url = format!("{}/health", self.config.api_base);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Spawns llama-server as a child process with the flags from the config
    /// file. The spawned process is killed when `shutdown()` is called (i.e.
    /// when the Vault server exits gracefully).
    pub fn spawn(&self) {
        let args = self.config.launch_args();
        info!(
            "Spawning model server: {}",
            args.iter().map(|a| a.as_str()).collect::<Vec<_>>().join(" ")
        );
        let mut cmd = std::process::Command::new(&args[0]);
        cmd.args(&args[1..])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                *self.spawned.lock().unwrap() = Some(child);
                // Monitor thread: when llama-server exits (crash or kill),
                // clear the handle so shutdown doesn't try to kill it again.
                let spawned = self.spawned.clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(2));
                    let exited = {
                        let mut guard = spawned.lock().unwrap();
                        match guard
                            .as_mut()
                            .and_then(|c| c.try_wait().ok().flatten())
                        {
                            Some(status) => {
                                warn!("Model server (pid {pid}) exited: {status}");
                                *guard = None;
                                true
                            }
                            None => false,
                        }
                    };
                    if exited {
                        break;
                    }
                });
            }
            Err(e) => warn!("Failed to spawn model server: {e}"),
        }
    }

    /// Kills the llama-server process this server spawned, if any. Reused
    /// (externally-launched) instances are left untouched.
    pub fn shutdown(&self) {
        let child = self.spawned.lock().unwrap().take();
        if let Some(mut child) = child {
            let pid = child.id();
            info!("Shutting down model server (pid {pid})");
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Ensures a model server is reachable, starting one if needed.
    pub async fn ensure_running(&self) {
        if self.is_healthy().await {
            info!("Model server already running at {}", self.config.api_base);
            return;
        }
        self.spawn();
        // Wait (up to ~90s) for the model to finish loading.
        for _ in 0..180 {
            if self.is_healthy().await {
                info!("Model server ready at {}", self.config.api_base);
                return;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        warn!("Model server did not become healthy in time");
    }

    /// Counts tokens for `text` using the running model's own tokenizer
    /// (llama.cpp `/tokenize`). Returns the number of tokens.
    pub async fn count_tokens(&self, text: &str) -> Result<usize, String> {
        let url = format!("{}/tokenize", self.config.api_base);
        let body = json!({ "content": text, "add_special": false });
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Tokenize request failed: {e}"))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read tokenize response: {e}"))?;
        if !status.is_success() {
            return Err(format!("Model server error {status}: {text}"));
        }
        let parsed: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Bad JSON from tokenize: {e}"))?;
        parsed
            .get("tokens")
            .and_then(|t| t.as_array())
            .map(|arr| arr.len())
            .ok_or_else(|| "Tokenize response missing tokens array".to_string())
    }

    /// Sends a chat-completions request and returns the assistant text.
    /// `system_prompt` and `user_prompt` are the grounded RAG turn.
    pub async fn chat(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, String> {
        let url = format!("{}/v1/chat/completions", self.config.api_base);

        let mut chat_kwargs = serde_json::Map::new();
        chat_kwargs.insert(
            "enable_thinking".to_string(),
            json!(self.config.request.enable_thinking),
        );

        let body = json!({
            "model": "local",
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt },
            ],
            "temperature": self.config.request.temperature,
            "max_tokens": self.config.request.max_tokens,
            "stream": false,
            "chat_template_kwargs": chat_kwargs,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Chat request failed: {e}"))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if !status.is_success() {
            return Err(format!("Model server error {status}: {text}"));
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Bad JSON from model server: {e}"))?;

        parsed
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.trim().to_string())
            .ok_or_else(|| {
                format!("Model response missing content: {}", text.chars().take(200).collect::<String>())
            })
    }

    /// Streams a chat-completions response, yielding each assistant text
    /// delta as it's generated. The returned receiver closes when the stream
    /// ends or errors (each item is `Ok(delta)` or `Err(message)`).
    pub async fn chat_stream(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<mpsc::Receiver<Result<String, String>>, String> {
        let url = format!("{}/v1/chat/completions", self.config.api_base);

        let mut chat_kwargs = serde_json::Map::new();
        chat_kwargs.insert(
            "enable_thinking".to_string(),
            json!(self.config.request.enable_thinking),
        );

        let body = json!({
            "model": "local",
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt },
            ],
            "temperature": self.config.request.temperature,
            "max_tokens": self.config.request.max_tokens,
            "stream": true,
            "chat_template_kwargs": chat_kwargs,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Chat request failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .map_err(|e| format!("Failed to read response: {e}"))?;
            return Err(format!("Model server error {status}: {text}"));
        }

        let (tx, rx) = mpsc::channel::<Result<String, String>>(32);
        tokio::spawn(async move {
            let mut stream = resp.bytes_stream();
            let mut buf: Vec<u8> = Vec::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => buf.extend_from_slice(&bytes),
                    Err(e) => {
                        let _ = tx.send(Err(format!("Stream read failed: {e}"))).await;
                        return;
                    }
                }

                // Split the SSE stream on "\n\n" boundaries; a chunk may contain
                // several complete events or just a fragment of one.
                loop {
                    let sep = buf.windows(2).position(|w| w == b"\n\n");
                    let Some(end) = sep else { break };
                    let event: Vec<u8> = buf.drain(..=end).collect();
                    let text = String::from_utf8_lossy(&event);

                    for line in text.lines() {
                        let line = line.trim();
                        let Some(data) = line.strip_prefix("data:") else {
                            continue;
                        };
                        let data = data.trim();
                        if data == "[DONE]" {
                            return;
                        }
                        let parsed: serde_json::Value = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        if let Some(delta) = parsed["choices"][0]["delta"]["content"].as_str() {
                            if !delta.is_empty()
                                && tx.send(Ok(delta.to_string())).await.is_err()
                            {
                                return;
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}
