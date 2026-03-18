//! Shared bill loading and directory walking utilities.
//!
//! Provides functions to discover bill directories and deserialize
//! extraction/verification artifacts from JSON files on disk.

use crate::approp::bill_meta::BillMeta;
use crate::approp::ontology::{BillExtraction, ExtractionMetadata};
use crate::approp::verification::VerificationReport;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::debug;
use walkdir::WalkDir;

/// A bill directory with its loaded artifacts.
#[derive(Debug, Clone)]
pub struct LoadedBill {
    /// Path to the bill directory
    pub dir: PathBuf,
    /// The extraction output (provisions, summary, bill info)
    pub extraction: BillExtraction,
    /// Verification report, if available
    pub verification: Option<VerificationReport>,
    /// Extraction metadata (model, prompt version, timestamps), if available
    pub metadata: Option<ExtractionMetadata>,
    /// Bill-level metadata (fiscal years, jurisdictions, advance classification), if available
    pub bill_meta: Option<BillMeta>,
}

/// Walk a directory tree, find all bill directories (those containing extraction.json),
/// load and deserialize all artifacts. Returns sorted by bill identifier.
pub fn load_bills(dir: &Path) -> Result<Vec<LoadedBill>> {
    let extraction_files = find_files(dir, "extraction.json");

    if extraction_files.is_empty() {
        return Ok(Vec::new());
    }

    let mut bills = Vec::with_capacity(extraction_files.len());

    for ext_path in &extraction_files {
        let bill_dir = ext_path.parent().unwrap_or(Path::new(".")).to_path_buf();

        let extraction: BillExtraction = load_json(ext_path)
            .with_context(|| format!("Failed to load {}", ext_path.display()))?;

        let verification: Option<VerificationReport> =
            load_json_optional(&bill_dir.join("verification.json"));

        let metadata: Option<ExtractionMetadata> =
            load_json_optional(&bill_dir.join("metadata.json"));

        let bill_meta: Option<BillMeta> = load_json_optional(&bill_dir.join("bill_meta.json"));

        debug!(
            bill = extraction.bill.identifier,
            dir = %bill_dir.display(),
            "Loaded bill"
        );

        bills.push(LoadedBill {
            dir: bill_dir,
            extraction,
            verification,
            metadata,
            bill_meta,
        });
    }

    // Sort by bill identifier for deterministic ordering
    bills.sort_by(|a, b| {
        a.extraction
            .bill
            .identifier
            .cmp(&b.extraction.bill.identifier)
    });

    Ok(bills)
}

/// Find all bill source files (BILLS-*.xml or BILLS-*.txt) in a directory tree.
/// Prefers XML over TXT when both exist for the same bill.
/// Returns (label, path) pairs where label is the parent directory name.
pub fn find_bill_sources(dir: &Path) -> Vec<(String, PathBuf)> {
    // Collect all BILLS-* files grouped by directory
    let mut by_dir: std::collections::HashMap<PathBuf, Vec<PathBuf>> =
        std::collections::HashMap::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file()
            && path
                .file_stem()
                .is_some_and(|n| n.to_string_lossy().starts_with("BILLS-"))
            && path.extension().is_some_and(|e| e == "xml" || e == "txt")
        {
            let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            by_dir.entry(parent).or_default().push(path.to_path_buf());
        }
    }

    let mut results = Vec::new();
    for (parent, files) in &by_dir {
        // Prefer XML over TXT. Group by stem to handle cases where both exist.
        let mut by_stem: std::collections::HashMap<String, PathBuf> =
            std::collections::HashMap::new();
        for file in files {
            let stem = file
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let ext = file
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match by_stem.get(&stem) {
                Some(existing) => {
                    // XML wins over TXT
                    let existing_ext = existing
                        .extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if ext == "xml" && existing_ext != "xml" {
                        by_stem.insert(stem, file.clone());
                    }
                }
                None => {
                    by_stem.insert(stem, file.clone());
                }
            }
        }

        // Prefer enrolled versions: if any file stem ends with "enr",
        // keep only enrolled files and discard other versions (ih, eh, eas, etc.)
        // to avoid processing draft versions that may have different XML structures.
        let has_enrolled = by_stem.keys().any(|s| s.ends_with("enr"));

        let label = parent
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        for (stem, path) in &by_stem {
            if has_enrolled && !stem.ends_with("enr") {
                continue;
            }
            results.push((label.clone(), path.clone()));
        }
    }

    results.sort_by(|a, b| a.1.cmp(&b.1));
    results
}

/// Find all bill text files (BILLS-*.txt) in a directory tree.
/// Returns (label, path) pairs where label is the parent directory name.
pub fn find_bill_texts(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file()
            && path.extension().is_some_and(|e| e == "txt")
            && path
                .file_stem()
                .is_some_and(|n| n.to_string_lossy().starts_with("BILLS-"))
        {
            let label = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            results.push((label, path.to_path_buf()));
        }
    }
    results.sort_by(|a, b| a.1.cmp(&b.1));
    results
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Find all files with a specific name in a directory tree.
fn find_files(dir: &Path, filename: &str) -> Vec<PathBuf> {
    let mut results = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.file_name().is_some_and(|n| n == filename) {
            results.push(path.to_path_buf());
        }
    }
    results.sort();
    results
}

