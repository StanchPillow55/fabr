mod calls;
mod events;
mod heartbeat;
mod registry;
mod signal;

pub use calls::{CallLog, Sink, cost_usd};
pub use signal::install as install_signals;

use crate::lifecycle::{advance, remove_registry, task_path_key, write_registry};
use crate::model::AnthropicModel;
use crate::now_utc;
use chrono::Utc;
use events::Events;
use fabr_core::fabric::{self, FabricListReq};
use fabr_core::redact::{fingerprint, register_secret};
use fabr_core::setup::{Ctx, read_secret, settings_read};
use fabr_core::taskrec::{Grants, Links, Requests, State, Step, TaskRecord, parse, stamp_updated, write_record};
use fabr_core::{FabrError, Secret, Workspace};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const RETRY_AFTER_TICKS: u32 = 6;

#[derive(Debug, Clone)]
pub struct Options {
    pub interval_s: u64,
    pub once: bool,
}

pub struct Gate {
    pub default_folder: PathBuf,
    pub model: String,
    pub key: Secret,
}

pub fn start_gate(ctx: &Ctx) -> Result<Gate, FabrError> {
    let refuse = |m: &str| Err(FabrError::BadRequest(m.to_string()));
    let read = settings_read(ctx)?;
    if !read.exists {
        return refuse("Setup has not been started, so open the app and finish the setup screens first.");
    }
    if !read.settings.setup.completed {
        return refuse("Setup is not complete, so finish the setup screens in the app before starting the agent.");
    }
    let Some(key) = read_secret(ctx, "anthropic_api_key") else {
        return refuse("No Anthropic API key is saved, so enter it in the app's setup first.");
    };
    if read.settings.model.trim().is_empty() {
        return refuse("No model is selected, so pick one in the app's setup first.");
    }
    let folder = PathBuf::from(&read.settings.default_folder);
    if !folder.is_dir() {
        return Err(FabrError::BadRequest(format!(
            "The default folder {} does not exist, so pick another one in setup.",
            folder.display()
        )));
    }
    register_secret(&key, &fingerprint(&key));
    if let Some(token) = read_secret(ctx, "telegram_bot_token") {
        register_secret(&token, "[bot token]");
    }
    Ok(Gate { default_folder: folder, model: read.settings.model.trim().to_string(), key: Secret::new(key) })
}

pub struct Runner {
    space: PathBuf,
    root: PathBuf,
    ws: Workspace,
    model: AnthropicModel,
    calls: Arc<CallLog>,
    opts: Options,
    sink: Sink,
    events: Events,
    gate_model: String,
    fingerprint: String,
    beating: Option<heartbeat::Beating>,
    emitted: HashSet<String>,
    baselined: bool,
    backoff: HashMap<PathBuf, u32>,
    stuck_seen: HashSet<String>,
    bad_records: HashSet<PathBuf>,
}

impl Runner {
    pub fn new(ctx: &Ctx, ws: Workspace, opts: Options, sink: Sink) -> Result<Runner, FabrError> {
        let gate = start_gate(ctx)?;
        let fp = fingerprint(gate.key.expose());
        let calls = CallLog::new(sink.clone(), fp.clone());
        let model =
            AnthropicModel::new(gate.key.clone(), gate.model.clone()).with_base_url(&ctx.anthropic_url).with_observer(calls.observer());
        let space = ctx.space();
        Ok(Runner {
            events: Events::new(&space),
            space,
            root: gate.default_folder,
            ws,
            model,
            calls,
            opts,
            sink,
            gate_model: gate.model,
            fingerprint: fp,
            beating: None,
            emitted: HashSet::new(),
            baselined: false,
            backoff: HashMap::new(),
            stuck_seen: HashSet::new(),
            bad_records: HashSet::new(),
        })
    }

