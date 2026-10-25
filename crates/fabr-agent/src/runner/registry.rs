use super::heartbeat::FILE;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

pub struct Entry {
    pub path: PathBuf,
    pub task_path: String,
    pub pid: u64,
    heartbeat_at: Option<DateTime<Utc>>,
    interval_s: i64,
}

impl Entry {
    pub fn age_s(&self, now: DateTime<Utc>) -> Option<i64> {
        self.heartbeat_at.map(|t| (now - t).num_seconds())
    }

    pub fn dead(&self, now: DateTime<Utc>) -> bool {
        self.age_s(now).is_none_or(|age| age > 3 * self.interval_s)
    }
}

pub fn list(space: &Path) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(space.join("registry")) else { return Vec::new() };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        let name = ent.file_name().to_string_lossy().into_owned();
        if name == FILE || name.starts_with('.') || path.extension().is_none_or(|e| e != "yaml") {
            continue;
        }
        let Some(doc) = std::fs::read_to_string(&path).ok().and_then(|t| serde_yaml::from_str::<serde_yaml::Value>(&t).ok()) else {
            out.push(Entry { path, task_path: String::new(), pid: 0, heartbeat_at: None, interval_s: 10 });
            continue;
        };
        let text = |k: &str| doc.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        out.push(Entry {
            task_path: text("task_path"),
            pid: doc.get("pid").and_then(|v| v.as_u64()).unwrap_or(0),
            heartbeat_at: DateTime::parse_from_rfc3339(&text("heartbeat_at")).ok().map(|t| t.with_timezone(&Utc)),
            interval_s: doc.get("heartbeat_interval_s").and_then(|v| v.as_i64()).filter(|n| *n > 0).unwrap_or(10),
            path,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}
