pub mod knowledge_store;
pub mod model;
pub mod models;
pub mod playbook;
pub mod session;
pub mod session_store;
pub mod stores;
pub mod tool;
pub mod tools;

use std::collections::HashMap;

use chrono::Utc;

use crate::agent::knowledge_store::KnowledgeStore;
use crate::agent::model::Model;
use crate::agent::session::{Message, MessageContent, Role, Session};
use crate::agent::tool::Tool;
use crate::error::Result;

pub struct Agent {
    pub tools: Vec<Box<dyn Tool>>,
    pub knowledge_store: Option<Box<dyn KnowledgeStore>>,
}

impl Agent {
    pub fn new(tools: Vec<Box<dyn Tool>>) -> Self {
        Self {
            tools,
            knowledge_store: None,
        }
    }

    pub fn with_knowledge_store(mut self, store: Box<dyn KnowledgeStore>) -> Self {
        self.knowledge_store = Some(store);
        self
    }

    pub async fn run(
        &self,
        session: &mut Session,
        user_message: Message,
        model: &dyn Model,
        settings: &HashMap<String, String>,
    ) -> Result<Vec<Message>> {
        // Augment message with knowledge if store is set
        let augmented_message = if let Some(store) = &self.knowledge_store {
            if let MessageContent::Text { text } = &user_message.content {
                match store.retrieve(text).await {
                    Ok(knowledge) if !knowledge.is_empty() => Message {
                        id: user_message.id.clone(),
                        role: user_message.role.clone(),
                        content: MessageContent::Text {
                            text: format!("[参考情報]\n{}\n\n{}", knowledge, text),
                        },
                        created_at: user_message.created_at,
                        model_id: None,
                    },
                    _ => user_message.clone(),
                }
            } else {
                user_message.clone()
            }
        } else {
            user_message.clone()
        };

        let mut new_messages: Vec<Message> = Vec::new();

        // `pending` is the next message to send; it is pushed to session only AFTER send succeeds.
        let mut pending = augmented_message;

        loop {
            let response = model
                .send(Some(session), &pending, &self.tools, settings)
                .await?;

            // Push pending to session now that send succeeded
            session.messages.push(pending);
            session.updated_at = Utc::now();

            session.total_input_tokens += response.input_tokens;
            session.total_output_tokens += response.output_tokens;

            if response.tool_calls.is_empty() {
                if let Some(text) = response.text {
                    let msg = Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        role: Role::Assistant,
                        content: MessageContent::Text { text },
                        created_at: Utc::now(),
                        model_id: Some(model.model_id().to_string()),
                    };
                    session.messages.push(msg.clone());
                    new_messages.push(msg);
                }
                break;
            }

            // Append tool call messages
            for tc in &response.tool_calls {
                let msg = Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: Role::Assistant,
                    content: MessageContent::ToolCall {
                        call_id: tc.call_id.clone(),
                        tool_name: tc.tool_name.clone(),
                        input: tc.input.clone(),
                    },
                    created_at: Utc::now(),
                    model_id: Some(model.model_id().to_string()),
                };
                session.messages.push(msg.clone());
                new_messages.push(msg);
            }

            // Execute each tool and collect results
            let mut tool_results: Vec<Message> = Vec::new();
            for tc in &response.tool_calls {
                let tool = self.tools.iter().find(|t| t.name() == tc.tool_name);
                let (output, is_error) = match tool {
                    Some(t) => (t.execute(tc.input.clone()).await, false),
                    None => (format!("Tool '{}' not found", tc.tool_name), true),
                };
                tool_results.push(Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: Role::Tool,
                    content: MessageContent::ToolResult {
                        call_id: tc.call_id.clone(),
                        tool_name: tc.tool_name.clone(),
                        output,
                        is_error,
                    },
                    created_at: Utc::now(),
                    model_id: None,
                });
            }

            // Push all tool results except the last to session now.
            // The last result becomes the next `pending` so it is pushed to session
            // only after the next send succeeds, maintaining the post-send invariant.
            let next_pending = tool_results.pop().unwrap();
            for msg in tool_results {
                session.messages.push(msg.clone());
                new_messages.push(msg);
            }
            new_messages.push(next_pending.clone());
            pending = next_pending;
        }

        session.updated_at = Utc::now();
        Ok(new_messages)
    }

    pub async fn generate_title(
        &self,
        first_message: &str,
        model: &dyn Model,
        settings: &HashMap<String, String>,
    ) -> String {
        let prompt = format!(
            "Generate a short, concise title (max 6 words, no quotes, no punctuation at end) for a chat session starting with: {}",
            first_message
        );
        let title_msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::User,
            content: MessageContent::Text { text: prompt },
            created_at: Utc::now(),
            model_id: None,
        };
        match model.send(None, &title_msg, &[], settings).await {
            Ok(resp) => resp
                .text
                .unwrap_or_else(|| "New Chat".to_string())
                .trim()
                .to_string(),
            Err(_) => "New Chat".to_string(),
        }
    }
}
