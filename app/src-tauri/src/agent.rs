use fabr_agent::runner::{Options, Runner, Sink, start_gate};
use fabr_core::FabrError;
use fabr_core::redact::redact;
use fabr_core::setup::Ctx;
use fabr_core::workspace::Workspace;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

const INTERVAL_S: u64 = 5;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    pub running: bool,
    pub reason: Option<String>,
}

struct Live {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

#[derive(Default)]
pub struct AgentHost {
    live: Mutex<Option<Live>>,
    reason: Arc<Mutex<Option<String>>>,
}

fn set(reason: &Mutex<Option<String>>, v: Option<String>) {
    *reason.lock().unwrap_or_else(PoisonError::into_inner) = v;
}

impl AgentHost {
    pub fn status(&self) -> Status {
        let live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        let running = live.as_ref().is_some_and(|l| !l.handle.is_finished());
        Status { running, reason: self.reason.lock().unwrap_or_else(PoisonError::into_inner).clone() }
    }

    pub fn start(&self, ctx: Ctx, workspace: fn(PathBuf) -> Result<Workspace, FabrError>) -> Status {
        let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        if live.as_ref().is_some_and(|l| !l.handle.is_finished()) {
            drop(live);
            return self.status();
        }
        if let Some(old) = live.take() {
            let _ = old.handle.join();
        }
        set(&self.reason, None);
        let gate = match start_gate(&ctx) {
            Ok(g) => g,
            Err(e) => {
                let msg = redact(&e.message());
                log::warn!("agent not started: {msg}");
                set(&self.reason, Some(msg));
                drop(live);
                return self.status();
            }
        };
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, reason) = (stop.clone(), self.reason.clone());
        let spawned = std::thread::Builder::new().name("fabr-agent".into()).spawn(move || {
            let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut runner = workspace(gate.default_folder)
                    .and_then(|ws| Runner::new(&ctx, ws, Options { interval_s: INTERVAL_S, once: false }, Sink::default()))?;
                runner.run(&flag)
            }));
            let failure = match run {
                Ok(Ok(())) => None,
                Ok(Err(e)) => Some(e.message()),
                Err(_) => Some("the agent crashed unexpectedly".to_string()),
            };
            if let Some(f) = failure {
                let msg = redact(&f);
                log::error!("agent stopped: {msg}");
                set(&reason, Some(msg));
            }
        });
        match spawned {
            Ok(handle) => *live = Some(Live { stop, handle }),
            Err(e) => set(&self.reason, Some(format!("could not start the agent thread: {e}"))),
        }
        drop(live);
        self.status()
    }

    pub fn stop(&self) {
        let live = self.live.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(l) = live {
            l.stop.store(true, Ordering::SeqCst);
            let _ = l.handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabr_core::AppDirs;

    fn ws(root: PathBuf) -> Result<Workspace, FabrError> {
        let base = root.with_file_name("cfgdir");
        AppDirs { config: base.join("c"), cache: base.join("k") }.workspace(Some(root))
    }

    #[test]
    fn refuses_before_setup_and_runs_after() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("work");
        std::fs::create_dir_all(&folder).unwrap();
        let ctx = Ctx::new(dir.path());
        let host = AgentHost::default();

        let s = host.start(ctx.clone(), ws);
        assert!(!s.running);
        assert!(s.reason.unwrap().contains("Setup has not been started"));

        std::fs::create_dir_all(ctx.space()).unwrap();
        std::fs::write(
            ctx.settings_path(),
            format!("default_folder = \"{}\"\nmodel = \"claude-sonnet-5-5\"\n\n[setup]\ncompleted = true\n", folder.display()),
        )
        .unwrap();
        std::fs::write(ctx.secrets_path(), "anthropic_api_key = \"sk-ant-fake-test-key\"\n").unwrap();

        let s = host.start(ctx.clone(), ws);
        assert_eq!(s, Status { running: true, reason: None });
        assert_eq!(host.start(ctx.clone(), ws), s);
        let beat = ctx.space().join("registry").join("_backend.yaml");
        for _ in 0..50 {
            if beat.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(beat.exists());
        host.stop();
        assert!(!host.status().running);
        assert!(!beat.exists());
    }
}
