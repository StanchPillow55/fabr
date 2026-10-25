mod demo;
mod sandbox;

pub use demo::DemoTools;
pub use sandbox::Sandbox;

use crate::model::ToolCall;
use std::path::Path;
use std::sync::Arc;

pub type ToolOutcome = Result<String, String>;

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub read_only: bool,
}

pub trait ToolSet: Send + Sync {
    fn specs(&self) -> Vec<ToolSpec>;

    fn call(&self, node_dir: &Path, call: &ToolCall) -> ToolOutcome;

    fn names(&self) -> Vec<String> {
        self.specs().into_iter().map(|s| s.name).collect()
    }
}

pub fn default_tools() -> Arc<dyn ToolSet> {
    Arc::new(DemoTools)
}

pub struct ReadOnly<'a>(pub &'a dyn ToolSet);

impl ToolSet for ReadOnly<'_> {
    fn specs(&self) -> Vec<ToolSpec> {
        self.0.specs().into_iter().filter(|s| s.read_only).collect()
    }

    fn call(&self, node_dir: &Path, call: &ToolCall) -> ToolOutcome {
        if self.names().contains(&call.name) { self.0.call(node_dir, call) } else { Err(format!("tool {} is not available", call.name)) }
    }
}

pub struct Only<'a> {
    inner: &'a dyn ToolSet,
    names: Vec<String>,
}

impl<'a> Only<'a> {
    pub fn new(inner: &'a dyn ToolSet, names: Vec<String>) -> Self {
        Only { inner, names }
    }
}

impl ToolSet for Only<'_> {
    fn specs(&self) -> Vec<ToolSpec> {
        self.inner.specs().into_iter().filter(|s| self.names.contains(&s.name)).collect()
    }

    fn call(&self, node_dir: &Path, call: &ToolCall) -> ToolOutcome {
        if self.names.contains(&call.name) { self.inner.call(node_dir, call) } else { Err(format!("tool {} is not allowed", call.name)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_hides_the_mutating_tools_and_refuses_their_calls() {
        let all = DemoTools;
        let ro = ReadOnly(&all);
        assert_eq!(all.names(), ["read_file", "list_dir", "write_file", "make_dir"]);
        assert_eq!(ro.names(), ["read_file", "list_dir"]);
        let tmp = tempfile::tempdir().unwrap();
        let call = ToolCall { name: "make_dir".into(), path: "x".into(), content: None };
        assert!(ro.call(tmp.path(), &call).is_err());
        assert!(!tmp.path().join("x").exists());
    }
}
