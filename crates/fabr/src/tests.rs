use crate::server;
use fabr_core::{Workspace, write_sample};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;

struct Srv {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    port: u16,
}

fn start() -> Srv {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("sample");
    write_sample(&root).unwrap();
    let ui = tmp.path().join("ui");
    std::fs::create_dir_all(ui.join("assets")).unwrap();
    std::fs::write(ui.join("index.html"), "<title>fabr</title>").unwrap();
    std::fs::write(ui.join("assets/app.js"), "console.log(1)").unwrap();
    std::fs::write(tmp.path().join("secret.txt"), "secret").unwrap();
    let ws = Workspace::from_config(&tmp.path().join("cfg"), &tmp.path().join("cache"), Some(root.clone())).unwrap();
    let (srv, port) = server::bind(0).unwrap();
    server::spawn_workers(srv, Arc::new(ws), Arc::new(server::Ui::new(Some(ui))), 2);
    Srv { _tmp: tmp, root, port }
}

impl Srv {
    fn raw(&self, method: &str, path: &str, body: Option<&[u8]>, headers: &[(&str, &str)]) -> (u16, String, Vec<u8>) {
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
        if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
            req.push_str(&format!("Host: 127.0.0.1:{}\r\n", self.port));
        }
        for (k, v) in headers {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        if let Some(b) = body {
            req.push_str(&format!("Content-Length: {}\r\n", b.len()));
        }
        req.push_str("\r\n");
        s.write_all(req.as_bytes()).unwrap();
        if let Some(b) = body {
            let _ = s.write_all(b);
        }
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        let split = out.windows(4).position(|w| w == b"\r\n\r\n").expect("response head");
        let head = String::from_utf8_lossy(&out[..split]).into_owned();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, head, out[split + 4..].to_vec())
    }

    fn get(&self, path: &str) -> (u16, Value) {
        let (s, _, b) = self.raw("GET", path, None, &[]);
        (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }

    fn post(&self, path: &str, body: &Value, headers: &[(&str, &str)]) -> (u16, Value) {
        let mut h = vec![("Content-Type", "application/json")];
        h.extend_from_slice(headers);
        let (s, _, b) = self.raw("POST", path, Some(body.to_string().as_bytes()), &h);
        (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }
}

#[test]
fn tree_and_static() {
    let srv = start();
    let (status, body) = srv.get("/api/get_tree");
    assert_eq!(status, 200);
    assert_eq!(
        (body["root"]["name"].as_str(), body["root"]["source"].as_str(), body["root"]["managed"].as_bool()),
        (Some("/"), Some(""), Some(false))
    );
    assert_eq!(body["root"]["children"].as_array().unwrap().len(), 1);
    assert_eq!(body["root"]["children"][0]["source"], "local");
    assert_eq!(body["root"]["children"][0]["meta"]["name"], "fabr");
    assert_eq!(body["sources"][0]["id"], "local");
    let (s, head, b) = srv.raw("GET", "/", None, &[]);
    assert_eq!(s, 200);
    assert!(String::from_utf8_lossy(&b).contains("fabr"));
    assert!(head.to_ascii_lowercase().contains("content-type: text/html"));
    let (s, head, _) = srv.raw("GET", "/assets/app.js", None, &[]);
    assert_eq!(s, 200);
    assert!(head.contains("text/javascript"));
    for bad in ["/../secret.txt", "/assets/../../secret.txt", "/%2e%2e/secret.txt", "/nope.js"] {
        assert_eq!(srv.raw("GET", bad, None, &[]).0, 404, "{bad}");
    }
}

#[test]
fn show_all_query() {
    let srv = start();
    std::fs::create_dir_all(srv.root.join("research/ideas/drafts")).unwrap();
    let count = |v: &Value| v.to_string().matches("research/ideas/drafts").count();
    assert_eq!(count(&srv.get("/api/get_tree?show_all=0").1), 0);
    assert_eq!(count(&srv.get("/api/get_tree?show_all=1").1), 1);
}

#[test]
fn save_conflict_and_stamp() {
    let srv = start();
    let (_, s1) = srv.get("/api/get_stamp");
    let (_, tree) = srv.get("/api/get_tree");
    let work = tree["root"]["children"][0]["children"].as_array().unwrap().iter().find(|c| c["path"] == "work").unwrap().clone();
    let mut items = work["items"].as_array().unwrap().clone();
    items.push(json!({"status": "todo", "due": null, "title": "from test"}));
    let req = json!({"source": "local", "path": "work", "version": work["version"], "items": items});
    let (status, body) = srv.post("/api/save_todo", &req, &[]);
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["items"].as_array().unwrap().last().unwrap()["title"], "from test");
    assert!(std::fs::read_to_string(srv.root.join("work/.fabr/.todo")).unwrap().contains("from test"));
    let (_, s2) = srv.get("/api/get_stamp");
    assert_ne!(s1["stamp"], s2["stamp"]);
    let (status, body) = srv.post("/api/save_todo", &req, &[]);
    assert_eq!(status, 409);
    assert_eq!(body["code"], 409);
    assert_eq!(body["node"]["path"], "work");
    assert!(body["message"].is_string());
}

