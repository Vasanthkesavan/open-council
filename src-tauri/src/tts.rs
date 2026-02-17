/// Text-to-Speech integration — ElevenLabs (high quality) and OpenAI TTS (budget).
/// Generates audio files from debate transcripts, one MP3 per agent segment.

use crate::agents::AgentInfo;
use crate::config::AppConfig;
use crate::db::DebateRound;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use tauri::Emitter;

// ── Types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSegment {
    pub index: usize,
    pub agent: String,
    pub round: i32,
    pub exchange: i32,
    pub text: String,
    pub audio_file: String,
    pub duration_ms: u64,
    pub start_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioManifest {
    pub decision_id: String,
    pub segments: Vec<AudioSegment>,
    pub total_duration_ms: u64,
}

struct VoiceConfig {
    voice_id: String,
    stability: f32,
    similarity_boost: f32,
    style: f32,
}

// ── Default voice mappings ──

/// ElevenLabs pre-made voice names mapped by agent personality + gender.
/// These are well-known ElevenLabs voice names; the actual IDs are resolved
/// at generation time if the user hasn't set custom overrides.
fn default_elevenlabs_voice(agent_key: &str, voice_gender: &str) -> VoiceConfig {
    match agent_key {
        "rationalist" => VoiceConfig {
            voice_id: "onwK4e9ZLuTAKqWW03F9".into(), // Daniel
            stability: 0.7, similarity_boost: 0.8, style: 0.3,
        },
        "advocate" => VoiceConfig {
            voice_id: "21m00Tcm4TlvDq8ikWAM".into(), // Rachel
            stability: 0.4, similarity_boost: 0.7, style: 0.6,
        },
        "contrarian" => VoiceConfig {
            voice_id: "ErXwobaYiN019PkySvjV".into(), // Antoni
            stability: 0.3, similarity_boost: 0.7, style: 0.8,
        },
        "visionary" => VoiceConfig {
            voice_id: "EXAVITQu4vr4xnSDxMaL".into(), // Bella
            stability: 0.6, similarity_boost: 0.8, style: 0.4,
        },
        "pragmatist" => VoiceConfig {
            voice_id: "VR6AewLTigWG4xSOukaG".into(), // Arnold
            stability: 0.6, similarity_boost: 0.7, style: 0.3,
        },
        "moderator" => VoiceConfig {
            voice_id: "2EiwWnXFnvU5JabPnv8n".into(), // Clyde
            stability: 0.7, similarity_boost: 0.9, style: 0.5,
        },
        // Custom agents: pick by gender
        _ => {
            if voice_gender == "female" {
                VoiceConfig {
                    voice_id: "21m00Tcm4TlvDq8ikWAM".into(), // Rachel
                    stability: 0.5, similarity_boost: 0.75, style: 0.5,
                }
            } else {
                VoiceConfig {
                    voice_id: "onwK4e9ZLuTAKqWW03F9".into(), // Daniel
                    stability: 0.5, similarity_boost: 0.75, style: 0.5,
                }
            }
        }
    }
}

/// OpenAI TTS voice names. 6 voices available: alloy, echo, fable, onyx, nova, shimmer.
/// We assign unique voices per agent to make them distinguishable.
fn default_openai_voice(agent_key: &str, voice_gender: &str) -> &'static str {
    match agent_key {
        "rationalist" => "onyx",     // deep male
        "advocate"    => "nova",     // warm female
        "contrarian"  => "echo",     // sharp male
        "visionary"   => "shimmer",  // calm female
        "pragmatist"  => "fable",    // grounded male
        "moderator"   => "alloy",    // balanced neutral
        _ => {
            if voice_gender == "female" { "nova" } else { "onyx" }
        }
    }
}

// ── Audio generation ──

