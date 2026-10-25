//! Feature: agent-runtime-wiring, Property 1: Single owner per task
//!
//! Validates: Requirements 6.1, 6.4, 10.4
//!
//! Exactly one owner may hold the active run for a task. The atomic claim in
//! `fabr_agent::claim_registry` is the single serialization point: the first
//! caller that observes "no live entry" wins by creating the entry with
//! exclusive-create semantics, and any other host that finds a *live* entry
//! owned by a different pid must decline without touching the winner's entry or
//! the task record.
//!
//! Two `Runner`s run in the same integration-test process, so
//! `std::process::id()` is identical for both and cannot be forked in-process.
//! We therefore exercise the claim at the function level: a conflicting
//! registry entry on disk, written with a *different* pid and a chosen
//! heartbeat age, stands in for "another host already owns this task". The
//! properties below vary the interval, the heartbeat age (fresh vs stale), and
//! whether a pre-existing entry exists at all, and assert the three claim
//! outcomes:
//!   (a) a FRESH entry owned by a DIFFERENT pid  -> claim declines (`Ok(false)`)
//!       and leaves both the entry file and the task record byte-identical;
//!   (b) a STALE entry (age > 3x interval)       -> claim succeeds (take over);
//!   (c) no entry at all                         -> claim succeeds and writes an
//!       entry recording our own pid.

use fabr_agent::{claim_registry, registry_path, task_path_key};
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::taskrec::{self, TaskRecord};
use proptest::prelude::*;
use std::path::{Path, PathBuf};

/// A pid that is guaranteed not to be our own, so a conflicting entry reads as
/// "owned by another process".
fn other_pid() -> u64 {
    let ours = std::process::id() as u64;
    if ours == 1 { 2 } else { 1 }
}

