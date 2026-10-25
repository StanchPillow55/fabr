//! Feature: agent-runtime-wiring, Property 1: Single owner per task
//!
//! Validates: Requirements 6.1, 6.3, 6.4, 10.3, 10.4
//!
//! Host-level companion to the function-level single-owner property
//! (`tests/single_owner_prop.rs`, task 4.2). Here two `Runner`s contend for one
//! task over the same `Fabric_Space`. Both run inside the same integration-test
//! process, so `std::process::id()` is identical for both and cannot be forked
//! in-process. We therefore simulate "another host already owns this task" by
//! planting a FRESH registry entry stamped with a DIFFERENT pid (heartbeat
//! within the 3x-interval liveness window) in the shared `<space>/registry/`
//! before driving a real `Runner`.
//!
//! The shared record is set up in `State::InProgress`/`Step::Execute` — the
//! faithful two-host picture, where the FOREIGN host already advanced the shared
//! task into execution and stamped its own registry entry. A single
//! `Runner::tick()` drives the record and then reaches the atomic claim at the
//! tail of `Runner::process`. Because a live foreign entry already owns the
//! task, `claim_registry` returns `Ok(false)`: the Runner surfaces an INFO line
//! reporting that another process holds the active run and leaves the foreign
//! entry byte-identical. The execute step's model call is directed at an unused
//! localhost address, so it fails fast BEFORE `execute_stage` mutates anything —
//! the shared record therefore stays byte-identical too.
//!
//! The properties vary the heartbeat interval, the foreign pid, and the
//! heartbeat age (always fresh/non-stale for the core "declines" property). A
//! contrasting case plants a STALE foreign entry (age > 3x interval) and asserts
//! the Runner is allowed to take it over and keep driving the task.

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_agent::{registry_path, task_path_key};
use fabr_core::Workspace;
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::setup::Ctx;
use fabr_core::taskrec::{self, Approval, Assertion, Check, PlanMeta, State, Step, TaskRecord, approval_digest};
use proptest::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const KEY: &str = "sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa";

/// A pid guaranteed not to equal our own, so a planted entry reads as "owned by
/// another host".
fn other_pid() -> u64 {
    let ours = std::process::id() as u64;
    if ours == 1 { 2 } else { 1 }
}

fn now_utc() -> String {
    format!("{}Z", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S"))
}

/// An rfc3339 timestamp `age_secs` seconds in the past.
fn ts_ago(age_secs: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::seconds(age_secs)).format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

struct Env {
    tmp: tempfile::TempDir,
    ctx: Ctx,
    proj: PathBuf,
}

/// A fabric space with a completed setup under a fresh tempdir. The
/// `anthropic_url` is an unused localhost address: the decline path never calls
/// the model, and the take-over case uses a `ScriptedModel`-free pipeline step
/// that only acquires the claim and flips the record to `InProgress` without a
/// model round-trip.
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
    let mut ctx = Ctx::new(&home);
    // Unused localhost address: the claim-decline path makes no model call.
    ctx.anthropic_url = "http://127.0.0.1:1".to_string();
    Env { tmp, ctx, proj }
}

fn runner(e: &Env, interval_s: u64) -> (Runner, Arc<Mutex<Vec<String>>>) {
    let cfg = e.tmp.path().join("cfg");
    let cache = e.tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let ws = Workspace::from_config(&cfg, &cache, Some(e.proj.clone())).unwrap();
    let (sink, lines) = Sink::capture();
    (Runner::new(&e.ctx, ws, Options { interval_s, once: false }, sink).unwrap(), lines)
}

