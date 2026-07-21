//! Local embedding retrieval branch (PoC lane B1, feature `local-embedding`).
//!
//! Adds a dense-vector recall branch on top of the BM25 / TF-IDF hybrid
//! retriever: documents are embedded with `fastembed` (ONNX Runtime) and
//! stored in SQLite via the `sqlite-vec` extension. The embedding ranking is
//! fused with the baseline ranking using Reciprocal Rank Fusion (RRF).
//!
//! This module is only compiled with `--features local-embedding`; the
//! default build and CI are unaffected.

use crate::error::{KnowledgeError, Result};
use crate::extract::ExtractedDocument;
use rusqlite::Connection;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

/// RRF smoothing constant (standard value from the original paper).
const RRF_K: f64 = 60.0;

/// Text embedder abstraction so tests can inject a deterministic fake
/// without downloading an ONNX model.
pub trait Embedder: Debug + Send + Sync {
    /// Embed a batch of texts into dense vectors (one vector per text).
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    /// Dimension of the produced vectors.
    fn dim(&self) -> usize;
}

/// `fastembed`-backed embedder (default model: BGE-small-zh-v1.5, 512 dims).
///
/// The first call downloads the ONNX model (~50-100 MB) into the fastembed
/// cache; subsequent runs load it from disk.
pub struct FastembedEmbedder {
    model: std::sync::Mutex<fastembed::TextEmbedding>,
    dim: usize,
}

impl Debug for FastembedEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastembedEmbedder")
            .field("dim", &self.dim)
            .finish_non_exhaustive()
    }
}

impl FastembedEmbedder {
    /// Load the given model, downloading it into `cache_dir` on first use.
    pub fn new(model: fastembed::EmbeddingModel, cache_dir: Option<PathBuf>) -> Result<Self> {
        let mut opts =
            fastembed::TextInitOptions::new(model.clone()).with_show_download_progress(true);
        if let Some(dir) = cache_dir {
            opts = opts.with_cache_dir(dir);
        }
        let dim = fastembed::TextEmbedding::get_model_info(&model)
            .map(|info| info.dim)
            .map_err(|e| KnowledgeError::Embedding(e.to_string()))?;
        let model = fastembed::TextEmbedding::try_new(opts)
            .map_err(|e| KnowledgeError::Embedding(e.to_string()))?;
        Ok(Self {
            model: std::sync::Mutex::new(model),
            dim,
        })
    }
}