fn now_utc() -> String {
    format!("{}Z", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S"))
}

/// Render an rfc3339 timestamp `age_secs` seconds in the past.
fn ts_ago(age_secs: i64) -> String {
    let t = chrono::Utc::now() - chrono::Duration::seconds(age_secs);
    t.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// A fresh node dir + fabric space + a real task record on disk, mirroring the
/// minimal setup used by `tests/runner.rs` and `tests/pipeline.rs`.
struct Fixture {
    _tmp: tempfile::TempDir,
    fabric_space: PathBuf,
    node_dir: PathBuf,
    record_path: PathBuf,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    // The node whose record the claim guards. `create_in` writes the record
    // under `<node_dir>/.fabric-space/tasks/`, so the node dir doubles as the
    // folder passed to the create request.
    let node_dir = tmp.path().join("proj");
    std::fs::create_dir_all(&node_dir).unwrap();
    // The runner-level fabric space (where the registry lives) is independent
    // of the node's own `.fabric-space`.
    let fabric_space = tmp.path().join(".fabric-space");
    std::fs::create_dir_all(&fabric_space).unwrap();

    let req = FabricCreateReq {
        folder: node_dir.to_string_lossy().into_owned(),
        title: "single owner".into(),
        due: Some("2026-11-01".into()),
        context: Some("some context".into()),
        spec: None,
        success: None,
    };
    // `create_in` expands `folder` against `user_home`; our folder is absolute,
    // so the home argument is unused for path resolution.
    let res = fabric::create_in(&req, tmp.path()).unwrap();
    let record_path = PathBuf::from(res.path);

    Fixture { _tmp: tmp, fabric_space, node_dir, record_path }
}

fn load(p: &Path) -> TaskRecord {
    taskrec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// Build a conflicting registry entry body using the same YAML fields the real
/// `write_registry` produces, recording `pid` and `heartbeat_at` as given.
fn conflicting_entry(task_path: &str, task_id: &str, folder: &Path, heartbeat_at: &str, interval_s: u64, pid: u64) -> String {
    let now = now_utc();
    format!(
        "task_path: {task_path:?}\n\
         task_id: {task_id:?}\n\
         folder: {:?}\n\
         step: execute\n\
         started_at: {now:?}\n\
         heartbeat_at: {heartbeat_at:?}\n\
         heartbeat_interval_s: {interval_s}\n\
         cost_usd: 0\n\
         log_tail: []\n\
         question: null\n\
         pid: {pid}\n",
        folder.to_string_lossy(),
    )
}

fn entry_path(fx: &Fixture, rec: &TaskRecord) -> PathBuf {
    registry_path(&fx.fabric_space, &rec.id, &task_path_key(&fx.record_path))
}

fn write_conflict(fx: &Fixture, rec: &TaskRecord, heartbeat_at: &str, interval_s: u64, pid: u64) -> PathBuf {
    let path = entry_path(fx, rec);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let body = conflicting_entry(&task_path_key(&fx.record_path), &rec.id, &fx.node_dir, heartbeat_at, interval_s, pid);
    std::fs::write(&path, body).unwrap();
    path
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(150))]

    /// (a) A FRESH entry owned by a DIFFERENT pid: the claim declines and does
    /// not modify the winner's entry or the task record. This is the core
    /// "single owner" guarantee — the loser never disturbs the winner.
    #[test]
    fn fresh_foreign_entry_is_declined_and_untouched(
        interval_s in 1u64..=120,
        // A heartbeat strictly inside the 3x-interval liveness window.
        age_frac in 0u64..3,
    ) {
        let fx = fixture();
        let rec = load(&fx.record_path);
        // age < 3*interval keeps the entry live; stay well clear of the boundary.
        let age = (age_frac * interval_s) as i64; // in [0, 3*interval)
        let heartbeat_at = ts_ago(age);
        let path = write_conflict(&fx, &rec, &heartbeat_at, interval_s, other_pid());

        let entry_before = std::fs::read(&path).unwrap();
        let record_before = std::fs::read(&fx.record_path).unwrap();

        let won = claim_registry(&fx.fabric_space, &fx.node_dir, &fx.record_path, &rec, interval_s).unwrap();

        prop_assert!(!won, "a live foreign entry must be declined");
        let entry_after = std::fs::read(&path).unwrap();
        let record_after = std::fs::read(&fx.record_path).unwrap();
        prop_assert_eq!(entry_before, entry_after, "declined claim must not touch the winner's entry");
        prop_assert_eq!(record_before, record_after, "declined claim must not touch the record");
    }

    /// (b) A STALE entry (age > 3x interval): the claim succeeds by taking it
    /// over, and the entry now records our own pid.
    #[test]
    fn stale_entry_is_taken_over(
        interval_s in 1u64..=120,
        // Strictly beyond the 3x-interval liveness window (+1s margin).
        extra in 1i64..=600,
    ) {
        let fx = fixture();
        let rec = load(&fx.record_path);
        let age = 3 * interval_s as i64 + extra;
        let heartbeat_at = ts_ago(age);
        let path = write_conflict(&fx, &rec, &heartbeat_at, interval_s, other_pid());

        let won = claim_registry(&fx.fabric_space, &fx.node_dir, &fx.record_path, &rec, interval_s).unwrap();

        prop_assert!(won, "a stale entry must be taken over");
        let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        prop_assert_eq!(doc.get("pid").and_then(|v| v.as_u64()), Some(std::process::id() as u64));
    }

    /// (c) No entry at all: the claim succeeds and writes an entry recording our
    /// own pid.
    #[test]
    fn empty_registry_claim_wins(interval_s in 1u64..=120) {
        let fx = fixture();
        let rec = load(&fx.record_path);
        let path = entry_path(&fx, &rec);
        prop_assert!(!path.exists(), "fixture must start with no entry");

        let won = claim_registry(&fx.fabric_space, &fx.node_dir, &fx.record_path, &rec, interval_s).unwrap();

        prop_assert!(won, "an unclaimed task must be won");
        prop_assert!(path.exists(), "winning the claim must create the entry");
        let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        prop_assert_eq!(doc.get("pid").and_then(|v| v.as_u64()), Some(std::process::id() as u64));
    }

    /// Exactly one winner: once a claim is held, a second attempt that finds a
    /// live foreign entry on disk declines — modelling two hosts racing one
    /// task where the first won. Combines (c) then (a) in one sequence.
    #[test]
    fn exactly_one_owner_across_two_attempts(interval_s in 1u64..=120) {
        let fx = fixture();
        let rec = load(&fx.record_path);

        // First host wins the empty claim.
        let first = claim_registry(&fx.fabric_space, &fx.node_dir, &fx.record_path, &rec, interval_s).unwrap();
        prop_assert!(first, "first attempt on an empty registry must win");

        // A concurrent second host is represented by a fresh foreign-pid entry
        // already on disk (the first winner, from the loser's point of view).
        let path = write_conflict(&fx, &rec, &now_utc(), interval_s, other_pid());
        let entry_before = std::fs::read(&path).unwrap();
        let record_before = std::fs::read(&fx.record_path).unwrap();

        let second = claim_registry(&fx.fabric_space, &fx.node_dir, &fx.record_path, &rec, interval_s).unwrap();
        prop_assert!(!second, "the second host must decline the live foreign claim");
        prop_assert_eq!(entry_before, std::fs::read(&path).unwrap(), "loser must not touch the winner's entry");
        prop_assert_eq!(record_before, std::fs::read(&fx.record_path).unwrap(), "loser must not touch the record");
    }
}
