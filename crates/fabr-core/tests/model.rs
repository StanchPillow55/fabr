mod common;
use common::*;
use fabr_core::fabric::{self, FabricCreateReq};
use fabr_core::scan::{count_nodes, default_prune};
use fabr_core::*;

fn node(ws: &Workspace, path: &str) -> Node {
    ws.get_node(NodeRef { source: "local".into(), path: path.into() }).unwrap()
}

#[test]
fn show_depth_and_skipped_parents() {
    let f = Fixture::new();
    let tree = f.ws().get_tree(false);
    assert_eq!((tree.root.name.as_str(), tree.root.source.as_str(), tree.root.path.as_str(), tree.root.managed), ("/", "", "", false));
    assert_eq!(tree.root.children.len(), 1);
    let src = &tree.root.children[0];
    assert_eq!((src.source.as_str(), src.path.as_str()), ("local", ""));
    assert_eq!(src.meta["name"], MetaValue::Str("fabr".into()));
    assert_eq!(src.name, "sample");
    assert!(tree.errors.is_empty() && !tree.truncated);
    let receipts = find(&tree.root, "personal/finance/receipts").unwrap();
    assert!(!receipts.managed);
    let kids: Vec<_> = receipts.children.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(kids, ["personal/finance/receipts/2026"]);
    assert!(find(&tree.root, "research/paper-a/figures/raw").is_some());
    let modules = find(&tree.root, "work/frontend/node_modules").unwrap();
    assert!(modules.children.is_empty());
    assert_eq!(count_nodes(src), 19);
}

#[test]
fn via_for_skipped_plain_dirs() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.p("work/api-server/src/deep/.fabr")).unwrap();
    let tree = f.ws().get_tree(false);
    let deep = find(&tree.root, "work/api-server/src/deep").unwrap();
    assert_eq!(deep.via.as_deref(), Some("src/deep"));
    let api = find(&tree.root, "work/api-server").unwrap();
    assert!(api.children.iter().any(|c| c.path == "work/api-server/src/deep"));
    assert!(find(&tree.root, "work/api-server/src").is_none());
}

#[test]
fn show_all_adds_plain_dirs() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.p("research/ideas/drafts/old")).unwrap();
    let ws = f.ws();
    let normal = paths(&ws.get_tree(false).root);
    let everything = paths(&ws.get_tree(true).root);
    let diff: Vec<_> = everything.difference(&normal).cloned().collect();
    assert_eq!(diff, ["research/ideas/drafts", "research/ideas/drafts/old"]);
}

#[test]
fn dot_dirs_and_symlinks_skipped() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.p(".hidden/x/.fabr")).unwrap();
    std::os::unix::fs::symlink(f.p("work"), f.p("research/link")).unwrap();
    let all = paths(&f.ws().get_tree(true).root);
    assert!(!all.iter().any(|p| p.contains(".hidden") || p.contains("link")), "{all:?}");
}

#[test]
fn sorted_case_insensitively() {
    let f = Fixture::new();
    for d in ["research/Beta", "research/alpha", "research/Gamma"] {
        std::fs::create_dir_all(f.p(d).join(".fabr")).unwrap();
    }
    let tree = f.ws().get_tree(false);
    let names: Vec<_> = find(&tree.root, "research").unwrap().children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["alpha", "Beta", "Gamma", "ideas", "paper-a"]);
}

#[test]
fn items_loaded() {
    let f = Fixture::new();
    let tree = f.ws().get_tree(false);
    let api = find(&tree.root, "work/api-server").unwrap();
    assert_eq!(api.items.len(), 5);
    assert!(api.warnings.is_empty());
    assert_eq!(api.meta["kind"], MetaValue::Str("project".into()));
    let titles: Vec<_> = api.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Set up CI pipeline",
            "Fix N+1 query on /users",
            "Migrate auth to OAuth2",
            "Add rate limiting",
            "Investigate flaky integration test"
        ]
    );
    let pris: Vec<_> = api.items.iter().map(|i| i.pri).collect();
    assert_eq!(pris, [0, 0, 2, 3, 0]);
    let status: Vec<_> = api.items.iter().map(|i| i.status).collect();
    assert_eq!(status, [Status::Done, Status::Done, Status::Wip, Status::Todo, Status::Todo]);
    let tax = find(&tree.root, "personal/finance").unwrap().items.iter().find(|i| i.title == "File tax return").unwrap();
    assert_eq!(tax.pri, 1);
    assert!(tax.due.is_some() && !tax.id.is_empty());
    let meetings = find(&tree.root, "work/meetings").unwrap();
    assert!(meetings.managed && meetings.items.is_empty());
    assert_eq!(api.version.as_ref().unwrap().len(), 40);
    assert_eq!(tree.today.len(), 10);
}

