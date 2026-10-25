use super::{SSH_TIMEOUT, Workspace, log_scan};
use crate::dto::{HostReq, InstallKeyReq, PasswordState, SourceIdReq, SourcePasswordReq, TrustResult};
use crate::error::FabrError;
use crate::remote::hostkey;
use crate::remote::keyinstall::{self, InstallKeyResult};
use crate::remote::ssh::{Auth, SshFailureKind, failure_parts, last_line, run_auth};
use crate::scan::{count_nodes, scan, source_prune};
use crate::secrets::Secret;
use crate::settings::TestResult;
use crate::sources::{Source, validate_source};
use std::path::{Path, PathBuf};
use std::time::Instant;

impl Workspace {
    pub fn test_source(&self, source: Source) -> TestResult {
        let start = Instant::now();
        let home = self.home();
        let probed = self.probe(source.clone(), start);
        let (ok, message, latency_ms, node_count, managed_root, kind, detail) = match probed {
            Ok((message, latency_ms, n, managed)) => (true, message, latency_ms, n, managed, None, None),
            Err(e) => {
                let (message, kind, detail) = explain_failure(source.host.as_deref(), &home, e.message());
                (false, message, start.elapsed().as_millis() as u64, 0, false, kind, detail)
            }
        };
        TestResult { ok, message, latency_ms, node_count, managed_root, failure_kind: kind.map(|k| k.as_str().to_string()), detail }
    }

    fn probe(&self, source: Source, start: Instant) -> Result<(String, u64, u32, bool), FabrError> {
        let source = validate_source(source)?;
        let backend = self.backend_for(&source);
        let fs = backend.snapshot()?;
        let latency_ms = start.elapsed().as_millis() as u64;
        let prune = source_prune(&source);
        let r = scan(fs.as_ref(), &source, &self.scan_opts(&source, false, &prune));
        log_scan(&source.id, " (test)", &r);
        let n = count_nodes(&r.root) as u32;
        let more = if r.truncated || backend.truncated() { "+" } else { "" };
        let message = match &r.truncated_reason {
            Some(why) => format!("connected: {n}{more} nodes in {latency_ms} ms ({why})"),
            None => format!("connected: {n}{more} nodes in {latency_ms} ms"),
        };
        Ok((message, latency_ms, n, r.root.managed))
    }

    pub fn home(&self) -> PathBuf {
        self.home.get()
    }

    pub fn set_home(&self, home: PathBuf) {
        self.home.set(home);
    }

    pub fn host_key(&self, req: HostReq) -> Result<Vec<hostkey::HostKey>, FabrError> {
        let keys = hostkey::host_keys(req.host.trim())?;
        log::info!("host_key {}: {} key(s)", req.host.trim(), keys.len());
        Ok(keys)
    }

    pub fn trust_host(&self, req: HostReq) -> Result<TrustResult, FabrError> {
        let t = hostkey::resolve(req.host.trim())?;
        let added = hostkey::trust_in(&self.home(), &t.known_name, &req.lines)?;
        Ok(TrustResult { added: added as u32 })
    }

    pub fn set_source_password(&self, req: SourcePasswordReq) -> Result<PasswordState, FabrError> {
        let id = req.source.trim();
        if !crate::sources::id_ok(id) {
            return Err(FabrError::BadRequest(format!("source id {id:?} must match ^[a-z0-9_-]+$")));
        }
        crate::secrets::set_password(id, req.password.expose())?;
        if let Ok(e) = self.entry(id) {
            e.backend.forget_password();
        }
        log::info!("password for source {id} {}", if req.password.is_empty() { "cleared" } else { "stored in the keychain" });
        Ok(PasswordState { stored: !req.password.is_empty() })
    }

    pub fn has_source_password(&self, req: SourceIdReq) -> PasswordState {
        PasswordState { stored: crate::secrets::get_password(req.source.trim()).is_some() }
    }

    pub fn install_key(&self, req: InstallKeyReq) -> Result<InstallKeyResult, FabrError> {
        let id = req.source.trim().to_string();
        let saved = self.entry(&id).ok().and_then(|e| e.source.host);
        let host = match (req.host.map(|h| h.trim().to_string()).filter(|h| !h.is_empty()), saved) {
            (Some(h), _) | (None, Some(h)) => h,
            (None, None) => return Err(FabrError::BadRequest(format!("source {id:?} has no ssh host"))),
        };
        let password = req.password.filter(|p| !p.is_empty()).or_else(|| crate::secrets::get_password(&id).map(Secret::new));
        let key = keyinstall::ensure_key(&host, &self.home())?;
        let auth = password.map_or(Auth::Batch, Auth::Password);
        let line = crate::remote::script::shell_command(&keyinstall::install_script(), &[&key.line, &key.type_and_key]);
        let out = run_auth(&host, &line, None, &self.cache_dir, SSH_TIMEOUT, &auth)?;
        if out.status != 0 {
            return Err(FabrError::Remote(format!(
                "could not update ~/.ssh/authorized_keys on {host} (exit {}): {}",
                out.status,
                last_line(&out.stderr)
            )));
        }
        let made = if key.generated { format!("Created {} (no passphrase). ", key.private.display()) } else { String::new() };
        log::info!("install_key {id}: key {} installed on {host}", key.private.display());
        let verified = run_auth(&host, "true", None, &self.cache_dir, SSH_TIMEOUT, &Auth::KeyOnly);
        let (installed, message) = match verified {
            Ok(o) if o.status == 0 => {
                (true, format!("{made}Key installed: {host} now accepts {} without a password.", key.private.display()))
            }
            Ok(o) => (false, format!("{made}The key was added, but the check login failed (exit {}).", o.status)),
            Err(e) => (
                false,
                format!(
                    "{made}The key was added to {host}, but a key-only login still fails ({}). Check IdentityFile in ~/.ssh/config and the permissions of ~/.ssh on the host.",
                    e.message()
                ),
            ),
        };
        Ok(InstallKeyResult { installed, generated_key: key.generated, message })
    }
}

fn explain_failure(host: Option<&str>, home: &Path, m: String) -> (String, Option<SshFailureKind>, Option<String>) {
    log::info!("test_source failed: {m}");
    let (mut kind, message, detail) = failure_parts(&m);
    if kind == Some(SshFailureKind::HostKeyUnknown)
        && let Some(h) = host
        && let Ok(t) = hostkey::resolve(h)
        && let Ok(text) = std::fs::read_to_string(hostkey::known_hosts_path(home))
        && !hostkey::known_keys(&text, &t.known_name).is_empty()
    {
        kind = Some(SshFailureKind::HostKeyChanged);
    }
    let message = match kind {
        Some(SshFailureKind::HostKeyChanged) if !message.starts_with("Host key changed") => {
            format!("Host key changed: {} presents a different key than the one in your known_hosts", host.unwrap_or(""))
        }
        _ => message,
    };
    (message, kind, detail)
}
