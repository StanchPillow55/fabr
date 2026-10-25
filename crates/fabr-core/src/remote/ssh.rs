use crate::error::FabrError;
use crate::remote::askpass::AskpassServer;
use crate::secrets::Secret;
use crate::sync::{Shared, lock};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct SshOutput {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

#[derive(Debug, Clone, Default)]
pub enum Auth {
    #[default]
    Batch,
    Password(Secret),
    KeyOnly,
}

static SSH_PROGRAM: Shared<Option<PathBuf>> = Shared::new(None);

pub fn set_ssh_program(p: Option<PathBuf>) {
    SSH_PROGRAM.set(p);
}

pub fn ssh_program() -> PathBuf {
    SSH_PROGRAM.get().unwrap_or_else(|| PathBuf::from("ssh"))
}

fn check_host(host: &str) -> Result<(), FabrError> {
    if host.is_empty() || host.starts_with('-') || host.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(FabrError::BadRequest(format!("bad host {host:?}")));
    }
    Ok(())
}

pub struct SshCommand {
    pub cmd: Command,
    pub askpass: Option<AskpassServer>,
}

pub fn ssh_command_auth(host: &str, cache_dir: &Path, auth: &Auth) -> Result<SshCommand, FabrError> {
    ssh_command_opts(host, cache_dir, auth, true)
}

pub const LOGIN_WAIT_TIMEOUT: Duration = Duration::from_secs(300);

pub fn ssh_command_opts(host: &str, cache_dir: &Path, auth: &Auth, keepalive: bool) -> Result<SshCommand, FabrError> {
    check_host(host)?;
    let sock_dir = cache_dir.join("ssh");
    private_dir(&sock_dir)?;
    let mut c = Command::new(ssh_program());
    c.arg("-T");
    let mut askpass = None;
    match auth {
        Auth::Batch => {
            c.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"]);
        }
        Auth::KeyOnly => {
            c.args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                "-o",
                "PasswordAuthentication=no",
                "-o",
                "KbdInteractiveAuthentication=no",
                "-o",
                "ControlMaster=no",
                "-o",
                "ControlPath=none",
            ]);
        }
        Auth::Password(pw) => {
            c.args([
                "-o",
                "BatchMode=no",
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
                "NumberOfPasswordPrompts=1",
                "-o",
                "ConnectTimeout=5",
                "-o",
                "KbdInteractiveAuthentication=yes",
                "-o",
                "PasswordAuthentication=yes",
            ]);
            let server = AskpassServer::start(cache_dir, pw.clone())?;
            server.apply(&mut c)?;
            askpass = Some(server);
        }
    }
    if !matches!(auth, Auth::KeyOnly) {
        if keepalive {
            c.args(["-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=2"]);
        } else {
            c.args(["-o", "ServerAliveInterval=0"]);
        }
        c.args(["-o", "ControlMaster=auto", "-o", "ControlPersist=60"]).arg("-o").arg(format!("ControlPath={}/%C", sock_dir.display()));
    }
    c.arg(host);
    Ok(SshCommand { cmd: c, askpass })
}

pub fn private_dir(dir: &Path) -> Result<(), FabrError> {
    std::fs::create_dir_all(dir).map_err(|e| FabrError::Io(format!("{}: {e}", dir.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

pub fn last_line(s: &str) -> String {
    s.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string()
}

pub fn run_command<T: Send + 'static>(
    cmd: Command,
    stdin: Option<Vec<u8>>,
    timeout: Duration,
    on_stdout: impl FnOnce(ChildStdout) -> (T, bool) + Send + 'static,
) -> Result<(i32, T, String), FabrError> {
    let (r, stderr) = run_command_full(cmd, stdin, timeout, on_stdout);
    r.map(|(code, v)| (code, v, stderr))
}

pub fn run_command_full<T: Send + 'static>(
    mut cmd: Command,
    stdin: Option<Vec<u8>>,
    timeout: Duration,
    on_stdout: impl FnOnce(ChildStdout) -> (T, bool) + Send + 'static,
) -> (Result<(i32, T), FabrError>, String) {
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let prog = cmd.get_program().to_string_lossy().into_owned();
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (Err(FabrError::Remote(format!("cannot run {prog}: {e}"))), String::new()),
    };
    if let (Some(data), Some(mut w)) = (stdin, child.stdin.take()) {
        std::thread::spawn(move || {
            let _ = w.write_all(&data);
        });
    }
    let mut err_pipe = child.stderr.take().expect("stderr piped");
    let err_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let err_thread = {
        let buf = err_buf.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(n) = err_pipe.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                lock(&buf).extend_from_slice(&chunk[..n]);
            }
            String::new()
        })
    };
    let collected = {
        let buf = err_buf.clone();
        move || String::from_utf8_lossy(&lock(&buf)).into_owned()
    };
    let out_pipe = child.stdout.take().expect("stdout piped");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(on_stdout(out_pipe));
    });
    let deadline = Instant::now() + timeout;
    let timed_out = |mut child: std::process::Child, err_thread: std::thread::JoinHandle<String>| {
        let _ = child.kill();
        let _ = child.wait();
        let until = Instant::now() + Duration::from_millis(300);
        while !err_thread.is_finished() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        let stderr = collected();
        (Err(FabrError::Timeout(format!("{prog} took longer than {}s", timeout.as_secs()))), stderr)
    };
    let (value, kill) = match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(v) => v,
        Err(_) => return timed_out(child, err_thread),
    };
    if kill {
        let _ = child.kill();
    }
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => return timed_out(child, err_thread),
            Err(e) => return (Err(FabrError::Io(e.to_string())), String::new()),
        }
    };
    let _ = err_thread.join();
    let stderr = collected();
    (Ok((status.code().unwrap_or(-1), value)), stderr)
}

