mod common;
use common::fake_ssh;
use fabr_core::remote::ssh;
use fabr_core::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn ws_at(tmp: &Path) -> Workspace {
    secrets::use_memory_store();
    ssh::set_ssh_program(Some(fake_ssh()));
    Workspace::from_config(&tmp.join("cfg"), &tmp.join("cache"), None).unwrap()
}

fn source(id: &str, name: &str, host: Option<&str>, root: &str) -> Source {
    Source {
        id: id.into(),
        name: name.into(),
        host: host.map(Into::into),
        root: root.into(),
        prune: None,
        search_depth: None,
        password: None,
    }
}

fn setup() -> (tempfile::TempDir, PathBuf, Workspace) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("remote sample's");
    write_fixture_tree(&root).unwrap();
    let ws = ws_at(tmp.path());
    let src = source("r", "Remote", Some("fake"), &root.display().to_string());
    let local = source("l", "Local", None, &root.display().to_string());
    ws.save_sources(vec![src, local]).unwrap();
    (tmp, root, ws)
}

fn strip_source(mut n: Node) -> Node {
    n.source.clear();
    n.name.clear();
    n.children = n.children.into_iter().map(strip_source).collect();
    n
}

#[test]
fn remote_tree_matches_local() {
    let (_t, _root, ws) = setup();
    ws.get_tree(false);
    assert!(ws.wait_remote(Duration::from_secs(30)));
    let tree = ws.get_tree(false);
    assert!(tree.errors.is_empty(), "{:?}", tree.errors);
    let r = strip_source(tree.root.children[0].clone());
    let l = strip_source(tree.root.children[1].clone());
    assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::to_value(&l).unwrap());
    assert_eq!(tree.root.children[0].source, "r");
    assert_eq!(tree.root.children[0].children[0].source, "r");
}

#[test]
fn remote_edits_conflicts_and_init() {
    let (_t, root, ws) = setup();
    ws.get_stamp();
    assert!(ws.wait_remote(Duration::from_secs(30)));
    let s1 = ws.get_stamp().stamp;
    let n = ws.get_node(NodeRef { source: "r".into(), path: "work".into() }).unwrap();
    let mut meta = n.meta.clone();
    meta.insert("description".into(), MetaValue::Str("edited".into()));
    let req = SaveMetaReq { source: "r".into(), path: "work".into(), version: n.version.clone().unwrap(), meta };
    let saved = ws.save_meta(req.clone()).unwrap();
    assert_eq!(saved.meta["description"], MetaValue::Str("edited".into()));
    assert!(std::fs::read_to_string(root.join("work/.fabr/meta.toml")).unwrap().contains("edited"));
    ws.get_stamp();
    assert!(ws.wait_remote(Duration::from_secs(30)));
    assert_ne!(s1, ws.get_stamp().stamp);
    let e = ws.save_meta(req).unwrap_err();
    let FabrError::Conflict { node: Some(cn), .. } = &e else { panic!("{e:?}") };
    assert_eq!(cn.version, saved.version);
    let mut meta = saved.meta.clone();
    meta.insert("show_depth".into(), MetaValue::Int(2));
    let m = ws.save_meta(SaveMetaReq { source: "r".into(), path: "work".into(), version: saved.version.clone().unwrap(), meta }).unwrap();
    assert_eq!(m.meta["show_depth"], MetaValue::Int(2));
    let at = NodeRef { source: "r".into(), path: "personal/finance/receipts".into() };
    let n = ws.init_node(at.clone()).unwrap();
    assert!(n.managed);
    assert_eq!(n.meta["name"], MetaValue::Str("receipts".into()));
    assert_eq!(ws.init_node(at).unwrap_err().code(), 409);
    assert_eq!(ws.init_node(NodeRef { source: "r".into(), path: "missing".into() }).unwrap_err().code(), 404);
    assert_eq!(ws.get_node(NodeRef { source: "r".into(), path: "../x".into() }).unwrap_err().code(), 400);
    let e =
        ws.save_meta(SaveMetaReq { source: "r".into(), path: "personal".into(), version: "0".repeat(40), meta: Meta::new() }).unwrap_err();
    assert_eq!(e.code(), 409);
    let t = ws.test_source(ws.list_sources()[0].clone());
    assert!(t.ok, "{}", t.message);
    assert_eq!(t.node_count, 19);
}

#[test]
fn init_of_an_unmanaged_remote_root_keeps_show_depth_at_one() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("deep");
    std::fs::create_dir_all(root.join("a/b/c/d/e")).unwrap();
    std::fs::create_dir_all(root.join("x/y")).unwrap();
    let ws = ws_at(tmp.path());
    let src = source("r", "Remote", Some("fake"), &root.display().to_string());
    let local = source("l", "Local", None, &root.display().to_string());
    ws.save_sources(vec![src, local]).unwrap();
    let depth = |n: &Node| -> usize {
        fn d(n: &Node) -> usize {
            1 + n.children.iter().map(d).max().unwrap_or(0)
        }
        d(n)
    };
    ws.get_tree(false);
    assert!(ws.wait_remote(Duration::from_secs(30)));
    let t = ws.get_tree(false);
    assert!(t.errors.is_empty(), "{:?}", t.errors);
    assert_eq!(depth(&t.root.children[0]), 1, "remote before init: {:?}", t.root.children[0]);
    assert_eq!(depth(&t.root.children[1]), 1, "local before init");
    let n = ws.init_node(NodeRef { source: "r".into(), path: "".into() }).unwrap();
    assert!(n.managed);
    ws.get_tree(false);
    assert!(ws.wait_remote(Duration::from_secs(30)));
    let t = ws.get_tree(false);
    assert!(t.errors.is_empty(), "{:?}", t.errors);
    let r = &t.root.children[0];
    assert!(r.managed);
    assert_eq!(r.children.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), vec!["a", "x"]);
    assert_eq!(depth(r), 2, "remote after init goes too deep: {}", serde_json::to_string_pretty(r).unwrap());
    assert_eq!(depth(&t.root.children[1]), 2, "local after init goes too deep");
}

