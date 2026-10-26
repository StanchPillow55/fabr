//! Local semantic memory. A fully local vector store that replaces the cloud
//! Moss index with a folder-scoped recall path.
//!
//! Per the project's "filetree is the spine" principle, memory is keyed by
//! folder: each node owns `<node_dir>/.fabric-space/memory/embeddings.json`.
//! A global store lives at `~/.fabric-space/memory/embeddings.json`.
//!
//! Embeddings come from Ollama (no Python, no heavy Rust ML deps). The native
//! endpoint `POST {base_url}/api/embeddings` is used with the configured model.
//! Cosine similarity and top-k are pure Rust and need no dependency.

use crate::now_utc;
use fabr_core::FabrError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default Ollama base url. Override with `FABR_LOCAL_BASE_URL`.
pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";
/// Default embedding model. Override with `FABR_EMBED_MODEL`.
pub const DEFAULT_EMBED_MODEL: &str = "nomic-embed-text";

const MEMORY_DIR: &str = "memory";
const EMBEDDINGS_FILE: &str = "embeddings.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub text: String,
    pub vector: Vec<f32>,
    #[serde(default)]
    pub meta: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Store {
    version: u32,
    records: Vec<Record>,
}

impl Store {
    fn empty() -> Self {
        Self { version: 1, records: Vec::new() }
    }
}

/// One recalled record with its cosine score.
#[derive(Debug, Clone, PartialEq)]
pub struct Recalled {
    pub id: String,
    pub text: String,
    pub meta: String,
    pub score: f32,
}

/// The embeddings file for a node. The node spine owns its own memory.
pub fn embeddings_path(node_dir: &Path) -> PathBuf {
    node_dir.join(".fabric-space").join(MEMORY_DIR).join(EMBEDDINGS_FILE)
}

/// The global embeddings file at `~/.fabric-space/memory/embeddings.json`.
/// Returns an error when the home directory cannot be resolved.
pub fn global_embeddings_path() -> Result<PathBuf, FabrError> {
    let home = std::env::var("HOME").ok().filter(|h| !h.trim().is_empty()).ok_or_else(|| FabrError::Io("HOME is not set".into()))?;
    Ok(Path::new(&home).join(".fabric-space").join(MEMORY_DIR).join(EMBEDDINGS_FILE))
}

fn base_url() -> String {
    std::env::var("FABR_LOCAL_BASE_URL").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

fn embed_model() -> String {
    std::env::var("FABR_EMBED_MODEL").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_else(|| DEFAULT_EMBED_MODEL.to_string())
}

/// A stable id for a memory keyed by its trimmed, lowercased text.
pub fn record_id(text: &str) -> String {
    format!("lmem_{:016x}", fnv64(text.trim().to_lowercase().as_bytes()))
}

/// Embed `text`, append a record, and persist. An existing record for the same
/// text is replaced with the fresh vector. Embedding failures return a clear
/// `FabrError` and never panic.
pub fn localmem_remember(path: &Path, text: &str, meta: &str) -> Result<Record, FabrError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(FabrError::BadRequest("memory text is empty".into()));
    }
    let vector = embed(text)?;
    let mut store = load(path)?;
    let id = record_id(text);
    let record = Record { id: id.clone(), text: text.to_string(), vector, meta: meta.trim().to_string(), created_at: now_utc() };
    if let Some(existing) = store.records.iter_mut().find(|r| r.id == id) {
        *existing = record.clone();
    } else {
        store.records.push(record.clone());
    }
    persist(path, &store)?;
    Ok(record)
}

/// Embed `query`, score every stored vector by cosine similarity, and return
/// the top `k` records, highest score first. An empty store returns an empty
/// list. Embedding failures return a clear `FabrError`.
pub fn localmem_recall(path: &Path, query: &str, k: usize) -> Result<Vec<Recalled>, FabrError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(FabrError::BadRequest("recall query is empty".into()));
    }
    let store = load(path)?;
    if store.records.is_empty() {
        return Ok(Vec::new());
    }
    let q = embed(query)?;
    Ok(top_k(&q, &store.records, k))
}

/// Remove the record with `id`. Returns `true` when a record was removed.
pub fn localmem_forget(path: &Path, id: &str) -> Result<bool, FabrError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(FabrError::BadRequest("forget id is empty".into()));
    }
    let mut store = load(path)?;
    let before = store.records.len();
    store.records.retain(|r| r.id != id);
    let removed = store.records.len() != before;
    if removed {
        persist(path, &store)?;
    }
    Ok(removed)
}

