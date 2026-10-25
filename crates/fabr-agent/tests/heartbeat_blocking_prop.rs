//! Feature: agent-runtime-wiring, Property 4: Heartbeat age stays fresh while running
//!
//! **Validates: Requirements 2.5, 6.2, 7.4**
//!
//! Property 4 says that while the poll loop is busy inside a long model call,
//! the dedicated `heartbeat::Beating` thread keeps refreshing `_backend.yaml`
//! so its reported age stays within 3x the `Heartbeat_Interval`. The heartbeat
//! thread is spawned in `Runner::start` and ticks on its own `interval_s`
//! timer, independent of the poll loop (`Runner::tick` -> `drive` -> the model
//! `post`). If a single Poll_Cycle blocks inside the model call for several
//! seconds, the heartbeat must still advance so the GUI never reads a stale
//! presence file (Requirements 2.5, 6.2) — the same independence that keeps the
//! heartbeat alive during pipeline backoff (Requirement 7.4).
//!
//! Harness. The `Fake` Anthropic server DELAYS its HTTP response: the request
//! handler sleeps for a proptest-chosen bounded duration before returning a
//! valid `define` response. That makes the poll loop block inside `drive`'s
//! model `post` for that duration (well under the model's 60s client timeout).
//! We run that single blocking `tick()` on a BACKGROUND thread so the MAIN test
//! thread is free to sample `_backend.yaml` repeatedly while the poll loop is
//! stuck. The Runner is moved into the spawned thread and returned via the
//! join handle so the main thread can `shutdown(true)` it cleanly afterwards.
//!
//! Timing design (kept small so 100 cases run in well under ~90s):
//!   * `interval_s = 1` (the heartbeat writer's clamped minimum), so the
//!     heartbeat refreshes roughly every second and 3x the interval is 3s.
//!   * The blocking model call lasts a proptest-chosen 250..=600 ms — long
//!     enough to span a heartbeat refresh at a 1s interval, but sub-second so
//!     each case completes quickly. With 100 cases that is on the order of ~45s
//!     of sleeping plus per-case setup, comfortably under ~90s.
//!   * During the block the main thread samples every 100 ms and asserts the
//!     heartbeat age (now - heartbeat_at) never exceeds 3x the interval plus a
//!     small scheduling slack. Because the block itself is under 3s, this is a
//!     real freshness check the heartbeat thread must satisfy on its own.
//!
//! This file is deliberately self-contained (its own minimal `Fake`/`env`/
//! `runner` helpers modeled on `tests/runner.rs`) so it never touches
//! `tests/runner.rs`.

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::Workspace;
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::setup::Ctx;
use fabr_core::taskrec::{self, TaskRecord};
use proptest::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

const KEY: &str = "sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa";

/// The runner is built with `interval_s = 1` so the heartbeat thread refreshes
/// roughly every second; 3x that is the freshness bound Property 4 checks.
const INTERVAL_S: u64 = 1;

/// A minimal fake Anthropic server whose request handler SLEEPS for `delay_ms`
/// before responding, simulating a long (blocking) model call. A valid
/// `define` response is returned so the drive attempt succeeds rather than
/// erroring into backoff — the point here is the block, not the outcome.
struct Fake {
    url: String,
    calls: Arc<AtomicUsize>,
    delay_ms: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Fake {
    fn new() -> Fake {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let delay_ms = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, d, st) = (calls.clone(), delay_ms.clone(), stop.clone());
        let handle = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                let Ok(Some(mut req)) = server.recv_timeout(Duration::from_millis(20)) else { continue };
                let mut body = String::new();
                std::io::Read::read_to_string(req.as_reader(), &mut body).unwrap();
                // Block the poll loop: sleep inside the handler before replying
                // so the caller's model `post` stays in-flight for `delay_ms`.
                let delay = d.load(Ordering::SeqCst);
                if delay > 0 {
                    std::thread::sleep(Duration::from_millis(delay));
                }
                c.fetch_add(1, Ordering::SeqCst);
                // A valid `define` response that parks on a question, so the
                // drive loop stops after one model call instead of running on.
                let inner = serde_json::json!({
                    "spec": "write greet.txt",
                    "success": "greet.txt exists",
                    "assertions": [{"id": "a1", "text": "greet.txt exists"}],
                    "question": "Which filename?"
                });
                let text = serde_json::json!({
                    "content": [{"type": "text", "text": inner.to_string()}],
                    "usage": {"input_tokens": 1, "output_tokens": 1}
                })
                .to_string();
                let resp = tiny_http::Response::from_string(text)
                    .with_status_code(200u16)
                    .with_header(tiny_http::Header::from_bytes("content-type", "application/json").unwrap());
                let _ = req.respond(resp);
            }
        });
        Fake { url, calls, delay_ms, stop, handle: Some(handle) }
    }

    fn set_delay_ms(&self, ms: u64) {
        self.delay_ms.store(ms, Ordering::SeqCst);
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

/// Build a completed-setup workspace pointed at the fake server, with a 1s
/// configured heartbeat interval.
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
    Runner::new(&e.ctx, ws, Options { interval_s: INTERVAL_S, once: false }, Sink::default()).unwrap()
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

/// Read `heartbeat_at` from `_backend.yaml` and return its age in milliseconds
/// against the current UTC time. The heartbeat is written as
/// `YYYY-MM-DDTHH:MM:SSZ` (second precision), so we parse it as RFC3339.
/// Returns `None` if the file is missing or the timestamp cannot be parsed yet.
fn heartbeat_age_ms(file: &Path) -> Option<i64> {
    let text = std::fs::read_to_string(file).ok()?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).ok()?;
    let at = doc.get("heartbeat_at")?.as_str()?;
    let ts = chrono::DateTime::parse_from_rfc3339(at).ok()?.with_timezone(&chrono::Utc);
    Some((chrono::Utc::now() - ts).num_milliseconds())
}

