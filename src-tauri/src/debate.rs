use crate::agents::{self, Agent};
use crate::commands::AppState;
use crate::config;
use crate::decisions;
use crate::llm;
use crate::profile;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

/// Build the decision brief from profile files + decision data + conversation messages.
fn compile_brief(
    app_handle: &tauri::AppHandle,
    decision_id: &str,
) -> Result<String, String> {
    let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
    let state_guard = state.lock().map_err(|e| e.to_string())?;

    let decision = state_guard.db
        .get_decision(decision_id)
        .map_err(|e| e.to_string())?
        .ok_or("Decision not found")?;

    // Read profile files
    let profiles = profile::read_all_profiles(&state_guard.app_data_dir)
        .unwrap_or_default();
    let profile_text = if profiles.is_empty() {
        "No profile information available.".to_string()
    } else {
        profiles
            .iter()
            .map(|(name, content)| format!("### {}\n{}", name, content))
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    // Get conversation messages for context
    let messages = state_guard.db
        .get_messages(&decision.conversation_id)
        .map_err(|e| e.to_string())?;
    let conversation_summary = messages
        .iter()
        .map(|m| format!("{}: {}", if m.role == "user" { "User" } else { "AI" }, m.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    // Parse summary
    let summary_text = if let Some(ref sj) = decision.summary_json {
        if let Ok(summary) = serde_json::from_str::<Value>(sj) {
            let mut parts = Vec::new();

            if let Some(options) = summary.get("options").and_then(|v| v.as_array()) {
                let opts: Vec<String> = options.iter().map(|o| {
                    let label = o["label"].as_str().unwrap_or("?");
                    let desc = o["description"].as_str().unwrap_or("");
                    if desc.is_empty() { label.to_string() } else { format!("- **{}**: {}", label, desc) }
                }).collect();
                parts.push(format!("## Options Under Consideration\n{}", opts.join("\n")));
            }

            if let Some(vars) = summary.get("variables").and_then(|v| v.as_array()) {
                let vs: Vec<String> = vars.iter().map(|v| {
                    let label = v["label"].as_str().unwrap_or("?");
                    let value = v["value"].as_str().unwrap_or("?");
                    let impact = v["impact"].as_str().unwrap_or("medium");
                    format!("- **{}**: {} (impact: {})", label, value, impact)
                }).collect();
                parts.push(format!("## Key Variables & Constraints\n{}", vs.join("\n")));
            }

            if let Some(pc) = summary.get("pros_cons").and_then(|v| v.as_array()) {
                let analysis: Vec<String> = pc.iter().map(|p| {
                    let option = p["option"].as_str().unwrap_or("?");
                    let pros = p["pros"].as_array().map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| format!("  + {}", s)).collect::<Vec<_>>().join("\n")).unwrap_or_default();
                    let cons = p["cons"].as_array().map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| format!("  - {}", s)).collect::<Vec<_>>().join("\n")).unwrap_or_default();
                    let score = p["alignment_score"].as_i64().map(|s| format!(" (alignment: {}/10)", s)).unwrap_or_default();
                    format!("### {}{}\nPros:\n{}\nCons:\n{}", option, score, pros, cons)
                }).collect();
                parts.push(format!("## Initial Analysis\n{}", analysis.join("\n\n")));
            }

            parts.join("\n\n")
        } else {
            "No structured summary available.".to_string()
        }
    } else {
        "No structured summary available.".to_string()
    };

    let brief = format!(
        r#"# Decision Brief

## About the Person
{profile_text}

## The Decision
**{title}**

### Conversation Context
{conversation_summary}

{summary_text}"#,
        title = decision.title,
    );

    Ok(brief)
}

/// Format the debate transcript so far for injection into prompts.
fn format_transcript(rounds: &[crate::db::DebateRound]) -> String {
    let mut sections: Vec<String> = Vec::new();
    let mut current_round = -1i32;
    let mut current_exchange = -1i32;

    for r in rounds {
        if r.round_number != current_round || r.exchange_number != current_exchange {
            current_round = r.round_number;
            current_exchange = r.exchange_number;
            let header = match current_round {
                1 => "--- Round 1: Opening Positions ---".to_string(),
                2 => format!("--- Round 2: Debate (Exchange {}) ---", current_exchange),
                3 => "--- Round 3: Final Positions ---".to_string(),
                99 => "--- Moderator Synthesis ---".to_string(),
                _ => format!("--- Round {} ---", current_round),
            };
            sections.push(header);
        }

        let agent = Agent::all_debaters()
            .into_iter()
            .find(|a| a.key() == r.agent)
            .or_else(|| if r.agent == "moderator" { Some(Agent::Moderator) } else { None });

        let label = agent.map(|a| format!("{} {}", a.emoji(), a.label())).unwrap_or_else(|| r.agent.clone());
        sections.push(format!("**{}**:\n{}", label, r.content));
    }

    sections.join("\n\n")
}

