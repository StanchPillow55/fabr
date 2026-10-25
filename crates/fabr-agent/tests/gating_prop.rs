//! Feature: agent-runtime-wiring, Property 6: Gating withholds all side effects.
//!
//! Validates Requirements 3.2, 3.3, 3.5, 3.6, 3.7:
//! - While setup is incomplete the worker writes no `_backend.yaml`, invokes no
//!   pipeline, yet the Data_API keeps answering.
//! - When setup transitions false -> true in the SAME process, heartbeat and
//!   pipeline both begin on the next cycle with no restart.
//!
//! The runner gates inside `Runner::new` (it calls `start_gate`), so at the
//! library level "the worker stays idle while incomplete" is proven at the
//! gate + filesystem: `start_gate(&ctx).is_err()` AND no `_backend.yaml` is
//! ever written AND `fabr_core::ops::dispatch` still serves reads. Completing
//! setup in-process (rewriting `settings.toml`/`secrets.toml`, no new process)
//! then lets `start_gate` pass, the `Runner` build, and a single tick drive the
//! pipeline — mirroring the desktop host's retry loop.

use fabr_agent::runner::{Options, Runner, Sink, start_gate};
use fabr_core::Workspace;
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::setup::Ctx;
use proptest::prelude::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const KEY: &str = "sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa";
const MODEL: &str = "claude-sonnet-5-5";