impl Embedder for FastembedEmbedder {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut model = self
            .model
            .lock()
            .map_err(|e| KnowledgeError::Embedding(format!("model lock poisoned: {e}")))?;
        model
            .embed(texts, None)
            .map_err(|e| KnowledgeError::Embedding(e.to_string()))
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

/// Register the statically linked `sqlite-vec` extension with SQLite.
// The transmute target type is the standard `sqlite3_extension_init`
// signature; spelling it out adds noise, so silence the annotation lint.
#[allow(clippy::missing_transmute_annotations)]
fn register_sqlite_vec() {
    use std::sync::Once;
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| {
        // SAFETY: `sqlite3_vec_init` is a valid `sqlite3_extension_init`
        // entry point provided by the statically linked sqlite-vec C
        // extension; `sqlite3_auto_extension` is called exactly once here.
        // This is the registration pattern documented by sqlite-vec itself.
        #[allow(unsafe_code)]
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

/// Dense-vector store backed by SQLite + `sqlite-vec`.
///
/// The connection is wrapped in a mutex so the store (and therefore
/// `HybridRetriever`) stays `Sync` for async callers; contention is
/// irrelevant at knowledge-index scale.
#[derive(Debug)]
pub struct VectorStore {
    conn: std::sync::Mutex<Connection>,
}

impl VectorStore {
    /// Open (or create) a vector store file with the given vector dimension.
    pub fn open(path: &Path, dim: usize) -> Result<Self> {
        register_sqlite_vec();
        let conn = Connection::open(path)?;
        Self::init(conn, dim)
    }

    /// Open an in-memory store (used by tests).
    pub fn open_in_memory(dim: usize) -> Result<Self> {
        register_sqlite_vec();
        let conn = Connection::open_in_memory()?;
        Self::init(conn, dim)
    }

    fn init(conn: Connection, dim: usize) -> Result<Self> {
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_vec USING vec0(\
             embedding float[{dim}] distance_metric=cosine)"
        ))?;
        Ok(Self {
            conn: std::sync::Mutex::new(conn),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| KnowledgeError::Embedding(format!("vector store lock poisoned: {e}")))
    }

    /// Insert or replace the vector for a document row id.
    pub fn upsert(&self, rowid: i64, vector: &[f32]) -> Result<()> {
        let blob = vector_blob(vector);
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM knowledge_vec WHERE rowid = ?1",
            rusqlite::params![rowid],
        )?;
        conn.execute(
            "INSERT INTO knowledge_vec(rowid, embedding) VALUES (?1, ?2)",
            rusqlite::params![rowid, blob],
        )?;
        Ok(())
    }

    /// Remove the vector for a document row id, if present.
    pub fn remove(&self, rowid: i64) -> Result<()> {
        self.lock()?.execute(
            "DELETE FROM knowledge_vec WHERE rowid = ?1",
            rusqlite::params![rowid],
        )?;
        Ok(())
    }

    /// KNN search: return `(rowid, cosine_distance)` pairs, nearest first.
    pub fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(i64, f32)>> {
        let blob = vector_blob(query);
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT rowid, distance FROM knowledge_vec \
             WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![blob, limit as i64], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f32>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

/// Serialize an f32 vector into the little-endian blob sqlite-vec expects.
fn vector_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Embedding recall branch: owns the embedder, the vector store, and the
/// path ↔ rowid mapping. Documents are (re-)embedded lazily on search.
#[derive(Debug)]
pub struct EmbeddingBranch {
    embedder: Box<dyn Embedder>,
    store: VectorStore,
    path_to_rowid: HashMap<PathBuf, i64>,
    rowid_to_path: HashMap<i64, PathBuf>,
    next_rowid: i64,
    /// Paths whose search text changed since the last vector refresh.
    dirty: Vec<PathBuf>,
}

impl EmbeddingBranch {
    /// Create a branch over an existing embedder and on-disk store.
    pub fn new(db_path: &Path, embedder: Box<dyn Embedder>) -> Result<Self> {
        let store = VectorStore::open(db_path, embedder.dim())?;
        Ok(Self::with_store(store, embedder))
    }

    /// Create a branch over an explicit store (used by tests).
    pub fn with_store(store: VectorStore, embedder: Box<dyn Embedder>) -> Self {
        Self {
            embedder,
            store,
            path_to_rowid: HashMap::new(),
            rowid_to_path: HashMap::new(),
            next_rowid: 1,
            dirty: Vec::new(),
        }
    }

    /// Mark a document as needing (re-)embedding.
    pub fn mark_dirty(&mut self, path: &Path) {
        self.dirty.push(path.to_path_buf());
    }

    /// Remove a document's vector.
    pub fn remove(&mut self, path: &Path) -> Result<()> {
        if let Some(rowid) = self.path_to_rowid.remove(path) {
            self.rowid_to_path.remove(&rowid);
            self.store.remove(rowid)?;
        }
        self.dirty.retain(|p| p != path);
        Ok(())
    }

    /// Embed all dirty documents and upsert their vectors.
    pub fn ensure_index(
        &mut self,
        documents: &HashMap<PathBuf, ExtractedDocument>,
        search_text: impl Fn(&ExtractedDocument) -> String,
    ) -> Result<()> {
        if self.dirty.is_empty() {
            return Ok(());
        }
        let pending: Vec<(PathBuf, String)> = self
            .dirty
            .drain(..)
            .filter_map(|p| documents.get(&p).map(|d| (p, search_text(d))))
            .collect();
        if pending.is_empty() {
            return Ok(());
        }
        let texts: Vec<String> = pending.iter().map(|(_, t)| t.clone()).collect();
        let vectors = self.embedder.embed(&texts)?;
        for ((path, _), vector) in pending.iter().zip(vectors.iter()) {
            let rowid = match self.path_to_rowid.get(path) {
                Some(&rowid) => rowid,
                None => {
                    let rowid = self.next_rowid;
                    self.next_rowid += 1;
                    self.path_to_rowid.insert(path.clone(), rowid);
                    self.rowid_to_path.insert(rowid, path.clone());
                    rowid
                }
            };
            self.store.upsert(rowid, vector)?;
        }
        Ok(())
    }

    /// Embed the query and return `(path, cosine_similarity)` pairs.
    pub fn search(&self, text: &str, limit: usize) -> Result<Vec<(PathBuf, f32)>> {
        let vectors = self.embedder.embed(&[text.to_string()])?;
        let Some(query) = vectors.into_iter().next() else {
            return Ok(Vec::new());
        };
        let hits = self.store.search(&query, limit)?;
        Ok(hits
            .into_iter()
            .filter_map(|(rowid, distance)| {
                self.rowid_to_path
                    .get(&rowid)
                    .map(|p| (p.clone(), 1.0 - distance))
            })
            .collect())
    }
}

/// Reciprocal Rank Fusion over multiple ranked lists.
///
/// Each list is a sequence of paths ordered from most to least relevant.
/// Returns `(path, rrf_score)` pairs sorted by descending score; every path
/// appearing in any input list is present in the output.
pub fn reciprocal_rank_fusion(lists: &[Vec<PathBuf>]) -> Vec<(PathBuf, f64)> {
    let mut scores: HashMap<PathBuf, f64> = HashMap::new();
    for list in lists {
        for (rank, path) in list.iter().enumerate() {
            *scores.entry(path.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f64 + 1.0);
        }
    }
    let mut out: Vec<(PathBuf, f64)> = scores.into_iter().collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic fake embedder: maps texts to orthogonal-ish directions
    /// via a synonym table, so paraphrase queries hit without any model.
    #[derive(Debug)]
    struct FakeEmbedder {
        /// (token, dimension) pairs: a text containing `token` gets +1 at `dimension`.
        table: Vec<(String, usize)>,
        dim: usize,
    }

    impl Embedder for FakeEmbedder {
        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|t| {
                    let mut v = vec![0.0f32; self.dim];
                    for (token, dim) in &self.table {
                        if t.contains(token.as_str()) {
                            v[*dim] += 1.0;
                        }
                    }
                    // Avoid all-zero vectors (undefined cosine distance).
                    v[self.dim - 1] = 1.0;
                    v
                })
                .collect())
        }

        fn dim(&self) -> usize {
            self.dim
        }
    }

