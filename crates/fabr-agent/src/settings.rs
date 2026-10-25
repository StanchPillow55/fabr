use fabr_core::{FabrError, Secret};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AgentSettings {
    pub default_folder: String,
    pub model: String,
    pub api_key: Secret,
    pub heartbeat_interval_s: u64,
    pub completed_steps: Vec<String>,
}

pub fn load_agent_settings(dir: &Path) -> Result<AgentSettings, FabrError> {
    let settings_path = dir.join("settings.toml");
    let secrets_path = dir.join("secrets.toml");
    let settings_text = std::fs::read_to_string(&settings_path).map_err(|e| FabrError::Io(format!("{}: {e}", settings_path.display())))?;
    let secrets_text = std::fs::read_to_string(&secrets_path).map_err(|e| FabrError::Io(format!("{}: {e}", secrets_path.display())))?;
    let doc = settings_text.parse::<toml_edit::DocumentMut>().map_err(|e| FabrError::BadRequest(format!("settings.toml: {e}")))?;
    let secrets = secrets_text.parse::<toml_edit::DocumentMut>().map_err(|e| FabrError::BadRequest(format!("secrets.toml: {e}")))?;
    let model = doc.get("model").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let default_folder = doc.get("default_folder").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let heartbeat_interval_s =
        doc.get("heartbeat_interval_s").and_then(|v| v.as_integer()).filter(|n| *n > 0).map(|n| n as u64).unwrap_or(10);
    let completed_steps = doc
        .get("setup")
        .and_then(|s| s.get("completed_steps"))
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let key = secrets.get("anthropic_api_key").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let api_key = Secret::new(key);
    let settings = AgentSettings { default_folder, model, api_key, heartbeat_interval_s, completed_steps };
    require_ready(&settings)?;
    Ok(settings)
}

pub fn require_ready(s: &AgentSettings) -> Result<(), FabrError> {
    for step in ["folder", "api_key", "model"] {
        if !s.completed_steps.iter().any(|have| have == step) {
            return Err(FabrError::BadRequest(format!("setup is missing {step}")));
        }
    }
    if s.model.is_empty() {
        return Err(FabrError::BadRequest("model is required".into()));
    }
    if s.api_key.is_empty() {
        return Err(FabrError::BadRequest("api key is required".into()));
    }
    Ok(())
}
