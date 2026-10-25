use crate::error::FabrError;
use crate::secrets::Secret;
use crate::sync::Shared;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const ENV_SOCK: &str = "FABR_ASKPASS_SOCK";
pub const ENV_TOKEN: &str = "FABR_ASKPASS_TOKEN";

static HELPER: Shared<Option<PathBuf>> = Shared::new(None);

pub fn set_helper(p: Option<PathBuf>) {
    HELPER.set(p);
}

fn helper() -> Result<PathBuf, FabrError> {
    if let Some(p) = HELPER.get() {
        return Ok(p);
    }
    std::env::current_exe().map_err(|e| FabrError::Io(format!("cannot find the askpass helper: {e}")))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Password,
    No,
    Refuse,
}

pub fn answer_for(prompt: &str) -> Answer {
    let p = prompt.to_ascii_lowercase();
    if p.contains("yes/no") || p.contains("(yes/no") || p.contains("continue connecting") {
        Answer::No
    } else if p.contains("password") {
        Answer::Password
    } else {
        Answer::Refuse
    }
}

fn random_hex(bytes: usize) -> String {
    (0..bytes).map(|_| format!("{:02x}", rand::random::<u8>())).collect()
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(unix)]
mod imp {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    pub struct AskpassServer {
        path: PathBuf,
        pub(super) token: String,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl AskpassServer {
        pub fn start(cache_dir: &Path, password: Secret) -> Result<AskpassServer, FabrError> {
            let dir = cache_dir.join("askpass");
            crate::remote::ssh::private_dir(&dir)?;
            let path = dir.join(format!("{}.sock", random_hex(8)));
            let _ = std::fs::remove_file(&path);
            let listener = {
                use std::os::unix::fs::PermissionsExt;
                let l = UnixListener::bind(&path).map_err(|e| FabrError::Io(format!("askpass socket {}: {e}", path.display())))?;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| FabrError::Io(format!("askpass socket {}: {e}", path.display())))?;
                l
            };
            listener.set_nonblocking(true).map_err(|e| FabrError::Io(e.to_string()))?;
            let token = random_hex(32);
            let stop = Arc::new(AtomicBool::new(false));
            let (tok, st) = (token.clone(), stop.clone());
            let thread = std::thread::spawn(move || {
                let mut answered = false;
                while !st.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((conn, _)) => serve_one(conn, &tok, &password, &mut answered),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(15)),
                        Err(e) => {
                            log::debug!("askpass accept: {e}");
                            std::thread::sleep(Duration::from_millis(15));
                        }
                    }
                }
            });
            Ok(AskpassServer { path, token, stop, thread: Some(thread) })
        }

        pub fn apply(&self, cmd: &mut Command) -> Result<(), FabrError> {
            cmd.env("SSH_ASKPASS", helper()?).env("SSH_ASKPASS_REQUIRE", "force").env(ENV_SOCK, &self.path).env(ENV_TOKEN, &self.token);
            if std::env::var_os("DISPLAY").is_none() {
                cmd.env("DISPLAY", ":0");
            }
            Ok(())
        }

        pub fn socket_path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for AskpassServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn serve_one(conn: UnixStream, token: &str, password: &Secret, answered: &mut bool) {
        let _ = conn.set_nonblocking(false);
        let _ = conn.set_read_timeout(Some(Duration::from_secs(5)));
        let mut r = BufReader::new((&conn).take(8192));
        let (mut tok, mut prompt) = (String::new(), String::new());
        if r.read_line(&mut tok).is_err() || r.read_line(&mut prompt).is_err() {
            return;
        }
        let mut w = &conn;
        if !ct_eq(tok.trim_end().as_bytes(), token.as_bytes()) {
            log::warn!("askpass: request with a wrong token refused");
            let _ = w.write_all(b"err\n");
            return;
        }
        let prompt = prompt.trim_end();
        let a = match answer_for(prompt) {
            Answer::Password if *answered => Answer::Refuse,
            Answer::Password => {
                *answered = true;
                Answer::Password
            }
            other => other,
        };
        log::debug!("askpass prompt {prompt:?}: {a:?}");
        let reply: Vec<u8> = match a {
            Answer::Password => [b"ok\n".as_slice(), password.expose().as_bytes()].concat(),
            Answer::No => b"ok\nno".to_vec(),
            Answer::Refuse => b"err\n".to_vec(),
        };
        let _ = w.write_all(&reply);
    }

    pub fn ask(prompt: &str) -> Result<String, String> {
        let sock = std::env::var_os(ENV_SOCK).ok_or("FABR_ASKPASS_SOCK is not set")?;
        let token = std::env::var(ENV_TOKEN).map_err(|_| "FABR_ASKPASS_TOKEN is not set")?;
        let mut c = UnixStream::connect(&sock).map_err(|e| format!("askpass socket: {e}"))?;
        let _ = c.set_read_timeout(Some(Duration::from_secs(10)));
        let line = prompt.replace(['\n', '\r'], " ");
        c.write_all(format!("{token}\n{line}\n").as_bytes()).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        c.take(64 * 1024).read_to_end(&mut out).map_err(|e| e.to_string())?;
        let out = String::from_utf8(out).map_err(|_| "bad reply".to_string())?;
        match out.strip_prefix("ok\n") {
            Some(a) => Ok(a.to_string()),
            None => Err("no answer for this prompt".into()),
        }
    }
}

