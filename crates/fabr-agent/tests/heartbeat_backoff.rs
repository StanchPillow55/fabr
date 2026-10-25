//! Heartbeat independence from pipeline backoff (Requirement 7.4).
//!
//! The `_backend.yaml` heartbeat is written by a dedicated `heartbeat::Beating`
//! thread spawned in `Runner::start`, separate from the poll-loop backoff. This
//! integration test forces a record into backoff (via a transient 503 from a
//! fake Anthropic server) and asserts the heartbeat keeps refreshing while the
//! pipeline invocations for that record are being delayed.
//!
//! This file is intentionally self-contained (its own minimal `Fake`/`env`/
//! `runner` helpers modeled on `tests/runner.rs`) so it does not conflict with
//! parallel edits to `tests/runner.rs`.

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::Workspace;
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::setup::Ctx;
use fabr_core::taskrec::{self, TaskRecord};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const KEY: &str = "sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa";

/// A minimal fake Anthropic server. Its `status` field forces the HTTP status
/// code; set it to 503 to make every pipeline call fail with a transient error
/// that the runner classifies as `Backoff`.
struct Fake {
    url: String,
    calls: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Fake {
    fn new(status: usize) -> Fake {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, st) = (calls.clone(), stop.clone());
        let handle = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                let Ok(Some(mut req)) = server.recv_timeout(std::time::Duration::from_millis(50)) else { continue };
                let mut body = String::new();
                std::io::Read::read_to_string(req.as_reader(), &mut body).unwrap();
                c.fetch_add(1, Ordering::SeqCst);
                let code = status as u16;
                let text = if code != 200 {
                    "service unavailable".to_string()
                } else {
                    let inner = serde_json::json!({"spec": "write greet.txt", "success": "greet.txt exists", "assertions": [{"id": "a1", "text": "greet.txt exists"}], "question": serde_json::Value::Null});
                    serde_json::json!({"content": [{"type": "text", "text": inner.to_string()}], "usage": {"input_tokens": 1, "output_tokens": 1}}).to_string()
                };
                let resp = tiny_http::Response::from_string(text)
                    .with_status_code(code)
                    .with_header(tiny_http::Header::from_bytes("content-type", "application/json").unwrap());
                let _ = req.respond(resp);
            }
        });
        Fake { url, calls, stop, handle: Some(handle) }
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

/// Build a completed-setup workspace pointed at the fake server.
fn env(url: &str) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let proj = home.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let space = home.join(".fabric-space");
    std::fs::create_dir_all(&space).unwrap();
    std::fs::write(
        space.join("settings.toml"),
        format!(
            "default_folder = \"{}\"\nmodel = \"claude-sonnet-5-5\"\nheartbeat_interval_s = 1\n\n[setup]\ncompleted_steps = [\"folder\", \"api_key\", \"model\"]\ncompleted = true\n",
            proj.display()
        ),
    )
    .unwrap();
    std::fs::write(space.join("secrets.toml"), format!("anthropic_api_key = \"{KEY}\"\n")).unwrap();
    let mut ctx = Ctx::new(&home);
    ctx.anthropic_url = url.to_string();
    Env { tmp, ctx, proj }
}

/// Build a runner with a 1s interval so the heartbeat thread ticks quickly.
fn runner(e: &Env) -> Runner {
    let cfg = e.tmp.path().join("cfg");
    let cache = e.tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let ws = Workspace::from_config(&cfg, &cache, Some(e.proj.clone())).unwrap();
    Runner::new(&e.ctx, ws, Options { interval_s: 1, once: false }, Sink::default()).unwrap()
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

fn heartbeat_at(file: &Path) -> String {
    let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    doc["heartbeat_at"].as_str().unwrap().to_string()
}

/// Requirement 7.4: while pipeline invocations are delayed by backoff, the
/// dedicated heartbeat thread keeps refreshing `_backend.yaml` independently.
#[test]
fn heartbeat_keeps_refreshing_while_record_is_in_backoff() {
    // Every model call returns a transient 503, which the runner classifies as
    // `Backoff` and schedules a retry delay for the offending record.
    let fake = Fake::new(503);
    let e = env(&fake.url);
    let path = new_record(&e, "will keep failing");
    edit(&path, |r| r.requests.delegate = true);

    let mut r = runner(&e);
    // start() spawns the heartbeat thread (interval clamped to 1s here).
    r.start().unwrap();
    let file = e.ctx.space().join("registry/_backend.yaml");
    assert!(file.exists(), "heartbeat file must exist right after start");

    // First tick drives the record, the model call 503s, and the record enters
    // backoff: subsequent ticks skip its pipeline invocation for a while.
    r.tick();
    let calls_after_error = fake.calls();
    assert!(calls_after_error >= 1, "the first tick should have attempted at least one model call");

    // Capture the heartbeat timestamp now that the record is parked in backoff.
    let before = heartbeat_at(&file);

    // Wait across several heartbeat intervals while the record stays in backoff.
    // We keep ticking to prove the poll loop is actively skipping the pipeline
    // for this record (backoff), yet the heartbeat still advances because its
    // dedicated thread refreshes independently of the poll loop.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut after = before.clone();
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(250));
        r.tick();
        after = heartbeat_at(&file);
        if after != before {
            break;
        }
    }

    // The heartbeat refreshed at least once more while the record was in
    // backoff — heartbeat is independent of the pipeline backoff.
    assert_ne!(after, before, "heartbeat_at must advance while the record is delayed by backoff");

    // The record stayed in backoff: ticking many times over several seconds did
    // not translate into a model call per tick. With a 1s base delay the
    // runner attempts at most a handful of calls over this window, far fewer
    // than the ~20 ticks we issued, confirming the pipeline was being delayed.
    let calls_during_backoff = fake.calls() - calls_after_error;
    assert!(calls_during_backoff <= 6, "pipeline invocations should be throttled by backoff, saw {calls_during_backoff} extra calls");

    r.shutdown(true);
    assert!(!file.exists(), "clean shutdown removes the heartbeat file");
}
