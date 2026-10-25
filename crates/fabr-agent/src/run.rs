use crate::lifecycle::{self, Ctl, advance_with};
use crate::link::{self, find_for_todo, load_record};
use crate::model::Model;
use crate::settings::load_agent_settings;
use crate::state::{parked, pending_tail};
use crate::store::mutate;
use crate::{require_local, task_path_key};
use fabr_core::taskrec::State;
use fabr_core::todo::Status;
use fabr_core::{FabrError, LocalFs, NodeRef, SaveTodoReq, Workspace, load_node};
use std::path::Path;

pub fn run_once(ws: &Workspace, source: &str, node_path: &str, fabric_space: &Path, model: &impl Model) -> Result<(), FabrError> {
    require_local(source)?;
    let settings = load_agent_settings(fabric_space)?;
    let mut ctl = Ctl::new(fabric_space, settings.heartbeat_interval_s);
    ctl.check_timeout_s = settings.check_timeout_s;
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
            drive(ws, source, node_path, &node_dir, &path, &ctl, model)?;
        } else if item.status != Status::Done {
            let path = link::delegate(ws, source, node_path, &item)?;
            mutate(&path, |r| r.requests.delegate = true)?;
            drive(ws, source, node_path, &node_dir, &path, &ctl, model)?;
        }
    }
    for (path, rec) in link::list_records(&node_dir)? {
        if rec.source.is_none() && (!link::is_terminal(&rec) || pending_tail(&rec)) {
            drive(ws, source, node_path, &node_dir, &path, &ctl, model)?;
        }
    }
    Ok(())
}

fn drive(
    ws: &Workspace,
    source: &str,
    node_path: &str,
    node_dir: &Path,
    path: &Path,
    ctl: &Ctl,
    model: &impl Model,
) -> Result<(), FabrError> {
    for _ in 0..16 {
        let rec = load_record(path)?;
        if link::is_terminal(&rec) && !pending_tail(&rec) {
            break;
        }
        if parked(&rec) || (rec.state == State::Reminder && !rec.requests.delegate) {
            break;
        }
        advance_with(ws, source, node_path, node_dir, path, ctl, model)?;
    }
    Ok(())
}
