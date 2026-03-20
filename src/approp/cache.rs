//! Cache for suggest/accept workflows.
//!
//! Stores suggestion results from `normalize suggest-text-match`,
//! `normalize suggest-llm`, and `link suggest` in an OS-appropriate
//! cache directory (`$HOME/.congress-approp/cache/`).
//!
//! The cache enables the suggest/accept pattern:
//! 1. `suggest` computes results, writes to cache, displays to user
//! 2. User reviews and picks hashes
//! 3. `accept` reads from cache, matches hashes, writes to persistent storage
//!
//! Cache entries are automatically invalidated when the underlying bill
//! data changes (tracked via extraction.json modification times).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A cached set of suggestions from a suggest command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSuggestions {
    /// Schema version for forward compatibility.
    pub schema_version: String,
    /// The resolved data directory path that produced these suggestions.
    pub data_dir: String,
    /// Hash of extraction.json modification times — used for invalidation.
    pub data_hash: String,
    /// Which command produced these suggestions.
    pub command: String,
    /// Unix timestamp when the cache was written.
    pub timestamp: f64,
    /// The actual suggestions, stored as opaque JSON values.
    /// Each value must have a "hash" field for the accept command to match on.
    pub suggestions: Vec<serde_json::Value>,
}

/// Get the cache directory path: `$HOME/.congress-approp/cache/`
pub fn cache_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
    Ok(PathBuf::from(home).join(".congress-approp").join("cache"))
}

/// Compute a hash of the data directory's state for cache invalidation.
///
/// Hashes the modification times of all `extraction.json` files found
/// recursively under `data_dir`. If any bill is added, removed, or
/// re-extracted, this hash changes and the cache is invalidated.
pub fn compute_data_hash(data_dir: &Path) -> String {
    let mut entries: Vec<String> = Vec::new();

    if let Ok(reader) = std::fs::read_dir(data_dir) {
        let mut dirs: Vec<_> = reader.filter_map(|e| e.ok()).collect();
        dirs.sort_by_key(|e| e.file_name());

        for entry in dirs {
            let ext_path = entry.path().join("extraction.json");
            if ext_path.is_file() {
                let mtime = ext_path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);
                entries.push(format!("{}:{}", entry.file_name().to_string_lossy(), mtime));
            }
        }
    }

    let combined = entries.join("|");
    let digest = Sha256::digest(combined.as_bytes());
    format!("{:x}", digest)[..16].to_string()
}

/// Compute a cache key from the data directory path.
///
/// Different `--dir` values produce different cache files, even if they
/// contain the same bills. The key is derived from the canonical
/// (resolved) path.
pub fn compute_cache_key(data_dir: &Path) -> String {
    let canonical = data_dir
        .canonicalize()
        .unwrap_or_else(|_| data_dir.to_path_buf());
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    format!("{:x}", digest)[..12].to_string()
}

/// Get the cache file path for a specific command and data directory.
///
/// Format: `~/.congress-approp/cache/{command}-{cache_key}.json`
pub fn cache_path(data_dir: &Path, command: &str) -> Result<PathBuf> {
    let dir = cache_dir()?;
    let key = compute_cache_key(data_dir);
    Ok(dir.join(format!("{command}-{key}.json")))
}

/// Write suggestions to the cache.
///
/// Creates the cache directory if it doesn't exist. The cache entry
/// includes a `data_hash` for invalidation checking on read.
pub fn write_suggestions(
    data_dir: &Path,
    command: &str,
    suggestions: &[serde_json::Value],
) -> Result<PathBuf> {
    let path = cache_path(data_dir, command)?;

    // Create cache directory
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache directory: {}", parent.display()))?;
    }

    let entry = CachedSuggestions {
        schema_version: "1.0".to_string(),
        data_dir: data_dir
            .canonicalize()
            .unwrap_or_else(|_| data_dir.to_path_buf())
            .to_string_lossy()
            .to_string(),
        data_hash: compute_data_hash(data_dir),
        command: command.to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
        suggestions: suggestions.to_vec(),
    };

    let json = serde_json::to_string_pretty(&entry)?;
    std::fs::write(&path, json)
        .with_context(|| format!("Failed to write cache file: {}", path.display()))?;

    Ok(path)
}