#[test]
fn warnings_for_bad_files() {
    let f = Fixture::new();
    std::fs::write(f.p("work/.fabr/meta.toml"), "name = \n").unwrap();
    std::fs::write(f.p("personal/.fabr/.todo"), "# ok\ntodo - old line\n").unwrap();
    std::fs::write(f.p("research/.fabr/meta.toml"), "show_depth = -1\n").unwrap();
    let tree = f.ws().get_tree(false);
    assert!(find(&tree.root, "work").unwrap().warnings[0].starts_with("meta.toml does not parse"));
    let personal = find(&tree.root, "personal").unwrap();
    assert_eq!(personal.warnings.len(), 1);
    assert!(personal.warnings[0].starts_with(".fabr/.todo is no longer read"), "{:?}", personal.warnings);
    assert_eq!(personal.items.len(), 1);
    assert!(find(&tree.root, "research").unwrap().warnings[0].contains("show_depth -1"));
}

#[test]
fn truncation_at_cap() {
    let f = Fixture::new();
    let src = default_local(&f.root);
    let prune = default_prune();
    let fs = LocalFs::new(f.root.clone());
    let r = scan(&fs, &src, &ScanOpts { max_nodes: 5, ..ScanOpts::new(false, &prune) });
    assert!(r.truncated);
    assert_eq!(count_nodes(&r.root), 6);
    let r = scan(&fs, &src, &ScanOpts { max_nodes: 18, ..ScanOpts::new(false, &prune) });
    assert!(!r.truncated);
    let r = scan(&fs, &src, &ScanOpts { max_nodes: 17, ..ScanOpts::new(false, &prune) });
    assert!(r.truncated);
}

#[test]
fn the_version_hash_covers_meta_only() {
    let f = Fixture::new();
    let ws = f.ws();
    let n = node(&ws, "work");
    let v = n.version.clone().unwrap();
    assert_eq!(v, version_of(read(&f.p("work/.fabr/meta.toml")).as_bytes()));
    let req = FabricCreateReq {
        folder: f.p("work").display().to_string(),
        title: "new one".into(),
        due: None,
        pri: None,
        context: None,
        spec: None,
        success: None,
    };
    fabric::create_in(&req, f.tmp.path()).unwrap();
    std::fs::write(f.p("work/.fabr/.todo"), "todo - stray\n").unwrap();
    let after = node(&ws, "work");
    assert_eq!(after.version.as_ref(), Some(&v));
    assert_eq!(after.items.len(), n.items.len() + 1);
    let mut meta = n.meta.clone();
    meta.insert("description".into(), MetaValue::Str("changed".into()));
    let saved = ws.save_meta(SaveMetaReq { source: "local".into(), path: "work".into(), version: v.clone(), meta: meta.clone() }).unwrap();
    assert_ne!(saved.version.as_ref(), Some(&v));
    assert_eq!(saved.items.len(), n.items.len() + 1);
    let e = ws.save_meta(SaveMetaReq { source: "local".into(), path: "work".into(), version: v, meta }).unwrap_err();
    assert_eq!(e.code(), 409);
    let FabrError::Conflict { node: Some(cn), .. } = &e else { panic!("{e:?}") };
    assert_eq!(cn.version, saved.version);
}

