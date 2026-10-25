use crate::{now_utc, require_local, today};
use fabr_core::taskrec::{Grants, Links, Requests, TaskRecord, TaskSource, stamp_updated, write_record};
use fabr_core::todo::Item;
use fabr_core::{FabrError, LocalFs, NodeRef, SaveTodoReq, Workspace, load_node};
use std::path::{Path, PathBuf};

pub fn open_reminder(node_dir: &Path, title: &str) -> Result<PathBuf, FabrError> {
    let tasks = node_dir.join(".fabric-space").join("tasks");
    std::fs::create_dir_all(&tasks)?;
    let id = allocate_id(&tasks, title);
    let item = Item { id: None, status: fabr_core::todo::Status::Todo, due: None, title: title.to_string(), pri: None };
    let mut rec = new_reminder(&id, "", "", &item);
    rec.source = None;
    let path = tasks.join(format!("{id}.md"));
    create_record(&path, &mut rec)?;
    Ok(path)
}

pub fn delegate(ws: &Workspace, source: &str, node_path: &str, item: &Item) -> Result<PathBuf, FabrError> {
    require_local(source)?;
    let node_dir = ws.local_path(&NodeRef { source: source.to_string(), path: node_path.to_string() })?;
    let item = ensure_saved_id(ws, source, node_path, item)?;
    let todo_id = item.id.clone().filter(|s| !s.is_empty()).ok_or_else(|| FabrError::BadRequest("todo id was not assigned".into()))?;
    if let Some(path) = find_active(&node_dir, &todo_id)? {
        return Ok(path);
    }
    let tasks = node_dir.join(".fabric-space").join("tasks");
    std::fs::create_dir_all(&tasks)?;
    let id = allocate_id(&tasks, &item.title);
    let mut rec = new_reminder(&id, node_path, &todo_id, &item);
    let path = tasks.join(format!("{id}.md"));
    create_record(&path, &mut rec)?;
    Ok(path)
}

fn ensure_saved_id(ws: &Workspace, source: &str, node_path: &str, item: &Item) -> Result<Item, FabrError> {
    let root = ws.local_path(&NodeRef { source: source.to_string(), path: String::new() })?;
    let node = load_node(&LocalFs::new(root), source, node_path);
    if !node.managed {
        return Err(FabrError::NotFound("directory is not a fabr node".into()));
    }
    if let Some(id) = item.id.as_ref().filter(|s| !s.is_empty()) {
        return node
            .items
            .into_iter()
            .find(|i| i.id.as_deref() == Some(id.as_str()))
            .ok_or_else(|| FabrError::NotFound(format!("todo {id} is not on this node")));
    }
    let mut items = node.items;
    if !items.iter().any(|i| i.title == item.title && i.id.as_deref().is_none_or(str::is_empty)) {
        items.push(item.clone());
    }
    let version = node.version.ok_or_else(|| FabrError::NotFound("node has no version".into()))?;
    let saved = ws.save_todo(SaveTodoReq { source: source.to_string(), path: node_path.to_string(), version, items })?;
    saved
        .items
        .into_iter()
        .rev()
        .find(|i| i.title == item.title && i.id.as_deref().is_some_and(|s| !s.is_empty()))
        .ok_or_else(|| FabrError::BadRequest("todo id was not assigned".into()))
}

pub(crate) fn new_reminder(id: &str, node_path: &str, todo_id: &str, item: &Item) -> TaskRecord {
    let now = now_utc();
    TaskRecord {
        schema: 2,
        id: id.to_string(),
        source: Some(TaskSource { node_path: node_path.to_string(), todo_id: todo_id.to_string() }),
        title: item.title.clone(),
        due: item.due.clone(),
        state: fabr_core::taskrec::State::Reminder,
        step: None,
        waiting: false,
        created_at: now.clone(),
        updated_at: now,
        requests: Requests { delegate: false, cancel: false, redelegate: false },
        question: None,
        answers: Vec::new(),
        grants: Grants { requested: Vec::new(), granted: Vec::new() },
        plan_meta: None,
        assertions: Vec::new(),
        approval: None,
        verdict: None,
        cost_usd: 0.0,
        links: Links { redelegated_from: None, redelegated_to: None },
        context: String::new(),
        spec: String::new(),
        success: String::new(),
        plan: String::new(),
        log: String::new(),
    }
}

fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut hyphen = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            hyphen = false;
        } else if !hyphen && !out.is_empty() {
            out.push('-');
            hyphen = true;
        }
        if out.len() >= 40 {
            break;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "task".into() } else { out }
}

fn allocate_id(tasks: &Path, title: &str) -> String {
    let date = today();
    let base = slugify(title);
    let mut id = format!("{date}-{base}");
    let mut n = 2u32;
    while tasks.join(format!("{id}.md")).exists() {
        id = format!("{date}-{base}-{n}");
        n += 1;
    }
    id
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

pub(crate) fn find_active(node_dir: &Path, todo_id: &str) -> Result<Option<PathBuf>, FabrError> {
    Ok(list_records(node_dir)?.into_iter().find(|(_, rec)| active_for(rec, todo_id)).map(|(path, _)| path))
}

pub(crate) fn find_for_todo(node_dir: &Path, todo_id: &str) -> Result<Option<(PathBuf, TaskRecord)>, FabrError> {
    let mut terminal = None;
    for (path, rec) in list_records(node_dir)? {
        if rec.source.as_ref().is_none_or(|s| s.todo_id != todo_id) {
            continue;
        }
        if active_for(&rec, todo_id) {
            return Ok(Some((path, rec)));
        }
        terminal = Some((path, rec));
    }
    Ok(terminal)
}

fn active_for(rec: &TaskRecord, todo_id: &str) -> bool {
    rec.source.as_ref().is_some_and(|s| s.todo_id == todo_id) && !is_terminal(rec)
}

pub(crate) fn is_terminal(rec: &TaskRecord) -> bool {
    matches!(rec.state, fabr_core::taskrec::State::Succeeded | fabr_core::taskrec::State::Failed)
}

pub(crate) fn create_record(path: &Path, rec: &mut TaskRecord) -> Result<(), FabrError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = now_utc();
    if rec.created_at.is_empty() {
        rec.created_at = now.clone();
    }
    stamp_updated(rec, &now);
    write_record(path, rec, None)
}

pub(crate) fn update_record(path: &Path, rec: &mut TaskRecord, expected_updated_at: &str) -> Result<(), FabrError> {
    stamp_updated(rec, &now_utc());
    match write_record(path, rec, Some(expected_updated_at)) {
        Ok(()) => Ok(()),
        Err(FabrError::Conflict { message, .. }) => {
            log::warn!("record write conflict on {}: {message}", path.display());
            let disk = load_record(path)?;
            let expected = disk.updated_at.clone();
            stamp_updated(rec, &now_utc());
            write_record(path, rec, Some(&expected))
        }
        Err(e) => Err(e),
    }
}
