use crate::commands::AppState;
use crate::decisions;
use crate::profile;
use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::{Emitter, Manager};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

const SYSTEM_PROMPT: &str = r#"You are a personal decision-making assistant. Your primary job right now is to deeply understand the user — who they are, what they value, what their life situation looks like, and what matters most to them.

You have access to a set of profile files stored as markdown on the user's machine. These files contain what you've learned about the user so far. Before every response, you should read the relevant profile files to remind yourself what you know.

As you learn new things about the user through conversation, you should update or create profile files to remember this information. Be organized — create separate files for different aspects of the user's life (career, finances, family, values, goals, health, etc.). Don't ask permission to save — just save what you learn naturally.

When saving profile information:
- Write in a clear, structured markdown format
- Use headers and bullet points for organization
- Include context and nuance, not just bare facts
- Update existing files rather than duplicating information
- Create new files when you discover a new significant aspect of the user's life

Be conversational and warm. Ask thoughtful follow-up questions. Don't interrogate — let understanding develop naturally through genuine conversation. You're building a relationship, not filling out a form.

When you have enough context about the user and they bring you a decision to make, you should:
1. Read all relevant profile files
2. Consider all variables and how they interact
3. Weigh tradeoffs against the user's stated values and priorities
4. Give a clear, committed recommendation with transparent reasoning
5. Explain what they'd be giving up with your recommended choice

But for now, focus on learning about the user. The better you understand them, the better your future recommendations will be."#;

const DECISION_SYSTEM_PROMPT: &str = r#"You are a personal decision-making assistant. The user is working through a specific decision and needs your help analyzing it thoroughly.

You have access to the user's profile files — markdown files that contain everything you've learned about them: their values, priorities, life situation, constraints, finances, career, family, and goals. READ THESE FIRST before engaging with the decision.

Your job is to:

1. UNDERSTAND THE DECISION
   - What are they deciding between? (Surface all options, including ones they haven't considered)
   - What's the timeline? Is this reversible?
   - What triggered this decision now?

2. MAP ALL VARIABLES
   - What factors are at play? (financial, career, emotional, relational, health, etc.)
   - What are the second and third-order effects of each option?
   - What are they not seeing? What blind spots might they have?
   - What assumptions are they making?

3. ANALYZE AGAINST THEIR PROFILE
   - How does each option align with their stated values and priorities?
   - How does each option interact with their current constraints (financial, family, etc.)?
   - What does their risk tolerance suggest?
   - What would matter most to them based on what you know?

4. RECOMMEND
   - Give a CLEAR, COMMITTED recommendation. Do not hedge with "it depends" or "only you can decide."
   - Explain your reasoning transparently — which values and factors drove the recommendation
   - Explicitly state what they'd be giving up with your recommended choice
   - Rate your confidence (high/medium/low) and explain why

5. UPDATE THE DECISION SUMMARY
   After each significant exchange, update the decision summary by calling the `update_decision_summary` tool. This populates the structured panel the user sees alongside the chat. Update it progressively — don't wait until the end.

Guidelines:
- Ask focused questions, one or two at a time. Don't overwhelm.
- Push back if the user is framing the decision too narrowly ("should I quit?" is rarely binary)
- Name cognitive biases if you spot them (sunk cost, anchoring, status quo bias, etc.)
- Be honest even if it's not what they want to hear
- If you don't have enough information from the profile files, ask for it
- If new information emerges that should be saved to the profile, update the profile files too

6. REFLECT ON OUTCOMES
   When you see a message starting with "[DECISION OUTCOME LOGGED]", the user has reported how their decision turned out. This is a critical learning moment:

   a) READ PROFILE FILES first to understand the full context of who this person is
   b) COMPARE: your recommendation vs. what the user chose vs. what actually happened
   c) ANALYZE: factors you over/underweighted, biases at play, what the user's intuition captured that your analysis missed (or vice versa), unpredictable external factors vs foreseeable outcomes
   d) UPDATE PROFILE FILES with lessons learned — create or update a "decision-patterns.md" file tracking what works for this user and what doesn't, and update other relevant profiles if the outcome reveals new info about their values, risk tolerance, or priorities. Be specific — e.g. "user's read on organizational culture tends to be more reliable than quantitative analysis" rather than "user trusts gut feelings"
   e) SHARE your reflection transparently in the chat. Be honest about what you got right, what you got wrong, and how this will change your future recommendations for this user"#;

