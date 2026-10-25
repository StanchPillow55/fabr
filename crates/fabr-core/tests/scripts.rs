mod common;
use common::*;
use fabr_core::remote::script::*;
use fabr_core::scan::default_prune;
use fabr_core::*;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

fn sh(script: &str, args: &[&str], stdin: &[u8]) -> (i32, Vec<u8>, String) {
    let line = shell_command(script, args);
    let mut child =
        Command::new("sh").arg("-c").arg(&line).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code().unwrap_or(-1), out.stdout, String::from_utf8_lossy(&out.stderr).into_owned())
}

fn prune_list() -> Vec<String> {
    let mut p: Vec<String> = default_prune().into_iter().collect();
    p.sort();
    p
}

fn untar(out: &[u8]) -> TarSnapshot {
    TarSnapshot::from_tar_capped(out, usize::MAX).unwrap().0
}

fn snapshot_of(root: &Path) -> TarSnapshot {
    let (code, out, err) = sh(&snapshot_script(&prune_list()).unwrap(), &[root.to_str().unwrap()], b"");
    assert!(code == 0 || code == 1, "exit {code}: {err}");
    untar(&out)
}

fn scan_json(fs: &dyn Fs, src: &Source, show_all: bool) -> serde_json::Value {
    let prune = default_prune();
    let r = scan(fs, src, &ScanOpts::new(show_all, &prune));
    serde_json::to_value(&r.root).unwrap()
}

#[test]
fn scripts_have_no_backslashes() {
    for s in [snapshot_script(&prune_list()).unwrap(), write_script(), remove_script(), init_script(), node_script(), list_script()] {
        assert!(!s.contains('\\'), "{s}");
        assert!(!s.contains('\n'), "{s}");
    }
    assert!(snapshot_script(&["a b".into()]).is_err());
    assert!(snapshot_script(&["$(x)".into()]).is_err());
    assert!(snapshot_script(&[]).unwrap().contains("find ."));
}

#[test]
fn quoting_round_trips() {
    let (code, out, _) = sh("printf %s \"$1|$2\"", &["it's a \"test\"", "$HOME `x` \\n"], b"");
    assert_eq!(code, 0);
    assert_eq!(String::from_utf8(out).unwrap(), "it's a \"test\"|$HOME `x` \\n");
    assert_eq!(quote("a'b"), "'a'\"'\"'b'");
}

#[test]
fn tar_snapshot_scans_like_local_fs() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.p("work/api-server/src/deep/.fabr")).unwrap();
    std::fs::create_dir_all(f.p("research/ideas/drafts/old")).unwrap();
    std::fs::create_dir_all(f.p(".hidden/x/.fabr")).unwrap();
    std::fs::write(f.p("work/.fabr/.other"), "x").unwrap();
    std::os::unix::fs::symlink(f.p("work"), f.p("research/link")).unwrap();
    let src = default_local(&f.root);
    let snap = snapshot_of(&f.root);
    let local = LocalFs::new(f.root.clone());
    for show_all in [false, true] {
        let (a, b) = (scan_json(&snap, &src, show_all), scan_json(&local, &src, show_all));
        assert!(a == b, "show_all={show_all}\n{a:#}\n---\n{b:#}");
    }
    assert!(snap.files.contains_key("work/.fabr/.todo"));
    assert!(snap.files.contains_key(".fabr/meta.toml"));
    assert!(!snap.dirs.iter().any(|d| d.contains(".hidden") || d.contains("pkg")));
    assert!(snap.dirs.contains("work/frontend/node_modules"));
}

#[test]
fn tar_snapshot_hash_is_deterministic() {
    let f = Fixture::new();
    let a = snapshot_of(&f.root);
    let b = snapshot_of(&f.root);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.hash().len(), 40);
    std::fs::write(f.p("work/.fabr/.todo"), "todo -  changed\n").unwrap();
    assert_ne!(a.hash(), snapshot_of(&f.root).hash());
    std::fs::create_dir(f.p("work/newdir")).unwrap();
    let c = snapshot_of(&f.root);
    assert!(c.dirs.contains("work/newdir"));
    assert_eq!(c.subdirs("work/frontend"), [".fabr", "node_modules"]);
    let mut s = c.subdirs("");
    s.sort();
    assert_eq!(s, [".fabr", "archive", "personal", "research", "work"]);
    let (code, out, _) = sh(&snapshot_script(&prune_list()).unwrap(), &[f.root.to_str().unwrap()], b"");
    assert!(code <= 1);
    let (small, cut) = TarSnapshot::from_tar_capped(&out[..], 3).unwrap();
    assert!(cut && small.dirs.len() <= 4);
}

