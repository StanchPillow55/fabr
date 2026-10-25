//! Feature: agent-runtime-wiring, Property 7: Shutdown removes only our own entries
//!
//! For any registry populated by multiple pids, a clean shutdown by one host
//! removes exactly the entries whose pid equals that host's process id and
//! leaves all others intact.
//!
//! Validates: Requirements 6.5, 10.6
//!
//! This is a self-contained test: it stands up a fabric space with a completed
//! setup so `Runner::new`/`start_gate` succeed, hand-writes a mix of registry
//! entries (some owned by our pid, some by arbitrary foreign pids), calls
//! `Runner::shutdown(true)`, and asserts only our-pid entries are removed while
//! every foreign-pid entry survives byte-for-byte. The `anthropic_url` points
//! at an unused localhost address; `shutdown` makes no model call, and `start`
//! is intentionally NOT called (the heartbeat `beating` stays `None`, which
//! `shutdown` handles via its `Option`).

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::Workspace;
use fabr_core::setup::Ctx;
use proptest::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A single generated registry entry: whether this host owns it (pid == ours)
/// and the pid to stamp when it is foreign.
#[derive(Debug, Clone)]
struct GenEntry {
    ours: bool,
    foreign_pid: u64,
}

struct Env {
    tmp: tempfile::TempDir,
    ctx: Ctx,
    proj: PathBuf,
}

/// Build a fabric space with a completed setup under a fresh tempdir. The
/// `anthropic_url` is an unused localhost address because `shutdown` never
/// calls the model.
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
    std::fs::write(space.join("secrets.toml"), "anthropic_api_key = \"sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa\"\n").unwrap();
    let mut ctx = Ctx::new(&home);
    // Unused localhost address: construction + shutdown never reach the model.
    ctx.anthropic_url = "http://127.0.0.1:1".to_string();
    Env { tmp, ctx, proj }
}

fn runner(e: &Env) -> Runner {
    let cfg = e.tmp.path().join("cfg");
    let cache = e.tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let ws = Workspace::from_config(&cfg, &cache, Some(e.proj.clone())).unwrap();
    Runner::new(&e.ctx, ws, Options { interval_s: 10, once: false }, Sink::default()).unwrap()
}

/// Hand-write a registry entry mirroring `lifecycle::registry_body` closely
/// enough for `registry::list` to read `pid`, `heartbeat_at`, and
/// `heartbeat_interval_s`. A fresh (now) heartbeat keeps foreign entries
/// non-stale, isolating the test to ownership, not staleness.
fn write_entry(reg: &Path, name: &str, pid: u64) -> String {
    let now = format!("{}Z", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S"));
    let task_path = format!("/x/.fabric-space/tasks/{name}.md");
    let body = format!(
        "task_path: {task_path}\n\
         task_id: {name}\n\
         folder: /x\n\
         step: execute\n\
         started_at: \"{now}\"\n\
         heartbeat_at: \"{now}\"\n\
         heartbeat_interval_s: 10\n\
         cost_usd: 0.0\n\
         log_tail: []\n\
         question: null\n\
         pid: {pid}\n"
    );
    std::fs::write(reg.join(format!("{name}.yaml")), &body).unwrap();
    body
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// Feature: agent-runtime-wiring, Property 7: Shutdown removes only our own entries
    ///
    /// Validates: Requirements 6.5, 10.6
    #[test]
    fn shutdown_removes_only_our_own_entries(
        entries in prop::collection::vec(
            (any::<bool>(), 1u64..1_000_000u64).prop_map(|(ours, pid)| GenEntry { ours, foreign_pid: pid }),
            1..12usize,
        ),
    ) {
        let ours = std::process::id() as u64;
        let e = env();
        let reg = e.ctx.space().join("registry");
        std::fs::create_dir_all(&reg).unwrap();

        // A heartbeat file that must survive a clean shutdown here: `start` was
        // never called, so `beating` is `None` and shutdown does not touch it.
        let backend = reg.join("_backend.yaml");
        std::fs::write(&backend, "heartbeat_at: \"2020-01-01T00:00:00Z\"\nheartbeat_interval_s: 10\npid: 42\nstarted_at: \"2020-01-01T00:00:00Z\"\n").unwrap();
        let backend_before = std::fs::read_to_string(&backend).unwrap();

        // Materialize the generated entries with unique names. Record the
        // expected post-shutdown disposition per file: our-pid entries should
        // be gone; foreign-pid entries should remain byte-identical.
        let mut expect_removed: Vec<String> = Vec::new();
        let mut expect_kept: BTreeMap<String, String> = BTreeMap::new();
        for (i, ge) in entries.iter().enumerate() {
            let name = format!("task-{i:04}");
            // Never let a "foreign" pid collide with ours.
            let pid = if ge.ours { ours } else if ge.foreign_pid == ours { ours + 1 } else { ge.foreign_pid };
            let body = write_entry(&reg, &name, pid);
            let file = format!("{name}.yaml");
            if pid == ours {
                expect_removed.push(file);
            } else {
                expect_kept.insert(file, body);
            }
        }

        let mut r = runner(&e);
        // No `start()`: `beating` is `None`. A clean shutdown must still only
        // touch our own pid's registry entries.
        r.shutdown(true);

        // Every entry we owned is gone.
        for file in &expect_removed {
            prop_assert!(!reg.join(file).exists(), "our own entry {file} should have been removed");
        }

        // Every foreign entry survives, byte-for-byte unchanged.
        for (file, body) in &expect_kept {
            let path = reg.join(file);
            prop_assert!(path.exists(), "foreign entry {file} should remain");
            let after = std::fs::read_to_string(&path).unwrap();
            prop_assert_eq!(&after, body, "foreign entry {} was modified", file);
        }

        // The backend heartbeat is untouched when shutdown runs without a start.
        prop_assert!(backend.exists(), "_backend.yaml should remain when start() was never called");
        prop_assert_eq!(std::fs::read_to_string(&backend).unwrap(), backend_before, "_backend.yaml must not change");

        // Nothing unexpected is left behind: the only yaml files remaining are
        // the kept foreign entries plus _backend.yaml.
        let mut remaining: Vec<String> = std::fs::read_dir(&reg)
            .unwrap()
            .flatten()
            .map(|f| f.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".yaml"))
            .collect();
        remaining.sort();
        let mut want: Vec<String> = expect_kept.keys().cloned().collect();
        want.push("_backend.yaml".to_string());
        want.sort();
        prop_assert_eq!(remaining, want, "unexpected registry contents after shutdown");
    }
}
