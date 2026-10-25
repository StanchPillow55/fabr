use fabr_core::FabrError;
use fabr_core::fabric::{FabricCreateReq, create_in};
use fabr_core::taskrec::TaskRecord;
use std::path::{Path, PathBuf};

pub fn open_reminder(node_dir: &Path, title: &str) -> Result<PathBuf, FabrError> {
    let req = FabricCreateReq {
        folder: node_dir.to_string_lossy().into_owned(),
        title: title.to_string(),
        due: None,
        pri: None,
        context: None,
        spec: None,
        success: None,
    };
    Ok(PathBuf::from(create_in(&req, node_dir)?.path))
}

pub(crate) fn load_record(path: &Path) -> Result<TaskRecord, FabrError> {
    let text = std::fs::read_to_string(path).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))?;
    fabr_core::taskrec::parse(&text)
}

pub(crate) fn list_records(node_dir: &Path) -> Result<Vec<(PathBuf, TaskRecord)>, FabrError> {
    let dir = node_dir.join(".fabric-space").join("tasks");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for ent in std::fs::read_dir(&dir).map_err(|e| FabrError::Io(format!("{}: {e}", dir.display())))? {
        let ent = ent.map_err(|e| FabrError::Io(e.to_string()))?;
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        out.push((path.clone(), load_record(&path)?));
    }
    Ok(out)
}

pub(crate) use crate::state::is_terminal;

pub(crate) fn update_record(path: &Path, rec: &mut TaskRecord, base: &mut TaskRecord) -> Result<(), FabrError> {
    crate::store::update(path, rec, base)
}

#[cfg(test)]
pub(crate) fn test_reminder(title: &str) -> TaskRecord {
    let dir = tempfile::tempdir().unwrap();
    let path = open_reminder(dir.path(), title).unwrap();
    load_record(&path).unwrap()
}