#[test]
fn snapshot_of_missing_root_is_44() {
    let (code, _, _) = sh(&snapshot_script(&[]).unwrap(), &["/nonexistent/fabr"], b"");
    assert_eq!(code, EXIT_NOT_NODE);
}

#[test]
fn write_script_ok_mismatch_and_not_a_node() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let dir = f.p("work");
    let todo = dir.join(".fabr/.todo");
    std::fs::set_permissions(&todo, std::fs::Permissions::from_mode(0o640)).unwrap();
    let meta_b = std::fs::read(dir.join(".fabr/meta.toml")).unwrap();
    let v = version_of(&meta_b, &std::fs::read(&todo).unwrap());
    let d = dir.to_str().unwrap();
    let (code, out, err) = sh(&write_script(), &[d, &v, ".todo"], b"todo -  new  id:ab\n");
    assert_eq!(code, 0, "{err}");
    assert_eq!(read(&todo), "todo -  new  id:ab\n");
    assert_eq!(std::fs::metadata(&todo).unwrap().permissions().mode() & 0o777, 0o640);
    let new_v = version_of(&meta_b, b"todo -  new  id:ab\n");
    assert_eq!(String::from_utf8(out).unwrap().trim(), new_v);
    let (code, _, err) = sh(&write_script(), &[d, &v, ".todo"], b"other\n");
    assert_eq!(code, EXIT_MISMATCH);
    assert_eq!(err.trim(), new_v);
    assert_eq!(read(&todo), "todo -  new  id:ab\n");
    let plain = f.p("personal/finance/receipts");
    let (code, _, _) = sh(&write_script(), &[plain.to_str().unwrap(), &v, ".todo"], b"x");
    assert_eq!(code, EXIT_NOT_NODE);
    let meetings = f.p("work/meetings");
    let mv = version_of(&std::fs::read(meetings.join(".fabr/meta.toml")).unwrap(), b"");
    let (code, _, err) = sh(&write_script(), &[meetings.to_str().unwrap(), &mv, ".todo"], b"# t\n");
    assert_eq!(code, 0, "{err}");
    assert_eq!(read(&meetings.join(".fabr/.todo")), "# t\n");
    let leftovers = std::fs::read_dir(meetings.join(".fabr")).unwrap().count();
    assert_eq!(leftovers, 2);
}

#[test]
fn init_script_ok_exists_missing() {
    let f = Fixture::new();
    let plain = f.p("personal/finance/receipts");
    let p = plain.to_str().unwrap();
    assert_eq!(sh(&init_script(), &[p], b"").0, 0);
    assert!(plain.join(".fabr").is_dir());
    assert_eq!(sh(&init_script(), &[p], b"").0, EXIT_MISMATCH);
    assert_eq!(sh(&init_script(), &["/nonexistent/fabr"], b"").0, EXIT_NOT_NODE);
    let v0 = version_of(b"", b"");
    let (code, out, err) = sh(&write_script(), &[p, &v0, "meta.toml"], b"name = \"receipts\"\n");
    assert_eq!(code, 0, "{err}");
    let v1 = String::from_utf8(out).unwrap().trim().to_string();
    assert_eq!(v1, version_of(b"name = \"receipts\"\n", b""));
    assert_eq!(sh(&write_script(), &[p, &v1, ".todo"], b"# fabr todo\n").0, 0);
}

#[test]
fn node_script_reads_one_node() {
    let f = Fixture::new();
    let root = f.root.to_str().unwrap();
    let (code, out, err) = sh(&node_script(), &[root, "work/api-server"], b"");
    assert_eq!(code, 0, "{err}");
    let snap = untar(&out);
    let local = load_node(&LocalFs::new(f.root.clone()), "s", "work/api-server");
    let remote = load_node(&snap, "s", "work/api-server");
    assert_eq!(serde_json::to_value(&local).unwrap(), serde_json::to_value(&remote).unwrap());
    assert!(!snap.dirs.contains("work/api-server/docs"));
    let (code, out, _) = sh(&node_script(), &[root, "."], b"");
    assert_eq!(code, 0);
    assert!(untar(&out).files.contains_key(".fabr/.todo"));
    assert_eq!(sh(&node_script(), &[root, "personal/finance/receipts"], b"").0, EXIT_NOT_NODE);
}