#[cfg(not(unix))]
mod imp {
    use super::*;
    pub struct AskpassServer;
    impl AskpassServer {
        pub fn start(_: &Path, _: Secret) -> Result<AskpassServer, FabrError> {
            Err(FabrError::BadRequest("password authentication needs a Unix system".into()))
        }
        pub fn apply(&self, _: &mut Command) -> Result<(), FabrError> {
            Ok(())
        }
    }
    pub fn ask(_: &str) -> Result<String, String> {
        Err("askpass needs a Unix system".into())
    }
}

pub use imp::{AskpassServer, ask};

pub fn run_helper_if_invoked() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let prompt = match args.as_slice() {
        [flag, p] if flag == "--askpass" => p.clone(),
        [p] if std::env::var_os(ENV_TOKEN).is_some() && std::env::var_os(ENV_SOCK).is_some() => p.clone(),
        _ => return None,
    };
    match ask(&prompt) {
        Ok(a) => {
            println!("{a}");
            Some(0)
        }
        Err(e) => {
            eprintln!("fabr askpass: {e}");
            Some(1)
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn prompts_and_tokens() {
        assert_eq!(answer_for("me@box's password: "), Answer::Password);
        assert_eq!(answer_for("Password:"), Answer::Password);
        assert_eq!(answer_for("Are you sure you want to continue connecting (yes/no/[fingerprint])?"), Answer::No);
        assert_eq!(answer_for("Enter passphrase for key '/x':"), Answer::Refuse);
        assert!(ct_eq(b"ab", b"ab") && !ct_eq(b"ab", b"ac") && !ct_eq(b"ab", b"abc"));
    }

    #[test]
    fn server_answers_over_the_socket() {
        use std::io::{Read, Write};
        let tmp = tempfile::tempdir().unwrap();
        let s = AskpassServer::start(tmp.path(), Secret::new("pa ss")).unwrap();
        let mode = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(s.socket_path()).unwrap().permissions());
        assert_eq!(mode & 0o777, 0o600);
        let q = |tok: &str, prompt: &str| {
            let mut c = std::os::unix::net::UnixStream::connect(s.socket_path()).unwrap();
            c.write_all(format!("{tok}\n{prompt}\n").as_bytes()).unwrap();
            let mut out = String::new();
            c.read_to_string(&mut out).unwrap();
            out
        };
        let tok = s.token.clone();
        assert_eq!(q("wrong", "password:"), "err\n");
        assert_eq!(q(&tok, "me@h's password:"), "ok\npa ss");
        assert_eq!(q(&tok, "me@h's password:"), "err\n", "the password is handed out once");
        assert_eq!(q(&tok, "continue connecting (yes/no)?"), "ok\nno");
        assert_eq!(q(&tok, "Verification code:"), "err\n");
        assert_eq!(q("wrong", "password:"), "err\n");
        let p = s.socket_path().to_path_buf();
        drop(s);
        assert!(!p.exists());
    }
}
