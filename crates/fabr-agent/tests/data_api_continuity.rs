//! Data_API continuity (task 10.2, Requirements 9.1 and 3.7).
//!
//! Requirement 9.1 states the in-process Data_API — the operations dispatched
//! by `fabr_core::ops::dispatch` — must respond with "identical request inputs,
//! response outputs, and error responses" whether or not the Agent_Worker is
//! running. This integration test exercises the five read/write ops the task
//! names (`get_tree`, `get_stamp`, `fabric_list`, `fabric_read`,
//! `fabric_write`) twice: once with no worker thread (BASELINE) and once with a
//! `Runner` running on a background thread, and asserts the responses match.
//!
//! The requirement is about the *Data_API contract* (same inputs produce the
//! same Ok/Err disposition and the same response shape), not that the bytes on
//! disk are frozen while the agent works. So we keep the worker from mutating
//! the records the Data_API reads: the single record is a `Reminder` with no
//! `delegate` request, which the runner's `process()` leaves untouched
//! (`drive_it` is false for `Reminder` unless delegate is set). The worker still
//! does its other job — it writes the Backend_Heartbeat (`_backend.yaml`) under
//! the *home* `.fabric-space/registry`, which is a different directory from the
//! project's `.fabric-space` that holds the record.
//!
//! One field legitimately differs: `fabric_list`'s `backend` entry is absent
//! until the worker writes `_backend.yaml`, then present. That is the heartbeat
//! presence signal, not a Data_API contract change, so we assert it explicitly
//! as expected-and-acceptable and compare the `records`/`registry`/`events`
//! portions for exact equality. Every other op must match byte-for-byte.

use fabr_agent::runner::{Options, Runner, Sink};
use fabr_core::fabric::{FabricCreateReq, FabricListReq, FabricReadReq, FabricWriteReq};
use fabr_core::ops::{Done, dispatch};
use fabr_core::setup::Ctx;
use fabr_core::{Workspace, fabric};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

const KEY: &str = "sk-ant-api03-FAKEFAKEFAKEFAKEFAKE-x9Qa";

/// `Ctx::from_env()` reads the process-global `FABR_HOME`. A handful of ops
/// (`fabric_*`) fall back to it only when a request omits `home`; we always
/// pass `home` explicitly, but `Runner::start` also compares `self.space`
/// against `Ctx::from_env().space()` and WARNs on a mismatch. Serialize the
/// whole test so a stray env var from another test cannot interfere, and point
/// `FABR_HOME` at this test's home for the duration of the run.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// A minimal fake Anthropic endpoint. The parked record never triggers a model
/// call in this test, but `Runner` needs a reachable base URL to pass its gate
/// and construct the model client, so we keep a server alive and simply never
/// expect it to be hit.
struct Fake {
    url: String,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Fake {
    fn new() -> Fake {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        let handle = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                let Ok(Some(req)) = server.recv_timeout(std::time::Duration::from_millis(50)) else { continue };
                let resp = tiny_http::Response::from_string("{}")
                    .with_header(tiny_http::Header::from_bytes("content-type", "application/json").unwrap());
                let _ = req.respond(resp);
            }
        });
        Fake { url, stop, handle: Some(handle) }
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
    home: PathBuf,
    proj: PathBuf,
}

/// Build a completed-setup workspace home with a project folder as the
/// default folder, mirroring the layout the runner tests use (home holds the
/// registry/heartbeat; the project holds the record).
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
    Env { tmp, ctx, home, proj }
}

/// Construct a Workspace rooted at the project folder (the single local
/// source), the same shape the desktop host builds.
fn workspace(e: &Env) -> Workspace {
    let cfg = e.tmp.path().join("cfg");
    let cache = e.tmp.path().join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    Workspace::from_config(&cfg, &cache, Some(e.proj.clone())).unwrap()
}