#[test]
fn remote_remove_node() {
    let (_t, root, ws) = setup();
    let at = |p: &str| NodeRef { source: "r".into(), path: p.into() };
    let n = ws.get_node(at("work")).unwrap();
    let req = RemoveNodeReq { source: "r".into(), path: "work".into(), version: "0".repeat(40) };
    let e = ws.remove_node(req.clone()).unwrap_err();
    assert!(matches!(e, FabrError::Conflict { node: Some(_), .. }));
    std::fs::write(root.join("work/.fabr/x"), "x").unwrap();
    let e = ws.remove_node(RemoveNodeReq { version: n.version.clone().unwrap(), ..req.clone() }).unwrap_err();
    assert!(e.message().contains("remove it by hand"), "{}", e.message());
    std::fs::remove_file(root.join("work/.fabr/x")).unwrap();
    let out = ws.remove_node(RemoveNodeReq { version: n.version.unwrap(), ..req.clone() }).unwrap();
    assert!(!out.managed);
    assert!(!root.join("work/.fabr").exists() && root.join("work").is_dir());
    assert_eq!(ws.remove_node(RemoveNodeReq { version: "x".into(), ..req }).unwrap_err().code(), 404);
}

#[test]
fn remote_list_dirs_and_init_children() {
    let (_t, root, ws) = setup();
    std::fs::create_dir_all(root.join("work/frontend/node_modules/pkg/.fabr")).unwrap();
    std::fs::create_dir_all(root.join("work/frontend/node_modules/other")).unwrap();
    std::fs::create_dir_all(root.join("work/.dot")).unwrap();
    let lr = |s: &str, p: &str| ListDirsReq { source: s.into(), path: p.into() };
    let before = ws.list_dirs(lr("r", "work")).unwrap();
    ws.get_tree(false);
    assert!(ws.wait_remote(Duration::from_secs(30)));
    assert!(ws.get_tree(false).root.children[0].managed);
    let after = ws.list_dirs(lr("r", "work")).unwrap();
    let local = ws.list_dirs(lr("l", "work")).unwrap();
    assert_eq!(before, local);
    assert_eq!(after, local);
    assert_eq!(local.len(), 3);
    let nm = ws.list_dirs(lr("r", "work/frontend/node_modules")).unwrap();
    assert_eq!(nm, ws.list_dirs(lr("l", "work/frontend/node_modules")).unwrap());
    assert_eq!(nm.iter().map(|d| (d.name.as_str(), d.managed)).collect::<Vec<_>>(), [("other", false), ("pkg", true)]);
    assert_eq!(ws.list_dirs(lr("r", "missing")).unwrap_err().code(), 404);
    std::fs::create_dir_all(root.join("personal/finance/receipts/2025")).unwrap();
    let n = ws
        .init_children(InitChildrenReq {
            source: "r".into(),
            path: "personal/finance/receipts".into(),
            names: vec!["2025".into(), "2026".into()],
        })
        .unwrap();
    assert!(n.managed);
    assert_eq!(n.meta["show_depth"], MetaValue::Int(1));
    assert!(n.children.iter().all(|c| c.managed && c.source == "r"));
    assert_eq!(n.children.len(), 2);
    let e = ws.init_children(InitChildrenReq { source: "r".into(), path: "".into(), names: vec!["zzz".into()] }).unwrap_err();
    assert_eq!(e.code(), 400);
    assert!(ws.test_source(ws.list_sources()[0].clone()).managed_root);
}

#[test]
fn remote_failure_is_collected() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = ws_at(tmp.path());
    let bad = source("r", "R", Some("fake"), "/nonexistent/fabr");
    ws.save_sources(vec![bad]).unwrap();
    ws.get_tree(false);
    assert!(ws.wait_remote(Duration::from_secs(30)));
    let tree = ws.get_tree(false);
    assert_eq!(tree.errors.len(), 1);
    assert_eq!((tree.root.source.as_str(), tree.root.name.as_str()), ("", VIRTUAL_ROOT_NAME));
    let r = &tree.root.children[0];
    assert_eq!((r.source.as_str(), r.path.as_str(), r.name.as_str()), ("r", "", "R"));
    assert!(!r.warnings.is_empty());
    assert_eq!(ws.get_stamp().stamp.len(), 40);
    assert!(!ws.test_source(ws.list_sources()[0].clone()).ok);
}
