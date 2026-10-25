mod calls;
mod events;
pub(crate) mod heartbeat;
mod registry;
mod signal;

pub use calls::{CallLog, Sink, StepUsage, cost_usd};
pub(crate) use heartbeat::write_atomic;
pub use heartbeat::write_failure;
pub use signal::install as install_signals;

use crate::board::{Activity, Board, Extras};
use crate::lifecycle::{Ctl, advance_with, task_path_key, trailing_error};
use crate::model::AnthropicModel;
use crate::now_utc;
use crate::settings::check_timeout_s;
use crate::state::{append_log, clean_message, is_terminal, parked, pending_tail, stage_label, state_name};
use crate::store::mutate;
use chrono::Utc;
use events::Events;
use fabr_core::fabric::{self, FabricListReq};
use fabr_core::redact::{fingerprint, register_secret};
use fabr_core::setup::{Ctx, read_secret, settings_read};
use fabr_core::taskrec::{Grants, Links, Requests, State, TaskRecord, parse, write_record};
use fabr_core::{FabrError, Secret, Workspace};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const RETRY_AFTER_TICKS: u64 = 6;
const RETRY_CAP_S: u64 = 600;
const FRESH_EVENT_WINDOW_S: i64 = 24 * 3600;

#[derive(Debug, Clone)]
pub struct Options {
    pub interval_s: u64,
    pub once: bool,
}

pub struct Gate {
    pub default_folder: PathBuf,
    pub model: String,
    pub key: Secret,
    pub heartbeat_interval_s: u64,
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
    Ok(Gate {
        default_folder: folder,
        model: read.settings.model.trim().to_string(),
        key: Secret::new(key),
        heartbeat_interval_s: u64::from(read.settings.heartbeat_interval_s).clamp(1, 60),
    })
}

pub struct Runner {
    space: PathBuf,
    root: PathBuf,
    model: AnthropicModel,
    calls: Arc<CallLog>,
    board: Arc<Board>,
    ctl: Ctl,
    opts: Options,
    sink: Sink,
    events: Events,
    gate_model: String,
    fingerprint: String,
    hb_interval_s: u64,
    retry_base_s: u64,
    beating: Option<heartbeat::Beating>,
    emitted: HashSet<String>,
    seeded: bool,
    event_cutoff: Option<String>,
    stuck_seen: HashSet<String>,
    bad_records: HashSet<PathBuf>,
}

impl Runner {
    pub fn new(ctx: &Ctx, _ws: Workspace, opts: Options, sink: Sink) -> Result<Runner, FabrError> {
        let gate = start_gate(ctx)?;
        let fp = fingerprint(gate.key.expose());
        let calls = CallLog::new(sink.clone(), fp.clone());
        let model =
            AnthropicModel::new(gate.key.clone(), gate.model.clone()).with_base_url(&ctx.anthropic_url).with_observer(calls.observer());
        let space = ctx.space();
        let board = Board::new(&space, gate.heartbeat_interval_s, Some(calls.clone()));
        let ctl = Ctl::with_board(board.clone(), check_timeout_s(&space));
        Ok(Runner {
            events: Events::new(&space),
            space,
            root: gate.default_folder,
            model,
            calls,
            board,
            ctl,
            retry_base_s: opts.interval_s.max(1) * RETRY_AFTER_TICKS,
            opts,
            sink,
            gate_model: gate.model,
            fingerprint: fp,
            hb_interval_s: gate.heartbeat_interval_s,
            beating: None,
            emitted: HashSet::new(),
            seeded: false,
            event_cutoff: None,
            stuck_seen: HashSet::new(),
            bad_records: HashSet::new(),
        })
    }

    pub fn with_retry_base_s(mut self, seconds: u64) -> Runner {
        self.retry_base_s = seconds;
        self
    }

    pub fn with_model_retry_base(mut self, base: Duration) -> Runner {
        self.model = self.model.with_retry_base(base);
        self
    }

    pub fn with_check_timeout_s(mut self, seconds: u64) -> Runner {
        self.ctl.check_timeout_s = seconds;
        self
    }

