use crate::board::{Activity, Board, Extras};
use crate::link::{list_records, load_record, update_record};
use crate::model::Model;
use crate::now_utc;
use crate::state::{append_log, clean_message, is_terminal, pending_tail, question_open, stage_label, state_name, suspended_now};
use crate::store::mutate;
use crate::tools;
use crate::verify::{self, CheckOutcome};
use fabr_core::FabrError;
use fabr_core::taskrec::{
    Approval, Assertion, Check, PlanMeta, Question, State, Step, TaskRecord, Verdict, VerdictItem, approval_digest, registry_filename,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub const MAX_STAGE_ERRORS: u32 = 3;
pub const SUPPORTED_TOOLS: [&str; 3] = ["read_file", "write_file", "list_dir"];
const TOOL_LINE_PREFIXES: [&str; 3] = ["read_file ", "write_file ", "list_dir "];

pub struct Ctl {
    pub board: Arc<Board>,
    pub check_timeout_s: u64,
}

impl Ctl {
    pub fn new(space: &Path, heartbeat_interval_s: u64) -> Ctl {
        Ctl { board: Board::new(space, heartbeat_interval_s, None), check_timeout_s: verify::DEFAULT_CHECK_TIMEOUT_S }
    }

    pub fn with_board(board: Arc<Board>, check_timeout_s: u64) -> Ctl {
        Ctl { board, check_timeout_s }
    }
}

pub fn approval_allows_execute(rec: &TaskRecord) -> Result<(), FabrError> {
    match check_approval(rec)? {
        None => Ok(()),
        Some(reason) => Err(FabrError::BadRequest(reason.into())),
    }
}

fn check_approval(rec: &TaskRecord) -> Result<Option<&'static str>, FabrError> {
    let Some(approval) = &rec.approval else {
        return Err(FabrError::BadRequest("approval is missing".into()));
    };
    if approval.status != "approved" {
        return Err(FabrError::BadRequest(format!("approval is {}", approval.status)));
    }
    if approval.by.as_deref() == Some("auto") && rec.plan_meta.as_ref().and_then(|m| m.reversibility.as_deref()) == Some("irreversible") {
        return Ok(Some("auto approval is not allowed for an irreversible plan"));
    }
    if let Some(meta) = &rec.plan_meta {
        validate_plan(meta)?;
    }
    if approval.digest.as_deref() != Some(approval_digest(rec).as_str()) {
        return Ok(Some("approval digest mismatch"));
    }
    Ok(None)
}

