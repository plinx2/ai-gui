pub mod agent;
pub mod config;
pub mod error;

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::sync::Mutex;

use crate::agent::model::{ConfigField, ModelInfo, ModelRegistry};
use crate::agent::models::agent_api::AgentApiModel;
use crate::agent::models::gemini::GeminiModel;
use crate::agent::playbook::{Playbook, PlaybookStore};
use crate::agent::session::{Message, MessageContent, Role, Session, SessionSummary};
use crate::agent::session_store::SessionStore;
use crate::agent::stores::local_playbook_store::LocalPlaybookStore;
use crate::agent::stores::local_session_store::LocalSessionStore;
use crate::agent::tools::browser::create_browser_tools;
use crate::agent::tools::choice::{ChoiceTool, PendingChoices};
use crate::agent::tools::clipboard::{ClipboardReadTool, ClipboardWriteTool};
use crate::agent::tools::filesystem::{
    CopyFileTool, ListDirectoryTool, MoveFileTool, ReadFileTool, SearchInFilesTool, TrashFileTool,
    WriteFileTool,
};
use crate::agent::tools::http::HttpRequestTool;
use crate::agent::tools::shell::ShellTool;
use crate::agent::tools::ssh::{SshDownloadTool, SshExecTool, SshListHostsTool, SshUploadTool};
use crate::agent::tools::time::TimeTool;
use crate::agent::Agent;
use crate::config::{load_config, save_config, Config};

pub struct AppState {
    pub config: Mutex<Config>,
    pub sessions: Mutex<HashMap<String, Session>>,
    pub agent: Agent,
    pub session_store: Box<dyn SessionStore>,
    pub playbook_store: Box<dyn PlaybookStore>,
    pub http_client: reqwest::Client,
    pub pending_choices: PendingChoices,
    /// Read-only after setup — no Mutex needed.
    pub model_registry: ModelRegistry,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResponse {
    pub session_id: String,
    pub session_title: String,
    pub new_messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileAttachmentInput {
    pub name: String,
    pub mime_type: String,
    pub data_base64: String,
}

// --- Tauri Commands ---

#[tauri::command]
async fn get_sessions(state: tauri::State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    let sessions = state.sessions.lock().await;
    let mut summaries: Vec<SessionSummary> = sessions.values().map(SessionSummary::from).collect();
    summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(summaries)
}

#[tauri::command]
async fn get_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<Session, String> {
    let sessions = state.sessions.lock().await;
    sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| format!("Session not found: {}", session_id))
}

#[tauri::command]
async fn send_message(
    state: tauri::State<'_, AppState>,
    session_id: Option<String>,
    content: String,
    model_id: String,
    file_attachment: Option<FileAttachmentInput>,
) -> Result<SendMessageResponse, String> {
    // Read config once and release the lock before any async API calls
    let (settings, mut session) = {
        let cfg = state.config.lock().await;

        let session = if let Some(id) = &session_id {
            let sessions = state.sessions.lock().await;
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Session not found: {}", id))?
        } else {
            Session {
                id: uuid::Uuid::new_v4().to_string(),
                title: "New Chat".to_string(),
                model_name: model_id.clone(),
                messages: Vec::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                total_input_tokens: 0,
                total_output_tokens: 0,
            }
        };
        (cfg.settings.clone(), session)
    };

    // Resolve the model from the registry
    let model = state
        .model_registry
        .get(&model_id)
        .ok_or_else(|| format!("Unknown model: {}", model_id))?;

    // Availability guard
    if !model.is_available(&settings) {
        return Err(format!(
            "Model '{}' is not available. Please set the required API key in Settings.",
            model_id
        ));
    }

    // Build user message content
    let user_content = if let Some(attachment) = file_attachment {
        MessageContent::FileAttachment {
            name: attachment.name,
            mime_type: attachment.mime_type,
            data_base64: attachment.data_base64,
        }
    } else {
        MessageContent::Text {
            text: content.clone(),
        }
    };

    let user_message = Message {
        id: uuid::Uuid::new_v4().to_string(),
        role: Role::User,
        content: user_content,
        created_at: Utc::now(),
        model_id: None,
    };

    // Generate title for new sessions
    if session_id.is_none() {
        let title = state.agent.generate_title(&content, model, &settings).await;
        session.title = title;
    }

    // Run agent loop (no lock held during API calls)
    let new_messages = state
        .agent
        .run(&mut session, user_message, model, &settings)
        .await
        .map_err(|e| e.to_string())?;

    let response = SendMessageResponse {
        session_id: session.id.clone(),
        session_title: session.title.clone(),
        new_messages,
    };

    // Save to store and update in-memory map
    state
        .session_store
        .save(&session)
        .await
        .map_err(|e| e.to_string())?;

    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(session.id.clone(), session);
    }