#[test]
fn meta_init_sources_settings() {
    let srv = start();
    let (_, tree) = srv.get("/api/get_tree");
    let v = tree["root"]["children"][0]["version"].clone();
    let (s, n) = srv.post(
        "/api/save_meta",
        &json!({"source": "local", "path": "", "version": v, "meta": {"name": "renamed", "show_depth": 1}}),
        &[],
    );
    assert_eq!(s, 200, "{n}");
    assert_eq!(n["meta"]["name"], "renamed");
    let (s, n) = srv.post("/api/init_node", &json!({"source": "local", "path": "personal/finance/receipts"}), &[]);
    assert_eq!((s, n["managed"].clone()), (200, json!(true)));
    let (s, n) = srv.post("/api/init_node", &json!({"source": "local", "path": "personal/finance/receipts"}), &[]);
    assert_eq!(s, 409);
    assert_eq!(n["node"]["managed"], true);
    let (s, l) = srv.get("/api/list_sources");
    assert_eq!((s, l[0]["id"].clone()), (200, json!("local")));
    let (s, st) = srv.get("/api/get_settings");
    assert_eq!((s, st["poll_ms"].clone()), (200, json!(2000)));
    let (s, st) = srv.post("/api/save_settings", &json!({"poll_ms": 3000, "remote_poll_ms": 40000}), &[]);
    assert_eq!((s, st["poll_ms"].clone()), (200, json!(3000)));
    let (s, _) = srv.post("/api/save_settings", &json!({"poll_ms": 1, "remote_poll_ms": 40000}), &[]);
    assert_eq!(s, 400);
    let src = json!({"id": "local", "name": "L", "host": null, "root": srv.root.display().to_string(), "prune": null});
    let (s, t) = srv.post("/api/test_source", &src, &[]);
    assert_eq!((s, t["ok"].clone(), t["node_count"].clone()), (200, json!(true), json!(19)));
    let (s, out) = srv.post("/api/save_sources", &json!({"sources": [src]}), &[]);
    assert_eq!((s, out[0]["name"].clone()), (200, json!("L")));
    let (s, _) = srv.post("/api/save_sources", &json!({"sources": [{"id": "BAD", "name": "", "root": "/"}]}), &[]);
    assert_eq!(s, 400);
    let (s, e) = srv.post("/api/open_path", &json!({"source": "local", "path": ""}), &[]);
    assert_eq!((s, e["code"].clone()), (501, json!(501)));
}

