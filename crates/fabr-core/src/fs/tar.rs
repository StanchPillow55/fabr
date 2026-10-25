use super::Fs;
use crate::error::FabrError;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

const MAX_FILE: u64 = 4 << 20;

#[derive(Debug, Clone, Default)]
pub struct TarSnapshot {
    pub dirs: BTreeSet<String>,
    pub files: BTreeMap<String, Vec<u8>>,
}

fn norm(p: &str) -> Option<String> {
    let mut parts = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => return None,
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

impl TarSnapshot {
    pub fn from_tar_capped<R: Read>(r: R, cap: usize) -> Result<(Self, bool), FabrError> {
        let bad = |e: std::io::Error| FabrError::Remote(format!("bad snapshot stream: {e}"));
        let mut snap = TarSnapshot::default();
        let mut ar = tar::Archive::new(r);
        let mut n = 0usize;
        for entry in ar.entries().map_err(bad)? {
            let mut entry = entry.map_err(bad)?;
            n += 1;
            if n > cap {
                return Ok((snap, true));
            }
            let raw = entry.path_bytes();
            let Some(path) = std::str::from_utf8(&raw).ok().and_then(norm) else { continue };
            match entry.header().entry_type() {
                tar::EntryType::Directory => snap.add_dir(&path),
                tar::EntryType::Regular | tar::EntryType::Continuous => {
                    if entry.size() > MAX_FILE {
                        continue;
                    }
                    let mut buf = Vec::with_capacity(entry.size() as usize);
                    entry.read_to_end(&mut buf).map_err(bad)?;
                    if let Some((parent, _)) = path.rsplit_once('/') {
                        snap.add_dir(parent);
                    } else {
                        snap.add_dir("");
                    }
                    snap.files.insert(path, buf);
                }
                _ => {}
            }
        }
        Ok((snap, false))
    }

    fn add_dir(&mut self, path: &str) {
        let mut p = path;
        loop {
            if !self.dirs.insert(p.to_string()) {
                return;
            }
            match p.rsplit_once('/') {
                Some((parent, _)) => p = parent,
                None if p.is_empty() => return,
                None => p = "",
            }
        }
    }

    pub fn hash(&self) -> String {
        let mut h = sha1_smol::Sha1::new();
        for d in &self.dirs {
            h.update(b"d\0");
            h.update(d.as_bytes());
            h.update(b"\0");
        }
        for (p, b) in &self.files {
            h.update(b"f\0");
            h.update(p.as_bytes());
            h.update(format!("\0{}\0", b.len()).as_bytes());
            h.update(b);
        }
        h.digest().to_string()
    }
}

impl Fs for TarSnapshot {
    fn is_dir(&self, rel: &str) -> bool {
        self.dirs.contains(rel)
    }
    fn subdirs(&self, rel: &str) -> Vec<String> {
        let prefix = if rel.is_empty() { String::new() } else { format!("{rel}/") };
        self.dirs
            .range(prefix.clone()..)
            .take_while(|d| d.starts_with(&prefix))
            .filter_map(|d| {
                let rest = &d[prefix.len()..];
                (!rest.is_empty() && !rest.contains('/')).then(|| rest.to_string())
            })
            .collect()
    }
    fn exists(&self, rel: &str) -> bool {
        self.dirs.contains(rel) || self.files.contains_key(rel)
    }
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        self.files.get(rel).cloned()
    }
    fn files(&self, rel: &str) -> Vec<String> {
        let prefix = if rel.is_empty() { String::new() } else { format!("{rel}/") };
        self.files
            .range(prefix.clone()..)
            .take_while(|(p, _)| p.starts_with(&prefix))
            .filter_map(|(p, _)| Some(&p[prefix.len()..]).filter(|r| !r.contains('/')).map(String::from))
            .collect()
    }
}
