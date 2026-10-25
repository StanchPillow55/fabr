use crate::now_utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

pub const FILE: &str = "_backend.yaml";

pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

pub struct Heartbeat {
    pub path: PathBuf,
    pub interval_s: u64,
    started_at: String,
}

impl Heartbeat {
    pub fn new(space: &Path, interval_s: u64) -> Heartbeat {
        Heartbeat { path: space.join("registry").join(FILE), interval_s, started_at: now_utc() }
    }

    pub fn write(&self) -> std::io::Result<()> {
        let text = format!(
            "heartbeat_at: \"{}\"\nheartbeat_interval_s: {}\npid: {}\nstarted_at: \"{}\"\n",
            now_utc(),
            self.interval_s,
            std::process::id(),
            self.started_at
        );
        write_atomic(&self.path, &text)
    }

    pub fn remove(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub struct Beating {
    stop: Sender<()>,
    handle: JoinHandle<()>,
    pub heartbeat: Arc<Heartbeat>,
}

impl Beating {
    pub fn spawn(heartbeat: Heartbeat) -> std::io::Result<Beating> {
        heartbeat.write()?;
        let heartbeat = Arc::new(heartbeat);
        let (stop, rx) = channel::<()>();
        let hb = heartbeat.clone();
        let handle = std::thread::spawn(move || {
            while rx.recv_timeout(Duration::from_secs(hb.interval_s)).is_err_and(|e| e == std::sync::mpsc::RecvTimeoutError::Timeout) {
                if let Err(e) = hb.write() {
                    log::warn!("heartbeat write failed: {e}");
                }
            }
        });
        Ok(Beating { stop, handle, heartbeat })
    }

    pub fn stop(self, remove: bool) {
        let _ = self.stop.send(());
        let _ = self.handle.join();
        if remove {
            self.heartbeat.remove();
        }
    }
}