/// Read suggestions from cache.
///
/// Returns `None` if:
/// - The cache file doesn't exist
/// - The cache file can't be parsed
/// - The cache is stale (data_hash doesn't match current state)
pub fn read_suggestions(data_dir: &Path, command: &str) -> Result<Option<Vec<serde_json::Value>>> {
    let path = cache_path(data_dir, command)?;

    if !path.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read cache file: {}", path.display()))?;

    let entry: CachedSuggestions = match serde_json::from_str(&content) {
        Ok(e) => e,
        Err(_) => return Ok(None), // Corrupt cache — treat as miss
    };

    // Check staleness
    let current_hash = compute_data_hash(data_dir);
    if entry.data_hash != current_hash {
        tracing::debug!(
            "Cache stale for {command} (data_hash {} != {current_hash})",
            entry.data_hash
        );
        return Ok(None);
    }

    Ok(Some(entry.suggestions))
}

/// Read suggestions from multiple cache sources.
///
/// Tries each command name in order and returns the first valid
/// (non-stale) cache hit. Used by `accept` to find suggestions from
/// either `suggest-text-match` or `suggest-llm`.
pub fn read_any_suggestions(
    data_dir: &Path,
    commands: &[&str],
) -> Result<Option<(String, Vec<serde_json::Value>)>> {
    for command in commands {
        if let Some(suggestions) = read_suggestions(data_dir, command)? {
            return Ok(Some((command.to_string(), suggestions)));
        }
    }
    Ok(None)
}

/// Find a suggestion by hash across all cached suggestion sources.
///
/// Returns the matching suggestion JSON value if found.
pub fn find_by_hash(
    data_dir: &Path,
    hash: &str,
    commands: &[&str],
) -> Result<Option<serde_json::Value>> {
    for command in commands {
        if let Some(suggestions) = read_suggestions(data_dir, command)? {
            for s in &suggestions {
                if s.get("hash").and_then(|h| h.as_str()) == Some(hash) {
                    return Ok(Some(s.clone()));
                }
            }
        }
    }
    Ok(None)
}