pub fn timeout_error(host: &str, stderr: &str, timeout: FabrError) -> FabrError {
    let f = classify(stderr);
    if f.kind == SshFailureKind::LoginRequired { FabrError::Remote(f.message(host)) } else { timeout }
}

pub fn reset_master(host: &str, cache_dir: &Path) {
    let sock_dir = cache_dir.join("ssh");
    let exited = (|| -> Result<bool, FabrError> {
        check_host(host)?;
        let mut c = Command::new(ssh_program());
        c.args(["-T", "-O", "exit", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5"])
            .arg("-o")
            .arg(format!("ControlPath={}/%C", sock_dir.display()))
            .arg(host);
        let (code, _, _) = run_command(c, None, Duration::from_secs(5), collect)?;
        Ok(code == 0)
    })()
    .unwrap_or(false);
    if exited {
        return;
    }
    if let Ok(rd) = std::fs::read_dir(&sock_dir) {
        for e in rd.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

pub fn collect(mut out: ChildStdout) -> (Vec<u8>, bool) {
    let mut b = Vec::new();
    let _ = out.read_to_end(&mut b);
    (b, false)
}

pub fn run_auth(
    host: &str,
    command: &str,
    stdin: Option<&[u8]>,
    cache_dir: &Path,
    timeout: Duration,
    auth: &Auth,
) -> Result<SshOutput, FabrError> {
    let SshCommand { mut cmd, askpass } = ssh_command_auth(host, cache_dir, auth)?;
    cmd.arg(command);
    let (r, stderr) = run_command_full(cmd, stdin.map(<[u8]>::to_vec), timeout, collect);
    drop(askpass);
    let (status, stdout) = r.map_err(|e| if matches!(e, FabrError::Timeout(_)) { timeout_error(host, &stderr, e) } else { e })?;
    if status == 255 {
        return Err(ssh_failed(host, &stderr));
    }
    Ok(SshOutput { status, stdout, stderr })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshFailureKind {
    HostKeyUnknown,
    HostKeyChanged,
    AuthFailed,
    ConnectFailed,
    LoginRequired,
    Other,
}

impl SshFailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SshFailureKind::HostKeyUnknown => "host_key_unknown",
            SshFailureKind::HostKeyChanged => "host_key_changed",
            SshFailureKind::AuthFailed => "auth_failed",
            SshFailureKind::ConnectFailed => "connect_failed",
            SshFailureKind::LoginRequired => "login_required",
            SshFailureKind::Other => "other",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            SshFailureKind::HostKeyUnknown => "Unknown host key:",
            SshFailureKind::HostKeyChanged => "Host key changed:",
            SshFailureKind::AuthFailed => "Authentication failed:",
            SshFailureKind::ConnectFailed => "Could not connect:",
            SshFailureKind::LoginRequired => "Log in to Tailscale to reach",
            SshFailureKind::Other => "ssh failed:",
        }
    }

    pub fn from_message(m: &str) -> Option<SshFailureKind> {
        use SshFailureKind::*;
        [HostKeyUnknown, HostKeyChanged, AuthFailed, ConnectFailed, LoginRequired, Other].into_iter().find(|k| m.starts_with(k.prefix()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshFailure {
    pub kind: SshFailureKind,
    pub detail: String,
}

impl SshFailure {
    pub fn summary(&self, host: &str) -> String {
        let p = self.kind.prefix();
        match self.kind {
            SshFailureKind::HostKeyUnknown => format!("{p} {host} is not in your known_hosts yet"),
            SshFailureKind::HostKeyChanged => {
                format!("{p} {host} presents a different key than the one in your known_hosts")
            }
            SshFailureKind::AuthFailed => format!("{p} the host did not accept your key or password"),
            SshFailureKind::ConnectFailed => format!("{p} {}", self.detail.trim_start_matches("ssh: ")),
            SshFailureKind::LoginRequired => format!("{p} {host}: {}", self.detail),
            SshFailureKind::Other => format!("{p} {}", self.detail),
        }
    }

    pub fn message(&self, host: &str) -> String {
        let s = self.summary(host);
        if matches!(self.kind, SshFailureKind::ConnectFailed | SshFailureKind::LoginRequired | SshFailureKind::Other)
            || self.detail.is_empty()
        {
            s
        } else {
            format!("{s} (ssh: {})", self.detail)
        }
    }
}

pub fn login_url(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|l| {
        let (_, rest) = l.split_once("To authenticate, visit:")?;
        let start = rest.find("https://")?;
        let url: String = rest[start..].chars().take_while(|c| !c.is_whitespace()).collect();
        (url.len() > "https://".len()).then_some(url)
    })
}

pub fn classify(stderr: &str) -> SshFailure {
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let find = |pats: &[&str]| lines.iter().find(|l| pats.iter().any(|p| l.contains(p))).map(|l| l.to_string());
    let lower = stderr.to_ascii_lowercase();
    const CONNECT: [&str; 7] = [
        "Connection refused",
        "timed out",
        "Could not resolve",
        "No route to host",
        "Network is unreachable",
        "Connection closed by",
        "Connection reset",
    ];
    if let Some(url) = login_url(stderr) {
        return SshFailure { kind: SshFailureKind::LoginRequired, detail: url };
    }
    let (kind, detail) = if lower.contains("remote host identification has changed") || lower.contains("possible dns spoofing") {
        (SshFailureKind::HostKeyChanged, find(&["Host key verification failed", "IDENTIFICATION HAS CHANGED", "DNS SPOOFING"]))
    } else if stderr.contains("Host key verification failed") || stderr.contains("host key is known for") {
        (SshFailureKind::HostKeyUnknown, find(&["host key is known for", "Host key verification failed"]))
    } else if stderr.contains("Permission denied (") || lower.contains("too many authentication failures") {
        (SshFailureKind::AuthFailed, find(&["Permission denied", "Too many authentication failures"]))
    } else if CONNECT.iter().any(|p| stderr.contains(p)) {
        (SshFailureKind::ConnectFailed, find(&CONNECT))
    } else {
        (SshFailureKind::Other, None)
    };
    SshFailure { kind, detail: detail.unwrap_or_else(|| last_line(stderr)) }
}

pub fn ssh_failed(host: &str, stderr: &str) -> FabrError {
    if stderr.trim().is_empty() {
        return FabrError::Remote(format!("ssh to {host} failed"));
    }
    let f = classify(stderr);
    log::debug!("ssh {host}: {} ({})", f.kind.as_str(), f.detail);
    FabrError::Remote(f.message(host))
}

pub fn failure_parts(message: &str) -> (Option<SshFailureKind>, String, Option<String>) {
    let kind = SshFailureKind::from_message(message);
    match message.rsplit_once(" (ssh: ") {
        Some((s, d)) if kind.is_some() => (kind, s.to_string(), Some(d.trim_end_matches(')').to_string())),
        _ => (kind, message.to_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplexed_modes_get_keepalive() {
        let dir = std::env::temp_dir().join(format!("xs-ka-{}", std::process::id()));
        let args = |a: &Auth| -> Vec<String> {
            ssh_command_auth("h", &dir, a).unwrap().cmd.get_args().map(|s| s.to_string_lossy().into_owned()).collect()
        };
        for a in [Auth::Batch, Auth::Password(Secret::new("x"))] {
            let v = args(&a);
            assert!(v.contains(&"ServerAliveInterval=15".to_string()), "{v:?}");
            assert!(v.contains(&"ServerAliveCountMax=2".to_string()), "{v:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn classifies_ssh_stderr() {
        let unknown =
            "No ED25519 host key is known for devbox and you have requested strict checking.\r\nHost key verification failed.\r\n";
        let f = classify(unknown);
        assert_eq!(f.kind, SshFailureKind::HostKeyUnknown);
        assert!(f.detail.starts_with("No ED25519 host key"), "{f:?}");
        let changed = "@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@\n@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\n@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@\nIT IS POSSIBLE THAT SOMEONE IS DOING SOMETHING NASTY!\nHost key for devbox has changed and you have requested strict checking.\nHost key verification failed.\n";
        assert_eq!(classify(changed).kind, SshFailureKind::HostKeyChanged);
        assert_eq!(
            classify("@ WARNING: POSSIBLE DNS SPOOFING DETECTED! @\nHost key verification failed.").kind,
            SshFailureKind::HostKeyChanged
        );
        let auth = classify("me@devbox: Permission denied (publickey,password).\n");
        assert_eq!(auth.kind, SshFailureKind::AuthFailed);
        assert_eq!(auth.detail, "me@devbox: Permission denied (publickey,password).");
        assert_eq!(classify("Permission denied (password).").kind, SshFailureKind::AuthFailed);
        for c in [
            "ssh: connect to host devbox port 22: Connection refused",
            "ssh: connect to host 10.0.0.9 port 22: Operation timed out",
            "ssh: Could not resolve hostname nope: Name or service not known",
        ] {
            assert_eq!(classify(c).kind, SshFailureKind::ConnectFailed, "{c}");
        }
        assert_eq!(classify("kex_exchange_identification: banner line contains invalid characters").kind, SshFailureKind::Other);
    }

    const BANNER: &str =
        "# To authenticate, visit: https://login.tailscale.com/a/1a2b3c4d5e\r\nConnection to 100.64.0.7 port 22 timed out\n";

    #[test]
    fn classifies_the_tailscale_login_banner() {
        let f = classify(BANNER);
        assert_eq!(f.kind, SshFailureKind::LoginRequired, "beats the timeout line");
        assert_eq!(f.detail, "https://login.tailscale.com/a/1a2b3c4d5e");
        assert_eq!(f.message("box"), "Log in to Tailscale to reach box: https://login.tailscale.com/a/1a2b3c4d5e");
        assert_eq!(
            classify("To authenticate, visit: https://login.tailscale.com/a/x\nssh: connect to host h port 22: Connection refused").kind,
            SshFailureKind::LoginRequired
        );
        assert_eq!(classify("To authenticate, visit:\nConnection to h port 22 timed out").kind, SshFailureKind::ConnectFailed);
        let m = ssh_failed("box", BANNER).message();
        assert_eq!(failure_parts(&m).0, Some(SshFailureKind::LoginRequired));
        assert_eq!(SshFailureKind::LoginRequired.as_str(), "login_required");
    }

    #[test]
    fn run_command_timeout_returns_the_collected_stderr() {
        let mut c = Command::new("sh");
        c.arg("-c").arg("echo '# To authenticate, visit: https://login.tailscale.com/a/zz' >&2; sleep 30");
        let t = Instant::now();
        let (r, stderr) = run_command_full(c, None, Duration::from_millis(500), collect);
        assert!(matches!(r, Err(FabrError::Timeout(_))));
        assert!(t.elapsed() < Duration::from_secs(5));
        assert!(stderr.contains("https://login.tailscale.com/a/zz"), "{stderr:?}");
        let e = timeout_error("box", &stderr, FabrError::Timeout("x".into()));
        assert!(e.message().starts_with("Log in to Tailscale to reach box: https://"), "{}", e.message());
        assert!(matches!(timeout_error("box", "nothing", FabrError::Timeout("x".into())), FabrError::Timeout(_)));
    }

    #[test]
    fn login_wait_command_has_no_keepalive() {
        let dir = std::env::temp_dir().join(format!("xs-lw-{}", std::process::id()));
        let v: Vec<String> =
            ssh_command_opts("h", &dir, &Auth::Batch, false).unwrap().cmd.get_args().map(|s| s.to_string_lossy().into_owned()).collect();
        assert!(v.contains(&"ServerAliveInterval=0".to_string()) && !v.contains(&"ServerAliveInterval=15".to_string()), "{v:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn messages_carry_the_kind() {
        let e = ssh_failed("devbox", "me@devbox: Permission denied (publickey).");
        let m = e.message();
        assert_eq!(
            m,
            "Authentication failed: the host did not accept your key or password (ssh: me@devbox: Permission denied (publickey).)"
        );
        let (k, s, d) = failure_parts(&m);
        assert_eq!(k, Some(SshFailureKind::AuthFailed));
        assert_eq!(s, "Authentication failed: the host did not accept your key or password");
        assert_eq!(d.as_deref(), Some("me@devbox: Permission denied (publickey)."));
        let c = ssh_failed("x", "ssh: Could not resolve hostname x: nodename nor servname provided").message();
        assert_eq!(c, "Could not connect: Could not resolve hostname x: nodename nor servname provided");
        assert_eq!(failure_parts(&c).0, Some(SshFailureKind::ConnectFailed));
        assert_eq!(failure_parts("snapshot failed (exit 2): x").0, None);
    }
}