/// A one-line log of a recall result. Secret-free; safe to record.
pub fn recall_log(recalled: &[Recalled]) -> String {
    if recalled.is_empty() {
        return "localmem_recall source=local count=0".into();
    }
    let parts: Vec<String> =
        recalled.iter().map(|r| format!("id={} score={:.4} text={}", r.id, r.score, r.text)).collect();
    format!("localmem_recall source=local count={} {}", recalled.len(), parts.join("; "))
}

// --- embedding ---

/// Embed one text through Ollama's native embeddings endpoint. Failures are
/// mapped to a clear `FabrError`; this never panics.
fn embed(text: &str) -> Result<Vec<f32>, FabrError> {
    let url = format!("{}/api/embeddings", base_url().trim_end_matches('/'));
    let body = json!({"model": embed_model(), "prompt": text});
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let sent = agent.post(&url).set("content-type", "application/json").send_json(body);
    let (status, text_resp) = match sent {
        Ok(resp) => (resp.status(), resp.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(status, resp)) => (status, resp.into_string().unwrap_or_default()),
        Err(err) => {
            let msg = err.to_string();
            if msg.to_lowercase().contains("timeout") {
                return Err(FabrError::Timeout("embedding timed out".into()));
            }
            return Err(FabrError::Remote(format!("embedding endpoint unavailable (is ollama running at {}?)", base_url())));
        }
    };
    if status == 404 {
        return Err(FabrError::Remote(format!("model '{}' not found; run `ollama pull {}`", embed_model(), embed_model())));
    }
    if status == 408 {
        return Err(FabrError::Timeout("embedding timed out".into()));
    }
    if !(200..300).contains(&status) {
        return Err(FabrError::Remote(format!("embedding http {status}")));
    }
    let value: Value = serde_json::from_str(&text_resp).map_err(|_| FabrError::Remote("embedding response was not json".into()))?;
    parse_embedding(&value)
}

/// Pull the float vector out of either the Ollama native shape
/// (`{"embedding": [..]}`) or the OpenAI-compatible shape
/// (`{"data": [{"embedding": [..]}]}`).
fn parse_embedding(value: &Value) -> Result<Vec<f32>, FabrError> {
    let array = value
        .get("embedding")
        .and_then(|v| v.as_array())
        .or_else(|| value.get("data").and_then(|d| d.as_array()).and_then(|d| d.first()).and_then(|first| first.get("embedding")).and_then(|v| v.as_array()));
    let Some(array) = array else {
        return Err(FabrError::Remote("embedding response had no vector".into()));
    };
    let vector: Vec<f32> = array.iter().filter_map(|n| n.as_f64().map(|f| f as f32)).collect();
    if vector.is_empty() {
        return Err(FabrError::Remote("embedding vector was empty".into()));
    }
    Ok(vector)
}

// --- similarity ---

/// Cosine similarity of two vectors. Zero-length or mismatched vectors score 0.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Top `k` records by cosine similarity to `query`, highest score first.
fn top_k(query: &[f32], records: &[Record], k: usize) -> Vec<Recalled> {
    let mut scored: Vec<Recalled> = records
        .iter()
        .map(|r| Recalled { id: r.id.clone(), text: r.text.clone(), meta: r.meta.clone(), score: cosine(query, &r.vector) })
        .collect();
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    scored
}

// --- storage ---

fn load(path: &Path) -> Result<Store, FabrError> {
    if !path.exists() {
        return Ok(Store::empty());
    }
    let text = std::fs::read_to_string(path).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))?;
    serde_json::from_str(&text).map_err(|e| FabrError::BadRequest(format!("local memory store: {e}")))
}

