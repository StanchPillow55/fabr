//! Confirmation test for task 9.3: `default_folder` is a *search root*, not a node.
//!
//! The `Runner` sets `root = default_folder` and uses it only to discover
//! records via `fabric::list` (`Runner::records`). The per-record node directory
//! is then derived from each record's own path by `node_dir_of()` (the record's
//! `<node>/.fabric-space/tasks/<id>.md` -> `<node>`) and passed to `advance`.
//!
//! This test places a managed node *below* the search root (at depth, with a
//! non-empty node path) and drives the `Runner`. A record discovered and
//! advanced under that nested node proves the root is a search root: the record
//! is driven against its own node directory, not against the root folder.

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::Workspace;
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::setup::Ctx;
use fabr_core::taskrec::{self, State, TaskRecord};
use fabr_core::{NodeRef, SaveTodoReq};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const KEY: &str = "sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa";

/// Minimal fake Anthropic server: every define call asks no question and
/// returns a spec/success/assertions payload, so a delegated record advances
/// from Reminder to the Define question/approval gate on a single tick. This
/// mirrors the `Fake` used in `tests/runner.rs` but is kept self-contained to
/// avoid cross-file coupling (task 9.3 asks for a standalone file).
struct Fake {
    url: String,
    stop: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Fake {
    fn new() -> Fake {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let (st, cl) = (stop.clone(), calls.clone());
        let handle = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                let Ok(Some(mut req)) = server.recv_timeout(std::time::Duration::from_millis(50)) else { continue };
                let mut body = String::new();
                std::io::Read::read_to_string(req.as_reader(), &mut body).unwrap();
                cl.fetch_add(1, Ordering::SeqCst);
                // Define stage asks for a question field; return null (no question)
                // so the record progresses to a plan/approval gate deterministically.
                let inner = if body.contains("keys spec, success, assertions, question") {
                    json!({"spec": "write greet.txt", "success": "greet.txt exists", "assertions": [{"id": "a1", "text": "greet.txt exists"}], "question": null})
                } else {
                    json!({"spec": "write greet.txt", "success": "greet.txt exists", "plan": "1. write it", "assertions": [{"id": "a1", "text": "greet.txt exists"}],
                        "plan_meta": {"reversibility": "reversible", "tools": ["write_file"], "checks": [{"cmd": "test", "args": ["-f", "greet.txt"]}]}})
                };
                let text =
                    json!({"content": [{"type": "text", "text": inner.to_string()}], "usage": {"input_tokens": 10, "output_tokens": 5}})
                        .to_string();
                let resp = tiny_http::Response::from_string(text)
                    .with_status_code(200u16)
                    .with_header(tiny_http::Header::from_bytes("content-type", "application/json").unwrap());
                let _ = req.respond(resp);
            }
        });
        Fake { url, stop, calls, handle: Some(handle) }
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

fn load(p: &Path) -> TaskRecord {
    taskrec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn edit(p: &Path, f: impl FnOnce(&mut TaskRecord)) {
    let mut rec = load(p);
    let expected = rec.updated_at.clone();
    f(&mut rec);
    taskrec::write_record(p, &rec, Some(&expected)).unwrap();
}

/// Build an environment where `default_folder` is a *search root* containing a
/// managed fabr node in a nested subdirectory (`work/errands`), and a record
/// lives under that nested node. Returns (tempdir guard, ctx, root, node_dir).
struct Env {
    _tmp: tempfile::TempDir,
    ctx: Ctx,
    root: PathBuf,
    node_dir: PathBuf,
}

fn nested_env(url: &str) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    // The search root. The managed node is at depth under it (non-empty node path).
    let root = home.join("search-root");
    let node_dir = root.join("work").join("errands");
    std::fs::create_dir_all(&node_dir).unwrap();

    // settings.toml points default_folder at the SEARCH ROOT, not the node.
    let space = home.join(".fabric-space");
    std::fs::create_dir_all(&space).unwrap();
    std::fs::write(
        space.join("settings.toml"),
        format!(
            "default_folder = \"{}\"\nmodel = \"claude-sonnet-5-5\"\nheartbeat_interval_s = 10\n\n[setup]\ncompleted_steps = [\"folder\", \"api_key\", \"model\"]\ncompleted = true\n",
            root.display()
        ),
    )
    .unwrap();
    std::fs::write(space.join("secrets.toml"), format!("anthropic_api_key = \"{KEY}\"\n")).unwrap();

    let mut ctx = Ctx::new(&home);
    ctx.anthropic_url = url.to_string();
    Env { _tmp: tmp, ctx, root, node_dir }
}

fn workspace(e: &Env) -> Workspace {
    let cfg = e._tmp.path().join("cfg");
    let cache = e._tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    // The workspace is rooted at the search root; the node path is relative to it.
    Workspace::from_config(&cfg, &cache, Some(e.root.clone())).unwrap()
}

