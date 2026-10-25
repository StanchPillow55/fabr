pub mod lifecycle;
pub mod link;
pub mod model;
pub mod run;
pub mod settings;
pub mod tools;
pub mod verify;

pub use lifecycle::{approval_allows_execute, registry_path, task_path_key};
pub use link::delegate;
pub use model::{AnthropicModel, DefineDraft, Model, PlanDraft, ScriptedModel, ToolCall};
pub use run::run_once;
pub use settings::{AgentSettings, load_agent_settings};
pub use verify::spawn_allowed;

use fabr_core::FabrError;

pub(crate) fn now_utc() -> String {
    format!("{}Z", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S"))
}

pub(crate) fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub(crate) fn require_local(source: &str) -> Result<(), FabrError> {
    if source == "local" { Ok(()) } else { Err(FabrError::BadRequest("only local sources are supported".into())) }
}
