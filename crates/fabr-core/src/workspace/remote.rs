use super::Entry;
use crate::backend::Backend;
use crate::fs::Fs;
use crate::sync::lock;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Cached {
    fs: Option<Arc<dyn Fs>>,
    stamp: String,
    error: Option<String>,
    at: Instant,
    in_flight: bool,
    stale: bool,
    epoch: u64,
    login_attempts: u8,
}

pub(super) struct RemoteView {
    pub(super) fs: Option<Arc<dyn Fs>>,
    pub(super) stamp: String,
    pub(super) error: Option<String>,
    pub(super) pending: bool,
}

#[derive(Default)]
pub(super) struct RemoteCache(Arc<Mutex<HashMap<String, Cached>>>);

const MAX_LOGIN_WAITS: u8 = 3;

fn is_login_required(msg: &str) -> bool {
    crate::remote::ssh::SshFailureKind::from_message(msg) == Some(crate::remote::ssh::SshFailureKind::LoginRequired)
}

fn fetch_remote(id: &str, backend: &dyn Backend, alive: &AtomicBool, cache: &Mutex<HashMap<String, Cached>>, epoch: u64, mut wait: bool) {
    loop {
        let t = Instant::now();
        let fetched = if wait { backend.snapshot_waiting_for_login() } else { backend.snapshot_with_stamp() };
        match &fetched {
            Ok(_) => log::info!("snapshot {id}: {} ms", t.elapsed().as_millis()),
            Err(err) => log::warn!("snapshot {id}: failed after {} ms: {}", t.elapsed().as_millis(), err.message()),
        }
        let mut cache = lock(cache);
        if !alive.load(Ordering::SeqCst) {
            return;
        }
        let Some(c) = cache.get_mut(id) else { return };
        let mut again = false;
        match fetched {
            Ok((fs, stamp)) => {
                c.fs = Some(fs);
                c.stamp = stamp;
                c.error = None;
                c.login_attempts = 0;
            }
            Err(err) => {
                let msg = err.message();
                if is_login_required(&msg) {
                    if wait {
                        c.login_attempts += 1;
                    }
                    again = c.login_attempts < MAX_LOGIN_WAITS && c.epoch == epoch;
                }
                c.error = Some(msg);
            }
        }
        c.at = Instant::now();
        if again {
            wait = true;
            continue;
        }
        c.in_flight = false;
        if c.epoch != epoch {
            c.stale = true;
        }
        return;
    }
}

impl RemoteCache {
    pub(super) fn view(&self, e: &Entry, max_age: Duration, backoff: Duration) -> RemoteView {
        let mut cache = lock(&self.0);
        let c = cache.entry(e.source.id.clone()).or_insert_with(|| Cached {
            fs: None,
            stamp: String::new(),
            error: None,
            at: Instant::now(),
            in_flight: false,
            stale: true,
            epoch: 0,
            login_attempts: 0,
        });
        let due = c.stale || c.at.elapsed() >= if c.error.is_some() { backoff } else { max_age };
        if due && !c.in_flight {
            c.in_flight = true;
            c.stale = false;
            let epoch = c.epoch;
            let wait = c.error.as_deref().is_some_and(is_login_required) && c.login_attempts < MAX_LOGIN_WAITS;
            let (id, backend, alive, shared) = (e.source.id.clone(), e.backend.clone(), e.alive.clone(), self.0.clone());
            let spawned = std::thread::Builder::new().name(format!("fabr-fetch-{id}")).spawn({
                let id = id.clone();
                move || fetch_remote(&id, backend.as_ref(), &alive, &shared, epoch, wait)
            });
            if let Err(err) = spawned {
                log::warn!("snapshot {id}: cannot start thread: {err}");
                c.in_flight = false;
                c.error = Some(format!("cannot start fetch: {err}"));
                c.at = Instant::now();
            }
        }
        RemoteView { fs: c.fs.clone(), stamp: c.stamp.clone(), error: c.error.clone(), pending: c.in_flight }
    }

