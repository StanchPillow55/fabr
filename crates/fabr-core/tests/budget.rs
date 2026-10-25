mod common;
use common::*;
use fabr_core::scan::{HOME_PRUNE, count_nodes, default_prune, is_home_root, source_prune};
use fabr_core::*;
use std::time::Duration;

fn mk(f: &Fixture, rel: &str) {
    std::fs::create_dir_all(f.p(rel)).unwrap();
}

#[test]
fn search_depth_limits_and_resets_at_managed_nodes() {
    let f = Fixture::new();
    mk(&f, "deep/l2/l3/l4/.fabr");
    mk(&f, "deep/l2/l3/l4/m1/m2/m3/m4/.fabr");
    mk(&f, "deep/l2/l3/l4/m1/m2/m3/m4/n1/n2/n3/n4/n5/.fabr");
    mk(&f, "far/l2/l3/l4/l5/.fabr");
    let tree = f.ws().get_tree(false);
    let l4 = find(&tree.root, "deep/l2/l3/l4").unwrap();
    assert_eq!(l4.via.as_deref(), Some("l2/l3/l4"));
    assert!(find(&tree.root, "deep/l2/l3/l4/m1/m2/m3/m4").is_some());
    assert!(find(&tree.root, "deep/l2/l3/l4/m1/m2/m3/m4/n1/n2/n3/n4/n5").is_none());
    assert!(find(&tree.root, "far/l2/l3/l4/l5").is_none());
    assert!(!tree.truncated && tree.truncated_reason.is_none());
    let ws = f.ws();
    let mut src = ws.list_sources()[0].clone();
    src.search_depth = Some(5);
    ws.save_sources(vec![src]).unwrap();
    assert!(find(&ws.get_tree(false).root, "far/l2/l3/l4/l5").is_some());
    let ws = f.ws();
    let mut s = ws.get_settings();
    s.search_depth = 1;
    ws.save_settings(s).unwrap();
    let t = ws.get_tree(false);
    assert!(find(&t.root, "deep/l2/l3/l4").is_none());
    assert!(find(&t.root, "research/paper-a/figures/raw").is_some());
}

#[test]
fn visited_and_time_budgets_truncate_with_reason() {
    let f = Fixture::new();
    let src = default_local(&f.root);
    let prune = default_prune();
    let fs = LocalFs::new(f.root.clone());
    let r = scan(&fs, &src, &ScanOpts { max_visited: 3, ..ScanOpts::new(false, &prune) });
    assert!(r.truncated);
    assert_eq!(r.visited, 3);
    assert!(r.truncated_reason.as_deref().unwrap().starts_with("stopped after 3 directories"), "{:?}", r.truncated_reason);
    let full = scan(&fs, &src, &ScanOpts::new(false, &prune));
    assert!(!full.truncated && full.truncated_reason.is_none());
    assert_eq!(full.emitted.len(), count_nodes(&full.root));
    assert_eq!(full.emitted[0].path, "");

    struct Slow(LocalFs);
    impl Fs for Slow {
        fn is_dir(&self, r: &str) -> bool {
            self.0.is_dir(r)
        }
        fn subdirs(&self, r: &str) -> Vec<String> {
            std::thread::sleep(Duration::from_millis(5));
            self.0.subdirs(r)
        }
        fn exists(&self, r: &str) -> bool {
            self.0.exists(r)
        }
        fn read(&self, r: &str) -> Option<Vec<u8>> {
            self.0.read(r)
        }
    }
    let slow = Slow(LocalFs::new(f.root.clone()));
    let r = scan(&slow, &src, &ScanOpts { time_budget_ms: 12, ..ScanOpts::new(false, &prune) });
    assert!(r.truncated);
    assert!(r.truncated_reason.as_deref().unwrap().starts_with("the disk was slow ("));
    assert_eq!(r.truncated_by, Some("time"));
    let r = scan(&fs, &src, &ScanOpts { max_nodes: 5, ..ScanOpts::new(false, &prune) });
    assert!(r.truncated_reason.as_deref().unwrap().starts_with("stopped at 5 nodes"));
    assert_eq!(r.truncated_by, Some("nodes"));
}

