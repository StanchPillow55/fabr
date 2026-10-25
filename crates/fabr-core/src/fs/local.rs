use super::Fs;
use crate::error::FabrError;
use std::path::{Component, Path, PathBuf};

pub struct LocalFs {
    pub root: PathBuf,
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

impl LocalFs {
    pub fn new(root: PathBuf) -> Self {
        LocalFs { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        if rel.is_empty() { self.root.clone() } else { self.root.join(rel) }
    }

    pub fn resolve(&self, rel: &str) -> Result<PathBuf, FabrError> {
        if rel.contains('\0') {
            return Err(FabrError::BadRequest("bad path".into()));
        }
        let root =
            self.root.canonicalize().map_err(|e| FabrError::NotFound(format!("source root {} is missing: {e}", self.root.display())))?;
        let joined = root.join(rel);
        let target = match joined.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                if !normalize(&joined).starts_with(&root) {
                    return Err(FabrError::BadRequest("path is outside the root".into()));
                }
                return Err(FabrError::NotFound("no such directory".into()));
            }
        };
        if !target.starts_with(&root) {
            return Err(FabrError::BadRequest("path is outside the root".into()));
        }
        if !target.is_dir() {
            return Err(FabrError::NotFound("no such directory".into()));
        }
        Ok(target)
    }
}

impl Fs for LocalFs {
    fn is_dir(&self, rel: &str) -> bool {
        self.path(rel).is_dir()
    }
    fn subdirs(&self, rel: &str) -> Vec<String> {
        self.try_subdirs(rel).unwrap_or_else(|e| {
            log::debug!("cannot read directory {}: {e}", self.path(rel).display());
            Vec::new()
        })
    }
    fn try_subdirs(&self, rel: &str) -> Result<Vec<String>, String> {
        let rd = std::fs::read_dir(self.path(rel)).map_err(|e| e.to_string())?;
        Ok(rd
            .filter_map(|e| {
                let e = e.ok()?;
                if !e.file_type().ok()?.is_dir() {
                    return None;
                }
                e.file_name().into_string().ok()
            })
            .collect())
    }
    fn exists(&self, rel: &str) -> bool {
        self.path(rel).exists()
    }
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(rel)).ok()
    }
    fn files(&self, rel: &str) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(self.path(rel)) else { return Vec::new() };
        rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file())).filter_map(|e| e.file_name().into_string().ok()).collect()
    }
}