    pub fn start(&mut self) -> Result<(), FabrError> {
        self.sink.info(format!(
            "agent starting: default_folder={} model={} key={} interval={}s heartbeat={}s",
            self.root.display(),
            self.gate_model,
            self.fingerprint,
            self.opts.interval_s,
            self.hb_interval_s
        ));
        self.event_cutoff = self.events.newest_at();
        self.clean_stale();
        let hb = heartbeat::Heartbeat::new(&self.space, self.hb_interval_s);
        self.beating = Some(heartbeat::Beating::spawn(hb, self.board.clone())?);
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
            self.board.clear();
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

    fn seed_events(&mut self, paths: &[PathBuf]) {
        let fresh = format!("{}Z", (Utc::now() - chrono::Duration::seconds(FRESH_EVENT_WINDOW_S)).format("%Y-%m-%dT%H:%M:%S"));
        let cutoff = self.event_cutoff.clone().unwrap_or(fresh);
        for path in paths {
            let Some(rec) = std::fs::read_to_string(path).ok().and_then(|t| parse(&t).ok()) else { continue };
            let task = task_path_key(path);
            for ev in event_keys(path, &rec) {
                if ev.at < cutoff || self.events.seen(ev.kind, &task, &ev.at) {
                    self.emitted.insert(ev.dedupe);
                }
            }
        }
    }

    pub fn tick(&mut self) {
        let now = Utc::now();
        let ours = std::process::id() as u64;
        for entry in registry::list(&self.space) {
            if entry.pid != ours && entry.dead(now) && self.stuck_seen.insert(entry.task_path.clone()) {
                let age = entry.age_s(now).map_or("no heartbeat".to_string(), |a| format!("{a}s old"));
                self.emit("stuck", &entry.task_path, &format!("agent stopped answering ({age})"));
            }
        }
        let paths = self.records();
        if !self.seeded {
            self.seeded = true;
            self.seed_events(&paths);
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

    fn retry_hold(&self, rec: &TaskRecord) -> Option<(u64, String)> {
        let err = trailing_error(&rec.log, stage_label(rec))?;
        let at = chrono::NaiveDateTime::parse_from_str(err.at.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S").ok()?.and_utc();
        let delay = self.retry_base_s.saturating_mul(u64::from(err.count)).min(RETRY_CAP_S) as i64;
        let left = (at + chrono::Duration::seconds(delay) - Utc::now()).num_seconds();
        (left > 0).then_some((left as u64, err.message))
    }

    fn sync(&self, path: &Path, node_dir: &Path, rec: &TaskRecord, extras: Option<Extras>) {
        let task = task_path_key(path);
        if rec.step.is_none() || (is_terminal(rec) && !pending_tail(rec)) {
            self.board.remove(&rec.id, &task);
            return;
        }
        let extras = extras.unwrap_or_else(|| {
            if parked(rec) {
                Extras::doing(Activity::Parked)
            } else if rec.state == State::Defined {
                Extras::doing(Activity::Queued)
            } else {
                Extras::doing(Activity::Tool)
            }
        });
        if let Err(e) = self.board.publish(node_dir, path, rec, extras) {
            self.sink.warn(format!("registry write failed for {}: {}", rec.id, e.message()));
        }
    }

    fn process(&mut self, path: &Path) {
        let Some(mut rec) = self.load(path) else { return };
        let node_dir = node_dir_of(path);

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
        let drive_it = match rec.state {
            State::Reminder => rec.requests.delegate,
            State::Task | State::Defined | State::InProgress => true,
            State::Succeeded | State::Failed => pending_tail(&rec),
        };
        let mut extras = None;
        if drive_it {
            if let Some((left, message)) = self.retry_hold(&rec) {
                extras = Some(Extras { activity: Some(Activity::Retry), retry_in_s: Some(left), last_error: Some(message) });
            } else {
                self.calls.begin(&rec.id);
                let r = self.drive(path, &node_dir);
                let cost = self.calls.take_cost();
                if cost > 0.0
                    && let Err(e) = mutate(path, |r| r.cost_usd += cost)
                {
                    self.sink.warn(format!("could not record cost on {}: {}", rec.id, e.message()));
                }
                if let Err(e) = r {
                    self.sink.warn(format!("{} stopped at {}: {}", rec.id, step_text(&rec), e.message()));
                    if let Some(now) = self.load(path)
                        && let Some(err) = trailing_error(&now.log, stage_label(&now))
                    {
                        let left = self.retry_base_s.saturating_mul(u64::from(err.count)).min(RETRY_CAP_S);
                        extras = Some(Extras {
                            activity: Some(Activity::Retry),
                            retry_in_s: Some(left),
                            last_error: Some(clean_message(&err.message, 300)),
                        });
                    }
                }
            }
        }
        let Some(rec) = self.load(path) else { return };
        self.sync(path, &node_dir, &rec, extras);
        let task = task_path_key(path);
        for ev in event_keys(path, &rec) {
            if self.emitted.insert(ev.dedupe) {
                self.emit(ev.kind, &task, &ev.detail);
            }
        }
    }

    fn note_err(&self, what: &str, path: &Path, err: Option<FabrError>) {
        if let Some(e) = err {
            self.sink.warn(format!("{what} on {} failed: {}; will retry next tick", path.display(), e.message()));
        }
    }

    fn drive(&self, path: &Path, node_dir: &Path) -> Result<(), FabrError> {
        for _ in 0..16 {
            let before = std::fs::read_to_string(path).map_err(|e| FabrError::Io(e.to_string()))?;
            let rec = parse(&before)?;
            if is_terminal(&rec) && !pending_tail(&rec) {
                break;
            }
            if parked(&rec) {
                break;
            }
            advance_with(node_dir, path, &self.ctl, &self.model)?;
            let after = std::fs::read_to_string(path).map_err(|e| FabrError::Io(e.to_string()))?;
            if after == before {
                break;
            }
            if let Ok(now) = parse(&after) {
                self.sync(path, node_dir, &now, None);
            }
        }
        Ok(())
    }

    fn cancel(&self, path: &Path) -> Result<(), FabrError> {
        let (rec, ()) = mutate(path, |r| {
            r.requests.cancel = false;
            r.state = State::Failed;
            r.step = None;
            r.waiting = false;
            r.verdict = None;
            append_log(&mut r.log, "execute", "cancelled by user");
        })?;
        self.board.remove(&rec.id, &task_path_key(path));
        self.sink.info(format!("cancelled {}", rec.id));
        Ok(())
    }

    fn ignore(&self, path: &Path, key: &str, state: State) -> Result<(), FabrError> {
        let msg = format!("ignored request {key} in state {}", state_name(state));
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
                    schema: fabr_core::taskrec::SCHEMA,
                    id: id.clone(),
                    source: old.source.clone(),
                    title: old.title.clone(),
                    due: old.due.clone(),
                    pri: old.pri,
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

fn node_dir_of(record: &Path) -> PathBuf {
    record.parent().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default()
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

fn step_text(rec: &TaskRecord) -> String {
    format!("{} {}", state_name(rec.state), rec.step.map_or("-".to_string(), |s| format!("{s:?}").to_lowercase()))
}

struct EventKey {
    dedupe: String,
    kind: &'static str,
    detail: String,
    at: String,
}

fn log_time(log: &str, labels: &[&str], needle: Option<&str>) -> Option<String> {
    log.lines().rev().find_map(|line| {
        let body = line.strip_prefix("- ")?;
        let (at, rest) = body.split_once(' ')?;
        let (label, msg) = rest.split_once(": ")?;
        (labels.contains(&label) && needle.is_none_or(|n| msg.contains(n))).then(|| at.to_string())
    })
}

fn event_keys(path: &Path, rec: &TaskRecord) -> Vec<EventKey> {
    let at = path.to_string_lossy();
    let mut out = Vec::new();
    if rec.waiting {
        if let Some(q) = rec.question.as_ref().filter(|q| q.answer.is_none()) {
            out.push(EventKey {
                dedupe: format!("{at}|waiting|{}|{}", q.id, q.asked_at.clone().unwrap_or_default()),
                kind: "waiting",
                detail: "one question".to_string(),
                at: q.asked_at.clone().unwrap_or_else(|| rec.updated_at.clone()),
            });
        } else if let Some(a) = rec.approval.as_ref().filter(|a| a.status == "pending") {
            out.push(EventKey {
                dedupe: format!("{at}|waiting|approval|{}", a.digest.clone().unwrap_or_default()),
                kind: "waiting",
                detail: "plan awaiting approval".to_string(),
                at: log_time(&rec.log, &["plan", "approve"], None).unwrap_or_else(|| rec.updated_at.clone()),
            });
        }
    }
    let verdict = |rec: &TaskRecord| {
        rec.verdict.as_ref().map(|v| format!("{} of {} assertions passed", v.items.iter().filter(|i| i.pass).count(), v.items.len()))
    };
    match rec.state {
        State::Succeeded => out.push(EventKey {
            dedupe: format!("{at}|done"),
            kind: "done",
            detail: verdict(rec).unwrap_or_else(|| "finished".into()),
            at: rec.verdict.as_ref().map(|v| v.at.clone()).unwrap_or_else(|| rec.updated_at.clone()),
        }),
        State::Failed => {
            let gave_up = rec.log.lines().rev().find_map(parse_failed_line);
            let detail = if rec.log.contains("cancelled by user") {
                "cancelled by user".to_string()
            } else if let Some((_, msg)) = &gave_up {
                msg.clone()
            } else {
                verdict(rec).unwrap_or_else(|| "failed".into())
            };
            let when = rec
                .verdict
                .as_ref()
                .map(|v| v.at.clone())
                .or_else(|| gave_up.map(|(at, _)| at))
                .or_else(|| log_time(&rec.log, &["execute"], Some("cancelled by user")))
                .unwrap_or_else(|| rec.updated_at.clone());
            out.push(EventKey { dedupe: format!("{at}|failed"), kind: "failed", detail, at: when });
        }
        _ => {}
    }
    out
}

fn parse_failed_line(line: &str) -> Option<(String, String)> {
    let body = line.strip_prefix("- ")?;
    let (at, rest) = body.split_once(' ')?;
    let (_, msg) = rest.split_once(": ")?;
    msg.starts_with("failed after ").then(|| (at.to_string(), clean_message(msg, 160)))
}