/// A managed node at the project root (so `get_tree` reports a real managed
/// node) that also owns one parked `Reminder` record (so the read ops return
/// meaningful data). Returns the absolute record path.
fn seed(e: &Env, ws: &Workspace) -> PathBuf {
    // Make the project root a managed node: `init_node` writes `.fabr/`.
    let empty = json!({"source": "local", "path": ""});
    dispatch(ws, "init_node", empty).unwrap();
    // Create a Reminder task record under the node's `.fabric-space/tasks`.
    let req = FabricCreateReq {
        folder: e.proj.to_string_lossy().into_owned(),
        title: "a steady reminder".into(),
        due: Some("2026-11-01".into()),
        context: Some("context that does not change".into()),
        spec: None,
        success: None,
    };
    let record = PathBuf::from(fabric::create_in(&req, &e.home).unwrap().path);

    // Normalize the record to the writer's canonical rendering once, up front.
    // `fabric_write` re-renders the YAML frontmatter through `taskrec` (field
    // order, flow vs block style, `0` vs `0.0`), so the first write changes the
    // bytes even for identical content. Doing it here — before any snapshot —
    // means every later `fabric_write` round-trip is a true no-op fixpoint and
    // both passes read identical bytes, isolating the test from the normalizer
    // rather than from the worker. (The worker does not touch this record.)
    let read = as_json(dispatch(ws, "fabric_read", json!({"path": record.to_string_lossy()})).unwrap());
    let text = read["text"].as_str().unwrap().to_string();
    let write =
        FabricWriteReq { path: record.to_string_lossy().into_owned(), text: text.clone(), expected_updated_at: fabric::updated_at(&text) };
    dispatch(ws, "fabric_write", serde_json::to_value(write).unwrap()).unwrap();
    record
}

fn as_json(d: Done) -> Value {
    match d {
        Done::Json(v) => v,
        Done::Desktop(_) => panic!("expected a JSON response, not a desktop action"),
    }
}

/// The five request bodies, built once from the record path so both passes use
/// identical inputs. `fabric_write` bumps `updated_at`, so each pass rewrites
/// with the value currently on disk and is compared only for its Ok shape.
struct Reqs {
    get_tree: Value,
    get_stamp: Value,
    fabric_list: Value,
    fabric_read: Value,
    record: PathBuf,
}

impl Reqs {
    fn new(e: &Env, record: PathBuf) -> Reqs {
        Reqs {
            get_tree: json!({"show_all": false, "rescan": false}),
            get_stamp: json!({}),
            fabric_list: serde_json::to_value(FabricListReq {
                root: e.proj.to_string_lossy().into_owned(),
                home: Some(e.home.to_string_lossy().into_owned()),
            })
            .unwrap(),
            fabric_read: serde_json::to_value(FabricReadReq { path: record.to_string_lossy().into_owned() }).unwrap(),
            record,
        }
    }
}

/// Snapshot of each op's response for one pass.
///
/// `fabric_read`/`fabric_write` payloads carry an `mtime`, a wall-clock value
/// that moves on every write by design, so we keep only the parts the
/// continuity contract covers: the record `text` for `fabric_read`, and the
/// Ok/Err disposition for `fabric_write`.
struct Snapshot {
    get_tree: Value,
    get_stamp: Value,
    fabric_list: fabric::FabricListRes,
    read_text: String,
    write_ok: bool,
}

/// Dispatch all five ops and snapshot the responses. `get_tree` carries a
/// `today` date field that is intentionally stripped so a date rollover between
/// passes cannot cause a spurious mismatch — it is not part of the continuity
/// contract.
fn snapshot(ws: &Workspace, r: &Reqs) -> Snapshot {
    let mut get_tree = as_json(dispatch(ws, "get_tree", r.get_tree.clone()).unwrap());
    if let Some(obj) = get_tree.as_object_mut() {
        obj.remove("today");
    }
    let get_stamp = as_json(dispatch(ws, "get_stamp", r.get_stamp.clone()).unwrap());
    let fabric_list: fabric::FabricListRes =
        serde_json::from_value(as_json(dispatch(ws, "fabric_list", r.fabric_list.clone()).unwrap())).unwrap();
    let read_text = read_text(ws, r);

    // Round-trip the record unchanged through `fabric_write` with the current
    // `updated_at`, proving the write op accepts and succeeds identically. The
    // record was normalized to the writer's canonical form in `seed`, so this
    // write produces byte-identical content (an idempotent round-trip) and does
    // not perturb the next read's `text`.
    let write_req = FabricWriteReq {
        path: r.record.to_string_lossy().into_owned(),
        text: read_text.clone(),
        expected_updated_at: fabric::updated_at(&read_text),
    };
    let write_ok = dispatch(ws, "fabric_write", serde_json::to_value(write_req).unwrap()).is_ok();

    Snapshot { get_tree, get_stamp, fabric_list, read_text, write_ok }
}

/// Read the record and return only its `text` (the stable part of the response;
/// `mtime` is a wall-clock value excluded from the continuity comparison).
fn read_text(ws: &Workspace, r: &Reqs) -> String {
    let res = as_json(dispatch(ws, "fabric_read", r.fabric_read.clone()).unwrap());
    res["text"].as_str().expect("fabric_read returns text").to_string()
}

