use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agent::session::{Message, Session};
use crate::agent::tool::Tool;
use crate::error;

// --- Value types for the frontend ---

/// A config field contributed by a model (e.g. GEMINI_API_KEY).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub is_secret: bool,
    pub placeholder: String,
}

/// Model info snapshot sent to the frontend for the model selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub is_available: bool,
}

// --- Agent response types ---

pub struct ModelResponse {
    pub text: Option<String>,
    pub tool_calls: Vec<ToolCallRequest>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub struct ToolCallRequest {
    pub call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
}

// --- Model trait ---

#[async_trait]
pub trait Model: Send + Sync {
    /// Stable unique identifier (e.g. "gemini-2.5-flash").
    fn model_id(&self) -> &str;

    /// Human-readable label for the UI selector.
    fn display_name(&self) -> &str;

    /// Config keys this model requires (e.g. ["GEMINI_API_KEY"]).
    fn required_config_keys(&self) -> Vec<String>;

    /// Full ConfigField descriptors so Settings can render inputs.
    fn config_fields(&self) -> Vec<ConfigField>;

    /// Returns true if all required config keys are present and non-empty.
    fn is_available(&self, settings: &HashMap<String, String>) -> bool {
        self.required_config_keys()
            .iter()
            .all(|k| settings.get(k).map_or(false, |v| !v.is_empty()))
    }

    async fn send(
        &self,
        session: Option<&Session>,
        message: &Message,
        tools: &[Box<dyn Tool>],
        settings: &HashMap<String, String>,
    ) -> error::Result<ModelResponse>;
}

// --- Model registry ---

pub struct ModelRegistry {
    models: Vec<Box<dyn Model>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self { models: Vec::new() }
    }

    pub fn register(&mut self, model: Box<dyn Model>) {
        self.models.push(model);
    }

    /// Snapshot of all models with live availability from current settings.
    pub fn list(&self, settings: &HashMap<String, String>) -> Vec<ModelInfo> {
        self.models
            .iter()
            .map(|m| ModelInfo {
                id: m.model_id().to_string(),
                display_name: m.display_name().to_string(),
                is_available: m.is_available(settings),
            })
            .collect()
    }

    /// Deduplicated union of all ConfigFields across registered models.
    pub fn config_schema(&self) -> Vec<ConfigField> {
        let mut seen = HashSet::new();
        let mut fields = Vec::new();
        for model in &self.models {
            for field in model.config_fields() {
                if seen.insert(field.key.clone()) {
                    fields.push(field);
                }
            }
        }
        fields
    }

    /// Ensure every required key exists in `settings` (inserts empty string if absent).
    /// Call this after loading config so the Settings UI shows all fields.
    pub fn seed_config(&self, settings: &mut HashMap<String, String>) {
        for field in self.config_schema() {
            settings.entry(field.key).or_default();
        }
    }

    /// Look up a model by its stable id.
    pub fn get(&self, model_id: &str) -> Option<&dyn Model> {
        self.models
            .iter()
            .find(|m| m.model_id() == model_id)
            .map(|m| m.as_ref())
    }
}
