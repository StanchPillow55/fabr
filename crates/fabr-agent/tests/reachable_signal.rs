//! Readiness separation: presence of the `_backend.yaml` heartbeat must not be
//! mistaken for the model being reachable.
//!
//! Requirement 2.1 writes the heartbeat on each Poll_Cycle once setup is
//! complete; Requirement 8.2 surfaces the model-reachable signal (and, on a
//! model error, the classified failure cause) separately through the `Sink`.
//! These tests assert that:
//!
//!   1. the heartbeat file is written at start *without* any "model reachable"
//!      line, and that line appears exactly once only after the first
//!      successful `ModelCall`; and
//!   2. a rejected key (HTTP 401) never emits "model reachable" and instead
//!      surfaces a terminal failure cause.
//!
//! This is a self-contained harness (it does not touch `tests/runner.rs`): a
//! minimal `Fake` Anthropic server, a completed-setup `Env`, and a `runner()`
//! helper that uses `Sink::capture()` to collect every INFO/WARN line.

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::Workspace;
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::setup::Ctx;
use fabr_core::taskrec::{self, State, TaskRecord};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const KEY: &str = "sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa";

/// Minimal fake Anthropic server. `status` defaults to 200; set it non-200 to
/// force every reply to be an error with a plausible body so the runner wraps
/// it as `FabrError::Remote("anthropic {code}: ...")`.
struct Fake {
    url: String,
    calls: Arc<AtomicUsize>,
    status: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Fake {
    fn new() -> Fake {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let status = Arc::new(AtomicUsize::new(200));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, s, st) = (calls.clone(), status.clone(), stop.clone());
        let handle = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                let Ok(Some(mut req)) = server.recv_timeout(std::time::Duration::from_millis(50)) else { continue };
                let mut body = String::new();
                std::io::Read::read_to_string(req.as_reader(), &mut body).unwrap();
                c.fetch_add(1, Ordering::SeqCst);
                let code = s.load(Ordering::SeqCst) as u16;
                let text = if code != 200 {
                    // Mirror a real rejected-key body so redaction and classification
                    // see the `anthropic {code}:` prefix the runner builds.
                    format!("bad key {KEY} in x-api-key: {KEY}")
                } else if body.contains("keys spec, success, assertions, question") {
                    // A clean `define` response with no question: lets the pipeline
                    // advance so a successful ModelCall is observed.
                    let inner = json!({
                        "spec": "write greet.txt",
                        "success": "greet.txt exists",
                        "assertions": [{"id": "a1", "text": "greet.txt exists"}],
                        "question": null
                    });
                    json!({"content": [{"type": "text", "text": inner.to_string()}], "usage": {"input_tokens": 120, "output_tokens": 45}})
                        .to_string()
                } else {
                    let inner = json!({
                        "spec": "write greet.txt",
                        "success": "greet.txt exists",
                        "plan": "1. write it",
                        "assertions": [{"id": "a1", "text": "greet.txt exists"}],
                        "plan_meta": {"reversibility": "reversible", "tools": ["write_file"], "checks": [{"cmd": "test", "args": ["-f", "greet.txt"]}]}
                    });
                    json!({"content": [{"type": "text", "text": inner.to_string()}], "usage": {"input_tokens": 300, "output_tokens": 80}})
                        .to_string()
                };
                let resp = tiny_http::Response::from_string(text)
                    .with_status_code(code)
                    .with_header(tiny_http::Header::from_bytes("content-type", "application/json").unwrap());
                let _ = req.respond(resp);
            }
        });
        Fake { url, calls, status, stop, handle: Some(handle) }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

struct Env {
    tmp: tempfile::TempDir,
    ctx: Ctx,
    proj: PathBuf,
}

/// A completed-setup environment pointed at the fake server.
fn ready(fake: &Fake) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let proj = home.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let space = home.join(".fabric-space");
    std::fs::create_dir_all(&space).unwrap();
    std::fs::write(
        space.join("settings.toml"),
        format!(
            "default_folder = \"{}\"\nmodel = \"claude-sonnet-5-5\"\nheartbeat_interval_s = 10\n\n[setup]\ncompleted_steps = [\"folder\", \"api_key\", \"model\"]\ncompleted = true\n",
            proj.display()
        ),
    )
    .unwrap();
    std::fs::write(space.join("secrets.toml"), format!("anthropic_api_key = \"{KEY}\"\n")).unwrap();
    let mut ctx = Ctx::new(&home);
    ctx.anthropic_url = fake.url.clone();
    Env { tmp, ctx, proj }
}

fn runner(e: &Env) -> (Runner, Arc<Mutex<Vec<String>>>) {
    let cfg = e.tmp.path().join("cfg");
    let cache = e.tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let ws = Workspace::from_config(&cfg, &cache, Some(e.proj.clone())).unwrap();
    let (sink, lines) = Sink::capture();
    (Runner::new(&e.ctx, ws, Options { interval_s: 5, once: false }, sink).unwrap(), lines)
}