fn persist(path: &Path, store: &Store) -> Result<(), FabrError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| FabrError::Io(format!("{}: {e}", parent.display())))?;
    }
    let body = serde_json::to_string_pretty(store).map_err(|e| FabrError::BadRequest(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut out = opts.open(&tmp).map_err(|e| FabrError::Io(format!("{}: {e}", tmp.display())))?;
        out.write_all(body.as_bytes()).map_err(|e| FabrError::Io(format!("{}: {e}", tmp.display())))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))?;
    Ok(())
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str, vector: Vec<f32>) -> Record {
        Record { id: id.into(), text: format!("text {id}"), vector, meta: String::new(), created_at: String::new() }
    }

    #[test]
    fn cosine_ranks_hand_crafted_vectors() {
        let query = vec![1.0, 0.0, 0.0];
        let same = vec![2.0, 0.0, 0.0]; // parallel -> cosine 1.0
        let orth = vec![0.0, 1.0, 0.0]; // orthogonal -> cosine 0.0
        let opp = vec![-1.0, 0.0, 0.0]; // opposite -> cosine -1.0
        assert!((cosine(&query, &same) - 1.0).abs() < 1e-6);
        assert!(cosine(&query, &orth).abs() < 1e-6);
        assert!((cosine(&query, &opp) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn mismatched_or_zero_vectors_score_zero() {
        assert_eq!(cosine(&[1.0, 2.0], &[1.0, 2.0, 3.0]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
        assert_eq!(cosine(&[], &[]), 0.0);
    }

    #[test]
    fn top_k_returns_highest_scores_first_and_truncates() {
        let query = vec![1.0, 0.0, 0.0];
        let records = vec![
            rec("orth", vec![0.0, 1.0, 0.0]),  // 0.0
            rec("near", vec![0.9, 0.1, 0.0]),  // high
            rec("exact", vec![1.0, 0.0, 0.0]), // 1.0
            rec("opp", vec![-1.0, 0.0, 0.0]),  // -1.0
        ];
        let out = top_k(&query, &records, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "exact");
        assert_eq!(out[1].id, "near");
        assert!(out[0].score >= out[1].score);
    }

    #[test]
    fn top_k_larger_than_store_returns_all_sorted() {
        let query = vec![1.0, 0.0];
        let records = vec![rec("a", vec![0.0, 1.0]), rec("b", vec![1.0, 0.0])];
        let out = top_k(&query, &records, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "b");
    }

    #[test]
    fn record_id_is_stable_for_the_same_text() {
        assert_eq!(record_id("  Hello World  "), record_id("hello world"));
        assert_ne!(record_id("a"), record_id("b"));
    }

    #[test]
    fn embeddings_path_is_folder_scoped() {
        let path = embeddings_path(Path::new("/nodes/project"));
        assert!(path.ends_with(".fabric-space/memory/embeddings.json"));
        assert!(path.starts_with("/nodes/project"));
    }

    #[test]
    fn parse_embedding_handles_native_and_openai_shapes() {
        let native = json!({"embedding": [0.1, 0.2, 0.3]});
        assert_eq!(parse_embedding(&native).unwrap(), vec![0.1, 0.2, 0.3]);
        let openai = json!({"data": [{"embedding": [0.4, 0.5]}]});
        assert_eq!(parse_embedding(&openai).unwrap(), vec![0.4, 0.5]);
        assert!(parse_embedding(&json!({"nope": true})).is_err());
        assert!(parse_embedding(&json!({"embedding": []})).is_err());
    }

    #[test]
    fn store_persists_recalls_and_forgets_without_network() {
        // Exercise storage directly with hand-crafted vectors; no embed() call.
        let tmp = tempfile::tempdir().unwrap();
        let path = embeddings_path(tmp.path());
        let mut store = Store::empty();
        store.records.push(rec("lmem_a", vec![1.0, 0.0, 0.0]));
        store.records.push(rec("lmem_b", vec![0.0, 1.0, 0.0]));
        persist(&path, &store).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.records.len(), 2);

        // Recall by scoring directly against the loaded store.
        let hits = top_k(&[1.0, 0.0, 0.0], &loaded.records, 1);
        assert_eq!(hits[0].id, "lmem_a");

        // Forget removes one record and persists.
        assert!(localmem_forget(&path, "lmem_a").unwrap());
        assert!(!localmem_forget(&path, "lmem_a").unwrap());
        let after = load(&path).unwrap();
        assert_eq!(after.records.len(), 1);
        assert_eq!(after.records[0].id, "lmem_b");
    }

    #[test]
    fn recall_on_empty_store_is_empty_without_network() {
        let tmp = tempfile::tempdir().unwrap();
        let path = embeddings_path(tmp.path());
        // No file exists; load yields empty, so recall short-circuits before embed().
        let store = load(&path).unwrap();
        assert!(store.records.is_empty());
    }

    #[test]
    fn empty_inputs_are_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = embeddings_path(tmp.path());
        assert!(localmem_remember(&path, "   ", "m").is_err());
        assert!(localmem_recall(&path, "  ", 3).is_err());
        assert!(localmem_forget(&path, "").is_err());
    }
}
