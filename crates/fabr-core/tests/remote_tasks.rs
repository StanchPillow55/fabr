mod common;
use common::fake_ssh;
use fabr_core::remote::ssh;
use fabr_core::taskrec::{self, State};
use fabr_core::*;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

fn fake_home() -> &'static Path {
    static HOME: OnceLock<tempfile::TempDir> = OnceLock::new();
    HOME.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let reg = d.path().join(".fabric-space/registry");
        std::fs::create_dir_all(&reg).unwrap();
        std::fs::write(reg.join("alpha.yaml"), "name: alpha\n").unwrap();
        std::fs::write(reg.join("_backend.yaml"), "beat: 1\n").unwrap();
        std::fs::write(d.path().join(".fabric-space/events.log"), "one\ntwo\n").unwrap();
        unsafe { std::env::set_var("HOME", d.path()) };
        d
    })
    .path()
}

fn setup() -> (tempfile::TempDir, std::path::PathBuf, Workspace) {
    fake_home();
    secrets::use_memory_store();
    ssh::set_ssh_program(Some(fake_ssh()));
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("box root");
    write_fixture_tree(&root).unwrap();
    let ws = Workspace::from_config(&tmp.path().join("cfg"), &tmp.path().join("cache"), None).unwrap();
    let src = Source {
        id: "r".into(),
        name: "Remote".into(),
        host: Some("fake".into()),
        root: root.display().to_string(),
        prune: None,
        search_depth: None,
        password: None,
    };
    ws.save_sources(vec![src]).unwrap();
    (tmp, root, ws)
}

fn call(ws: &Workspace, op: &str, body: Value) -> Result<Value, FabrError> {
    match dispatch(ws, op, body)? {
        Done::Json(v) => Ok(v),
        Done::Desktop(_) => panic!("desktop"),
    }
}

fn list(ws: &Workspace) -> Value {
    for _ in 0..50 {
        let v = call(ws, "fabric_remote_list", json!({"source": "r"})).unwrap();
        if v["stamp"] != "pending" {
            return v;
        }
        ws.wait_remote(Duration::from_secs(30));
    }
    panic!("never loaded")
}

fn record_of(v: &Value, needle: &str) -> (String, String) {
    let r = v["records"].as_array().unwrap().iter().find(|r| r["path"].as_str().unwrap().contains(needle)).unwrap();
    (r["path"].as_str().unwrap().to_string(), r["text"].as_str().unwrap().to_string())
}

fn on_disk(root: &Path, path: &str) -> String {
    std::fs::read_to_string(root.join(path.strip_prefix(&format!("{}/", root.display())).unwrap())).unwrap()
}

#[test]
fn remote_list_carries_records_and_home_state() {
    let (_t, root, ws) = setup();
    let v = list(&ws);
    let prefix = format!("{}/work/.fabric-space/tasks/", root.display());
    let recs = v["records"].as_array().unwrap();
    assert_eq!(recs.iter().filter(|r| r["path"].as_str().unwrap().starts_with(&prefix)).count(), 2);
    assert!(recs.iter().all(|r| r["path"].as_str().unwrap().ends_with(".md")));
    assert_eq!(v["registry"], json!([{"name": "alpha.yaml", "text": "name: alpha\n"}]));
    assert_eq!(v["backend"], "beat: 1\n");
    assert_eq!(v["events"], "one\ntwo\n");
    let stamp = v["stamp"].as_str().unwrap();
    assert_eq!(stamp.len(), 40);
    assert_eq!(call(&ws, "fabric_remote_stamp", json!({"source": "r"})).unwrap()["stamp"], stamp);
    assert_eq!(call(&ws, "fabric_remote_list", json!({"source": "nope"})).unwrap_err().code(), 404);
}