/// Call a single agent with retry logic, streaming tokens to frontend.
async fn call_agent_with_retry(
    api_key: &str,
    model: &str,
    agent: Agent,
    system_prompt: &str,
    user_prompt: &str,
    max_retries: u32,
    app_handle: &tauri::AppHandle,
    decision_id: &str,
    round_number: i32,
    exchange_number: i32,
) -> Result<String, String> {
    let mut last_err = String::new();
    for attempt in 0..=max_retries {
        match llm::call_llm_streaming_debate(
            api_key,
            model,
            system_prompt,
            user_prompt,
            app_handle,
            decision_id,
            round_number,
            exchange_number,
            agent.key(),
        ).await {
            Ok(text) => return Ok(text),
            Err(e) => {
                last_err = e;
                if attempt < max_retries {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }
    Err(format!("{} failed after {} retries: {}", agent.label(), max_retries + 1, last_err))
}

/// Run a full debate round where debaters respond one at a time (sequential streaming).
async fn run_sequential_round(
    api_key: &str,
    default_model: &str,
    agent_models: &HashMap<String, String>,
    brief: &str,
    existing_rounds: &[crate::db::DebateRound],
    round_number: i32,
    exchange_number: i32,
    app_handle: &tauri::AppHandle,
    decision_id: &str,
    cancel_flag: &Arc<AtomicBool>,
    app_data_dir: &std::path::PathBuf,
) -> Result<Vec<crate::db::DebateRound>, String> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Err("Debate cancelled".to_string());
    }

    let transcript = format_transcript(existing_rounds);

    let user_prompt = match round_number {
        1 => agents::round1_prompt(brief),
        2 => agents::round2_prompt(brief, &transcript, exchange_number),
        3 => agents::round3_prompt(brief, &transcript),
        _ => return Err("Invalid round number".to_string()),
    };

    let debaters = Agent::all_debaters();
    let mut new_rounds = Vec::new();

    for agent in &debaters {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Debate cancelled".to_string());
        }

        let system_prompt = agent.load_prompt(app_data_dir);
        let agent_model = agent_models.get(agent.key()).filter(|m| !m.is_empty()).map(|m| m.as_str()).unwrap_or(default_model);
        let result = call_agent_with_retry(
            api_key, agent_model,
            *agent, &system_prompt, &user_prompt, 2,
            app_handle, decision_id, round_number, exchange_number,
        ).await;

        match result {
            Ok(text) => {
                // Save to DB
                let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
                let round = {
                    let state_guard = state.lock().map_err(|e| e.to_string())?;
                    state_guard.db.save_debate_round(
                        decision_id,
                        round_number,
                        exchange_number,
                        agent.key(),
                        &text,
                    ).map_err(|e| e.to_string())?
                };

                // Emit per-agent complete event
                let _ = app_handle.emit("debate-agent-response", json!({
                    "decision_id": decision_id,
                    "round_number": round_number,
                    "exchange_number": exchange_number,
                    "agent": agent.key(),
                    "content": text,
                }));

                new_rounds.push(round);
            }
            Err(e) => {
                eprintln!("Agent call failed: {}", e);
                let _ = app_handle.emit("debate-agent-response", json!({
                    "decision_id": decision_id,
                    "round_number": round_number,
                    "exchange_number": exchange_number,
                    "agent": "error",
                    "content": format!("An agent was unable to participate: {}", e),
                }));
            }
        }
    }

    // Emit round-complete
    let _ = app_handle.emit("debate-round-complete", json!({
        "decision_id": decision_id,
        "round_number": round_number,
        "exchange_number": exchange_number,
    }));

    Ok(new_rounds)
}