#[test]
fn path_outside_root() {
    let f = Fixture::new();
    let ws = f.ws();
    for bad in ["../", "../../etc", "/etc", "work/../../x"] {
        let e = ws.save_meta(SaveMetaReq { source: "local".into(), path: bad.into(), version: "x".into(), meta: Meta::new() }).unwrap_err();
        assert_eq!(e.code(), 400, "{bad}");
    }
    let e = ws.save_meta(SaveMetaReq { source: "local".into(), path: "nope".into(), version: "x".into(), meta: Meta::new() }).unwrap_err();
    assert_eq!(e.code(), 404);
    let e = ws
        .save_meta(SaveMetaReq { source: "local".into(), path: "personal/finance/receipts".into(), version: "x".into(), meta: Meta::new() })
        .unwrap_err();
    assert_eq!(e.code(), 404);
    let e = ws.save_meta(SaveMetaReq { source: "zzz".into(), path: "".into(), version: "x".into(), meta: Meta::new() }).unwrap_err();
    assert_eq!(e.code(), 404);
    std::os::unix::fs::symlink("/", f.p("escape")).unwrap();
    let e = ws.get_node(NodeRef { source: "local".into(), path: "escape".into() }).unwrap_err();
    assert_eq!(e.code(), 400);
    let lfs = LocalFs::new(f.root.clone());
    assert_eq!(lfs.resolve("../x").unwrap_err().code(), 400);
    assert_eq!(lfs.resolve("missing").unwrap_err().code(), 404);
    assert_eq!(lfs.resolve("work").unwrap(), f.root.canonicalize().unwrap().join("work"));
}

#[test]
fn save_meta_changes_tree() {
    let f = Fixture::new();
    let ws = f.ws();
    let n = node(&ws, "research/paper-a");
    std::fs::create_dir(f.p("research/paper-a/plain")).unwrap();
    assert!(find(&ws.get_tree(false).root, "research/paper-a/plain").is_none());
    let mut meta = n.meta.clone();
    meta.insert("show_depth".into(), MetaValue::Int(1));
    ws.save_meta(SaveMetaReq { source: "local".into(), path: n.path.clone(), version: n.version.clone().unwrap(), meta }).unwrap();
    assert!(find(&ws.get_tree(false).root, "research/paper-a/plain").is_some());
}

#[test]
fn save_meta_keeps_comments_tables_and_refuses_broken() {
    let f = Fixture::new();
    let ws = f.ws();
    let mp = f.p("work/.fabr/meta.toml");
    std::fs::write(&mp, "# mine\nname = \"work\"  # keep\nkind = \"area\"\n\n[extra]\nfoo = 1\n").unwrap();
    let n = node(&ws, "work");
    let mut meta = n.meta.clone();
    meta.insert("description".into(), MetaValue::Str("hi".into()));
    meta.shift_remove("kind");
    let saved =
        ws.save_meta(SaveMetaReq { source: "local".into(), path: "work".into(), version: n.version.clone().unwrap(), meta }).unwrap();
    assert_eq!(read(&mp), "# mine\nname = \"work\"  # keep\ndescription = \"hi\"\n\n[extra]\nfoo = 1\n");
    assert!(!saved.meta.contains_key("extra"));
    let mut bad = saved.meta.clone();
    bad.insert("hidden".into(), MetaValue::Str("yes".into()));
    let e = ws
        .save_meta(SaveMetaReq { source: "local".into(), path: "work".into(), version: saved.version.clone().unwrap(), meta: bad })
        .unwrap_err();
    assert_eq!(e.code(), 400);
    std::fs::write(&mp, "name = \n").unwrap();
    let n = node(&ws, "work");
    let e = ws
        .save_meta(SaveMetaReq { source: "local".into(), path: "work".into(), version: n.version.unwrap(), meta: Meta::new() })
        .unwrap_err();
    assert_eq!(e.code(), 422);
}