pub fn validate_plan(meta: &PlanMeta) -> Result<(), FabrError> {
    for tool in &meta.tools {
        if !SUPPORTED_TOOLS.contains(&tool.as_str()) {
            return Err(FabrError::BadRequest(format!("unsupported tool {tool}")));
        }
    }
    if meta.checks.is_empty() {
        return Err(FabrError::BadRequest("plan has no checks".into()));
    }
    for check in &meta.checks {
        verify::validate_check(check)?;
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

pub fn write_registry(
    fabric_space: &Path,
    node_dir: &Path,
    record_path: &Path,
    rec: &TaskRecord,
    heartbeat_interval_s: u64,
) -> Result<(), FabrError> {
    Board::new(fabric_space, heartbeat_interval_s, None).publish(node_dir, record_path, rec, Extras::default())
}

fn mark(ctl: &Ctl, node_dir: &Path, record_path: &Path, rec: &TaskRecord, activity: Activity) {
    if let Err(e) = ctl.board.publish(node_dir, record_path, rec, Extras::doing(activity)) {
        log::warn!("registry write failed for {}: {}", rec.id, e.message());
    }
}

fn forget(ctl: &Ctl, rec: &TaskRecord, record_path: &Path) {
    ctl.board.remove(&rec.id, &task_path_key(record_path));
}

pub struct ErrorLine {
    pub at: String,
    pub label: String,
    pub count: u32,
    pub message: String,
}

pub fn parse_error_line(line: &str) -> Option<ErrorLine> {
    let line = line.strip_prefix("- ").unwrap_or(line);
    let (at, rest) = line.split_once(' ')?;
    let (label, tail) = rest.split_once(": ")?;
    let tail = tail.strip_prefix("error ")?;
    let (count, message) = tail.split_once(": ")?;
    Some(ErrorLine { at: at.to_string(), label: label.to_string(), count: count.parse().ok()?, message: message.to_string() })
}

pub fn trailing_error(log: &str, label: &str) -> Option<ErrorLine> {
    for line in log.lines().rev().filter(|l| !l.trim().is_empty()) {
        if let Some(err) = parse_error_line(line) {
            return (err.label == label).then_some(err);
        }
        let body = line.strip_prefix("- ").unwrap_or(line);
        let is_tool_line = body
            .split_once(' ')
            .and_then(|(_, rest)| rest.strip_prefix(label))
            .and_then(|rest| rest.strip_prefix(": "))
            .is_some_and(|msg| TOOL_LINE_PREFIXES.iter().any(|p| msg.starts_with(p)));
        if !is_tool_line {
            return None;
        }
    }
    None
}

fn stage_error(record_path: &Path, ctl: &Ctl, err: FabrError) -> Result<(), FabrError> {
    let message = clean_message(&err.message(), 300);
    let outcome = mutate(record_path, |r| {
        let label = stage_label(r);
        let count = trailing_error(&r.log, label).map_or(0, |e| e.count) + 1;
        append_log(&mut r.log, label, &format!("error {count}: {message}"));
        if count < MAX_STAGE_ERRORS {
            return None;
        }
        r.state = State::Failed;
        r.step = None;
        r.waiting = false;
        r.verdict = None;
        r.requests.delegate = false;
        append_log(&mut r.log, label, &format!("failed after {count} errors: {message}"));
        Some(())
    });
    match outcome {
        Ok((rec, Some(()))) => {
            forget(ctl, &rec, record_path);
            Ok(())
        }
        Ok((_, None)) => Err(err),
        Err(write_err) => {
            log::warn!("could not log the stage error on {}: {}", record_path.display(), write_err.message());
            Err(err)
        }
    }
}

pub fn advance(
    node_dir: &Path,
    record_path: &Path,
    fabric_space: &Path,
    heartbeat_interval_s: u64,
    model: &impl Model,
) -> Result<(), FabrError> {
    let ctl = Ctl::new(fabric_space, heartbeat_interval_s);
    advance_with(node_dir, record_path, &ctl, model)
}

pub fn advance_with(node_dir: &Path, record_path: &Path, ctl: &Ctl, model: &impl Model) -> Result<(), FabrError> {
    let rec = load_record(record_path)?;
    match step_once(node_dir, record_path, ctl, model, rec) {
        Ok(()) => Ok(()),
        Err(err) => stage_error(record_path, ctl, err),
    }
}

fn step_once(node_dir: &Path, record_path: &Path, ctl: &Ctl, model: &impl Model, mut rec: TaskRecord) -> Result<(), FabrError> {
    let mut base = rec.clone();
    if is_terminal(&rec) && !pending_tail(&rec) {
        forget(ctl, &rec, record_path);
        return Ok(());
    }
    if rec.paused || suspended_now(&rec) {
        return Ok(());
    }
    if rec.requests.redelegate {
        let state = state_name(rec.state);
        append_log(&mut rec.log, "notify", &format!("ignored request redelegate in state {state}"));
        rec.requests.redelegate = false;
        return persist(record_path, &mut rec, &mut base);
    }
    match rec.state {
        State::Reminder => {
            if rec.requests.cancel {
                return ignore_request(record_path, &mut rec, &mut base, "cancel");
            }
            if !rec.requests.delegate {
                return Ok(());
            }
            rec.requests.delegate = false;
            rec.state = State::Task;
            rec.step = Some(Step::Gather);
            rec.waiting = false;
            append_log(&mut rec.log, "task", "delegated");
            persist(record_path, &mut rec, &mut base)
        }
        State::Task => match rec.step {
            Some(Step::Define) => define_stage(node_dir, record_path, ctl, &mut rec, &mut base, model),
            Some(Step::Plan) => plan_stage(node_dir, record_path, ctl, &mut rec, &mut base, model, false),
            _ => gather_stage(node_dir, record_path, ctl, &mut rec, &mut base),
        },
        State::Defined => {
            if rec.requests.cancel {
                return ignore_request(record_path, &mut rec, &mut base, "cancel");
            }
            let status = rec.approval.as_ref().map(|a| a.status.as_str()).unwrap_or("");
            if status == "refused" {
                let note = rec.approval.as_ref().and_then(|a| a.note.clone());
                match &note {
                    Some(n) => append_log(&mut rec.log, "approve", &format!("refused: {n}")),
                    None => append_log(&mut rec.log, "approve", "refused"),
                }
                return plan_stage(node_dir, record_path, ctl, &mut rec, &mut base, model, true);
            }
            if status != "approved" {
                return Ok(());
            }
            if let Some(reason) = check_approval(&rec)? {
                let digest = approval_digest(&rec);
                rec.approval = Some(Approval { status: "pending".into(), by: None, at: None, note: None, digest: Some(digest) });
                rec.waiting = true;
                append_log(&mut rec.log, "approve", &format!("{reason}, reset to pending"));
                return persist(record_path, &mut rec, &mut base);
            }
            if other_running(node_dir, record_path)? {
                if let Err(e) = ctl.board.publish(node_dir, record_path, &rec, Extras::doing(Activity::Queued)) {
                    log::warn!("registry write failed for {}: {}", rec.id, e.message());
                }
                log::info!("node already has an in_progress task");
                return Ok(());
            }
            rec.state = State::InProgress;
            rec.step = Some(Step::Execute);
            rec.waiting = false;
            append_log(&mut rec.log, "execute", "started");
            persist(record_path, &mut rec, &mut base)?;
            mark(ctl, node_dir, record_path, &rec, Activity::Tool);
            Ok(())
        }
        State::InProgress => match rec.step {
            Some(Step::Evaluate) => finish_checks(node_dir, record_path, ctl, &mut rec, &mut base),
            Some(Step::Learn | Step::Notify) => tail_step(node_dir, record_path, ctl, &mut rec, &mut base),
            _ => execute_stage(node_dir, record_path, ctl, &mut rec, &mut base, model),
        },
        State::Succeeded | State::Failed => match rec.step {
            Some(Step::Learn | Step::Notify) => tail_step(node_dir, record_path, ctl, &mut rec, &mut base),
            _ => {
                forget(ctl, &rec, record_path);
                Ok(())
            }
        },
    }
}

fn tail_step(node_dir: &Path, record_path: &Path, ctl: &Ctl, rec: &mut TaskRecord, base: &mut TaskRecord) -> Result<(), FabrError> {
    if rec.step == Some(Step::Learn) {
        rec.step = Some(Step::Notify);
        append_log(&mut rec.log, "learn", "deferred");
        persist(record_path, rec, base)?;
        mark(ctl, node_dir, record_path, rec, Activity::Tool);
        return Ok(());
    }
    let result = state_name(rec.state);
    append_log(&mut rec.log, "notify", &format!("result {result}"));
    rec.step = None;
    persist(record_path, rec, base)?;
    forget(ctl, rec, record_path);
    Ok(())
}

fn gather_stage(node_dir: &Path, record_path: &Path, ctl: &Ctl, rec: &mut TaskRecord, base: &mut TaskRecord) -> Result<(), FabrError> {
    let mut view = rec.clone();
    view.step = Some(Step::Gather);
    mark(ctl, node_dir, record_path, &view, Activity::Tool);
    append_gathered(&mut rec.context, &gather_bundle(node_dir));
    rec.step = Some(Step::Define);
    rec.waiting = false;
    append_log(&mut rec.log, "gather", "context bundle written");
    persist(record_path, rec, base)?;
    mark(ctl, node_dir, record_path, rec, Activity::Tool);
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

pub fn answers_text(rec: &TaskRecord) -> String {
    let mut out = String::new();
    let answered = rec.answers.iter().chain(rec.question.iter()).filter(|q| q.answer.is_some());
    for q in answered {
        out.push_str(&format!("\n\nAlready asked: {}\nUser answered: {}", q.text, reply_text(q)));
    }
    out
}

fn reply_text(q: &Question) -> String {
    let Some(a) = q.answer.as_ref() else { return String::new() };
    let mut parts = Vec::new();
    if let Some(value) = a.value.as_deref().filter(|v| !v.is_empty()) {
        parts.push(value.to_string());
    }
    if let Some(replacement) = a.replacement.as_deref().filter(|v| !v.is_empty()) {
        parts.push(format!("use instead: {replacement}"));
    }
    if let Some(granted) = a.granted.as_ref().filter(|g| !g.is_empty()) {
        let fields: Vec<String> = granted.iter().map(|g| format!("{} ({})", g.field, g.scope)).collect();
        parts.push(format!("granted {}", fields.join(", ")));
    }
    let mut reply = parts.join("; ");
    if q.kind == "choice"
        && let Some(prior) = &q.prior
    {
        reply.push_str(&format!(" (earlier choice: {})", prior.text));
    }
    reply
}

pub fn context_with_answers(rec: &TaskRecord) -> String {
    let mut out = rec.context.clone();
    out.push_str(&answers_text(rec));
    out
}

fn next_question_id(rec: &TaskRecord) -> String {
    let highest = rec
        .answers
        .iter()
        .chain(rec.question.iter())
        .filter_map(|q| q.id.strip_prefix('q').and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0);
    format!("q{}", highest + 1)
}

fn define_stage(
    node_dir: &Path,
    record_path: &Path,
    ctl: &Ctl,
    rec: &mut TaskRecord,
    base: &mut TaskRecord,
    model: &impl Model,
) -> Result<(), FabrError> {
    if rec.waiting && question_open(rec) {
        return Ok(());
    }
    if rec.question.as_ref().is_some_and(|q| q.answer.is_some())
        && let Some(q) = rec.question.take()
    {
        rec.answers.push(q);
    }
    mark(ctl, node_dir, record_path, rec, Activity::Model);
    let draft = model.define(&rec.title, &context_with_answers(rec))?;
    if let Some(text) = draft.question.filter(|q| !q.trim().is_empty()) {
        let id = next_question_id(rec);
        rec.question = Some(Question {
            id: id.clone(),
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
        append_log(&mut rec.log, "define", &format!("asked {id}"));
        persist(record_path, rec, base)?;
        mark(ctl, node_dir, record_path, rec, Activity::Model);
        return Ok(());
    }
    if draft.spec.trim().is_empty() || draft.success.trim().is_empty() || draft.assertions.is_empty() {
        return Err(FabrError::BadRequest("spec is incomplete".into()));
    }
    rec.spec = draft.spec;
    rec.success = draft.success;
    rec.assertions = draft.assertions.into_iter().map(|a| Assertion { id: a.id, text: a.text }).collect();
    let mut requested: Vec<String> = Vec::new();
    for field in draft.grants {
        if !requested.contains(&field) {
            requested.push(field);
        }
    }
    rec.grants.requested = requested;
    rec.question = None;
    rec.waiting = false;
    rec.step = Some(Step::Plan);
    append_log(&mut rec.log, "define", "spec written");
    persist(record_path, rec, base)?;
    mark(ctl, node_dir, record_path, rec, Activity::Model);
    Ok(())
}

fn plan_stage(
    node_dir: &Path,
    record_path: &Path,
    ctl: &Ctl,
    rec: &mut TaskRecord,
    base: &mut TaskRecord,
    model: &impl Model,
    refused: bool,
) -> Result<(), FabrError> {
    let note = if refused { rec.approval.as_ref().and_then(|a| a.note.clone()) } else { None };
    let mut view = rec.clone();
    view.step = Some(Step::Plan);
    mark(ctl, node_dir, record_path, &view, Activity::Model);
    let draft = model.plan(&rec.title, &rec.spec, &rec.success, note.as_deref(), &answers_text(rec))?;
    if draft.plan.trim().is_empty() || draft.checks.is_empty() {
        return Err(FabrError::BadRequest("plan is incomplete".into()));
    }
    let meta = PlanMeta {
        reversibility: Some(draft.reversibility),
        tools: draft.tools,
        assumptions: Vec::new(),
        checks: draft.checks.into_iter().map(|c| Check { cmd: c.cmd, args: c.args }).collect(),
    };
    validate_plan(&meta)?;
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
    rec.plan_meta = Some(meta);
    let digest = approval_digest(rec);
    rec.approval = Some(Approval { status: "pending".into(), by: None, at: None, note: None, digest: Some(digest) });
    rec.state = State::Defined;
    rec.step = Some(Step::Approve);
    rec.waiting = true;
    append_log(&mut rec.log, "plan", if refused { "redrafted" } else { "plan written" });
    persist(record_path, rec, base)?;
    mark(ctl, node_dir, record_path, rec, Activity::Model);
    Ok(())
}

fn execute_stage(
    node_dir: &Path,
    record_path: &Path,
    ctl: &Ctl,
    rec: &mut TaskRecord,
    base: &mut TaskRecord,
    model: &impl Model,
) -> Result<(), FabrError> {
    if rec.requests.cancel {
        rec.requests.cancel = false;
        rec.state = State::Failed;
        rec.step = None;
        rec.waiting = false;
        rec.verdict = None;
        append_log(&mut rec.log, "execute", "cancelled by user");
        persist(record_path, rec, base)?;
        forget(ctl, rec, record_path);
        return Ok(());
    }
    mark(ctl, node_dir, record_path, rec, Activity::Model);
    let calls = model.execute(&rec.title, &rec.spec, &rec.plan)?;
    for call in calls {
        if call.name == "write_file" && log_has_write(&rec.log, &call.path) {
            continue;
        }
        mark(ctl, node_dir, record_path, rec, Activity::Tool);
        let message = apply_tool(node_dir, &call)?;
        append_log(&mut rec.log, "execute", &message);
        persist(record_path, rec, base)?;
        mark(ctl, node_dir, record_path, rec, Activity::Tool);
        if rec.requests.cancel {
            return Ok(());
        }
    }
    rec.step = Some(Step::Evaluate);
    rec.verdict = None;
    append_log(&mut rec.log, "execute", "tools finished");
    persist(record_path, rec, base)?;
    mark(ctl, node_dir, record_path, rec, Activity::Check);
    Ok(())
}

fn finish_checks(node_dir: &Path, record_path: &Path, ctl: &Ctl, rec: &mut TaskRecord, base: &mut TaskRecord) -> Result<(), FabrError> {
    let checks = rec.plan_meta.as_ref().map(|m| m.checks.clone()).unwrap_or_default();
    mark(ctl, node_dir, record_path, rec, Activity::Check);
    let outcomes = verify::run_checks(node_dir, &checks, Duration::from_secs(ctl.check_timeout_s))?;
    let items = verdict_items(node_dir, rec, &outcomes);
    let checks_pass = !outcomes.is_empty() && outcomes.iter().all(|o| o.pass);
    let pass = checks_pass && items.iter().all(|item| item.pass);
    rec.verdict = Some(Verdict { result: if pass { "pass" } else { "fail" }.into(), at: now_utc(), items });
    rec.state = if pass { State::Succeeded } else { State::Failed };
    rec.step = Some(Step::Learn);
    rec.waiting = false;
    append_log(&mut rec.log, "evaluate", if pass { "verdict pass" } else { "verdict fail" });
    persist(record_path, rec, base)?;
    mark(ctl, node_dir, record_path, rec, Activity::Tool);
    Ok(())
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
    Ok(list_records(node_dir)?
        .into_iter()
        .any(|(path, rec)| rec.state == State::InProgress && !rec.paused && !suspended_now(&rec) && task_path_key(&path) != current))
}

fn ignore_request(path: &Path, rec: &mut TaskRecord, base: &mut TaskRecord, key: &str) -> Result<(), FabrError> {
    let state = state_name(rec.state);
    append_log(&mut rec.log, "notify", &format!("ignored request {key} in state {state}"));
    match key {
        "cancel" => rec.requests.cancel = false,
        "delegate" => rec.requests.delegate = false,
        "redelegate" => rec.requests.redelegate = false,
        _ => {}
    }
    persist(path, rec, base)
}

fn persist(path: &Path, rec: &mut TaskRecord, base: &mut TaskRecord) -> Result<(), FabrError> {
    update_record(path, rec, base)
}

#[cfg(test)]
mod answers_in_context {
    use super::*;
    use fabr_core::taskrec::{Answer, Grant, Prior};

    fn answered(id: &str, kind: &str, text: &str, answer: Answer) -> Question {
        Question {
            id: id.into(),
            kind: kind.into(),
            text: text.into(),
            options: None,
            prior: None,
            fields: None,
            asked_at: None,
            answer: Some(answer),
        }
    }

    fn answer(value: Option<&str>) -> Answer {
        Answer { value: value.map(Into::into), replacement: None, granted: None, by: Some("gui".into()), at: None }
    }

    #[test]
    fn choice_answers_carry_the_prior_and_grant_answers_carry_the_fields() {
        let mut rec = crate::link::test_reminder("order");
        rec.context = "base context".into();
        let mut choice = answered("q1", "choice", "Reuse the shop?", answer(Some("always")));
        choice.prior = Some(Prior { text: "shop.example.test".into(), source_task: None, last_confirmed: None });
        let mut no = answered("q2", "choice", "Reuse the size?", answer(Some("no")));
        no.answer.as_mut().unwrap().replacement = Some("2 kg".into());
        let mut grant = answered("q3", "grant", "May I fill the address?", answer(None));
        grant.answer.as_mut().unwrap().granted = Some(vec![Grant { field: "address.zip".into(), scope: "task".into() }]);
        rec.answers = vec![choice, no];
        rec.question = Some(grant);
        let ctx = context_with_answers(&rec);
        assert!(ctx.starts_with("base context"));
        assert!(ctx.contains("Already asked: Reuse the shop?\nUser answered: always (earlier choice: shop.example.test)"), "{ctx}");
        assert!(ctx.contains("User answered: no; use instead: 2 kg"), "{ctx}");
        assert!(ctx.contains("User answered: granted address.zip (task)"), "{ctx}");
    }

    #[test]
    fn question_ids_do_not_repeat() {
        let mut rec = crate::link::test_reminder("order");
        assert_eq!(next_question_id(&rec), "q1");
        rec.answers = vec![answered("q1", "open", "a?", answer(Some("x"))), answered("q2", "open", "b?", answer(Some("y")))];
        assert_eq!(next_question_id(&rec), "q3");
    }
}
