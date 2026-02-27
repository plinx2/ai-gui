use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};

use crate::agent::tool::Tool;

/// Shared map of pending choice requests: call_id -> sender
pub type PendingChoices = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

/// Tauri event payload emitted to the frontend when choices are presented
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoicesPayload {
    pub call_id: String,
    pub question: String,
    pub choices: Vec<String>,
}

pub struct ChoiceTool {
    app_handle: AppHandle,
    pending: PendingChoices,
}

impl ChoiceTool {
    pub fn new(app_handle: AppHandle, pending: PendingChoices) -> Self {
        Self { app_handle, pending }
    }
}

#[async_trait]
impl Tool for ChoiceTool {
    fn name(&self) -> &str {
        "present_choices"
    }

    fn description(&self) -> &str {
        "Presents a question with multiple choice options to the user and waits for their selection. \
        Use this when you need the user to pick from specific options to proceed. \
        The user can also provide a free-text answer."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question or prompt to show the user"
                },
                "choices": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of 2-5 choices to present. A free-text option is always added automatically."
                }
            },
            "required": ["question", "choices"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let question = input
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("Please choose an option")
            .to_string();

        let choices: Vec<String> = input
            .get("choices")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if choices.is_empty() {
            return "Error: no choices provided".to_string();
        }

        let call_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<String>();

        self.pending.lock().await.insert(call_id.clone(), tx);

        let payload = ChoicesPayload {
            call_id: call_id.clone(),
            question,
            choices,
        };

        if let Err(e) = self.app_handle.emit("tool:choices", payload) {
            self.pending.lock().await.remove(&call_id);
            return format!("Error emitting choices event: {}", e);
        }

        // Await user selection (timeout: 10 minutes)
        match tokio::time::timeout(Duration::from_secs(600), rx).await {
            Ok(Ok(answer)) => answer,
            Ok(Err(_)) => "Error: choice channel was closed".to_string(),
            Err(_) => {
                self.pending.lock().await.remove(&call_id);
                "Error: timed out waiting for user selection".to_string()
            }
        }
    }
}
