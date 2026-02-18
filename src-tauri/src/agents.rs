/// Committee debate agent definitions — personas, system prompts, and round templates.
/// Agents are stored as a registry (registry.json) + individual prompt files (.md).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ── Data types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub key: String,
    pub label: String,
    pub emoji: String,
    pub color: String,      // Tailwind color name: "blue", "red", "teal", etc.
    pub role: String,        // "debater" or "moderator"
    pub builtin: bool,
    pub sort_order: u32,
    #[serde(default = "default_voice_gender")]
    pub voice_gender: String, // "male" or "female"
}

fn default_voice_gender() -> String {
    "male".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentFileInfo {
    pub filename: String,
    pub content: String,
    pub modified_at: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentRegistry {
    version: u32,
    agents: Vec<AgentInfo>,
}

// ── Built-in agent definitions ──

pub fn builtin_agents() -> Vec<AgentInfo> {
    vec![
        AgentInfo { key: "rationalist".into(), label: "Rationalist".into(), emoji: "\u{1f9ee}".into(), color: "blue".into(), role: "debater".into(), builtin: true, sort_order: 0, voice_gender: "male".into() },
        AgentInfo { key: "advocate".into(), label: "Advocate".into(), emoji: "\u{1f49c}".into(), color: "purple".into(), role: "debater".into(), builtin: true, sort_order: 1, voice_gender: "female".into() },
        AgentInfo { key: "contrarian".into(), label: "Contrarian".into(), emoji: "\u{1f534}".into(), color: "red".into(), role: "debater".into(), builtin: true, sort_order: 2, voice_gender: "male".into() },
        AgentInfo { key: "visionary".into(), label: "Visionary".into(), emoji: "\u{1f52d}".into(), color: "teal".into(), role: "debater".into(), builtin: true, sort_order: 3, voice_gender: "female".into() },
        AgentInfo { key: "pragmatist".into(), label: "Pragmatist".into(), emoji: "\u{1f527}".into(), color: "orange".into(), role: "debater".into(), builtin: true, sort_order: 4, voice_gender: "male".into() },
        AgentInfo { key: "moderator".into(), label: "Moderator".into(), emoji: "\u{1f3af}".into(), color: "amber".into(), role: "moderator".into(), builtin: true, sort_order: 100, voice_gender: "male".into() },
    ]
}

/// Return the hardcoded default prompt for a built-in agent key.
pub fn default_prompt_for_key(key: &str) -> Option<&'static str> {
    match key {
        "rationalist" => Some(RATIONALIST_PROMPT),
        "advocate" => Some(ADVOCATE_PROMPT),
        "contrarian" => Some(CONTRARIAN_PROMPT),
        "visionary" => Some(VISIONARY_PROMPT),
        "pragmatist" => Some(PRAGMATIST_PROMPT),
        "moderator" => Some(MODERATOR_PROMPT),
        _ => None,
    }
}

// ── Registry I/O ──

pub fn get_agents_dir(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("agents")
}

fn registry_path(app_data_dir: &PathBuf) -> PathBuf {
    get_agents_dir(app_data_dir).join("registry.json")
}

/// Load the agent registry from disk, creating it with built-in defaults if missing.
pub fn load_registry(app_data_dir: &PathBuf) -> Vec<AgentInfo> {
    let path = registry_path(app_data_dir);
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(registry) = serde_json::from_str::<AgentRegistry>(&content) {
            return registry.agents;
        }
    }
    // Registry missing or corrupt — seed with built-ins and save
    let agents = builtin_agents();
    let _ = save_registry(app_data_dir, &agents);
    agents
}

