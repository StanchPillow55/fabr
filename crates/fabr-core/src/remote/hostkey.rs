use crate::error::FabrError;
use crate::remote::ssh::{last_line, ssh_program};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const KEY_TYPES: [&str; 5] = ["ssh-ed25519", "ecdsa-sha2-nistp256", "ecdsa-sha2-nistp384", "ecdsa-sha2-nistp521", "ssh-rsa"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostKey {
    #[serde(rename = "type")]
    pub key_type: String,
    pub fingerprint: String,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub hostname: String,
    pub port: u16,
    pub known_name: String,
}

pub fn parse_ssh_g(out: &str, host: &str) -> Target {
    let mut hostname = None;
    let mut port = 22u16;
    let mut alias = None;
    for l in out.lines() {
        let Some((k, v)) = l.split_once(' ') else { continue };
        match k {
            "hostname" => hostname = Some(v.trim().to_string()),
            "port" => port = v.trim().parse().unwrap_or(22),
            "hostkeyalias" => alias = Some(v.trim().to_string()),
            _ => {}
        }
    }
    let hostname = hostname.unwrap_or_else(|| host.rsplit('@').next().unwrap_or(host).to_string());
    let name = alias.unwrap_or_else(|| hostname.clone());
    let known_name = if port == 22 { name } else { format!("[{name}]:{port}") };
    Target { hostname, port, known_name }
}

fn check_host(host: &str) -> Result<(), FabrError> {
    if host.is_empty() || host.starts_with('-') || host.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(FabrError::BadRequest(format!("bad host {host:?}")));
    }
    Ok(())
}

pub fn resolve(host: &str) -> Result<Target, FabrError> {
    check_host(host)?;
    let out = Command::new(ssh_program())
        .arg("-G")
        .arg(host)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| FabrError::Remote(format!("cannot run ssh: {e}")))?;
    if !out.status.success() {
        return Err(FabrError::BadRequest(format!("ssh -G {host}: {}", last_line(&String::from_utf8_lossy(&out.stderr)))));
    }
    Ok(parse_ssh_g(&String::from_utf8_lossy(&out.stdout), host))
}

pub fn fingerprint(b64: &str) -> Result<String, FabrError> {
    let blob = STANDARD.decode(b64.trim()).map_err(|_| FabrError::BadRequest("key is not base64".into()))?;
    Ok(format!("SHA256:{}", STANDARD_NO_PAD.encode(sha2::Sha256::digest(&blob))))
}

fn blob_type(b64: &str) -> Option<String> {
    let blob = STANDARD.decode(b64).ok()?;
    let n = u32::from_be_bytes(blob.get(..4)?.try_into().ok()?) as usize;
    String::from_utf8(blob.get(4..4 + n)?.to_vec()).ok()
}

fn parse_key_line(l: &str) -> Option<(String, String, String)> {
    let f: Vec<&str> = l.split_whitespace().collect();
    let [name, ty, key] = f.as_slice() else { return None };
    if !KEY_TYPES.contains(ty) || blob_type(key).as_deref() != Some(*ty) {
        return None;
    }
    Some((name.to_string(), ty.to_string(), key.to_string()))
}

pub fn parse_keyscan(out: &str, known_name: &str) -> Vec<HostKey> {
    let mut keys: Vec<HostKey> = out
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(parse_key_line)
        .filter_map(|(_, ty, key)| {
            Some(HostKey { fingerprint: fingerprint(&key).ok()?, line: format!("{known_name} {ty} {key}"), key_type: ty })
        })
        .collect();
    keys.sort_by_key(|k| KEY_TYPES.iter().position(|t| *t == k.key_type));
    keys.dedup();
    keys
}

