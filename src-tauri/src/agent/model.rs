use async_trait::async_trait;

use crate::agent::session::Message;
use crate::agent::tool::Tool;
use crate::error::Result;

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

#[async_trait]
pub trait Model: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    async fn send(
        &self,
        messages: &[Message],
        tools: &[Box<dyn Tool>],
        api_key: &str,
    ) -> Result<ModelResponse>;
}
