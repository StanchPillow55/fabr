use crate::error::FabrError;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

const SPACE: &str = ".fabric-space";
const BACKEND: &str = "_backend.yaml";
const SKIP: [&str; 2] = ["node_modules", "target"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStamp {
    pub path: String,
    pub mtime: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FabricListReq {
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub home: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FabricListRes {
    pub records: Vec<FileStamp>,
    pub registry: Vec<FileStamp>,
    pub events: Option<FileStamp>,
    #[serde(default)]
    pub backend: Option<FileStamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricReadReq {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricReadRes {
    pub text: String,
    pub mtime: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricWriteReq {
    pub path: String,
    pub text: String,
    pub expected_updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricWriteRes {
    pub mtime: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FabricStampReq {
    pub roots: Vec<String>,
    #[serde(default)]
    pub home: Option<String>,
}

fn mtime_of(m: &std::fs::Metadata) -> u64 {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64)
}

fn stat(p: &Path) -> Option<FileStamp> {
    let m = std::fs::metadata(p).ok().filter(|m| m.is_file())?;
    Some(FileStamp { path: p.to_string_lossy().into_owned(), mtime: mtime_of(&m) })
}

fn files_with_ext(dir: &Path, ext: &str, out: &mut Vec<FileStamp>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == ext)
            && !e.file_name().to_string_lossy().starts_with('.')
            && let Some(s) = stat(&p)
        {
            out.push(s);
        }
    }
}

fn walk(dir: &Path, out: &mut Vec<FileStamp>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name == SPACE {
            files_with_ext(&e.path().join("tasks"), "md", out);
        } else if !name.starts_with('.') && !SKIP.contains(&name.as_ref()) {
            walk(&e.path(), out);
        }
    }
}

pub fn list(req: &FabricListReq) -> Result<FabricListRes, FabrError> {
    let mut res = FabricListRes::default();
    if !req.root.is_empty() {
        let root = Path::new(&req.root);
        if !root.is_dir() {
            return Err(FabrError::NotFound(format!("not a folder: {}", req.root)));
        }
        files_with_ext(&root.join(SPACE).join("tasks"), "md", &mut res.records);
        walk(root, &mut res.records);
        res.records.sort_by(|a, b| a.path.cmp(&b.path));
        res.records.dedup_by(|a, b| a.path == b.path);
    }
    if let Some(h) = req.home.as_deref().filter(|h| !h.is_empty()) {
        let space = Path::new(h).join(SPACE);
        files_with_ext(&space.join("registry"), "yaml", &mut res.registry);
        res.registry.retain(|f| !f.path.ends_with(BACKEND));
        res.registry.sort_by(|a, b| a.path.cmp(&b.path));
        res.events = stat(&space.join("events.log"));
        res.backend = stat(&space.join("registry").join(BACKEND));
    }
    Ok(res)
}

pub fn stamp(req: &FabricStampReq) -> Result<String, FabrError> {
    let mut files: Vec<FileStamp> = Vec::new();
    let mut add = |l: FabricListRes| {
        files.extend(l.records);
        files.extend(l.registry);
        files.extend(l.events);
        files.extend(l.backend);
    };
    for (i, r) in req.roots.iter().enumerate() {
        if let Ok(l) = list(&FabricListReq { root: r.clone(), home: if i == 0 { req.home.clone() } else { None } }) {
            add(l);
        }
    }
    if req.roots.is_empty()
        && let Ok(l) = list(&FabricListReq { root: String::new(), home: req.home.clone() })
    {
        add(l);
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    let mut h = sha1_smol::Sha1::new();
    for f in &files {
        let body = std::fs::read(&f.path).unwrap_or_default();
        let content = sha1_smol::Sha1::from(&body).digest();
        h.update(format!("{}\0{}:{}:{}\n", f.path, f.mtime, body.len(), content).as_bytes());
    }
    Ok(h.digest().to_string())
}

fn checked(path: &str) -> Result<PathBuf, FabrError> {
    let p = Path::new(path);
    let inside = path.contains(&format!("/{SPACE}/"));
    if !inside || !p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return Err(FabrError::BadRequest(format!("only absolute paths inside a {SPACE} folder can be used: {path}")));
    }
    Ok(p.to_path_buf())
}

fn writable(p: &Path) -> bool {
    let parent = p.parent();
    let kind = parent.and_then(|d| d.file_name()).and_then(|n| n.to_str());
    let space = parent.and_then(|d| d.parent()).and_then(|d| d.file_name()).and_then(|n| n.to_str());
    let ext = p.extension().and_then(|e| e.to_str());
    space == Some(SPACE) && matches!((kind, ext), (Some("tasks"), Some("md")) | (Some("registry"), Some("yaml")))
}

pub fn read(req: &FabricReadReq) -> Result<FabricReadRes, FabrError> {
    let p = checked(&req.path)?;
    let m = std::fs::metadata(&p).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => FabrError::NotFound(format!("no such file: {}", req.path)),
        _ => FabrError::Io(format!("{}: {e}", req.path)),
    })?;
    if !m.is_file() {
        return Err(FabrError::BadRequest(format!("not a file: {}", req.path)));
    }
    let text = std::fs::read_to_string(&p).map_err(|e| FabrError::Io(format!("{}: {e}", req.path)))?;
    Ok(FabricReadRes { text, mtime: mtime_of(&m) })
}

pub fn updated_at(text: &str) -> String {
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return String::new();
    }
    for l in lines {
        if l.trim_end() == "---" {
            break;
        }
        if let Some(v) = l.strip_prefix("updated_at:") {
            return v.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
        }
    }
    String::new()
}

pub fn write(req: &FabricWriteReq) -> Result<FabricWriteRes, FabrError> {
    let p = checked(&req.path)?;
    if !writable(&p) {
        return Err(FabrError::BadRequest(format!("only tasks/*.md and registry/*.yaml can be written: {}", req.path)));
    }
    let old = std::fs::read_to_string(&p).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => FabrError::NotFound(format!("no such file: {}", req.path)),
        _ => FabrError::Io(format!("{}: {e}", req.path)),
    })?;
    let current = updated_at(&old);
    if current != req.expected_updated_at {
        return Err(FabrError::Conflict {
            message: format!("{} changed on disk: updated_at is {current:?}, expected {:?}", req.path, req.expected_updated_at),
            node: None,
        });
    }
    let dir = p.parent().ok_or_else(|| FabrError::BadRequest("no parent folder".into()))?;
    let io = |e: std::io::Error| FabrError::Io(format!("{}: {e}", req.path));
    let mut tmp = tempfile::Builder::new().prefix(".fabr-write-").suffix(".tmp").tempfile_in(dir).map_err(io)?;
    tmp.write_all(req.text.as_bytes()).map_err(io)?;
    tmp.as_file().sync_all().map_err(io)?;
    if let Ok(m) = std::fs::metadata(&p) {
        let _ = std::fs::set_permissions(tmp.path(), m.permissions());
    }
    tmp.persist(&p).map_err(|e| io(e.error))?;
    let m = std::fs::metadata(&p).map_err(io)?;
    Ok(FabricWriteRes { mtime: mtime_of(&m) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(updated: &str) -> String {
        format!("---\nid: a\nupdated_at: {updated}\n---\n\n## Context\n")
    }

    fn tree() -> (tempfile::TempDir, String, String) {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("proj");
        for (rel, body) in [
            ("proj/.fabric-space/tasks/a.md", rec("T1")),
            ("proj/sub/deep/.fabric-space/tasks/b.md", rec("T1")),
            ("proj/.fabric-space/tasks/note.txt", "x".into()),
            ("proj/.fabric-space/.agents/knowledge.md", "k".into()),
            ("proj/node_modules/x/.fabric-space/tasks/c.md", rec("T1")),
            ("proj/.hidden/.fabric-space/tasks/d.md", rec("T1")),
            ("proj/other/readme.md", "r".into()),
            ("home/.fabric-space/registry/r1.yaml", "a: 1".into()),
            ("home/.fabric-space/registry/r1.txt", "a: 1".into()),
            ("home/.fabric-space/events.log", "e".into()),
        ] {
            let f = d.path().join(rel);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, body).unwrap();
        }
        let home = d.path().join("home").to_string_lossy().into_owned();
        (d, root.to_string_lossy().into_owned(), home)
    }

    #[test]
    fn list_finds_nested_tasks_only() {
        let (_d, root, home) = tree();
        let r = list(&FabricListReq { root, home: Some(home) }).unwrap();
        let names: Vec<_> = r.records.iter().map(|f| f.path.rsplit('/').next().unwrap().to_string()).collect();
        assert_eq!(names, ["a.md", "b.md"]);
        assert_eq!(r.registry.len(), 1);
        assert!(r.events.unwrap().path.ends_with("events.log"));
    }

    #[test]
    fn read_refuses_outside_fabric_space() {
        let (d, root, _) = tree();
        let outside = format!("{root}/other/readme.md");
        assert!(matches!(read(&FabricReadReq { path: outside }), Err(FabrError::BadRequest(_))));
        let dotdot = format!("{root}/.fabric-space/../other/readme.md");
        assert!(read(&FabricReadReq { path: dotdot }).is_err());
        let ok = read(&FabricReadReq { path: format!("{root}/.fabric-space/tasks/a.md") }).unwrap();
        assert!(ok.text.contains("updated_at: T1"));
        drop(d);
    }

    #[test]
    fn write_checks_updated_at() {
        let (_d, root, _) = tree();
        let path = format!("{root}/.fabric-space/tasks/a.md");
        let stale = FabricWriteReq { path: path.clone(), text: rec("T3"), expected_updated_at: "T0".into() };
        let e = write(&stale).unwrap_err();
        assert!(matches!(e, FabrError::Conflict { .. }));
        assert!(e.message().contains("T1") && e.message().contains("T0"));
        assert!(std::fs::read_to_string(&path).unwrap().contains("updated_at: T1"));
        write(&FabricWriteReq { path: path.clone(), text: rec("T2"), expected_updated_at: "T1".into() }).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("updated_at: T2"));
        let left: Vec<_> = std::fs::read_dir(format!("{root}/.fabric-space/tasks")).unwrap().flatten().collect();
        assert_eq!(left.len(), 2);
    }

    #[test]
    fn write_refuses_other_files() {
        let (_d, root, home) = tree();
        for p in [
            format!("{root}/.fabric-space/tasks/note.txt"),
            format!("{root}/.fabric-space/.agents/knowledge.md"),
            format!("{home}/.fabric-space/events.log"),
        ] {
            let r = write(&FabricWriteReq { path: p, text: "x".into(), expected_updated_at: String::new() });
            assert!(matches!(r, Err(FabrError::BadRequest(_))));
        }
    }

    #[test]
    fn stamp_tracks_changes() {
        let (_d, root, home) = tree();
        let req = FabricStampReq { roots: vec![root.clone()], home: Some(home) };
        let s1 = stamp(&req).unwrap();
        assert_eq!(s1, stamp(&req).unwrap());
        write(&FabricWriteReq {
            path: format!("{root}/.fabric-space/tasks/a.md"),
            text: rec("T2 longer"),
            expected_updated_at: "T1".into(),
        })
        .unwrap();
        assert_ne!(s1, stamp(&req).unwrap());
    }

    fn put(p: &str, body: &str) {
        std::fs::create_dir_all(Path::new(p).parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn list_without_root_returns_home_files_only() {
        let (_d, _root, home) = tree();
        let r = list(&FabricListReq { root: String::new(), home: Some(home) }).unwrap();
        assert!(r.records.is_empty());
        assert_eq!(r.registry.len(), 1);
        assert!(r.events.is_some());
        let none: FabricListReq = serde_json::from_str(r#"{"home":"/nope"}"#).unwrap();
        assert!(none.root.is_empty());
    }

    #[test]
    fn backend_heartbeat_is_reported_and_kept_out_of_registry() {
        let (_d, root, home) = tree();
        let req = FabricListReq { root, home: Some(home.clone()) };
        assert!(list(&req).unwrap().backend.is_none());
        put(&format!("{home}/.fabric-space/registry/_backend.yaml"), "heartbeat_at: 2026-10-24T18:00:00Z\n");
        let r = list(&req).unwrap();
        assert!(r.backend.unwrap().path.ends_with("registry/_backend.yaml"));
        assert_eq!(r.registry.len(), 1);
        assert!(r.registry.iter().all(|f| !f.path.ends_with("_backend.yaml")));
    }

    #[test]
    fn stamp_sees_same_length_same_mtime_edit() {
        let (_d, root, home) = tree();
        let req = FabricStampReq { roots: vec![root.clone()], home: Some(home) };
        let path = format!("{root}/.fabric-space/tasks/a.md");
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        let s1 = stamp(&req).unwrap();
        std::fs::write(&path, rec("T9")).unwrap();
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(before).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), before);
        assert_ne!(s1, stamp(&req).unwrap());
    }

    #[test]
    fn stamp_covers_home_without_roots_and_the_heartbeat() {
        let (_d, _root, home) = tree();
        let req = FabricStampReq { roots: vec![], home: Some(home.clone()) };
        let s1 = stamp(&req).unwrap();
        put(&format!("{home}/.fabric-space/registry/_backend.yaml"), "heartbeat_at: x\n");
        assert_ne!(s1, stamp(&req).unwrap());
    }

    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let t = to.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                copy(&e.path(), &t)
            } else {
                std::fs::copy(e.path(), t).unwrap();
            }
        }
    }

    #[test]
    fn fixture_round_trip() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/05-running");
        let d = tempfile::tempdir().unwrap();
        copy(&src, d.path());
        let (root, home) = (d.path().join("project").to_string_lossy().into_owned(), d.path().join("home").to_string_lossy().into_owned());
        let l = list(&FabricListReq { root, home: Some(home) }).unwrap();
        assert_eq!(l.records.len(), 1);
        assert_eq!(l.registry.len(), 1);
        let path = l.records[0].path.clone();
        let r = read(&FabricReadReq { path: path.clone() }).unwrap();
        let cur = updated_at(&r.text);
        assert_eq!(cur, "2026-10-24T18:59:40Z");
        let text = r.text.replace(&cur, "2026-10-24T19:00:00Z");
        write(&FabricWriteReq { path: path.clone(), text: text.clone(), expected_updated_at: cur.clone() }).unwrap();
        assert_eq!(read(&FabricReadReq { path: path.clone() }).unwrap().text, text);
        assert!(write(&FabricWriteReq { path, text, expected_updated_at: cur }).is_err());
    }
}
