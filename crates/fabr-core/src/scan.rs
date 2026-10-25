use crate::fs::Fs;
use crate::meta::{MetaValue, shown};
use crate::node::{NODE_DIR, Node, cmp_name, join, load_node};
use crate::sources::Source;
use std::collections::HashSet;
use std::time::Instant;

pub const MAX_NODES: usize = 10_000;
pub const MAX_VISITED: usize = 20_000;
pub const TIME_BUDGET_MS: u64 = 4000;
pub const DEFAULT_PRUNE: [&str; 8] = ["node_modules", ".git", ".venv", "venv", "__pycache__", "target", "dist", "build"];
pub const HOME_PRUNE: [&str; 6] = ["Library", "Applications", "Movies", "Music", "Pictures", "Photos Library.photoslibrary"];

pub struct ScanOpts<'a> {
    pub show_all: bool,
    pub prune: &'a HashSet<String>,
    pub max_nodes: usize,
    pub search_depth: u32,
    pub max_visited: usize,
    pub time_budget_ms: u64,
}

impl<'a> ScanOpts<'a> {
    pub fn new(show_all: bool, prune: &'a HashSet<String>) -> Self {
        ScanOpts {
            show_all,
            prune,
            max_nodes: MAX_NODES,
            search_depth: crate::settings::DEFAULT_SEARCH_DEPTH,
            max_visited: MAX_VISITED,
            time_budget_ms: TIME_BUDGET_MS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    pub path: String,
    pub listed: bool,
}

pub struct ScanResult {
    pub root: Node,
    pub truncated: bool,
    pub truncated_reason: Option<String>,
    pub truncated_by: Option<&'static str>,
    pub visited: usize,
    pub emitted: Vec<Watched>,
    pub elapsed_ms: u64,
}

pub fn default_prune() -> HashSet<String> {
    DEFAULT_PRUNE.iter().map(|s| s.to_string()).collect()
}

pub fn is_home_root(source: &Source) -> bool {
    if source.host.is_some() {
        return false;
    }
    let Some(b) = directories::BaseDirs::new() else { return false };
    let canon = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    canon(&crate::sources::local_root(source)) == canon(b.home_dir())
}

pub fn source_prune(source: &Source) -> HashSet<String> {
    match &source.prune {
        Some(p) => p.iter().cloned().collect(),
        None => {
            let mut p = default_prune();
            if is_home_root(source) {
                p.extend(HOME_PRUNE.iter().map(|s| s.to_string()));
            }
            p
        }
    }
}

pub fn show_depth(node: &mut Node) -> usize {
    match node.meta.get("show_depth") {
        None => 0,
        Some(MetaValue::Int(d)) if *d >= 0 => *d as usize,
        Some(v) => {
            node.warnings.push(format!("show_depth {} is not an integer >= 0; using 0", shown(v)));
            0
        }
    }
}

struct Walker<'a> {
    fs: &'a dyn Fs,
    source: &'a str,
    opts: &'a ScanOpts<'a>,
    count: usize,
    visited: usize,
    start: Instant,
    stop: Option<String>,
    stop_by: Option<&'static str>,
    emitted: Vec<Watched>,
}

impl Walker<'_> {
    fn secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn list(&mut self, d: &str, parent: &mut Node) -> Option<Vec<String>> {
        if self.stop.is_some() {
            return None;
        }
        if self.visited >= self.opts.max_visited {
            self.stop = Some(format!(
                "stopped after {} directories ({:.1} s): narrow the root, prune big folders or lower search_depth",
                self.visited,
                self.secs()
            ));
            self.stop_by = Some("visited");
            return None;
        }
        if self.start.elapsed().as_millis() as u64 > self.opts.time_budget_ms {
            self.stop = Some(format!("the disk was slow ({} directories in {:.1} s); rescanning…", self.visited, self.secs()));
            self.stop_by = Some("time");
            return None;
        }
        self.visited += 1;
        let mut v = match self.fs.try_subdirs(d) {
            Ok(v) => v,
            Err(e) => {
                log::debug!("source {}: cannot read directory {d:?}: {e}", self.source);
                if parent.path == d {
                    parent.warnings.push(format!("cannot read directory: {e}"));
                }
                Vec::new()
            }
        };
        v.retain(|n| !n.starts_with('.'));
        v.sort_by(|a, b| cmp_name(a, b));
        Some(v)
    }

    fn walk(&mut self, d: &str, parent: &mut Node, budget: usize, via: &[String], search: u32, emit: Option<usize>) {
        if !(self.opts.show_all || budget > 0 || search > 0) {
            return;
        }
        let Some(names) = self.list(d, parent) else { return };
        if let Some(i) = emit {
            self.emitted[i].listed = true;
        }
        for name in names {
            if self.stop.is_some() {
                return;
            }
            if self.count >= self.opts.max_nodes {
                self.stop =
                    Some(format!("stopped at {} nodes ({:.1} s): narrow the root or lower show_depth", self.opts.max_nodes, self.secs()));
                self.stop_by = Some("nodes");
                return;
            }
            let child = join(d, &name);
            let pruned = self.opts.prune.contains(&name);
            let via_s = if via.is_empty() { None } else { Some(format!("{}/{}", via.join("/"), name)) };
            let child_search = if pruned { 0 } else { search.saturating_sub(1) };
            let managed = self.fs.is_dir(&join(&child, NODE_DIR));
            if managed || self.opts.show_all || budget > 0 {
                let mut node = load_node(self.fs, self.source, &child);
                node.via = via_s;
                self.count += 1;
                let (depth, s) = if managed {
                    (show_depth(&mut node), if pruned { 0 } else { self.opts.search_depth })
                } else {
                    (budget.saturating_sub(1), child_search)
                };
                let i = self.emit(&child);
                self.walk(&child, &mut node, depth, &[], s, Some(i));
                parent.children.push(node);
            } else if child_search > 0 {
                let mut v = via.to_vec();
                v.push(name);
                self.walk(&child, parent, 0, &v, child_search, None);
            }
        }
    }

    fn emit(&mut self, path: &str) -> usize {
        self.emitted.push(Watched { path: path.to_string(), listed: false });
        self.emitted.len() - 1
    }
}

pub fn scan(fs: &dyn Fs, source: &Source, opts: &ScanOpts) -> ScanResult {
    scan_at(fs, source, "", opts)
}

pub fn scan_at(fs: &dyn Fs, source: &Source, path: &str, opts: &ScanOpts) -> ScanResult {
    let mut top = load_node(fs, &source.id, path);
    if path.is_empty() {
        top.name = source.name.clone();
    }
    let budget = if top.managed { show_depth(&mut top) } else { 0 };
    let mut w = Walker {
        fs,
        source: &source.id,
        opts,
        count: 0,
        visited: 0,
        start: Instant::now(),
        stop: None,
        stop_by: None,
        emitted: vec![Watched { path: path.to_string(), listed: false }],
    };
    w.walk(path, &mut top, budget, &[], opts.search_depth, Some(0));
    let elapsed_ms = w.start.elapsed().as_millis() as u64;
    ScanResult {
        root: top,
        truncated: w.stop.is_some(),
        truncated_reason: w.stop,
        truncated_by: w.stop_by,
        visited: w.visited,
        emitted: w.emitted,
        elapsed_ms,
    }
}

pub fn count_nodes(n: &Node) -> usize {
    1 + n.children.iter().map(count_nodes).sum::<usize>()
}