pub fn host_keys(host: &str) -> Result<Vec<HostKey>, FabrError> {
    let t = resolve(host)?;
    let out = Command::new("ssh-keyscan")
        .args(["-T", "5", "-p", &t.port.to_string(), "-t", "ed25519,ecdsa,rsa", "--"])
        .arg(&t.hostname)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| FabrError::Remote(format!("cannot run ssh-keyscan: {e}")))?;
    let keys = parse_keyscan(&String::from_utf8_lossy(&out.stdout), &t.known_name);
    if keys.is_empty() {
        let why = last_line(&String::from_utf8_lossy(&out.stderr));
        return Err(FabrError::Remote(format!(
            "Could not connect: no host keys from {}:{}{}",
            t.hostname,
            t.port,
            if why.is_empty() { String::new() } else { format!(" ({why})") }
        )));
    }
    Ok(keys)
}

fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..20].copy_from_slice(&sha1_smol::Sha1::from(key).digest().bytes());
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let pad = |b: u8| k.iter().map(|x| x ^ b).collect::<Vec<u8>>();
    let mut inner = sha1_smol::Sha1::new();
    inner.update(&pad(0x36));
    inner.update(msg);
    let mut outer = sha1_smol::Sha1::new();
    outer.update(&pad(0x5c));
    outer.update(&inner.digest().bytes());
    outer.digest().bytes()
}

fn glob(p: &[u8], s: &[u8]) -> bool {
    match (p.first(), s.first()) {
        (None, None) => true,
        (Some(b'*'), _) => glob(&p[1..], s) || (!s.is_empty() && glob(p, &s[1..])),
        (Some(b'?'), Some(_)) => glob(&p[1..], &s[1..]),
        (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => glob(&p[1..], &s[1..]),
        _ => false,
    }
}

pub fn host_field_matches(field: &str, name: &str) -> bool {
    if let Some(rest) = field.strip_prefix("|1|") {
        let Some((salt, hash)) = rest.split_once('|') else { return false };
        let (Ok(salt), Ok(hash)) = (STANDARD.decode(salt), STANDARD.decode(hash)) else { return false };
        return hmac_sha1(&salt, name.as_bytes()).as_slice() == hash.as_slice();
    }
    let mut hit = false;
    for p in field.split(',') {
        if let Some(neg) = p.strip_prefix('!') {
            if glob(neg.as_bytes(), name.as_bytes()) {
                return false;
            }
        } else if glob(p.as_bytes(), name.as_bytes()) {
            hit = true;
        }
    }
    hit
}

pub fn known_keys(text: &str, name: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('@'))
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            (f.len() >= 3 && host_field_matches(f[0], name)).then(|| (f[1].to_string(), f[2].to_string()))
        })
        .collect()
}

pub fn known_hosts_path(home: &Path) -> PathBuf {
    home.join(".ssh").join("known_hosts")
}

