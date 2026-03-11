use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub model_name: String,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub content: MessageContent,
    pub created_at: DateTime<Utc>,
    /// Which model generated this message. None for user/tool messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MessageContent {
    Text {
        text: String,
    },
    ToolCall {
        call_id: String,
        tool_name: String,
        input: serde_json::Value,
    },
    ToolResult {
        call_id: String,
        tool_name: String,
        output: String,
        is_error: bool,
    },
    FileAttachment {
        name: String,
        mime_type: String,
        data_base64: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub model_name: String,
    pub updated_at: DateTime<Utc>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

impl From<&Session> for SessionSummary {
    fn from(s: &Session) -> Self {
        Self {
            id: s.id.clone(),
            title: s.title.clone(),
            model_name: s.model_name.clone(),
            updated_at: s.updated_at,
            total_input_tokens: s.total_input_tokens,
            total_output_tokens: s.total_output_tokens,
        }
    }
}
