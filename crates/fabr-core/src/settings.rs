use crate::error::FabrError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub poll_ms: u32,
    pub remote_poll_ms: u32,
    #[serde(default = "default_search_depth")]
    pub search_depth: u32,
    #[serde(default = "default_full_rescan_ms")]
    pub full_rescan_ms: u32,
    #[serde(default = "default_max_nodes")]
    pub max_nodes: u32,
    #[serde(default)]
    pub log_path: Option<String>,
}

pub const DEFAULT_SEARCH_DEPTH: u32 = 4;
pub const MAX_SEARCH_DEPTH: u32 = 12;
pub const DEFAULT_FULL_RESCAN_MS: u32 = 300_000;

pub const DEFAULT_MAX_NODES: u32 = 10_000;
pub const MIN_MAX_NODES: u32 = 100;
pub const MAX_MAX_NODES: u32 = 100_000;

fn default_max_nodes() -> u32 {
    DEFAULT_MAX_NODES
}
fn default_search_depth() -> u32 {
    DEFAULT_SEARCH_DEPTH
}
fn default_full_rescan_ms() -> u32 {
    DEFAULT_FULL_RESCAN_MS
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            poll_ms: 2000,
            remote_poll_ms: 30_000,
            search_depth: DEFAULT_SEARCH_DEPTH,
            full_rescan_ms: DEFAULT_FULL_RESCAN_MS,
            max_nodes: DEFAULT_MAX_NODES,
            log_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: u64,
    pub node_count: u32,
    #[serde(default)]
    pub managed_root: bool,
    #[serde(default)]
    pub failure_kind: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

pub const SETTINGS_FILE: &str = "settings.toml";

pub fn validate_search_depth(d: u32) -> Result<u32, FabrError> {
    if (1..=MAX_SEARCH_DEPTH).contains(&d) {
        Ok(d)
    } else {
        Err(FabrError::BadRequest(format!("search_depth must be 1..={MAX_SEARCH_DEPTH}")))
    }
}

pub fn validate_settings(mut s: Settings) -> Result<Settings, FabrError> {
    s.log_path = None;
    validate_search_depth(s.search_depth)?;
    if s.full_rescan_ms < 1000 {
        return Err(FabrError::BadRequest("full_rescan_ms must be >= 1000".into()));
    }
    if !(MIN_MAX_NODES..=MAX_MAX_NODES).contains(&s.max_nodes) {
        return Err(FabrError::BadRequest(format!("max_nodes must be {MIN_MAX_NODES}..={MAX_MAX_NODES}")));
    }
    if s.poll_ms < 250 {
        return Err(FabrError::BadRequest("poll_ms must be >= 250".into()));
    }
    if s.remote_poll_ms < 1000 {
        return Err(FabrError::BadRequest("remote_poll_ms must be >= 1000".into()));
    }
    Ok(s)
}

pub fn load_settings(config_dir: &Path) -> Result<Settings, FabrError> {
    let path = crate::sources::app_dir(config_dir).join(SETTINGS_FILE);
    let Some(doc) = crate::sources::read_config(&path)? else { return Ok(Settings::default()) };
    let d = Settings::default();
    let get = |k: &str, dflt: u32| doc.get(k).and_then(|v| v.as_integer()).map_or(dflt, |n| n.clamp(0, u32::MAX as i64) as u32);
    let s = Settings {
        poll_ms: get("poll_ms", d.poll_ms),
        remote_poll_ms: get("remote_poll_ms", d.remote_poll_ms),
        search_depth: get("search_depth", d.search_depth).clamp(1, MAX_SEARCH_DEPTH),
        full_rescan_ms: get("full_rescan_ms", d.full_rescan_ms).max(1000),
        max_nodes: get("max_nodes", d.max_nodes).clamp(MIN_MAX_NODES, MAX_MAX_NODES),
        log_path: None,
    };
    Ok(s)
}

pub fn save_settings(config_dir: &Path, s: &Settings) -> Result<(), FabrError> {
    let path = crate::sources::app_dir(config_dir).join(SETTINGS_FILE);
    let text = format!(
        "poll_ms = {}\nremote_poll_ms = {}\nsearch_depth = {}\nfull_rescan_ms = {}\nmax_nodes = {}\n",
        s.poll_ms, s.remote_poll_ms, s.search_depth, s.full_rescan_ms, s.max_nodes
    );
    crate::sources::write_config(&path, text.as_bytes())
}