pub fn trust_in(home: &Path, known_name: &str, lines: &[String]) -> Result<usize, FabrError> {
    if lines.is_empty() {
        return Err(FabrError::BadRequest("no host key lines to trust".into()));
    }
    let mut new: Vec<(String, String)> = Vec::new();
    for l in lines {
        match parse_key_line(l) {
            Some((name, ty, key)) if name == known_name => new.push((ty, key)),
            Some((name, ..)) => return Err(FabrError::BadRequest(format!("key line is for {name}, not {known_name}"))),
            None => return Err(FabrError::BadRequest("not a host key line".into())),
        }
    }
    let path = known_hosts_path(home);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(FabrError::Io(format!("{}: {e}", path.display()))),
    };
    let have = known_keys(&text, known_name);
    if have.iter().any(|k| !new.contains(k)) {
        return Err(FabrError::Conflict {
            message: format!("host_key_changed: your known_hosts already has a different key for {known_name}; nothing was changed"),
            node: None,
        });
    }
    let add: Vec<&(String, String)> = new.iter().filter(|k| !have.contains(k)).collect();
    let mut add_unique: Vec<&(String, String)> = Vec::new();
    for k in add {
        if !add_unique.contains(&k) {
            add_unique.push(k);
        }
    }
    if add_unique.is_empty() {
        return Ok(0);
    }
    let dir = path.parent().expect("known_hosts has a parent");
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| FabrError::Io(format!("{}: {e}", dir.display())))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| FabrError::Io(format!("{}: {e}", dir.display())))?;
        }
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))?;
    let mut out = String::new();
    if !text.is_empty() && !text.ends_with('\n') {
        out.push('\n');
    }
    for (ty, key) in &add_unique {
        out.push_str(&format!("{known_name} {ty} {key}\n"));
    }
    f.write_all(out.as_bytes()).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))?;
    log::info!("trusted {} host key(s) for {known_name}", add_unique.len());
    Ok(add_unique.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl";
    const ED2: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

    #[test]
    fn ssh_g_and_keyscan() {
        let t = parse_ssh_g("user me\nhostname 10.0.0.5\nport 2222\n", "box");
        assert_eq!(t, Target { hostname: "10.0.0.5".into(), port: 2222, known_name: "[10.0.0.5]:2222".into() });
        assert_eq!(parse_ssh_g("hostname gh\nport 22\nhostkeyalias alias\n", "x").known_name, "alias");
        let scan = format!("# github.com:22 SSH-2.0-x\ngithub.com ssh-ed25519 {ED}\ngithub.com ssh-rsa {ED}\njunk\n");
        let k = parse_keyscan(&scan, "gh");
        assert_eq!(k.len(), 1, "type mismatch inside the blob is dropped");
        assert_eq!(k[0].key_type, "ssh-ed25519");
        assert_eq!(k[0].line, format!("gh ssh-ed25519 {ED}"));
        assert_eq!(k[0].fingerprint, "SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU");
    }

    #[test]
    fn hashed_and_pattern_hosts() {
        let salt = [7u8; 20];
        let h = hmac_sha1(&salt, b"devbox");
        let field = format!("|1|{}|{}", STANDARD.encode(salt), STANDARD.encode(h));
        assert!(host_field_matches(&field, "devbox"));
        assert!(!host_field_matches(&field, "other"));
        assert!(host_field_matches("a,devbox,b", "devbox"));
        assert!(host_field_matches("*.lan", "x.lan"));
        assert!(!host_field_matches("*.lan,!x.lan", "x.lan"));
        assert!(host_field_matches("[devbox]:2222", "[devbox]:2222"));
        assert!(!host_field_matches("devbox", "[devbox]:2222"));
        let mac = hmac_sha1(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(mac.iter().map(|b| format!("{b:02x}")).collect::<String>(), "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79");
    }

    #[test]
    fn known_hosts_append_dedupe_and_changed() {
        let home = tempfile::tempdir().unwrap();
        let line = format!("devbox ssh-ed25519 {ED}");
        assert_eq!(trust_in(home.path(), "devbox", &[line.clone(), line.clone()]).unwrap(), 1);
        let p = known_hosts_path(home.path());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(std::fs::metadata(p.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        }
        assert_eq!(std::fs::read_to_string(&p).unwrap(), format!("{line}\n"));
        assert_eq!(trust_in(home.path(), "devbox", std::slice::from_ref(&line)).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(&p).unwrap().lines().count(), 1);
        let before = std::fs::read_to_string(&p).unwrap();
        let e = trust_in(home.path(), "devbox", &[format!("devbox ssh-ed25519 {ED2}")]).unwrap_err();
        assert_eq!(e.code(), 409);
        assert!(e.message().starts_with("host_key_changed"));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), before);
        std::fs::write(&p, before.trim_end()).unwrap();
        assert_eq!(trust_in(home.path(), "[devbox]:2222", &[format!("[devbox]:2222 ssh-ed25519 {ED2}")]).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(&p).unwrap().lines().count(), 2);
        assert_eq!(trust_in(home.path(), "devbox", &[format!("evil ssh-ed25519 {ED}")]).unwrap_err().code(), 400);
        assert_eq!(trust_in(home.path(), "devbox", &[format!("@cert-authority devbox ssh-ed25519 {ED}")]).unwrap_err().code(), 400);
        assert_eq!(trust_in(home.path(), "devbox", &["devbox ssh-ed25519 !!".into()]).unwrap_err().code(), 400);
    }
}
