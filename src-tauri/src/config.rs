use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Flat key-value store for provider config (e.g. "GEMINI_API_KEY").
    #[serde(default)]
    pub settings: HashMap<String, String>,
}

/// Only used during migration from the old config format.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyConfig {
    gemini_api_key: Option<String>,
    openai_api_key: Option<String>,
    anthropic_api_key: Option<String>,
}

impl From<LegacyConfig> for Config {
    fn from(old: LegacyConfig) -> Self {
        let mut settings = HashMap::new();
        if let Some(v) = old.gemini_api_key {
            settings.insert("GEMINI_API_KEY".to_string(), v);
        }
        if let Some(v) = old.openai_api_key {
            settings.insert("OPENAI_API_KEY".to_string(), v);
        }
        if let Some(v) = old.anthropic_api_key {
            settings.insert("ANTHROPIC_API_KEY".to_string(), v);
        }
        Config { settings }
    }
}

pub async fn load_config(config_dir: &PathBuf) -> Result<Config> {
    let path = config_dir.join("config.json");
    if !path.exists() {
        return Ok(Config::default());
    }
    let bytes = fs::read(&path).await?;

    // Try new format first (has "settings" key)
    if let Ok(cfg) = serde_json::from_slice::<Config>(&bytes) {
        if !cfg.settings.is_empty() {
            return Ok(cfg);
        }
    }
    // Fall back to legacy migration
    if let Ok(legacy) = serde_json::from_slice::<LegacyConfig>(&bytes) {
        return Ok(Config::from(legacy));
    }
    Ok(Config::default())
}

pub async fn save_config(config_dir: &PathBuf, config: &Config) -> Result<()> {
    fs::create_dir_all(config_dir).await?;
    let path = config_dir.join("config.json");
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json).await?;
    Ok(())
}