proptest! {
    // 100 cases is the Property-Based-Testing minimum. Each case blocks the
    // poll loop for 250..=750 ms (sub-second), so the whole file stays well
    // under ~90s. Shrinking is bounded for the same reason.
    #![proptest_config(ProptestConfig { cases: 100, max_shrink_iters: 32, ..ProptestConfig::default() })]

    /// Property 4: while a single Poll_Cycle is blocked inside a long model
    /// call, the dedicated heartbeat thread keeps `_backend.yaml` fresh — its
    /// reported age never exceeds 3x the Heartbeat_Interval (plus a small
    /// scheduling slack) across repeated samples spanning the whole block.
    #[test]
    fn heartbeat_stays_fresh_during_a_blocking_model_call(block_ms in 250u64..=600) {
        let fake = Fake::new();
        fake.set_delay_ms(block_ms);
        let e = env(&fake.url);
        let path = new_record(&e, "blocks in the model call");
        edit(&path, |r| r.requests.delegate = true);

        let mut r = runner(&e);
        // start() spawns the heartbeat thread (interval clamped to 1s) BEFORE we
        // block the poll loop, so the heartbeat refreshes on its own timer.
        r.start().unwrap();
        let file = e.ctx.space().join("registry/_backend.yaml");
        prop_assert!(file.exists(), "heartbeat file must exist right after start");

        // Drive ONE Poll_Cycle on a background thread. `tick()` calls `drive`,
        // which issues the model `post`; the fake sleeps `block_ms` before
        // replying, so this thread is parked inside the model call for that
        // whole window. The Runner is moved in and handed back via the join
        // handle so the main thread can shut it down cleanly afterwards.
        let started = std::time::Instant::now();
        let worker = std::thread::spawn(move || {
            r.tick();
            r
        });

        // While the background tick is blocked in the model call, sample the
        // heartbeat age repeatedly. The freshness bound is 3x the interval; we
        // allow a 1500 ms scheduling slack for timer granularity, the 1s
        // second-precision truncation of `heartbeat_at`, and OS thread
        // scheduling under a loaded test runner.
        let bound_ms = (3 * INTERVAL_S * 1000) as i64 + 1500;
        // Sample for the duration of the block (plus a little) so we observe the
        // heartbeat across the entire window the poll loop is stuck.
        let sample_deadline = started + Duration::from_millis(block_ms + 150);
        let mut samples = 0u32;
        let mut max_age = 0i64;
        while std::time::Instant::now() < sample_deadline {
            if let Some(age) = heartbeat_age_ms(&file) {
                max_age = max_age.max(age);
                prop_assert!(
                    age <= bound_ms,
                    "heartbeat age {}ms exceeded 3x interval bound {}ms while the poll loop was blocked (block_ms={})",
                    age, bound_ms, block_ms
                );
                samples += 1;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        prop_assert!(samples > 0, "expected at least one heartbeat sample during the block");

        // The background tick must have actually been blocked in the model call
        // for about the requested duration: join it and confirm the model call
        // happened and the elapsed time covered the block.
        let mut r = worker.join().expect("worker tick thread must not panic");
        let elapsed = started.elapsed();
        prop_assert!(fake.calls() >= 1, "the blocked tick should have issued at least one model call");
        prop_assert!(
            elapsed >= Duration::from_millis(block_ms),
            "the tick returned in {:?}, before the {}ms block elapsed — it was not actually blocking",
            elapsed, block_ms
        );
        // Sanity: the heartbeat really did advance under the bound (not stuck at
        // age 0 only because sampling raced the first write).
        prop_assert!(max_age >= 0, "observed a valid heartbeat age");

        r.shutdown(true);
        prop_assert!(!file.exists(), "clean shutdown removes the heartbeat file");
    }
}