// ── Stream event sent to frontend via Channel ──

#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "token")]
    Token { token: String },
    #[serde(rename = "tool_use")]
    ToolUse { tool: String },
}

// ── OpenAI-compatible tool format (used by OpenRouter) ──

fn get_tools(is_decision: bool) -> Value {
    let mut tools = json!([
        {
            "type": "function",
            "function": {
                "name": "read_profile_files",
                "description": "Read the list of all profile files and their contents. Call this at the start of conversations to refresh your memory about the user.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_profile_file",
                "description": "Create or update a profile file with information learned about the user. Use descriptive filenames like 'career.md', 'values.md', 'family.md', etc.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "filename": {
                            "type": "string",
                            "description": "The filename (e.g., 'career.md')"
                        },
                        "content": {
                            "type": "string",
                            "description": "The full markdown content of the file"
                        }
                    },
                    "required": ["filename", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "delete_profile_file",
                "description": "Delete a profile file that is no longer relevant or has been consolidated into another file.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "filename": {
                            "type": "string",
                            "description": "The filename to delete"
                        }
                    },
                    "required": ["filename"]
                }
            }
        }
    ]);

    if is_decision {
        if let Some(arr) = tools.as_array_mut() {
            arr.push(json!({
                "type": "function",
                "function": {
                    "name": "update_decision_summary",
                    "description": "Update the structured decision summary panel. Call this after each significant exchange to keep the summary current.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "options": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": { "type": "string" },
                                        "description": { "type": "string" }
                                    },
                                    "required": ["label"]
                                }
                            },
                            "variables": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": { "type": "string" },
                                        "value": { "type": "string" },
                                        "impact": { "type": "string" }
                                    },
                                    "required": ["label", "value"]
                                }
                            },
                            "pros_cons": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "option": { "type": "string" },
                                        "pros": { "type": "array", "items": { "type": "string" } },
                                        "cons": { "type": "array", "items": { "type": "string" } },
                                        "alignment_score": { "type": "integer" },
                                        "alignment_reasoning": { "type": "string" }
                                    },
                                    "required": ["option"]
                                }
                            },
                            "recommendation": {
                                "type": "object",
                                "properties": {
                                    "choice": { "type": "string" },
                                    "confidence": { "type": "string" },
                                    "reasoning": { "type": "string" },
                                    "tradeoffs": { "type": "string" },
                                    "next_steps": { "type": "array", "items": { "type": "string" } }
                                },
                                "required": ["choice", "confidence", "reasoning"]
                            },
                            "status": {
                                "type": "string",
                                "enum": ["exploring", "analyzing", "recommended"]
                            }
                        }
                    }
                }
            }));
        }
    }

    tools
}

// ── Shared tool execution ──