/// Remove all cache files for a data directory.
pub fn clear_cache(data_dir: &Path) -> Result<usize> {
    let key = compute_cache_key(data_dir);
    let dir = cache_dir()?;

    if !dir.is_dir() {
        return Ok(0);
    }

    let mut removed = 0;
    for entry in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.contains(&key)
            && name_str.ends_with(".json")
            && std::fs::remove_file(entry.path()).is_ok()
        {
            removed += 1;
        }
    }

    Ok(removed)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_data_hash_deterministic() {
        let dir = tempfile::TempDir::new().unwrap();
        let bill_dir = dir.path().join("118-hr9999");
        std::fs::create_dir_all(&bill_dir).unwrap();
        std::fs::write(bill_dir.join("extraction.json"), "{}").unwrap();

        let h1 = compute_data_hash(dir.path());
        let h2 = compute_data_hash(dir.path());
        assert_eq!(h1, h2, "Same data should produce same hash");
        assert_eq!(h1.len(), 16, "Hash should be 16 hex chars");
    }

    #[test]
    fn test_compute_data_hash_changes_on_new_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let bill_dir = dir.path().join("118-hr9999");
        std::fs::create_dir_all(&bill_dir).unwrap();
        std::fs::write(bill_dir.join("extraction.json"), "{}").unwrap();

        let h1 = compute_data_hash(dir.path());

        // Add another bill
        let bill_dir2 = dir.path().join("118-hr9998");
        std::fs::create_dir_all(&bill_dir2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(bill_dir2.join("extraction.json"), "{}").unwrap();

        let h2 = compute_data_hash(dir.path());
        assert_ne!(h1, h2, "Adding a bill should change the hash");
    }

    #[test]
    fn test_compute_cache_key_deterministic() {
        let k1 = compute_cache_key(Path::new("/some/path"));
        let k2 = compute_cache_key(Path::new("/some/path"));
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 12);
    }

    #[test]
    fn test_compute_cache_key_differs_for_different_paths() {
        let k1 = compute_cache_key(Path::new("/path/a"));
        let k2 = compute_cache_key(Path::new("/path/b"));
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_write_read_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let bill_dir = dir.path().join("118-hr9999");
        std::fs::create_dir_all(&bill_dir).unwrap();
        std::fs::write(bill_dir.join("extraction.json"), "{}").unwrap();

        let suggestions = vec![serde_json::json!({
            "hash": "abc12345",
            "canonical": "Test Agency",
            "members": ["Other Agency"],
        })];

        let path = write_suggestions(dir.path(), "suggest-text-match", &suggestions).unwrap();
        assert!(path.is_file());

        let loaded = read_suggestions(dir.path(), "suggest-text-match")
            .unwrap()
            .unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].get("hash").unwrap().as_str().unwrap(), "abc12345");
    }

    #[test]
    fn test_cache_invalidation() {
        let dir = tempfile::TempDir::new().unwrap();
        let bill_dir = dir.path().join("118-hr9999");
        std::fs::create_dir_all(&bill_dir).unwrap();
        std::fs::write(bill_dir.join("extraction.json"), "{}").unwrap();

        let suggestions = vec![serde_json::json!({"hash": "test1234"})];
        write_suggestions(dir.path(), "suggest-text-match", &suggestions).unwrap();

        // Cache should be readable
        assert!(
            read_suggestions(dir.path(), "suggest-text-match")
                .unwrap()
                .is_some()
        );

        // Modify data
        let bill_dir2 = dir.path().join("118-hr9998");
        std::fs::create_dir_all(&bill_dir2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(bill_dir2.join("extraction.json"), "{}").unwrap();

        // Cache should now be stale
        assert!(
            read_suggestions(dir.path(), "suggest-text-match")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_read_missing_returns_none() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(
            read_suggestions(dir.path(), "suggest-text-match")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_read_any_suggestions() {
        let dir = tempfile::TempDir::new().unwrap();
        let bill_dir = dir.path().join("118-hr9999");
        std::fs::create_dir_all(&bill_dir).unwrap();
        std::fs::write(bill_dir.join("extraction.json"), "{}").unwrap();

        // Only write llm cache
        let suggestions = vec![serde_json::json!({"hash": "llm12345"})];
        write_suggestions(dir.path(), "suggest-llm", &suggestions).unwrap();

        // Should find it via read_any
        let result =
            read_any_suggestions(dir.path(), &["suggest-text-match", "suggest-llm"]).unwrap();
        assert!(result.is_some());
        let (cmd, sugs) = result.unwrap();
        assert_eq!(cmd, "suggest-llm");
        assert_eq!(sugs.len(), 1);
    }

    #[test]
    fn test_find_by_hash() {
        let dir = tempfile::TempDir::new().unwrap();
        let bill_dir = dir.path().join("118-hr9999");
        std::fs::create_dir_all(&bill_dir).unwrap();
        std::fs::write(bill_dir.join("extraction.json"), "{}").unwrap();

        let suggestions = vec![
            serde_json::json!({"hash": "aaa11111", "canonical": "A"}),
            serde_json::json!({"hash": "bbb22222", "canonical": "B"}),
        ];
        write_suggestions(dir.path(), "suggest-text-match", &suggestions).unwrap();

        let found = find_by_hash(dir.path(), "bbb22222", &["suggest-text-match"]).unwrap();
        assert!(found.is_some());
        assert_eq!(
            found.unwrap().get("canonical").unwrap().as_str().unwrap(),
            "B"
        );

        let not_found = find_by_hash(dir.path(), "deadbeef", &["suggest-text-match"]).unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_clear_cache() {
        let dir = tempfile::TempDir::new().unwrap();
        let bill_dir = dir.path().join("118-hr9999");
        std::fs::create_dir_all(&bill_dir).unwrap();
        std::fs::write(bill_dir.join("extraction.json"), "{}").unwrap();

        write_suggestions(dir.path(), "suggest-text-match", &[]).unwrap();
        write_suggestions(dir.path(), "suggest-llm", &[]).unwrap();

        let removed = clear_cache(dir.path()).unwrap();
        assert_eq!(removed, 2);

        assert!(
            read_suggestions(dir.path(), "suggest-text-match")
                .unwrap()
                .is_none()
        );
    }
}
