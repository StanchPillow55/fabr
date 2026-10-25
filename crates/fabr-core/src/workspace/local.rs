use super::{Entry, log_scan};
use crate::error::FabrError;
use crate::node::Node;
use crate::scan::{ScanOpts, Watched, scan};
use crate::sync::lock;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct LocalScan {
    root: Node,
    truncated: bool,
    reason: Option<String>,
    by: Option<&'static str>,
    emitted: Vec<Watched>,
    at: Instant,
    stamp: String,
    dirty: bool,
}

type LocalKey = (String, bool);

pub(super) type LocalTree = (Node, bool, Option<String>, Option<&'static str>);

#[derive(Default)]
pub(super) struct LocalCache {
    scans: Mutex<HashMap<LocalKey, LocalScan>>,
    count: AtomicUsize,
}

fn log_slow_stamp(source: &str, t: Instant) {
    let ms = t.elapsed().as_millis();
    if ms > 200 {
        log::info!("stamp {source}: {ms} ms");
    }
}

fn timed_stamp(e: &Entry) -> Result<String, FabrError> {
    let t = Instant::now();
    let s = e.backend.stamp();
    log_slow_stamp(&e.source.id, t);
    s
}

fn sync_watched(e: &Entry, cache: &HashMap<LocalKey, LocalScan>) {
    let mut seen = HashSet::new();
    let mut out: Vec<Watched> = Vec::new();
    for all in [false, true] {
        if let Some(c) = cache.get(&(e.source.id.clone(), all)) {
            for w in &c.emitted {
                if seen.insert(w.path.clone()) {
                    out.push(w.clone());
                } else if w.listed
                    && let Some(o) = out.iter_mut().find(|o| o.path == w.path)
                {
                    o.listed = true;
                }
            }
        }
    }
    if out.is_empty() {
        out.push(Watched { path: String::new(), listed: true });
    }
    e.backend.set_watched(out);
}

impl LocalScan {
    fn view(&self) -> LocalTree {
        (self.root.clone(), self.truncated, self.reason.clone(), self.by)
    }
}

impl LocalCache {
    fn refresh(&self, e: &Entry, opts: &ScanOpts, cache: &mut HashMap<LocalKey, LocalScan>) -> Result<(), FabrError> {
        let key = (e.source.id.clone(), opts.show_all);
        let r = self.full_scan(e, opts).map(|fresh| {
            cache.insert(key.clone(), fresh);
        });
        if r.is_err() {
            cache.remove(&key);
        }
        sync_watched(e, cache);
        r
    }

    fn full_scan(&self, e: &Entry, opts: &ScanOpts) -> Result<LocalScan, FabrError> {
        self.count.fetch_add(1, Ordering::Relaxed);
        let fs = e.backend.snapshot()?;
        let r = scan(fs.as_ref(), &e.source, opts);
        log_scan(&e.source.id, if opts.show_all { " (all)" } else { "" }, &r);
        let t = Instant::now();
        let stamp = e.backend.stamp_for(&r.emitted)?;
        log_slow_stamp(&e.source.id, t);
        Ok(LocalScan {
            root: r.root,
            truncated: r.truncated,
            reason: r.truncated_reason,
            by: r.truncated_by,
            emitted: r.emitted,
            at: Instant::now(),
            stamp,
            dirty: false,
        })
    }

    pub(super) fn tree(&self, e: &Entry, opts: &ScanOpts, max_age: Duration, rescan: bool) -> Result<LocalTree, FabrError> {
        let key = (e.source.id.clone(), opts.show_all);
        let mut cache = lock(&self.scans);
        if !rescan
            && let Some(c) = cache.get(&key)
            && !c.dirty
            && c.at.elapsed() < max_age
        {
            e.backend.snapshot()?;
            if e.backend.stamp_for(&c.emitted)? == c.stamp {
                return Ok(c.view());
            }
        }
        self.refresh(e, opts, &mut cache)?;
        Ok(cache[&key].view())
    }

    pub(super) fn stamp<'p>(&self, e: &Entry, opts_for: &dyn Fn(bool) -> ScanOpts<'p>, max_age: Duration) -> Result<String, FabrError> {
        let mut cache = lock(&self.scans);
        let mut todo: Vec<bool> = [false, true]
            .into_iter()
            .filter(|all| cache.get(&(e.source.id.clone(), *all)).is_some_and(|c| c.at.elapsed() >= max_age))
            .collect();
        if !cache.contains_key(&(e.source.id.clone(), false)) && !cache.contains_key(&(e.source.id.clone(), true)) {
            todo.push(false);
        }
        for all in todo {
            self.refresh(e, &opts_for(all), &mut cache)?;
        }
        e.backend.snapshot()?;
        timed_stamp(e)
    }

    pub(super) fn invalidate(&self, id: &str) {
        let mut local = lock(&self.scans);
        for all in [false, true] {
            if let Some(c) = local.get_mut(&(id.to_string(), all)) {
                c.dirty = true;
            }
        }
    }

    pub(super) fn clear(&self) {
        lock(&self.scans).clear();
    }

    pub(super) fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}
