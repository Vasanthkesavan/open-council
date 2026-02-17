use crate::agents::{self, AgentInfo};
use crate::commands::AppState;
use crate::config;
use crate::decisions;
use crate::llm;
use crate::profile;
use crate::tts;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

/// Normalize model output so spoken debate feels conversational in UI + TTS.
fn normalize_spoken_debate_output(text: &str) -> String {
    let labels = [
        "position:",
        "key argument:",
        "concern:",
        "my vote:",
        "shifted?:",
        "remember this:",
    ];

    let mut parts: Vec<String> = Vec::new();
    for raw_line in text.lines() {
        let mut line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Remove markdown heading prefixes.
        while let Some(rest) = line.strip_prefix('#') {
            line = rest.trim_start();
        }

        // Remove common list markers.
        if let Some(rest) = line.strip_prefix("- ") {
            line = rest.trim_start();
        } else if let Some(rest) = line.strip_prefix("* ") {
            line = rest.trim_start();
        } else if let Some(rest) = line.strip_prefix("• ") {
            line = rest.trim_start();
        }

        // Remove numbered-list prefixes like "1. ".
        if let Some(dot_pos) = line.find(". ") {
            if line[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
                line = line[dot_pos + 2..].trim_start();
            }
        }

        let mut cleaned = line
            .replace("**", "")
            .replace("__", "")
            .replace('`', "");

        let lower = cleaned.to_ascii_lowercase();
        for label in labels {
            if lower.starts_with(label) {
                cleaned = cleaned[label.len()..].trim().to_string();
                break;
            }
        }

        if !cleaned.is_empty() {
            parts.push(cleaned);
        }
    }

    let merged = parts.join(" ");
    let compact = merged
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" .", ".")
        .replace(" ,", ",")
        .replace(" ;", ";")
        .replace(" :", ":");

    if compact.is_empty() {
        text.trim().to_string()
    } else {
        compact
    }
}

/// Shared state for live TTS generation during debate.
struct LiveTtsState {
    enabled: bool,
    config: config::AppConfig,
    registry: Vec<AgentInfo>,
    app_data_dir: std::path::PathBuf,
    segment_counter: Arc<AtomicUsize>,
    handles: Arc<Mutex<Vec<tokio::task::JoinHandle<Option<tts::AudioSegment>>>>>,
}

/// Spawn a TTS generation task for a single debate round segment.
fn spawn_segment_tts(
    tts_state: &LiveTtsState,
    app_handle: &tauri::AppHandle,
    decision_id: &str,
    round: &crate::db::DebateRound,
) {
    if !tts_state.enabled { return; }

    let segment_index = tts_state.segment_counter.fetch_add(1, Ordering::Relaxed);
    let ah = app_handle.clone();
    let did = decision_id.to_string();
    let round_clone = round.clone();
    let cfg = tts_state.config.clone();
    let reg = tts_state.registry.clone();
    let add = tts_state.app_data_dir.clone();
    let handles = Arc::clone(&tts_state.handles);

    let handle = tokio::spawn(async move {
        let mut spoken_round = round_clone;
        spoken_round.content = normalize_spoken_debate_output(&spoken_round.content);
        match tts::generate_segment_audio(
            &did, segment_index, &spoken_round, &cfg, &reg, &add,
        ).await {
            Ok(segment) => {
                let audio_dir = add.join("debates").join(&did);
                let _ = ah.emit("debate-segment-audio-ready", json!({
                    "decision_id": did,
                    "segment_index": segment_index,
                    "agent": segment.agent,
                    "round_number": segment.round,
                    "exchange_number": segment.exchange,
                    "audio_file": segment.audio_file,
                    "duration_ms": segment.duration_ms,
                    "audio_dir": audio_dir.to_string_lossy().to_string(),
                }));
                Some(segment)
            }
            Err(e) => {
                eprintln!("Live TTS failed for segment {}: {}", segment_index, e);
                let _ = ah.emit("debate-segment-audio-error", json!({
                    "decision_id": did,
                    "segment_index": segment_index,
                    "error": e,
                }));
                None
            }
        }
    });

    let _ = handles.lock().map(|mut h| h.push(handle));
}

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
fn format_transcript(rounds: &[crate::db::DebateRound], all_agents: &[AgentInfo]) -> String {
    let mut sections: Vec<String> = Vec::new();
    let mut current_round = -1i32;
    let mut current_exchange = -1i32;

    for r in rounds {
        if r.round_number != current_round || r.exchange_number != current_exchange {
            current_round = r.round_number;
            current_exchange = r.exchange_number;
            let header = match current_round {
                1 => "Round 1 (opening)".to_string(),
                2 => format!("Round 2 (exchange {})", current_exchange),
                3 => "Round 3 (final statements)".to_string(),
                99 => "Moderator synthesis".to_string(),
                _ => format!("Round {}", current_round),
            };
            sections.push(header);
        }

        let label = all_agents.iter()
            .find(|a| a.key == r.agent)
            .map(|a| a.label.clone())
            .unwrap_or_else(|| r.agent.clone());
        sections.push(format!("{}: {}", label, r.content));
    }

    sections.join("\n\n")
}