    pub fn start(&mut self) -> Result<(), FabrError> {
        self.sink.info(format!(
            "agent starting: default_folder={} model={} key={} interval={}s",
            self.root.display(),
            self.gate_model,
            self.fingerprint,
            self.opts.interval_s
        ));
        self.clean_stale();
        let hb = heartbeat::Heartbeat::new(&self.space, self.opts.interval_s.clamp(1, 10));
        self.beating = Some(heartbeat::Beating::spawn(hb)?);
        Ok(())
    }

    pub fn run(&mut self, stop: &AtomicBool) -> Result<(), FabrError> {
        self.start()?;
        loop {
            self.tick();
            if self.opts.once || stop.load(Ordering::SeqCst) {
                break;
            }
            let mut waited = Duration::ZERO;
            while waited < Duration::from_secs(self.opts.interval_s) && !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
                waited += Duration::from_millis(100);
            }
            if stop.load(Ordering::SeqCst) {
                break;
            }
        }
        self.shutdown(!self.opts.once);
        Ok(())
    }

    pub fn shutdown(&mut self, clean: bool) {
        if clean {
            let ours = std::process::id() as u64;
            for entry in registry::list(&self.space).into_iter().filter(|e| e.pid == ours) {
                let _ = std::fs::remove_file(&entry.path);
            }
        }
        if let Some(beating) = self.beating.take() {
            beating.stop(clean);
        }
        self.sink.info(if clean { "agent stopped" } else { "agent tick finished" });
    }

    fn clean_stale(&mut self) {
        let now = Utc::now();
        for entry in registry::list(&self.space) {
            if !entry.dead(now) {
                continue;
            }
            let age = entry.age_s(now).map_or("no heartbeat".to_string(), |a| format!("{a}s old"));
            self.sink.info(format!("removing stale registry entry {} ({age})", entry.path.display()));
            self.emit("stuck", &entry.task_path, &format!("agent stopped answering ({age})"));
            self.stuck_seen.insert(entry.task_path.clone());
            let _ = std::fs::remove_file(&entry.path);
        }
    }

    fn emit(&self, kind: &str, task_path: &str, detail: &str) {
        match self.events.append(kind, task_path, detail) {
            Ok(()) => self.sink.info(format!("event {kind} {task_path}")),
            Err(e) => self.sink.warn(format!("events.log append failed: {e}")),
        }
    }

    fn records(&self) -> Vec<PathBuf> {
        let req = FabricListReq { root: self.root.to_string_lossy().into_owned(), home: None };
        match fabric::list(&req) {
            Ok(res) => res.records.into_iter().map(|f| PathBuf::from(f.path)).collect(),
            Err(e) => {
                self.sink.warn(format!("cannot list records: {}", e.message()));
                Vec::new()
            }
        }
    }

    pub fn tick(&mut self) {
        let now = Utc::now();
        let ours = std::process::id() as u64;
        for entry in registry::list(&self.space) {
            if entry.pid == ours {
                let _ = registry::refresh(&entry.path);
            } else if entry.dead(now) && self.stuck_seen.insert(entry.task_path.clone()) {
                let age = entry.age_s(now).map_or("no heartbeat".to_string(), |a| format!("{a}s old"));
                self.emit("stuck", &entry.task_path, &format!("agent stopped answering ({age})"));
            }
        }
        let paths = self.records();
        if !self.baselined {
            self.baselined = true;
            for path in &paths {
                if let Some(rec) = std::fs::read_to_string(path).ok().and_then(|t| parse(&t).ok()) {
                    for (key, _, _) in event_keys(path, &rec) {
                        self.emitted.insert(key);
                    }
                }
            }
        }
        for path in paths {
            self.process(&path);
        }
    }

    fn load(&mut self, path: &Path) -> Option<TaskRecord> {
        match std::fs::read_to_string(path).map_err(|e| FabrError::Io(e.to_string())).and_then(|t| parse(&t)) {
            Ok(rec) => Some(rec),
            Err(e) => {
                if self.bad_records.insert(path.to_path_buf()) {
                    self.sink.warn(format!("skipping unreadable record {}: {}", path.display(), e.message()));
                }
                None
            }
        }
    }

    fn process(&mut self, path: &Path) {
        if let Some(left) = self.backoff.get_mut(path) {
            *left -= 1;
            if *left == 0 {
                self.backoff.remove(path);
            }
            return;
        }
        let Some(mut rec) = self.load(path) else { return };
        let node_dir = node_dir_of(path);
        let node_path = node_path_of(&self.root, &node_dir);

        let mut changed = false;
        if rec.requests.cancel {
            changed = true;
            let r = if rec.state == State::InProgress { self.cancel(path) } else { self.ignore(path, "cancel", rec.state) };
            self.note_err("cancel", path, r.err());
        }
        if rec.requests.redelegate {
            changed = true;
            if rec.state == State::Failed {
                let r = self.redelegate(path, &rec, &node_dir);
                self.note_err("redelegate", path, r.err());
            } else {
                let r = self.ignore(path, "redelegate", rec.state);
                self.note_err("redelegate", path, r.err());
            }
        }
        if rec.requests.delegate && rec.state != State::Reminder {
            changed = true;
            let r = self.ignore(path, "delegate", rec.state);
            self.note_err("delegate", path, r.err());
        }
        if changed {
            match self.load(path) {
                Some(r) => rec = r,
                None => return,
            }
        }
        let live = matches!(rec.state, State::Task | State::Defined | State::InProgress);
        if live && rec.question.as_ref().is_some_and(|q| q.answer.is_some()) {
            match fold_answer(path) {
                Ok(()) => self.sink.info(format!("answer received for {}; question moved to answers", rec.id)),
                Err(e) => {
                    self.sink.warn(format!("could not record the answer on {}: {}; will retry next tick", rec.id, e.message()));
                    return;
                }
            }
            match self.load(path) {
                Some(r) => rec = r,
                None => return,
            }
        }
        let tail = matches!(rec.step, Some(Step::Learn | Step::Notify));
        let drive_it = match rec.state {
            State::Reminder => rec.requests.delegate,
            State::Task | State::Defined | State::InProgress => true,
            State::Succeeded | State::Failed => tail,
        };
        if drive_it {
            self.calls.begin(&rec.id);
            let r = self.drive(path, &node_dir, &node_path);
            let cost = self.calls.take_cost();
            if cost > 0.0
                && let Err(e) = mutate(path, |r| r.cost_usd += cost)
            {
                self.sink.warn(format!("could not record cost on {}: {}", rec.id, e.message()));
            }
            if let Err(e) = r {
                self.sink.warn(format!(
                    "{} stopped at {}: {}; retrying in {RETRY_AFTER_TICKS} ticks",
                    rec.id,
                    step_text(&rec),
                    e.message()
                ));
                self.backoff.insert(path.to_path_buf(), RETRY_AFTER_TICKS);
            }
        }
        let Some(rec) = self.load(path) else { return };
        let key = task_path_key(path);
        if rec.state == State::InProgress {
            let live_entry = registry::list(&self.space).iter().any(|e| e.task_path == key);
            if !live_entry && let Err(e) = write_registry(&self.space, &node_dir, path, &rec, self.opts.interval_s) {
                self.sink.warn(format!("registry write failed for {}: {}", rec.id, e.message()));
            }
        } else {
            remove_registry(&self.space, &rec.id, &key);
        }
        for (k, kind, detail) in event_keys(path, &rec) {
            if self.emitted.insert(k) {
                self.emit(kind, &key, &detail);
            }
        }
    }

    fn note_err(&self, what: &str, path: &Path, err: Option<FabrError>) {
        if let Some(e) = err {
            self.sink.warn(format!("{what} on {} failed: {}; will retry next tick", path.display(), e.message()));
        }
    }

    fn drive(&self, path: &Path, node_dir: &Path, node_path: &str) -> Result<(), FabrError> {
        for _ in 0..16 {
            let before = std::fs::read_to_string(path).map_err(|e| FabrError::Io(e.to_string()))?;
            let rec = parse(&before)?;
            let terminal = matches!(rec.state, State::Succeeded | State::Failed);
            if terminal && !matches!(rec.step, Some(Step::Learn | Step::Notify)) {
                break;
            }
            if parked(&rec) {
                break;
            }
            advance(&self.ws, "local", node_path, node_dir, path, &self.space, self.opts.interval_s, &self.model)?;
            if std::fs::read_to_string(path).map_err(|e| FabrError::Io(e.to_string()))? == before {
                break;
            }
        }
        Ok(())
    }

    fn cancel(&self, path: &Path) -> Result<(), FabrError> {
        let rec = mutate(path, |r| {
            r.requests.cancel = false;
            r.state = State::Failed;
            r.step = None;
            r.waiting = false;
            r.verdict = None;
            append_log(&mut r.log, "execute", "cancelled by user");
        })?;
        remove_registry(&self.space, &rec.id, &task_path_key(path));
        self.sink.info(format!("cancelled {}", rec.id));
        Ok(())
    }

    fn ignore(&self, path: &Path, key: &str, state: State) -> Result<(), FabrError> {
        let msg = format!("ignored request {key} in state {}", state_text(state));
        mutate(path, |r| {
            match key {
                "cancel" => r.requests.cancel = false,
                "delegate" => r.requests.delegate = false,
                _ => r.requests.redelegate = false,
            }
            append_log(&mut r.log, "notify", &msg);
        })?;
        self.sink.info(format!("{}: {msg}", path.display()));
        Ok(())
    }

    fn redelegate(&mut self, path: &Path, old: &TaskRecord, node_dir: &Path) -> Result<(), FabrError> {
        let existing = old.links.redelegated_to.clone().filter(|p| Path::new(p).exists());
        let new_path = match existing {
            Some(p) => PathBuf::from(p),
            None => {
                let tasks = node_dir.join(".fabric-space").join("tasks");
                let id = free_id(&tasks, &old.title);
                let now = now_utc();
                let rec = TaskRecord {
                    schema: 2,
                    id: id.clone(),
                    source: old.source.clone(),
                    title: old.title.clone(),
                    due: old.due.clone(),
                    state: State::Reminder,
                    step: None,
                    waiting: false,
                    created_at: now.clone(),
                    updated_at: now,
                    requests: Requests { delegate: true, cancel: false, redelegate: false },
                    question: None,
                    answers: Vec::new(),
                    grants: Grants { requested: Vec::new(), granted: Vec::new() },
                    plan_meta: None,
                    assertions: Vec::new(),
                    approval: None,
                    verdict: None,
                    cost_usd: 0.0,
                    links: Links { redelegated_from: Some(task_path_key(path)), redelegated_to: None },
                    context: old.context.clone(),
                    spec: String::new(),
                    success: String::new(),
                    plan: String::new(),
                    log: String::new(),
                };
                let new_path = tasks.join(format!("{id}.md"));
                write_record(&new_path, &rec, None)?;
                new_path
            }
        };
        let to = task_path_key(&new_path);
        mutate(path, |r| {
            r.requests.redelegate = false;
            r.links.redelegated_to = Some(to.clone());
            append_log(&mut r.log, "notify", &format!("redelegated to {to}"));
        })?;
        self.sink.info(format!("redelegated {} to {}", old.id, new_path.display()));
        self.process(&new_path);
        Ok(())
    }
}

