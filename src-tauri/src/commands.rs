use crate::agents;
use crate::config::{self, AppConfig};
use crate::db::{Database, DebateRound, Decision};
use crate::debate;
use crate::llm;
use crate::profile;
use crate::profile::ProfileFileInfo;
use crate::llm::StreamEvent;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;
use std::sync::Mutex;

pub struct AppState {
    pub db: Database,
    pub app_data_dir: PathBuf,
    pub debate_cancel_flags: HashMap<String, Arc<AtomicBool>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub conversation_id: String,
    pub response: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub api_key_set: bool,
    pub api_key_preview: String,
    pub model: String,
    pub agent_models: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDecisionResponse {
    pub conversation_id: String,
    pub decision_id: String,
}

fn db_err(e: rusqlite::Error) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn send_message(
    app_handle: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    conversation_id: Option<String>,
    message: String,
    on_event: Channel<StreamEvent>,
) -> Result<SendMessageResponse, String> {
    let (api_key, model, conv_id, history_messages, conv_type, decision_id) = {
        let state = state.lock().map_err(|e| e.to_string())?;
        let config = config::load_config(&state.app_data_dir);

        if config.openrouter_api_key.is_empty() {
            return Err("API key not set. Please go to Settings to add your OpenRouter API key.".to_string());
        }

        let conv_id = match conversation_id {
            Some(id) => id,
            None => {
                let title = if message.len() > 50 {
                    format!("{}...", &message[..50])
                } else {
                    message.clone()
                };
                let conv = state.db.create_conversation(&title).map_err(db_err)?;
                conv.id
            }
        };

        state.db.add_message(&conv_id, "user", &message).map_err(db_err)?;

        let messages = state.db.get_messages(&conv_id).map_err(db_err)?;
        let history: Vec<serde_json::Value> = messages.iter().map(|m| {
            json!({
                "role": m.role,
                "content": m.content,
            })
        }).collect();

        let conv = state.db.get_conversation(&conv_id).map_err(db_err)?;
        let conv_type = conv.map(|c| c.conv_type).unwrap_or_else(|| "chat".to_string());

        let decision_id = if conv_type == "decision" {
            state.db.get_decision_by_conversation(&conv_id)
                .map_err(db_err)?
                .map(|d| d.id)
        } else {
            None
        };

        (config.openrouter_api_key, config.model, conv_id, history, conv_type, decision_id)
    };

    let app_data_dir = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.app_data_dir.clone()
    };

    let response_text = llm::send_message(
        &api_key,
        &model,
        history_messages,
        &app_data_dir,
        &on_event,
        &conv_type,
        decision_id.as_deref(),
        &app_handle,
    ).await?;

    {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.db.add_message(&conv_id, "assistant", &response_text).map_err(db_err)?;
    }

    Ok(SendMessageResponse {
        conversation_id: conv_id,
        response: response_text,
    })
}

#[tauri::command]
pub fn get_conversations(state: State<'_, Mutex<AppState>>) -> Result<Vec<crate::db::Conversation>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    state.db.get_conversations_by_type("chat").map_err(db_err)
}

#[tauri::command]
pub fn get_messages(state: State<'_, Mutex<AppState>>, conversation_id: String) -> Result<Vec<crate::db::Message>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    state.db.get_messages(&conversation_id).map_err(db_err)
}

#[tauri::command]
pub fn get_settings(state: State<'_, Mutex<AppState>>) -> Result<SettingsResponse, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let config = config::load_config(&state.app_data_dir);
    let preview = if config.openrouter_api_key.len() > 8 {
        format!("{}...{}", &config.openrouter_api_key[..4], &config.openrouter_api_key[config.openrouter_api_key.len()-4..])
    } else if !config.openrouter_api_key.is_empty() {
        "****".to_string()
    } else {
        String::new()
    };
    Ok(SettingsResponse {
        api_key_set: !config.openrouter_api_key.is_empty(),
        api_key_preview: preview,
        model: config.model,
        agent_models: config.agent_models,
    })
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, Mutex<AppState>>,
    api_key: String,
    model: String,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let existing = config::load_config(&state.app_data_dir);
    let final_key = if api_key.is_empty() { existing.openrouter_api_key } else { api_key };
    let config = AppConfig {
        openrouter_api_key: final_key,
        model,
        agent_models: existing.agent_models,
    };
    config::save_config(&state.app_data_dir, &config)
}

