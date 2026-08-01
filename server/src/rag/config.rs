use serde::Deserialize;
use std::path::Path;

/// Mirrors server/rag/model_config.toml — the single editable file that
/// controls which model + flags the Vault launches for RAG chat.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub binary: String,
    pub model: String,
    pub api_base: String,
    pub flags: Vec<String>,
    pub request: RequestSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestSettings {
    pub enable_thinking: bool,
    pub temperature: f32,
    pub max_tokens: u32,
    pub connect_timeout_secs: u64,
}

impl ModelConfig {
    /// Loads the config file from <repo_root>/server/rag/model_config.toml.
    pub fn load(repo_root: &Path) -> Result<ModelConfig, String> {
        let path = repo_root.join("server/rag/model_config.toml");
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read model config {}: {e}", path.display()))?;
        toml::from_str(&contents)
            .map_err(|e| format!("Invalid model config {}: {e}", path.display()))
    }

    /// Builds the full llama-server command line (binary + -m + flags).
    pub fn launch_args(&self) -> Vec<String> {
        let mut args = vec![self.binary.clone(), "-m".into(), self.model.clone()];
        args.extend(self.flags.iter().cloned());
        args
    }

    /// Context window size from `--ctx-size` flag (default 8192).
    pub fn ctx_size(&self) -> usize {
        self.flags
            .windows(2)
            .find(|w| w[0] == "--ctx-size")
            .and_then(|w| w[1].parse().ok())
            .unwrap_or(8192)
    }
}