fn fold_answer(path: &Path) -> Result<(), FabrError> {
    mutate(path, |r| {
        if r.question.as_ref().is_some_and(|q| q.answer.is_some())
            && let Some(q) = r.question.take()
        {
            let id = q.id.clone();
            r.answers.push(q);
            r.waiting = r.state == State::Defined && r.approval.as_ref().is_some_and(|a| a.status == "pending");
            append_log(&mut r.log, "define", &format!("answered {id}"));
        }
    })
    .map(|_| ())
}

fn mutate(path: &Path, f: impl Fn(&mut TaskRecord)) -> Result<TaskRecord, FabrError> {
    let mut attempt = 0;
    loop {
        let mut rec = read_record(path)?;
        let expected = rec.updated_at.clone();
        f(&mut rec);
        stamp_updated(&mut rec, &now_utc());
        match write_record(path, &rec, Some(&expected)) {
            Ok(()) => return Ok(rec),
            Err(FabrError::Conflict { .. }) if attempt == 0 => attempt += 1,
            Err(e) => return Err(e),
        }
    }
}

fn read_record(path: &Path) -> Result<TaskRecord, FabrError> {
    parse(&std::fs::read_to_string(path).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))?)
}

fn append_log(log: &mut String, step: &str, message: &str) {
    if !log.is_empty() && !log.ends_with('\n') {
        log.push('\n');
    }
    let _ = writeln!(log, "- {} {step}: {message}", now_utc());
}

