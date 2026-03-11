use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agent::model::{ConfigField, Model, ModelResponse, ToolCallRequest};
use crate::agent::session::{Message, MessageContent, Session};
use crate::agent::tool::Tool;
use crate::error::{AppError, Result};

pub struct AgentApiModel {
    client: reqwest::Client,
}

impl AgentApiModel {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

// --- Request types ---

#[derive(Serialize)]
struct RunRequest {
    message: ReqMessageContent,
    session_id: String,
    model: String,
    tools: Vec<ReqToolSchema>,
}

#[derive(Serialize)]
struct ReqMessageContent {
    parts: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct ReqToolSchema {
    name: String,
    description: String,
    json_schema: serde_json::Value,
}

// --- Response types ---

#[derive(Deserialize)]
struct ExtSession {
    messages: Vec<ExtMessage>,
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

#[derive(Deserialize)]
struct ExtMessage {
    role: String,
    content: ExtMessageContent,
}

#[derive(Deserialize)]
struct ExtMessageContent {
    parts: Vec<serde_json::Value>,
}

// --- Helpers ---

fn build_req_parts(msg: &Message) -> Vec<serde_json::Value> {
    match &msg.content {
        MessageContent::Text { text } => vec![serde_json::json!({ "text": text })],
        MessageContent::ToolCall {
            call_id,
            tool_name,
            input,
        } => vec![serde_json::json!({
            "call_id": call_id,
            "tool_name": tool_name,
            "input": input,
        })],
        MessageContent::ToolResult {
            call_id,
            tool_name,
            output,
            is_error,
        } => vec![serde_json::json!({
            "call_id": call_id,
            "tool_name": tool_name,
            "output": output,
            "is_error": is_error,
        })],
        MessageContent::FileAttachment {
            name,
            mime_type,
            data_base64,
        } => vec![serde_json::json!({
            "name": name,
            "mime_type": mime_type,
            "data": data_base64,
        })],
    }
}

fn parse_text(parts: &[serde_json::Value]) -> Option<String> {
    parts
        .iter()
        .find_map(|p| p.get("text")?.as_str().map(|s| s.to_string()))
}

fn parse_tool_calls(parts: &[serde_json::Value]) -> Vec<ToolCallRequest> {
    parts
        .iter()
        .filter_map(|p| {
            let call_id = p.get("call_id")?.as_str()?.to_string();
            let tool_name = p.get("tool_name")?.as_str()?.to_string();
            let input = p.get("input")?.clone();
            Some(ToolCallRequest {
                call_id,
                tool_name,
                input,
            })
        })
        .collect()
}

/// Strips the `/run` path suffix to derive the API base URL.
/// e.g. `http://localhost:8080/run` → `http://localhost:8080`
fn api_base(run_url: &str) -> &str {
    let s = run_url.trim_end_matches('/');
    s.strip_suffix("/run").unwrap_or(s)
}

// --- Model impl ---

#[async_trait]
impl Model for AgentApiModel {
    fn model_id(&self) -> &str {
        "external-agent"
    }

    fn display_name(&self) -> &str {
        "External Agent"
    }

    fn required_config_keys(&self) -> Vec<String> {
        vec![
            "EXTERNAL_AGENT_URL".to_string(),
            "EXTERNAL_AGENT_TOKEN".to_string(),
        ]
    }

    fn config_fields(&self) -> Vec<ConfigField> {
        vec![
            ConfigField {
                key: "EXTERNAL_AGENT_URL".to_string(),
                label: "Agent API URL".to_string(),
                is_secret: false,
                placeholder: "http://localhost:8080/run".to_string(),
            },
            ConfigField {
                key: "EXTERNAL_AGENT_TOKEN".to_string(),
                label: "Agent API Token".to_string(),
                is_secret: true,
                placeholder: "your-api-token".to_string(),
            },
        ]
    }

    async fn send(
        &self,
        session: Option<&Session>,
        message: &Message,
        tools: &[Box<dyn Tool>],
        settings: &HashMap<String, String>,
    ) -> Result<ModelResponse> {
        let run_url = settings
            .get("EXTERNAL_AGENT_URL")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::MissingApiKey("External Agent URL".to_string()))?
            .clone();
        let token = settings
            .get("EXTERNAL_AGENT_TOKEN")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::MissingApiKey("External Agent Token".to_string()))?
            .clone();

        let session_id = session.map(|s| s.id.clone()).unwrap_or_default();

        // Fetch previous cumulative tokens so we can compute delta after the run.
        let (prev_input, prev_output) = if !session_id.is_empty() {
            let session_url = format!("{}/session/{}", api_base(&run_url), session_id);
            match self
                .client
                .get(&session_url)
                .bearer_auth(&token)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => resp
                    .json::<ExtSession>()
                    .await
                    .map(|s| (s.input_tokens, s.output_tokens))
                    .unwrap_or((0, 0)),
                _ => (0, 0),
            }
        } else {
            (0, 0)
        };

        // Build request
        let tools_req: Vec<ReqToolSchema> = tools
            .iter()
            .map(|t| ReqToolSchema {
                name: t.name().to_string(),
                description: t.description().to_string(),
                json_schema: t.parameters_schema(),
            })
            .collect();

        let request = RunRequest {
            message: ReqMessageContent {
                parts: build_req_parts(message),
            },
            session_id,
            model: String::new(),
            tools: tools_req,
        };

        let response = self
            .client
            .post(&run_url)
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "Agent API error {}: {}",
                status, body
            )));
        }

        let ext_session: ExtSession = response.json().await?;

        // Compute token delta (guard against underflow if counts diverge)
        let delta_input = ext_session.input_tokens.saturating_sub(prev_input);
        let delta_output = ext_session.output_tokens.saturating_sub(prev_output);

        // The last assistant message is the response for this turn.
        let last_assistant = ext_session
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .ok_or_else(|| AppError::Api("No assistant message in response".to_string()))?;

        Ok(ModelResponse {
            text: parse_text(&last_assistant.content.parts),
            tool_calls: parse_tool_calls(&last_assistant.content.parts),
            input_tokens: delta_input,
            output_tokens: delta_output,
        })
    }
}
