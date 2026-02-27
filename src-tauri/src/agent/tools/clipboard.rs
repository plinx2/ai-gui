use async_trait::async_trait;
use serde_json::json;

use crate::agent::tool::Tool;

pub struct ClipboardReadTool;

#[async_trait]
impl Tool for ClipboardReadTool {
    fn name(&self) -> &str {
        "clipboard_read"
    }

    fn description(&self) -> &str {
        "Read the current text content of the system clipboard."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: serde_json::Value) -> String {
        tokio::task::spawn_blocking(|| {
            match arboard::Clipboard::new() {
                Ok(mut cb) => cb.get_text().unwrap_or_else(|e| format!("error: {e}")),
                Err(e) => format!("error: {e}"),
            }
        })
        .await
        .unwrap_or_else(|e| format!("error: task panicked: {e}"))
    }
}

pub struct ClipboardWriteTool;

#[async_trait]
impl Tool for ClipboardWriteTool {
    fn name(&self) -> &str {
        "clipboard_write"
    }

    fn description(&self) -> &str {
        "Write text to the system clipboard."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to copy to the clipboard"
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let text = match input["text"].as_str() {
            Some(t) => t.to_string(),
            None => return "error: missing required parameter \"text\"".to_string(),
        };

        tokio::task::spawn_blocking(move || {
            match arboard::Clipboard::new() {
                Ok(mut cb) => match cb.set_text(&text) {
                    Ok(_) => "ok: text copied to clipboard".to_string(),
                    Err(e) => format!("error: {e}"),
                },
                Err(e) => format!("error: {e}"),
            }
        })
        .await
        .unwrap_or_else(|e| format!("error: task panicked: {e}"))
    }
}