    #[test]
    fn vector_store_roundtrip() {
        let store = VectorStore::open_in_memory(3).unwrap();
        store.upsert(1, &[1.0, 0.0, 0.0]).unwrap();
        store.upsert(2, &[0.0, 1.0, 0.0]).unwrap();
        store.upsert(3, &[0.9, 0.1, 0.0]).unwrap();

        let hits = store.search(&[1.0, 0.0, 0.0], 3).unwrap();
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].0, 1, "exact match must rank first");
        assert_eq!(hits[1].0, 3, "near vector must rank second");

        store.remove(1).unwrap();
        let hits = store.search(&[1.0, 0.0, 0.0], 3).unwrap();
        assert_eq!(hits[0].0, 3);
    }

    #[test]
    fn vector_store_upsert_replaces() {
        let store = VectorStore::open_in_memory(2).unwrap();
        store.upsert(1, &[1.0, 0.0]).unwrap();
        store.upsert(1, &[0.0, 1.0]).unwrap();
        let hits = store.search(&[0.0, 1.0], 1).unwrap();
        assert_eq!(hits[0].0, 1);
        assert!(hits[0].1 < 1e-6);
    }

    #[test]
    fn embedding_branch_paraphrase_recall() {
        // "人工智能" and "深度学习" share a dimension in the fake table,
        // simulating a semantic match that BM25 cannot see.
        let fake = FakeEmbedder {
            table: vec![
                ("人工智能".to_string(), 0),
                ("深度学习".to_string(), 0),
                ("cooking".to_string(), 1),
            ],
            dim: 3,
        };
        let mut branch =
            EmbeddingBranch::with_store(VectorStore::open_in_memory(3).unwrap(), Box::new(fake));

        let mut documents = HashMap::new();
        let doc = ExtractedDocument {
            path: PathBuf::from("ai.md"),
            title: Some("深度学习入门".to_string()),
            content: "深度学习与神经网络基础。".to_string(),
            frontmatter: serde_json::Value::Null,
            links: Vec::new(),
            tags: Vec::new(),
            headings: Vec::new(),
        };
        documents.insert(doc.path.clone(), doc);

        branch.mark_dirty(Path::new("ai.md"));
        branch
            .ensure_index(&documents, |d| {
                format!("{} {}", d.title.clone().unwrap_or_default(), d.content)
            })
            .unwrap();

        let hits = branch.search("人工智能", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, PathBuf::from("ai.md"));
        assert!(hits[0].1 > 0.0);
    }

    #[test]
    fn rrf_orders_by_combined_rank() {
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let c = PathBuf::from("c.md");
        // a ranks first in list 1, b ranks first in list 2 and second in
        // list 1: b should win the fusion.
        let fused =
            reciprocal_rank_fusion(&[vec![a.clone(), b.clone()], vec![b.clone(), c.clone()]]);
        assert_eq!(fused[0].0, b);
        // Every path from every list must appear.
        assert_eq!(fused.len(), 3);
        // Scores must be strictly positive and descending.
        assert!(fused.windows(2).all(|w| w[0].1 >= w[1].1));
        let _ = a;
        let _ = c;
    }
}
