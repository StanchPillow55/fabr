use crate::error::FabrError;
use crate::paths::APP;
use crate::sync::lock;
use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const MAX_LOG_BYTES: u64 = 5 << 20;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn default_log_path() -> PathBuf {
    let home = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf());
    let file = Path::new(APP).join(format!("{APP}.log"));
    if cfg!(target_os = "macos")
        && let Some(h) = &home
    {
        return h.join("Library/Logs").join(&file);
    }
    if cfg!(target_os = "linux") {
        if let Some(x) = std::env::var_os("XDG_STATE_HOME").filter(|x| Path::new(x).is_absolute()) {
            return PathBuf::from(x).join(&file);
        }
        if let Some(h) = &home {
            return h.join(".local/state").join(&file);
        }
    }
    std::env::temp_dir().join(&file)
}

pub fn level_from_env(var: &str, default: LevelFilter) -> LevelFilter {
    std::env::var(var).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(default)
}

pub fn log_path() -> Option<PathBuf> {
    LOG_PATH.get().cloned()
}

pub struct RollingFile {
    path: PathBuf,
    max: u64,
    file: Option<File>,
    size: u64,
}

fn backup_of(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".1");
    PathBuf::from(s)
}

impl RollingFile {
    pub fn open(path: &Path, max: u64) -> std::io::Result<Self> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(RollingFile { path: path.to_path_buf(), max, file: Some(file), size })
    }

    fn roll(&mut self) {
        self.file = None;
        let _ = std::fs::rename(&self.path, backup_of(&self.path));
        self.file = OpenOptions::new().create(true).append(true).open(&self.path).ok();
        self.size = 0;
    }

    pub fn write_line(&mut self, line: &str) {
        if self.size > 0 && self.size + line.len() as u64 > self.max {
            self.roll();
        }
        if let Some(f) = &mut self.file
            && f.write_all(line.as_bytes()).is_ok()
        {
            self.size += line.len() as u64;
        }
    }
}

struct Logger {
    level: LevelFilter,
    file: Mutex<RollingFile>,
    stderr: bool,
}

impl Logger {
    fn allowed(&self, target: &str, level: log::Level) -> bool {
        let max = if target.starts_with("fabr") { self.level } else { self.level.min(LevelFilter::Warn) };
        level <= max
    }
}

impl Log for Logger {
    fn enabled(&self, m: &Metadata) -> bool {
        self.allowed(m.target(), m.level())
    }
    fn log(&self, r: &Record) {
        if !self.allowed(r.target(), r.level()) {
            return;
        }
        let ts = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z");
        let line = format!("{ts} {:<5} {}: {}\n", r.level(), r.target(), r.args());
        if self.stderr {
            eprint!("{line}");
        }
        lock(&self.file).write_line(&line);
    }
    fn flush(&self) {
        if let Some(f) = &mut lock(&self.file).file {
            let _ = f.flush();
        }
    }
}

pub fn init(path: &Path, level: LevelFilter) -> Result<PathBuf, FabrError> {
    init_with(path, level, false)
}

pub fn init_with(path: &Path, level: LevelFilter, stderr: bool) -> Result<PathBuf, FabrError> {
    if let Some(p) = LOG_PATH.get() {
        return Ok(p.clone());
    }
    let file = RollingFile::open(path, MAX_LOG_BYTES).map_err(|e| FabrError::Io(format!("log file {}: {e}", path.display())))?;
    let logger = Logger { level, file: Mutex::new(file), stderr };
    log::set_boxed_logger(Box::new(logger)).map_err(|e| FabrError::Io(format!("logger: {e}")))?;
    log::set_max_level(level.max(LevelFilter::Warn));
    let _ = LOG_PATH.set(path.to_path_buf());
    Ok(path.to_path_buf())
}

pub fn init_default(stderr: bool) -> Result<PathBuf, FabrError> {
    let level = level_from_env("FABR_LOG", LevelFilter::Info);
    match init_with(&default_log_path(), level, stderr) {
        Ok(p) => Ok(p),
        Err(_) => init_with(&std::env::temp_dir().join(APP).join(format!("{APP}.log")), level, stderr),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_and_keeps_one_backup() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("logs/x.log");
        let mut f = RollingFile::open(&p, 100).unwrap();
        for i in 0..30 {
            f.write_line(&format!("line {i:03}\n"));
        }
        let cur = std::fs::read_to_string(&p).unwrap();
        let bak = std::fs::read_to_string(backup_of(&p)).unwrap();
        assert!(cur.len() <= 100 && bak.len() <= 100);
        assert!(cur.ends_with("line 029\n"));
        assert!(!d.path().join("logs/x.log.2").exists());
        let mut f = RollingFile::open(&p, 1000).unwrap();
        f.write_line("again\n");
        assert!(std::fs::read_to_string(&p).unwrap().ends_with("line 029\nagain\n"));
    }

    #[test]
    fn levels_and_paths() {
        assert_eq!(level_from_env("FABR_TEST_UNSET_VAR", LevelFilter::Info), LevelFilter::Info);
        assert!(default_log_path().ends_with("fabr/fabr.log"));
        let l = Logger {
            level: LevelFilter::Debug,
            file: Mutex::new(RollingFile::open(&std::env::temp_dir().join("fabr-test-unused.log"), 10).unwrap()),
            stderr: false,
        };
        assert!(l.allowed("fabr_core::scan", log::Level::Debug));
        assert!(!l.allowed("tao", log::Level::Info));
        assert!(l.allowed("tao", log::Level::Warn));
        let _ = std::fs::remove_file(std::env::temp_dir().join("fabr-test-unused.log"));
    }
}