/// Call a single agent with retry logic, streaming tokens to frontend.
async fn call_agent_with_retry(
    api_key: &str,
    model: &str,
    agent_key: &str,
    agent_label: &str,
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
            agent_key,
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
    Err(format!("{} failed after {} retries: {}", agent_label, max_retries + 1, last_err))
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
    debaters: &[AgentInfo],
    all_agents: &[AgentInfo],
    tts_state: &LiveTtsState,
) -> Result<Vec<crate::db::DebateRound>, String> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Err("Debate cancelled".to_string());
    }

    let transcript = format_transcript(existing_rounds, all_agents);

    let user_prompt = match round_number {
        1 => agents::round1_prompt(brief),
        2 => agents::round2_prompt(brief, &transcript, exchange_number),
        3 => agents::round3_prompt(brief, &transcript),
        _ => return Err("Invalid round number".to_string()),
    };

    let mut new_rounds = Vec::new();

    for agent in debaters {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Debate cancelled".to_string());
        }

        let base_system_prompt = agents::read_agent_prompt(app_data_dir, &agent.key);
        let system_prompt = format!(
            "{}\n\n{}",
            base_system_prompt,
            agents::debate_spoken_style_overlay()
        );
        let agent_model = agent_models.get(&agent.key).filter(|m| !m.is_empty()).map(|m| m.as_str()).unwrap_or(default_model);
        let result = call_agent_with_retry(
            api_key, agent_model,
            &agent.key, &agent.label, &system_prompt, &user_prompt, 2,
            app_handle, decision_id, round_number, exchange_number,
        ).await;

        match result {
            Ok(text) => {
                let normalized_text = normalize_spoken_debate_output(&text);
                // Save to DB
                let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
                let round = {
                    let state_guard = state.lock().map_err(|e| e.to_string())?;
                    state_guard.db.save_debate_round(
                        decision_id,
                        round_number,
                        exchange_number,
                        &agent.key,
                        &normalized_text,
                    ).map_err(|e| e.to_string())?
                };

                // Emit per-agent complete event
                let _ = app_handle.emit("debate-agent-response", json!({
                    "decision_id": decision_id,
                    "round_number": round_number,
                    "exchange_number": exchange_number,
                    "agent": agent.key,
                    "content": normalized_text,
                }));

                // Spawn live TTS for this segment
                spawn_segment_tts(tts_state, app_handle, decision_id, &round);

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
    selected_agent_keys: Option<Vec<String>>,
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

    // Load agent registry and determine participants
    let registry = agents::load_registry(&app_data_dir);
    let all_debaters_in_registry: Vec<AgentInfo> = registry.iter()
        .filter(|a| a.role == "debater")
        .cloned()
        .collect();

    let debaters: Vec<AgentInfo> = match selected_agent_keys {
        Some(ref keys) if !keys.is_empty() => {
            // Use selected agents in the order they appear in the registry
            all_debaters_in_registry.iter()
                .filter(|a| keys.contains(&a.key))
                .cloned()
                .collect()
        }
        _ => all_debaters_in_registry,
    };

    if debaters.is_empty() {
        return Err("No debaters selected for the debate".to_string());
    }

    // Build participant names for moderator
    let participant_names = agents::format_participant_names(&debaters);

    // All agents for transcript formatting (debaters + moderator)
    let all_agents: Vec<AgentInfo> = registry.clone();

    // Set up live TTS state
    let tts_config = config::load_config(&app_data_dir);
    let has_tts = match tts_config.tts_provider.as_str() {
        "openai" => !tts_config.openrouter_api_key.is_empty(),
        _ => !tts_config.elevenlabs_api_key.is_empty(),
    };
    let tts_state = LiveTtsState {
        enabled: has_tts,
        config: tts_config,
        registry: registry.clone(),
        app_data_dir: app_data_dir.clone(),
        segment_counter: Arc::new(AtomicUsize::new(0)),
        handles: Arc::new(Mutex::new(Vec::new())),
    };

    let mut all_rounds: Vec<crate::db::DebateRound> = Vec::new();

    // 4. Round 1: Opening Positions
    let round1 = run_sequential_round(
        &api_key, &model, &agent_models,
        &brief, &all_rounds, 1, 1,
        &app_handle, &decision_id, &cancel_flag, &app_data_dir,
        &debaters, &all_agents, &tts_state,
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
            &debaters, &all_agents, &tts_state,
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
            &debaters, &all_agents, &tts_state,
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
            &debaters, &all_agents, &tts_state,
        ).await?;
        all_rounds.extend(round3);
    }

    // 8. Moderator Synthesis
    if cancel_flag.load(Ordering::Relaxed) {
        return handle_cancellation(&app_handle, &decision_id);
    }

    let transcript = format_transcript(&all_rounds, &all_agents);
    let moderator_user_prompt = agents::moderator_prompt(&brief, &transcript, &participant_names);
    let moderator_system_prompt = agents::read_agent_prompt(&app_data_dir, "moderator");

    let moderator_model = agent_models.get("moderator").filter(|m| !m.is_empty()).map(|m| m.as_str()).unwrap_or(&model);
    let moderator_response = call_agent_with_retry(
        &api_key, moderator_model,
        "moderator", "Moderator", &moderator_system_prompt, &moderator_user_prompt, 2,
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

    // Spawn live TTS for moderator segment
    {
        let moderator_round = crate::db::DebateRound {
            id: String::new(),
            decision_id: decision_id.clone(),
            round_number: 99,
            exchange_number: 1,
            agent: "moderator".to_string(),
            content: moderator_response.clone(),
            created_at: String::new(),
        };
        spawn_segment_tts(&tts_state, &app_handle, &decision_id, &moderator_round);
    }

    // 9. Parse moderator output and update decision summary
    update_summary_from_debate(&app_handle, &decision_id, &all_rounds, &moderator_response, &debaters)?;

    // 10. Mark debate complete
    {
        let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
        let state_guard = state.lock().map_err(|e| e.to_string())?;
        state_guard.db.update_debate_completed(&decision_id).map_err(|e| e.to_string())?;
        state_guard.db.update_decision_status(&decision_id, "recommended").map_err(|e| e.to_string())?;
    }

    let _ = app_handle.emit("debate-complete", json!({ "decision_id": decision_id }));

    // Await all live TTS tasks and build the manifest
    if has_tts {
        let handles_to_await = {
            let mut h = tts_state.handles.lock().map_err(|e| e.to_string())?;
            std::mem::take(&mut *h)
        };

        let mut completed_segments: Vec<tts::AudioSegment> = Vec::new();
        for handle in handles_to_await {
            if let Ok(Some(segment)) = handle.await {
                completed_segments.push(segment);
            }
        }

        if !completed_segments.is_empty() {
            let manifest = tts::build_manifest_from_segments(&decision_id, completed_segments);
            let manifest_json = serde_json::to_string_pretty(&manifest).unwrap_or_default();
            let audio_dir_path = app_data_dir.join("debates").join(&decision_id);
            let audio_dir_str = audio_dir_path.to_string_lossy().to_string();

            // Save manifest.json to disk
            let _ = std::fs::write(audio_dir_path.join("manifest.json"), &manifest_json);

            // Save to DB
            let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
            if let Ok(sg) = state.lock() {
                let _ = sg.db.save_debate_audio(
                    &decision_id,
                    &manifest_json,
                    manifest.total_duration_ms as i64,
                    &audio_dir_str,
                );
            }

            // Emit final manifest for AudioPlayer replay
            let _ = app_handle.emit("audio-generation-complete", json!({
                "decision_id": decision_id,
                "manifest": manifest,
            }));
        }
    }

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

/// Extract final votes from the last round and build debate_summary for the decision.
fn update_summary_from_debate(
    app_handle: &tauri::AppHandle,
    decision_id: &str,
    all_rounds: &[crate::db::DebateRound],
    moderator_response: &str,
    debaters: &[AgentInfo],
) -> Result<(), String> {
    let mut final_votes = serde_json::Map::new();

    for agent in debaters {
        let last_entry = all_rounds.iter()
            .filter(|r| r.agent == agent.key)
            .last();
        if let Some(entry) = last_entry {
            let vote = entry.content.chars().take(200).collect::<String>();
            final_votes.insert(agent.key.clone(), Value::String(vote));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_extract_section_reads_content_until_next_heading() {
        let content = r#"
## Where the Committee Agreed
- Shared value
- Shared risk

## Key Disagreements
- Cost vs growth
"#;

        let section = extract_section(content, "Where the Committee Agreed");
        assert!(section.contains("Shared value"));
        assert!(!section.contains("Key Disagreements"));
    }

    #[test]
    fn unit_split_to_points_strips_bullets_and_empty_lines() {
        let points = split_to_points(
            r#"
- First
* Second

Third
"#,
        );
        assert_eq!(points, vec!["First", "Second", "Third"]);
    }

    #[test]
    fn unit_parse_moderator_recommendation_extracts_choice_confidence_and_steps() {
        let full_text = r#"
## Recommendation
**Choice**: Option B
**Confidence**: High
**Reasoning**: Better upside with manageable risk.

## What You're Giving Up
- Predictability
- Familiar team

## Action Plan
- Call recruiter today
- Draft a 90-day transition plan
"#;

        let rec_section = extract_section(full_text, "Recommendation");
        let recommendation =
            parse_moderator_recommendation(&rec_section, full_text).expect("recommendation should parse");

        assert_eq!(recommendation["choice"], "Option B");
        assert_eq!(recommendation["confidence"], "high");
        assert_eq!(recommendation["reasoning"], "Better upside with manageable risk.");
        assert_eq!(
            recommendation["next_steps"][0],
            "Call recruiter today"
        );
        assert_eq!(
            recommendation["tradeoffs"],
            "- Predictability\n- Familiar team"
        );
    }

    #[test]
    fn unit_parse_moderator_recommendation_returns_none_without_recommendation_fields() {
        let no_recommendation = "## Where the Committee Agreed\n- Point A";
        assert!(parse_moderator_recommendation("", no_recommendation).is_none());
    }

    #[test]
    fn unit_normalize_spoken_debate_output_removes_rigid_markdown_format() {
        let raw = r#"
## Opening
- **Position**: Go with Option B.
- **Key argument**: Better upside over 5 years.
1. **Concern**: Burnout risk is still real.
"#;
        let cleaned = normalize_spoken_debate_output(raw);
        assert!(!cleaned.contains("##"));
        assert!(!cleaned.contains("**"));
        assert!(!cleaned.contains("- "));
        assert!(cleaned.contains("Go with Option B."));
        assert!(cleaned.contains("Better upside over 5 years."));
        assert!(cleaned.contains("Burnout risk is still real."));
    }
}
