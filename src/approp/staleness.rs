//! Hash-chain staleness detection for the extraction → embedding pipeline.
//!
//! Each bill directory maintains a chain of SHA-256 hashes:
//!
//!   source XML  →  extraction.json  →  embeddings.json
//!
//! This module checks whether any link in the chain is stale (i.e. the
//! upstream file has changed since the downstream artifact was produced).

use crate::approp::loading::LoadedBill;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

/// A warning indicating that a derived artifact is out of date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleWarning {
    /// The source XML has changed since the extraction was produced.
    ExtractionStale { bill: String },
    /// The extraction.json has changed since embeddings were produced.
    EmbeddingsStale { bill: String },
}

impl fmt::Display for StaleWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StaleWarning::ExtractionStale { bill } => {
                write!(f, "{bill}: extraction is stale (source XML has changed)")
            }
            StaleWarning::EmbeddingsStale { bill } => {
                write!(
                    f,
                    "{bill}: embeddings are stale (extraction.json has changed)"
                )
            }
        }
    }
}

/// Compute the SHA-256 hex digest of a file's contents.
pub fn file_sha256(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Check hash-chain integrity for all loaded bills.
///
/// Returns a list of warnings for any stale artifacts found.
pub fn check(bills: &[LoadedBill]) -> Vec<StaleWarning> {
    let mut warnings = Vec::new();

    for bill in bills {
        let identifier = &bill.extraction.bill.identifier;

        // ── 1. Check extraction staleness (source XML → metadata.source_xml_sha256) ──
        if let Some(meta) = &bill.metadata
            && let Some(ref recorded_hash) = meta.source_xml_sha256
            && let Some(xml_path) = find_bills_xml(&bill.dir)
            && let Ok(current_hash) = file_sha256(&xml_path)
            && current_hash != *recorded_hash
        {
            warnings.push(StaleWarning::ExtractionStale {
                bill: identifier.clone(),
            });
        }

        // ── 2. Check embeddings staleness (extraction.json → embeddings.extraction_sha256) ──
        let embeddings_path = bill.dir.join("embeddings.json");
        if embeddings_path.exists()
            && let Ok(emb_text) = std::fs::read_to_string(&embeddings_path)
            && let Ok(emb_json) = serde_json::from_str::<serde_json::Value>(&emb_text)
            && let Some(recorded_hash) = emb_json.get("extraction_sha256").and_then(|v| v.as_str())
        {
            let extraction_path = bill.dir.join("extraction.json");
            if let Ok(current_hash) = file_sha256(&extraction_path)
                && current_hash != recorded_hash
            {
                warnings.push(StaleWarning::EmbeddingsStale {
                    bill: identifier.clone(),
                });
            }
        }
    }

    warnings
}

/// Find a `BILLS-*.xml` file in the given directory.
fn find_bills_xml(dir: &Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|x| x == "xml")
            && path
                .file_stem()
                .is_some_and(|n| n.to_string_lossy().starts_with("BILLS-"))
        {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_extraction_stale() {
        let w = StaleWarning::ExtractionStale {
            bill: "H.R. 4366".to_string(),
        };
        assert!(w.to_string().contains("extraction is stale"));
    }

    #[test]
    fn display_embeddings_stale() {
        let w = StaleWarning::EmbeddingsStale {
            bill: "H.R. 4366".to_string(),
        };
        assert!(w.to_string().contains("embeddings are stale"));
    }

    #[test]
    fn file_sha256_works() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, b"hello world").unwrap();
        let hash = file_sha256(&path).unwrap();
        // Known SHA-256 of "hello world"
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn file_sha256_not_found() {
        let result = file_sha256(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn check_empty_bills() {
        let warnings = check(&[]);
        assert!(warnings.is_empty());
    }
}