#[test]
fn init_node() {
    let f = Fixture::new();
    let ws = f.ws();
    let r = NodeRef { source: "local".into(), path: "personal/finance/receipts".into() };
    let n = ws.init_node(r.clone()).unwrap();
    assert!(n.managed && n.items.is_empty());
    assert_eq!(n.meta["name"], MetaValue::Str("receipts".into()));
    let files: Vec<_> = std::fs::read_dir(f.p("personal/finance/receipts/.fabr")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(files, ["meta.toml"]);
    let e = ws.init_node(r).unwrap_err();
    assert_eq!(e.code(), 409);
    assert!(matches!(e, FabrError::Conflict { node: Some(_), .. }));
    assert_eq!(ws.init_node(NodeRef { source: "local".into(), path: "nope".into() }).unwrap_err().code(), 404);
}

#[test]
fn write_keeps_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let tp = f.p("work/.fabr/meta.toml");
    std::fs::set_permissions(&tp, std::fs::Permissions::from_mode(0o640)).unwrap();
    let ws = f.ws();
    let n = node(&ws, "work");
    let mut meta = n.meta.clone();
    meta.insert("description".into(), MetaValue::Str("perm".into()));
    ws.save_meta(SaveMetaReq { source: "local".into(), path: "work".into(), version: n.version.unwrap(), meta }).unwrap();
    assert!(read(&tp).contains("perm"));
    assert_eq!(std::fs::metadata(&tp).unwrap().permissions().mode() & 0o777, 0o640);
    let leftovers: Vec<_> = std::fs::read_dir(f.p("work/.fabr")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
}

#[test]
fn stamp_changes_on_edit() {
    let f = Fixture::new();
    let ws = f.ws();
    let s1 = ws.get_stamp().stamp;
    assert_eq!(s1, ws.get_stamp().stamp);
    assert_eq!(s1.len(), 40);
    let req = FabricCreateReq {
        folder: f.p("work").display().to_string(),
        title: "from test".into(),
        due: None,
        pri: None,
        context: None,
        spec: None,
        success: None,
    };
    fabric::create_in(&req, f.tmp.path()).unwrap();
    assert_ne!(s1, ws.get_stamp().stamp);
    let s2 = ws.get_stamp().stamp;
    std::fs::create_dir(f.p("work/frontend/node_modules/x")).unwrap();
    assert_eq!(s2, ws.get_stamp().stamp);
    let some = [Watched { path: "work".into(), listed: true }];
    assert_ne!(stamp_paths(&f.root, &some), stamp_paths(&f.root, &[Watched { path: "".into(), listed: true }]));
}

#[test]
fn sources_settings_and_multi_source_tree() {
    let f = Fixture::new();
    let other = f.tmp.path().join("other");
    write_fixture_tree(&other).unwrap();
    let ws = Workspace::from_config(&f.config(), &f.cache(), None).unwrap();
    assert!(ws.list_sources().is_empty());
    let empty = ws.get_tree(false);
    assert_eq!((empty.root.name.as_str(), empty.root.children.len()), ("/", 0));
    let a = Source {
        id: "a".into(),
        name: "A".into(),
        host: None,
        root: f.root.display().to_string(),
        prune: None,
        search_depth: None,
        password: None,
    };
    let b = Source {
        id: "b".into(),
        name: "B".into(),
        host: None,
        root: other.display().to_string(),
        prune: Some(vec![]),
        search_depth: None,
        password: None,
    };
    let gone = Source {
        id: "gone".into(),
        name: "Gone".into(),
        host: None,
        root: "/nonexistent/fabr".into(),
        prune: None,
        search_depth: None,
        password: None,
    };
    assert_eq!(ws.save_sources(vec![a.clone(), a.clone()]).unwrap_err().code(), 400);
    ws.save_sources(vec![a.clone(), b.clone(), gone.clone()]).unwrap();
    let tree = ws.get_tree(false);
    assert_eq!(tree.root.name, VIRTUAL_ROOT_NAME);
    assert_eq!((tree.root.source.as_str(), tree.root.path.as_str(), tree.root.managed), ("", "", false));
    let names: Vec<_> = tree.root.children.iter().map(|c| (c.source.as_str(), c.name.as_str())).collect();
    assert_eq!(names, [("a", "A"), ("b", "B"), ("gone", "Gone")]);
    assert_eq!(tree.errors.len(), 1);
    assert_eq!(tree.errors[0].source, "gone");
    assert_eq!(tree.root.children[0].children[0].source, "a");
    assert_eq!(tree.sources.len(), 3);
    let ws2 = Workspace::from_config(&f.config(), &f.cache(), None).unwrap();
    assert_eq!(ws2.list_sources(), vec![a, b, gone]);
    let n = ws.get_node(NodeRef { source: "b".into(), path: "work".into() }).unwrap();
    let mut meta = n.meta.clone();
    meta.insert("description".into(), MetaValue::Str("only b".into()));
    ws.save_meta(SaveMetaReq { source: "b".into(), path: "work".into(), version: n.version.unwrap(), meta }).unwrap();
    assert!(read(&other.join("work/.fabr/meta.toml")).contains("only b"));
    assert!(!read(&f.p("work/.fabr/meta.toml")).contains("only b"));
    assert_eq!(ws.get_settings(), Settings::default());
    assert_eq!(ws.save_settings(Settings { poll_ms: 1, ..Settings::default() }).unwrap_err().code(), 400);
    let s = Settings { poll_ms: 5000, remote_poll_ms: 60_000, search_depth: 6, full_rescan_ms: 60_000, max_nodes: 5000, log_path: None };
    ws.save_settings(s.clone()).unwrap();
    assert_eq!(Workspace::from_config(&f.config(), &f.cache(), None).unwrap().get_settings(), s);
}

#[test]
fn test_source_and_local_path() {
    let f = Fixture::new();
    let ws = f.ws();
    let r = ws.test_source(default_local(&f.root));
    assert!(r.ok, "{}", r.message);
    assert_eq!(r.node_count, 19);
    let r = ws.test_source(Source { root: "/nonexistent/fabr".into(), ..default_local(&f.root) });
    assert!(!r.ok);
    let r = ws.test_source(Source { id: "BAD".into(), ..default_local(&f.root) });
    assert!(!r.ok);
    let p = ws.local_path(&NodeRef { source: "local".into(), path: "work".into() }).unwrap();
    assert!(p.ends_with("sample/work"));
    let ssh = Source {
        id: "r".into(),
        name: "R".into(),
        host: Some("example.invalid".into()),
        root: "/x".into(),
        prune: None,
        search_depth: None,
        password: None,
    };
    ws.save_sources(vec![ssh]).unwrap();
    assert_eq!(ws.local_path(&NodeRef { source: "r".into(), path: "".into() }).unwrap_err().code(), 400);
}

#[test]
fn json_shapes() {
    let f = Fixture::new();
    let tree = f.ws().get_tree(false);
    let v = serde_json::to_value(&tree).unwrap();
    for k in ["root", "sources", "today", "truncated", "errors"] {
        assert!(v.get(k).is_some(), "{k}");
    }
    assert_eq!(
        (&v["root"]["name"], &v["root"]["source"], &v["root"]["path"]),
        (&serde_json::json!("/"), &serde_json::json!(""), &serde_json::json!(""))
    );
    let api = &v["root"]["children"][0]["children"].as_array().unwrap().iter().find(|c| c["path"] == "work").unwrap()["children"][0];
    assert_eq!(api["path"], "work/api-server");
    let item = &api["items"][2];
    assert_eq!(item["status"], "wip");
    assert_eq!(item["pri"], 2);
    assert!(item["id"].is_string() && item["due"].is_string() && item["title"] == "Migrate auth to OAuth2");
    assert!(api["items"][4]["due"].is_null() && api["items"][4]["pri"] == 0);
    assert!(api.get("has_todo").is_none());
    assert_eq!(v["sources"][0]["host"], serde_json::Value::Null);
    let e = serde_json::to_value(ErrorDto::from(FabrError::NotFound("x".into()))).unwrap();
    assert_eq!(e, serde_json::json!({"code": 404, "message": "x", "node": null}));
}

#[allow(clippy::result_large_err)]
fn rm(ws: &Workspace, path: &str, version: String) -> Result<Node, FabrError> {
    ws.remove_node(RemoveNodeReq { source: "local".into(), path: path.into(), version })
}

#[test]
fn remove_node_ok_keeps_folder_and_children() {
    let f = Fixture::new();
    let ws = f.ws();
    let v = node(&ws, "work/api-server").version.unwrap();
    let n = rm(&ws, "work/api-server", v).unwrap();
    assert!(!n.managed && n.version.is_none());
    assert_eq!(n.name, "api-server");
    assert!(!f.p("work/api-server/.fabr").exists());
    assert!(f.p("work/api-server").is_dir());
    assert!(f.p("work/api-server/docs/.fabr/meta.toml").is_file());
    let tree = ws.get_tree(false);
    let work = find(&tree.root, "work").unwrap();
    for (p, via) in [("work/api-server/docs", "api-server/docs"), ("work/api-server/tests", "api-server/tests")] {
        let c = work.children.iter().find(|c| c.path == p).unwrap_or_else(|| panic!("{p} not attached to work"));
        assert!(c.managed);
        assert_eq!(c.via.as_deref(), Some(via));
    }
    assert!(find(&tree.root, "work/api-server").is_none());
}

#[test]
fn remove_node_404_and_path_checks() {
    let f = Fixture::new();
    let ws = f.ws();
    assert_eq!(rm(&ws, "personal/finance/receipts", "x".into()).unwrap_err().code(), 404);
    assert_eq!(rm(&ws, "nope", "x".into()).unwrap_err().code(), 404);
    assert_eq!(rm(&ws, "../x", "x".into()).unwrap_err().code(), 400);
    let e = ws.remove_node(RemoveNodeReq { source: "".into(), path: "".into(), version: "x".into() }).unwrap_err();
    assert_eq!(e.code(), 400);
}

#[test]
fn remove_node_version_mismatch_is_409_with_node() {
    let f = Fixture::new();
    let ws = f.ws();
    let e = rm(&ws, "work", "0".repeat(40)).unwrap_err();
    assert_eq!(e.code(), 409);
    assert!(matches!(e, FabrError::Conflict { node: Some(ref n), .. } if n.managed && n.path == "work"));
    assert!(f.p("work/.fabr/meta.toml").is_file());
}

#[test]
fn remove_node_with_other_files_deletes_nothing() {
    let f = Fixture::new();
    let ws = f.ws();
    std::fs::write(f.p("work/.fabr/notes.txt"), "keep").unwrap();
    let v = node(&ws, "work").version.unwrap();
    let e = rm(&ws, "work", v).unwrap_err();
    assert_eq!(e.code(), 409);
    assert!(e.message().contains("contains other files; remove it by hand"), "{}", e.message());
    assert!(f.p("work/.fabr/meta.toml").is_file());
    assert!(f.p("work/.fabr/notes.txt").is_file());
}

#[test]
fn remove_source_root_node() {
    let f = Fixture::new();
    let ws = f.ws();
    let v = node(&ws, "").version.unwrap();
    let n = rm(&ws, "", v).unwrap();
    assert!(!n.managed);
    assert!(!f.p(".fabr").exists());
    assert!(f.p("work/.fabr").is_dir());
}

#[test]
fn rename_source_root_and_nested_node_in_a_two_source_tree() {
    let f = Fixture::new();
    let other = f.tmp.path().join("other");
    write_fixture_tree(&other).unwrap();
    let ws = Workspace::from_config(&f.config(), &f.cache(), None).unwrap();
    let a = Source {
        id: "a".into(),
        name: "A".into(),
        host: None,
        root: f.root.display().to_string(),
        prune: None,
        search_depth: None,
        password: None,
    };
    let b = Source {
        id: "b".into(),
        name: "B".into(),
        host: None,
        root: other.display().to_string(),
        prune: None,
        search_depth: None,
        password: None,
    };
    ws.save_sources(vec![a, b]).unwrap();
    let tree = ws.get_tree(false);
    for (path, new) in [("", "B renamed"), ("work/api-server", "API renamed")] {
        let n = find(&tree.root.children[1], path).unwrap();
        let mut meta = n.meta.clone();
        meta.insert("name".into(), MetaValue::Str(new.into()));
        let saved = ws.save_meta(SaveMetaReq { source: "b".into(), path: path.into(), version: n.version.clone().unwrap(), meta }).unwrap();
        assert_eq!((saved.source.as_str(), saved.path.as_str()), ("b", path));
        assert_eq!(saved.meta["name"], MetaValue::Str(new.into()));
        let t = ws.get_tree(false);
        let fresh = find(&t.root.children[1], path).unwrap();
        assert_eq!(fresh.meta["name"], MetaValue::Str(new.into()), "{path}");
        assert_eq!(fresh.version, saved.version);
        if path.is_empty() {
            assert_eq!(fresh.name, "B");
        }
        assert_ne!(find(&t.root.children[0], path).unwrap().meta.get("name"), Some(&MetaValue::Str(new.into())));
    }
    assert!(read(&other.join(".fabr/meta.toml")).contains("name = \"B renamed\""));
}

#[test]
fn two_show_all_clients_do_not_rescan_each_other() {
    let f = Fixture::new();
    let ws = f.ws();
    let folders = ws.get_tree(false);
    let all = ws.get_tree(true);
    assert_eq!(ws.local_scan_count(), 2);
    assert!(count_nodes(&all.root) >= count_nodes(&folders.root));
    let stamp = ws.get_stamp().stamp;
    for _ in 0..3 {
        assert_eq!(paths(&ws.get_tree(false).root), paths(&folders.root));
        assert_eq!(paths(&ws.get_tree(true).root), paths(&all.root));
        assert_eq!(ws.get_stamp().stamp, stamp, "stamp must not flip between clients");
    }
    assert_eq!(ws.local_scan_count(), 2, "alternating polls must not rescan");

    let n = node(&ws, "work");
    let mut meta = n.meta.clone();
    meta.insert("description".into(), MetaValue::Str("moved".into()));
    ws.save_meta(SaveMetaReq { source: "local".into(), path: "work".into(), version: n.version.unwrap(), meta }).unwrap();
    let moved = ws.get_stamp().stamp;
    assert_ne!(moved, stamp);
    for _ in 0..3 {
        ws.get_tree(false);
        ws.get_tree(true);
        assert_eq!(ws.get_stamp().stamp, moved);
    }
    assert_eq!(ws.local_scan_count(), 4);
    assert_eq!(find(&ws.get_tree(true).root, "work").unwrap().items.len(), 2);
    assert_eq!(find(&ws.get_tree(false).root, "work").unwrap().items.len(), 2);
    assert_eq!(ws.local_scan_count(), 4);

    let outside = FabricCreateReq {
        folder: f.p("work").display().to_string(),
        title: "added outside".into(),
        due: None,
        pri: None,
        context: None,
        spec: None,
        success: None,
    };
    fabric::create_in(&outside, f.tmp.path()).unwrap();
    assert_ne!(ws.get_stamp().stamp, moved);
    assert_eq!(find(&ws.get_tree(false).root, "work").unwrap().items.len(), 3);
}

#[test]
fn truncation_is_reported_per_cache_entry() {
    let f = Fixture::new();
    let ws = f.ws();
    for i in 0..150 {
        std::fs::create_dir_all(f.p(&format!("bulk/deep/d{i}"))).unwrap();
    }
    let s = Settings { max_nodes: 100, ..Settings::default() };
    ws.save_settings(s).unwrap();
    let folders = ws.get_tree(false);
    let all = ws.get_tree(true);
    assert!(!folders.truncated && folders.truncated_reason.is_none(), "{:?}", folders.truncated_reason);
    assert!(all.truncated && all.truncated_reason.is_some() && all.truncated_by.as_deref() == Some("nodes"));
    let n = ws.local_scan_count();
    let folders2 = ws.get_tree(false);
    let all2 = ws.get_tree(true);
    assert_eq!(ws.local_scan_count(), n);
    assert!(!folders2.truncated && folders2.truncated_reason.is_none());
    assert_eq!((all2.truncated, all2.truncated_reason), (true, all.truncated_reason));
}