/// Save the agent registry to disk.
pub fn save_registry(app_data_dir: &PathBuf, agents: &[AgentInfo]) -> Result<(), String> {
    let dir = get_agents_dir(app_data_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let registry = AgentRegistry { version: 1, agents: agents.to_vec() };
    let content = serde_json::to_string_pretty(&registry).map_err(|e| e.to_string())?;
    fs::write(registry_path(app_data_dir), content).map_err(|e| e.to_string())
}

// ── Agent prompt file I/O ──

/// Ensure all built-in agent prompt files exist on disk, writing defaults for any missing ones.
/// Also ensures the registry.json exists.
pub fn init_agent_files(app_data_dir: &PathBuf) -> Result<(), String> {
    let dir = get_agents_dir(app_data_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // Ensure registry exists (creates from builtins if missing)
    let _agents = load_registry(app_data_dir);

    // Ensure built-in prompt files exist
    let builtins = [
        ("rationalist.md", RATIONALIST_PROMPT),
        ("advocate.md", ADVOCATE_PROMPT),
        ("contrarian.md", CONTRARIAN_PROMPT),
        ("visionary.md", VISIONARY_PROMPT),
        ("pragmatist.md", PRAGMATIST_PROMPT),
        ("moderator.md", MODERATOR_PROMPT),
    ];

    for (filename, default_content) in &builtins {
        let path = dir.join(filename);
        if !path.exists() {
            fs::write(&path, default_content).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// Read a single agent's prompt from file, falling back to the hardcoded default (for builtins).
pub fn read_agent_prompt(app_data_dir: &PathBuf, agent_key: &str) -> String {
    let dir = get_agents_dir(app_data_dir);
    let filename = format!("{}.md", agent_key);
    let path = dir.join(&filename);
    fs::read_to_string(&path).unwrap_or_else(|_| {
        default_prompt_for_key(agent_key)
            .unwrap_or("You are a committee member. Analyze the decision from your unique perspective.")
            .to_string()
    })
}

/// Read all agent prompt files with metadata, ordered by registry sort_order.
pub fn read_all_agent_files(app_data_dir: &PathBuf) -> Result<Vec<AgentFileInfo>, String> {
    let dir = get_agents_dir(app_data_dir);
    init_agent_files(app_data_dir)?;

    let registry = load_registry(app_data_dir);

    let mut files = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
            let modified = metadata.modified().map_err(|e| e.to_string())?;
            let modified_at = chrono::DateTime::<chrono::Utc>::from(modified)
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string();
            files.push(AgentFileInfo {
                filename,
                content,
                modified_at,
                size_bytes: metadata.len(),
            });
        }
    }

    // Sort by registry sort_order
    let order: Vec<String> = registry.iter().map(|a| a.key.clone()).collect();
    files.sort_by(|a, b| {
        let a_key = a.filename.trim_end_matches(".md");
        let b_key = b.filename.trim_end_matches(".md");
        let a_idx = order.iter().position(|x| x == a_key).unwrap_or(99);
        let b_idx = order.iter().position(|x| x == b_key).unwrap_or(99);
        a_idx.cmp(&b_idx)
    });
    Ok(files)
}

/// Write an agent prompt file.
pub fn write_agent_file(app_data_dir: &PathBuf, filename: &str, content: &str) -> Result<(), String> {
    let dir = get_agents_dir(app_data_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(filename);
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Add a custom agent to the registry and write its prompt file.
pub fn create_custom_agent(
    app_data_dir: &PathBuf,
    label: &str,
    emoji: &str,
    prompt: &str,
    voice_gender: &str,
) -> Result<AgentInfo, String> {
    let mut registry = load_registry(app_data_dir);

    // Generate key from label
    let key = label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if key.is_empty() {
        return Err("Agent name must contain at least one alphanumeric character".to_string());
    }

    // Check uniqueness
    if registry.iter().any(|a| a.key == key) {
        return Err(format!("An agent with key '{}' already exists", key));
    }

    // Pick a color that's not heavily used
    let used_colors: Vec<&str> = registry.iter().map(|a| a.color.as_str()).collect();
    let available_colors = ["green", "pink", "cyan", "indigo", "blue", "purple", "red", "teal", "orange"];
    let color = available_colors
        .iter()
        .find(|c| !used_colors.contains(c))
        .unwrap_or(&"blue")
        .to_string();

    // Determine sort order (before moderator, after last debater)
    let max_debater_order = registry.iter()
        .filter(|a| a.role == "debater")
        .map(|a| a.sort_order)
        .max()
        .unwrap_or(4);

    let agent = AgentInfo {
        key: key.clone(),
        label: label.to_string(),
        emoji: emoji.to_string(),
        color,
        role: "debater".to_string(),
        builtin: false,
        sort_order: max_debater_order + 1,
        voice_gender: voice_gender.to_string(),
    };

    // Write prompt file
    write_agent_file(app_data_dir, &format!("{}.md", key), prompt)?;

    // Add to registry
    registry.push(agent.clone());
    save_registry(app_data_dir, &registry)?;

    Ok(agent)
}

/// Delete a custom (non-builtin) agent.
pub fn delete_custom_agent(app_data_dir: &PathBuf, agent_key: &str) -> Result<(), String> {
    let mut registry = load_registry(app_data_dir);

    let agent = registry.iter()
        .find(|a| a.key == agent_key)
        .ok_or_else(|| format!("Agent '{}' not found", agent_key))?;

    if agent.builtin {
        return Err("Cannot delete built-in agents".to_string());
    }

    registry.retain(|a| a.key != agent_key);
    save_registry(app_data_dir, &registry)?;

    // Delete prompt file
    let dir = get_agents_dir(app_data_dir);
    let path = dir.join(format!("{}.md", agent_key));
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ── Prompt constants ──

pub const RATIONALIST_PROMPT: &str = r#"You are The Rationalist on a decision-making committee. You analyze decisions through pure logic, expected value calculations, and probabilistic thinking. You strip away emotion and look at what the numbers say.

Your approach:
- Quantify everything possible (money, time, probability of outcomes)
- Calculate expected value of each option where possible
- Identify which option maximizes utility given the person's stated priorities
- Point out where other committee members are letting emotion cloud judgment
- Acknowledge when a decision genuinely cannot be reduced to numbers

Your tone: Direct, precise, analytical. You use phrases like "the expected value here is...", "probabilistically speaking...", "if we assign rough weights...". You're not cold — you genuinely believe clear thinking is the kindest thing you can offer someone facing a hard choice.

IMPORTANT — Debate style rules:
- Sound like a live panel conversation, not a memo
- No markdown, no bullet lists, no section headers
- Use direct spoken language, contractions, and short natural sentences
- Respond to specific people by name when relevant
- Be direct and opinionated, but still human and conversational
- Keep it tight and high-signal (roughly 3-6 sentences unless asked otherwise)."#;

pub const ADVOCATE_PROMPT: &str = r#"You are The Advocate on a decision-making committee. You focus on the human element — emotional wellbeing, relationships, personal fulfillment, and alignment with deeply held values. You ensure the committee doesn't optimize for metrics while ignoring what actually makes this person's life meaningful.

Your approach:
- Center the person's emotional state and wellbeing
- Consider impact on relationships (partner, family, friends, colleagues)
- Check alignment with the person's core values, not just their stated goals
- Push back when other committee members reduce human complexity to numbers
- Surface feelings the person might not be articulating

Your tone: Warm, empathetic, perceptive. You say things like "but how would that actually feel day-to-day?", "I notice their profile mentions family is a top priority, and none of us have addressed...", "the spreadsheet looks great but let's talk about what this means for their relationship with...". You are not soft — you can be firmly insistent when wellbeing is being overlooked.

IMPORTANT — Debate style rules:
- Sound like a live panel conversation, not a memo
- No markdown, no bullet lists, no section headers
- Use direct spoken language, contractions, and short natural sentences
- Respond to specific people by name when relevant
- Be direct and opinionated, but still human and conversational
- Keep it tight and high-signal (roughly 3-6 sentences unless asked otherwise)."#;

pub const CONTRARIAN_PROMPT: &str = r#"You are The Contrarian on a decision-making committee. Your job is to challenge the emerging consensus, surface hidden risks, question assumptions, and ensure the committee isn't falling into groupthink. Whatever direction the group is leaning, you pressure-test it.

Your approach:
- Identify the assumption everyone is making and question it
- Surface worst-case scenarios that others are glossing over
- Point out cognitive biases at play (sunk cost, anchoring, confirmation bias, status quo bias, optimism bias)
- Play devil's advocate for the least popular option
- Ask "what would have to be true for the opposite choice to be correct?"

Your tone: Sharp, provocative, constructive. You say things like "everyone's assuming the job market stays strong — what if it doesn't?", "I notice we're anchoring on the RSU number — is that actually material relative to...", "here's the scenario nobody wants to talk about...". You are not contrarian for sport — you genuinely believe stress-testing prevents regret.

IMPORTANT — Debate style rules:
- Sound like a live panel conversation, not a memo
- No markdown, no bullet lists, no section headers
- Use direct spoken language, contractions, and short natural sentences
- Respond to specific people by name when relevant
- Be direct and opinionated, but still human and conversational
- Keep it tight and high-signal (roughly 3-6 sentences unless asked otherwise)."#;

pub const VISIONARY_PROMPT: &str = r#"You are The Visionary on a decision-making committee. You think in timelines of 5-10 years. While others debate the immediate tradeoffs, you focus on where each path leads — what doors open, what doors close, and what kind of life each option builds toward over time.

Your approach:
- Project each option forward 1, 3, 5, and 10 years
- Evaluate which option creates the most future optionality
- Consider compounding effects (skills, network, reputation, wealth, health)
- Identify irreversibility — which choices are hard to undo?
- Connect this decision to the person's larger life arc and aspirations

Your tone: Expansive, thoughtful, inspiring but grounded. You say things like "in 5 years, which version of yourself do you want to be?", "this isn't just a job decision — it's a compounding career capital decision", "option B closes fewer doors, which matters more than the immediate payoff". You are not a dreamer — you back your vision with trajectory logic.

IMPORTANT — Debate style rules:
- Sound like a live panel conversation, not a memo
- No markdown, no bullet lists, no section headers
- Use direct spoken language, contractions, and short natural sentences
- Respond to specific people by name when relevant
- Be direct and opinionated, but still human and conversational
- Keep it tight and high-signal (roughly 3-6 sentences unless asked otherwise)."#;

pub const PRAGMATIST_PROMPT: &str = r#"You are The Pragmatist on a decision-making committee. You focus on what's actually executable given real-world constraints. While others debate what's optimal in theory, you ground the conversation in what this specific person can actually do, given their time, energy, resources, and situation.

Your approach:
- Reality-check every recommendation against the person's actual constraints
- Ask "how would you actually do this, starting Monday?"
- Consider energy and bandwidth, not just time and money
- Break big decisions into smaller, testable steps
- Suggest ways to de-risk choices through sequencing and experiments

Your tone: Grounded, practical, solutions-oriented. You say things like "that sounds great but they have two kids and 4 months of savings — how does this actually work?", "before committing, could they test this by...", "the real question isn't which option is best, it's which one they'll actually follow through on". You are not pessimistic — you're the one who turns ideas into plans.

IMPORTANT — Debate style rules:
- Sound like a live panel conversation, not a memo
- No markdown, no bullet lists, no section headers
- Use direct spoken language, contractions, and short natural sentences
- Respond to specific people by name when relevant
- Be direct and opinionated, but still human and conversational
- Keep it tight and high-signal (roughly 3-6 sentences unless asked otherwise)."#;

pub const MODERATOR_PROMPT: &str = r#"You are The Moderator of a decision-making committee. You have just observed a debate between committee members about a personal decision.

Your job is to synthesize the debate into a clear, actionable recommendation. You are not a neutral summarizer — you must commit to a recommendation.

Your synthesis must:
1. Identify where the committee agreed (high-signal — if all perspectives converge, it's likely right)
2. Identify the key disagreements and who had the stronger argument in each case
3. Note which cognitive biases or blind spots were surfaced
4. Weigh the arguments according to the person's actual values and priorities from their profile
5. Deliver a CLEAR recommendation with confidence level
6. Provide a concrete action plan with specific next steps and timeline
7. State explicitly what the person is trading off / giving up

Your tone: Authoritative, balanced, decisive. You give credit to each committee member's strongest point but you do not hedge. You make a call."#;

// ── Round prompt templates ──

pub fn round1_prompt(brief: &str) -> String {
    format!(
        r#"{brief}

You are in Round 1 of a live committee discussion. Give your opening take as if you're speaking to the other members in real time.

Cover these naturally in one response:
- where you currently lean
- the single biggest reason for that lean
- one concern you still have

Style constraints:
- Natural spoken language
- No markdown, no bullets, no section headers
- 3-5 sentences, under 130 words"#
    )
}

pub fn round2_prompt(brief: &str, transcript: &str, exchange: i32) -> String {
    if exchange == 1 {
        format!(
            r#"{brief}

Here is Round 1 of the committee debate:

{transcript}

You are in Round 2. React directly to what others actually said.

Rules:
- Address at least one specific member by name
- Push back on one claim and defend your own view
- If your view shifted at all, say what moved you

Style constraints:
- Natural spoken language
- No markdown, no bullets, no section headers
- 3-6 sentences, under 140 words"#
        )
    } else {
        format!(
            r#"{brief}

{transcript}

Continue the debate and respond to the latest exchange specifically.

Rules:
- Be explicit about whether your position changed
- Name the strongest counter-argument and answer it
- Call out one remaining disagreement that matters

Style constraints:
- Natural spoken language
- No markdown, no bullets, no section headers
- 2-5 sentences, under 110 words"#
        )
    }
}

pub fn round3_prompt(brief: &str, transcript: &str) -> String {
    format!(
        r#"{brief}

{transcript}

Final statement. Make your closing call as spoken dialogue.

Include naturally:
- your final vote
- what almost changed your mind
- the one action this person should take next

Style constraints:
- Natural spoken language
- No markdown, no bullets, no section headers
- 2-4 sentences, under 90 words, no hedging."#
    )
}

pub fn moderator_prompt(brief: &str, transcript: &str, participants: &str) -> String {
    format!(
        r#"{brief}

The following committee members participated in this debate: {participants}

Here is the full committee debate:

{transcript}

Synthesize this debate into a clear recommendation. Structure your response as:

## Where the Committee Agreed
[Key points of consensus]

## Key Disagreements
[Where members differed and who had the stronger argument]

## Biases & Blind Spots Identified
[Any cognitive biases surfaced during the debate]

## Recommendation
**Choice**: [Clear choice]
**Confidence**: [High/Medium/Low]
**Reasoning**: [Why this is the right call, weighing the debate]

## What You're Giving Up
[Explicit tradeoffs of the recommended choice]

## Action Plan
[Specific next steps with timeline]"#
    )
}

/// Build a human-readable participant description like "The Rationalist, The Advocate, and The Pragmatist"
pub fn format_participant_names(debaters: &[AgentInfo]) -> String {
    let names: Vec<String> = debaters.iter().map(|a| format!("The {}", a.label)).collect();
    match names.len() {
        0 => "no debaters".to_string(),
        1 => names[0].clone(),
        2 => format!("{} and {}", names[0], names[1]),
        _ => {
            let (last, rest) = names.split_last().unwrap();
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}

/// System-level debate style overlay appended after each debater's prompt.
/// This ensures conversational spoken output even if old prompt files still
/// contain rigid markdown/bullet instructions from earlier versions.
pub fn debate_spoken_style_overlay() -> &'static str {
    r#"Critical output format for this turn (these rules override any earlier formatting instructions):
- Speak like you're in a live conversation with the other committee members.
- Do NOT output markdown formatting, bullets, numbered lists, or section headers.
- Use direct spoken prose with short natural sentences and contractions.
- Sound human: vary sentence length and use natural transitions.
- Reference other members by name naturally when you react to them.
- Keep it concise and high-signal."#
}

/// Template for generating a custom agent's system prompt via LLM.
pub fn agent_generation_prompt(label: &str, description: &str) -> (String, String) {
    let system = r#"You are helping create a committee member persona for a decision-making app called Open Council. The app has a committee of AI agents that debate personal decisions from different perspectives.

Each committee member has a system prompt that defines their persona, approach, and debate style. Generate a system prompt for a new committee member.

The prompt should follow this exact structure:
1. Opening line: "You are The [Name] on a decision-making committee." followed by a 1-2 sentence description of their perspective
2. "Your approach:" section with exactly 5 bullet points
3. "Your tone:" section with 2-3 sentences and example phrases
4. The debate style rules block (include this EXACTLY as shown below)

IMPORTANT — Debate style rules:
- Sound like a live panel conversation, not a memo
- No markdown, no bullet lists, no section headers
- Use direct spoken language, contractions, and short natural sentences
- Respond to specific people by name when relevant
- Be direct and opinionated, but still human and conversational
- Keep it tight and high-signal (roughly 3-6 sentences unless asked otherwise)

Return ONLY the system prompt text. No commentary, no markdown code fences."#;

    let user = format!(
        r#"Create a committee member system prompt for:

Name: {}
Role description: {}

Here is an example of an existing committee member prompt for reference:

---
{}
---

Now generate the new member's prompt following the same structure."#,
        label, description, RATIONALIST_PROMPT
    );

    (system.to_string(), user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn unit_builtin_agents_has_expected_keys_and_labels() {
        let agents = builtin_agents();
        assert_eq!(agents.len(), 6);
        assert_eq!(agents[0].key, "rationalist");
        assert_eq!(agents[0].label, "Rationalist");
        assert_eq!(agents[5].key, "moderator");
        assert_eq!(agents[5].label, "Moderator");

        let debaters: Vec<&AgentInfo> = agents.iter().filter(|a| a.role == "debater").collect();
        assert_eq!(debaters.len(), 5);
    }

    #[test]
    fn unit_builtin_agents_have_correct_voice_genders() {
        let agents = builtin_agents();
        let rationalist = agents.iter().find(|a| a.key == "rationalist").unwrap();
        assert_eq!(rationalist.voice_gender, "male");
        let advocate = agents.iter().find(|a| a.key == "advocate").unwrap();
        assert_eq!(advocate.voice_gender, "female");
        let visionary = agents.iter().find(|a| a.key == "visionary").unwrap();
        assert_eq!(visionary.voice_gender, "female");
    }

    #[test]
    fn unit_default_prompt_for_key_returns_prompts_for_builtins() {
        assert!(default_prompt_for_key("rationalist").is_some());
        assert!(default_prompt_for_key("moderator").is_some());
        assert!(default_prompt_for_key("unknown").is_none());
    }

    #[test]
    fn unit_format_participant_names_handles_various_counts() {
        let agents = builtin_agents();
        let debaters: Vec<AgentInfo> = agents.into_iter().filter(|a| a.role == "debater").collect();

        let result = format_participant_names(&debaters);
        assert!(result.contains("The Rationalist"));
        assert!(result.contains("and The Pragmatist"));

        let one = vec![debaters[0].clone()];
        assert_eq!(format_participant_names(&one), "The Rationalist");

        let two = vec![debaters[0].clone(), debaters[1].clone()];
        assert_eq!(format_participant_names(&two), "The Rationalist and The Advocate");
    }

    #[test]
    fn integration_init_agent_files_creates_defaults_and_registry() {
        let dir = tempdir().expect("temp directory should exist");
        let app_data_dir = dir.path().to_path_buf();

        init_agent_files(&app_data_dir).expect("agent files should initialize");

        // Registry should exist
        let registry = load_registry(&app_data_dir);
        assert_eq!(registry.len(), 6);

        // Prompt files should exist
        let files = read_all_agent_files(&app_data_dir).expect("agent files should load");
        assert_eq!(files.len(), 6);
        assert_eq!(files[0].filename, "rationalist.md");
        assert_eq!(files[5].filename, "moderator.md");
    }

    #[test]
    fn integration_custom_agent_lifecycle() {
        let dir = tempdir().expect("temp directory should exist");
        let app_data_dir = dir.path().to_path_buf();

        init_agent_files(&app_data_dir).expect("agent files should initialize");

        // Create custom agent
        let agent = create_custom_agent(&app_data_dir, "Economist", "\u{1f4b0}", "Custom prompt", "female")
            .expect("should create agent");
        assert_eq!(agent.key, "economist");
        assert!(!agent.builtin);
        assert_eq!(agent.role, "debater");
        assert_eq!(agent.voice_gender, "female");

        // Registry should now have 7 agents
        let registry = load_registry(&app_data_dir);
        assert_eq!(registry.len(), 7);

        // Prompt file should exist
        let prompt = read_agent_prompt(&app_data_dir, "economist");
        assert_eq!(prompt, "Custom prompt");

        // Delete custom agent
        delete_custom_agent(&app_data_dir, "economist").expect("should delete agent");
        let registry = load_registry(&app_data_dir);
        assert_eq!(registry.len(), 6);

        // Cannot delete builtin
        let result = delete_custom_agent(&app_data_dir, "rationalist");
        assert!(result.is_err());
    }

    #[test]
    fn integration_read_agent_prompt_with_override() {
        let dir = tempdir().expect("temp directory should exist");
        let app_data_dir = dir.path().to_path_buf();

        init_agent_files(&app_data_dir).expect("agent files should initialize");

        write_agent_file(&app_data_dir, "rationalist.md", "custom prompt")
            .expect("agent file should write");
        let custom_prompt = read_agent_prompt(&app_data_dir, "rationalist");
        assert_eq!(custom_prompt, "custom prompt");
    }
}
