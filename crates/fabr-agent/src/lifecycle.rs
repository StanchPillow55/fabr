use crate::link::{is_terminal, list_records, load_record, update_record};
use crate::model::Model;
use crate::now_utc;
use crate::tools;
use crate::verify::{self, CheckOutcome};
use fabr_core::taskrec::{
    Approval, Assertion, Check, PlanMeta, Question, State, Step, TaskRecord, Verdict, VerdictItem, approval_digest, registry_filename,
};
use fabr_core::todo::Status;
use fabr_core::{FabrError, NodeRef, SaveTodoReq, Workspace};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub fn approval_allows_execute(rec: &TaskRecord) -> Result<(), FabrError> {
    let Some(approval) = &rec.approval else {
        return Err(FabrError::BadRequest("approval is missing".into()));
    };
    if approval.status != "approved" {
        return Err(FabrError::BadRequest(format!("approval is {}", approval.status)));
    }
    if approval.by.as_deref() == Some("auto") && rec.plan_meta.as_ref().and_then(|m| m.reversibility.as_deref()) == Some("irreversible") {
        return Err(FabrError::BadRequest("auto approval is not allowed for an irreversible plan".into()));
    }
    unsupported_tools(rec)?;
    let digest = approval_digest(rec);
    if approval.digest.as_deref() != Some(digest.as_str()) {
        return Err(FabrError::BadRequest("approval digest does not match the plan".into()));
    }
    Ok(())
}

fn unsupported_tools(rec: &TaskRecord) -> Result<(), FabrError> {
    let Some(meta) = &rec.plan_meta else {
        return Ok(());
    };
    for tool in &meta.tools {
        if !matches!(tool.as_str(), "read_file" | "write_file" | "list_dir") {
            return Err(FabrError::BadRequest(format!("unsupported tool {tool}")));
        }
    }
    for check in &meta.checks {
        if matches!(check.cmd.as_str(), "sh" | "bash" | "zsh" | "shell") {
            return Err(FabrError::BadRequest(format!("unsupported tool {}", check.cmd)));
        }
    }
    Ok(())
}