/// Deserialize a JSON file into a typed value.
fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let value: T = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(value)
}

/// Try to deserialize a JSON file, returning None if the file doesn't exist
/// or can't be parsed.
fn load_json_optional<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    if !path.exists() {
        return None;
    }
    match load_json(path) {
        Ok(v) => Some(v),
        Err(e) => {
            debug!("Could not load {}: {e}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_minimal_extraction() -> String {
        serde_json::json!({
            "bill": {
                "identifier": "H.R. 1",
                "classification": "regular",
                "short_title": null,
                "fiscal_years": [2024],
                "divisions": [],
                "public_law": null
            },
            "provisions": [],
            "summary": {
                "total_provisions": 0,
                "by_division": {},
                "by_type": {},
                "total_budget_authority": 0,
                "total_rescissions": 0,
                "sections_with_no_provisions": [],
                "flagged_issues": []
            }
        })
        .to_string()
    }

    #[test]
    fn load_bills_empty_dir() {
        let dir = TempDir::new().unwrap();
        let bills = load_bills(dir.path()).unwrap();
        assert!(bills.is_empty());
    }

    #[test]
    fn load_bills_finds_extraction() {
        let dir = TempDir::new().unwrap();
        let bill_dir = dir.path().join("hr").join("1");
        fs::create_dir_all(&bill_dir).unwrap();
        fs::write(bill_dir.join("extraction.json"), make_minimal_extraction()).unwrap();

        let bills = load_bills(dir.path()).unwrap();
        assert_eq!(bills.len(), 1);
        assert_eq!(bills[0].extraction.bill.identifier, "H.R. 1");
        assert!(bills[0].verification.is_none());
        assert!(bills[0].metadata.is_none());
    }

    #[test]
    fn find_bill_texts_filters_correctly() {
        let dir = TempDir::new().unwrap();
        let bill_dir = dir.path().join("9468");
        fs::create_dir_all(&bill_dir).unwrap();
        fs::write(bill_dir.join("BILLS-118hr9468enr.txt"), "bill text").unwrap();
        fs::write(bill_dir.join("notes.txt"), "not a bill").unwrap();

        let texts = find_bill_texts(dir.path());
        assert_eq!(texts.len(), 1);
        assert!(texts[0].1.to_string_lossy().contains("BILLS-"));
    }

    #[test]
    fn find_bill_sources_prefers_xml() {
        let dir = TempDir::new().unwrap();
        let bill_dir = dir.path().join("4366");
        fs::create_dir_all(&bill_dir).unwrap();
        fs::write(bill_dir.join("BILLS-118hr4366enr.xml"), "<bill/>").unwrap();
        fs::write(bill_dir.join("BILLS-118hr4366enr.txt"), "text").unwrap();

        let sources = find_bill_sources(dir.path());
        assert_eq!(sources.len(), 1);
        assert!(sources[0].1.to_string_lossy().ends_with(".xml"));
    }

    #[test]
    fn find_bill_sources_falls_back_to_txt() {
        let dir = TempDir::new().unwrap();
        let bill_dir = dir.path().join("9468");
        fs::create_dir_all(&bill_dir).unwrap();
        fs::write(bill_dir.join("BILLS-118hr9468enr.txt"), "text only").unwrap();

        let sources = find_bill_sources(dir.path());
        assert_eq!(sources.len(), 1);
        assert!(sources[0].1.to_string_lossy().ends_with(".txt"));
    }

    #[test]
    fn find_bill_sources_prefers_enrolled_over_other_versions() {
        let dir = TempDir::new().unwrap();
        let bill_dir = dir.path().join("7463");
        fs::create_dir_all(&bill_dir).unwrap();
        fs::write(bill_dir.join("BILLS-118hr7463enr.xml"), "<bill/>").unwrap();
        fs::write(bill_dir.join("BILLS-118hr7463ih.xml"), "<bill/>").unwrap();
        fs::write(bill_dir.join("BILLS-118hr7463eh.xml"), "<bill/>").unwrap();
        fs::write(bill_dir.join("BILLS-118hr7463eas.xml"), "<bill/>").unwrap();

        let sources = find_bill_sources(dir.path());
        assert_eq!(sources.len(), 1, "Should return only the enrolled version");
        assert!(
            sources[0].1.to_string_lossy().contains("enr"),
            "Should be the enrolled version"
        );
    }

    #[test]
    fn find_bill_sources_keeps_all_if_no_enrolled() {
        let dir = TempDir::new().unwrap();
        let bill_dir = dir.path().join("9999");
        fs::create_dir_all(&bill_dir).unwrap();
        fs::write(bill_dir.join("BILLS-118hr9999ih.xml"), "<bill/>").unwrap();
        fs::write(bill_dir.join("BILLS-118hr9999eh.xml"), "<bill/>").unwrap();

        let sources = find_bill_sources(dir.path());
        assert_eq!(
            sources.len(),
            2,
            "Should return all versions when no enrolled exists"
        );
    }
}