#[test]
fn unreadable_emitted_dir_gets_a_warning() {
    struct Failing(LocalFs);
    impl Fs for Failing {
        fn is_dir(&self, r: &str) -> bool {
            self.0.is_dir(r)
        }
        fn subdirs(&self, r: &str) -> Vec<String> {
            self.0.subdirs(r)
        }
        fn try_subdirs(&self, r: &str) -> Result<Vec<String>, String> {
            if r == "work" || r == "work/api-server/src" {
                Err("Operation not permitted (os error 1)".into())
            } else {
                Ok(self.0.subdirs(r))
            }
        }
        fn exists(&self, r: &str) -> bool {
            self.0.exists(r)
        }
        fn read(&self, r: &str) -> Option<Vec<u8>> {
            self.0.read(r)
        }
    }
    let f = Fixture::new();
    let prune = default_prune();
    let r = scan(&Failing(LocalFs::new(f.root.clone())), &default_local(&f.root), &ScanOpts::new(false, &prune));
    let work = find(&r.root, "work").unwrap();
    assert!(work.children.is_empty());
    assert_eq!(work.warnings, ["cannot read directory: Operation not permitted (os error 1)"]);
    assert!(find(&r.root, "personal/finance").is_some());
}

#[test]
fn home_root_prunes_media_folders() {
    let home = directories::BaseDirs::new().unwrap().home_dir().to_path_buf();
    let mut s = default_local(&home);
    assert!(is_home_root(&s));
    let p = source_prune(&s);
    assert!(HOME_PRUNE.iter().all(|n| p.contains(*n)) && p.contains("node_modules"));
    s.root = "~".into();
    assert!(is_home_root(&s));
    s.prune = Some(vec!["x".into()]);
    assert!(!source_prune(&s).contains("Library"));
    let f = Fixture::new();
    assert!(!source_prune(&default_local(&f.root)).contains("Library"));
    let ssh = Source { host: Some("box".into()), root: "~".into(), ..default_local(&home) };
    assert!(!is_home_root(&ssh));
}

#[test]
fn stamp_is_incremental_and_rescan_finds_deep_changes() {
    let f = Fixture::new();
    mk(&f, "zz/a/b");
    let ws = f.ws();
    let t1 = ws.get_tree(false);
    assert!(find(&t1.root, "zz").is_some() && find(&t1.root, "zz/a").is_none());
    let s1 = ws.get_stamp().stamp;
    assert_eq!(s1, ws.get_stamp().stamp);
    mk(&f, "zz/a/b/c/.fabr");
    std::fs::write(f.p("zz/a/new.txt"), "x").unwrap();
    assert_eq!(s1, ws.get_stamp().stamp);
    assert!(find(&ws.get_tree(false).root, "zz/a/b/c").is_none());
    let t = ws.get_tree_req(GetTreeReq { show_all: false, rescan: true });
    assert_eq!(find(&t.root, "zz/a/b/c").unwrap().via.as_deref(), Some("a/b/c"));
    let s2 = ws.get_stamp().stamp;
    assert_ne!(s1, s2);
    let req = fabric::FabricCreateReq {
        folder: f.p("personal/health").display().to_string(),
        title: "external edit".into(),
        due: None,
        pri: None,
        context: None,
        spec: None,
        success: None,
    };
    fabric::create_in(&req, f.tmp.path()).unwrap();
    let s3 = ws.get_stamp().stamp;
    assert_ne!(s2, s3);
    let t = ws.get_tree(false);
    assert!(find(&t.root, "personal/health").unwrap().items.iter().any(|i| i.title == "external edit"));
    mk(&f, "personal/newdir");
    assert_ne!(s3, ws.get_stamp().stamp);
}

#[test]
fn stale_scan_is_redone_automatically() {
    let f = Fixture::new();
    mk(&f, "zz/a/b");
    let ws = f.ws();
    let mut s = ws.get_settings();
    s.full_rescan_ms = 1000;
    ws.save_settings(s).unwrap();
    ws.get_tree(false);
    let s1 = ws.get_stamp().stamp;
    mk(&f, "zz/a/b/c/.fabr");
    assert_eq!(s1, ws.get_stamp().stamp);
    std::thread::sleep(Duration::from_millis(1100));
    assert_ne!(s1, ws.get_stamp().stamp);
    assert!(find(&ws.get_tree(false).root, "zz/a/b/c").is_some());
}

#[test]
fn list_dirs_local() {
    let f = Fixture::new();
    mk(&f, "work/.hidden");
    mk(&f, "work/Zeta");
    std::os::unix::fs::symlink(f.p("personal"), f.p("work/link")).unwrap();
    std::fs::write(f.p("work/file.txt"), "x").unwrap();
    let ws = f.ws();
    let l = ws.list_dirs(ListDirsReq { source: "local".into(), path: "work".into() }).unwrap();
    let names: Vec<_> = l.iter().map(|d| (d.name.as_str(), d.managed, d.pruned)).collect();
    assert_eq!(names, [("api-server", true, false), ("frontend", true, false), ("meetings", true, false), ("Zeta", false, false)]);
    let l = ws.list_dirs(ListDirsReq { source: "local".into(), path: "work/frontend".into() }).unwrap();
    assert!(l.iter().any(|d| d.name == "node_modules" && d.pruned && !d.managed));
    let v = serde_json::to_value(&l[0]).unwrap();
    assert_eq!(v.as_object().unwrap().keys().collect::<Vec<_>>(), ["managed", "name", "pruned"]);
    assert_eq!(ws.list_dirs(ListDirsReq { source: "local".into(), path: "nope".into() }).unwrap_err().code(), 404);
    assert_eq!(ws.list_dirs(ListDirsReq { source: "local".into(), path: "../".into() }).unwrap_err().code(), 400);
}

