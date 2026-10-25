//! Opt-in, env-gated runner-level smoke test against the real Anthropic API.
//!
//! Mirrors `crates/fabr-agent/src/model.rs::live::live_anthropic_smoke`: the
//! test is `#[ignore]` and reads `ANTHROPIC_API_KEY` from the environment, so
//! default CI never runs it. Running it requires the user to export a real key
//! (and optionally `FABR_ANTHROPIC_MODEL`).
//!
//! Unlike `tests/runner.rs`, this test does NOT point the runner at a `Fake`
//! server. It runs ONE real pipeline pass against the real Anthropic base URL
//! and verifies the model-reachable signal (signal 3) end-to-end: the INFO line
//! `model reachable: model=<model> key=<fingerprint>` emitted by `CallLog`
//! on the first successful `ModelCall`.

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::Workspace;
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::setup::Ctx;
use fabr_core::taskrec::{self, TaskRecord};
use std::path::{Path, PathBuf};

struct Env {
    tmp: tempfile::TempDir,
    ctx: Ctx,
    proj: PathBuf,
}

/// Build a completed-setup environment that talks to the REAL Anthropic API.
///
/// `Ctx::new` defaults `anthropic_url` to `https://api.anthropic.com`, so we
/// deliberately leave it untouched (no fake server override).
fn live_env(key: &str, model: &str) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let proj = home.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let space = home.join(".fabric-space");
    std::fs::create_dir_all(&space).unwrap();
    std::fs::write(
        space.join("settings.toml"),
        format!(
            "default_folder = \"{}\"\nmodel = \"{model}\"\nheartbeat_interval_s = 10\n\n[setup]\ncompleted_steps = [\"folder\", \"api_key\", \"model\"]\ncompleted = true\n",
            proj.display()
        ),
    )
    .unwrap();
    std::fs::write(space.join("secrets.toml"), format!("anthropic_api_key = \"{key}\"\n")).unwrap();
    // Leave ctx.anthropic_url at its real default (https://api.anthropic.com).
    let ctx = Ctx::new(&home);
    Env { tmp, ctx, proj }
}

fn runner(e: &Env) -> (Runner, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
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

#[test]
#[ignore = "reads ANTHROPIC_API_KEY and calls the real Anthropic API"]
fn live_runner_model_reachable_smoke() {
    let key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY");
    let model = std::env::var("FABR_ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-sonnet-5-5".into());

    let e = live_env(&key, &model);
    let path = new_record(&e, "add greet.txt that contains hi");
    edit(&path, |r| r.requests.delegate = true);

    let (mut r, lines) = runner(&e);
    r.start().unwrap();
    // One real pipeline pass: the define stage makes one real model call.
    r.tick();
    r.shutdown(true);

    let captured = lines.lock().unwrap().join("\n");
    assert!(
        captured.lines().any(|l| l.starts_with("model reachable:") && l.contains(&format!("model={model}"))),
        "expected a 'model reachable:' signal for model={model} end-to-end; captured:\n{captured}"
    );
}
