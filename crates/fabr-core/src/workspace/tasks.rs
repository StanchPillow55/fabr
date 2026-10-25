use super::{Entry, Workspace};
use crate::dto::Stamp;
use crate::error::FabrError;
use crate::fabric::{self, FabricCreateReq};
use crate::fs::Fs;
use crate::sources::Source;
use crate::sync::lock;
use crate::taskrec;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const TASKS: &str = "/.fabric-space/tasks/";

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteSourceReq {
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteRecord {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteRegistryFile {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteListRes {
    pub records: Vec<RemoteRecord>,
    pub registry: Vec<RemoteRegistryFile>,
    pub events: String,
    pub backend: Option<String>,
    pub stamp: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteWriteReq {
    pub source: String,
    pub path: String,
    pub text: String,
    pub expected_updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteWriteRes {
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteCreateReq {
    pub source: String,
    #[serde(flatten)]
    pub task: FabricCreateReq,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteCreateRes {
    pub path: String,
}

fn bad(msg: impl Into<String>) -> FabrError {
    FabrError::BadRequest(msg.into())
}

fn root_of(s: &Source) -> &str {
    s.root.trim_end_matches('/')
}

fn rel_in_root(s: &Source, folder: &str) -> Result<String, FabrError> {
    let rest = folder
        .strip_prefix(root_of(s))
        .filter(|r| r.is_empty() || r.starts_with('/'))
        .ok_or_else(|| bad(format!("not inside the source root: {folder}")))?;
    let rel = rest.trim_start_matches('/');
    if !rel.is_empty() && (rel.contains('\0') || rel.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..")) {
        return Err(bad(format!("bad folder: {folder}")));
    }
    Ok(rel.to_string())
}

fn split_record(s: &Source, path: &str) -> Result<(String, String), FabrError> {
    let idx = path.rfind(TASKS).ok_or_else(|| bad(format!("not a task record path: {path}")))?;
    let file = &path[idx + TASKS.len()..];
    if !file.ends_with(".md") || file.len() <= 3 || file.contains('/') || file.starts_with('.') || path.split('/').any(|seg| seg == "..") {
        return Err(bad(format!("not a task record path: {path}")));
    }
    Ok((rel_in_root(s, &path[..idx])?, file.to_string()))
}

fn record_key(rel: &str, file: &str) -> String {
    if rel.is_empty() { format!(".fabric-space/tasks/{file}") } else { format!("{rel}/.fabric-space/tasks/{file}") }
}

fn digest(res: &RemoteListRes) -> String {
    let mut h = sha1_smol::Sha1::new();
    let mut part = |tag: &str, a: &str, b: &str| {
        h.update(format!("{tag}\0{}\0{a}\0{}\0{b}\0", a.len(), b.len()).as_bytes());
    };
    for r in &res.records {
        part("r", &r.path, &r.text);
    }
    for r in &res.registry {
        part("y", &r.name, &r.text);
    }
    part("e", "", &res.events);
    part("b", "", res.backend.as_deref().unwrap_or("\0none"));
    h.digest().to_string()
}

impl Workspace {
    fn remote_entry(&self, source: &str) -> Result<Entry, FabrError> {
        let e = self.entry(source)?;
        if e.source.host.is_none() {
            return Err(bad(format!("{source:?} is not a remote source")));
        }
        Ok(e)
    }

    pub fn remote_tasks(&self, req: &RemoteSourceReq) -> Result<RemoteListRes, FabrError> {
        let e = self.remote_entry(&req.source)?;
        let v = self.remote_view(&e);
        let mut res =
            RemoteListRes { records: Vec::new(), registry: Vec::new(), events: String::new(), backend: None, stamp: String::new() };
        let Some(fs) = v.fs else {
            return match v.error {
                Some(m) => Err(FabrError::Remote(m)),
                None => {
                    res.stamp = "pending".into();
                    Ok(res)
                }
            };
        };
        let Some(snap) = fs.as_tar() else { return Err(FabrError::Remote("no snapshot".into())) };
        let root = root_of(&e.source);
        for (key, bytes) in &snap.files {
            let k = format!("/{key}");
            let Some(idx) = k.rfind(TASKS) else { continue };
            let file = &k[idx + TASKS.len()..];
            if file.ends_with(".md") && !file.contains('/') {
                res.records.push(RemoteRecord { path: format!("{root}{k}"), text: String::from_utf8_lossy(bytes).into_owned() });
            }
        }
        res.registry = snap.home_registry().into_iter().map(|(name, text)| RemoteRegistryFile { name, text }).collect();
        res.events = snap.home_events();
        res.backend = snap.home_backend();
        res.stamp = digest(&res);
        Ok(res)
    }

    pub fn remote_task_stamp(&self, req: &RemoteSourceReq) -> Result<Stamp, FabrError> {
        Ok(Stamp { stamp: self.remote_tasks(req)?.stamp })
    }

    fn refresh_remote(&self, e: &Entry) {
        self.invalidate(e);
        if let Ok((fs, stamp)) = e.backend.snapshot_with_stamp() {
            self.remote.replace(&e.source.id, fs, stamp);
        }
    }

    fn snapshot_of(&self, e: &Entry) -> Option<Arc<dyn Fs>> {
        self.remote.snapshot(&e.source.id)
    }

    pub fn remote_task_write(&self, req: &RemoteWriteReq) -> Result<RemoteWriteRes, FabrError> {
        let e = self.remote_entry(&req.source)?;
        let (rel, file) = split_record(&e.source, &req.path)?;
        let _g = lock(&self.write_lock);
        let fs = self.snapshot_of(&e).ok_or_else(|| FabrError::NotFound(format!("source {:?} is not loaded yet", req.source)))?;
        let old = fs
            .as_tar()
            .and_then(|s| s.files.get(&record_key(&rel, &file)))
            .ok_or_else(|| FabrError::NotFound(format!("no such task: {}", req.path)))?;
        let disk = taskrec::parse(&String::from_utf8_lossy(old))?;
        let incoming = taskrec::parse(&req.text)?;
        let rec = fabric::gui_record(&disk, &incoming)?;
        let text = taskrec::render(&rec);
        log::info!("remote task write {}:{}", e.source.id, req.path);
        let r = e.backend.write_task(&rel, &file, &req.expected_updated_at, text.as_bytes());
        self.refresh_remote(&e);
        r?;
        Ok(RemoteWriteRes { updated_at: rec.updated_at })
    }

    pub fn remote_task_create(&self, req: &RemoteCreateReq) -> Result<RemoteCreateRes, FabrError> {
        let e = self.remote_entry(&req.source)?;
        let nt = fabric::validate_new(&req.task)?;
        let folder = req.task.folder.trim().trim_end_matches('/');
        let rel = rel_in_root(&e.source, folder)?;
        let _g = lock(&self.write_lock);
        let fs = self.snapshot_of(&e);
        let now = chrono::Utc::now();
        let base = fabric::new_task_base(&nt.title, now);
        for n in 1..=1000 {
            let id = if n == 1 { base.clone() } else { format!("{base}-{n}") };
            let file = format!("{id}.md");
            if fs.as_ref().and_then(|f| f.as_tar()).is_some_and(|s| s.files.contains_key(&record_key(&rel, &file))) {
                continue;
            }
            let text = fabric::render_new(&req.task, &nt, &id, now);
            let made = e.backend.create_task(&rel, &file, text.as_bytes());
            if !matches!(made, Ok(false)) {
                self.refresh_remote(&e);
            }
            if made? {
                return Ok(RemoteCreateRes { path: format!("{folder}{TASKS}{file}") });
            }
        }
        Err(FabrError::Io("too many tasks with the same title".into()))
    }
}
