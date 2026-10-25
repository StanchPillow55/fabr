use crate::error::FabrError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Table, value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub host: Option<String>,
    pub root: String,
    #[serde(default)]
    pub prune: Option<Vec<String>>,
    #[serde(default)]
    pub search_depth: Option<u32>,
    #[serde(default, skip_serializing)]
    pub password: Option<crate::secrets::Secret>,
}

pub const SOURCES_FILE: &str = "sources.toml";

pub fn app_dir(config_dir: &Path) -> PathBuf {
    config_dir.join(crate::paths::APP)
}

pub fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

pub fn prune_name_ok(n: &str) -> bool {
    !n.is_empty() && n.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

pub fn validate_source(mut s: Source) -> Result<Source, FabrError> {
    let bad = |m: String| Err(FabrError::BadRequest(m));
    s.id = s.id.trim().to_string();
    if !id_ok(&s.id) {
        return bad(format!("source id {:?} must match ^[a-z0-9_-]+$", s.id));
    }
    s.name = s.name.trim().to_string();
    if s.name.is_empty() {
        s.name = s.id.clone();
    }
    s.host = s.host.map(|h| h.trim().to_string()).filter(|h| !h.is_empty());
    if let Some(h) = &s.host
        && (h.starts_with('-') || h.chars().any(|c| c.is_whitespace() || c.is_control()))
    {
        return bad(format!("source {}: bad host {h:?}", s.id));
    }
    s.root = s.root.trim().to_string();
    if s.root.is_empty() || s.root.contains('\0') || s.root.contains('\n') {
        return bad(format!("source {}: root must be a path", s.id));
    }
    if let Some(p) = &mut s.prune {
        p.retain(|n| !n.trim().is_empty());
        for n in p.iter_mut() {
            *n = n.trim().to_string();
            if !prune_name_ok(n) {
                return bad(format!("source {}: bad prune name {n:?}", s.id));
            }
        }
    }
    if let Some(d) = s.search_depth {
        crate::settings::validate_search_depth(d).map_err(|e| FabrError::BadRequest(format!("source {}: {}", s.id, e.message())))?;
    }
    Ok(s)
}

pub fn validate_sources(sources: Vec<Source>) -> Result<Vec<Source>, FabrError> {
    let mut out: Vec<Source> = Vec::with_capacity(sources.len());
    for s in sources {
        let s = validate_source(s)?;
        if out.iter().any(|o| o.id == s.id) {
            return Err(FabrError::BadRequest(format!("duplicate source id {:?}", s.id)));
        }
        out.push(s);
    }
    Ok(out)
}

fn str_of(t: &Table, k: &str) -> Option<String> {
    t.get(k).and_then(|v| v.as_str()).map(str::to_string)
}

pub fn load_sources(config_dir: &Path) -> Result<Vec<Source>, FabrError> {
    let path = app_dir(config_dir).join(SOURCES_FILE);
    let Some(doc) = read_config(&path)? else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    if let Some(arr) = doc.get("source").and_then(|i| i.as_array_of_tables()) {
        for t in arr.iter() {
            let prune = t
                .get("prune")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect::<Vec<_>>());
            out.push(Source {
                id: str_of(t, "id").unwrap_or_default(),
                name: str_of(t, "name").unwrap_or_default(),
                host: str_of(t, "host"),
                root: str_of(t, "root").unwrap_or_default(),
                prune,
                search_depth: t.get("search_depth").and_then(|v| v.as_integer()).map(|n| n.clamp(0, u32::MAX as i64) as u32),
                password: None,
            });
        }
    }
    validate_sources(out).map_err(|e| FabrError::Unprocessable(format!("{}: {}", path.display(), e.message())))
}

pub(crate) fn read_config(path: &Path) -> Result<Option<DocumentMut>, FabrError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(FabrError::Io(format!("{}: {e}", path.display()))),
    };
    text.parse().map(Some).map_err(|e: toml_edit::TomlError| FabrError::Unprocessable(format!("{}: {e}", path.display())))
}

pub(crate) fn write_config(path: &Path, bytes: &[u8]) -> Result<(), FabrError> {
    use std::io::Write;
    let parent = path.parent().ok_or_else(|| FabrError::Io("bad config path".into()))?;
    std::fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.persist(path).map_err(|e| FabrError::Io(e.to_string()))?;
    Ok(())
}

pub fn save_sources(config_dir: &Path, sources: &[Source]) -> Result<(), FabrError> {
    let mut arr = ArrayOfTables::new();
    for s in sources {
        let mut t = Table::new();
        t["id"] = value(&s.id);
        t["name"] = value(&s.name);
        if let Some(h) = &s.host {
            t["host"] = value(h);
        }
        t["root"] = value(&s.root);
        if let Some(p) = &s.prune {
            t["prune"] = value(p.iter().map(String::as_str).collect::<Array>());
        }
        if let Some(d) = s.search_depth {
            t["search_depth"] = value(d as i64);
        }
        arr.push(t);
    }
    let mut doc = DocumentMut::new();
    doc.insert("source", toml_edit::Item::ArrayOfTables(arr));
    let text = doc.to_string();
    write_config(&app_dir(config_dir).join(SOURCES_FILE), text.as_bytes())
}

pub fn default_local(root: &Path) -> Source {
    let abs = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    let abs = abs.canonicalize().unwrap_or(abs);
    let name = abs.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| abs.display().to_string());
    Source { id: "local".into(), name, host: None, root: abs.display().to_string(), prune: None, search_depth: None, password: None }
}

pub fn local_root(s: &Source) -> PathBuf {
    let r = s.root.as_str();
    if (r == "~" || r.starts_with("~/"))
        && let Some(b) = directories::BaseDirs::new()
    {
        return b.home_dir().join(r.trim_start_matches('~').trim_start_matches('/'));
    }
    PathBuf::from(r)
}

pub fn remote_root(s: &Source) -> String {
    let r = s.root.as_str();
    if r == "~" {
        ".".into()
    } else if let Some(rest) = r.strip_prefix("~/") {
        if rest.is_empty() { ".".into() } else { rest.to_string() }
    } else {
        r.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_validation() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_sources(dir.path()).unwrap().is_empty());
        let s = vec![
            Source {
                id: "local".into(),
                name: "Here".into(),
                host: None,
                root: "/tmp/x".into(),
                prune: None,
                search_depth: None,
                password: None,
            },
            Source {
                id: "box-2".into(),
                name: "Box".into(),
                host: Some("me@box".into()),
                root: "~/notes".into(),
                prune: Some(vec!["node_modules".into()]),
                search_depth: Some(6),
                password: None,
            },
        ];
        save_sources(dir.path(), &s).unwrap();
        assert_eq!(load_sources(dir.path()).unwrap(), s);
        let bad = |f: fn(&mut Source)| {
            let mut x = s[0].clone();
            f(&mut x);
            validate_sources(vec![x]).is_err()
        };
        assert!(bad(|x| x.id = "Bad".into()));
        assert!(bad(|x| x.host = Some("-oProxyCommand=x".into())));
        assert!(bad(|x| x.root = "".into()));
        assert!(bad(|x| x.prune = Some(vec!["a b".into()])));
        assert!(bad(|x| x.search_depth = Some(0)));
        assert!(bad(|x| x.search_depth = Some(13)));
        assert!(validate_sources(vec![s[0].clone(), s[0].clone()]).is_err());
        let v = validate_source(Source { name: "".into(), host: Some(" ".into()), ..s[0].clone() }).unwrap();
        assert_eq!((v.name.as_str(), v.host), ("local", None));
        assert_eq!(remote_root(&s[1]), "notes");
    }
}
