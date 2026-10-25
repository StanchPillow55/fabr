//! In-process worker host for the desktop app.
//!
//! [`AgentHandle`] owns only the lifecycle of the background worker thread: a
//! dedicated, desktop-owned stop flag and the thread's [`JoinHandle`]. The
//! actual polling loop, heartbeat, gating, and registry locking all live in
//! `fabr_agent::runner::Runner`, which this host drives as a library.
//!
//! The stop flag is a module-owned `static AtomicBool`. The GUI must NOT install
//! process-global SIGINT/SIGTERM handlers (`fabr_agent::runner::install_signals`
//! is deliberately not called): a desktop app observes window-close / exit
//! events instead, and a shared process-global handler would fight the Tauri
//! runtime for those signals.

use fabr_agent::runner::{Options, Runner, Sink, start_gate};
use fabr_core::setup::Ctx;
use fabr_core::workspace::Workspace;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

/// Poll interval for the hosted worker, in seconds.
const INTERVAL_S: u64 = 10;

/// Desktop-owned stop flag shared with the worker thread.
///
/// This is a dedicated flag for the GUI host — explicitly NOT the process-global
/// signal flag from `fabr_agent::runner::install_signals`. A `&'static` borrow of
/// it can be handed to the worker thread safely. [`AgentHandle::spawn`] resets it
/// to `false` so a re-spawn after a previous shutdown starts cleanly.
static STOP: AtomicBool = AtomicBool::new(false);

/// Owns the worker thread's lifecycle: the stop flag and the join handle.
///
/// Dropping or calling [`AgentHandle::shutdown`] flips the stop flag and joins
/// the worker. The worker ends within one poll interval because `Runner::run`
/// checks the flag during its sleep slices.
pub struct AgentHandle {
    stop: &'static AtomicBool,
    join: Option<JoinHandle<()>>,
}

impl AgentHandle {
    /// Spawn the background worker over the given workspace.
    ///
    /// Startup is non-blocking and can never fail the app: the worker thread
    /// builds its own `Ctx::from_env()` and keeps retrying `Runner::new` once
    /// per interval while setup is incomplete (the gate inside `Runner::new`
    /// returns an error until setup is finished). Once the runner is built, the
    /// thread calls `Runner::run`, which owns the poll loop and exits within one
    /// interval after the stop flag flips. Any error from `Runner::new` or
    /// `Runner::run` is logged and never propagated, and the whole worker body
    /// is wrapped in `catch_unwind` so a panic cannot take down the Tauri
    /// process.
    pub fn spawn(ws: Workspace) -> AgentHandle {
        STOP.store(false, Ordering::SeqCst);
        let stop: &'static AtomicBool = &STOP;
        let join = std::thread::Builder::new()
            .name("fabr-agent".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker(ws, stop)));
                if result.is_err() {
                    log::error!("agent worker panicked; the worker is down but the app keeps running");
                }
            })
            .map(Some)
            .unwrap_or_else(|e| {
                // Failing to spawn the OS thread must not fail the app either:
                // log and carry on with no worker.
                log::error!("could not start the agent worker thread: {e}");
                None
            });
        AgentHandle { stop, join }
    }

    /// Flip the stop flag and join the worker thread.
    ///
    /// The worker observes the flag within one poll interval (`Runner::run`
    /// checks it during its sleep slices) and then runs its own clean shutdown,
    /// which removes only this process's registry entries. Joining blocks until
    /// the worker has fully returned.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take()
            && let Err(e) = join.join()
        {
            log::error!("agent worker thread did not join cleanly: {e:?}");
        }
    }
}

/// The worker thread body: build the runner (retrying while the gate is not
/// ready) and then run the poll loop until stop is requested.
///
/// `Runner::new` runs the setup gate (`start_gate`) and returns an error while
/// setup is incomplete. The gate is checked up front with `start_gate` — which
/// only borrows the `Ctx` and so can be retried without consuming the owned
/// `Workspace` — once per interval, honoring the stop flag between attempts, so
/// startup is non-blocking and gate failures are retried inside the loop and
/// never fail the app (Requirements 1.1, 1.4). Only once the gate passes is the
/// owned `ws` moved into `Runner::new`. Both a construction error and any run
/// error are logged without propagating.
fn worker(ws: Workspace, stop: &'static AtomicBool) {
    let ctx = Ctx::from_env();
    let opts = Options { interval_s: INTERVAL_S, once: false };
    loop {
        if stop.load(Ordering::SeqCst) {
            log::info!("agent worker stopped before setup completed");
            return;
        }
        if let Err(e) = start_gate(&ctx) {
            log::info!("agent not starting yet ({}); will retry in {INTERVAL_S}s", e.message());
            sleep_or_stop(stop, INTERVAL_S);
            continue;
        }
        break;
    }
    // The gate passed, so `Runner::new` should now succeed; move the owned `ws`
    // into it. If it still errors (a race with setup changing underfoot), log
    // and give up rather than fail the app — the next app launch retries.
    let mut runner = match Runner::new(&ctx, ws, opts, Sink::default()) {
        Ok(runner) => runner,
        Err(e) => {
            log::error!("agent worker could not build the runner: {}", e.message());
            return;
        }
    };
    if let Err(e) = runner.run(stop) {
        log::error!("agent worker exited with an error: {}", e.message());
    } else {
        log::info!("agent worker exited cleanly");
    }
}

