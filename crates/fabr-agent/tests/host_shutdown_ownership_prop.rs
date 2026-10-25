//! Feature: agent-runtime-wiring, Property 7: Shutdown removes only our own entries
//!
//! Host-level companion to `shutdown_ownership_prop.rs`. Where that test checks
//! the `shutdown(true)` function in isolation (no `start`, no live heartbeat),
//! this one exercises the FULL host lifecycle: a real `Runner` that has
//! `start()`ed — so its dedicated heartbeat thread is running and has written
//! its OWN `_backend.yaml` entry (owned by this process's pid) — over a registry
//! that ALSO contains foreign-pid entries. A clean shutdown must then remove
//! exactly this process's own entries (the per-task entry we plant with our pid
//! plus the `_backend.yaml` heartbeat the running host wrote) and leave every
//! foreign-pid entry byte-for-byte intact.
//!
//! Validates: Requirements 6.5, 10.6
//!
//! This is a self-contained test: it stands up a fabric space with a completed
//! setup so `Runner::new`/`start_gate`/`start` succeed. The `anthropic_url`
//! points at an unused localhost address; `start` + `shutdown` make no model
//! call (the heartbeat thread only writes `_backend.yaml`, and we shut down
//! immediately after `start`, so no long sleeps are needed — `Beating::spawn`
//! writes the heartbeat once immediately and `stop` returns as soon as the
//! thread observes the stop signal).

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::Workspace;
use fabr_core::setup::Ctx;
use proptest::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A single generated foreign registry entry, stamped with a non-stale (now)
/// heartbeat and a pid that is never this process's pid.
#[derive(Debug, Clone)]
struct ForeignEntry {
    pid: u64,
}

struct Env {
    tmp: tempfile::TempDir,
    ctx: Ctx,
    proj: PathBuf,
}

/// Build a fabric space with a completed setup under a fresh tempdir. The
/// `anthropic_url` is an unused localhost address because neither `start` nor
/// `shutdown` calls the model.
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
    // Unused localhost address: construction + start + shutdown never reach the model.
    ctx.anthropic_url = "http://127.0.0.1:1".to_string();
    Env { tmp, ctx, proj }
}

fn runner(e: &Env, interval_s: u64) -> Runner {
    let cfg = e.tmp.path().join("cfg");
    let cache = e.tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let ws = Workspace::from_config(&cfg, &cache, Some(e.proj.clone())).unwrap();
    Runner::new(&e.ctx, ws, Options { interval_s, once: false }, Sink::default()).unwrap()
}

/// Hand-write a registry entry mirroring `lifecycle::registry_body` closely
/// enough for `registry::list` to read `pid`, `heartbeat_at`, and
/// `heartbeat_interval_s`. A fresh (now) heartbeat keeps entries non-stale, so
/// the test isolates to ownership rather than staleness.
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
    #![proptest_config(ProptestConfig { cases: 100, ..ProptestConfig::default() })]

    /// Feature: agent-runtime-wiring, Property 7: Shutdown removes only our own entries
    ///
    /// A running host (one that has `start()`ed, so its heartbeat thread wrote
    /// `_backend.yaml`) over a registry mixing foreign-pid entries with one
    /// entry owned by our pid: a clean shutdown removes exactly our own entries
    /// (the planted own-pid task entry and the live `_backend.yaml` heartbeat)
    /// and leaves every foreign entry intact.
    ///
    /// Validates: Requirements 6.5, 10.6
    #[test]
    fn running_host_clean_shutdown_removes_only_our_own_entries(
        foreign in prop::collection::vec(
            (1u64..1_000_000u64).prop_map(|pid| ForeignEntry { pid }),
            1..12usize,
        ),
        interval_s in 1u64..10u64,
    ) {
        let ours = std::process::id() as u64;
        let e = env();
        let reg = e.ctx.space().join("registry");
        std::fs::create_dir_all(&reg).unwrap();

        // Materialize the generated FOREIGN entries with unique names and pids
        // that never collide with ours. Record each one's expected
        // post-shutdown bytes so we can assert it survives unchanged.
        let mut expect_kept: BTreeMap<String, String> = BTreeMap::new();
        for (i, fe) in foreign.iter().enumerate() {
            let name = format!("foreign-{i:04}");
            let pid = if fe.pid == ours { ours + 1 } else { fe.pid };
            let body = write_entry(&reg, &name, pid);
            expect_kept.insert(format!("{name}.yaml"), body);
        }

        // Plant exactly one entry owned by OUR pid. Clean shutdown must remove
        // it (part (c): entries where pid == std::process::id()).
        let own_file = "own-0000.yaml".to_string();
        write_entry(&reg, "own-0000", ours);

        let backend = reg.join("_backend.yaml");

        // Start the real host: `Beating::spawn` writes `_backend.yaml`
        // (owned by our pid) immediately, and the heartbeat thread begins
        // running. `clean_stale` runs over the registry at start but leaves
        // our fresh (non-stale) foreign and own entries untouched.
        let mut r = runner(&e, interval_s);
        r.start().unwrap();

        // The running host wrote its own heartbeat file.
        prop_assert!(backend.exists(), "_backend.yaml must exist while the host is running");
        let backend_doc: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&backend).unwrap()).unwrap();
        prop_assert_eq!(backend_doc["pid"].as_u64(), Some(ours), "_backend.yaml must be owned by our pid");

        // Clean shutdown: stop the heartbeat thread and remove our own entries.
        r.shutdown(true);

        // (c) Our planted per-task entry is gone.
        prop_assert!(!reg.join(&own_file).exists(), "our own task entry {own_file} should have been removed");

        // (b) The live heartbeat file we wrote on start is gone.
        prop_assert!(!backend.exists(), "_backend.yaml should be removed by a clean shutdown of a running host");

        // (a) Every foreign entry survives, byte-for-byte unchanged.
        for (file, body) in &expect_kept {
            let path = reg.join(file);
            prop_assert!(path.exists(), "foreign entry {file} should remain");
            let after = std::fs::read_to_string(&path).unwrap();
            prop_assert_eq!(&after, body, "foreign entry {} was modified", file);
        }

        // Nothing unexpected is left behind: the only yaml files remaining are
        // the kept foreign entries (no own entry, no _backend.yaml).
        let mut remaining: Vec<String> = std::fs::read_dir(&reg)
            .unwrap()
            .flatten()
            .map(|f| f.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".yaml"))
            .collect();
        remaining.sort();
        let mut want: Vec<String> = expect_kept.keys().cloned().collect();
        want.sort();
        prop_assert_eq!(remaining, want, "unexpected registry contents after shutdown");
    }
}
