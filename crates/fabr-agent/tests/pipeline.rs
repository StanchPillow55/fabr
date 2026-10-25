use fabr_agent::lifecycle::advance;
use fabr_agent::link::{delegate, open_reminder};
use fabr_agent::model::{DraftAssertion, DraftCheck, PlanDraft, ScriptedModel, ToolCall};
use fabr_agent::run_once;
use fabr_core::taskrec::{self, State, Step};
use fabr_core::todo::{Item, Status};
use fabr_core::{NodeRef, SaveTodoReq, Workspace};
use std::path::{Path, PathBuf};

fn workspace(root: &Path) -> Workspace {
    let cfg = root.join("cfg");
    let cache = root.join("cache");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    Workspace::from_config(&cfg, &cache, Some(root.to_path_buf())).unwrap()
}

fn setup() -> (tempfile::TempDir, PathBuf, Workspace) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    let ws = workspace(&root);
    ws.init_node(NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let fabric = tmp.path().join("fabric-space");
    std::fs::create_dir_all(&fabric).unwrap();
    std::fs::write(
        fabric.join("settings.toml"),
        "default_folder = \"/\"\nmodel = \"claude-sonnet-5-5\"\nheartbeat_interval_s = 10\n\n[setup]\ncompleted_steps = [\"folder\", \"api_key\", \"model\"]\n",
    )
    .unwrap();
    std::fs::write(fabric.join("secrets.toml"), "anthropic_api_key = \"test-key\"\n").unwrap();
    (tmp, fabric, ws)
}

fn item(title: &str) -> Item {
    Item { id: None, status: Status::Todo, due: None, title: title.into(), pri: None }
}

fn draft(plan: &str, tools: &[&str], check: (&str, &[&str])) -> PlanDraft {
    PlanDraft {
        spec: "Write greet.txt containing hi".into(),
        success: "greet.txt contains hi".into(),
        plan: plan.into(),
        assertions: vec![DraftAssertion { id: "a1".into(), text: "greet.txt contains hi".into() }],
        reversibility: "reversible".into(),
        tools: tools.iter().map(|t| (*t).to_string()).collect(),
        checks: vec![DraftCheck { cmd: check.0.into(), args: check.1.iter().map(|a| (*a).to_string()).collect() }],
    }
}

fn greet_plan() -> PlanDraft {
    draft("1. Write greet.txt", &["write_file"], ("test", &["-f", "greet.txt"]))
}

fn write_call() -> ToolCall {
    ToolCall { name: "write_file".into(), path: "greet.txt".into(), content: Some("hi".into()) }
}

fn record_path(root: &Path) -> PathBuf {
    let tasks = root.join(".fabric-space").join("tasks");
    std::fs::read_dir(&tasks).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == "md")).unwrap()
}

