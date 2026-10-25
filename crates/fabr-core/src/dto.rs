use crate::meta::Meta;
use crate::node::Node;
use crate::secrets::Secret;
use crate::sources::Source;
use crate::todo::Item;
use serde::{Deserialize, Serialize};

pub const VIRTUAL_ROOT_NAME: &str = "/";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceError {
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    pub root: Node,
    pub sources: Vec<Source>,
    pub today: String,
    pub truncated: bool,
    pub truncated_reason: Option<String>,
    #[serde(default)]
    pub truncated_by: Option<String>,
    pub errors: Vec<SourceError>,
    #[serde(default)]
    pub pending: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GetTreeReq {
    #[serde(default)]
    pub show_all: bool,
    #[serde(default)]
    pub rescan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDirsReq {
    pub source: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitChildrenReq {
    pub source: String,
    pub path: String,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRef {
    pub source: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveNodeReq {
    pub source: String,
    pub path: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTodoReq {
    pub source: String,
    pub path: String,
    pub version: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveMetaReq {
    pub source: String,
    pub path: String,
    pub version: String,
    pub meta: Meta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostReq {
    pub host: String,
    #[serde(default)]
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustResult {
    pub added: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourcePasswordReq {
    pub source: String,
    pub password: Secret,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceIdReq {
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordState {
    pub stored: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallKeyReq {
    pub source: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub password: Option<Secret>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stamp {
    pub stamp: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveSourcesReq {
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenUrlReq {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RevealPathReq {
    pub path: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Empty {}
