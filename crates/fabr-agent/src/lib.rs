pub mod board;
pub mod lifecycle;
pub mod link;
pub mod model;
pub mod run;
pub mod runner;
pub mod settings;
pub mod state;
pub mod store;
pub mod tools;
pub mod verify;

pub use lifecycle::{Ctl, advance_with, approval_allows_execute, registry_path, task_path_key};
pub use model::{AnthropicModel, CallObserver, DefineDraft, Model, ModelCall, PlanDraft, ScriptedModel, ToolCall};
pub use run::run_once;
pub use settings::{AgentSettings, load_agent_settings};
pub use verify::spawn_allowed;

use fabr_core::FabrError;

pub(crate) fn now_utc() -> String {
    format!("{}Z", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S"))
}

pub(crate) fn require_local(source: &str) -> Result<(), FabrError> {
    if source == "local" { Ok(()) } else { Err(FabrError::BadRequest("only local sources are supported".into())) }
}
