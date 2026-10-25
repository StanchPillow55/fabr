use crate::fs::Fs;
use crate::item::{Item, items_in};
use crate::meta::{Meta, parse_meta};
use serde::{Deserialize, Serialize};

pub const NODE_DIR: &str = ".fabr";
pub const META_FILE: &str = "meta.toml";
pub const LEGACY_TODO_FILE: &str = ".todo";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub source: String,
    pub path: String,
    pub name: String,
    pub managed: bool,
    pub meta: Meta,
    pub items: Vec<Item>,
    pub warnings: Vec<String>,
    pub version: Option<String>,
    pub via: Option<String>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn plain(source: &str, path: &str, name: &str) -> Node {
        Node {
            source: source.to_string(),
            path: path.to_string(),
            name: name.to_string(),
            managed: false,
            meta: Meta::new(),
            items: Vec::new(),
            warnings: Vec::new(),
            version: None,
            via: None,
            children: Vec::new(),
        }
    }
}

pub fn cmp_name(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

pub fn leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn join(a: &str, b: &str) -> String {
    if a.is_empty() { b.to_string() } else { format!("{a}/{b}") }
}

pub fn version_of(meta: &[u8]) -> String {
    sha1_smol::Sha1::from(meta).digest().to_string()
}

pub fn load_node(fs: &dyn Fs, source: &str, path: &str) -> Node {
    let mut node = Node::plain(source, path, leaf(path));
    let xs = join(path, NODE_DIR);
    if !fs.is_dir(&xs) {
        return node;
    }
    let meta_p = join(&xs, META_FILE);
    let meta_b = fs.read(&meta_p).unwrap_or_default();
    node.managed = true;
    node.version = Some(version_of(&meta_b));
    match std::str::from_utf8(&meta_b) {
        Ok(text) => match parse_meta(text) {
            Ok(m) => {
                node.warnings.extend(crate::meta::color_warning(&m));
                node.meta = m;
            }
            Err(e) => node.warnings.push(format!("meta.toml does not parse: {e}")),
        },
        Err(e) => node.warnings.push(format!("meta.toml does not parse: {e}")),
    }
    if fs.exists(&join(&xs, LEGACY_TODO_FILE)) {
        node.warnings.push(".fabr/.todo is no longer read; run `fabr import` to turn its lines into reminders".into());
    }
    node.items = items_in(fs, path);
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::local::LocalFs;

    #[test]
    fn invalid_color_warns_and_is_kept() {
        let d = tempfile::tempdir().unwrap();
        for (dir, color, warns) in [("a", "\"teal\"", false), ("b", "\"#12ab9F\"", false), ("c", "\"chartreuse\"", true), ("e", "3", true)]
        {
            let xs = d.path().join(dir).join(NODE_DIR);
            std::fs::create_dir_all(&xs).unwrap();
            std::fs::write(xs.join(META_FILE), format!("name = \"{dir}\"\ncolor = {color}\n")).unwrap();
            let n = load_node(&LocalFs::new(d.path().to_path_buf()), "s", dir);
            assert_eq!(n.warnings.iter().any(|w| w.starts_with("color ")), warns, "{dir}: {:?}", n.warnings);
            assert!(n.meta.contains_key("color"));
        }
    }
}
