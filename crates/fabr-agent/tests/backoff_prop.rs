//! Feature: agent-runtime-wiring, Property 5: Backoff escalates then resets
//!
//! **Validates: Requirements 7.2, 7.3**
//!
//! Property 5 says that when consecutive pipeline invocations fail with a
//! transient (Backoff) error, the delay before each next invocation is
//! non-decreasing and bounded by a cap; and that the first successful
//! invocation resets the consecutive-error count and the delay back to the
//! base (one Heartbeat_Interval).
//!
//! This test drives the real `Runner` against a `Fake` Anthropic server. The
//! server returns a transient non-200 (`503`, which `classify` maps to
//! `Backoff` — never `401/403`, which are `Terminal`) for a run of errors,
//! then a `200`, then another run of errors after the record is made drivable
//! again. We tick the runner one Poll_Cycle at a time and observe, via
//! `fake.calls()` deltas, which ticks actually invoked the pipeline. The number
//! of skipped ticks between two successive invocations is exactly the backoff
//! delay the runner scheduled, so the observed gap sequence must:
//!   * be non-decreasing and capped during the error run (Requirement 7.2), and
//!   * fall back to the base after the success (Requirement 7.3).
//!
//! The harness here is deliberately self-contained (a minimal copy of the
//! `Fake`/`env`/`runner` helpers) so this file never touches `tests/runner.rs`.

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::Workspace;
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::setup::Ctx;
use fabr_core::taskrec::{self, Answer, TaskRecord};
use proptest::prelude::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const KEY: &str = "sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa";

/// The runner's backoff ladder, mirrored from `Runner::process`:
/// base = 1 tick, doubling per consecutive error, capped at 32 ticks.
const BASE_TICKS: u32 = 1;
const MAX_TICKS: u32 = 32;

/// Expected delay (in skipped ticks) scheduled after the `n`-th consecutive
/// transient error, matching `runner::backoff_delay`.
fn expected_delay(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let factor = 1u32.checked_shl((n - 1).min(31)).unwrap_or(u32::MAX);
    (BASE_TICKS.saturating_mul(factor)).min(MAX_TICKS)
}

/// A minimal fake Anthropic server whose HTTP status is switchable at runtime.
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
                let Ok(Some(mut req)) = server.recv_timeout(std::time::Duration::from_millis(20)) else { continue };
                let mut body = String::new();
                std::io::Read::read_to_string(req.as_reader(), &mut body).unwrap();
                c.fetch_add(1, Ordering::SeqCst);
                let code = s.load(Ordering::SeqCst) as u16;
                let text = if code != 200 {
                    // Body text is irrelevant to `classify`; the status code is
                    // what maps 503 -> Backoff.
                    "{\"error\":\"transient\"}".to_string()
                } else if body.contains("keys spec, success, assertions, question") {
                    // A successful `define`: return a question so the record
                    // parks (waiting) instead of running on to `plan`. One
                    // successful invocation, then the drive loop stops cleanly
                    // and `on_drive_success` fires (Requirement 7.3).
                    let inner = json!({
                        "spec": "", "success": "",
                        "assertions": [{"id": "a1", "text": "greet.txt exists"}],
                        "question": "Which filename?"
                    });
                    json!({"content": [{"type": "text", "text": inner.to_string()}], "usage": {"input_tokens": 1, "output_tokens": 1}})
                        .to_string()
                } else {
                    let inner = json!({"spec": "s", "success": "ok", "plan": "1.", "assertions": [{"id": "a1", "text": "x"}],
                        "plan_meta": {"reversibility": "reversible", "tools": ["write_file"], "checks": [{"cmd": "test", "args": ["-f", "greet.txt"]}]}});
                    json!({"content": [{"type": "text", "text": inner.to_string()}], "usage": {"input_tokens": 1, "output_tokens": 1}})
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

    fn set_status(&self, code: usize) {
        self.status.store(code, Ordering::SeqCst);
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
            "default_folder = \"{}\"\nmodel = \"claude-sonnet-5-5\"\nheartbeat_interval_s = 10\n\n[setup]\ncompleted_steps = [\"folder\", \"api_key\", \"model\"]\ncompleted = true\n",
            proj.display()
        ),
    )
    .unwrap();
    std::fs::write(space.join("secrets.toml"), format!("anthropic_api_key = \"{KEY}\"\n")).unwrap();
    let mut ctx = Ctx::new(&home);
    ctx.anthropic_url = url.to_string();
    Env { tmp, ctx, proj }
}

