mod local;
mod onboard;
mod remote;

use self::local::LocalCache;
use self::remote::{RemoteCache, RemoteView};
use crate::backend::{Backend, CONFLICT_MSG, DirEntry, LocalBackend, NOT_A_NODE, NodeFiles, SshBackend, TODO_HEADER, dir_entries};
use crate::dto::{
    GetTreeReq, InitChildrenReq, ListDirsReq, NodeRef, RemoveNodeReq, SaveMetaReq, SaveTodoReq, SourceError, Stamp, Tree, VIRTUAL_ROOT_NAME,
};
use crate::error::FabrError;
use crate::fs::{Fs, LocalFs};
use crate::meta::{render_meta, validate_meta};
use crate::node::{META_FILE, NODE_DIR, Node, TODO_FILE, join, leaf, load_node};
use crate::scan::{ScanOpts, ScanResult, scan, scan_at, source_prune};
use crate::settings::{Settings, load_settings, save_settings, validate_settings};
use crate::sources::{Source, default_local, load_sources, local_root, save_sources, validate_sources};
use crate::sync::{Shared, lock};
use crate::todo::{Line, parse_todo, render_todo, validate_items};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const SSH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(crate) struct Entry {
    source: Source,
    backend: Arc<dyn Backend>,
    alive: Arc<AtomicBool>,
}

pub struct Workspace {
    config_dir: PathBuf,
    cache_dir: PathBuf,
    entries: Shared<Vec<Entry>>,
    settings: Shared<Settings>,
    remote: RemoteCache,
    local: LocalCache,
    write_lock: Mutex<()>,
    home: Shared<PathBuf>,
}

fn merge_by(a: Option<&'static str>, b: Option<&'static str>) -> Option<&'static str> {
    if a == Some("time") { a } else { b.or(a) }
}

fn log_scan(source: &str, what: &str, r: &ScanResult) {
    log::info!(
        "scan {source}{what}: {} ms, {} dirs visited, {} nodes emitted{}",
        r.elapsed_ms,
        r.visited,
        r.emitted.len(),
        r.truncated_reason.as_deref().map(|m| format!(", truncated: {m}")).unwrap_or_default()
    );
}

fn name_ok(n: &str) -> bool {
    !n.is_empty() && n != "." && !n.contains('/') && !n.contains("..") && !n.contains('\\') && !n.contains('\0')
}

pub fn today() -> String {
    chrono::Local::now().date_naive().format("%Y-%m-%d").to_string()
}

fn check_path(path: &str) -> Result<String, FabrError> {
    let p = path.trim_end_matches('/');
    if path.starts_with('/') || p.split('/').any(|s| s == "..") {
        return Err(FabrError::BadRequest("path is outside the root".into()));
    }
    if p.contains('\0') || p.contains('\\') || (!p.is_empty() && p.split('/').any(|s| s.is_empty() || s == ".")) {
        return Err(FabrError::BadRequest(format!("bad path {path:?}")));
    }
    Ok(p.to_string())
}

impl Workspace {
    pub fn from_config(config_dir: &Path, cache_dir: &Path, local_root_override: Option<PathBuf>) -> Result<Self, FabrError> {
        let sources = match local_root_override {
            Some(root) => vec![default_local(&root)],
            None => load_sources(config_dir)?,
        };
        let settings = load_settings(config_dir).unwrap_or_else(|e| {
            log::warn!("settings: {}; using defaults", e.message());
            Settings::default()
        });
        log::info!("config dir {}", crate::sources::app_dir(config_dir).display());
        for s in &sources {
            match &s.host {
                None => log::info!("source {} (local): {}", s.id, local_root(s).display()),
                Some(h) => log::info!("source {} (ssh {h}): {}", s.id, s.root),
            }
        }
        let ws = Workspace {
            config_dir: config_dir.to_path_buf(),
            cache_dir: cache_dir.to_path_buf(),
            entries: Shared::new(Vec::new()),
            settings: Shared::new(settings),
            remote: RemoteCache::default(),
            local: LocalCache::default(),
            write_lock: Mutex::new(()),
            home: Shared::new(directories::BaseDirs::new().map_or_else(std::env::temp_dir, |b| b.home_dir().to_path_buf())),
        };
        ws.set_sources(sources);
        Ok(ws)
    }