fn parked(rec: &TaskRecord) -> bool {
    if rec.waiting && rec.question.as_ref().is_some_and(|q| q.answer.is_none()) {
        return true;
    }
    rec.state == State::Defined && !matches!(rec.approval.as_ref().map(|a| a.status.as_str()), Some("approved" | "refused"))
}

fn node_dir_of(record: &Path) -> PathBuf {
    record.parent().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default()
}

fn node_path_of(root: &Path, node_dir: &Path) -> String {
    let rel = node_dir.strip_prefix(root).unwrap_or(Path::new(""));
    rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
}

fn free_id(tasks: &Path, title: &str) -> String {
    let base = format!("{}-{}", chrono::Local::now().format("%Y-%m-%d"), fabric::slugify(title));
    let mut id = base.clone();
    let mut n = 2;
    while tasks.join(format!("{id}.md")).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

fn state_text(state: State) -> &'static str {
    match state {
        State::Reminder => "reminder",
        State::Task => "task",
        State::Defined => "defined",
        State::InProgress => "in_progress",
        State::Succeeded => "succeeded",
        State::Failed => "failed",
    }
}

fn step_text(rec: &TaskRecord) -> String {
    format!("{} {}", state_text(rec.state), rec.step.map_or("-".to_string(), |s| format!("{s:?}").to_lowercase()))
}