/// A minimal Anthropic stand-in that always answers 200 with a plan so a driven
/// record advances. We only ever assert that it was *called* (or not), so the
/// body shape just has to parse through the pipeline's first step.
struct Fake {
    url: String,
    calls: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Fake {
    fn new() -> Fake {
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
                let inner = if body.contains("keys spec, success, assertions, question") {
                    json!({"spec": "write greet.txt", "success": "greet.txt exists", "assertions": [{"id": "a1", "text": "greet.txt exists"}], "question": null})
                } else {
                    json!({"spec": "write greet.txt", "success": "greet.txt exists", "plan": "1. write it", "assertions": [{"id": "a1", "text": "greet.txt exists"}],
                        "plan_meta": {"reversibility": "reversible", "tools": ["write_file"], "checks": [{"cmd": "test", "args": ["-f", "greet.txt"]}]}})
                };
                let text =
                    json!({"content": [{"type": "text", "text": inner.to_string()}], "usage": {"input_tokens": 120, "output_tokens": 45}})
                        .to_string();
                let resp = tiny_http::Response::from_string(text)
                    .with_status_code(200u16)
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

/// Build a fresh home with a settings/secrets pair. `completed`, `key`, `model`
/// and whether the folder exists are all independently controllable so a
/// property can vary exactly which part of setup is missing.
fn env(url: &str, completed: bool, key: Option<&str>, model: &str, folder_exists: bool) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let proj = home.join("proj");
    if folder_exists {
        std::fs::create_dir_all(&proj).unwrap();
    } else {
        // Keep home around (so settings can be read) but leave proj absent.
        std::fs::create_dir_all(&home).unwrap();
    }
    let space = home.join(".fabric-space");
    std::fs::create_dir_all(&space).unwrap();
    write_settings(&space, &proj, completed, model);
    std::fs::write(space.join("secrets.toml"), key.map(|k| format!("anthropic_api_key = \"{k}\"\n")).unwrap_or_default()).unwrap();
    let mut ctx = Ctx::new(&home);
    ctx.anthropic_url = url.to_string();
    Env { tmp, ctx, proj }
}

fn write_settings(space: &Path, proj: &Path, completed: bool, model: &str) {
    std::fs::write(
        space.join("settings.toml"),
        format!(
            "default_folder = \"{}\"\nmodel = \"{model}\"\nheartbeat_interval_s = 10\n\n[setup]\ncompleted_steps = [\"folder\", \"api_key\", \"model\"]\ncompleted = {completed}\n",
            proj.display()
        ),
    )
    .unwrap();
}

fn workspace(e: &Env) -> Workspace {
    let cfg = e.tmp.path().join("cfg");
    let cache = e.tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    Workspace::from_config(&cfg, &cache, Some(e.proj.clone())).unwrap()
}

fn backend_yaml(e: &Env) -> PathBuf {
    e.ctx.space().join("registry").join("_backend.yaml")
}

/// The Data_API: dispatch a read op through the same path the desktop `call`
/// command uses. `get_tree` reads the configured sources, which exist whether
/// or not agent setup is complete, so it must answer in both states.
fn data_api_ok(ws: &Workspace) {
    let got = fabr_core::ops::dispatch(ws, "get_tree", json!({ "show_all": true }));
    assert!(got.is_ok(), "Data_API get_tree must answer while gated: {got:?}");
    let listed = fabr_core::ops::dispatch(ws, "fabric_list", json!({ "root": "" }));
    assert!(listed.is_ok(), "Data_API fabric_list must answer while gated: {listed:?}");
}

/// Assert the full withholding contract for an incomplete-setup variant: the
/// gate refuses, no heartbeat was written, and the Data_API still serves.
fn assert_withheld(e: &Env, ws: &Workspace) {
    assert!(start_gate(&e.ctx).is_err(), "start_gate must refuse while setup is incomplete");
    // No pipeline could even be constructed: building the Runner fails for the
    // same reason, so no pipeline side effects occur this cycle (Req 3.3).
    let build = Runner::new(&e.ctx, workspace(e), Options { interval_s: 5, once: true }, Sink::default());
    assert!(build.is_err(), "Runner::new must refuse while setup is incomplete");
    // No Backend_Heartbeat on disk (Req 3.2).
    assert!(!backend_yaml(e).exists(), "no _backend.yaml may exist while setup is incomplete");
    // Data_API continuity (Req 3.7).
    data_api_ok(ws);
}

fn new_delegated_record(e: &Env, title: &str) -> PathBuf {
    let req = FabricCreateReq {
        folder: e.proj.to_string_lossy().into_owned(),
        title: title.into(),
        due: Some("2026-11-01".into()),
        context: Some("some context".into()),
        spec: None,
        success: None,
    };
    let path = PathBuf::from(fabric::create_in(&req, &e.ctx.home).unwrap().path);
    // Flip the delegate request so the first tick drives the pipeline.
    let text = std::fs::read_to_string(&path).unwrap();
    let mut rec = fabr_core::taskrec::parse(&text).unwrap();
    let expected = rec.updated_at.clone();
    rec.requests.delegate = true;
    fabr_core::taskrec::write_record(&path, &rec, Some(&expected)).unwrap();
    path
}

/// Which single part of setup is missing in an incomplete variant.
#[derive(Debug, Clone, Copy)]
enum Missing {
    SetupFlag,
    Key,
    Model,
    Folder,
}

fn missing_strategy() -> impl Strategy<Value = Missing> {
    prop_oneof![Just(Missing::SetupFlag), Just(Missing::Key), Just(Missing::Model), Just(Missing::Folder)]
}

fn env_for(url: &str, missing: Missing) -> Env {
    match missing {
        Missing::SetupFlag => env(url, false, Some(KEY), MODEL, true),
        Missing::Key => env(url, true, None, MODEL, true),
        Missing::Model => env(url, true, Some(KEY), "", true),
        Missing::Folder => env(url, true, Some(KEY), MODEL, false),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// Property 6 (withholding half): for every way setup can be incomplete,
    /// the worker writes no heartbeat, invokes no pipeline, and the Data_API
    /// keeps answering. An unreachable Anthropic URL guarantees that any pipeline
    /// side effect would be observable as a networking attempt — but none occurs
    /// because the gate refuses before construction.
    #[test]
    fn incomplete_setup_withholds_all_side_effects(missing in missing_strategy()) {
        let e = env_for("http://127.0.0.1:1", missing);
        let ws = workspace(&e);
        assert_withheld(&e, &ws);
    }
}

/// Property 6 (transition half): completing setup in the SAME process lets the
/// gate pass, the Runner build, and one tick begin both the heartbeat and the
/// pipeline — with no process restart. This is the deterministic companion to
/// the property above; it needs a live `Fake` so the driven record can advance.
#[test]
fn completing_setup_begins_heartbeat_and_pipeline_without_restart() {
    let fake = Fake::new();
    // Start incomplete: the setup flag is false.
    let e = env(&fake.url, false, Some(KEY), MODEL, true);
    let ws_gated = workspace(&e);

    // While incomplete: nothing is written, the pipeline is not invoked, and
    // the Data_API answers. Create the delegated record now so it is already
    // present when setup completes — proving the record was NOT driven while
    // gated (the Fake records zero calls).
    let path = new_delegated_record(&e, "write a greeting");
    assert_withheld(&e, &ws_gated);
    assert_eq!(fake.calls(), 0, "no model call may happen while setup is incomplete");

    // Flip setup to complete IN PLACE — same process, same files, no restart.
    write_settings(&e.ctx.space(), &e.proj, true, MODEL);
    assert!(start_gate(&e.ctx).is_ok(), "start_gate must pass once setup is complete");

    // The next cycle: build the Runner and tick once.
    let ws = workspace(&e);
    let (sink, _lines) = Sink::capture();
    let mut runner = Runner::new(&e.ctx, ws, Options { interval_s: 5, once: false }, sink).unwrap();
    runner.start().unwrap();
    runner.tick();

    // Heartbeat began (Req 3.5) ...
    assert!(backend_yaml(&e).exists(), "heartbeat must begin on the cycle after setup completes");
    // ... and the pipeline began (Req 3.6): the driven record reached the model.
    assert!(fake.calls() > 0, "pipeline must begin invoking the model after setup completes");
    assert!(driven(&path), "the delegated record must have advanced off Reminder");

    runner.shutdown(true);
    // Clean shutdown removes our heartbeat again.
    assert!(!backend_yaml(&e).exists());
}

/// A record has been driven off its initial Reminder state once delegation has
/// been consumed and work has started.
fn driven(path: &Path) -> bool {
    let rec = fabr_core::taskrec::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    use fabr_core::taskrec::State;
    !rec.requests.delegate && rec.state != State::Reminder
}