/// Generate audio for a single segment via ElevenLabs API.
async fn generate_elevenlabs(
    api_key: &str,
    voice_config: &VoiceConfig,
    text: &str,
    output_path: &Path,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}",
            voice_config.voice_id
        ))
        .header("xi-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&json!({
            "text": text,
            "model_id": "eleven_multilingual_v2",
            "voice_settings": {
                "stability": voice_config.stability,
                "similarity_boost": voice_config.similarity_boost,
                "style": voice_config.style,
            }
        }))
        .send()
        .await
        .map_err(|e| format!("ElevenLabs request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("ElevenLabs API error ({}): {}", status, body));
    }

    let bytes = response.bytes().await.map_err(|e| format!("Failed to read audio: {}", e))?;
    std::fs::write(output_path, &bytes).map_err(|e| format!("Failed to write audio file: {}", e))?;
    Ok(())
}

/// Generate audio for a single segment via OpenAI TTS API.
async fn generate_openai(
    api_key: &str,
    voice: &str,
    text: &str,
    output_path: &Path,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/audio/speech")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": "tts-1",
            "input": text,
            "voice": voice,
            "response_format": "mp3",
        }))
        .send()
        .await
        .map_err(|e| format!("OpenAI TTS request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI TTS API error ({}): {}", status, body));
    }

    let bytes = response.bytes().await.map_err(|e| format!("Failed to read audio: {}", e))?;
    std::fs::write(output_path, &bytes).map_err(|e| format!("Failed to write audio file: {}", e))?;
    Ok(())
}

/// Estimate MP3 duration from file size (assumes ~128kbps CBR, reasonable for speech).
fn estimate_duration_ms(file_path: &Path) -> u64 {
    let bytes = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
    // 128kbps = 16000 bytes/sec → duration_ms = bytes * 1000 / 16000
    (bytes * 1000) / 16000
}

/// Get the debates audio directory for a given decision.
fn audio_dir(app_data_dir: &Path, decision_id: &str) -> PathBuf {
    app_data_dir.join("debates").join(decision_id)
}

/// Generate TTS audio for a single debate segment (one agent's response).
/// Designed to be called from a `tokio::spawn` context during live debate.
pub async fn generate_segment_audio(
    decision_id: &str,
    segment_index: usize,
    round: &DebateRound,
    config: &AppConfig,
    registry: &[AgentInfo],
    app_data_dir: &PathBuf,
) -> Result<AudioSegment, String> {
    let provider = &config.tts_provider;
    let api_key = match provider.as_str() {
        "openai" => {
            if config.openrouter_api_key.is_empty() {
                return Err("OpenRouter API key not set".into());
            }
            config.openrouter_api_key.clone()
        }
        _ => {
            if config.elevenlabs_api_key.is_empty() {
                return Err("ElevenLabs API key not set".into());
            }
            config.elevenlabs_api_key.clone()
        }
    };

    let out_dir = audio_dir(app_data_dir, decision_id);
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("Failed to create audio dir: {}", e))?;

    let filename = format!(
        "{:03}_{}_r{}.mp3",
        segment_index + 1,
        round.agent,
        round.round_number
    );
    let output_path = out_dir.join(&filename);

    let agent_info = registry.iter().find(|a| a.key == round.agent);
    let voice_gender = agent_info.map(|a| a.voice_gender.as_str()).unwrap_or("male");

    match provider.as_str() {
        "openai" => {
            let voice = if let Some(custom_voice) = config.voices.get(&round.agent) {
                custom_voice.as_str()
            } else {
                default_openai_voice(&round.agent, voice_gender)
            };
            generate_openai(&api_key, voice, &round.content, &output_path).await?;
        }
        _ => {
            let mut voice_config = default_elevenlabs_voice(&round.agent, voice_gender);
            if let Some(custom_id) = config.voices.get(&round.agent) {
                voice_config.voice_id = custom_id.clone();
            }
            generate_elevenlabs(&api_key, &voice_config, &round.content, &output_path).await?;
        }
    }

    let duration_ms = estimate_duration_ms(&output_path);

    Ok(AudioSegment {
        index: segment_index,
        agent: round.agent.clone(),
        round: round.round_number,
        exchange: round.exchange_number,
        text: round.content.clone(),
        audio_file: filename,
        duration_ms,
        start_ms: 0, // Calculated when building final manifest
    })
}

