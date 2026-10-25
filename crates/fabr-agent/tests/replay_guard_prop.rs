//! Feature: agent-runtime-wiring, Property 3: Non-idempotent action is never replayed
//!
//! **Validates: Requirements 9.2, 9.3, 5.6**
//!
//! Property 3 says that when a `TaskRecord` is resumed into `Execute` and its
//! persisted log shows an execute attempt whose outcome is uncertain
//! (interrupted mid-execute — a tool was applied but `tools finished` was never
//! reached), the runtime must NOT auto-replay an irreversible / non-idempotent
//! action. Instead it routes to `Evaluate` so the checks run and surface a
//! verdict / needs-human, and the model's `execute` is never re-invoked.
//!
//! The replay guard lives in `lifecycle::execute_stage`
//! (`crates/fabr-agent/src/lifecycle.rs`): it computes `execute_interrupted(log)`
//! (an applied tool line — `write_file `/`read_file `/`list_dir ` under
//! `execute:` — with no `execute: tools finished` line) and `safe_to_replay(rec)`
//! (true only when `plan_meta.reversibility != "irreversible"` AND every applied
//! tool is an idempotent `write_file`). When interrupted AND not safe to replay,
//! it routes to `Evaluate` and returns WITHOUT calling `model.execute`.
//!
//! This test drives the real public `advance` entry point with a stub `Model`
//! whose `execute` flips an `AtomicBool` the moment it is called, so the test
//! proves — by observing the flag — whether execute was re-invoked. proptest
//! generates records over the full input space:
//!   * reversibility: `irreversible` vs `reversible`
//!   * applied-tool logs: sequences of `read_file`/`list_dir`/`write_file`
//!     actions WITHOUT a `tools finished` line (i.e. interrupted)
//!
//! Oracle (mirrors the guard):
//!   * guard fires  <=> interrupted AND (irreversible OR any non-`write_file`
//!     action applied). Then: step == Evaluate AND execute NEVER called.
//!   * control case (reversible AND only `write_file` applied) is `safe_to_replay`:
//!     the guard does not fire, replay is allowed (execute MAY be called), and
//!     the record still reaches Evaluate.
//!
//! The harness is self-contained and does not touch `tests/pipeline.rs`.

use fabr_agent::lifecycle::advance;
use fabr_agent::link::delegate;
use fabr_agent::model::{DefineDraft, Model, PlanDraft, ToolCall};
use fabr_core::taskrec::{self, Check, PlanMeta, State, Step, TaskRecord};
use fabr_core::todo::{Item, Status};
use fabr_core::{FabrError, NodeRef, Workspace};
use proptest::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// A stub model that records whether `execute` was ever called. `define` and
/// `plan` are unreachable on the Execute-resume path under test, but must exist
/// to satisfy the `Model` trait.
#[derive(Debug, Default)]
struct SpyModel {
    executed: AtomicBool,
}

impl SpyModel {
    fn executed(&self) -> bool {
        self.executed.load(Ordering::SeqCst)
    }
}

impl Model for SpyModel {
    fn define(&self, _: &str, _: &str) -> Result<DefineDraft, FabrError> {
        panic!("define must not be called while resuming into Execute");
    }

    fn plan(&self, _: &str, _: &str, _: &str, _: Option<&str>) -> Result<PlanDraft, FabrError> {
        panic!("plan must not be called while resuming into Execute");
    }

    fn execute(&self, _: &str, _: &str, _: &str) -> Result<Vec<ToolCall>, FabrError> {
        self.executed.store(true, Ordering::SeqCst);
        // When replay IS allowed (the safe control case) the stage iterates the
        // returned calls. Return a single `write_file` to the already-applied
        // path so `log_has_write` skips re-applying it and the stage lands on
        // Evaluate without touching the filesystem.
        Ok(vec![ToolCall { name: "write_file".into(), path: "greet.txt".into(), content: Some("hi".into()) }])
    }
}

fn workspace(root: &Path) -> Workspace {
    let cfg = root.join("cfg");
    let cache = root.join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    Workspace::from_config(&cfg, &cache, Some(root.to_path_buf())).unwrap()
}