#[test]
fn init_children_and_root_show_depth() {
    let f = Fixture::new();
    mk(&f, "personal/finance/receipts/2025/q1");
    let ws = f.ws();
    let req = |path: &str, names: &[&str]| InitChildrenReq {
        source: "local".into(),
        path: path.into(),
        names: names.iter().map(|s| s.to_string()).collect(),
    };
    let e = ws.init_children(req("personal/finance/receipts", &["2025", "nope", "../x", "a/b", ".fabr"])).unwrap_err();
    assert_eq!(e.code(), 400);
    for n in ["\"nope\"", "\"../x\"", "\"a/b\"", "\".fabr\""] {
        assert!(e.message().contains(n), "{}", e.message());
    }
    assert!(!f.p("personal/finance/receipts/.fabr").exists());
    let n = ws.init_children(req("personal/finance/receipts", &["2025", "2026"])).unwrap();
    assert!(n.managed);
    assert_eq!(n.meta["show_depth"], MetaValue::Int(1));
    assert_eq!(n.path, "personal/finance/receipts");
    let kids: Vec<_> = n.children.iter().map(|c| (c.path.as_str(), c.managed)).collect();
    assert_eq!(kids, [("personal/finance/receipts/2025", true), ("personal/finance/receipts/2026", true)]);
    assert_eq!(n.children[0].children.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), ["personal/finance/receipts/2025/q1"]);
    assert_eq!(read(&f.p("personal/finance/receipts/2025/.fabr/meta.toml")), "name = \"2025\"\nshow_depth = 1\n");
    assert!(!f.p("personal/finance/receipts/2025/.fabr/.todo").exists());
    assert!(read(&f.p("personal/finance/receipts/2026/.fabr/meta.toml")).contains("kind"));
    assert!(find(&ws.get_tree(false).root, "personal/finance/receipts/2025").unwrap().managed);
    let fresh = f.tmp.path().join("fresh");
    std::fs::create_dir_all(fresh.join("a")).unwrap();
    let ws2 = Workspace::from_config(&f.config(), &f.cache(), Some(fresh.clone())).unwrap();
    assert!(!ws2.test_source(default_local(&fresh)).managed_root);
    let root = ws2.init_node(NodeRef { source: "local".into(), path: "".into() }).unwrap();
    assert_eq!(root.meta["show_depth"], MetaValue::Int(1));
    assert!(ws2.test_source(default_local(&fresh)).managed_root);
    assert_eq!(find(&ws2.get_tree(false).root, "a").map(|n| n.managed), Some(false));
    let r = ws2.init_children(InitChildrenReq { source: "local".into(), path: "".into(), names: vec!["a".into()] }).unwrap();
    assert!(r.children[0].managed);
}

#[test]
fn settings_additions() {
    let f = Fixture::new();
    let ws = f.ws();
    let s = ws.get_settings();
    assert_eq!((s.search_depth, s.full_rescan_ms), (4, 300_000));
    for bad in [0, 13] {
        assert_eq!(ws.save_settings(Settings { search_depth: bad, ..s.clone() }).unwrap_err().code(), 400);
    }
    assert_eq!(ws.save_settings(Settings { full_rescan_ms: 10, ..s.clone() }).unwrap_err().code(), 400);
    let saved = ws.save_settings(Settings { search_depth: 7, log_path: Some("/evil".into()), ..s.clone() }).unwrap();
    assert_eq!(saved.search_depth, 7);
    assert_eq!(s.max_nodes, 10_000);
    for bad in [99, 100_001] {
        assert_eq!(ws.save_settings(Settings { max_nodes: bad, ..s.clone() }).unwrap_err().code(), 400);
    }
    assert_eq!(ws.save_settings(Settings { max_nodes: 100, ..s.clone() }).unwrap().max_nodes, 100);
    assert_eq!(ws.get_settings().max_nodes, 100);
    assert_eq!(saved.log_path, None);
    let old: Settings = serde_json::from_str(r#"{"poll_ms": 2000, "remote_poll_ms": 30000}"#).unwrap();
    assert_eq!(old, Settings::default());
    let v = serde_json::to_value(ws.get_tree(false)).unwrap();
    assert!(v["truncated_reason"].is_null());
}