    Ok(response)
}

#[tauri::command]
async fn get_models(state: tauri::State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let cfg = state.config.lock().await;
    Ok(state.model_registry.list(&cfg.settings))
}

#[tauri::command]
async fn get_config_schema(state: tauri::State<'_, AppState>) -> Result<Vec<ConfigField>, String> {
    Ok(state.model_registry.config_schema())
}

#[tauri::command]
async fn delete_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    state
        .session_store
        .delete(&session_id)
        .await
        .map_err(|e| e.to_string())?;
    let mut sessions = state.sessions.lock().await;
    sessions.remove(&session_id);
    Ok(())
}

#[tauri::command]
async fn get_playbooks(state: tauri::State<'_, AppState>) -> Result<Vec<Playbook>, String> {
    state
        .playbook_store
        .load_all()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_playbook(
    state: tauri::State<'_, AppState>,
    playbook: Playbook,
) -> Result<(), String> {
    state
        .playbook_store
        .save(&playbook)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_playbook(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .playbook_store
        .delete(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn run_playbook(
    state: tauri::State<'_, AppState>,
    playbook_id: String,
    user_message: Option<String>,
) -> Result<SendMessageResponse, String> {
    // Load config
    let settings = {
        let cfg = state.config.lock().await;
        cfg.settings.clone()
    };

    // Find playbook
    let playbooks = state
        .playbook_store
        .load_all()
        .await
        .map_err(|e| e.to_string())?;
    let playbook = playbooks
        .iter()
        .find(|p| p.id == playbook_id)
        .ok_or_else(|| format!("Playbook not found: {}", playbook_id))?
        .clone();

    // Resolve model (model_id must be set on the playbook)
    let model_id = playbook
        .model_id
        .clone()
        .ok_or_else(|| "Playbook has no model set. Please set a model before running.".to_string())?;
    let model = state
        .model_registry
        .get(&model_id)
        .ok_or_else(|| format!("Unknown model: {}", model_id))?;
    if !model.is_available(&settings) {
        return Err(format!(
            "Model '{}' is not available. Please set the required API key in Settings.",
            model_id
        ));
    }

    // Build single prompt
    let mut prompt = format!("# {}", playbook.title);
    if !playbook.description.is_empty() {
        prompt.push_str(&format!("\n{}", playbook.description));
    }
    if let Some(msg) = &user_message {
        let msg = msg.trim();
        if !msg.is_empty() {
            prompt.push_str(&format!("\n\n{}", msg));
        }
    }
    if !playbook.steps.is_empty() {
        prompt.push_str("\n\n## Steps\n\n");
        for (i, step) in playbook.steps.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, step));
        }
    }
    if !playbook.notes.is_empty() {
        prompt.push_str(&format!("\n\n## Notes\n\n{}", playbook.notes));
    }

    // Create a new session with the playbook's title
    let mut session = Session {
        id: uuid::Uuid::new_v4().to_string(),
        title: playbook.title.clone(),
        model_name: model_id.clone(),
        messages: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        total_input_tokens: 0,
        total_output_tokens: 0,
    };

    let user_msg = Message {
        id: uuid::Uuid::new_v4().to_string(),
        role: Role::User,
        content: MessageContent::Text { text: prompt },
        created_at: Utc::now(),
        model_id: None,
    };

    // Run agent
    let new_messages = state
        .agent
        .run(&mut session, user_msg, model, &settings)
        .await
        .map_err(|e| e.to_string())?;

    let response = SendMessageResponse {
        session_id: session.id.clone(),
        session_title: session.title.clone(),
        new_messages,
    };

    // Persist
    state
        .session_store
        .save(&session)
        .await
        .map_err(|e| e.to_string())?;
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(session.id.clone(), session);
    }

    Ok(response)
}