#[tauri::command]
pub fn get_profile_files(state: State<'_, Mutex<AppState>>) -> Result<std::collections::HashMap<String, String>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    profile::read_all_profiles(&state.app_data_dir)
}

#[tauri::command]
pub fn open_profile_folder(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let dir = profile::get_profile_dir(&state.app_data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn delete_conversation(state: State<'_, Mutex<AppState>>, conversation_id: String) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    state.db.delete_conversation(&conversation_id).map_err(db_err)
}

// ── Decision Commands ──

#[tauri::command]
pub fn create_decision(state: State<'_, Mutex<AppState>>, title: String) -> Result<CreateDecisionResponse, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conv = state.db.create_conversation_with_type(&title, "decision").map_err(db_err)?;
    let decision = state.db.create_decision(&conv.id, &title).map_err(db_err)?;
    Ok(CreateDecisionResponse {
        conversation_id: conv.id,
        decision_id: decision.id,
    })
}

#[tauri::command]
pub fn get_decisions(state: State<'_, Mutex<AppState>>) -> Result<Vec<Decision>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    state.db.get_decisions().map_err(db_err)
}

#[tauri::command]
pub fn get_decision(state: State<'_, Mutex<AppState>>, decision_id: String) -> Result<Decision, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    state.db.get_decision(&decision_id)
        .map_err(db_err)?
        .ok_or_else(|| "Decision not found".to_string())
}

#[tauri::command]
pub fn get_decision_by_conversation(state: State<'_, Mutex<AppState>>, conversation_id: String) -> Result<Decision, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    state.db.get_decision_by_conversation(&conversation_id)
        .map_err(db_err)?
        .ok_or_else(|| "Decision not found".to_string())
}

#[tauri::command]
pub fn update_decision_status(
    state: State<'_, Mutex<AppState>>,
    decision_id: String,
    status: String,
    user_choice: Option<String>,
    user_choice_reasoning: Option<String>,
    outcome: Option<String>,
) -> Result<Decision, String> {
    let state = state.lock().map_err(|e| e.to_string())?;

    match status.as_str() {
        "decided" => {
            let choice = user_choice.ok_or("user_choice is required when status is 'decided'")?;
            state.db.update_decision_choice(&decision_id, &choice, user_choice_reasoning.as_deref()).map_err(db_err)?;
        }
        "reviewed" => {
            let outcome_text = outcome.ok_or("outcome is required when status is 'reviewed'")?;
            state.db.update_decision_outcome(&decision_id, &outcome_text).map_err(db_err)?;
        }
        _ => {
            state.db.update_decision_status(&decision_id, &status).map_err(db_err)?;
        }
    }

    state.db.get_decision(&decision_id)
        .map_err(db_err)?
        .ok_or_else(|| "Decision not found after update".to_string())
}

// ── Profile Viewer Commands ──

#[tauri::command]
pub fn get_profile_files_detailed(state: State<'_, Mutex<AppState>>) -> Result<Vec<ProfileFileInfo>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    profile::read_all_profiles_detailed(&state.app_data_dir)
}

#[tauri::command]
pub fn update_profile_file(state: State<'_, Mutex<AppState>>, filename: String, content: String) -> Result<ProfileFileInfo, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    profile::write_profile_file(&state.app_data_dir, &filename, &content)?;
    let dir = profile::get_profile_dir(&state.app_data_dir);
    let path = dir.join(&filename);
    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let modified = metadata.modified().map_err(|e| e.to_string())?;
    let modified_at = chrono::DateTime::<chrono::Utc>::from(modified)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    Ok(ProfileFileInfo {
        filename,
        content,
        modified_at,
        size_bytes: metadata.len(),
    })
}

#[tauri::command]
pub fn remove_profile_file(state: State<'_, Mutex<AppState>>, filename: String) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    profile::delete_profile_file(&state.app_data_dir, &filename)?;
    Ok(())
}

// ── Committee Agent Commands ──

#[tauri::command]
pub fn get_agent_files(state: State<'_, Mutex<AppState>>) -> Result<Vec<agents::AgentFileInfo>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    agents::read_all_agent_files(&state.app_data_dir)
}

#[tauri::command]
pub fn update_agent_file(state: State<'_, Mutex<AppState>>, filename: String, content: String) -> Result<agents::AgentFileInfo, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    agents::write_agent_file(&state.app_data_dir, &filename, &content)?;
    let dir = agents::get_agents_dir(&state.app_data_dir);
    let path = dir.join(&filename);
    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let modified = metadata.modified().map_err(|e| e.to_string())?;
    let modified_at = chrono::DateTime::<chrono::Utc>::from(modified)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    Ok(agents::AgentFileInfo {
        filename,
        content,
        modified_at,
        size_bytes: metadata.len(),
    })
}

#[tauri::command]
pub fn save_agent_model(
    state: State<'_, Mutex<AppState>>,
    agent_key: String,
    model: String,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let mut config = config::load_config(&state.app_data_dir);
    if model.is_empty() {
        config.agent_models.remove(&agent_key);
    } else {
        config.agent_models.insert(agent_key, model);
    }
    config::save_config(&state.app_data_dir, &config)
}

#[tauri::command]
pub fn open_agents_folder(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let dir = agents::get_agents_dir(&state.app_data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

// ── Debate Commands ──

#[tauri::command]
pub async fn start_debate(
    app_handle: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    decision_id: String,
    quick_mode: bool,
) -> Result<(), String> {
    {
        let state = state.lock().map_err(|e| e.to_string())?;
        let decision = state.db.get_decision(&decision_id)
            .map_err(db_err)?
            .ok_or_else(|| "Decision not found".to_string())?;

        if let Some(ref summary_json) = decision.summary_json {
            let summary: serde_json::Value = serde_json::from_str(summary_json)
                .map_err(|_| "Invalid summary JSON".to_string())?;
            let has_options = summary.get("options")
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            let has_variables = summary.get("variables")
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            if !has_options || !has_variables {
                return Err("Decision needs at least one option and one variable before starting a debate.".to_string());
            }
        } else {
            return Err("Decision has no summary data. Chat with the AI first to build context.".to_string());
        }
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        state.debate_cancel_flags.insert(decision_id.clone(), cancel_flag.clone());
    }

    let dec_id = decision_id.clone();
    tokio::spawn(async move {
        if let Err(e) = debate::run_debate(app_handle.clone(), dec_id.clone(), quick_mode, cancel_flag).await {
            eprintln!("Debate error: {}", e);
            let _ = tauri::Emitter::emit(&app_handle, "debate-error", serde_json::json!({
                "decision_id": dec_id,
                "error": e,
            }));
        }
    });

    Ok(())
}

#[tauri::command]
pub fn get_debate(state: State<'_, Mutex<AppState>>, decision_id: String) -> Result<Vec<DebateRound>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    state.db.get_debate_rounds(&decision_id).map_err(db_err)
}

#[tauri::command]
pub fn cancel_debate(state: State<'_, Mutex<AppState>>, decision_id: String) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    if let Some(flag) = state.debate_cancel_flags.get(&decision_id) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    state.db.update_decision_status(&decision_id, "analyzing").map_err(db_err)?;
    state.debate_cancel_flags.remove(&decision_id);
    Ok(())
}
