use crate::node::{META_FILE, NODE_DIR, TODO_FILE};
use crate::scan::Watched;
use std::path::Path;
use std::time::UNIX_EPOCH;

fn mtime_ns(m: &std::fs::Metadata) -> u128 {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos())
}

pub fn stamp_paths(root: &Path, paths: &[Watched]) -> String {
    let mut h = sha1_smol::Sha1::new();
    for w in paths {
        let d = if w.path.is_empty() { root.to_path_buf() } else { root.join(&w.path) };
        h.update(w.path.as_bytes());
        h.update(b"\0");
        if w.listed {
            match std::fs::metadata(&d) {
                Ok(st) => h.update(format!("d{}", mtime_ns(&st)).as_bytes()),
                Err(_) => h.update(b"d-"),
            }
        }
        let xs = d.join(NODE_DIR);
        match std::fs::metadata(&xs) {
            Ok(st) if st.is_dir() => {
                h.update(format!("x{}", mtime_ns(&st)).as_bytes());
                for f in [META_FILE, TODO_FILE] {
                    match std::fs::metadata(xs.join(f)) {
                        Ok(st) => h.update(format!("f{}:{}", mtime_ns(&st), st.len()).as_bytes()),
                        Err(_) => h.update(b"f-"),
                    }
                }
            }
            _ => h.update(b"x-"),
        }
        h.update(b"\n");
    }
    h.digest().to_string()
}