#[tauri::command]
async fn get_config(state: tauri::State<'_, AppState>) -> Result<Config, String> {
    let cfg = state.config.lock().await;
    Ok(cfg.clone())
}

#[tauri::command]
async fn submit_choice(
    state: tauri::State<'_, AppState>,
    call_id: String,
    answer: String,
) -> Result<(), String> {
    let mut pending = state.pending_choices.lock().await;
    match pending.remove(&call_id) {
        Some(sender) => {
            let _ = sender.send(answer);
            Ok(())
        }
        None => Err(format!("No pending choice with id: {}", call_id)),
    }
}

#[tauri::command]
async fn get_config_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    Ok(config_dir
        .join("config.json")
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
async fn update_config(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    config: Config,
) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    save_config(&config_dir, &config)
        .await
        .map_err(|e| e.to_string())?;
    let mut cfg = state.config.lock().await;
    *cfg = config;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let config_dir = handle.path().app_config_dir().expect("No config dir");
                let data_dir = handle.path().app_data_dir().expect("No data dir");
                let sessions_dir = data_dir.join("sessions");

                let mut config = load_config(&config_dir).await.unwrap_or_default();

                let session_store = LocalSessionStore::new(sessions_dir)
                    .await
                    .expect("Failed to create session store");
                let all_sessions = session_store.load_all().await.unwrap_or_default();
                let sessions_map: HashMap<String, Session> = all_sessions
                    .into_iter()
                    .map(|s| (s.id.clone(), s))
                    .collect();

                let playbooks_dir = data_dir.join("playbooks");
                let playbook_store = LocalPlaybookStore::new(playbooks_dir)
                    .await
                    .expect("Failed to create playbook store");

                let http_client = reqwest::Client::new();

                // Build model registry
                let mut model_registry = ModelRegistry::new();
                let gemini_variants = [
                    "gemini-2.5-flash",
                    "gemini-2.5-pro",
                    "gemini-1.5-flash",
                    "gemini-1.5-pro",
                ];
                for variant in gemini_variants {
                    model_registry.register(Box::new(GeminiModel::with_client(
                        variant,
                        http_client.clone(),
                    )));
                }

                model_registry.register(Box::new(AgentApiModel::new(http_client.clone())));

                // Seed any missing config keys and save (also handles legacy migration)
                model_registry.seed_config(&mut config.settings);
                let _ = save_config(&config_dir, &config).await;

                let pending_choices: PendingChoices = Arc::new(Mutex::new(HashMap::new()));
                let (_, browser_tools) = create_browser_tools();
                let mut tools: Vec<Box<dyn crate::agent::tool::Tool>> = vec![
                    Box::new(TimeTool),
                    Box::new(ShellTool),
                    Box::new(ChoiceTool::new(
                        handle.clone(),
                        Arc::clone(&pending_choices),
                    )),
                    // Filesystem
                    Box::new(ReadFileTool),
                    Box::new(WriteFileTool),
                    Box::new(ListDirectoryTool),
                    Box::new(SearchInFilesTool),
                    Box::new(MoveFileTool),
                    Box::new(CopyFileTool),
                    Box::new(TrashFileTool),
                    // HTTP
                    Box::new(HttpRequestTool::new(reqwest::Client::new())),
                    // SSH
                    Box::new(SshListHostsTool),
                    Box::new(SshExecTool),
                    Box::new(SshUploadTool),
                    Box::new(SshDownloadTool),
                    // Clipboard
                    Box::new(ClipboardReadTool),
                    Box::new(ClipboardWriteTool),
                ];
                tools.extend(browser_tools);
                let agent = Agent::new(tools);

                let state = AppState {
                    config: Mutex::new(config),
                    sessions: Mutex::new(sessions_map),
                    agent,
                    session_store: Box::new(session_store),
                    playbook_store: Box::new(playbook_store),
                    http_client,
                    pending_choices,
                    model_registry,
                };
                handle.manage(state);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_sessions,
            get_session,
            send_message,
            delete_session,
            get_config,
            get_config_path,
            update_config,
            submit_choice,
            get_playbooks,
            save_playbook,
            delete_playbook,
            run_playbook,
            get_models,
            get_config_schema,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
