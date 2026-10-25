use super::{ALREADY_NODE, Backend, CONFLICT_MSG, DirEntry, EXTRA_FILES_MSG, NOT_A_NODE, NodeFiles, dir_entries};
use crate::error::FabrError;
use crate::fs::{Fs, LocalFs};
use crate::node::{META_FILE, NODE_DIR, leaf, version_of};
use crate::scan::Watched;
use crate::sources::{Source, local_root};
use crate::stamp::stamp_paths;
use crate::sync::Shared;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct LocalBackend {
    source: Source,
    root: PathBuf,
    watched: Shared<Arc<Vec<Watched>>>,
}

fn io_at(p: &Path) -> impl Fn(std::io::Error) -> FabrError + '_ {
    move |e| FabrError::Io(format!("{}: {e}", p.display()))
}

fn read_or_empty(p: &Path) -> Result<Vec<u8>, FabrError> {
    match std::fs::read(p) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        r => r.map_err(io_at(p)),
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), FabrError> {
    let io = |e: std::io::Error| FabrError::Io(format!("write failed: {}: {e}", path.display()));
    let parent = path.parent().ok_or_else(|| FabrError::Io("no parent dir".into()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(io)?;
    tmp.write_all(bytes).map_err(io)?;
    tmp.as_file().sync_all().map_err(io)?;
    match std::fs::metadata(path) {
        Ok(m) => std::fs::set_permissions(tmp.path(), m.permissions()).map_err(io)?,
        Err(_) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o644)).map_err(io)?;
            }
        }
    }
    tmp.persist(path).map_err(|e| io(e.error))?;
    Ok(())
}

impl LocalBackend {
    pub fn new(source: Source) -> Self {
        let root = local_root(&source);
        let root = root.canonicalize().unwrap_or(root);
        let watched = Shared::new(Arc::new(vec![Watched { path: String::new(), listed: true }]));
        LocalBackend { source, root, watched }
    }

    fn fs(&self) -> LocalFs {
        LocalFs::new(self.root.clone())
    }

    fn checked_node(&self, path: &str, expected_version: &str) -> Result<(PathBuf, PathBuf), FabrError> {
        let xs = self.fs().resolve(path)?.join(NODE_DIR);
        if !xs.is_dir() {
            return Err(FabrError::NotFound(NOT_A_NODE.into()));
        }
        let meta_p = xs.join(META_FILE);
        if version_of(&read_or_empty(&meta_p)?) != expected_version {
            return Err(FabrError::Conflict { message: CONFLICT_MSG.into(), node: None });
        }
        Ok((xs, meta_p))
    }

    fn check_root(&self) -> Result<(), FabrError> {
        if self.root.is_dir() {
            Ok(())
        } else {
            Err(FabrError::NotFound(format!("source root {} is not a directory", self.root.display())))
        }
    }
}

impl Backend for LocalBackend {
    fn snapshot(&self) -> Result<Arc<dyn Fs>, FabrError> {
        self.check_root()?;
        Ok(Arc::new(self.fs()))
    }

    fn stamp(&self) -> Result<String, FabrError> {
        self.check_root()?;
        let paths = self.watched.get();
        Ok(stamp_paths(&self.root, &paths))
    }

    fn stamp_for(&self, paths: &[Watched]) -> Result<String, FabrError> {
        self.check_root()?;
        Ok(stamp_paths(&self.root, paths))
    }

    fn set_watched(&self, paths: Vec<Watched>) {
        self.watched.set(Arc::new(paths));
    }

    fn list_dirs(&self, path: &str, prune: &HashSet<String>) -> Result<Vec<DirEntry>, FabrError> {
        let d = self.fs().resolve(path)?;
        let rd = std::fs::read_dir(&d).map_err(|e| {
            log::debug!("source {}: cannot read directory {}: {e}", self.source.id, d.display());
            FabrError::Io(format!("cannot read directory: {e}"))
        })?;
        let raw = rd.filter_map(|e| {
            let e = e.ok()?;
            if !e.file_type().ok()?.is_dir() {
                return None;
            }
            let name = e.file_name().into_string().ok()?;
            let managed = e.path().join(NODE_DIR).is_dir();
            Some((name, managed))
        });
        Ok(dir_entries(raw, prune))
    }

    fn read_node(&self, path: &str) -> Result<Arc<dyn Fs>, FabrError> {
        self.fs().resolve(path)?;
        Ok(Arc::new(self.fs()))
    }

    fn write_node(&self, path: &str, expected_version: &str, files: NodeFiles) -> Result<(), FabrError> {
        let (_, meta_p) = self.checked_node(path, expected_version)?;
        if let Some(b) = &files.meta {
            atomic_write(&meta_p, b)?;
        }
        Ok(())
    }

    fn remove_node(&self, path: &str, expected_version: &str) -> Result<(), FabrError> {
        let (xs, meta_p) = self.checked_node(path, expected_version)?;
        for entry in std::fs::read_dir(&xs).map_err(io_at(&xs))? {
            let name = entry.map_err(io_at(&xs))?.file_name();
            if name != META_FILE {
                return Err(FabrError::Conflict { message: EXTRA_FILES_MSG.into(), node: None });
            }
        }
        match std::fs::remove_file(&meta_p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(io_at(&meta_p)(e)),
            _ => {}
        }
        std::fs::remove_dir(&xs).map_err(io_at(&xs))
    }

    fn init_node(&self, path: &str, show_depth: Option<u32>) -> Result<(), FabrError> {
        let d = self.fs().resolve(path)?;
        let xs = d.join(NODE_DIR);
        if xs.exists() {
            return Err(FabrError::Conflict { message: ALREADY_NODE.into(), node: None });
        }
        std::fs::create_dir(&xs).map_err(io_at(&xs))?;
        let name = if path.is_empty() {
            d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.source.name.clone())
        } else {
            leaf(path).to_string()
        };
        let files = NodeFiles::initial(&name, show_depth);
        atomic_write(&xs.join(META_FILE), files.meta.as_deref().unwrap_or_default())?;
        Ok(())
    }
}