    pub(super) fn wait(&self, timeout: Duration) -> bool {
        let t = Instant::now();
        loop {
            if !lock(&self.0).values().any(|c| c.in_flight) {
                return true;
            }
            if t.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub(super) fn invalidate(&self, id: &str) {
        if let Some(c) = lock(&self.0).get_mut(id) {
            c.stale = true;
            c.epoch += 1;
            c.login_attempts = 0;
        }
    }

    pub(super) fn retain(&self, kept: &HashSet<String>) {
        lock(&self.0).retain(|id, _| kept.contains(id));
    }

    pub(super) fn snapshot(&self, id: &str) -> Option<Arc<dyn Fs>> {
        lock(&self.0).get(id).and_then(|c| c.fs.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::DirEntry;
    use crate::dto::{GetTreeReq, Tree};
    use crate::error::FabrError;
    use crate::sources::Source;
    use crate::workspace::Workspace;
    use std::sync::atomic::AtomicUsize;

    fn setup() -> (tempfile::TempDir, Workspace) {
        crate::secrets::use_memory_store();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("remote sample's");
        crate::sample::write_sample(&root).unwrap();
        let ws = Workspace::from_config(&tmp.path().join("cfg"), &tmp.path().join("cache"), None).unwrap();
        let source = |id: &str, name: &str, host: Option<&str>| Source {
            id: id.into(),
            name: name.into(),
            host: host.map(Into::into),
            root: root.display().to_string(),
            prune: None,
            search_depth: None,
            password: None,
        };
        ws.save_sources(vec![source("r", "Remote", Some("box")), source("l", "Local", None)]).unwrap();
        (tmp, ws)
    }

    fn with_backend(backend: impl Backend + 'static) -> (tempfile::TempDir, Workspace) {
        let (t, ws) = setup();
        let mut es = ws.entries.get();
        es.iter_mut().find(|e| e.source.id == "r").unwrap().backend = Arc::new(backend);
        ws.entries.set(es);
        lock(&ws.remote.0).clear();
        (t, ws)
    }

    struct SlowBackend {
        calls: Arc<AtomicUsize>,
        delay: Duration,
        fail: Arc<AtomicBool>,
    }
    impl Backend for SlowBackend {
        fn snapshot(&self) -> Result<Arc<dyn Fs>, FabrError> {
            self.snapshot_with_stamp().map(|(f, _)| f)
        }
        fn stamp(&self) -> Result<String, FabrError> {
            Ok("snap".into())
        }
        fn snapshot_with_stamp(&self) -> Result<(Arc<dyn Fs>, String), FabrError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(self.delay);
            if self.fail.load(Ordering::SeqCst) {
                return Err(FabrError::Remote("Could not connect: nope".into()));
            }
            Ok((Arc::new(crate::fs::TarSnapshot::default()), "snap".into()))
        }
        fn write_node(&self, _: &str, _: &str, _: crate::backend::NodeFiles) -> Result<(), FabrError> {
            unreachable!()
        }
        fn init_node(&self, _: &str, _: Option<u32>) -> Result<(), FabrError> {
            unreachable!()
        }
        fn remove_node(&self, _: &str, _: &str) -> Result<(), FabrError> {
            unreachable!()
        }
        fn list_dirs(&self, _: &str, _: &HashSet<String>) -> Result<Vec<DirEntry>, FabrError> {
            unreachable!()
        }
    }

    fn slow_setup(delay_ms: u64, fail: bool) -> (tempfile::TempDir, Workspace, Arc<AtomicUsize>, Arc<AtomicBool>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let failing = Arc::new(AtomicBool::new(fail));
        let (t, ws) = with_backend(SlowBackend { calls: calls.clone(), delay: Duration::from_millis(delay_ms), fail: failing.clone() });
        (t, ws, calls, failing)
    }

    fn tree_req(ws: &Workspace, rescan: bool) -> Tree {
        ws.get_tree_req(GetTreeReq { show_all: false, rescan })
    }

    #[test]
    fn slow_first_fetch_does_not_block_the_request() {
        let (_t, ws, calls, _) = slow_setup(2000, false);
        let t0 = Instant::now();
        let t = tree_req(&ws, false);
        let s1 = ws.get_stamp().stamp;
        assert!(t0.elapsed() < Duration::from_millis(200), "took {:?}", t0.elapsed());
        assert_eq!(t.pending, vec!["r".to_string()]);
        assert!(t.errors.is_empty());
        let r = t.root.children.iter().find(|c| c.source == "r").unwrap();
        assert!(!r.managed && r.children.is_empty());
        assert!(ws.wait_remote(Duration::from_secs(10)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_ne!(s1, ws.get_stamp().stamp, "the landing fetch changes the stamp");
        let t = tree_req(&ws, false);
        assert!(t.pending.is_empty() && t.errors.is_empty());
    }

    #[test]
    fn concurrent_requests_share_one_fetch() {
        let (_t, ws, calls, _) = slow_setup(500, false);
        let ws = Arc::new(ws);
        let hs: Vec<_> = (0..5)
            .map(|_| {
                let ws = ws.clone();
                std::thread::spawn(move || tree_req(&ws, false).pending.len())
            })
            .collect();
        for h in hs {
            assert_eq!(h.join().unwrap(), 1);
        }
        let _ = ws.get_stamp();
        assert!(ws.wait_remote(Duration::from_secs(10)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn failed_remote_backs_off_until_rescan() {
        let (_t, ws, calls, _) = slow_setup(0, true);
        tree_req(&ws, false);
        assert!(ws.wait_remote(Duration::from_secs(10)));
        let t0 = Instant::now();
        let t = tree_req(&ws, false);
        let _ = ws.get_stamp();
        assert!(t0.elapsed() < Duration::from_millis(200));
        assert!(t.pending.is_empty());
        assert!(t.errors.iter().any(|e| e.source == "r" && e.message.contains("Could not connect")), "served at once");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no refetch before the back-off");
        tree_req(&ws, true);
        assert!(ws.wait_remote(Duration::from_secs(10)));
        assert_eq!(calls.load(Ordering::SeqCst), 2, "rescan starts exactly one fetch");
    }

    struct LoginBackend {
        plain: Arc<AtomicUsize>,
        waits: Arc<AtomicUsize>,
        succeed_on: usize,
    }
    const LOGIN_MSG: &str = "Log in to Tailscale to reach box: https://login.tailscale.com/a/abc";
    impl Backend for LoginBackend {
        fn snapshot(&self) -> Result<Arc<dyn Fs>, FabrError> {
            self.snapshot_with_stamp().map(|(f, _)| f)
        }
        fn stamp(&self) -> Result<String, FabrError> {
            Ok("snap".into())
        }
        fn snapshot_with_stamp(&self) -> Result<(Arc<dyn Fs>, String), FabrError> {
            self.plain.fetch_add(1, Ordering::SeqCst);
            Err(FabrError::Remote(LOGIN_MSG.into()))
        }
        fn snapshot_waiting_for_login(&self) -> Result<(Arc<dyn Fs>, String), FabrError> {
            let n = self.waits.fetch_add(1, Ordering::SeqCst) + 1;
            std::thread::sleep(Duration::from_millis(300));
            if n == self.succeed_on {
                return Ok((Arc::new(crate::fs::TarSnapshot::default()), "snap".into()));
            }
            Err(FabrError::Remote(LOGIN_MSG.into()))
        }
        fn write_node(&self, _: &str, _: &str, _: crate::backend::NodeFiles) -> Result<(), FabrError> {
            unreachable!()
        }
        fn init_node(&self, _: &str, _: Option<u32>) -> Result<(), FabrError> {
            unreachable!()
        }
        fn remove_node(&self, _: &str, _: &str) -> Result<(), FabrError> {
            unreachable!()
        }
        fn list_dirs(&self, _: &str, _: &HashSet<String>) -> Result<Vec<DirEntry>, FabrError> {
            unreachable!()
        }
    }

    fn login_setup(succeed_on: usize) -> (tempfile::TempDir, Workspace, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let (plain, waits) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (t, ws) = with_backend(LoginBackend { plain: plain.clone(), waits: waits.clone(), succeed_on });
        (t, ws, plain, waits)
    }

    #[test]
    fn login_required_waits_at_once_keeps_the_error_and_recovers() {
        let (_t, ws, plain, waits) = login_setup(2);
        let t0 = Instant::now();
        tree_req(&ws, false);
        std::thread::sleep(Duration::from_millis(150));
        let t = tree_req(&ws, false);
        let e = ws.entry("r").unwrap();
        assert!(ws.remote_view(&e).pending, "the long attempt is in flight");
        assert!(t.errors.iter().any(|e| e.source == "r" && e.message == LOGIN_MSG), "the link stays visible: {:?}", t.errors);
        assert!(ws.wait_remote(Duration::from_secs(10)));
        assert!(t0.elapsed() < Duration::from_secs(5), "no back-off wait between attempts");
        assert_eq!((plain.load(Ordering::SeqCst), waits.load(Ordering::SeqCst)), (1, 2));
        let t = tree_req(&ws, false);
        assert!(t.errors.is_empty() && t.pending.is_empty(), "success replaces the error: {:?}", t.errors);
        assert_eq!(plain.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn login_wait_is_bounded_to_three_attempts() {
        let (_t, ws, plain, waits) = login_setup(0);
        tree_req(&ws, false);
        assert!(ws.wait_remote(Duration::from_secs(10)));
        assert_eq!((plain.load(Ordering::SeqCst), waits.load(Ordering::SeqCst)), (1, 3));
        let t = tree_req(&ws, false);
        assert!(t.pending.is_empty() && t.errors.len() == 1);
        let _ = ws.get_stamp();
        assert!(ws.wait_remote(Duration::from_secs(1)));
        assert_eq!((plain.load(Ordering::SeqCst), waits.load(Ordering::SeqCst)), (1, 3));
        tree_req(&ws, true);
        assert!(ws.wait_remote(Duration::from_secs(10)));
        assert_eq!(waits.load(Ordering::SeqCst), 6);
    }

    #[test]
    fn invalidate_keeps_serving_the_old_snapshot() {
        let (_t, ws, calls, _) = slow_setup(300, false);
        tree_req(&ws, false);
        assert!(ws.wait_remote(Duration::from_secs(10)));
        let e = ws.entry("r").unwrap();
        ws.invalidate(&e);
        let r = ws.remote_view(&e);
        assert!(r.pending && r.fs.is_some(), "old snapshot served while the refetch runs");
        let t = tree_req(&ws, false);
        assert!(t.pending.is_empty(), "a source with a snapshot is not listed as connecting");
        assert!(t.root.children.iter().any(|c| c.source == "r"));
        assert!(ws.wait_remote(Duration::from_secs(10)));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(!ws.remote_view(&e).pending);
    }

    #[test]
    fn removed_source_is_not_resurrected_by_a_running_fetch() {
        let (_t, ws, _calls, _) = slow_setup(300, false);
        tree_req(&ws, false);
        let started = Instant::now();
        ws.save_sources(vec![ws.list_sources().into_iter().find(|s| s.id == "l").unwrap()]).unwrap();
        assert!(started.elapsed() < Duration::from_millis(200), "save_sources does not wait");
        std::thread::sleep(Duration::from_millis(600));
        assert!(lock(&ws.remote.0).is_empty());
    }
}
