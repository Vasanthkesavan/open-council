use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub openrouter_api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub agent_models: HashMap<String, String>,
}

fn default_model() -> String {
    "anthropic/claude-sonnet-4-5".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            openrouter_api_key: String::new(),
            model: default_model(),
            agent_models: HashMap::new(),
        }
    }
}

pub fn get_config_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("config.json")
}

pub fn load_config(app_data_dir: &PathBuf) -> AppConfig {
    let path = get_config_path(app_data_dir);
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(app_data_dir: &PathBuf, config: &AppConfig) -> Result<(), String> {
    let path = get_config_path(app_data_dir);
    fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn unit_load_config_returns_default_when_file_missing() {
        let dir = tempdir().expect("temp directory should exist");
        let app_data_dir = dir.path().to_path_buf();

        let loaded = load_config(&app_data_dir);

        assert!(loaded.openrouter_api_key.is_empty());
        assert_eq!(loaded.model, "anthropic/claude-sonnet-4-5");
        assert!(loaded.agent_models.is_empty());
    }

    #[test]
    fn integration_save_and_load_config_round_trip() {
        let dir = tempdir().expect("temp directory should exist");
        let app_data_dir = dir.path().to_path_buf();

        let mut agent_models = HashMap::new();
        agent_models.insert("moderator".to_string(), "anthropic/custom-model".to_string());

        let config = AppConfig {
            openrouter_api_key: "sk-test-key".to_string(),
            model: "anthropic/claude-sonnet-4-5".to_string(),
            agent_models,
        };

        save_config(&app_data_dir, &config).expect("config should save");
        let loaded = load_config(&app_data_dir);

        assert_eq!(loaded.openrouter_api_key, "sk-test-key");
        assert_eq!(loaded.model, "anthropic/claude-sonnet-4-5");
        assert_eq!(
            loaded.agent_models.get("moderator").map(String::as_str),
            Some("anthropic/custom-model")
        );
    }
}