/// Main debate orchestrator. Runs the full debate asynchronously.
pub async fn run_debate(
    app_handle: tauri::AppHandle,
    decision_id: String,
    quick_mode: bool,
    cancel_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    // 1. Compile brief
    let brief = compile_brief(&app_handle, &decision_id)?;

    // 2. Save brief and update status
    {
        let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
        let state_guard = state.lock().map_err(|e| e.to_string())?;
        state_guard.db.delete_debate_rounds(&decision_id).map_err(|e| e.to_string())?;
        state_guard.db.update_debate_brief(&decision_id, &brief).map_err(|e| e.to_string())?;
        state_guard.db.update_debate_started(&decision_id).map_err(|e| e.to_string())?;
    }

    // 3. Emit debate-started
    let _ = app_handle.emit("debate-started", json!({ "decision_id": decision_id }));

    // Load LLM config and app_data_dir
    let (api_key, model, agent_models, app_data_dir) = {
        let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
        let state_guard = state.lock().map_err(|e| e.to_string())?;
        let config = config::load_config(&state_guard.app_data_dir);
        (config.openrouter_api_key, config.model, config.agent_models, state_guard.app_data_dir.clone())
    };

    // Ensure agent prompt files exist
    agents::init_agent_files(&app_data_dir).ok();

    let mut all_rounds: Vec<crate::db::DebateRound> = Vec::new();

    // 4. Round 1: Opening Positions
    let round1 = run_sequential_round(
        &api_key, &model, &agent_models,
        &brief, &all_rounds, 1, 1,
        &app_handle, &decision_id, &cancel_flag, &app_data_dir,
    ).await?;
    all_rounds.extend(round1);

    if quick_mode {
        // Quick mode: skip Round 2 & 3, go straight to moderator
    } else {
        // 5. Round 2 Exchange 1
        if cancel_flag.load(Ordering::Relaxed) {
            return handle_cancellation(&app_handle, &decision_id);
        }
        let r2e1 = run_sequential_round(
            &api_key, &model, &agent_models,
            &brief, &all_rounds, 2, 1,
            &app_handle, &decision_id, &cancel_flag, &app_data_dir,
        ).await?;
        all_rounds.extend(r2e1);

        // 6. Round 2 Exchange 2
        if cancel_flag.load(Ordering::Relaxed) {
            return handle_cancellation(&app_handle, &decision_id);
        }
        let r2e2 = run_sequential_round(
            &api_key, &model, &agent_models,
            &brief, &all_rounds, 2, 2,
            &app_handle, &decision_id, &cancel_flag, &app_data_dir,
        ).await?;
        all_rounds.extend(r2e2);

        // 7. Round 3: Final Positions
        if cancel_flag.load(Ordering::Relaxed) {
            return handle_cancellation(&app_handle, &decision_id);
        }
        let round3 = run_sequential_round(
            &api_key, &model, &agent_models,
            &brief, &all_rounds, 3, 1,
            &app_handle, &decision_id, &cancel_flag, &app_data_dir,
        ).await?;
        all_rounds.extend(round3);
    }

    // 8. Moderator Synthesis
    if cancel_flag.load(Ordering::Relaxed) {
        return handle_cancellation(&app_handle, &decision_id);
    }

    let transcript = format_transcript(&all_rounds);
    let moderator_user_prompt = agents::moderator_prompt(&brief, &transcript);
    let moderator_system_prompt = Agent::Moderator.load_prompt(&app_data_dir);

    let moderator_model = agent_models.get("moderator").filter(|m| !m.is_empty()).map(|m| m.as_str()).unwrap_or(&model);
    let moderator_response = call_agent_with_retry(
        &api_key, moderator_model,
        Agent::Moderator, &moderator_system_prompt, &moderator_user_prompt, 2,
        &app_handle, &decision_id, 99, 1,
    ).await?;

    // Save moderator round
    {
        let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
        let state_guard = state.lock().map_err(|e| e.to_string())?;
        state_guard.db.save_debate_round(
            &decision_id, 99, 1, "moderator", &moderator_response,
        ).map_err(|e| e.to_string())?;
    }

    let _ = app_handle.emit("debate-agent-response", json!({
        "decision_id": decision_id,
        "round_number": 99,
        "exchange_number": 1,
        "agent": "moderator",
        "content": moderator_response,
    }));

    // 9. Parse moderator output and update decision summary
    update_summary_from_debate(&app_handle, &decision_id, &all_rounds, &moderator_response)?;

    // 10. Mark debate complete
    {
        let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
        let state_guard = state.lock().map_err(|e| e.to_string())?;
        state_guard.db.update_debate_completed(&decision_id).map_err(|e| e.to_string())?;
        state_guard.db.update_decision_status(&decision_id, "recommended").map_err(|e| e.to_string())?;
    }

    let _ = app_handle.emit("debate-complete", json!({ "decision_id": decision_id }));

    Ok(())
}

fn handle_cancellation(app_handle: &tauri::AppHandle, decision_id: &str) -> Result<(), String> {
    let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    state_guard.db.update_decision_status(decision_id, "analyzing").map_err(|e| e.to_string())?;
    let _ = app_handle.emit("debate-error", json!({
        "decision_id": decision_id,
        "error": "Debate cancelled",
    }));
    Ok(())
}

