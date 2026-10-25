use crate::error::FabrError;
use crate::remote::ssh::{last_line, ssh_program};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallKeyResult {
    pub installed: bool,
    pub generated_key: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct LocalKey {
    pub private: PathBuf,
    pub line: String,
    pub type_and_key: String,
    pub generated: bool,
}

pub fn install_script() -> String {
    "umask 077; d=\"$HOME/.ssh\"; f=\"$d/authorized_keys\"; mkdir -p \"$d\" && touch \"$f\" || exit 43; \
     grep -qF -- \"$2\" \"$f\" && exit 0; \
     if [ -s \"$f\" ] && [ -n \"$(tail -c 1 \"$f\")\" ]; then echo >>\"$f\"; fi; \
     printf %s \"$1\" >>\"$f\" && echo >>\"$f\""
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse_pubkey(line: &str) -> Result<(String, String), FabrError> {
    let line = line.trim();
    let bad = || FabrError::Unprocessable("the public key file is not a single ssh public key line".into());
    if line.contains(['\n', '\r', '\\', '\'', '"', '`', '$']) || line.chars().any(char::is_control) {
        return Err(bad());
    }
    let mut f = line.split_whitespace();
    let (Some(ty), Some(key)) = (f.next(), f.next()) else { return Err(bad()) };
    let ok_type = ty.starts_with("ssh-") || ty.starts_with("ecdsa-") || ty.starts_with("sk-");
    if !ok_type || base64::engine::general_purpose::STANDARD.decode(key).is_err() {
        return Err(bad());
    }
    Ok((line.to_string(), format!("{ty} {key}")))
}

pub fn identity_files(ssh_g: &str, home: &Path) -> Vec<PathBuf> {
    ssh_g
        .lines()
        .filter_map(|l| l.strip_prefix("identityfile "))
        .filter_map(|p| {
            let p = p.trim();
            let h = home.display().to_string();
            let p = if let Some(rest) = p.strip_prefix("~/") { format!("{h}/{rest}") } else { p.replace("%d", &h) };
            (!p.contains('%')).then(|| PathBuf::from(p))
        })
        .collect()
}

fn read_pub(private: &Path) -> Result<Option<String>, FabrError> {
    let mut p = private.as_os_str().to_owned();
    p.push(".pub");
    match std::fs::read_to_string(PathBuf::from(p)) {
        Ok(s) => Ok(Some(s.lines().next().unwrap_or("").to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let out = Command::new("ssh-keygen").args(["-y", "-P", ""]).arg("-f").arg(private).stdin(Stdio::null()).output();
            match out {
                Ok(o) if o.status.success() => Ok(Some(String::from_utf8_lossy(&o.stdout).trim().to_string())),
                _ => Ok(None),
            }
        }
        Err(e) => Err(FabrError::Io(format!("{}: {e}", private.display()))),
    }
}

pub fn ensure_key(host: &str, home: &Path) -> Result<LocalKey, FabrError> {
    let g = Command::new(ssh_program()).arg("-G").arg(host).stdin(Stdio::null()).output();
    let ids = match g {
        Ok(o) if o.status.success() => identity_files(&String::from_utf8_lossy(&o.stdout), home),
        _ => Vec::new(),
    };
    for id in &ids {
        if id.is_file()
            && let Some(line) = read_pub(id)?
        {
            let (line, type_and_key) = parse_pubkey(&line)?;
            return Ok(LocalKey { private: id.clone(), line, type_and_key, generated: false });
        }
    }
    let ed = home.join(".ssh").join("id_ed25519");
    if ed.exists() {
        return match read_pub(&ed)? {
            Some(line) => {
                let (line, type_and_key) = parse_pubkey(&line)?;
                Ok(LocalKey { private: ed, line, type_and_key, generated: false })
            }
            None => Err(FabrError::Unprocessable(format!(
                "{} exists but has no .pub file; run `ssh-keygen -y -f {0} > {0}.pub` in Terminal",
                ed.display()
            ))),
        };
    }
    crate::remote::ssh::private_dir(&home.join(".ssh"))?;
    let comment = format!("fabr@{}", hostname());
    let out = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", &comment, "-f"])
        .arg(&ed)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| FabrError::Io(format!("cannot run ssh-keygen: {e}")))?;
    if !out.status.success() {
        return Err(FabrError::Io(format!("ssh-keygen failed: {}", last_line(&String::from_utf8_lossy(&out.stderr)))));
    }
    log::info!("generated {}", ed.display());
    let line = read_pub(&ed)?.ok_or_else(|| FabrError::Io("ssh-keygen wrote no public key".into()))?;
    let (line, type_and_key) = parse_pubkey(&line)?;
    Ok(LocalKey { private: ed, line, type_and_key, generated: true })
}

fn hostname() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .unwrap_or_else(|| "localhost".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_and_parsing() {
        let s = install_script();
        assert!(!s.contains('\\') && !s.contains('\n'), "{s}");
        let (line, tk) = parse_pubkey("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl me@mac\n").unwrap();
        assert_eq!(line, "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl me@mac");
        assert_eq!(tk, "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl");
        assert!(parse_pubkey("ssh-ed25519 AAAA me'; rm -rf ~").is_err());
        assert!(parse_pubkey("garbage").is_err());
        let ids = identity_files("user x\nidentityfile ~/.ssh/id_rsa\nidentityfile %d/.ssh/k\nidentityfile /a/%h\n", Path::new("/h"));
        assert_eq!(ids, [PathBuf::from("/h/.ssh/id_rsa"), PathBuf::from("/h/.ssh/k")]);
    }

    #[test]
    fn script_appends_once() {
        let home = tempfile::tempdir().unwrap();
        let key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl me@mac";
        let tk = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl";
        let ak = home.path().join(".ssh/authorized_keys");
        let run = || {
            let line = crate::remote::script::shell_command(&install_script(), &[key, tk]);
            let st = Command::new("sh").arg("-c").arg(&line).env("HOME", home.path()).status().unwrap();
            assert!(st.success());
        };
        run();
        std::fs::write(&ak, format!("{}ssh-rsa AAAA other", std::fs::read_to_string(&ak).unwrap())).unwrap();
        run();
        assert_eq!(std::fs::read_to_string(&ak).unwrap(), format!("{key}\nssh-rsa AAAA other"));
        std::fs::write(&ak, "ssh-rsa AAAA other").unwrap();
        run();
        assert_eq!(std::fs::read_to_string(&ak).unwrap(), format!("ssh-rsa AAAA other\n{key}\n"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(home.path().join(".ssh")).unwrap().permissions().mode() & 0o777, 0o700);
        }
    }
}