/// The reported file paths, dropping the wall-clock `mtime`.
fn paths(stamps: &[fabric::FileStamp]) -> Vec<&str> {
    stamps.iter().map(|f| f.path.as_str()).collect()
}

fn set_home<'a>(home: &std::path::Path) -> MutexGuard<'a, ()> {
    let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the whole test holds ENV_LOCK, so no other thread in this test
    // binary mutates or reads the env concurrently.
    unsafe { std::env::set_var("FABR_HOME", home) };
    guard
}

#[test]
fn data_api_responses_are_identical_with_and_without_the_worker() {
    let fake = Fake::new();
    let e = env(&fake.url);
    let _guard = set_home(&e.home);

    let ws = workspace(&e);
    let record = seed(&e, &ws);
    let reqs = Reqs::new(&e, record.clone());

    // BASELINE: no worker thread exists.
    let before = snapshot(&ws, &reqs);

    // The baseline must have real data to make the comparison meaningful: a
    // managed node in the tree, the record listed, and the record readable.
    assert!(before.get_tree["root"]["children"].as_array().is_some_and(|c| !c.is_empty()), "baseline tree has a source root");
    assert_eq!(before.fabric_list.records.len(), 1, "baseline lists the one record");
    assert!(before.fabric_list.backend.is_none(), "no heartbeat before the worker runs");
    assert!(before.read_text.contains("a steady reminder"), "record is readable");
    assert!(before.write_ok, "baseline write succeeds");

    // WITH the worker: run a real `Runner` on a background thread. It writes the
    // heartbeat but, because the record is a parked Reminder with no delegate
    // request, it does not mutate the record the Data_API reads.
    let stop = Arc::new(AtomicBool::new(false));
    let worker_ws = workspace(&e);
    let ctx = e.ctx.clone();
    let st = stop.clone();
    let handle = std::thread::spawn(move || {
        let mut runner = Runner::new(&ctx, worker_ws, Options { interval_s: 1, once: false }, Sink::default()).unwrap();
        let _ = runner.run(&st);
    });

    // Wait until the worker has written `_backend.yaml`, confirming it is live.
    let heartbeat = e.ctx.space().join("registry").join("_backend.yaml");
    let started = std::time::Instant::now();
    while !heartbeat.exists() {
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "worker never wrote a heartbeat");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    // One more interval so any (non-)driving tick has surely run.
    std::thread::sleep(std::time::Duration::from_millis(300));

    let after = snapshot(&ws, &reqs);

    stop.store(true, Ordering::SeqCst);
    handle.join().unwrap();

    // get_tree / get_stamp are Workspace node scans that never touch
    // `.fabric-space`; they must be byte-for-byte identical.
    assert_eq!(before.get_tree, after.get_tree, "get_tree identical with and without the worker");
    assert_eq!(before.get_stamp, after.get_stamp, "get_stamp identical with and without the worker");

    // fabric_read of the stable record and the fabric_write disposition are
    // identical. (`mtime` is excluded by design — it is a wall-clock value that
    // every write bumps, independent of the worker.)
    assert_eq!(before.read_text, after.read_text, "fabric_read text identical with and without the worker");
    assert_eq!(before.write_ok, after.write_ok, "fabric_write disposition identical with and without the worker");

    // fabric_list: the set of paths it reports — records, registry, events — is
    // identical. The per-file `mtime` is a wall-clock value that each snapshot's
    // own `fabric_write` bumps on the record (independent of the worker), so it
    // is compared by path, not by raw stamp. The one acceptable shape difference
    // is the heartbeat `backend` entry: absent before the worker, present after.
    // That presence signal is not a Data_API contract change.
    assert_eq!(paths(&before.fabric_list.records), paths(&after.fabric_list.records), "fabric_list records identical");
    assert_eq!(paths(&before.fabric_list.registry), paths(&after.fabric_list.registry), "fabric_list registry identical");
    assert_eq!(
        before.fabric_list.events.as_ref().map(|f| &f.path),
        after.fabric_list.events.as_ref().map(|f| &f.path),
        "fabric_list events identical",
    );
    assert!(after.fabric_list.backend.is_some(), "heartbeat is reported once the worker runs");
    assert!(
        after.fabric_list.backend.as_ref().unwrap().path.ends_with("registry/_backend.yaml"),
        "the reported backend is the heartbeat file",
    );
}