#[test]
fn rejects_bad_requests() {
    let srv = start();
    let (status, body) = srv.post("/api/save_todo", &json!({"source": "local", "path": "../", "version": "x", "items": []}), &[]);
    assert_eq!(status, 400);
    assert_eq!(body["code"], 400);
    let ok = json!({"source": "local", "path": "work", "version": "x", "items": []});
    let (status, _) = srv.post("/api/save_todo", &ok, &[("Origin", "http://evil.example")]);
    assert_eq!(status, 403);
    let origin = format!("http://127.0.0.1:{}", srv.port);
    let (status, _) = srv.post("/api/save_todo", &ok, &[("Origin", origin.as_str())]);
    assert_eq!(status, 409);
    let (status, _) = srv.post("/api/save_todo", &ok, &[("Origin", "null")]);
    assert_eq!(status, 403);
    let (s, _, _) = srv.raw("GET", "/api/get_tree", None, &[("Host", "evil.example")]);
    assert_eq!(s, 403);
    let host = format!("localhost:{}", srv.port);
    let (s, _, _) = srv.raw("GET", "/api/get_stamp", None, &[("Host", host.as_str())]);
    assert_eq!(s, 200);
    let (s, _, _) = srv.raw("POST", "/api/save_todo", Some(ok.to_string().as_bytes()), &[("Content-Type", "text/plain")]);
    assert_eq!(s, 415);
    let (s, _, _) = srv.raw("POST", "/api/save_todo", Some(b"{}"), &[]);
    assert_eq!(s, 415);
    let big = vec![b' '; server::MAX_BODY + 10];
    let (s, _, _) = srv.raw("POST", "/api/save_todo", Some(&big), &[("Content-Type", "application/json")]);
    assert_eq!(s, 413);
    let (s, _, _) = srv.raw("POST", "/api/save_todo", Some(b"{nope"), &[("Content-Type", "application/json; charset=utf-8")]);
    assert_eq!(s, 400);
    let (s, _) = srv.post(
        "/api/save_todo",
        &json!({"source": "local", "path": "work", "version": "x", "items": [{"status": "nope", "title": "x"}]}),
        &[],
    );
    assert_eq!(s, 400);
    assert_eq!(srv.get("/api/nope").0, 404);
    assert_eq!(srv.get("/api/save_todo").0, 405);
    assert_eq!(srv.post("/api/get_tree", &json!({}), &[]).0, 405);
}

#[test]
fn host_and_origin_helpers() {
    assert!(server::host_is_loopback("127.0.0.1:8080"));
    assert!(server::host_is_loopback("localhost"));
    assert!(server::host_is_loopback("[::1]:80"));
    assert!(!server::host_is_loopback("example.com:80"));
    assert!(!server::host_is_loopback("127.0.0.1.evil.com"));
    assert!(server::origin_matches("http://127.0.0.1:5", "127.0.0.1:5"));
    assert!(!server::origin_matches("http://127.0.0.1:6", "127.0.0.1:5"));
    assert!(!server::origin_matches("null", "127.0.0.1:5"));
}

