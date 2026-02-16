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