    fn set_sources(&self, sources: Vec<Source>) {
        let old = self.entries();
        let entries: Vec<Entry> = sources
            .into_iter()
            .map(|s| match old.iter().find(|o| o.source == s) {
                Some(o) => o.clone(),
                None => Entry { backend: self.backend_for(&s), source: s, alive: Arc::new(AtomicBool::new(true)) },
            })
            .collect();
        let kept: HashSet<String> =
            entries.iter().filter(|e| old.iter().any(|o| Arc::ptr_eq(&o.alive, &e.alive))).map(|e| e.source.id.clone()).collect();
        for o in &old {
            if !entries.iter().any(|e| Arc::ptr_eq(&o.alive, &e.alive)) {
                o.alive.store(false, Ordering::SeqCst);
            }
        }
        self.entries.set(entries);
        self.remote.retain(&kept);
        self.local.clear();
    }

    pub fn log_path(&self) -> Option<PathBuf> {
        crate::logging::log_path()
    }

    pub fn revealable(&self, p: &Path) -> bool {
        self.log_path().is_some_and(|l| l == p || l.parent() == Some(p))
    }

    fn scan_opts<'a>(&self, source: &Source, show_all: bool, prune: &'a HashSet<String>) -> ScanOpts<'a> {
        let mut o = ScanOpts::new(show_all, prune);
        let s = self.get_settings();
        o.search_depth = source.search_depth.unwrap_or(s.search_depth);
        o.max_nodes = s.max_nodes as usize;
        o
    }

    fn full_rescan_age(&self) -> Duration {
        Duration::from_millis(self.get_settings().full_rescan_ms as u64)
    }

    fn backend_for(&self, source: &Source) -> Arc<dyn Backend> {
        match source.host {
            None => Arc::new(LocalBackend::new(source.clone())),
            Some(_) => Arc::new(SshBackend::new(source.clone(), self.cache_dir.clone(), SSH_TIMEOUT)),
        }
    }

    #[doc(hidden)]
    pub fn local_scan_count(&self) -> usize {
        self.local.count()
    }

    fn entries(&self) -> Vec<Entry> {
        self.entries.get()
    }

    fn entry(&self, id: &str) -> Result<Entry, FabrError> {
        self.entries().into_iter().find(|e| e.source.id == id).ok_or_else(|| FabrError::NotFound(format!("no such source {id:?}")))
    }

    fn remote_max_age(&self) -> Duration {
        Duration::from_millis(self.get_settings().remote_poll_ms as u64)
    }

    fn remote_failure_backoff(&self) -> Duration {
        self.remote_max_age().max(Duration::from_secs(30))
    }

    fn remote_view(&self, e: &Entry) -> RemoteView {
        self.remote.view(e, self.remote_max_age(), self.remote_failure_backoff())
    }

    pub fn wait_remote(&self, timeout: Duration) -> bool {
        self.remote.wait(timeout)
    }

    fn invalidate(&self, e: &Entry) {
        self.remote.invalidate(&e.source.id);
        self.local.invalidate(&e.source.id);
    }

    pub fn get_tree(&self, show_all: bool) -> Tree {
        self.get_tree_req(GetTreeReq { show_all, rescan: false })
    }

    pub fn get_tree_req(&self, req: GetTreeReq) -> Tree {
        let GetTreeReq { show_all, rescan } = req;
        let entries = self.entries();
        let mut roots = Vec::new();
        let mut errors = Vec::new();
        let mut truncated = false;
        let mut reasons = Vec::new();
        let mut by: Option<&'static str> = None;
        let mut pending = Vec::new();
        for e in &entries {
            let prune = source_prune(&e.source);
            let opts = self.scan_opts(&e.source, show_all, &prune);
            let (tree, err) = if e.source.host.is_some() {
                if rescan {
                    self.invalidate(e);
                }
                let RemoteView { fs, error, pending: busy, .. } = self.remote_view(e);
                if busy && fs.is_none() {
                    pending.push(e.source.id.clone());
                }
                let tree = fs.map(|fs| {
                    let r = scan(fs.as_ref(), &e.source, &opts);
                    log_scan(&e.source.id, " (snapshot)", &r);
                    let capped = e.backend.truncated();
                    let reason = r.truncated_reason.or_else(|| {
                        capped.then(|| format!("remote snapshot cut short at {} entries", crate::backend::ssh::MAX_SNAPSHOT_ENTRIES))
                    });
                    (r.root, r.truncated || capped, reason, r.truncated_by)
                });
                (tree, error)
            } else {
                match self.local.tree(e, &opts, self.full_rescan_age(), rescan) {
                    Ok(t) => (Some(t), None),
                    Err(err) => (None, Some(err.message())),
                }
            };
            let mut root = match tree {
                Some((root, t, reason, b)) => {
                    truncated |= t;
                    by = merge_by(by, b);
                    reasons.extend(reason.map(|m| format!("{}: {m}", e.source.id)));
                    root
                }
                None => Node::plain(&e.source.id, "", &e.source.name),
            };
            if let Some(m) = err {
                log::warn!("source {}: {m}", e.source.id);
                root.warnings.push(m.clone());
                errors.push(SourceError { source: e.source.id.clone(), message: m });
            }
            roots.push(root);
        }
        let mut root = Node::plain("", "", VIRTUAL_ROOT_NAME);
        root.children = roots;
        let truncated_reason = (!reasons.is_empty()).then(|| reasons.join("; "));
        Tree {
            root,
            sources: entries.into_iter().map(|e| e.source).collect(),
            today: today(),
            truncated,
            truncated_reason,
            truncated_by: by.map(String::from),
            errors,
            pending,
        }
    }

    pub fn get_stamp(&self) -> Stamp {
        let mut h = sha1_smol::Sha1::new();
        for e in self.entries() {
            h.update(format!("{}\0", e.source.id).as_bytes());
            if e.source.host.is_some() {
                let r = self.remote_view(&e);
                h.update(r.stamp.as_bytes());
                if r.pending && r.fs.is_none() {
                    h.update(b"\0pending");
                }
                if let Some(m) = r.error {
                    h.update(format!("\0!{m}").as_bytes());
                }
            } else {
                let prune = source_prune(&e.source);
                match self.local.stamp(&e, &|all| self.scan_opts(&e.source, all, &prune), self.full_rescan_age()) {
                    Ok(s) => h.update(s.as_bytes()),
                    Err(err) => h.update(format!("!{}", err.message()).as_bytes()),
                }
            }
            h.update(b"\n");
        }
        Stamp { stamp: h.digest().to_string() }
    }

    fn fresh(&self, e: &Entry, path: &str) -> Result<(Node, Arc<dyn Fs>), FabrError> {
        let fs = e.backend.read_node(path)?;
        let mut n = load_node(fs.as_ref(), &e.source.id, path);
        if path.is_empty() {
            n.name = e.source.name.clone();
        }
        Ok((n, fs))
    }

    fn checked(&self, e: &Entry, path: &str, version: &str) -> Result<(Node, Arc<dyn Fs>), FabrError> {
        let (node, fs) = self.fresh(e, path)?;
        if !node.managed {
            return Err(FabrError::NotFound(NOT_A_NODE.into()));
        }
        if node.version.as_deref() != Some(version) {
            return Err(FabrError::Conflict { message: CONFLICT_MSG.into(), node: Some(node) });
        }
        Ok((node, fs))
    }

    fn locked<T>(&self, source: &str, path: &str, f: impl FnOnce(&Entry, &str) -> Result<T, FabrError>) -> Result<T, FabrError> {
        let e = self.entry(source)?;
        let path = check_path(path)?;
        let _g = lock(&self.write_lock);
        f(&e, &path)
    }

    fn settle(&self, e: &Entry, path: &str, r: Result<(), FabrError>) -> Result<(), FabrError> {
        self.invalidate(e);
        r.map_err(|err| match err {
            FabrError::Conflict { message, node: None } => FabrError::Conflict { message, node: self.fresh(e, path).ok().map(|(n, _)| n) },
            other => other,
        })
    }

    fn settle_fresh(&self, e: &Entry, path: &str, r: Result<(), FabrError>) -> Result<Node, FabrError> {
        self.settle(e, path, r)?;
        Ok(self.fresh(e, path)?.0)
    }

    fn save(
        &self,
        op: &str,
        source: &str,
        path: &str,
        version: &str,
        file: &str,
        build: impl FnOnce(&str) -> Result<NodeFiles, FabrError>,
    ) -> Result<Node, FabrError> {
        self.locked(source, path, |e, path| {
            let (_, fs) = self.checked(e, path, version)?;
            let old = fs.read(&join(&join(path, NODE_DIR), file)).unwrap_or_default();
            let files = build(&String::from_utf8_lossy(&old))?;
            log::info!("{op} {}:{path}", e.source.id);
            self.settle_fresh(e, path, e.backend.write_node(path, version, files))
        })
    }

    pub fn save_todo(&self, req: SaveTodoReq) -> Result<Node, FabrError> {
        self.save("save_todo", &req.source, &req.path, &req.version, TODO_FILE, |old| {
            let items = validate_items(req.items)?;
            let mut old = parse_todo(old);
            if old.is_empty() {
                old.push(Line::Raw { text: TODO_HEADER.into(), bad: false, line: 1 });
            }
            Ok(NodeFiles { meta: None, todo: Some(render_todo(&old, items).into_bytes()) })
        })
    }

    pub fn save_meta(&self, req: SaveMetaReq) -> Result<Node, FabrError> {
        self.save("save_meta", &req.source, &req.path, &req.version, META_FILE, |old| {
            let meta = validate_meta(req.meta)?;
            Ok(NodeFiles { meta: Some(render_meta(old, &meta)?.into_bytes()), todo: None })
        })
    }

    pub fn init_node(&self, req: NodeRef) -> Result<Node, FabrError> {
        self.locked(&req.source, &req.path, |e, path| {
            log::info!("init_node {}:{path}", e.source.id);
            self.settle_fresh(e, path, e.backend.init_node(path, Some(1)))
        })
    }

    pub fn remove_node(&self, req: RemoveNodeReq) -> Result<Node, FabrError> {
        if req.source.is_empty() {
            return Err(FabrError::BadRequest("no source given".into()));
        }
        self.locked(&req.source, &req.path, |e, path| {
            self.checked(e, path, &req.version)?;
            log::info!("remove_node {}:{path}", e.source.id);
            self.settle(e, path, e.backend.remove_node(path, &req.version))?;
            let name = if path.is_empty() { e.source.name.as_str() } else { leaf(path) };
            Ok(Node::plain(&e.source.id, path, name))
        })
    }

    pub fn list_dirs(&self, req: ListDirsReq) -> Result<Vec<DirEntry>, FabrError> {
        let e = self.entry(&req.source)?;
        let path = check_path(&req.path)?;
        let prune = source_prune(&e.source);
        if e.source.host.is_some() {
            let snap = self.remote.snapshot(&e.source.id);
            let covered = !e.backend.truncated() && !path.split('/').any(|seg| prune.contains(seg) || seg.starts_with('.'));
            if let Some(fs) = snap.filter(|fs| covered && fs.is_dir(&path)) {
                let raw = fs.subdirs(&path).into_iter().map(|n| {
                    let m = fs.is_dir(&join(&join(&path, &n), NODE_DIR));
                    (n, m)
                });
                return Ok(dir_entries(raw, &prune));
            }
        }
        e.backend.list_dirs(&path, &prune)
    }

    pub fn init_children(&self, req: InitChildrenReq) -> Result<Node, FabrError> {
        let e = self.entry(&req.source)?;
        let path = check_path(&req.path)?;
        let prune = source_prune(&e.source);
        let listing = e.backend.list_dirs(&path, &prune)?;
        let mut bad = Vec::new();
        let mut todo = Vec::new();
        for n in &req.names {
            match listing.iter().find(|d| &d.name == n) {
                Some(d) if name_ok(n) => {
                    if !d.managed && !todo.contains(n) {
                        todo.push(n.clone());
                    }
                }
                _ => bad.push(format!("{n:?}")),
            }
        }
        if !bad.is_empty() {
            return Err(FabrError::BadRequest(format!("not immediate subdirectories of {path:?}: {}", bad.join(", "))));
        }
        {
            let _g = lock(&self.write_lock);
            let r = (|| {
                match e.backend.init_node(&path, Some(1)) {
                    Ok(()) => log::info!("init_children {}:{path}: initialised the parent", e.source.id),
                    Err(FabrError::Conflict { .. }) => {}
                    Err(err) => return Err(err),
                }
                for n in &todo {
                    let child = join(&path, n);
                    log::info!("init_node {}:{child}", e.source.id);
                    match e.backend.init_node(&child, Some(1)) {
                        Ok(()) | Err(FabrError::Conflict { .. }) => {}
                        Err(err) => return Err(err),
                    }
                }
                Ok(())
            })();
            self.invalidate(&e);
            r?;
        }
        let fs = e.backend.snapshot()?;
        let r = scan_at(fs.as_ref(), &e.source, &path, &self.scan_opts(&e.source, false, &prune));
        log_scan(&e.source.id, &format!(" at {path:?}"), &r);
        Ok(r.root)
    }

    pub fn get_node(&self, req: NodeRef) -> Result<Node, FabrError> {
        let e = self.entry(&req.source)?;
        let path = check_path(&req.path)?;
        Ok(self.fresh(&e, &path)?.0)
    }

    pub fn list_sources(&self) -> Vec<Source> {
        self.entries().into_iter().map(|e| e.source).collect()
    }

    pub fn save_sources(&self, sources: Vec<Source>) -> Result<Vec<Source>, FabrError> {
        let mut sources = validate_sources(sources)?;
        for s in &mut sources {
            s.password = None;
        }
        save_sources(&self.config_dir, &sources)?;
        let removed: Vec<String> = self
            .entries()
            .into_iter()
            .filter(|e| e.source.host.is_some() && !sources.iter().any(|s| s.id == e.source.id))
            .map(|e| e.source.id)
            .collect();
        for id in removed {
            if crate::secrets::get_password(&id).is_some() {
                let _ = crate::secrets::delete_password(&id);
            }
        }
        self.set_sources(sources.clone());
        Ok(sources)
    }

    pub fn get_settings(&self) -> Settings {
        let mut s = self.settings.get();
        s.log_path = self.log_path().map(|p| p.display().to_string());
        s
    }

    pub fn save_settings(&self, settings: Settings) -> Result<Settings, FabrError> {
        let s = validate_settings(settings)?;
        save_settings(&self.config_dir, &s)?;
        let changed_depth = {
            let old = self.settings.get();
            old.search_depth != s.search_depth || old.max_nodes != s.max_nodes
        };
        self.settings.set(s);
        if changed_depth {
            self.local.clear();
        }
        log::info!("settings saved");
        Ok(self.get_settings())
    }

    pub fn local_path(&self, r: &NodeRef) -> Result<PathBuf, FabrError> {
        let e = self.entry(&r.source)?;
        if e.source.host.is_some() {
            return Err(FabrError::BadRequest("only local sources can be opened".into()));
        }
        let path = check_path(&r.path)?;
        LocalFs::new(local_root(&e.source)).resolve(&path)
    }
}
