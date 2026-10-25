//! Tests for the startup assertion and INFO/WARN logging added in task 9.1
//! (`Runner::start` in `crates/fabr-agent/src/runner/mod.rs`), covering
//! Requirements 8.1 (startup INFO line with the absolute folder and interval;
//! loud WARN on a space mismatch so "not connected" is diagnosable) and 8.3
//! (WARN when a discovered record's canonical path escapes the search root).
//!
//! This is a self-contained harness (deliberately separate from
//! `tests/runner.rs`) because the space-mismatch test mutates the process-global
//! `FABR_HOME` env var, which `Ctx::from_env()` reads. Keeping that mutation in
//! its own test binary and confined to a single test avoids racing other tests
//! that also resolve `Ctx::from_env()`.

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::Workspace;
use fabr_core::setup::Ctx;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const KEY: &str = "sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa";

/// A ready (setup-complete) fabric home rooted at a fresh tempdir.
struct Env {
    tmp: tempfile::TempDir,
    ctx: Ctx,
    proj: PathBuf,
}

/// Build a setup-complete `Env`: `<tmp>/home` is the fabric home, `<home>/proj`
/// the default folder, with settings + secrets written so `start_gate` passes.
fn env() -> Env {
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
    // No real server is contacted: these tests only exercise `start()`, which
    // never performs a model call. A bogus URL keeps the runner offline.
    let mut ctx = Ctx::new(&home);
    ctx.anthropic_url = "http://127.0.0.1:1".to_string();
    Env { tmp, ctx, proj }
}

/// Build a runner over `e` with the given heartbeat interval, capturing its
/// INFO/WARN lines.
fn runner(e: &Env, interval_s: u64) -> (Runner, Arc<Mutex<Vec<String>>>) {
    let cfg = e.tmp.path().join("cfg");
    let cache = e.tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let ws = Workspace::from_config(&cfg, &cache, Some(e.proj.clone())).unwrap();
    let (sink, lines) = Sink::capture();
    (Runner::new(&e.ctx, ws, Options { interval_s, once: false }, sink).unwrap(), lines)
}

fn captured(lines: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    lines.lock().unwrap().clone()
}

#[test]
fn startup_info_line_has_absolute_folder_and_interval() {
    let e = env();
    let (mut r, lines) = runner(&e, 7);
    r.start().unwrap();
    r.shutdown(true);

    let all = captured(&lines);
    let start = all.iter().find(|l| l.starts_with("agent starting:")).expect("one startup INFO line");
    // The default folder is logged as an absolute path (Requirement 8.1): on
    // unix an absolute path starts with `/`. We also assert it is exactly the
    // canonical project directory so a relative/raw path would fail the test.
    let abs = e.proj.canonicalize().unwrap();
    assert!(start.contains(&format!("default_folder={}", abs.display())), "{start}");
    assert!(start.starts_with("agent starting:") && abs.is_absolute(), "{start}");
    // The interval is included and expressed in seconds (Requirement 8.1).
    assert!(start.contains("interval=7s"), "{start}");
}

#[test]
fn space_mismatch_emits_warn() {
    // Make `Ctx::from_env().space()` deterministically differ from the runner's
    // space by pointing `FABR_HOME` at a distinct directory. `Ctx::from_env()`
    // reads `FABR_HOME` from the process env, while the runner's space comes
    // from the tempdir-based `Ctx` passed to `Runner::new`, so the two resolve
    // to different `.fabric-space` paths and the mismatch WARN must fire.
    //
    // `FABR_HOME` is process-global, so this mutation is confined to this one
    // test (set at the top, cleared before returning) to avoid interfering with
    // other tests that call `Ctx::from_env()`.
    let other = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("FABR_HOME", other.path());
    }

    let e = env();
    let (mut r, lines) = runner(&e, 5);
    r.start().unwrap();
    r.shutdown(true);

    unsafe {
        std::env::remove_var("FABR_HOME");
    }

    let all = captured(&lines);
    assert!(
        all.iter().any(|l| l.contains("fabric space mismatch") && l.contains("check FABR_HOME")),
        "expected a space-mismatch WARN, got: {all:?}"
    );
}

#[cfg(unix)]
#[test]
fn out_of_root_record_emits_warn() {
    use std::os::unix::fs::symlink;

    let e = env();
    // Place a record outside the project, then point the project's
    // `.fabric-space/tasks` directory at it via a symlink. `fabric::list` reads
    // the tasks directory through the symlink (so the record is discovered under
    // the search root's path string), but the record's canonical path resolves
    // to the outside location, which must trigger the out-of-root WARN
    // (Requirement 8.3).
    let outside = e.tmp.path().join("outside-tasks");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("escaped.md"),
        "---\nschema: 2\nid: escaped\nsource: local\ntitle: escaped\nstate: reminder\ncreated_at: T1\nupdated_at: T1\n---\n",
    )
    .unwrap();

    let space = e.proj.join(".fabric-space");
    std::fs::create_dir_all(&space).unwrap();
    symlink(&outside, space.join("tasks")).unwrap();

    let (mut r, lines) = runner(&e, 5);
    r.start().unwrap();
    r.shutdown(true);

    let all = captured(&lines);
    assert!(
        all.iter().any(|l| l.contains("outside the default folder") && l.contains("escaped.md")),
        "expected an out-of-root WARN, got: {all:?}"
    );
}