fn execute_tool(
    name: &str,
    input: &Value,
    app_data_dir: &PathBuf,
    decision_id: Option<&str>,
    app_handle: &tauri::AppHandle,
) -> String {
    match name {
        "read_profile_files" => {
            match profile::read_all_profiles(app_data_dir) {
                Ok(files) => serde_json::to_string(&files).unwrap_or_else(|_| "{}".to_string()),
                Err(e) => format!("Error reading profiles: {}", e),
            }
        }
        "write_profile_file" => {
            let filename = input["filename"].as_str().unwrap_or("unknown.md");
            let content = input["content"].as_str().unwrap_or("");
            match profile::write_profile_file(app_data_dir, filename, content) {
                Ok(msg) => msg,
                Err(e) => format!("Error writing profile: {}", e),
            }
        }
        "delete_profile_file" => {
            let filename = input["filename"].as_str().unwrap_or("");
            match profile::delete_profile_file(app_data_dir, filename) {
                Ok(msg) => msg,
                Err(e) => format!("Error deleting profile: {}", e),
            }
        }
        "update_decision_summary" => {
            let Some(dec_id) = decision_id else {
                return "Error: no decision context for update_decision_summary".to_string();
            };
            let state: tauri::State<'_, Mutex<AppState>> = app_handle.state();
            let state_guard = match state.lock() {
                Ok(s) => s,
                Err(e) => return format!("Error locking state: {}", e),
            };

            let existing_summary = state_guard.db
                .get_decision(dec_id)
                .ok()
                .flatten()
                .and_then(|d| d.summary_json);

            let merged = decisions::merge_summary(existing_summary.as_deref(), input);

            if let Err(e) = state_guard.db.update_decision_summary(dec_id, &merged) {
                return format!("Error saving summary: {}", e);
            }

            if let Some(status) = input.get("status").and_then(|v| v.as_str()) {
                if let Err(e) = state_guard.db.update_decision_status(dec_id, status) {
                    return format!("Error updating status: {}", e);
                }
            }

            let _ = app_handle.emit("decision-summary-updated", json!({
                "decision_id": dec_id,
                "summary": merged,
                "status": input.get("status").and_then(|v| v.as_str()),
            }));

            "Decision summary updated successfully.".to_string()
        }
        _ => format!("Unknown tool: {}", name),
    }
}

// ── Helpers ──

fn openrouter_headers(api_key: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Authorization", format!("Bearer {}", api_key).parse().unwrap());
    headers.insert("HTTP-Referer", "https://decisioncopilot.app".parse().unwrap());
    headers.insert("X-Title", "Decision Copilot".parse().unwrap());
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers
}

fn map_api_error(status: reqwest::StatusCode, body: &str) -> String {
    match status.as_u16() {
        401 => "Invalid API key. Check your key at openrouter.ai/keys".to_string(),
        402 => "Insufficient credits. Visit openrouter.ai to add funds.".to_string(),
        429 => "Rate limited. Please wait a moment and try again.".to_string(),
        400 if body.contains("model_not_found") || body.contains("not found") => {
            "Model not found. Check the model ID at openrouter.ai/models".to_string()
        }
        500 | 502 | 503 => "OpenRouter is temporarily unavailable. Try again in a moment.".to_string(),
        _ => format!("API error ({}): {}", status, body),
    }
}

// ── Streaming tool call accumulator ──
// OpenAI streaming sends tool_calls incrementally: first chunk has id+name,
// subsequent chunks append to arguments string.

#[derive(Debug, Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

// ── Public entry point: send_message ──

