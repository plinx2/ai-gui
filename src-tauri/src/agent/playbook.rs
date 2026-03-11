use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playbook {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub steps: Vec<String>,
    #[serde(default)]
    pub notes: String,
    /// Model to use when running this playbook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait PlaybookStore: Send + Sync {
    async fn load_all(&self) -> Result<Vec<Playbook>>;
    async fn save(&self, playbook: &Playbook) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}
