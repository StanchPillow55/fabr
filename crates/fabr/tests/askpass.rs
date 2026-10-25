#![cfg(unix)]
use fabr_core::remote::{askpass, ssh};
use fabr_core::{InstallKeyReq, Source, SourceIdReq, SourcePasswordReq, Workspace, secrets};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const PASSWORD: &str = "s3cret pw";

fn fake_ssh(dir: &Path) -> PathBuf {
    let remote_home = dir.join("remote-home");
    std::fs::create_dir_all(&remote_home).unwrap();
    let log = dir.join("askpass.log");
    let script = format!(
        r#"#!/bin/sh
for a; do last="$a"; done
if [ "$1" = "-G" ]; then printf 'hostname %s\nport 22\nidentityfile ~/.ssh/id_ed25519\n' "$2"; exit 0; fi
case " $* " in *" BatchMode=no "*) batch=no ;; *) batch=yes ;; esac
ak="{home}/.ssh/authorized_keys"
case " $* " in *" PasswordAuthentication=no "*) keyonly=1 ;; *) keyonly=0 ;; esac
if [ -s "$ak" ] && grep -q ssh-ed25519 "$ak"; then :
elif [ "$batch" = yes ] || [ "$keyonly" = 1 ]; then echo "fake@box: Permission denied (publickey,password)." >&2; exit 255
else
  [ "$SSH_ASKPASS_REQUIRE" = force ] || {{ echo "askpass not forced" >&2; exit 255; }}
  pw=$("$SSH_ASKPASS" "fake@box's password: ") || {{ echo "askpass failed" >&2; exit 255; }}
  printf '%s\n' "got:$pw" >> "{log}"
  [ "$pw" = "{pw}" ] || {{ echo "fake@box: Permission denied (password)." >&2; exit 255; }}
fi
HOME="{home}" exec sh -c "$last"
"#,
        home = remote_home.display(),
        log = log.display(),
        pw = PASSWORD
    );
    let p = dir.join("fake-ssh");
    std::fs::write(&p, script).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn password_over_askpass_then_install_key() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("sample");
    assert_eq!(fabr_core::write_fixture_tree(&root).unwrap(), 19);
    ssh::set_ssh_program(Some(fake_ssh(tmp.path())));
    let helper = std::env::var_os("FABR_TEST_ASKPASS_HELPER").map_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_fabr")), PathBuf::from);
    askpass::set_helper(Some(helper));
    secrets::use_memory_store();
    let ws = Workspace::from_config(&tmp.path().join("cfg"), &tmp.path().join("cache"), None).unwrap();
    let local_home = tmp.path().join("local-home");
    std::fs::create_dir_all(&local_home).unwrap();
    ws.set_home(local_home.clone());
    let src = Source {
        id: "box".into(),
        name: "Box".into(),
        host: Some("fakebox".into()),
        root: root.display().to_string(),
        prune: None,
        search_depth: None,
        password: None,
    };

    let r = ws.test_source(src.clone());
    assert!(!r.ok);
    assert_eq!(r.failure_kind.as_deref(), Some("auth_failed"), "{}", r.message);
    assert_eq!(r.message, "Authentication failed: the host did not accept your key or password");
    assert_eq!(r.detail.as_deref(), Some("fake@box: Permission denied (publickey,password)."));

    let r = ws.test_source(Source { password: Some(secrets::Secret::new("nope")), ..src.clone() });
    assert_eq!(r.failure_kind.as_deref(), Some("auth_failed"), "{}", r.message);

    let r = ws.test_source(Source { password: Some(secrets::Secret::new(PASSWORD)), ..src.clone() });
    assert!(r.ok, "{} {:?}", r.message, r.detail);
    assert_eq!(r.node_count, 19);
    let log = std::fs::read_to_string(tmp.path().join("askpass.log")).unwrap();
    assert_eq!(log.lines().collect::<Vec<_>>(), ["got:nope", &format!("got:{PASSWORD}")]);
    assert_eq!(std::fs::read_dir(tmp.path().join("cache/askpass")).unwrap().count(), 0);

    ws.save_sources(vec![Source { password: Some(secrets::Secret::new(PASSWORD)), ..src.clone() }]).unwrap();
    let toml = std::fs::read_to_string(tmp.path().join("cfg/fabr/sources.toml")).unwrap();
    assert!(!toml.contains(PASSWORD) && !toml.contains("password"), "{toml}");
    assert!(!serde_json::to_string(&ws.list_sources()).unwrap().contains("password"));
    assert!(!ws.has_source_password(SourceIdReq { source: "box".into() }).stored);
    ws.get_tree(false);
    ws.wait_remote(std::time::Duration::from_secs(60));
    let tree = ws.get_tree(false);
    assert_eq!(tree.errors.len(), 1, "no stored password yet");
    ws.set_source_password(SourcePasswordReq { source: "box".into(), password: secrets::Secret::new(PASSWORD) }).unwrap();
    assert!(ws.has_source_password(SourceIdReq { source: "box".into() }).stored);
    ws.get_tree_req(fabr_core::GetTreeReq { show_all: false, rescan: true });
    ws.wait_remote(std::time::Duration::from_secs(60));
    let tree = ws.get_tree(false);
    assert!(tree.errors.is_empty(), "{:?}", tree.errors);

    let res = ws.install_key(InstallKeyReq { source: "box".into(), host: None, password: None }).unwrap();
    assert!(res.installed, "{}", res.message);
    assert!(res.generated_key);
    let private = local_home.join(".ssh").join(["id", "ed25519"].join("_"));
    assert!(private.is_file());
    let mut pub_path = private.clone().into_os_string();
    pub_path.push(".pub");
    let pubkey = std::fs::read_to_string(PathBuf::from(pub_path)).unwrap();
    let ak = tmp.path().join("remote-home/.ssh/authorized_keys");
    assert_eq!(std::fs::read_to_string(&ak).unwrap(), pubkey);
    let again = ws.install_key(InstallKeyReq { source: "box".into(), host: None, password: None }).unwrap();
    assert!(again.installed && !again.generated_key, "{}", again.message);
    assert_eq!(std::fs::read_to_string(&ak).unwrap().lines().count(), 1);

    ws.set_source_password(SourcePasswordReq { source: "box".into(), password: secrets::Secret::new("") }).unwrap();
    assert!(!ws.has_source_password(SourceIdReq { source: "box".into() }).stored);
    assert!(ws.test_source(src).ok);
}

#[test]
fn helper_mode_without_server_fails_cleanly() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_fabr"))
        .args(["--askpass", "Password:"])
        .env_remove(askpass::ENV_SOCK)
        .env_remove(askpass::ENV_TOKEN)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}