#[test]
fn ssh_loopback() {
    let Ok(host) = std::env::var("FABR_TEST_SSH") else {
        eprintln!("skipped: FABR_TEST_SSH not set");
        return;
    };
    let f = Fixture::new();
    let src = Source {
        id: "remote".into(),
        name: "sample".into(),
        host: Some(host),
        root: f.root.display().to_string(),
        prune: None,
        search_depth: None,
        password: None,
    };
    let be = SshBackend::new(src.clone(), f.cache(), Duration::from_secs(30));
    let snap = be.snapshot().unwrap();
    let local = LocalFs::new(f.root.clone());
    assert_eq!(scan_json(snap.as_ref(), &src, false), scan_json(&local, &src, false));
    let n = load_node(snap.as_ref(), "remote", "work");
    be.write_node("work", n.version.as_deref().unwrap(), NodeFiles { meta: None, todo: Some(b"# over ssh\n".to_vec()) }).unwrap();
    assert_eq!(read(&f.p("work/.fabr/.todo")), "# over ssh\n");
    let e = be.write_node("work", n.version.as_deref().unwrap(), NodeFiles { meta: None, todo: Some(b"x".to_vec()) }).unwrap_err();
    assert_eq!(e.code(), 409);
    be.init_node("personal/finance/receipts", None).unwrap();
    assert_eq!(be.init_node("personal/finance/receipts", None).unwrap_err().code(), 409);
    let ws = Workspace::from_config(&f.config(), &f.cache(), None).unwrap();
    let r = ws.test_source(src);
    assert!(r.ok, "{}", r.message);
}

#[test]
fn list_script_lists_immediate_subdirs() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.p("work/.dot")).unwrap();
    std::fs::create_dir_all(f.p("work/plain dir/inner/.fabr")).unwrap();
    std::os::unix::fs::symlink(f.p("personal"), f.p("work/link")).unwrap();
    let root = f.root.display().to_string();
    let (code, out, _) = sh(&list_script(), &[&root, "work"], b"");
    assert_eq!(code, 0);
    let mut got = parse_list(&out);
    got.sort();
    let want: Vec<(String, bool)> =
        [(".dot", false), (".fabr", false), ("api-server", true), ("frontend", true), ("meetings", true), ("plain dir", false)]
            .iter()
            .map(|(n, m)| (n.to_string(), *m))
            .collect();
    assert_eq!(got, want);
    assert_eq!(sh(&list_script(), &[&root, "missing"], b"").0, 44);
}

#[test]
fn remove_script_ok_mismatch_not_a_node_extra_files() {
    let f = Fixture::new();
    let dir = f.p("work");
    let xs = dir.join(".fabr");
    let d = dir.to_str().unwrap();
    let v = version_of(&std::fs::read(xs.join("meta.toml")).unwrap(), &std::fs::read(xs.join(".todo")).unwrap());
    let (code, _, err) = sh(&remove_script(), &[d, &"0".repeat(40)], b"");
    assert_eq!(code, EXIT_MISMATCH);
    assert_eq!(err.trim(), v);
    assert!(xs.join("meta.toml").is_file());
    let plain = f.p("personal/finance/receipts");
    assert_eq!(sh(&remove_script(), &[plain.to_str().unwrap(), &v], b"").0, EXIT_NOT_NODE);
    assert_eq!(sh(&remove_script(), &["/nonexistent/fabr", &v], b"").0, EXIT_NOT_NODE);
    std::fs::write(xs.join(".hidden"), "x").unwrap();
    let (code, _, _) = sh(&remove_script(), &[d, &v], b"");
    assert_eq!(code, EXIT_EXTRA_FILES);
    assert!(xs.join("meta.toml").is_file() && xs.join(".todo").is_file() && xs.join(".hidden").is_file());
    std::fs::remove_file(xs.join(".hidden")).unwrap();
    let (code, _, err) = sh(&remove_script(), &[d, &v], b"");
    assert_eq!(code, 0, "{err}");
    assert!(!xs.exists() && dir.is_dir());
    assert!(f.p("work/api-server/.fabr/meta.toml").is_file());
    let odd = f.p("it's here");
    std::fs::create_dir_all(odd.join(".fabr")).unwrap();
    let v0 = version_of(b"", b"");
    assert_eq!(sh(&remove_script(), &[odd.to_str().unwrap(), &v0], b"").0, 0);
    assert!(!odd.join(".fabr").exists());
}

#[test]
fn remove_script_works_with_a_node_dir_relative_to_the_cwd() {
    let f = Fixture::new();
    let root = f.p("").to_str().unwrap().to_string();
    let xs = f.p("personal/health/.fabr");
    let v = version_of(&std::fs::read(xs.join("meta.toml")).unwrap(), &std::fs::read(xs.join(".todo")).unwrap());
    let script = format!("cd '{}' || exit 99; {}", root.replace('\'', "'\\''"), remove_script());
    let (code, _, err) = sh(&script, &["./personal/health", &v], b"");
    assert_eq!(code, 0, "{err}");
    assert!(!xs.exists() && f.p("personal/health").is_dir());
    assert_eq!(sh(&script, &["./personal/health", &v], b"").0, EXIT_NOT_NODE);
}