fn new_record(e: &Env, title: &str) -> PathBuf {
    let req = FabricCreateReq {
        folder: e.proj.to_string_lossy().into_owned(),
        title: title.into(),
        due: Some("2026-11-01".into()),
        context: Some("some context".into()),
        spec: None,
        success: None,
    };
    PathBuf::from(fabric::create_in(&req, &e.ctx.home).unwrap().path)
}

fn load(p: &Path) -> TaskRecord {
    taskrec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn edit(p: &Path, f: impl FnOnce(&mut TaskRecord)) {
    let mut rec = load(p);
    let expected = rec.updated_at.clone();
    f(&mut rec);
    taskrec::write_record(p, &rec, Some(&expected)).unwrap();
}

fn snapshot(lines: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    lines.lock().unwrap().clone()
}

fn reachable_count(lines: &[String]) -> usize {
    lines.iter().filter(|l| l.starts_with("model reachable:")).count()
}

/// The `_backend.yaml` heartbeat proves only that a host is alive; it must not
/// carry a "model reachable" signal. Only the first successful `ModelCall`
/// surfaces reachability, and it does so exactly once.
#[test]
fn presence_alone_does_not_imply_reachable() {
    let fake = Fake::new();
    let e = ready(&fake);
    let (mut r, lines) = runner(&e);

    // Start the heartbeat thread but drive no pipeline work yet.
    r.start().unwrap();
    let heartbeat = e.ctx.space().join("registry/_backend.yaml");
    let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&heartbeat).unwrap()).unwrap();
    assert!(doc["heartbeat_at"].as_str().unwrap().ends_with('Z'), "heartbeat must be fresh");
    assert_eq!(doc["pid"].as_u64(), Some(std::process::id() as u64));

    // Presence is written, but reachability is not yet proven and no call made.
    let before = snapshot(&lines);
    assert_eq!(reachable_count(&before), 0, "no reachable signal before any model call: {before:?}");
    assert_eq!(fake.calls(), 0, "no model call should have happened yet");

    // Now drive a delegated record so a successful ModelCall occurs.
    let path = new_record(&e, "write a greeting");
    edit(&path, |rec| rec.requests.delegate = true);
    r.tick();

    let after = snapshot(&lines);
    assert!(fake.calls() >= 1, "the pipeline should have reached the model");
    assert_eq!(reachable_count(&after), 1, "exactly one reachable signal after success: {after:?}");
    let reachable = after.iter().find(|l| l.starts_with("model reachable:")).unwrap();
    assert!(
        reachable.contains("model=claude-sonnet-5-5") && reachable.contains("key=sk-ant-…x9Qa"),
        "reachable line names the model and redacted key: {reachable}"
    );

    // The record actually advanced off Reminder via a real (fake) model call.
    assert_ne!(load(&path).state, State::Reminder, "the delegated record advanced");

    // Draining more ticks never emits a second reachable signal.
    r.tick();
    r.tick();
    assert_eq!(reachable_count(&snapshot(&lines)), 1, "reachable signal is emitted at most once");

    r.shutdown(true);
    assert!(!heartbeat.exists(), "clean shutdown removes our own heartbeat");
}

/// A rejected key (HTTP 401) is terminal: the observer never fires, so no
/// "model reachable" line appears; instead the classified terminal cause is
/// surfaced through the `Sink`, carrying the 401 and never leaking the key.
#[test]
fn rejected_key_surfaces_terminal_cause_not_reachable() {
    let fake = Fake::new();
    fake.status.store(401, Ordering::SeqCst);
    let e = ready(&fake);
    let (mut r, lines) = runner(&e);
    r.start().unwrap();

    let path = new_record(&e, "will be rejected");
    edit(&path, |rec| rec.requests.delegate = true);
    r.tick();

    let out = snapshot(&lines);
    assert!(fake.calls() >= 1, "the pipeline attempted a model call");
    assert_eq!(reachable_count(&out), 0, "a rejected key never proves reachability: {out:?}");

    // The 401 is terminal: the failure is surfaced with its cause.
    let terminal = out
        .iter()
        .find(|l| l.contains("terminal failure, not retrying until corrected"))
        .unwrap_or_else(|| panic!("expected a terminal failure line: {out:?}"));
    assert!(terminal.contains("anthropic 401"), "terminal cause names the 401 status: {terminal}");

    // The delegate request was consumed (Reminder -> Task) before the define
    // call, but the terminal 401 stops advancement there: the record never
    // reaches a defined/in-progress/terminal state and records no verdict.
    let rec = load(&path);
    assert!(!rec.requests.delegate, "the delegate request was consumed");
    assert!(
        matches!(rec.state, State::Reminder | State::Task),
        "a terminal model error does not advance past the failing define: {:?}",
        rec.state
    );
    assert!(rec.verdict.is_none(), "no verdict recorded on a terminal failure");

    // The raw key is never leaked in any surfaced line.
    let all = out.join("\n");
    assert!(!all.contains(KEY) && !all.contains("FAKEFAKE"), "the key is redacted everywhere: {all}");

    r.shutdown(true);
}
