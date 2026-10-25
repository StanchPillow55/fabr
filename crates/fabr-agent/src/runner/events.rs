use fabr_core::redact::redact;
use std::io::Write;
use std::path::PathBuf;

pub struct Events {
    path: PathBuf,
}

impl Events {
    pub fn new(space: &std::path::Path) -> Events {
        Events { path: space.join("events.log") }
    }

    pub fn append(&self, kind: &str, task_path: &str, detail: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let detail = detail.replace('\n', " ");
        let line = redact(&format!("{} {kind:<7} {task_path} {detail}\n", crate::now_utc()));
        std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?.write_all(line.as_bytes())
    }
}
