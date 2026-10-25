#![cfg(unix)]
use fabr_core::remote::ssh;
use fabr_core::{HostReq, Source, Workspace};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn keygen(path: &Path) {
    let st = Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-f"]).arg(path).status().unwrap();
    assert!(st.success());
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

struct Sshd(Child);
impl Drop for Sshd {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_sshd(dir: &Path, port: u16) -> Sshd {
    let cfg = dir.join("sshd_config");
    std::fs::write(
        &cfg,
        format!(
            "Port {port}\nListenAddress 127.0.0.1\nHostKey {d}/hostkey\nAuthorizedKeysFile {d}/authorized_keys\n\
             PidFile {d}/sshd.pid\nStrictModes no\nPasswordAuthentication no\nKbdInteractiveAuthentication no\n",
            d = dir.display()
        ),
    )
    .unwrap();
    let sshd = which("sshd").expect("sshd");
    let child = Command::new(sshd).arg("-D").arg("-e").arg("-f").arg(&cfg).stderr(Stdio::null()).spawn().unwrap();
    let t = Instant::now();
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(t.elapsed() < Duration::from_secs(10), "sshd did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    Sshd(child)
}

fn which(name: &str) -> Option<PathBuf> {
    for d in ["/usr/sbin", "/usr/bin", "/usr/local/sbin", "/sbin"] {
        let p = Path::new(d).join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

#[test]
fn trust_on_first_use_and_changed_key() {
    if std::env::var_os("FABR_TEST_SSHD").is_none() || which("sshd").is_none() {
        eprintln!("skipped: set FABR_TEST_SSHD=1 (needs sshd)");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let root = d.join("sample");
    fabr_core::write_fixture_tree(&root).unwrap();
    keygen(&d.join("hostkey"));
    let client = d.join("clientkey");
    keygen(&client);
    std::fs::copy(d.join("clientkey.pub"), d.join("authorized_keys")).unwrap();
    let port = free_port();
    let home = d.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let user = std::env::var("USER").unwrap_or_else(|_| "root".into());
    std::fs::write(
        d.join("ssh_config"),
        format!(
            "Host xtest\n  HostName 127.0.0.1\n  Port {port}\n  User {user}\n  IdentityFile {k}\n  IdentitiesOnly yes\n  \
             UserKnownHostsFile {h}/.ssh/known_hosts\n  GlobalKnownHostsFile /dev/null\n",
            k = client.display(),
            h = home.display()
        ),
    )
    .unwrap();
    let wrapper = d.join("ssh-wrapper");
    std::fs::write(&wrapper, format!("#!/bin/sh\nexec ssh -F {}/ssh_config \"$@\"\n", d.display())).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    ssh::set_ssh_program(Some(wrapper));

    let mut sshd = start_sshd(d, port);
    let ws = Workspace::from_config(&d.join("cfg"), &d.join("cache"), None).unwrap();
    ws.set_home(home.clone());
    let src = Source {
        id: "x".into(),
        name: "X".into(),
        host: Some("xtest".into()),
        root: root.display().to_string(),
        prune: None,
        search_depth: None,
        password: None,
    };

    let r = ws.test_source(src.clone());
    assert_eq!(r.failure_kind.as_deref(), Some("host_key_unknown"), "{} {:?}", r.message, r.detail);
    let keys = ws.host_key(HostReq { host: "xtest".into(), lines: vec![] }).unwrap();
    assert_eq!(keys.len(), 1, "{keys:?}");
    assert_eq!(keys[0].key_type, "ssh-ed25519");
    assert!(keys[0].line.starts_with(&format!("[127.0.0.1]:{port} ssh-ed25519 ")), "{}", keys[0].line);
    assert!(keys[0].fingerprint.starts_with("SHA256:"));
    let lines: Vec<String> = keys.iter().map(|k| k.line.clone()).collect();
    assert_eq!(ws.trust_host(HostReq { host: "xtest".into(), lines: lines.clone() }).unwrap().added, 1);
    assert_eq!(ws.trust_host(HostReq { host: "xtest".into(), lines }).unwrap().added, 0);
    let r = ws.test_source(src.clone());
    assert!(r.ok, "{} {:?}", r.message, r.detail);
    assert_eq!(r.node_count, 19);
    let _ = Command::new("ssh")
        .args(["-F"])
        .arg(d.join("ssh_config"))
        .args(["-O", "exit", "-o"])
        .arg(format!("ControlPath={}/cache/ssh/%C", d.display()))
        .arg("xtest")
        .stderr(Stdio::null())
        .status();
    let r = ws.test_source(Source { password: Some(fabr_core::secrets::Secret::new("unused")), ..src.clone() });
    assert!(r.ok, "{} {:?}", r.message, r.detail);

    drop(sshd);
    let _ = Command::new("ssh")
        .args(["-F"])
        .arg(d.join("ssh_config"))
        .args(["-O", "exit", "-o"])
        .arg(format!("ControlPath={}/cache/ssh/%C", d.display()))
        .arg("xtest")
        .stderr(Stdio::null())
        .status();
    std::fs::remove_file(d.join("hostkey")).unwrap();
    std::fs::remove_file(d.join("hostkey.pub")).unwrap();
    keygen(&d.join("hostkey"));
    sshd = start_sshd(d, port);
    let r = ws.test_source(src.clone());
    assert_eq!(r.failure_kind.as_deref(), Some("host_key_changed"), "{} {:?}", r.message, r.detail);
    let keys = ws.host_key(HostReq { host: "xtest".into(), lines: vec![] }).unwrap();
    let e = ws.trust_host(HostReq { host: "xtest".into(), lines: keys.iter().map(|k| k.line.clone()).collect() }).unwrap_err();
    assert_eq!(e.code(), 409);
    drop(sshd);
}

#[test]
fn init_remote_root_over_real_ssh_keeps_show_depth() {
    if std::env::var_os("FABR_TEST_SSHD").is_none() || which("sshd").is_none() {
        eprintln!("skipped: set FABR_TEST_SSHD=1 (needs sshd)");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let real_home = PathBuf::from(std::env::var("HOME").unwrap());
    let rel = format!(".cache/fabr-sshd-test-{}", std::process::id());
    let deep = real_home.join(&rel).join("deep");
    std::fs::create_dir_all(deep.join("a/b/c/d/e")).unwrap();
    std::fs::create_dir_all(deep.join("x/y")).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _c = Cleanup(real_home.join(&rel));

    keygen(&d.join("hostkey"));
    let client = d.join("clientkey");
    keygen(&client);
    std::fs::copy(d.join("clientkey.pub"), d.join("authorized_keys")).unwrap();
    let port = free_port();
    let home = d.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let user = std::env::var("USER").unwrap_or_else(|_| "root".into());
    std::fs::write(
        d.join("ssh_config"),
        format!(
            "Host xtest\n  HostName 127.0.0.1\n  Port {port}\n  User {user}\n  IdentityFile {k}\n  IdentitiesOnly yes\n  \
             UserKnownHostsFile {h}/.ssh/known_hosts\n  GlobalKnownHostsFile /dev/null\n  StrictHostKeyChecking no\n",
            k = client.display(),
            h = home.display()
        ),
    )
    .unwrap();
    let wrapper = d.join("ssh-wrapper");
    std::fs::write(&wrapper, format!("#!/bin/sh\nexec ssh -F {}/ssh_config \"$@\"\n", d.display())).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    ssh::set_ssh_program(Some(wrapper));
    let _sshd = start_sshd(d, port);

    let ws = Workspace::from_config(&d.join("cfg"), &d.join("cache"), None).unwrap();
    ws.set_home(home.clone());
    let tilde = Source {
        id: "r".into(),
        name: "R".into(),
        host: Some("xtest".into()),
        root: format!("~/{rel}/deep"),
        prune: None,
        search_depth: None,
        password: None,
    };
    let local = Source {
        id: "l".into(),
        name: "L".into(),
        host: None,
        root: deep.display().to_string(),
        prune: None,
        search_depth: None,
        password: None,
    };
    ws.save_sources(vec![tilde, local]).unwrap();
    fn depth(n: &fabr_core::Node) -> usize {
        1 + n.children.iter().map(depth).max().unwrap_or(0)
    }

    ws.get_tree(false);
    ws.wait_remote(std::time::Duration::from_secs(60));
    let t = ws.get_tree(false);
    assert!(t.errors.is_empty(), "{:?}", t.errors);
    assert_eq!(depth(&t.root.children[0]), 1, "remote before init: {}", serde_json::to_string_pretty(&t.root.children[0]).unwrap());
    let n = ws.init_node(fabr_core::NodeRef { source: "r".into(), path: "".into() }).unwrap();
    assert!(n.managed, "{n:?}");
    assert!(deep.join(".fabr/meta.toml").is_file());
    ws.get_tree(false);
    ws.wait_remote(std::time::Duration::from_secs(60));
    let t = ws.get_tree(false);
    assert!(t.errors.is_empty(), "{:?}", t.errors);
    let r = &t.root.children[0];
    assert!(r.managed, "{}", serde_json::to_string_pretty(r).unwrap());
    assert_eq!(depth(r), 2, "remote after init: {}", serde_json::to_string_pretty(r).unwrap());
    assert_eq!(depth(&t.root.children[1]), 2, "local after init: {}", serde_json::to_string_pretty(&t.root.children[1]).unwrap());
}