fn load(p: &Path) -> TaskRecord {
    taskrec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// Create a record under `<proj>/.fabric-space/tasks/`, then set it directly to
/// `State::InProgress`/`Step::Execute` with an approved, digest-matching plan —
/// the faithful shared view when another host has already advanced the task into
/// execution. In this state `Runner::process` reaches the atomic claim at its
/// tail, and the execute step's model call (to an unused address) fails fast
/// before mutating the record. Returns the record path.
fn in_progress_record(e: &Env) -> PathBuf {
    let req = FabricCreateReq {
        folder: e.proj.to_string_lossy().into_owned(),
        title: "two host contention".into(),
        due: Some("2026-11-01".into()),
        context: Some("some context".into()),
        spec: None,
        success: None,
    };
    let path = PathBuf::from(fabric::create_in(&req, &e.ctx.home).unwrap().path);

    let mut rec = load(&path);
    let expected = rec.updated_at.clone();
    rec.state = State::InProgress;
    rec.step = Some(Step::Execute);
    rec.waiting = false;
    rec.spec = "Write greet.txt containing hi".into();
    rec.success = "greet.txt contains hi".into();
    rec.plan = "1. Write greet.txt".into();
    rec.assertions = vec![Assertion { id: "a1".into(), text: "greet.txt contains hi".into() }];
    rec.plan_meta = Some(PlanMeta {
        reversibility: Some("reversible".into()),
        tools: vec!["write_file".into()],
        assumptions: Vec::new(),
        checks: vec![Check { cmd: "test".into(), args: vec!["-f".into(), "greet.txt".into()] }],
    });
    // A matching digest keeps the approved plan internally consistent even
    // though execution is already in flight.
    let digest = approval_digest(&rec);
    rec.approval = Some(Approval {
        status: "approved".into(),
        by: Some("gui".into()),
        at: Some("2026-10-24T00:00:00Z".into()),
        note: None,
        digest: Some(digest),
    });
    rec.log = "- 2026-10-24T00:00:00Z execute: started\n".into();
    taskrec::write_record(&path, &rec, Some(&expected)).unwrap();
    path
}

/// Plant a foreign-pid registry entry for `rec` in the shared registry, using
/// the same YAML fields `lifecycle::registry_body` produces. `heartbeat_at`
/// controls staleness; `pid` marks the owning (foreign) host. Returns the entry
/// path.
fn plant_foreign(e: &Env, record_path: &Path, rec: &TaskRecord, heartbeat_at: &str, interval_s: u64, pid: u64) -> PathBuf {
    let task_path = task_path_key(record_path);
    let path = registry_path(&e.ctx.space(), &rec.id, &task_path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let now = now_utc();
    let body = format!(
        "task_path: {task_path:?}\n\
         task_id: {:?}\n\
         folder: {:?}\n\
         step: execute\n\
         started_at: {now:?}\n\
         heartbeat_at: {heartbeat_at:?}\n\
         heartbeat_interval_s: {interval_s}\n\
         cost_usd: 0\n\
         log_tail: []\n\
         question: null\n\
         pid: {pid}\n",
        rec.id,
        e.proj.to_string_lossy(),
    );
    std::fs::write(&path, body).unwrap();
    path
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// A FRESH foreign-pid entry: a real `Runner` driving an approved task
    /// DECLINES to start the run and does not modify the foreign entry or the
    /// task record. This is the host-level "single owner" guarantee.
    ///
    /// Feature: agent-runtime-wiring, Property 1: Single owner per task
    /// Validates: Requirements 6.1, 6.3, 6.4, 10.3, 10.4
    #[test]
    fn runner_declines_on_a_live_foreign_entry(
        interval_s in 1u64..=120,
        // A heartbeat strictly inside the 3x-interval liveness window.
        age_frac in 0u64..3,
        // A foreign pid distinct from ours (collision is remapped below).
        foreign in 1u64..1_000_000,
    ) {
        let e = env();
        let path = in_progress_record(&e);
        let rec = load(&path);

        let ours = std::process::id() as u64;
        let pid = if foreign == ours { other_pid() } else { foreign };
        let age = (age_frac * interval_s) as i64; // in [0, 3*interval)
        let entry = plant_foreign(&e, &path, &rec, &ts_ago(age), interval_s, pid);

        let entry_before = std::fs::read(&entry).unwrap();
        let record_before = std::fs::read(&path).unwrap();

        let (mut r, lines) = runner(&e, interval_s);
        r.start().unwrap();
        r.tick();
        r.shutdown(false);

        // The declining host leaves the foreign entry and the task record
        // byte-identical: it never took the claim nor mutated the shared record.
        prop_assert_eq!(&std::fs::read(&entry).unwrap(), &entry_before, "the winner's entry must be untouched");
        prop_assert_eq!(&std::fs::read(&path).unwrap(), &record_before, "the task record must be untouched");

        // The entry still records the FOREIGN pid, not ours.
        let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&entry).unwrap()).unwrap();
        prop_assert_eq!(doc.get("pid").and_then(|v| v.as_u64()), Some(pid), "the foreign owner must be preserved");

        // The Runner surfaced that another process holds the active run.
        let logged = lines.lock().unwrap().iter().any(|l| l.contains("another process holds the active run"));
        prop_assert!(logged, "the Runner must report that another host owns the run");
    }

    /// A STALE foreign entry (age > 3x interval): the Runner is allowed to take
    /// it over and advance the approved task to `InProgress`. Contrasting case
    /// that confirms the decline above is specifically about LIVE foreign
    /// ownership, not any foreign entry.
    ///
    /// Feature: agent-runtime-wiring, Property 1: Single owner per task
    /// Validates: Requirements 6.1, 6.3, 6.4, 10.3, 10.4
    #[test]
    fn runner_takes_over_a_stale_foreign_entry(
        interval_s in 1u64..=120,
        // Strictly beyond the 3x-interval liveness window (+1s margin).
        extra in 1i64..=600,
    ) {
        let e = env();
        let path = in_progress_record(&e);
        let rec = load(&path);

        let age = 3 * interval_s as i64 + extra;
        let entry = plant_foreign(&e, &path, &rec, &ts_ago(age), interval_s, other_pid());

        let (mut r, _lines) = runner(&e, interval_s);
        r.start().unwrap();
        r.tick();
        r.shutdown(false);

        // The task is still in flight: the stale entry did not block us.
        let after = load(&path);
        prop_assert_eq!(after.state, State::InProgress, "a stale foreign entry must not block takeover");

        // The claim now records our own pid (we took the stale entry over).
        let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&entry).unwrap()).unwrap();
        prop_assert_eq!(doc.get("pid").and_then(|v| v.as_u64()), Some(std::process::id() as u64), "we must own the taken-over entry");
    }
}
