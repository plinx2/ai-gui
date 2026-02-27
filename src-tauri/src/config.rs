use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub gemini_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub default_model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gemini_api_key: None,
            openai_api_key: None,
            anthropic_api_key: None,
            default_model: "gemini-2.5-flash".to_string(),
        }
    }
}

pub async fn load_config(config_dir: &PathBuf) -> Result<Config> {
    let path = config_dir.join("config.json");
    if !path.exists() {
        return Ok(Config::default());
    }
    let bytes = fs::read(&path).await?;
    let config = serde_json::from_slice(&bytes)?;
    Ok(config)
}

pub async fn save_config(config_dir: &PathBuf, config: &Config) -> Result<()> {
    fs::create_dir_all(config_dir).await?;
    let path = config_dir.join("config.json");
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json).await?;
    Ok(())
}

impl Config {
    pub fn gemini_api_key(&self) -> Result<&str> {
        self.gemini_api_key
            .as_deref()
            .ok_or_else(|| AppError::MissingApiKey("Gemini".to_string()))
    }
}
