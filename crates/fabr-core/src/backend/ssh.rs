use super::{ALREADY_NODE, Backend, CONFLICT_MSG, DirEntry, EXTRA_FILES_MSG, NOT_A_NODE, NodeFiles, dir_entries};
use crate::error::FabrError;
use crate::fs::{Fs, TarSnapshot};
use crate::node::META_FILE;
use crate::remote::askpass::AskpassServer;
use crate::remote::script::{
    EXIT_EXTRA_FILES, EXIT_IO, EXIT_MISMATCH, EXIT_NOT_NODE, init_script, list_script, node_script, parse_list, remove_script,
    shell_command, snapshot_script, write_script,
};
use crate::remote::ssh::{
    Auth, LOGIN_WAIT_TIMEOUT, SshCommand, collect, last_line, reset_master, run_command_full, ssh_command_opts, ssh_failed, timeout_error,
};
use crate::scan::source_prune;
use crate::secrets::Secret;
use crate::sources::{Source, remote_root};
use crate::sync::Shared;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::{ChildStdout, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const MAX_SNAPSHOT_ENTRIES: usize = 200_000;

pub struct SshBackend {
    pub source: Source,
    pub cache_dir: PathBuf,
    pub timeout: Duration,
    truncated: AtomicBool,
    password: Shared<Option<Option<Secret>>>,
}

type Prepared = (Command, Option<AskpassServer>);

enum Fail {
    Conflict(&'static str),
    Io,
    NotFound(String),
}

fn outcome(
    host: &str,
    what: &str,
    code: i32,
    stderr: &str,
    ok: &[i32],
    known: &[(i32, Fail)],
    other: fn(String) -> FabrError,
) -> Result<(), FabrError> {
    if ok.contains(&code) {
        return Ok(());
    }
    if code == 255 {
        return Err(ssh_failed(host, stderr));
    }
    let last = last_line(stderr);
    Err(match known.iter().find(|(c, _)| *c == code).map(|(_, f)| f) {
        Some(Fail::Conflict(m)) => FabrError::Conflict { message: (*m).into(), node: None },
        Some(Fail::Io) => FabrError::Io(format!("remote {what} failed: {last}")),
        Some(Fail::NotFound(m)) => FabrError::NotFound(m.clone()),
        None => other(format!("{what} failed (exit {code}): {last}")),
    })
}

fn leaf(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

impl SshBackend {
    pub fn new(source: Source, cache_dir: PathBuf, timeout: Duration) -> Self {
        let password = source.password.clone().filter(|p| !p.is_empty()).map(Some);
        SshBackend { source, cache_dir, timeout, truncated: AtomicBool::new(false), password: Shared::new(password) }
    }

    fn auth(&self) -> Auth {
        if let Some(p) = self.password.get() {
            return p.map_or(Auth::Batch, Auth::Password);
        }
        let p = crate::secrets::get_password(&self.source.id).filter(|p| !p.is_empty()).map(Secret::new);
        self.password.set(Some(p.clone()));
        p.map_or(Auth::Batch, Auth::Password)
    }

    fn host(&self) -> &str {
        self.source.host.as_deref().unwrap_or("")
    }

    fn command(&self, remote: &str, keepalive: bool) -> Result<Prepared, FabrError> {
        let SshCommand { mut cmd, askpass } = ssh_command_opts(self.host(), &self.cache_dir, &self.auth(), keepalive)?;
        cmd.arg(remote);
        Ok((cmd, askpass))
    }

    fn exec<T: Send + 'static>(
        &self,
        cmd: Command,
        stdin: Option<Vec<u8>>,
        timeout: Duration,
        on_stdout: impl FnOnce(ChildStdout) -> (T, bool) + Send + 'static,
    ) -> Result<(i32, T, String), FabrError> {
        let (r, stderr) = run_command_full(cmd, stdin, timeout, on_stdout);
        match r {
            Ok((code, v)) => Ok((code, v, stderr)),
            Err(e @ FabrError::Timeout(_)) => {
                let e = timeout_error(self.host(), &stderr, e);
                if matches!(e, FabrError::Timeout(_)) {
                    log::warn!("ssh {}: timed out; resetting the control master", self.host());
                    reset_master(self.host(), &self.cache_dir);
                }
                Err(e)
            }
            Err(e) => Err(e),
        }
    }

    fn run_script(&self, script: &str, args: &[&str], stdin: Option<Vec<u8>>) -> Result<(i32, Vec<u8>, String), FabrError> {
        let (cmd, _askpass) = self.command(&shell_command(script, args), true)?;
        self.exec(cmd, stdin, self.timeout, collect)
    }

    fn node_dir(&self, path: &str) -> String {
        let root = remote_root(&self.source);
        if path.is_empty() { root } else { format!("{}/{path}", root.trim_end_matches('/')) }
    }

    fn fetch_tar(&self, script: &str, args: &[&str], cap: usize, wait_for_login: bool) -> Result<(TarSnapshot, bool), FabrError> {
        let (cmd, _askpass) = self.command(&shell_command(script, args), !wait_for_login)?;
        let timeout = if wait_for_login { LOGIN_WAIT_TIMEOUT } else { self.timeout };
        let (code, parsed, stderr) = self.exec(cmd, None, timeout, move |out| {
            let r = TarSnapshot::from_tar_capped(out, cap);
            let kill = matches!(r, Ok((_, true)));
            (r, kill)
        })?;
        if matches!(parsed, Ok((_, true))) {
            return parsed;
        }
        let ok: &[i32] = if matches!(&parsed, Ok((s, _)) if !s.dirs.is_empty()) { &[0, 1, 2] } else { &[0, 1] };
        let missing = format!("{}:{} is not a directory or not a node", self.host(), args.join("/"));
        outcome(self.host(), "snapshot", code, &stderr, ok, &[(EXIT_NOT_NODE, Fail::NotFound(missing))], FabrError::Remote)?;
        parsed
    }

    fn write_one(&self, path: &str, expected: &str, name: &str, bytes: &[u8]) -> Result<String, FabrError> {
        let dir = self.node_dir(path);
        let (code, out, stderr) = self.run_script(&write_script(), &[&dir, expected, name], Some(bytes.to_vec()))?;
        let known =
            [(EXIT_MISMATCH, Fail::Conflict(CONFLICT_MSG)), (EXIT_IO, Fail::Io), (EXIT_NOT_NODE, Fail::NotFound(NOT_A_NODE.into()))];
        outcome(self.host(), "write", code, &stderr, &[0], &known, FabrError::Remote)?;
        let v = String::from_utf8_lossy(&out).trim().to_string();
        if v.len() == 40 { Ok(v) } else { Err(FabrError::Remote(format!("write returned {v:?}"))) }
    }

    fn fetch_snapshot(&self, wait_for_login: bool) -> Result<TarSnapshot, FabrError> {
        let mut prune: Vec<String> = source_prune(&self.source).into_iter().collect();
        prune.sort();
        let root = remote_root(&self.source);
        let (snap, capped) = self.fetch_tar(&snapshot_script(&prune)?, &[&root], MAX_SNAPSHOT_ENTRIES, wait_for_login)?;
        self.truncated.store(capped, Ordering::Relaxed);
        if !snap.dirs.contains("") {
            return Err(FabrError::Remote(format!("empty snapshot from {}", self.host())));
        }
        Ok(snap)
    }
}

impl Backend for SshBackend {
    fn snapshot(&self) -> Result<Arc<dyn Fs>, FabrError> {
        Ok(Arc::new(self.fetch_snapshot(false)?))
    }

    fn stamp(&self) -> Result<String, FabrError> {
        Ok(self.fetch_snapshot(false)?.hash())
    }

    fn snapshot_with_stamp(&self) -> Result<(Arc<dyn Fs>, String), FabrError> {
        let s = self.fetch_snapshot(false)?;
        let h = s.hash();
        Ok((Arc::new(s), h))
    }

    fn snapshot_waiting_for_login(&self) -> Result<(Arc<dyn Fs>, String), FabrError> {
        let s = self.fetch_snapshot(true)?;
        reset_master(self.host(), &self.cache_dir);
        let h = s.hash();
        Ok((Arc::new(s), h))
    }

    fn read_node(&self, path: &str) -> Result<Arc<dyn Fs>, FabrError> {
        let root = remote_root(&self.source);
        let rel = if path.is_empty() { "." } else { path };
        let (snap, _) = self.fetch_tar(&node_script(), &[&root, rel], 1000, false)?;
        Ok(Arc::new(snap))
    }

    fn forget_password(&self) {
        if self.source.password.is_none() {
            self.password.set(None);
        }
    }

    fn truncated(&self) -> bool {
        self.truncated.load(Ordering::Relaxed)
    }

    fn write_node(&self, path: &str, expected_version: &str, files: NodeFiles) -> Result<(), FabrError> {
        if let Some(b) = &files.meta {
            self.write_one(path, expected_version, META_FILE, b)?;
        }
        Ok(())
    }

    fn remove_node(&self, path: &str, expected_version: &str) -> Result<(), FabrError> {
        let dir = self.node_dir(path);
        let (code, _, stderr) = self.run_script(&remove_script(), &[&dir, expected_version], None)?;
        let known = [
            (EXIT_MISMATCH, Fail::Conflict(CONFLICT_MSG)),
            (EXIT_EXTRA_FILES, Fail::Conflict(EXTRA_FILES_MSG)),
            (EXIT_NOT_NODE, Fail::NotFound(NOT_A_NODE.into())),
            (EXIT_IO, Fail::Io),
        ];
        outcome(self.host(), "remove", code, &stderr, &[0], &known, FabrError::Remote)
    }

    fn list_dirs(&self, path: &str, prune: &HashSet<String>) -> Result<Vec<DirEntry>, FabrError> {
        let root = remote_root(&self.source);
        let rel = if path.is_empty() { "." } else { path };
        let (code, out, stderr) = self.run_script(&list_script(), &[&root, rel], None)?;
        let known = [(EXIT_NOT_NODE, Fail::NotFound("no such directory".into()))];
        outcome(self.host(), "list", code, &stderr, &[0, 1], &known, FabrError::Remote)?;
        Ok(dir_entries(parse_list(&out), prune))
    }

    fn init_node(&self, path: &str, show_depth: Option<u32>) -> Result<(), FabrError> {
        let dir = self.node_dir(path);
        let (code, _, stderr) = self.run_script(&init_script(), &[&dir], None)?;
        let known = [(EXIT_MISMATCH, Fail::Conflict(ALREADY_NODE)), (EXIT_NOT_NODE, Fail::NotFound("no such directory".into()))];
        outcome(self.host(), "init", code, &stderr, &[0], &known, FabrError::Io)?;
        let name = if path.is_empty() {
            Some(leaf(dir.trim_end_matches('/'))).filter(|n| !n.is_empty() && *n != ".").unwrap_or(&self.source.name)
        } else {
            leaf(path)
        };
        self.write_node(path, &crate::node::version_of(b""), NodeFiles::initial(name, show_depth))
    }
}