pub async fn send_message(
    api_key: &str,
    model: &str,
    messages: Vec<Value>,
    app_data_dir: &PathBuf,
    on_event: &Channel<StreamEvent>,
    conv_type: &str,
    decision_id: Option<&str>,
    app_handle: &tauri::AppHandle,
) -> Result<String, String> {
    let client = Client::new();
    let is_decision = conv_type == "decision";
    let system_prompt = if is_decision { DECISION_SYSTEM_PROMPT } else { SYSTEM_PROMPT };

    // Build message list with system prompt as first message
    let mut openrouter_messages: Vec<Value> = vec![
        json!({"role": "system", "content": system_prompt}),
    ];
    for msg in &messages {
        openrouter_messages.push(msg.clone());
    }

    let mut all_text = String::new();

    loop {
        let request_body = json!({
            "model": model,
            "messages": openrouter_messages,
            "tools": get_tools(is_decision),
            "temperature": 0.7,
            "max_tokens": 4096,
            "stream": true,
        });

        let mut response = client
            .post(OPENROUTER_URL)
            .headers(openrouter_headers(api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.map_err(|e| format!("Read error: {}", e))?;
            return Err(map_api_error(status, &error_text));
        }

        let mut iteration_text = String::new();
        let mut pending_tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut buffer = String::new();

        while let Some(chunk) = response.chunk().await.map_err(|e| format!("Stream error: {}", e))? {
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE lines (data: {...}\n\n)
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                let data_str = match line.strip_prefix("data: ") {
                    Some(d) => d,
                    None => continue,
                };

                if data_str == "[DONE]" {
                    continue;
                }

                let data: Value = match serde_json::from_str(data_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let choice = &data["choices"][0];
                let delta = &choice["delta"];

                // Text content
                if let Some(content) = delta["content"].as_str() {
                    if !content.is_empty() {
                        iteration_text.push_str(content);
                        let _ = on_event.send(StreamEvent::Token { token: content.to_string() });
                    }
                }

                // Tool calls (streamed incrementally)
                if let Some(tool_calls) = delta["tool_calls"].as_array() {
                    for tc in tool_calls {
                        let index = tc["index"].as_u64().unwrap_or(0) as usize;

                        // Ensure we have enough slots
                        while pending_tool_calls.len() <= index {
                            pending_tool_calls.push(PendingToolCall {
                                id: String::new(),
                                name: String::new(),
                                arguments: String::new(),
                            });
                        }

                        // First chunk for this tool call has id and function name
                        if let Some(id) = tc["id"].as_str() {
                            pending_tool_calls[index].id = id.to_string();
                        }
                        if let Some(name) = tc["function"]["name"].as_str() {
                            pending_tool_calls[index].name = name.to_string();
                            let _ = on_event.send(StreamEvent::ToolUse { tool: name.to_string() });
                        }
                        // Subsequent chunks append to arguments
                        if let Some(args) = tc["function"]["arguments"].as_str() {
                            pending_tool_calls[index].arguments.push_str(args);
                        }
                    }
                }
            }
        }

        // Filter out empty tool calls (shouldn't happen, but defensive)
        let tool_calls: Vec<PendingToolCall> = pending_tool_calls
            .into_iter()
            .filter(|tc| !tc.name.is_empty())
            .collect();

        if tool_calls.is_empty() {
            all_text.push_str(&iteration_text);
            return Ok(all_text);
        }

        // Handle tool calls — build assistant message and tool results
        all_text.push_str(&iteration_text);

        // Build the assistant message with tool_calls
        let assistant_tool_calls: Vec<Value> = tool_calls.iter().map(|tc| {
            json!({
                "id": tc.id,
                "type": "function",
                "function": {
                    "name": tc.name,
                    "arguments": tc.arguments,
                }
            })
        }).collect();

        let mut assistant_msg = json!({"role": "assistant"});
        if !iteration_text.is_empty() {
            assistant_msg["content"] = json!(iteration_text);
        } else {
            assistant_msg["content"] = Value::Null;
        }
        assistant_msg["tool_calls"] = json!(assistant_tool_calls);
        openrouter_messages.push(assistant_msg);

        // Execute each tool and append results
        for tc in &tool_calls {
            let input: Value = serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
            let result = execute_tool(&tc.name, &input, app_data_dir, decision_id, app_handle);
            openrouter_messages.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": result,
            }));
        }
    }
}

// ── Streaming LLM call for debate (no tools, emits per-token events) ──

pub async fn call_llm_streaming_debate(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    app_handle: &tauri::AppHandle,
    decision_id: &str,
    round_number: i32,
    exchange_number: i32,
    agent_key: &str,
) -> Result<String, String> {
    let client = Client::new();
    let request_body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt},
        ],
        "temperature": 0.7,
        "max_tokens": 2048,
        "stream": true,
    });

    let mut response = client
        .post(OPENROUTER_URL)
        .headers(openrouter_headers(api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.map_err(|e| format!("Read error: {}", e))?;
        return Err(map_api_error(status, &error_text));
    }

    let mut all_text = String::new();
    let mut buffer = String::new();

    while let Some(chunk) = response.chunk().await.map_err(|e| format!("Stream error: {}", e))? {
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end().to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let data_str = match line.strip_prefix("data: ") {
                Some(d) => d,
                None => continue,
            };

            if data_str == "[DONE]" {
                continue;
            }

            let data: Value = match serde_json::from_str(data_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if let Some(content) = data["choices"][0]["delta"]["content"].as_str() {
                if !content.is_empty() {
                    all_text.push_str(content);
                    let _ = app_handle.emit("debate-agent-token", json!({
                        "decision_id": decision_id,
                        "round_number": round_number,
                        "exchange_number": exchange_number,
                        "agent": agent_key,
                        "token": content,
                    }));
                }
            }
        }
    }

    Ok(all_text)
}
