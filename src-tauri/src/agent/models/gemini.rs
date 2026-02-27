use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agent::model::{Model, ModelResponse, ToolCallRequest};
use crate::agent::session::{Message, MessageContent, Role};
use crate::agent::tool::Tool;
use crate::error::{AppError, Result};

const GEMINI_API_BASE: &str =
    "https://generativelanguage.googleapis.com/v1beta/models";

pub struct GeminiModel {
    model_name: String,
    client: reqwest::Client,
}

impl GeminiModel {
    pub fn new(model_name: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_client(model_name: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            model_name: model_name.into(),
            client,
        }
    }
}

// --- Request types ---

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<GeminiToolDeclarations>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_config: Option<GeminiToolConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiInlineData,
    },
}

#[derive(Serialize, Deserialize, Clone)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone)]
struct GeminiInlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

#[derive(Serialize)]
struct GeminiToolDeclarations {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct GeminiToolConfig {
    #[serde(rename = "functionCallingConfig")]
    function_calling_config: GeminiFunctionCallingConfig,
}

#[derive(Serialize)]
struct GeminiFunctionCallingConfig {
    mode: String,
}

// --- Response types ---

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata", default)]
    usage_metadata: GeminiUsageMetadata,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(rename = "finishReason", default)]
    _finish_reason: String,
}

#[derive(Deserialize, Default)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount", default)]
    prompt_token_count: u64,
    #[serde(rename = "candidatesTokenCount", default)]
    candidates_token_count: u64,
}

// --- Message conversion ---

fn messages_to_gemini_contents(messages: &[Message]) -> Vec<GeminiContent> {
    let mut contents: Vec<GeminiContent> = Vec::new();

    let mut i = 0;
    while i < messages.len() {
        let msg = &messages[i];
        match &msg.role {
            Role::User => {
                let parts = match &msg.content {
                    MessageContent::Text { text } => vec![GeminiPart::Text { text: text.clone() }],
                    MessageContent::FileAttachment {
                        mime_type,
                        data_base64,
                        ..
                    } => vec![GeminiPart::InlineData {
                        inline_data: GeminiInlineData {
                            mime_type: mime_type.clone(),
                            data: data_base64.clone(),
                        },
                    }],
                    _ => vec![],
                };
                if !parts.is_empty() {
                    contents.push(GeminiContent {
                        role: "user".to_string(),
                        parts,
                    });
                }
                i += 1;
            }
            Role::Assistant => {
                // Collect all consecutive assistant messages (text + tool calls)
                let mut parts = Vec::new();
                while i < messages.len() {
                    if let Role::Assistant = &messages[i].role {
                        match &messages[i].content {
                            MessageContent::Text { text } => {
                                parts.push(GeminiPart::Text { text: text.clone() });
                            }
                            MessageContent::ToolCall {
                                tool_name, input, ..
                            } => {
                                parts.push(GeminiPart::FunctionCall {
                                    function_call: GeminiFunctionCall {
                                        name: tool_name.clone(),
                                        args: input.clone(),
                                    },
                                });
                            }
                            _ => {}
                        }
                        i += 1;
                    } else {
                        break;
                    }
                }
                if !parts.is_empty() {
                    contents.push(GeminiContent {
                        role: "model".to_string(),
                        parts,
                    });
                }
            }
            Role::Tool => {
                // Collect all consecutive tool results into one "user" content block.
                // Screenshot results (prefixed with "SCREENSHOT:mime:base64") become an
                // inlineData part alongside the functionResponse so Gemini can see the image.
                let mut parts = Vec::new();
                while i < messages.len() {
                    if let Role::Tool = &messages[i].role {
                        if let MessageContent::ToolResult {
                            tool_name, output, ..
                        } = &messages[i].content
                        {
                            if let Some(rest) = output.strip_prefix("SCREENSHOT:") {
                                // Format: "SCREENSHOT:mime_type:base64data"
                                if let Some(colon) = rest.find(':') {
                                    let mime_type = rest[..colon].to_string();
                                    let data = rest[colon + 1..].to_string();
                                    parts.push(GeminiPart::FunctionResponse {
                                        function_response: GeminiFunctionResponse {
                                            name: tool_name.clone(),
                                            response: serde_json::json!({
                                                "output": "Screenshot captured — see attached image"
                                            }),
                                        },
                                    });
                                    parts.push(GeminiPart::InlineData {
                                        inline_data: GeminiInlineData { mime_type, data },
                                    });
                                } else {
                                    parts.push(GeminiPart::FunctionResponse {
                                        function_response: GeminiFunctionResponse {
                                            name: tool_name.clone(),
                                            response: serde_json::json!({ "output": output }),
                                        },
                                    });
                                }
                            } else {
                                parts.push(GeminiPart::FunctionResponse {
                                    function_response: GeminiFunctionResponse {
                                        name: tool_name.clone(),
                                        response: serde_json::json!({ "output": output }),
                                    },
                                });
                            }
                        }
                        i += 1;
                    } else {
                        break;
                    }
                }
                if !parts.is_empty() {
                    contents.push(GeminiContent {
                        role: "user".to_string(),
                        parts,
                    });
                }
            }
        }
    }

    contents
}

#[async_trait]
impl Model for GeminiModel {
    fn name(&self) -> &str {
        &self.model_name
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn send(
        &self,
        messages: &[Message],
        tools: &[Box<dyn Tool>],
        api_key: &str,
    ) -> Result<ModelResponse> {
        let contents = messages_to_gemini_contents(messages);

        let tool_declarations: Vec<GeminiFunctionDeclaration> = tools
            .iter()
            .map(|t| GeminiFunctionDeclaration {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters_schema(),
            })
            .collect();

        let (tools_field, tool_config) = if tool_declarations.is_empty() {
            (vec![], None)
        } else {
            (
                vec![GeminiToolDeclarations {
                    function_declarations: tool_declarations,
                }],
                Some(GeminiToolConfig {
                    function_calling_config: GeminiFunctionCallingConfig {
                        mode: "AUTO".to_string(),
                    },
                }),
            )
        };

        let request = GeminiRequest {
            contents,
            tools: tools_field,
            tool_config,
        };

        let url = format!(
            "{}/{}:generateContent?key={}",
            GEMINI_API_BASE, self.model_name, api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "Gemini API error {}: {}",
                status, body
            )));
        }

        let gemini_resp: GeminiResponse = response.json().await?;

        let candidate = gemini_resp
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Api("No candidates in Gemini response".to_string()))?;

        let mut text: Option<String> = None;
        let mut tool_calls: Vec<ToolCallRequest> = Vec::new();

        for part in candidate.content.parts {
            match part {
                GeminiPart::Text { text: t } => {
                    text = Some(t);
                }
                GeminiPart::FunctionCall { function_call } => {
                    tool_calls.push(ToolCallRequest {
                        call_id: uuid::Uuid::new_v4().to_string(),
                        tool_name: function_call.name,
                        input: function_call.args,
                    });
                }
                _ => {}
            }
        }

        Ok(ModelResponse {
            text,
            tool_calls,
            input_tokens: gemini_resp.usage_metadata.prompt_token_count,
            output_tokens: gemini_resp.usage_metadata.candidates_token_count,
        })
    }
}
