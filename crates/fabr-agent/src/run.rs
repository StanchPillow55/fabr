use crate::lifecycle::{self, Ctl, advance_with};
use crate::link::{self, load_record};
use crate::model::Model;
use crate::settings::load_agent_settings;
use crate::state::{parked, pending_tail};
use crate::{require_local, task_path_key};
use fabr_core::taskrec::State;
use fabr_core::{FabrError, LocalFs, NodeRef, Workspace, load_node};
use std::path::Path;

pub fn run_once(ws: &Workspace, source: &str, node_path: &str, fabric_space: &Path, model: &impl Model) -> Result<(), FabrError> {
    require_local(source)?;
    let settings = load_agent_settings(fabric_space)?;
    let mut ctl = Ctl::new(fabric_space, settings.heartbeat_interval_s);
    ctl.check_timeout_s = settings.check_timeout_s;
    ctl.execute_max_turns = settings.execute_max_turns;
    ctl.evaluate_max_reads = settings.evaluate_max_reads;
    let node_dir = ws.local_path(&NodeRef { source: source.to_string(), path: node_path.to_string() })?;
    let root = ws.local_path(&NodeRef { source: source.to_string(), path: String::new() })?;
    let node = load_node(&LocalFs::new(root), source, node_path);
    if !node.managed {
        return Err(FabrError::NotFound("directory is not a fabr node".into()));
    }
    for (path, rec) in link::list_records(&node_dir)? {
        if link::is_terminal(&rec) && !pending_tail(&rec) {
            lifecycle::remove_registry(fabric_space, &rec.id, &task_path_key(&path));
        } else {
            drive(&node_dir, &path, &ctl, model)?;
        }
    }
    Ok(())
}

fn drive(node_dir: &Path, path: &Path, ctl: &Ctl, model: &impl Model) -> Result<(), FabrError> {
    for _ in 0..16 {
        let rec = load_record(path)?;
        if link::is_terminal(&rec) && !pending_tail(&rec) {
            break;
        }
        if parked(&rec) || (rec.state == State::Reminder && !rec.requests.delegate) {
            break;
        }
        advance_with(node_dir, path, ctl, model)?;
    }
    Ok(())
}