/// Extract final votes from Round 3 and build debate_summary for the decision.
fn update_summary_from_debate(
    app_handle: &tauri::AppHandle,
    decision_id: &str,
    all_rounds: &[crate::db::DebateRound],
    moderator_response: &str,
) -> Result<(), String> {
    let mut final_votes = serde_json::Map::new();
    let debaters = Agent::all_debaters();

    for agent in &debaters {
        let last_entry = all_rounds.iter()
            .filter(|r| r.agent == agent.key())
            .last();
        if let Some(entry) = last_entry {
            let vote = entry.content.chars().take(200).collect::<String>();
            final_votes.insert(agent.key().to_string(), Value::String(vote));
        }
    }

    let consensus = extract_section(moderator_response, "Where the Committee Agreed");
    let disagreements = extract_section(moderator_response, "Key Disagreements");
    let biases = extract_section(moderator_response, "Biases & Blind Spots Identified");

    let debate_summary = json!({
        "consensus_points": split_to_points(&consensus),
        "key_disagreements": split_to_points(&disagreements),
        "biases_identified": split_to_points(&biases),
        "final_votes": final_votes,
    });

    let rec_section = extract_section(moderator_response, "Recommendation");
    let recommendation = parse_moderator_recommendation(&rec_section, moderator_response);

    let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
    let state_guard = state.lock().map_err(|e| e.to_string())?;

    let existing_summary = state_guard.db
        .get_decision(decision_id)
        .ok()
        .flatten()
        .and_then(|d| d.summary_json);

    let update = if let Some(rec) = recommendation {
        json!({
            "debate_summary": debate_summary,
            "recommendation": rec,
        })
    } else {
        json!({
            "debate_summary": debate_summary,
        })
    };

    let merged = decisions::merge_summary(existing_summary.as_deref(), &update);
    state_guard.db.update_decision_summary(decision_id, &merged).map_err(|e| e.to_string())?;

    let _ = app_handle.emit("decision-summary-updated", json!({
        "decision_id": decision_id,
        "summary": merged,
        "status": "recommended",
    }));

    Ok(())
}

/// Extract a markdown section by heading.
fn extract_section(text: &str, heading: &str) -> String {
    let marker = format!("## {}", heading);
    if let Some(start) = text.find(&marker) {
        let after = &text[start + marker.len()..];
        let end = after.find("\n## ").unwrap_or(after.len());
        after[..end].trim().to_string()
    } else {
        String::new()
    }
}

/// Split text into bullet points.
fn split_to_points(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    text.lines()
        .map(|l| l.trim().trim_start_matches('-').trim_start_matches('*').trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// Parse the moderator's recommendation section into a structured Recommendation object.
fn parse_moderator_recommendation(rec_section: &str, full_text: &str) -> Option<Value> {
    if rec_section.is_empty() && !full_text.contains("**Choice**") {
        return None;
    }

    let text = if rec_section.is_empty() { full_text } else { rec_section };

    let choice = extract_bold_value(text, "Choice")
        .unwrap_or_else(|| "See moderator's synthesis".to_string());
    let confidence = extract_bold_value(text, "Confidence")
        .unwrap_or_else(|| "medium".to_string())
        .to_lowercase();
    let reasoning = extract_bold_value(text, "Reasoning")
        .unwrap_or_else(|| {
            rec_section.lines()
                .filter(|l| !l.starts_with("**"))
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        });

    let tradeoffs = extract_section(full_text, "What You're Giving Up");

    let action_plan = extract_section(full_text, "Action Plan");
    let next_steps: Vec<String> = split_to_points(&action_plan);

    let conf = if confidence.contains("high") {
        "high"
    } else if confidence.contains("low") {
        "low"
    } else {
        "medium"
    };

    Some(json!({
        "choice": choice,
        "confidence": conf,
        "reasoning": reasoning,
        "tradeoffs": if tradeoffs.is_empty() { None } else { Some(tradeoffs) },
        "next_steps": if next_steps.is_empty() { None } else { Some(next_steps) },
    }))
}

/// Extract a value after a bold label like **Choice**: value
fn extract_bold_value(text: &str, label: &str) -> Option<String> {
    let pattern = format!("**{}**:", label);
    if let Some(pos) = text.find(&pattern) {
        let after = &text[pos + pattern.len()..];
        let end = after.find('\n').unwrap_or(after.len());
        let value = after[..end].trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    } else {
        None
    }
}