fn setup() -> (tempfile::TempDir, PathBuf, Workspace, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    let ws = workspace(&root);
    ws.init_node(NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let fabric = tmp.path().join("fabric-space");
    std::fs::create_dir_all(&fabric).unwrap();
    (tmp, fabric, ws, root)
}

fn item(title: &str) -> Item {
    Item { id: None, status: Status::Todo, due: None, title: title.into(), pri: None }
}

fn load(path: &Path) -> TaskRecord {
    taskrec::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A single applied-tool entry the prior (interrupted) host wrote to the log.
#[derive(Debug, Clone)]
enum Applied {
    Write,
    Read,
    List,
}

impl Applied {
    fn log_line(&self) -> &'static str {
        match self {
            // Mirror the real executor's log messages (`apply_tool`): the guard
            // only inspects the `write_file `/`read_file `/`list_dir ` prefix.
            Applied::Write => "- 2026-10-24T00:00:00Z execute: write_file greet.txt",
            Applied::Read => "- 2026-10-24T00:00:00Z execute: read_file greet.txt (2 bytes)",
            Applied::List => "- 2026-10-24T00:00:00Z execute: list_dir . (1)",
        }
    }

    fn is_write(&self) -> bool {
        matches!(self, Applied::Write)
    }
}

fn applied_strategy() -> impl Strategy<Value = Applied> {
    prop_oneof![Just(Applied::Write), Just(Applied::Read), Just(Applied::List)]
}

/// Build the interrupted-execute log: a `started` line, then the generated
/// applied-tool lines, and deliberately NO `tools finished` line.
fn interrupted_log(applied: &[Applied]) -> String {
    let mut log = String::from("- 2026-10-24T00:00:00Z execute: started\n");
    for a in applied {
        log.push_str(a.log_line());
        log.push('\n');
    }
    log
}

/// Seed a record that is resuming into Execute with the given reversibility and
/// interrupted log, then run `advance` once with the spy model.
fn drive(irreversible: bool, applied: &[Applied]) -> (SpyModel, TaskRecord) {
    let (_tmp, fabric, ws, root) = setup();
    let path = delegate(&ws, "local", "", &item("resume")).unwrap();
    // A file already exists (the prior host may have written it); its presence
    // must not cause the guard to re-apply anything irreversibly.
    std::fs::write(root.join("greet.txt"), "hi").unwrap();
    let mut rec = load(&path);
    rec.state = State::InProgress;
    rec.step = Some(Step::Execute);
    rec.spec = "Write greet.txt containing hi".into();
    rec.plan = "1. Write greet.txt".into();
    rec.log = interrupted_log(applied);
    rec.plan_meta = Some(PlanMeta {
        reversibility: Some(if irreversible { "irreversible".into() } else { "reversible".into() }),
        tools: vec!["write_file".into(), "read_file".into(), "list_dir".into()],
        assumptions: Vec::new(),
        checks: vec![Check { cmd: "test".into(), args: vec!["-f".into(), "greet.txt".into()] }],
    });
    let expected = rec.updated_at.clone();
    taskrec::write_record(&path, &rec, Some(&expected)).unwrap();

    let model = SpyModel::default();
    advance(&ws, "local", "", &root, &path, &fabric, 10, &model).unwrap();
    let after = load(&path);
    (model, after)
}

/// The oracle: whether an interrupted execute with these inputs is safe to
/// replay (reversible AND every applied tool is an idempotent `write_file`).
fn safe_to_replay(irreversible: bool, applied: &[Applied]) -> bool {
    !irreversible && applied.iter().all(Applied::is_write)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

    /// Property 3: a resumed, interrupted non-idempotent/irreversible execute is
    /// never replayed — it routes to Evaluate and `model.execute` is not called.
    /// The reversible + write-only control case stays replayable.
    #[test]
    fn non_idempotent_action_is_never_replayed(
        irreversible in any::<bool>(),
        // At least one applied tool so the log is genuinely "interrupted"
        // (a started-but-no-tool log is not an interrupted execute).
        applied in prop::collection::vec(applied_strategy(), 1..=6),
    ) {
        let (model, after) = drive(irreversible, &applied);
        let safe = safe_to_replay(irreversible, &applied);

        // Either path lands on Evaluate: the guard routes there, and the
        // replayable control path also finishes execute and moves to Evaluate.
        prop_assert_eq!(after.step, Some(Step::Evaluate), "resumed execute must reach Evaluate");
        // The verdict is always cleared on this transition.
        prop_assert!(after.verdict.is_none(), "verdict must be cleared before evaluate");

        if safe {
            // Reversible + only write_file applied: replay is allowed, so the
            // model's execute was invoked (idempotent writes are safe).
            prop_assert!(model.executed(), "safe-to-replay case should allow execute");
            prop_assert!(
                after.log.contains("tools finished"),
                "a replayed execute should finish its tools"
            );
        } else {
            // Irreversible OR any non-write_file action applied: the guard must
            // fire. The irreversible / non-idempotent action is NEVER replayed.
            prop_assert!(!model.executed(), "non-idempotent/irreversible execute must not be replayed");
            prop_assert!(
                after.log.contains("routing to evaluate instead of replaying"),
                "the guard should log that it routed to evaluate"
            );
            prop_assert!(
                !after.log.contains("tools finished"),
                "the guard must not run tools to completion"
            );
        }
    }
}