#[test]
fn list_dirs_init_children_and_rescan() {
    let srv = start();
    let (s, l) = srv.post("/api/list_dirs", &json!({"source": "local", "path": "work/frontend"}), &[]);
    assert_eq!(s, 200, "{l}");
    assert!(l.as_array().unwrap().iter().any(|d| d == &json!({"name": "node_modules", "managed": false, "pruned": true})));
    let (s, e) = srv.post("/api/list_dirs", &json!({"source": "local", "path": "nope"}), &[]);
    assert_eq!((s, e["code"].clone()), (404, json!(404)));
    assert_eq!(srv.get("/api/list_dirs").0, 405);
    std::fs::create_dir_all(srv.root.join("personal/finance/receipts/2025")).unwrap();
    let body = json!({"source": "local", "path": "personal/finance/receipts", "names": ["2025", "2026"]});
    let (s, n) = srv.post("/api/init_children", &body, &[]);
    assert_eq!(s, 200, "{n}");
    assert_eq!((n["managed"].clone(), n["meta"]["show_depth"].clone()), (json!(true), json!(1)));
    assert_eq!(n["children"].as_array().unwrap().len(), 2);
    assert!(n["children"].as_array().unwrap().iter().all(|c| c["managed"] == true));
    let (s, e) = srv.post("/api/init_children", &json!({"source": "local", "path": "", "names": ["../x", "nope"]}), &[]);
    assert_eq!(s, 400);
    assert!(e["message"].as_str().unwrap().contains("\"nope\""));
    std::fs::create_dir_all(srv.root.join("zz/a/b")).unwrap();
    let (_, t) = srv.get("/api/get_tree?show_all=0&rescan=1");
    assert!(t["truncated_reason"].is_null());
    assert!(t["truncated_by"].is_null());
    std::fs::create_dir_all(srv.root.join("zz/a/b/c/.fabr")).unwrap();
    let has = |v: &Value| v.to_string().contains("\"zz/a/b/c\"");
    assert!(!has(&srv.get("/api/get_tree").1));
    assert!(has(&srv.get("/api/get_tree?rescan=1").1));
    let (_, st) = srv.get("/api/get_settings");
    assert_eq!((st["search_depth"].clone(), st["full_rescan_ms"].clone()), (json!(4), json!(300000)));
    assert!(st.get("log_path").is_some());
    let (_, t) = srv.post("/api/test_source", &json!({"id": "x", "name": "", "root": srv.root.display().to_string()}), &[]);
    assert_eq!(t["managed_root"], true);
    let (s, _) = srv.post("/api/reveal_path", &json!({"path": "/tmp"}), &[]);
    assert_eq!(s, 501);
}

#[test]
fn cli_init_depth() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("t");
    for d in ["a/x/deep", "b", "node_modules/p", ".dot"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    crate::cmd_init(root.clone(), false, None).unwrap();
    assert!(std::fs::read_to_string(root.join(".fabr/meta.toml")).unwrap().contains("show_depth = 1"));
    assert!(!root.join("a/.fabr").exists());
    assert_eq!(crate::cmd_init(root.clone(), false, None).unwrap_err().code(), 409);
    crate::cmd_init(root.clone(), false, Some(2)).unwrap();
    for d in ["a", "b", "a/x"] {
        assert!(root.join(d).join(".fabr/.todo").is_file(), "{d}");
    }
    for d in ["a/x/deep", "node_modules", ".dot"] {
        assert!(!root.join(d).join(".fabr").exists(), "{d}");
    }
}

#[test]
fn remove_node_route() {
    let srv = start();
    let (_, tree) = srv.get("/api/get_tree");
    let work = tree["root"]["children"][0]["children"].as_array().unwrap().iter().find(|c| c["path"] == "work").unwrap();
    let api = work["children"].as_array().unwrap().iter().find(|c| c["path"] == "work/api-server").unwrap().clone();
    let body = |v: &Value| json!({"source": "local", "path": "work/api-server", "version": v});
    let (s, e) = srv.post("/api/remove_node", &body(&json!("0".repeat(40))), &[]);
    assert_eq!((s, e["code"].clone()), (409, json!(409)));
    assert_eq!(e["node"]["path"], "work/api-server");
    assert!(srv.root.join("work/api-server/.fabr").is_dir());
    let (s, n) = srv.post("/api/remove_node", &body(&api["version"]), &[]);
    assert_eq!(s, 200, "{n}");
    assert_eq!(n["managed"], false);
    assert!(!srv.root.join("work/api-server/.fabr").exists());
    assert!(srv.root.join("work/api-server/docs/.fabr").is_dir());
    assert_eq!(srv.post("/api/remove_node", &body(&json!("x")), &[]).0, 404);
    let (s, _) = srv.post("/api/remove_node", &json!({"source": "", "path": "", "version": "x"}), &[]);
    assert_eq!(s, 400);
    assert_eq!(srv.get("/api/remove_node").0, 405);
    assert_eq!(srv.raw("POST", "/api/remove_node", Some(b"{}"), &[]).0, 415);
}