#[test]
fn remote_write_ticks_conflicts_and_refreshes() {
    let (_t, root, ws) = setup();
    let v = list(&ws);
    let (path, text) = v["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["path"].as_str().unwrap().to_string(), r["text"].as_str().unwrap().to_string()))
        .find(|(_, t)| taskrec::parse(t).unwrap().state == State::Reminder)
        .unwrap();
    let rec = taskrec::parse(&text).unwrap();
    let mut done = rec.clone();
    done.state = State::Succeeded;
    done.updated_at = "2099-01-01T00:00:00Z".into();
    let body = |text: &str, expected: &str| json!({"source": "r", "path": path, "text": text, "expected_updated_at": expected});
    let mut paused = rec.clone();
    paused.paused = true;
    let e = call(&ws, "fabric_remote_write", body(&taskrec::render(&paused), &rec.updated_at)).unwrap_err();
    assert_eq!(e.code(), 400);
    let before = on_disk(&root, &path);
    let e = call(&ws, "fabric_remote_write", body(&taskrec::render(&done), "1999-01-01T00:00:00Z")).unwrap_err();
    assert_eq!(e.code(), 409);
    assert_eq!(on_disk(&root, &path), before);
    let ok = call(&ws, "fabric_remote_write", body(&taskrec::render(&done), &rec.updated_at)).unwrap();
    assert_eq!(ok["updated_at"], "2099-01-01T00:00:00Z");
    assert_eq!(taskrec::parse(&on_disk(&root, &path)).unwrap().state, State::Succeeded);
    let (_, fresh) = record_of(&list(&ws), &path);
    assert_eq!(taskrec::parse(&fresh).unwrap().state, State::Succeeded);
    let again = call(&ws, "fabric_remote_write", body(&taskrec::render(&done), &rec.updated_at)).unwrap_err();
    assert_eq!(again.code(), 409);
    for bad in [format!("{}/work/../x/.fabric-space/tasks/a.md", root.display()), format!("{}/work/notes.md", root.display())] {
        let e = call(&ws, "fabric_remote_write", json!({"source": "r", "path": bad, "text": "x", "expected_updated_at": ""})).unwrap_err();
        assert_eq!(e.code(), 400, "{bad}");
    }
    let e = call(
        &ws,
        "fabric_remote_write",
        json!({"source": "r", "path": "/elsewhere/.fabric-space/tasks/a.md", "text": "x", "expected_updated_at": ""}),
    );
    assert_eq!(e.unwrap_err().code(), 400);
}

#[test]
fn remote_create_retries_collisions_and_makes_folders() {
    let (_t, root, ws) = setup();
    list(&ws);
    let folder = format!("{}/personal/finance/receipts/", root.display());
    let make = |title: &str| {
        call(&ws, "fabric_remote_create", json!({"source": "r", "folder": folder, "title": title, "pri": 2, "context": "why"}))
    };
    let a = make("Pay rent").unwrap();
    let a = a["path"].as_str().unwrap().to_string();
    assert!(a.starts_with(&format!("{}/personal/finance/receipts/.fabric-space/tasks/", root.display())), "{a}");
    let text = on_disk(&root, &a);
    let rec = taskrec::parse(&text).unwrap();
    assert_eq!((rec.title.as_str(), rec.pri, rec.state), ("Pay rent", 2, State::Reminder));
    assert!(text.contains("## Context\nwhy\n"));
    let b = make("Pay rent").unwrap()["path"].as_str().unwrap().to_string();
    assert!(b.ends_with("-2.md"), "{b}");
    std::fs::write(Path::new(&on_disk_path(&root, &b)).with_file_name(format!("{}-3.md", leaf_id(&a))), "x").unwrap();
    let c = make("Pay rent").unwrap()["path"].as_str().unwrap().to_string();
    assert!(c.ends_with("-4.md"), "{c}");
    let paths: Vec<String> = list(&ws)["records"].as_array().unwrap().iter().map(|r| r["path"].as_str().unwrap().to_string()).collect();
    assert!(paths.contains(&a) && paths.contains(&b));
    assert_eq!(make("   ").unwrap_err().code(), 400);
    let e = call(&ws, "fabric_remote_create", json!({"source": "r", "folder": format!("{}/missing", root.display()), "title": "t"}));
    assert_eq!(e.unwrap_err().code(), 404);
    let at_root = call(&ws, "fabric_remote_create", json!({"source": "r", "folder": root.display().to_string(), "title": "Top"})).unwrap();
    assert!(at_root["path"].as_str().unwrap().starts_with(&format!("{}/.fabric-space/tasks/", root.display())));
}

fn on_disk_path(root: &Path, path: &str) -> String {
    root.join(path.strip_prefix(&format!("{}/", root.display())).unwrap()).display().to_string()
}

fn leaf_id(path: &str) -> String {
    let f = path.rsplit('/').next().unwrap();
    f.strip_suffix(".md").unwrap().to_string()
}
