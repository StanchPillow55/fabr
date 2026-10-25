mod backoff;
mod calls;
mod events;
mod heartbeat;
mod registry;
mod signal;

pub use backoff::{Retry, classify};
pub use calls::{CallLog, Sink, cost_usd};
pub use signal::install as install_signals;

use crate::lifecycle::{advance, claim_registry, remove_registry, task_path_key};
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

/// Base backoff delay for a transient pipeline error, in Poll_Cycle ticks.
///
/// One tick is one `Heartbeat_Interval`, so the base delay equals the interval.
const BACKOFF_BASE_TICKS: u32 = 1;

/// Upper bound on the escalating backoff delay, in Poll_Cycle ticks.
///
/// Transient errors escalate `1, 2, 4, 8, ...` ticks and are clamped here so a
/// long run of failures never stalls a record for more than this many cycles.
const BACKOFF_MAX_TICKS: u32 = 32;

/// Escalating, bounded delay (in ticks) for the `n`-th consecutive error.
///
/// `n == 1` yields the base; each further consecutive error doubles the delay
/// up to [`BACKOFF_MAX_TICKS`]. Returns 0 for `n == 0` (no error run).
fn backoff_delay(consecutive_errors: u32) -> u32 {
    if consecutive_errors == 0 {
        return 0;
    }
    let shift = consecutive_errors - 1;
    // Saturate the doubling before multiplying to avoid overflow on long runs.
    let factor = 1u32.checked_shl(shift.min(31)).unwrap_or(u32::MAX);
    BACKOFF_BASE_TICKS.saturating_mul(factor).min(BACKOFF_MAX_TICKS)
}

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
    errors: HashMap<PathBuf, u32>,
    terminal_seen: HashMap<PathBuf, String>,
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
            errors: HashMap::new(),
            terminal_seen: HashMap::new(),
            stuck_seen: HashSet::new(),
            bad_records: HashSet::new(),
        })
    }

    pub fn start(&mut self) -> Result<(), FabrError> {
        // Log the ABSOLUTE default_folder so the startup line is diagnosable
        // regardless of the process's current working directory (Requirement
        // 8.1). `canonicalize` also resolves symlinks; fall back to a plain
        // absolute path, then the raw value, if the folder cannot be resolved.
        let root_abs = absolute_path(&self.root);
        self.sink.info(format!(
            "agent starting: default_folder={} model={} key={} interval={}s",
            root_abs.display(),
            self.gate_model,
            self.fingerprint,
            self.opts.interval_s
        ));
        // The worker writes presence (registry + heartbeat) to `self.space`,
        // which the GUI reads via the same `home`. If the two disagree (e.g.
        // `FABR_HOME` set differently for the GUI vs the worker), the GUI looks
        // for presence in a different directory than the worker writes and
        // reports "not connected" with no obvious cause. Make that loud rather
        // than silent (Requirement 8.1 diagnosability). This is a WARN, never a
        // panic: a mismatch is a misconfiguration, not a reason to crash.
        let env_space = Ctx::from_env().space();
        if self.space != env_space {
            self.sink.warn(format!(
                "fabric space mismatch: the worker writes presence to {} but Ctx::from_env() resolves {}; \
                 the GUI will read the latter, so it may report \"not connected\" — check FABR_HOME",
                self.space.display(),
                env_space.display()
            ));
        }
        self.clean_stale();
        self.warn_out_of_root();
        let hb = heartbeat::Heartbeat::new(&self.space, self.opts.interval_s.clamp(1, 10));
        self.beating = Some(heartbeat::Beating::spawn(hb)?);
        Ok(())
    }

    /// Warn once per record whose canonical path escapes the search root.
    ///
    /// `default_folder` is a search root: every discovered record is expected to
    /// live beneath it. A symlink or canonicalization surprise can place a
    /// record's real path outside the canonical root, which means the worker is
    /// driving a task the user did not expect under that folder. Surface each
    /// offending record once (deduped via `bad_records`) so the escape is
    /// diagnosable (Requirement 8.3). Comparisons use canonical paths on both
    /// sides; records that cannot be canonicalized are left for `load` to report.
    fn warn_out_of_root(&mut self) {
        let Ok(root) = self.root.canonicalize() else { return };
        for path in self.records() {
            let Ok(canon) = path.canonicalize() else { continue };
            if !canon.starts_with(&root) && self.bad_records.insert(path.clone()) {
                self.sink.warn(format!(
                    "record {} resolves to {}, outside the default folder {}; skipping would be safer — \
                     check for a symlink escaping the search root",
                    path.display(),
                    canon.display(),
                    root.display()
                ));
            }
        }
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
            match r {
                Ok(()) => self.on_drive_success(path),
                Err(e) => self.on_drive_error(path, &rec, &e),
            }
        }
        let Some(rec) = self.load(path) else { return };
        let key = task_path_key(path);
        if rec.state == State::InProgress {
            // Ensure a claim exists for an in-progress record. `claim_registry`
            // is the atomic acquisition: it creates the entry if absent, keeps
            // refreshing one we already own, takes over a stale one, and
            // declines (without touching it) when another live host owns it.
            match claim_registry(&self.space, &node_dir, path, &rec, self.opts.interval_s) {
                Ok(true) => {}
                Ok(false) => self.sink.info(format!("another process holds the active run for {}", rec.id)),
                Err(e) => self.sink.warn(format!("registry claim failed for {}: {}", rec.id, e.message())),
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

    /// Clear the error run for a record after a successful pipeline invocation.
    ///
    /// Resets the consecutive-error count and drops any pending backoff countdown
    /// so the next eligible cycle drives the record again at the base interval
    /// (Requirement 7.3). A later failure that re-surfaces a terminal cause is
    /// logged afresh, so the suppression entry is cleared too.
    fn on_drive_success(&mut self, path: &Path) {
        self.errors.remove(path);
        self.backoff.remove(path);
        self.terminal_seen.remove(path);
    }

    /// React to a failed pipeline invocation per the failure classification.
    ///
    /// Transient (`Backoff`): grow the consecutive-error count and schedule an
    /// escalating, bounded delay before the next invocation (Requirement 7.2).
    /// Terminal: stop retrying that cause — do not schedule a countdown — and
    /// surface it once per distinct cause, suppressing duplicates until the
    /// cause value changes (Requirements 7.5, 7.6, 8.3). The affected record's
    /// persisted state is left untouched either way (Requirement 7.5).
    fn on_drive_error(&mut self, path: &Path, rec: &TaskRecord, err: &FabrError) {
        match classify(err) {
            Retry::Backoff => {
                self.terminal_seen.remove(path);
                let n = self.errors.entry(path.to_path_buf()).or_insert(0);
                *n += 1;
                let delay = backoff_delay(*n);
                self.sink.warn(format!(
                    "{} stopped at {}: {}; retrying in {delay} tick(s) (error #{n})",
                    rec.id,
                    step_text(rec),
                    err.message()
                ));
                self.backoff.insert(path.to_path_buf(), delay);
            }
            Retry::Terminal => {
                // Stop retrying this cause: clear any pending countdown and error
                // run so a later, different cause starts fresh.
                self.backoff.remove(path);
                self.errors.remove(path);
                let cause = err.message().to_string();
                if self.terminal_seen.get(path) != Some(&cause) {
                    self.terminal_seen.insert(path.to_path_buf(), cause);
                    self.sink.warn(format!(
                        "{} stopped at {}: {}; terminal failure, not retrying until corrected",
                        rec.id,
                        step_text(rec),
                        err.message()
                    ));
                }
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

/// Resolve a path to an absolute form for logging.
///
/// Prefer `canonicalize` (absolute + symlinks resolved); if that fails (e.g.
/// the path does not exist yet), fall back to `std::path::absolute` which makes
/// a relative path absolute against the current directory without touching the
/// filesystem; if even that fails, return the path unchanged so logging never
/// loses information.
fn absolute_path(path: &Path) -> PathBuf {
    path.canonicalize().or_else(|_| std::path::absolute(path)).unwrap_or_else(|_| path.to_path_buf())
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
