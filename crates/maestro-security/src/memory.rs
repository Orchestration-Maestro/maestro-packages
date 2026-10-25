//! Semantic memory: a small, dependency-free vector index over findings.
//!
//! Embeddings use deterministic feature hashing (token hashes spread into a
//! fixed-dimension vector with signed sublinear weighting, then L2
//! normalized), so the same text always yields the same vector across runs
//! and platforms, with no model call and no external crate. Search is cosine
//! similarity over the normalized vectors. The index is persisted under the
//! shared base state directory so it survives across tool calls.

use serde::{Deserialize, Serialize};

use crate::evidence;
use crate::store::now_secs;

/// Vector dimension for the feature-hashing embedder.
const DIM: usize = 256;
/// Schema tag for the persisted memory index.
pub const SCHEMA: &str = "maestro-security/memory/1";

/// One indexed memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Stable id, e.g. `mem-0001`.
    pub id: String,
    /// Target the text came from.
    pub target: String,
    /// The indexed text (finding title + class + impact + fix).
    pub text: String,
    /// The normalized embedding vector.
    pub vector: Vec<f32>,
    /// Unix seconds indexed.
    pub created_at: u64,
}

/// One search hit: an entry plus its similarity score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHit {
    /// The entry id.
    pub id: String,
    /// The target.
    pub target: String,
    /// The indexed text.
    pub text: String,
    /// Cosine similarity in `[0, 1]` for matching vectors.
    pub score: f32,
}

/// The semantic memory index.
#[derive(Debug, Clone, Default)]
pub struct Memory {
    /// Indexed entries.
    entries: Vec<MemoryEntry>,
}

impl Memory {
    /// Load the persisted index; a missing file is an empty index.
    ///
    /// # Errors
    /// Fails if the file exists but is malformed.
    pub fn load() -> Result<Self, String> {
        let path = evidence::base_dir()?.join("memory.json");
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(Self::default());
        };
        let doc: MemoryDoc =
            serde_json::from_str(&text).map_err(|e| format!("memory.json: {e}"))?;
        Ok(Self {
            entries: doc.entries,
        })
    }

    /// An empty, in-memory index (used by tests and one-shot calls).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Persist the index to disk.
    ///
    /// # Errors
    /// Fails if the state directory cannot be created or the write fails.
    pub fn save(&self) -> Result<(), String> {
        let path = evidence::base_dir()?.join("memory.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("memory dir: {e}"))?;
        }
        let doc = MemoryDoc {
            schema: SCHEMA.into(),
            entries: self.entries.clone(),
        };
        let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("memory write: {e}"))
    }

    /// Index one piece of text; returns its id.
    ///
    /// # Errors
    /// Fails if the index cannot be persisted.
    pub fn index(&mut self, target: &str, text: &str) -> Result<String, String> {
        let id = format!("mem-{:04}", self.entries.len() + 1);
        let entry = MemoryEntry {
            id: id.clone(),
            target: target.into(),
            text: text.into(),
            vector: embed(text),
            created_at: now_secs(),
        };
        self.entries.push(entry);
        self.save()?;
        Ok(id)
    }

    /// The top-`k` entries most similar to `query` (cosine, descending).
    /// An empty index or an empty query returns no hits.
    #[must_use]
    pub fn search(&self, query: &str, k: usize) -> Vec<MemoryHit> {
        if self.entries.is_empty() || query.trim().is_empty() || k == 0 {
            return Vec::new();
        }
        let qv = embed(query);
        let mut scored: Vec<MemoryHit> = self
            .entries
            .iter()
            .map(|e| MemoryHit {
                id: e.id.clone(),
                target: e.target.clone(),
                text: e.text.clone(),
                score: cosine(&qv, &e.vector),
            })
            .collect();
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        scored.truncate(k);
        scored
    }

    /// Number of indexed entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing is indexed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The persisted memory document.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryDoc {
    /// Schema tag.
    schema: String,
    /// Indexed entries.
    entries: Vec<MemoryEntry>,
}

/// Embed text with deterministic feature hashing into a normalized vector.
#[must_use]
pub fn embed(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; DIM];
    for token in tokenize(text) {
        let h = fnv1a(token.as_bytes());
        let bin = usize::try_from(h % DIM as u64).unwrap_or_default();
        let sign = if (h >> 32) & 1 == 0 { 1.0 } else { -1.0 };
        v[bin] += sign;
    }
    normalize(&mut v);
    v
}

/// Cosine similarity of two vectors (dot product when both are normalized).
#[must_use]
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    a[..n].iter().zip(&b[..n]).map(|(x, y)| x * y).sum()
}

/// L2-normalize a vector in place; a zero vector is left as-is.
fn normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Lowercase alphanumeric tokens of four or fewer characters are kept only if
/// they are all-alnum; longer tokens are kept as-is. Non-alnum runs separate.
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()) {
        if word.len() >= 2 {
            out.push(word.to_string());
        }
    }
    out
}

/// 64-bit FNV-1a hash: stable across runs and platforms, no crate needed.
#[must_use]
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    //! Unit tests for the embedder and the memory index.

    use super::{cosine, embed, tokenize, Memory};

    /// The same text always embeds to the same vector.
    #[test]
    fn embed_is_deterministic() {
        assert_eq!(embed("open port 22 ssh"), embed("open port 22 ssh"));
    }

    /// Identical vectors score 1.0; unrelated text scores lower.
    #[test]
    fn cosine_ranks_related_higher() {
        let q = embed("ssh root login exposed");
        let related = embed("ssh root login is exposed on port 22");
        let unrelated = embed("the http cache is stale and returns 304");
        assert!(cosine(&q, &related) > cosine(&q, &unrelated));
    }

    /// Search returns the most similar entry first.
    #[test]
    fn search_orders_by_similarity() {
        let mut mem = Memory::new();
        mem.entries.push(entry("mem-0001", "ssh root login exposed"));
        mem.entries.push(entry("mem-0002", "http cache stale 304"));
        let hits = mem.search("ssh root login", 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "mem-0001");
    }

    /// Tokenizer keeps words of two or more characters, lowercased.
    #[test]
    fn tokenizer_filters_short_and_lowercases() {
        assert_eq!(tokenize("Port 22 SSH"), vec!["port", "22", "ssh"]);
    }

    fn entry(id: &str, text: &str) -> super::MemoryEntry {
        super::MemoryEntry {
            id: id.into(),
            target: "t".into(),
            text: text.into(),
            vector: embed(text),
            created_at: 0,
        }
    }
}