fn event_keys(path: &Path, rec: &TaskRecord) -> Vec<(String, &'static str, String)> {
    let at = path.to_string_lossy();
    let mut out = Vec::new();
    if rec.waiting {
        if let Some(q) = rec.question.as_ref().filter(|q| q.answer.is_none()) {
            out.push((format!("{at}|waiting|{}|{}", q.id, q.asked_at.clone().unwrap_or_default()), "waiting", "one question".to_string()));
        } else if let Some(a) = rec.approval.as_ref().filter(|a| a.status == "pending") {
            out.push((
                format!("{at}|waiting|approval|{}", a.digest.clone().unwrap_or_default()),
                "waiting",
                "plan awaiting approval".to_string(),
            ));
        }
    }
    let verdict = |rec: &TaskRecord| {
        rec.verdict.as_ref().map(|v| format!("{} of {} assertions passed", v.items.iter().filter(|i| i.pass).count(), v.items.len()))
    };
    match rec.state {
        State::Succeeded => out.push((format!("{at}|done"), "done", verdict(rec).unwrap_or_else(|| "finished".into()))),
        State::Failed => {
            let detail = if rec.log.contains("cancelled by user") {
                "cancelled by user".to_string()
            } else {
                verdict(rec).unwrap_or_else(|| "failed".into())
            };
            out.push((format!("{at}|failed"), "failed", detail));
        }
        _ => {}
    }
    out
}