fn load(path: &Path) -> taskrec::TaskRecord {
    taskrec::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn approve(path: &Path) {
    let mut rec = load(path);
    let approval = rec.approval.as_mut().unwrap();
    approval.status = "approved".into();
    approval.by = Some("gui".into());
    approval.at = Some("2026-10-24T00:00:00Z".into());
    let expected = rec.updated_at.clone();
    taskrec::write_record(path, &rec, Some(&expected)).unwrap();
}

#[test]
fn fixture_gather_persists_through_the_record_writer() {
    let (_tmp, _fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/01-reminder/project/.fabric-space/tasks/2026-10-24-check-printer-firmware.md");
    let text = std::fs::read_to_string(&fixture).unwrap();
    let rec = taskrec::parse(&text).unwrap();
    let path = root.join(".fabric-space").join("tasks").join(format!("{}.md", rec.id));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    taskrec::write_record(&path, &rec, None).unwrap();
    let model = ScriptedModel::new(greet_plan(), vec![write_call()]);
    advance(&ws, "local", "", &root, &path, &root, 10, &model).unwrap();
    advance(&ws, "local", "", &root, &path, &root, 10, &model).unwrap();
    let again = load(&path);
    assert_eq!(again.state, State::Task);
    assert_eq!(again.step, Some(Step::Define));
    assert!(again.context.contains("Release notes are on the maker site."));
    assert!(again.context.contains("### Gathered"));
    let round = taskrec::parse(&taskrec::render(&again)).unwrap();
    assert_eq!(round.id, again.id);
    assert_eq!(round.context, again.context);
}

#[test]
fn delegating_twice_keeps_one_record_and_the_todo_id() {
    let (_tmp, _fabric, ws) = setup();
    let first = delegate(&ws, "local", "", &item("alpha")).unwrap();
    let saved = ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let second = delegate(&ws, "local", "", &saved.items[0]).unwrap();
    assert_eq!(first, second);
    let node = ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let id = node.items[0].id.clone().unwrap();
    let version = node.version.clone().unwrap();
    ws.save_todo(SaveTodoReq { source: "local".into(), path: String::new(), version, items: node.items }).unwrap();
    let again = ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap();
    assert_eq!(again.items[0].id.as_deref(), Some(id.as_str()));
}

#[test]
fn registry_names_differ_for_the_same_slug() {
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let a = left.path().join(".fabric-space").join("tasks").join("2026-10-24-alpha.md");
    let b = right.path().join(".fabric-space").join("tasks").join("2026-10-24-alpha.md");
    let name_a = taskrec::registry_filename("2026-10-24-alpha", &a.to_string_lossy());
    let name_b = taskrec::registry_filename("2026-10-24-alpha", &b.to_string_lossy());
    assert_ne!(name_a, name_b);
}

#[test]
fn planner_stops_at_pending_approval() {
    let (_tmp, fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let model = ScriptedModel::new(greet_plan(), vec![write_call()]);
    ws.save_todo(SaveTodoReq {
        source: "local".into(),
        path: String::new(),
        version: ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap().version.unwrap(),
        items: vec![item("add greet.txt that contains hi")],
    })
    .unwrap();
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let rec = load(&record_path(&root));
    assert_eq!(rec.state, State::Defined);
    assert_eq!(rec.step, Some(Step::Approve));
    assert!(rec.waiting);
    assert!(!rec.spec.trim().is_empty());
    assert!(!rec.success.trim().is_empty());
    assert!(!rec.plan.trim().is_empty());
    assert!(!rec.assertions.is_empty());
    assert!(rec.plan_meta.as_ref().unwrap().checks.len() == 1);
    assert_eq!(rec.plan_meta.as_ref().unwrap().reversibility.as_deref(), Some("reversible"));
    let digest = rec.approval.as_ref().unwrap().digest.clone().unwrap();
    assert_eq!(digest, taskrec::approval_digest(&rec));
    assert_eq!(rec.approval.as_ref().unwrap().status, "pending");
    assert_eq!(model.execute_count(), 0);
    assert!(!root.join("greet.txt").exists());
}

#[test]
fn define_question_pauses_without_a_new_state() {
    let (_tmp, fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let model = ScriptedModel::with_question(greet_plan(), "Which filename?");
    ws.save_todo(SaveTodoReq {
        source: "local".into(),
        path: String::new(),
        version: ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap().version.unwrap(),
        items: vec![item("ask first")],
    })
    .unwrap();
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let rec = load(&record_path(&root));
    assert_eq!(rec.state, State::Task);
    assert_eq!(rec.step, Some(Step::Define));
    assert!(rec.waiting);
    assert_eq!(rec.question.as_ref().unwrap().text, "Which filename?");
    assert!(rec.question.as_ref().unwrap().answer.is_none());
    assert!(rec.spec.trim().is_empty());
    assert!(rec.plan.trim().is_empty());
    assert_eq!(model.execute_count(), 0);
}

#[test]
fn unsupported_tool_does_not_execute() {
    let (_tmp, fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let model = ScriptedModel::new(draft("1. Open the shop", &["browser"], ("test", &["-f", "greet.txt"])), vec![write_call()]);
    ws.save_todo(SaveTodoReq {
        source: "local".into(),
        path: String::new(),
        version: ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap().version.unwrap(),
        items: vec![item("browse")],
    })
    .unwrap();
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let path = record_path(&root);
    approve(&path);
    let err = run_once(&ws, "local", "", &fabric, &model).unwrap_err();
    assert!(err.message().contains("unsupported tool browser"));
    assert_eq!(model.execute_count(), 0);
    assert_eq!(load(&path).state, State::Defined);
}

#[test]
fn approval_refuses_pending_refused_and_stale_digest() {
    let (_tmp, fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let model = ScriptedModel::new(greet_plan(), vec![write_call()]);
    ws.save_todo(SaveTodoReq {
        source: "local".into(),
        path: String::new(),
        version: ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap().version.unwrap(),
        items: vec![item("gate")],
    })
    .unwrap();
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let path = record_path(&root);
    assert_eq!(load(&path).approval.as_ref().unwrap().status, "pending");
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    assert_eq!(load(&path).state, State::Defined);
    assert_eq!(model.execute_count(), 0);

    let mut rec = load(&path);
    rec.approval.as_mut().unwrap().status = "refused".into();
    rec.approval.as_mut().unwrap().note = Some("shorter plan".into());
    let expected = rec.updated_at.clone();
    taskrec::write_record(&path, &rec, Some(&expected)).unwrap();
    model.push_plan(draft("1. Write greet.txt differently", &["write_file"], ("test", &["-f", "greet.txt"])));
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let redrafted = load(&path);
    assert_eq!(redrafted.state, State::Defined);
    assert_eq!(redrafted.approval.as_ref().unwrap().status, "pending");
    assert!(redrafted.plan.contains("differently"));
    assert_eq!(redrafted.approval.as_ref().unwrap().digest.as_deref(), Some(taskrec::approval_digest(&redrafted).as_str()));
    assert!(redrafted.log.contains("refused: shorter plan"));

    approve(&path);
    let mut stale = load(&path);
    stale.plan.push_str("\nextra");
    let expected = stale.updated_at.clone();
    taskrec::write_record(&path, &stale, Some(&expected)).unwrap();
    let err = advance(&ws, "local", "", &root, &path, &fabric, 10, &model).unwrap_err();
    assert!(err.message().contains("digest"));
    assert_eq!(load(&path).state, State::Defined);
    assert_eq!(model.execute_count(), 0);
}

#[test]
fn executor_does_not_set_the_verdict() {
    let (_tmp, fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let model = ScriptedModel::new(greet_plan(), vec![write_call()]);
    ws.save_todo(SaveTodoReq {
        source: "local".into(),
        path: String::new(),
        version: ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap().version.unwrap(),
        items: vec![item("split")],
    })
    .unwrap();
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let path = record_path(&root);
    approve(&path);
    advance(&ws, "local", "", &root, &path, &fabric, 10, &model).unwrap();
    let started = load(&path);
    assert_eq!(started.state, State::InProgress);
    assert_eq!(started.step, Some(Step::Execute));
    assert!(started.verdict.is_none());
    advance(&ws, "local", "", &root, &path, &fabric, 10, &model).unwrap();
    let executed = load(&path);
    assert_eq!(executed.state, State::InProgress);
    assert_eq!(executed.step, Some(Step::Evaluate));
    assert!(executed.verdict.is_none());
    assert_eq!(std::fs::read_to_string(root.join("greet.txt")).unwrap(), "hi");
    advance(&ws, "local", "", &root, &path, &fabric, 10, &model).unwrap();
    let judged = load(&path);
    assert_eq!(judged.verdict.as_ref().unwrap().result, "pass");
    assert_eq!(judged.state, State::Succeeded);
    assert_eq!(judged.verdict.as_ref().unwrap().items[0].id, "a1");
}

#[test]
fn failed_check_does_not_succeed() {
    let (_tmp, fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let mut plan = greet_plan();
    plan.assertions = vec![DraftAssertion { id: "a1".into(), text: "the file is present".into() }];
    plan.checks = vec![DraftCheck { cmd: "test".into(), args: vec!["-f".into(), "missing-file".into()] }];
    let model = ScriptedModel::new(plan, vec![write_call()]);
    ws.save_todo(SaveTodoReq {
        source: "local".into(),
        path: String::new(),
        version: ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap().version.unwrap(),
        items: vec![item("fail check")],
    })
    .unwrap();
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let path = record_path(&root);
    approve(&path);
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let rec = load(&path);
    assert_eq!(rec.state, State::Failed);
    assert_eq!(rec.verdict.as_ref().unwrap().result, "fail");
    let node = ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap();
    assert_eq!(node.items[0].status, Status::Wip);
}

#[test]
fn restart_does_not_write_the_file_twice() {
    let (_tmp, fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let model = ScriptedModel::new(
        greet_plan(),
        vec![ToolCall { name: "write_file".into(), path: "greet.txt".into(), content: Some("HI-AGAIN".into()) }],
    );
    let path = delegate(&ws, "local", "", &item("resume")).unwrap();
    let id = load(&path).id.clone();
    std::fs::write(root.join("greet.txt"), "hi").unwrap();
    let mut rec = load(&path);
    rec.state = State::InProgress;
    rec.step = Some(Step::Execute);
    rec.spec = "Write greet.txt containing hi".into();
    rec.plan = "1. Write greet.txt".into();
    rec.log = "- 2026-10-24T00:00:00Z execute: write_file greet.txt\n".into();
    rec.plan_meta = Some(taskrec::PlanMeta {
        reversibility: Some("reversible".into()),
        tools: vec!["write_file".into()],
        assumptions: Vec::new(),
        checks: vec![taskrec::Check { cmd: "test".into(), args: vec!["-f".into(), "greet.txt".into()] }],
    });
    let expected = rec.updated_at.clone();
    taskrec::write_record(&path, &rec, Some(&expected)).unwrap();
    advance(&ws, "local", "", &root, &path, &fabric, 10, &model).unwrap();
    assert_eq!(std::fs::read_to_string(root.join("greet.txt")).unwrap(), "hi");
    let after = load(&path);
    assert_eq!(after.id, id);
    assert_eq!(after.log.matches("write_file greet.txt").count(), 1);
    assert_eq!(after.step, Some(Step::Evaluate));
    assert!(after.verdict.is_none());
    let tasks = root.join(".fabric-space").join("tasks");
    let files: Vec<_> = std::fs::read_dir(&tasks).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "md")).collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn reminder_without_a_todo_still_runs() {
    let (_tmp, fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let path = open_reminder(&root, "standalone").unwrap();
    assert!(load(&path).source.is_none());
    let model = ScriptedModel::new(greet_plan(), vec![write_call()]);
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let rec = load(&path);
    assert_eq!(rec.state, State::Defined);
    assert!(rec.source.is_none());
    assert_eq!(rec.approval.as_ref().unwrap().status, "pending");
}

#[test]
fn end_to_end_greet_file() {
    let (_tmp, fabric, ws) = setup();
    let root = ws.local_path(&NodeRef { source: "local".into(), path: String::new() }).unwrap();
    let model = ScriptedModel::new(greet_plan(), vec![write_call()]);
    ws.save_todo(SaveTodoReq {
        source: "local".into(),
        path: String::new(),
        version: ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap().version.unwrap(),
        items: vec![item("add greet.txt that contains hi")],
    })
    .unwrap();
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let path = record_path(&root);
    let drafted = load(&path);
    assert!(drafted.source.is_some());
    assert!(!drafted.id.is_empty());
    approve(&path);
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let rec = load(&path);
    assert_eq!(std::fs::read_to_string(root.join("greet.txt")).unwrap(), "hi");
    assert_eq!(rec.state, State::Succeeded);
    assert_eq!(rec.verdict.as_ref().unwrap().result, "pass");
    assert!(rec.step.is_none());
    assert!(rec.log.contains("learn: deferred"));
    assert!(rec.log.contains("notify: result succeeded"));
    let registry = fabric.join("registry");
    let registry_files = std::fs::read_dir(&registry).map(|rd| rd.flatten().count()).unwrap_or(0);
    assert_eq!(registry_files, 0);
    let node = ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap();
    assert_eq!(node.items[0].status, Status::Done);
    let id = node.items[0].id.clone();
    run_once(&ws, "local", "", &fabric, &model).unwrap();
    let final_node = ws.get_node(NodeRef { source: "local".into(), path: String::new() }).unwrap();
    assert_eq!(final_node.items[0].id, id);
    assert_eq!(final_node.items[0].status, Status::Done);
    assert_eq!(load(&path).state, State::Succeeded);
}