/// Sleep for up to `secs`, waking early if stop is requested.
///
/// Slices the wait into short naps so a stop request is observed promptly
/// instead of after a full interval.
fn sleep_or_stop(stop: &AtomicBool, secs: u64) {
    let total = Duration::from_secs(secs);
    let mut waited = Duration::ZERO;
    while waited < total && !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
        waited += Duration::from_millis(100);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Build a `Workspace` over a temp dir. On the gate-failure path the worker
    /// never reaches `Runner::new`, so this workspace is constructed but never
    /// actually used — any valid one suffices.
    fn workspace(root: &std::path::Path) -> Workspace {
        let cfg = root.join("cfg");
        let cache = root.join("cache");
        let proj = root.join("proj");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        Workspace::from_config(&cfg, &cache, Some(proj)).unwrap()
    }

    /// Exercises the whole `AgentHandle` lifecycle against a never-passing
    /// setup gate (Requirements 1.4, 1.5, 10.6).
    ///
    /// Both behaviors under test mutate the process-global `FABR_HOME` that
    /// `Ctx::from_env()` reads, so they live in one test to guarantee the env
    /// mutation is never concurrent with another test in this crate's harness
    /// (the default parallel runner would otherwise race on it). The var is set
    /// once at the top and restored before the function returns.
    ///
    /// Pointing `FABR_HOME` at a fresh temp dir with no completed setup makes
    /// `start_gate` fail forever, so the worker loops in its sliced retry sleep
    /// and never reaches `Runner::new` (so the passed `Workspace` is never
    /// actually consumed). This is exactly the "incomplete setup" path: the
    /// worker logs and retries without panicking (1.5), and a shutdown shortly
    /// after spawn must break the sliced sleep and join well under the 10s
    /// `INTERVAL_S`, flipping the stop flag (1.4, 10.6).
    #[test]
    fn spawn_then_shutdown_flips_stop_and_joins_promptly_without_panicking() {
        let home = tempfile::tempdir().unwrap();
        let ws_root = tempfile::tempdir().unwrap();
        let prev = std::env::var_os("FABR_HOME");
        // SAFETY: `set_var`/`remove_var` are `unsafe` in newer Rust editions.
        // This mutation is confined to this single test and restored before the
        // function returns, so no other test observes it.
        unsafe {
            std::env::set_var("FABR_HOME", home.path());
        }

        // Run the lifecycle inside a closure so a panicking assertion cannot
        // skip the env restore below.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let ws = workspace(ws_root.path());
            let started = Instant::now();
            let mut handle = AgentHandle::spawn(ws);

            // Let the worker reach its gate-failure retry sleep so shutdown must
            // actively interrupt the sliced sleep rather than catch a thread
            // that has not started looping yet. The gate failing and being
            // logged here (no panic escapes the worker's `catch_unwind`) is the
            // 1.5 path.
            std::thread::sleep(Duration::from_millis(250));

            // 10.6 / 1.4: shutdown flips the stop flag and joins the worker.
            handle.shutdown();
            let elapsed = started.elapsed();

            assert!(STOP.load(Ordering::SeqCst), "shutdown must flip the stop flag");
            assert!(handle.join.is_none(), "shutdown must take and join the worker handle");

            // 1.4: the sliced-sleep stop check means we join far faster than one
            // full interval (10s). A lenient 3s bound keeps this non-flaky while
            // still proving we did not wait a whole interval.
            assert!(elapsed < Duration::from_secs(3), "spawn→shutdown took too long: {elapsed:?}");

            // A second shutdown is a harmless no-op (handle already joined).
            handle.shutdown();
        }));

        // Restore the global env regardless of how the body finished.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("FABR_HOME", v),
                None => std::env::remove_var("FABR_HOME"),
            }
        }

        if let Err(e) = outcome {
            std::panic::resume_unwind(e);
        }
    }
}
