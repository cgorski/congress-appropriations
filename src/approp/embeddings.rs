//! Embedding storage: JSON metadata file + binary vector file.
//!
//! Each bill directory can optionally contain:
//!   embeddings.json  — metadata (model, dimensions, hashes)
//!   vectors.bin      — raw float32 array [count × dimensions]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Metadata stored in embeddings.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsMetadata {
    pub schema_version: String,
    pub model: String,
    pub dimensions: usize,
    pub count: usize,
    pub extraction_sha256: String,
    pub vectors_file: String,
    pub vectors_sha256: String,
}

/// Loaded embeddings for one bill.
pub struct LoadedEmbeddings {
    pub metadata: EmbeddingsMetadata,
    /// Flat float32 array: count * dimensions elements.
    pub vectors: Vec<f32>,
}

impl LoadedEmbeddings {
    /// Get the embedding vector for provision at index i.
    pub fn vector(&self, i: usize) -> &[f32] {
        let d = self.metadata.dimensions;
        &self.vectors[i * d..(i + 1) * d]
    }

    pub fn count(&self) -> usize {
        self.metadata.count
    }

    pub fn dimensions(&self) -> usize {
        self.metadata.dimensions
    }
}

/// Load embeddings from a bill directory. Returns None if not present.
pub fn load(dir: &Path) -> Result<Option<LoadedEmbeddings>> {
    let meta_path = dir.join("embeddings.json");
    if !meta_path.exists() {
        return Ok(None);
    }

    let meta_text = std::fs::read_to_string(&meta_path)
        .with_context(|| format!("Failed to read {}", meta_path.display()))?;
    let metadata: EmbeddingsMetadata = serde_json::from_str(&meta_text)
        .with_context(|| format!("Failed to parse {}", meta_path.display()))?;

    let vec_path = dir.join(&metadata.vectors_file);
    let vec_bytes = std::fs::read(&vec_path)
        .with_context(|| format!("Failed to read {}", vec_path.display()))?;

    let expected_len = metadata.count * metadata.dimensions * 4; // f32 = 4 bytes
    anyhow::ensure!(
        vec_bytes.len() == expected_len,
        "vectors.bin size mismatch: expected {} bytes ({} × {} × 4), got {}",
        expected_len,
        metadata.count,
        metadata.dimensions,
        vec_bytes.len()
    );

    // Convert Vec<u8> → Vec<f32> using little-endian byte order
    let vectors: Vec<f32> = vec_bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    Ok(Some(LoadedEmbeddings { metadata, vectors }))
}

/// Save embeddings to a bill directory.
/// Writes embeddings.json (metadata) and vectors.bin (raw float32).
pub fn save(
    dir: &Path,
    model: &str,
    dimensions: usize,
    extraction_sha256: &str,
    vectors: &[f32],
) -> Result<()> {
    let count = vectors.len() / dimensions;
    anyhow::ensure!(
        vectors.len() == count * dimensions,
        "vector length not divisible by dimensions"
    );

    // Write binary vectors
    let vec_path = dir.join("vectors.bin");
    let vec_bytes: Vec<u8> = vectors.iter().flat_map(|f| f.to_le_bytes()).collect();
    std::fs::write(&vec_path, &vec_bytes)
        .with_context(|| format!("Failed to write {}", vec_path.display()))?;

    let vectors_sha256 = format!("{:x}", Sha256::digest(&vec_bytes));

    let metadata = EmbeddingsMetadata {
        schema_version: "1.0".to_string(),
        model: model.to_string(),
        dimensions,
        count,
        extraction_sha256: extraction_sha256.to_string(),
        vectors_file: "vectors.bin".to_string(),
        vectors_sha256,
    };

    let meta_path = dir.join("embeddings.json");
    let meta_text = serde_json::to_string_pretty(&metadata)?;
    std::fs::write(&meta_path, meta_text)
        .with_context(|| format!("Failed to write {}", meta_path.display()))?;

    Ok(())
}

/// Normalize a vector in place (L2 norm).
pub fn normalize(vec: &mut [f32]) {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        vec.iter_mut().for_each(|x| *x /= norm);
    }
}

/// Cosine similarity between two vectors (assumes both are L2-normalized).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Find top-N most similar vectors to query.
/// `vectors` is a flat array of `count` vectors, each of size `dimensions`.
/// Returns (original_index, similarity) pairs, sorted descending.
pub fn top_n_similar(
    query: &[f32],
    vectors: &[f32],
    dimensions: usize,
    n: usize,
    exclude_index: Option<usize>,
) -> Vec<(usize, f32)> {
    let count = vectors.len() / dimensions;
    let mut scores: Vec<(usize, f32)> = (0..count)
        .filter(|i| exclude_index != Some(*i))
        .map(|i| {
            let vec = &vectors[i * dimensions..(i + 1) * dimensions];
            (i, cosine_similarity(query, vec))
        })
        .collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores.truncate(n);
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let vectors: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        save(dir.path(), "test-model", 3, "abc123", &vectors).unwrap();

        let loaded = load(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.count(), 2);
        assert_eq!(loaded.dimensions(), 3);
        assert_eq!(loaded.vector(0), &[1.0, 2.0, 3.0]);
        assert_eq!(loaded.vector(1), &[4.0, 5.0, 6.0]);
        assert_eq!(loaded.metadata.model, "test-model");
        assert_eq!(loaded.metadata.extraction_sha256, "abc123");
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-6);
    }

    #[test]
    fn test_top_n_similar() {
        let vectors: Vec<f32> = vec![
            1.0, 0.0, 0.0, // 0
            0.9, 0.1, 0.0, // 1 — most similar to query
            0.0, 1.0, 0.0, // 2 — least similar
            0.5, 0.5, 0.0, // 3
        ];
        let query = vec![1.0, 0.0, 0.0];
        let results = top_n_similar(&query, &vectors, 3, 2, None);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0); // identity
        assert_eq!(results[1].0, 1); // next most similar
    }

    #[test]
    fn test_top_n_with_exclude() {
        let vectors: Vec<f32> = vec![1.0, 0.0, 0.0, 0.9, 0.1, 0.0, 0.0, 1.0, 0.0];
        let query = vec![1.0, 0.0, 0.0];
        let results = top_n_similar(&query, &vectors, 3, 2, Some(0));
        assert_eq!(results[0].0, 1); // excluded index 0, so index 1 is top
    }

    #[test]
    fn load_missing_returns_none() {
        let dir = tempfile::TempDir::new().unwrap();
        let result = load(dir.path()).unwrap();
        assert!(result.is_none());
    }
}