fn runner(e: &Env) -> Runner {
    let cfg = e.tmp.path().join("cfg");
    let cache = e.tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let ws = Workspace::from_config(&cfg, &cache, Some(e.proj.clone())).unwrap();
    // interval_s only affects the heartbeat thread's sleep; we drive `tick()`
    // directly, so a tick costs no wall-clock interval here.
    Runner::new(&e.ctx, ws, Options { interval_s: 5, once: false }, Sink::default()).unwrap()
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

/// Tick the runner until the pipeline is invoked once more (one `fake.calls()`
/// increment), returning the number of ticks that elapsed to get there.
///
/// Those ticks are: some number skipped by the backoff countdown, then the one
/// tick that actually invokes the pipeline. So the return value equals
/// `scheduled_backoff_delay + 1`. A safety bound prevents a runaway loop if the
/// record ever stops being drivable unexpectedly.
fn ticks_to_next_call(r: &mut Runner, fake: &Fake) -> u32 {
    let before = fake.calls();
    let mut ticks = 0u32;
    // Cap well above MAX_TICKS + 1 so a correct cap never trips it but a bug
    // (unbounded growth / never firing) fails fast instead of hanging.
    while ticks < MAX_TICKS + 50 {
        r.tick();
        ticks += 1;
        if fake.calls() > before {
            return ticks;
        }
    }
    panic!("pipeline was never invoked within {ticks} ticks (before={before}, now={})", fake.calls());
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, max_shrink_iters: 64, ..ProptestConfig::default() })]

    /// Property 5: during a transient-error run the per-invocation delay is
    /// non-decreasing and capped, and the first success resets it to the base.
    #[test]
    fn backoff_escalates_then_resets(run_a in 1u32..=8, run_b in 1u32..=6) {
        let fake = Fake::new();
        let e = env(&fake.url);
        let path = new_record(&e, "will fail then recover");
        edit(&path, |r| r.requests.delegate = true);

        let mut r = runner(&e);
        r.start().unwrap();

        // --- Error run A: `run_a` consecutive transient failures. ---
        //
        // We observe `run_a + 1` invocations. `ticks_to_next_call` returns the
        // number of ticks to reach each invocation: for the very first call
        // there is no preceding backoff (the error that schedules a delay has
        // not happened yet), so that gap is 1. Every later call is preceded by
        // the delay scheduled by the immediately prior error: the gap to the
        // invocation following error #n is `expected_delay(n) + 1` (the delay's
        // skipped ticks plus the one invoking tick).
        fake.set_status(503);
        let mut gaps_a = Vec::new();
        for _ in 0..=run_a {
            gaps_a.push(ticks_to_next_call(&mut r, &fake));
        }

        // First invocation: no prior error, hence no backoff.
        prop_assert_eq!(gaps_a[0], 1, "first invocation should have no preceding backoff");
        // The gap preceding invocation #(n+1) reflects the delay from error #n.
        for n in 1..=run_a {
            let gap = gaps_a[n as usize];
            prop_assert_eq!(
                gap,
                expected_delay(n) + 1,
                "error #{} scheduled gap {} != expected {}+1",
                n, gap, expected_delay(n)
            );
        }
        // The delay sequence (skip the no-backoff first entry) is non-decreasing
        // and bounded by the cap (+1 invoking tick) — Requirement 7.2.
        for w in gaps_a[1..].windows(2) {
            prop_assert!(w[1] >= w[0], "delay decreased: {} -> {}", w[0], w[1]);
        }
        for &gap in &gaps_a[1..] {
            prop_assert!(gap <= MAX_TICKS + 1, "delay {} exceeds cap {}", gap, MAX_TICKS + 1);
        }

        // --- Success: one 200 invocation resets the error run. ---
        fake.set_status(200);
        // The next invocation succeeds (define returns a question -> record
        // parks waiting, drive returns Ok, on_drive_success fires).
        let _ = ticks_to_next_call(&mut r, &fake);
        // Let one more tick settle so the parked record is observed stable.
        r.tick();
        let rec = load(&path);
        prop_assert!(rec.waiting, "record should be waiting on a question after the successful define");

        // Make the record drivable again by answering the question, so the
        // next error restarts a fresh run rather than parking.
        edit(&path, |rec| {
            if let Some(q) = rec.question.as_mut() {
                q.answer = Some(Answer {
                    value: Some("greet.txt".into()),
                    replacement: None,
                    granted: None,
                    by: Some("test".into()),
                    at: Some("2026-10-24T18:05:02Z".into()),
                });
            }
        });

        // --- Error run B: failures resume; the count must start at the base. ---
        //
        // Because the success reset the error run, the first invocation here is
        // again preceded by no backoff, and the delay scheduled by the first
        // error of run B is the *base* (`expected_delay(1)`), not a
        // continuation of run A's higher, escalated count — Requirement 7.3.
        fake.set_status(503);
        let mut gaps_b = Vec::new();
        for _ in 0..=run_b {
            gaps_b.push(ticks_to_next_call(&mut r, &fake));
        }

        prop_assert_eq!(gaps_b[0], 1, "first post-reset invocation should have no preceding backoff");
        // Reset-on-success: error #1 of run B schedules the base delay again.
        prop_assert_eq!(
            gaps_b[1],
            expected_delay(1) + 1,
            "delay did not reset to the base after success (got gap {})",
            gaps_b[1]
        );
        // And the full post-reset ladder escalates from the base once more.
        for n in 1..=run_b {
            let gap = gaps_b[n as usize];
            prop_assert_eq!(
                gap,
                expected_delay(n) + 1,
                "post-reset error #{} scheduled gap {} != expected {}+1",
                n, gap, expected_delay(n)
            );
        }

        r.shutdown(true);
    }
}
