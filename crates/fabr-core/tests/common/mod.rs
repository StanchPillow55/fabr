#![allow(clippy::new_without_default)]
#![allow(dead_code)]
use fabr_core::{Node, Workspace, write_sample};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct Fixture {
    pub tmp: tempfile::TempDir,
    pub root: PathBuf,
}

impl Fixture {
    pub fn new() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("sample");
        assert_eq!(write_sample(&root).unwrap(), 19);
        Fixture { tmp, root }
    }
    pub fn config(&self) -> PathBuf {
        self.tmp.path().join("config")
    }
    pub fn cache(&self) -> PathBuf {
        self.tmp.path().join("cache")
    }
    pub fn ws(&self) -> Workspace {
        Workspace::from_config(&self.config(), &self.cache(), Some(self.root.clone())).unwrap()
    }
    pub fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

pub fn find<'a>(n: &'a Node, path: &str) -> Option<&'a Node> {
    if n.path == path {
        return Some(n);
    }
    n.children.iter().find_map(|c| find(c, path))
}

pub fn paths(n: &Node) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::from([n.path.clone()]);
    for c in &n.children {
        out.extend(paths(c));
    }
    out
}

pub fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}

pub fn fake_ssh() -> PathBuf {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("ssh");
        let script = r#"#!/bin/sh
while [ $# -gt 0 ]; do
  case "$1" in
    -O) exit 0 ;;
    -o) shift 2 ;;
    -*) shift ;;
    *) break ;;
  esac
done
shift
script="$1"
shift
exec sh -c "$script" sh "$@"
"#;
        std::fs::write(&p, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        d
    });
    dir.path().join("ssh")
}
