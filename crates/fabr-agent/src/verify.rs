use fabr_core::FabrError;
use fabr_core::taskrec::Check;
use std::io::Read;
use std::path::{Component, Path};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

pub const DEFAULT_CHECK_TIMEOUT_S: u64 = 60;
pub const OUTPUT_CAP: usize = 16 * 1024;

pub const ALLOWED_COMMANDS: [&str; 18] = [
    "test",
    "[",
    "true",
    "false",
    "cat",
    "grep",
    "egrep",
    "fgrep",
    "ls",
    "wc",
    "head",
    "tail",
    "diff",
    "cmp",
    "stat",
    "sha256sum",
    "md5sum",
    "file",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutcome {
    pub pass: bool,
    pub evidence: String,
}

#[derive(Debug)]
pub struct Exec {
    pub status: ExitStatus,
    pub output: Vec<u8>,
}

pub fn validate_check(check: &Check) -> Result<(), FabrError> {
    let cmd = check.cmd.as_str();
    if cmd.is_empty() || cmd.contains('/') || cmd.contains('\0') || !ALLOWED_COMMANDS.contains(&cmd) {
        return Err(FabrError::BadRequest(format!("check command {cmd} is not allowed")));
    }
    for arg in &check.args {
        if arg.contains('\0') {
            return Err(FabrError::BadRequest("check argument has a NUL byte".into()));
        }
        let path = Path::new(arg);
        if path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(FabrError::BadRequest(format!("check argument {arg} leaves the node")));
        }
    }
    Ok(())
}

pub fn run_checks(node_root: &Path, checks: &[Check], timeout: Duration) -> Result<Vec<CheckOutcome>, FabrError> {
    let mut out = Vec::with_capacity(checks.len());
    for check in checks {
        let line = format!("{} {}", check.cmd, check.args.join(" "));
        match exec_allowed(node_root, checks, check, timeout) {
            Ok(done) => {
                let code = done.status.code().unwrap_or(-1);
                let mut evidence = format!("{line} exit {code}");
                if !done.status.success() {
                    let text = String::from_utf8_lossy(&done.output);
                    let snippet = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !snippet.is_empty() {
                        evidence.push_str(": ");
                        evidence.push_str(&snippet.chars().take(200).collect::<String>());
                    }
                }
                out.push(CheckOutcome { pass: done.status.success(), evidence });
            }
            Err(FabrError::Io(m)) if m.starts_with("spawn ") => {
                out.push(CheckOutcome { pass: false, evidence: format!("{line} failed to start: {m}") })
            }
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

pub fn spawn_allowed(node_root: &Path, allowed: &[Check], check: &Check) -> Result<ExitStatus, FabrError> {
    exec_allowed(node_root, allowed, check, Duration::from_secs(DEFAULT_CHECK_TIMEOUT_S)).map(|e| e.status)
}

pub fn exec_allowed(node_root: &Path, allowed: &[Check], check: &Check, timeout: Duration) -> Result<Exec, FabrError> {
    if !allowed.iter().any(|c| c.cmd == check.cmd && c.args == check.args) {
        return Err(FabrError::BadRequest(format!("command is not on the list: {} {}", check.cmd, check.args.join(" "))));
    }
    validate_check(check)?;
    let mut child = Command::new(&check.cmd)
        .args(&check.args)
        .current_dir(node_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| FabrError::Io(format!("spawn {}: {e}", check.cmd)))?;
    let readers: Vec<_> = [
        child.stdout.take().map(|s| std::thread::spawn(move || capped(s))),
        child.stderr.take().map(|s| std::thread::spawn(move || capped(s))),
    ]
    .into_iter()
    .flatten()
    .collect();
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(FabrError::Timeout(format!(
                    "check timed out after {}s: {} {}",
                    timeout.as_secs(),
                    check.cmd,
                    check.args.join(" ")
                )));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(FabrError::Io(format!("wait {}: {e}", check.cmd))),
        }
    };
    let mut output = Vec::new();
    for r in readers {
        if let Ok(bytes) = r.join() {
            let room = OUTPUT_CAP.saturating_sub(output.len());
            output.extend_from_slice(&bytes[..bytes.len().min(room)]);
        }
    }
    Ok(Exec { status, output })
}

fn capped(mut reader: impl Read) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let room = OUTPUT_CAP.saturating_sub(kept.len());
                kept.extend_from_slice(&buf[..n.min(room)]);
            }
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(cmd: &str, args: &[&str]) -> Check {
        Check { cmd: cmd.into(), args: args.iter().map(|a| (*a).to_string()).collect() }
    }

    #[test]
    fn allowlist_rejects_interpreters_and_paths() {
        for bad in [check("env", &["sh", "-c", "id"]), check("python3", &["-c", "1"]), check("/bin/cat", &["x"]), check("sh", &[])] {
            assert!(validate_check(&bad).is_err(), "{bad:?}");
        }
        assert!(validate_check(&check("cat", &["/etc/passwd"])).is_err());
        assert!(validate_check(&check("cat", &["../x"])).is_err());
        assert!(validate_check(&check("test", &["-f", "greet.txt"])).is_ok());
    }

    #[test]
    fn a_blocked_check_is_killed_at_the_timeout() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), "x").unwrap();
        let c = check("tail", &["-f", "f"]);
        let started = Instant::now();
        let err = run_checks(dir.path(), std::slice::from_ref(&c), Duration::from_millis(300)).unwrap_err();
        assert!(matches!(err, FabrError::Timeout(_)), "{err:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn output_is_capped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big"), vec![b'a'; 200_000]).unwrap();
        let c = check("cat", &["big"]);
        let done = exec_allowed(dir.path(), std::slice::from_ref(&c), &c, Duration::from_secs(10)).unwrap();
        assert!(done.status.success());
        assert_eq!(done.output.len(), OUTPUT_CAP);
    }
}
