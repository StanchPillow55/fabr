pub mod local;
pub mod ssh;

pub use self::local::LocalBackend;
pub use self::ssh::SshBackend;

use crate::error::FabrError;
use crate::fs::Fs;
use crate::meta::{Meta, MetaValue, render_meta};
use crate::node::cmp_name;
use crate::scan::Watched;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub managed: bool,
    pub pruned: bool,
}

pub fn dir_entries(raw: impl IntoIterator<Item = (String, bool)>, prune: &HashSet<String>) -> Vec<DirEntry> {
    let mut v: Vec<DirEntry> = raw
        .into_iter()
        .filter(|(n, _)| !n.starts_with('.') && !n.is_empty())
        .map(|(name, managed)| DirEntry { pruned: prune.contains(&name), name, managed })
        .collect();
    v.sort_by(|a, b| cmp_name(&a.name, &b.name));
    v.dedup_by(|a, b| a.name == b.name);
    v
}

#[derive(Debug, Clone, Default)]
pub struct NodeFiles {
    pub meta: Option<Vec<u8>>,
}

impl NodeFiles {
    pub fn initial(name: &str, show_depth: Option<u32>) -> NodeFiles {
        let mut m = Meta::new();
        m.insert("name".into(), MetaValue::Str(name.to_string()));
        if let Some(d) = show_depth {
            m.insert("show_depth".into(), MetaValue::Int(d as i64));
        }
        let meta = render_meta("", &m).expect("empty toml parses");
        NodeFiles { meta: Some(meta.into_bytes()) }
    }
}

pub(crate) const EXTRA_FILES_MSG: &str = "`.fabr/` contains other files; remove it by hand";
pub(crate) const CONFLICT_MSG: &str = "the node changed on disk since you loaded it";
pub const NOT_A_NODE: &str = "directory is not a fabr node";
pub const ALREADY_NODE: &str = "already a fabr node";

pub trait Backend: Send + Sync {
    fn snapshot(&self) -> Result<Arc<dyn Fs>, FabrError>;
    fn stamp(&self) -> Result<String, FabrError>;
    fn write_node(&self, path: &str, expected_version: &str, files: NodeFiles) -> Result<(), FabrError>;
    fn init_node(&self, path: &str, show_depth: Option<u32>) -> Result<(), FabrError>;
    fn remove_node(&self, path: &str, expected_version: &str) -> Result<(), FabrError>;
    fn list_dirs(&self, path: &str, prune: &HashSet<String>) -> Result<Vec<DirEntry>, FabrError>;
    fn set_watched(&self, paths: Vec<Watched>) {
        let _ = paths;
    }
    fn stamp_for(&self, paths: &[Watched]) -> Result<String, FabrError> {
        let _ = paths;
        self.stamp()
    }
    fn forget_password(&self) {}

    fn snapshot_with_stamp(&self) -> Result<(Arc<dyn Fs>, String), FabrError> {
        Ok((self.snapshot()?, self.stamp()?))
    }
    fn snapshot_waiting_for_login(&self) -> Result<(Arc<dyn Fs>, String), FabrError> {
        self.snapshot_with_stamp()
    }
    fn read_node(&self, path: &str) -> Result<Arc<dyn Fs>, FabrError> {
        let _ = path;
        self.snapshot()
    }
    fn truncated(&self) -> bool {
        false
    }
    fn write_task(&self, folder: &str, file: &str, expected_updated_at: &str, bytes: &[u8]) -> Result<(), FabrError> {
        let _ = (folder, file, expected_updated_at, bytes);
        Err(FabrError::BadRequest("not a remote source".into()))
    }
    fn create_task(&self, folder: &str, file: &str, bytes: &[u8]) -> Result<bool, FabrError> {
        let _ = (folder, file, bytes);
        Err(FabrError::BadRequest("not a remote source".into()))
    }
}
