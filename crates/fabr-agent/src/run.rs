use crate::lifecycle::{self, advance};
use crate::link::{self, find_for_todo, load_record};
use crate::model::Model;
use crate::settings::load_agent_settings;
use crate::{require_local, task_path_key};
use fabr_core::taskrec::State;
use fabr_core::todo::Status;
use fabr_core::{FabrError, LocalFs, NodeRef, SaveTodoReq, Workspace, load_node};
use std::path::Path;

pub fn run_once(ws: &Workspace, source: &str, node_path: &str, fabric_space: &Path, model: &impl Model) -> Result<(), FabrError> {
    require_local(source)?;
    let settings = load_agent_settings(fabric_space)?;
    let node_dir = ws.local_path(&NodeRef { source: source.to_string(), path: node_path.to_string() })?;
    let root = ws.local_path(&NodeRef { source: source.to_string(), path: String::new() })?;
    let mut node = load_node(&LocalFs::new(root), source, node_path);
    if !node.managed {
        return Err(FabrError::NotFound("directory is not a fabr node".into()));
    }
    if node.items.iter().any(|i| i.status != Status::Done && i.id.as_deref().is_none_or(str::is_empty)) {
        let version = node.version.clone().ok_or_else(|| FabrError::NotFound("node has no version".into()))?;
        node = ws.save_todo(SaveTodoReq { source: source.to_string(), path: node_path.to_string(), version, items: node.items })?;
    }
    for item in node.items {
        let Some(todo_id) = item.id.clone().filter(|s| !s.is_empty()) else { continue };
        if let Some((path, rec)) = find_for_todo(&node_dir, &todo_id)? {
            if link::is_terminal(&rec) && !pending_tail(&rec) {
                lifecycle::remove_registry(fabric_space, &rec.id, &task_path_key(&path));
                continue;
            }
            drive(ws, source, node_path, &node_dir, &path, fabric_space, settings.heartbeat_interval_s, model)?;
        } else if item.status != Status::Done {
            let path = link::delegate(ws, source, node_path, &item)?;
            drive(ws, source, node_path, &node_dir, &path, fabric_space, settings.heartbeat_interval_s, model)?;
        }
    }
    for (path, rec) in link::list_records(&node_dir)? {
        if rec.source.is_none() && (!link::is_terminal(&rec) || pending_tail(&rec)) {
            drive(ws, source, node_path, &node_dir, &path, fabric_space, settings.heartbeat_interval_s, model)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn drive(
    ws: &Workspace,
    source: &str,
    node_path: &str,
    node_dir: &Path,
    path: &Path,
    fabric_space: &Path,
    heartbeat_interval_s: u64,
    model: &impl Model,
) -> Result<(), FabrError> {
    for _ in 0..16 {
        let rec = load_record(path)?;
        if link::is_terminal(&rec) && !pending_tail(&rec) {
            break;
        }
        if parked(&rec) {
            break;
        }
        advance(ws, source, node_path, node_dir, path, fabric_space, heartbeat_interval_s, model)?;
    }
    Ok(())
}

fn pending_tail(rec: &fabr_core::taskrec::TaskRecord) -> bool {
    matches!(rec.step, Some(fabr_core::taskrec::Step::Learn | fabr_core::taskrec::Step::Notify))
}

fn parked(rec: &fabr_core::taskrec::TaskRecord) -> bool {
    if rec.waiting && rec.question.as_ref().is_some_and(|q| q.answer.is_none()) {
        return true;
    }
    if rec.state == State::Defined {
        let status = rec.approval.as_ref().map(|a| a.status.as_str()).unwrap_or("");
        if status != "approved" && status != "refused" {
            return true;
        }
    }
    false
}
