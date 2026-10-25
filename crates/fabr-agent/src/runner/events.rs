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

    fn lines(&self) -> Vec<String> {
        std::fs::read_to_string(&self.path).unwrap_or_default().lines().map(str::to_string).collect()
    }

    pub fn newest_at(&self) -> Option<String> {
        self.lines().iter().filter_map(|l| l.split_whitespace().next().map(str::to_string)).max()
    }

    pub fn seen(&self, kind: &str, task_path: &str, at: &str) -> bool {
        self.lines().iter().any(|line| {
            let mut head = line.splitn(3, char::is_whitespace);
            let (Some(ts), Some(k), Some(rest)) = (head.next(), head.next(), head.next()) else { return false };
            let rest = rest.trim_start();
            k == kind
                && ts >= at
                && rest.strip_prefix(task_path).is_some_and(|after| after.is_empty() || after.starts_with(char::is_whitespace))
        })
    }
}
