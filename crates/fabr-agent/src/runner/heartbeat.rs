use crate::board::Board;
use crate::now_utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

pub const FILE: &str = "_backend.yaml";

static SEQ: AtomicU64 = AtomicU64::new(0);

pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.{}.tmp", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

pub fn write_failure(space: &Path, message: &str) -> std::io::Result<()> {
    let hb = Heartbeat::new(space, 10);
    let quoted = message.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ");
    let text = format!(
        "heartbeat_at: \"{}\"\nheartbeat_interval_s: {}\npid: {}\nstarted_at: \"{}\"\nlast_error: \"{quoted}\"\n",
        now_utc(),
        hb.interval_s,
        std::process::id(),
        hb.started_at
    );
    write_atomic(&hb.path, &text)
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
    pub fn spawn(heartbeat: Heartbeat, board: Arc<Board>) -> std::io::Result<Beating> {
        heartbeat.write()?;
        let heartbeat = Arc::new(heartbeat);
        let (stop, rx) = channel::<()>();
        let hb = heartbeat.clone();
        let handle = std::thread::spawn(move || {
            while rx.recv_timeout(Duration::from_secs(hb.interval_s)).is_err_and(|e| e == std::sync::mpsc::RecvTimeoutError::Timeout) {
                if let Err(e) = hb.write() {
                    log::warn!("heartbeat write failed: {e}");
                }
                board.beat();
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