pub fn task_path_key(path: &Path) -> String {
    let canon = path.canonicalize().unwrap_or_else(|_| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
    canon.to_string_lossy().trim_end_matches('/').to_string()
}

pub fn registry_path(fabric_space: &Path, task_id: &str, task_path: &str) -> PathBuf {
    fabric_space.join("registry").join(registry_filename(task_id, task_path))
}

pub fn remove_registry(fabric_space: &Path, task_id: &str, task_path: &str) {
    let path = registry_path(fabric_space, task_id, task_path);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("remove registry {}: {e}", path.display()),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn advance(
    ws: &Workspace,
    source: &str,
    node_path: &str,
    node_dir: &Path,
    record_path: &Path,
    fabric_space: &Path,
    heartbeat_interval_s: u64,
    model: &impl Model,
) -> Result<(), FabrError> {
    let mut rec = load_record(record_path)?;
    let expected = rec.updated_at.clone();
    if is_terminal(&rec) && !matches!(rec.step, Some(Step::Learn | Step::Notify)) {
        remove_registry(fabric_space, &rec.id, &task_path_key(record_path));
        return Ok(());
    }
    if rec.requests.redelegate {
        let state = state_name(rec.state);
        append_log(&mut rec.log, "notify", &format!("ignored request redelegate in state {state}"));
        rec.requests.redelegate = false;
        let expected = persist(record_path, &mut rec, &expected)?;
        let _ = expected;
        return Ok(());
    }
    match rec.state {
        State::Reminder => {
            if rec.requests.cancel {
                ignore_request(record_path, &mut rec, &expected, "cancel")?;
                return Ok(());
            }
            rec.requests.delegate = false;
            rec.state = State::Task;
            rec.step = Some(Step::Gather);
            rec.waiting = false;
            append_log(&mut rec.log, "task", "delegated");
            persist(record_path, &mut rec, &expected)?;
            project(ws, source, node_path, &rec, Status::Wip)?;
            Ok(())
        }
        State::Task => match rec.step {
            Some(Step::Define) => define_stage(record_path, &mut rec, &expected, model),
            Some(Step::Plan) => plan_stage(ws, source, node_path, record_path, &mut rec, &expected, model, false),
            _ => gather_stage(node_dir, record_path, &mut rec, &expected),
        },
        State::Defined => {
            if rec.requests.cancel {
                ignore_request(record_path, &mut rec, &expected, "cancel")?;
                return Ok(());
            }
            let status = rec.approval.as_ref().map(|a| a.status.as_str()).unwrap_or("");
            if status == "refused" {
                let note = rec.approval.as_ref().and_then(|a| a.note.clone());
                match &note {
                    Some(n) => append_log(&mut rec.log, "approve", &format!("refused: {n}")),
                    None => append_log(&mut rec.log, "approve", "refused"),
                }
                return plan_stage(ws, source, node_path, record_path, &mut rec, &expected, model, true);
            }
            if status != "approved" {
                return Ok(());
            }
            approval_allows_execute(&rec)?;
            if other_running(node_dir, record_path)? {
                log::info!("node already has an in_progress task");
                return Ok(());
            }
            rec.state = State::InProgress;
            rec.step = Some(Step::Execute);
            rec.waiting = false;
            append_log(&mut rec.log, "execute", "started");
            persist(record_path, &mut rec, &expected)?;
            write_registry(fabric_space, node_dir, record_path, &rec, heartbeat_interval_s)?;
            project(ws, source, node_path, &rec, Status::Wip)
        }
        State::InProgress => match rec.step {
            Some(Step::Evaluate) => finish_checks(ws, source, node_path, node_dir, record_path, &mut rec, &expected),
            Some(Step::Learn) => {
                rec.step = Some(Step::Notify);
                append_log(&mut rec.log, "learn", "deferred");
                persist(record_path, &mut rec, &expected)?;
                Ok(())
            }
            Some(Step::Notify) => {
                let result = state_name(rec.state);
                append_log(&mut rec.log, "notify", &format!("result {result}"));
                rec.step = None;
                persist(record_path, &mut rec, &expected)?;
                remove_registry(fabric_space, &rec.id, &task_path_key(record_path));
                Ok(())
            }
            _ => execute_stage(node_dir, record_path, fabric_space, &mut rec, &expected, heartbeat_interval_s, model),
        },
        State::Succeeded | State::Failed => match rec.step {
            Some(Step::Learn) => {
                rec.step = Some(Step::Notify);
                append_log(&mut rec.log, "learn", "deferred");
                persist(record_path, &mut rec, &expected)?;
                Ok(())
            }
            Some(Step::Notify) => {
                let result = state_name(rec.state);
                append_log(&mut rec.log, "notify", &format!("result {result}"));
                rec.step = None;
                persist(record_path, &mut rec, &expected)?;
                remove_registry(fabric_space, &rec.id, &task_path_key(record_path));
                Ok(())
            }
            _ => {
                remove_registry(fabric_space, &rec.id, &task_path_key(record_path));
                Ok(())
            }
        },
    }
}

fn gather_stage(node_dir: &Path, record_path: &Path, rec: &mut TaskRecord, expected: &str) -> Result<(), FabrError> {
    append_gathered(&mut rec.context, &gather_bundle(node_dir));
    rec.step = Some(Step::Define);
    rec.waiting = false;
    append_log(&mut rec.log, "gather", "context bundle written");
    persist(record_path, rec, expected)?;
    Ok(())
}

fn gather_bundle(node_dir: &Path) -> String {
    let mut lines = Vec::new();
    if let Ok(rd) = std::fs::read_dir(node_dir) {
        let mut names: Vec<_> = rd.flatten().collect();
        names.sort_by_key(|e| e.file_name());
        for ent in names {
            let name = ent.file_name().to_string_lossy().to_string();
            if name == ".fabr" || name == ".fabric-space" || name.starts_with('.') {
                continue;
            }
            let path = ent.path();
            if path.is_file() && ent.metadata().is_ok_and(|m| m.len() <= 4096) {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let preview = text.lines().next().unwrap_or("").trim();
                lines.push(format!("{name}: {preview}"));
            } else {
                lines.push(name);
            }
        }
    }
    let body = if lines.is_empty() { "(empty)".to_string() } else { lines.join("\n") };
    format!("### Gathered\n{body}\n")
}

fn append_gathered(context: &mut String, bundle: &str) {
    if let Some(i) = context.find("### Gathered") {
        context.truncate(i);
    }
    if !context.is_empty() && !context.ends_with('\n') {
        context.push('\n');
    }
    context.push_str(bundle);
}

fn define_stage(record_path: &Path, rec: &mut TaskRecord, expected: &str, model: &impl Model) -> Result<(), FabrError> {
    if rec.waiting && rec.question.as_ref().is_some_and(|q| q.answer.is_none()) {
        return Ok(());
    }
    let draft = model.define(&rec.title, &rec.context)?;
    if let Some(text) = draft.question.filter(|q| !q.trim().is_empty()) {
        rec.question = Some(Question {
            id: "q1".into(),
            kind: "open".into(),
            text,
            options: None,
            prior: None,
            fields: None,
            asked_at: Some(now_utc()),
            answer: None,
        });
        rec.waiting = true;
        rec.step = Some(Step::Define);
        append_log(&mut rec.log, "define", "asked q1");
        persist(record_path, rec, expected)?;
        return Ok(());
    }
    if draft.spec.trim().is_empty() || draft.success.trim().is_empty() || draft.assertions.is_empty() {
        return Err(FabrError::BadRequest("spec is incomplete".into()));
    }
    rec.spec = draft.spec;
    rec.success = draft.success;
    rec.assertions = draft.assertions.into_iter().map(|a| Assertion { id: a.id, text: a.text }).collect();
    rec.question = None;
    rec.waiting = false;
    rec.step = Some(Step::Plan);
    append_log(&mut rec.log, "define", "spec written");
    persist(record_path, rec, expected)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn plan_stage(
    ws: &Workspace,
    source: &str,
    node_path: &str,
    record_path: &Path,
    rec: &mut TaskRecord,
    expected: &str,
    model: &impl Model,
    refused: bool,
) -> Result<(), FabrError> {
    let note = if refused { rec.approval.as_ref().and_then(|a| a.note.clone()) } else { None };
    let draft = model.plan(&rec.title, &rec.spec, &rec.success, note.as_deref())?;
    if draft.plan.trim().is_empty() || draft.checks.is_empty() {
        return Err(FabrError::BadRequest("plan is incomplete".into()));
    }
    if !draft.spec.trim().is_empty() {
        rec.spec = draft.spec;
    }
    if !draft.success.trim().is_empty() {
        rec.success = draft.success;
    }
    if !draft.assertions.is_empty() {
        rec.assertions = draft.assertions.into_iter().map(|a| Assertion { id: a.id, text: a.text }).collect();
    }
    rec.plan = draft.plan;
    rec.plan_meta = Some(PlanMeta {
        reversibility: Some(draft.reversibility),
        tools: draft.tools,
        assumptions: Vec::new(),
        checks: draft.checks.into_iter().map(|c| Check { cmd: c.cmd, args: c.args }).collect(),
    });
    let digest = approval_digest(rec);
    rec.approval = Some(Approval { status: "pending".into(), by: None, at: None, note: None, digest: Some(digest) });
    rec.state = State::Defined;
    rec.step = Some(Step::Approve);
    rec.waiting = true;
    append_log(&mut rec.log, "plan", if refused { "redrafted" } else { "plan written" });
    persist(record_path, rec, expected)?;
    project(ws, source, node_path, rec, Status::Wip)
}

fn execute_stage(
    node_dir: &Path,
    record_path: &Path,
    fabric_space: &Path,
    rec: &mut TaskRecord,
    expected: &str,
    heartbeat_interval_s: u64,
    model: &impl Model,
) -> Result<(), FabrError> {
    if rec.requests.cancel {
        rec.requests.cancel = false;
        rec.state = State::Failed;
        rec.step = None;
        rec.waiting = false;
        rec.verdict = None;
        append_log(&mut rec.log, "execute", "cancelled by user");
        persist(record_path, rec, expected)?;
        remove_registry(fabric_space, &rec.id, &task_path_key(record_path));
        return Ok(());
    }
    let calls = model.execute(&rec.title, &rec.spec, &rec.plan)?;
    let mut expected = expected.to_string();
    for call in calls {
        if call.name == "write_file" && log_has_write(&rec.log, &call.path) {
            continue;
        }
        let message = apply_tool(node_dir, &call)?;
        append_log(&mut rec.log, "execute", &message);
        expected = persist(record_path, rec, &expected)?;
        write_registry(fabric_space, node_dir, record_path, rec, heartbeat_interval_s)?;
    }
    rec.step = Some(Step::Evaluate);
    rec.verdict = None;
    append_log(&mut rec.log, "execute", "tools finished");
    persist(record_path, rec, &expected)?;
    Ok(())
}

fn finish_checks(
    ws: &Workspace,
    source: &str,
    node_path: &str,
    node_dir: &Path,
    record_path: &Path,
    rec: &mut TaskRecord,
    expected: &str,
) -> Result<(), FabrError> {
    let checks = rec.plan_meta.as_ref().map(|m| m.checks.clone()).unwrap_or_default();
    let outcomes = verify::run_checks(node_dir, &checks)?;
    let items = verdict_items(node_dir, rec, &outcomes);
    let checks_pass = !outcomes.is_empty() && outcomes.iter().all(|o| o.pass);
    let pass = checks_pass && items.iter().all(|item| item.pass);
    rec.verdict = Some(Verdict { result: if pass { "pass" } else { "fail" }.into(), at: now_utc(), items });
    rec.state = if pass { State::Succeeded } else { State::Failed };
    rec.step = Some(Step::Learn);
    rec.waiting = false;
    append_log(&mut rec.log, "evaluate", if pass { "verdict pass" } else { "verdict fail" });
    persist(record_path, rec, expected)?;
    project(ws, source, node_path, rec, if pass { Status::Done } else { Status::Wip })
}

fn verdict_items(node_dir: &Path, rec: &TaskRecord, outcomes: &[CheckOutcome]) -> Vec<VerdictItem> {
    if rec.assertions.is_empty() {
        return outcomes
            .iter()
            .enumerate()
            .map(|(i, outcome)| VerdictItem { id: format!("c{}", i + 1), pass: outcome.pass, evidence: outcome.evidence.clone() })
            .collect();
    }
    rec.assertions
        .iter()
        .enumerate()
        .map(|(i, assertion)| {
            let (pass, evidence) = assertion_evidence(node_dir, &assertion.text, outcomes.get(i));
            VerdictItem { id: assertion.id.clone(), pass, evidence }
        })
        .collect()
}

fn assertion_evidence(node_dir: &Path, text: &str, outcome: Option<&CheckOutcome>) -> (bool, String) {
    if let Some((file, needle)) = text.split_once(" contains ") {
        let file = file.trim();
        let needle = needle.trim();
        return match tools::read_file(node_dir, file) {
            Ok(body) => {
                let pass = body.contains(needle);
                (pass, format!("read {file}: {pass}"))
            }
            Err(e) => (false, e.message()),
        };
    }
    match outcome {
        Some(outcome) => (outcome.pass, outcome.evidence.clone()),
        None => (false, "no evidence".into()),
    }
}

fn apply_tool(node_dir: &Path, call: &crate::model::ToolCall) -> Result<String, FabrError> {
    match call.name.as_str() {
        "read_file" => {
            let text = tools::read_file(node_dir, &call.path)?;
            Ok(format!("read_file {} ({} bytes)", call.path, text.len()))
        }
        "write_file" => {
            tools::write_file(node_dir, &call.path, call.content.as_deref().unwrap_or(""))?;
            Ok(format!("write_file {}", call.path))
        }
        "list_dir" => {
            let names = tools::list_dir(node_dir, &call.path)?;
            Ok(format!("list_dir {} ({})", call.path, names.len()))
        }
        other => Err(FabrError::BadRequest(format!("unknown tool {other}"))),
    }
}

fn log_has_write(log: &str, path: &str) -> bool {
    let needle = format!("execute: write_file {path}");
    log.lines().any(|line| line.contains(&needle))
}

fn other_running(node_dir: &Path, current: &Path) -> Result<bool, FabrError> {
    let current = task_path_key(current);
    Ok(list_records(node_dir)?.into_iter().any(|(path, rec)| rec.state == State::InProgress && task_path_key(&path) != current))
}

fn ignore_request(path: &Path, rec: &mut TaskRecord, expected: &str, key: &str) -> Result<String, FabrError> {
    let state = state_name(rec.state);
    append_log(&mut rec.log, "notify", &format!("ignored request {key} in state {state}"));
    match key {
        "cancel" => rec.requests.cancel = false,
        "delegate" => rec.requests.delegate = false,
        "redelegate" => rec.requests.redelegate = false,
        _ => {}
    }
    persist(path, rec, expected)
}

fn persist(path: &Path, rec: &mut TaskRecord, expected: &str) -> Result<String, FabrError> {
    update_record(path, rec, expected)?;
    Ok(rec.updated_at.clone())
}

fn project(ws: &Workspace, source: &str, node_path: &str, rec: &TaskRecord, status: Status) -> Result<(), FabrError> {
    let Some(todo_id) = rec.source.as_ref().map(|s| s.todo_id.clone()) else {
        log::warn!("record {} has no source; skipping todo projection", rec.id);
        return Ok(());
    };
    for attempt in 0..2 {
        let node = ws.get_node(NodeRef { source: source.to_string(), path: node_path.to_string() })?;
        let Some(version) = node.version.clone() else {
            return Err(FabrError::NotFound("node has no version".into()));
        };
        let mut items = node.items;
        let Some(item) = items.iter_mut().find(|i| i.id.as_deref() == Some(todo_id.as_str())) else {
            log::warn!("todo {todo_id} is missing from {node_path}; skipping projection");
            return Ok(());
        };
        if item.status == status {
            return Ok(());
        }
        item.status = status;
        match ws.save_todo(SaveTodoReq { source: source.to_string(), path: node_path.to_string(), version, items }) {
            Ok(_) => return Ok(()),
            Err(FabrError::Conflict { .. }) if attempt == 0 => continue,
            Err(e) => return Err(e),
        }
    }
    Err(FabrError::Conflict { message: "todo changed while projecting status".into(), node: None })
}

pub(crate) fn append_log(log: &mut String, step: &str, message: &str) {
    if !log.is_empty() && !log.ends_with('\n') {
        log.push('\n');
    }
    let _ = writeln!(log, "- {} {step}: {message}", now_utc());
}

fn state_name(state: State) -> &'static str {
    match state {
        State::Reminder => "reminder",
        State::Task => "task",
        State::Defined => "defined",
        State::InProgress => "in_progress",
        State::Succeeded => "succeeded",
        State::Failed => "failed",
    }
}

pub fn write_registry(
    fabric_space: &Path,
    node_dir: &Path,
    record_path: &Path,
    rec: &TaskRecord,
    heartbeat_interval_s: u64,
) -> Result<(), FabrError> {
    let task_path = task_path_key(record_path);
    let path = registry_path(fabric_space, &rec.id, &task_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = now_utc();
    let started = read_started(&path).unwrap_or_else(|| now.clone());
    let folder = node_dir.canonicalize().unwrap_or_else(|_| node_dir.to_path_buf());
    let step = rec.step.map(step_name);
    let mut body = String::new();
    let _ = writeln!(body, "task_path: {}", yaml_quote(&task_path));
    let _ = writeln!(body, "task_id: {}", yaml_quote(&rec.id));
    let _ = writeln!(body, "folder: {}", yaml_quote(&folder.to_string_lossy()));
    match step {
        Some(step) => {
            let _ = writeln!(body, "step: {step}");
        }
        None => body.push_str("step: null\n"),
    }
    let _ = writeln!(body, "started_at: {}", yaml_quote(&started));
    let _ = writeln!(body, "heartbeat_at: {}", yaml_quote(&now));
    let _ = writeln!(body, "heartbeat_interval_s: {heartbeat_interval_s}");
    let _ = writeln!(body, "cost_usd: {}", format_cost(rec.cost_usd));
    let tail = log_tail(&rec.log);
    if tail.is_empty() {
        body.push_str("log_tail: []\n");
    } else {
        body.push_str("log_tail:\n");
        for line in tail {
            let _ = writeln!(body, "  - {}", yaml_quote(&line));
        }
    }
    let _ = writeln!(body, "question: {}", question_yaml(rec));
    let _ = writeln!(body, "pid: {}", std::process::id());
    std::fs::write(&path, body).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))
}

fn question_yaml(rec: &TaskRecord) -> String {
    match &rec.question {
        None => "null".into(),
        Some(q) => serde_json::to_string(q).unwrap_or_else(|_| "null".into()),
    }
}

fn log_tail(log: &str) -> Vec<String> {
    let lines: Vec<String> = log.lines().filter(|l| !l.trim().is_empty()).map(|l| l.strip_prefix("- ").unwrap_or(l).to_string()).collect();
    let start = lines.len().saturating_sub(20);
    lines[start..].to_vec()
}

fn read_started(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let rest = line.strip_prefix("started_at:")?.trim().trim_matches('"');
        if !rest.is_empty() {
            return Some(rest.to_string());
        }
    }
    None
}

fn step_name(step: Step) -> &'static str {
    match step {
        Step::Gather => "gather",
        Step::Define => "define",
        Step::Plan => "plan",
        Step::Approve => "approve",
        Step::Execute => "execute",
        Step::Evaluate => "evaluate",
        Step::Learn => "learn",
        Step::Notify => "notify",
    }
}

fn format_cost(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") }
}

fn yaml_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