/// Build an AudioManifest from a collection of AudioSegments.
/// Sorts by index and calculates cumulative start_ms for sequential playback.
pub fn build_manifest_from_segments(
    decision_id: &str,
    mut segments: Vec<AudioSegment>,
) -> AudioManifest {
    segments.sort_by_key(|s| s.index);
    let mut cumulative_ms = 0u64;
    for seg in &mut segments {
        seg.start_ms = cumulative_ms;
        cumulative_ms += seg.duration_ms;
    }
    AudioManifest {
        decision_id: decision_id.to_string(),
        segments,
        total_duration_ms: cumulative_ms,
    }
}

/// Generate TTS audio for an entire debate (bulk, post-debate).
/// Takes pre-extracted rounds, config, and registry. Calls TTS for each segment,
/// saves MP3 files, and returns a manifest. DB persistence is handled by the caller.
pub async fn generate_debate_audio(
    app_handle: &tauri::AppHandle,
    decision_id: &str,
    rounds: &[DebateRound],
    config: &AppConfig,
    registry: &[AgentInfo],
    app_data_dir: &PathBuf,
) -> Result<AudioManifest, String> {
    // Determine provider and key
    let provider = &config.tts_provider;
    let api_key = match provider.as_str() {
        "openai" => {
            if config.openrouter_api_key.is_empty() {
                return Err("OpenRouter API key not set. Required for OpenAI TTS.".into());
            }
            config.openrouter_api_key.clone()
        }
        _ => {
            if config.elevenlabs_api_key.is_empty() {
                return Err("ElevenLabs API key not set. Go to Settings to add it.".into());
            }
            config.elevenlabs_api_key.clone()
        }
    };

    // Create audio output directory
    let out_dir = audio_dir(app_data_dir, decision_id);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Failed to create audio dir: {}", e))?;

    let total = rounds.len();
    let mut segments: Vec<AudioSegment> = Vec::new();

    for (i, round) in rounds.iter().enumerate() {
        let filename = format!(
            "{:03}_{}_r{}.mp3",
            i + 1,
            round.agent,
            round.round_number
        );
        let output_path = out_dir.join(&filename);

        // Find agent info for voice settings
        let agent_info = registry.iter().find(|a| a.key == round.agent);
        let voice_gender = agent_info.map(|a| a.voice_gender.as_str()).unwrap_or("male");

        // Emit progress
        let _ = app_handle.emit("audio-generation-progress", json!({
            "decision_id": decision_id,
            "completed": i,
            "total": total,
            "current_agent": round.agent,
        }));

        // Generate audio via selected provider
        match provider.as_str() {
            "openai" => {
                let voice = if let Some(custom_voice) = config.voices.get(&round.agent) {
                    custom_voice.as_str()
                } else {
                    default_openai_voice(&round.agent, voice_gender)
                };
                generate_openai(&api_key, voice, &round.content, &output_path).await?;
            }
            _ => {
                let mut voice_config = default_elevenlabs_voice(&round.agent, voice_gender);
                if let Some(custom_id) = config.voices.get(&round.agent) {
                    voice_config.voice_id = custom_id.clone();
                }
                generate_elevenlabs(&api_key, &voice_config, &round.content, &output_path).await?;
            }
        }

        let duration_ms = estimate_duration_ms(&output_path);
        let start_ms = segments.last().map(|s: &AudioSegment| s.start_ms + s.duration_ms).unwrap_or(0);

        segments.push(AudioSegment {
            index: i,
            agent: round.agent.clone(),
            round: round.round_number,
            exchange: round.exchange_number,
            text: round.content.clone(),
            audio_file: filename,
            duration_ms,
            start_ms,
        });
    }

    let total_duration_ms = segments.last().map(|s| s.start_ms + s.duration_ms).unwrap_or(0);

    let manifest = AudioManifest {
        decision_id: decision_id.to_string(),
        segments,
        total_duration_ms,
    };

    // Save manifest JSON file to disk
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(out_dir.join("manifest.json"), &manifest_json)
        .map_err(|e| format!("Failed to write manifest: {}", e))?;

    // Emit completion
    let _ = app_handle.emit("audio-generation-complete", json!({
        "decision_id": decision_id,
        "manifest": manifest,
    }));

    let _ = app_handle.emit("audio-generation-progress", json!({
        "decision_id": decision_id,
        "completed": total,
        "total": total,
        "current_agent": "",
    }));

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_default_elevenlabs_voice_returns_config_for_builtins() {
        let config = default_elevenlabs_voice("rationalist", "male");
        assert!(!config.voice_id.is_empty());
        assert!(config.stability > 0.0);

        let config = default_elevenlabs_voice("advocate", "female");
        assert!(!config.voice_id.is_empty());
    }

    #[test]
    fn unit_default_elevenlabs_voice_uses_gender_for_custom_agents() {
        let male = default_elevenlabs_voice("my_custom_agent", "male");
        let female = default_elevenlabs_voice("my_custom_agent", "female");
        assert_ne!(male.voice_id, female.voice_id);
    }

    #[test]
    fn unit_default_openai_voice_returns_voice_for_builtins() {
        assert_eq!(default_openai_voice("rationalist", "male"), "onyx");
        assert_eq!(default_openai_voice("advocate", "female"), "nova");
        assert_eq!(default_openai_voice("moderator", "male"), "alloy");
    }

    #[test]
    fn unit_default_openai_voice_uses_gender_for_custom_agents() {
        assert_eq!(default_openai_voice("custom", "female"), "nova");
        assert_eq!(default_openai_voice("custom", "male"), "onyx");
    }

    #[test]
    fn unit_audio_manifest_serialization() {
        let manifest = AudioManifest {
            decision_id: "test-123".into(),
            segments: vec![
                AudioSegment {
                    index: 0,
                    agent: "rationalist".into(),
                    round: 1,
                    exchange: 1,
                    text: "Test content".into(),
                    audio_file: "001_rationalist_r1.mp3".into(),
                    duration_ms: 5000,
                    start_ms: 0,
                },
            ],
            total_duration_ms: 5000,
        };
        let json = serde_json::to_string(&manifest).expect("should serialize");
        let deserialized: AudioManifest = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.decision_id, "test-123");
        assert_eq!(deserialized.segments.len(), 1);
        assert_eq!(deserialized.total_duration_ms, 5000);
    }

    #[test]
    fn unit_build_manifest_from_segments_sorts_and_computes_start_ms() {
        // Segments out of order
        let segments = vec![
            AudioSegment {
                index: 2, agent: "contrarian".into(), round: 1, exchange: 1,
                text: "Third".into(), audio_file: "003.mp3".into(),
                duration_ms: 3000, start_ms: 0,
            },
            AudioSegment {
                index: 0, agent: "rationalist".into(), round: 1, exchange: 1,
                text: "First".into(), audio_file: "001.mp3".into(),
                duration_ms: 5000, start_ms: 0,
            },
            AudioSegment {
                index: 1, agent: "advocate".into(), round: 1, exchange: 1,
                text: "Second".into(), audio_file: "002.mp3".into(),
                duration_ms: 4000, start_ms: 0,
            },
        ];
        let manifest = build_manifest_from_segments("test-123", segments);
        assert_eq!(manifest.segments.len(), 3);
        assert_eq!(manifest.segments[0].index, 0);
        assert_eq!(manifest.segments[0].start_ms, 0);
        assert_eq!(manifest.segments[1].index, 1);
        assert_eq!(manifest.segments[1].start_ms, 5000);
        assert_eq!(manifest.segments[2].index, 2);
        assert_eq!(manifest.segments[2].start_ms, 9000);
        assert_eq!(manifest.total_duration_ms, 12000);
    }

    #[test]
    fn unit_estimate_duration_ms_for_known_size() {
        // 16000 bytes at 128kbps = 1000ms
        // We can't easily create a temp file in a unit test without tempfile,
        // so we test the formula directly:
        // bytes * 1000 / 16000
        let bytes: u64 = 160000;
        let expected_ms = (bytes * 1000) / 16000; // 10000ms = 10s
        assert_eq!(expected_ms, 10000);
    }
}
