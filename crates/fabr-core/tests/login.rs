use fabr_core::remote::ssh;
use fabr_core::{Backend, FabrError, Source, SshBackend};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

fn fake_ssh(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("ssh");
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn banner_then_hang_reports_login_required_with_the_url() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("args");
    let prog = fake_ssh(
        tmp.path(),
        &format!("echo \"$@\" >> {}\necho '# To authenticate, visit: https://login.tailscale.com/a/f00d' >&2\nsleep 30", log.display()),
    );
    ssh::set_ssh_program(Some(prog));
    let src = Source {
        id: "t".into(),
        name: "T".into(),
        host: Some("tailbox".into()),
        root: "~".into(),
        prune: None,
        search_depth: None,
        password: None,
    };
    let be = SshBackend::new(src, tmp.path().join("cache"), Duration::from_millis(800));

    let t = Instant::now();
    let e = be.snapshot().err().expect("must fail");
    assert!(t.elapsed() < Duration::from_secs(10));
    let FabrError::Remote(m) = &e else { panic!("{e:?}") };
    assert_eq!(m, "Log in to Tailscale to reach tailbox: https://login.tailscale.com/a/f00d");
    assert_eq!(ssh::failure_parts(m).0, Some(ssh::SshFailureKind::LoginRequired));

    let prog = fake_ssh(
        tmp.path(),
        "echo '# To authenticate, visit: https://login.tailscale.com/a/f00d' >&2\necho 'Connection to 100.1.2.3 port 22 timed out' >&2\nexit 255",
    );
    ssh::set_ssh_program(Some(prog));
    let e = be.snapshot().err().unwrap();
    assert!(e.message().starts_with("Log in to Tailscale to reach tailbox: https://"), "{}", e.message());

    let prog = fake_ssh(tmp.path(), &format!("echo \"$@\" > {}\nexit 255", log.display()));
    ssh::set_ssh_program(Some(prog));
    let _ = be.snapshot_waiting_for_login();
    let args = std::fs::read_to_string(&log).unwrap();
    assert!(args.contains("ServerAliveInterval=0") && !args.contains("ServerAliveInterval=15"), "{args}");
    ssh::set_ssh_program(None);
}