/// A record under a node *below* the search root is discovered by the Runner
/// (via `fabric::list` on `default_folder`) and advanced against that node's
/// own directory — not the root. Confirms `default_folder` is a search root and
/// `node_dir` is derived per record. (Requirements 4.1, 4.4)
#[test]
fn record_under_a_nested_node_is_discovered_and_driven() {
    let fake = Fake::new();
    let e = nested_env(&fake.url);

    // Create a record under the NESTED node (depth >= 1 below the search root).
    let req = FabricCreateReq {
        folder: e.node_dir.to_string_lossy().into_owned(),
        title: "write a greeting".into(),
        due: Some("2026-11-01".into()),
        context: Some("some context".into()),
        spec: None,
        success: None,
    };
    let path = PathBuf::from(fabric::create_in(&req, &e.ctx.home).unwrap().path);

    // The record lives under <root>/work/errands/.fabric-space/tasks/, i.e.
    // strictly below the search root with a non-empty node path.
    assert!(path.starts_with(&e.node_dir), "record must live under the nested node: {}", path.display());
    assert_ne!(e.node_dir, e.root, "the node must be below the search root, not the root itself");

    edit(&path, |r| r.requests.delegate = true);

    let ws = workspace(&e);
    let (sink, _lines) = Sink::capture();
    let mut r = Runner::new(&e.ctx, ws, Options { interval_s: 5, once: false }, sink).unwrap();
    r.start().unwrap();
    r.tick();
    r.shutdown(true);

    // The model was called (the nested record was discovered and driven) and the
    // record advanced out of Reminder along the normal pipeline. If the root had
    // been treated as the node, this nested record would not have been driven
    // against its own node directory.
    assert!(fake.calls.load(Ordering::SeqCst) >= 1, "the nested record should have been driven");
    let rec = load(&path);
    assert!(!rec.requests.delegate, "delegate request should be consumed");
    assert_ne!(rec.state, State::Reminder, "the record should have advanced past Reminder: {rec:?}");
    assert!(
        matches!(rec.state, State::Task | State::Defined) && rec.step.is_some(),
        "expected advancement into the pipeline, got {:?}/{:?}",
        rec.state,
        rec.step
    );
}

/// Delegation turns a node Todo into a record under that node's
/// `.fabric-space/tasks/` — for a managed node located *below* the search root.
/// Confirms the `run_once` -> `link::delegate` path keys off the node directory,
/// not the search root. (Requirements 4.1, 4.4)
#[test]
fn delegate_creates_a_reminder_under_the_nested_node() {
    let fake = Fake::new();
    let e = nested_env(&fake.url);
    let ws = workspace(&e);

    // Make the nested directory a managed node and give it a Todo.
    let node_ref = NodeRef { source: "local".into(), path: "work/errands".into() };
    ws.init_node(node_ref.clone()).unwrap();
    let node = ws.get_node(node_ref.clone()).unwrap();
    ws.save_todo(SaveTodoReq {
        source: "local".into(),
        path: "work/errands".into(),
        version: node.version.unwrap(),
        items: vec![fabr_core::todo::Item {
            id: None,
            status: fabr_core::todo::Status::Todo,
            due: None,
            title: "mail the package".into(),
            pri: None,
        }],
    })
    .unwrap();

    // Delegate the Todo: a Reminder record must be created under the NESTED
    // node's tasks dir, not under the search root's.
    let saved = ws.get_node(node_ref).unwrap();
    let created = fabr_agent::link::delegate(&ws, "local", "work/errands", &saved.items[0]).unwrap();

    // Compare canonical paths: on macOS the tempdir resolves through a /private
    // symlink, so a plain `starts_with` on the un-canonicalized paths would miss.
    let tasks = e.node_dir.join(".fabric-space").join("tasks").canonicalize().unwrap();
    let created_canon = created.canonicalize().unwrap();
    assert!(created_canon.starts_with(&tasks), "reminder must be created under the nested node: {}", created.display());
    let root_tasks = e.root.join(".fabric-space").join("tasks");
    assert!(!root_tasks.exists(), "reminder must not land under the search root's own tasks dir");

    let rec = load(&created);
    assert_eq!(rec.state, State::Reminder);
    assert_eq!(rec.source.as_ref().unwrap().node_path, "work/errands");
    assert_eq!(rec.title, "mail the package");

    // And the Runner, scanning the search root, finds this nested reminder.
    edit(&created, |r| r.requests.delegate = true);
    let (sink, _lines) = Sink::capture();
    let mut r = Runner::new(&e.ctx, workspace(&e), Options { interval_s: 5, once: false }, sink).unwrap();
    r.start().unwrap();
    r.tick();
    r.shutdown(true);
    let advanced = load(&created);
    assert_ne!(advanced.state, State::Reminder, "the nested reminder should advance once delegated: {advanced:?}");
    assert!(fake.calls.load(Ordering::SeqCst) >= 1, "the model should have been called driving the nested reminder");
}
