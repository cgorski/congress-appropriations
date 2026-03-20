//! Persistent cross-bill provision links.
//!
//! Links represent verified relationships between provisions in different
//! bills — e.g., "Transit Formula Grants in H.R. 4366 is the same account
//! as Transit Formula Grants in H.R. 7148." They are discovered via embedding
//! similarity (`relate`, `link suggest`) and persisted via `link accept`.
//!
//! The link file lives at `<data_dir>/links/links.json` — at the data root,
//! not inside any bill directory, because links are *between* bills.

use crate::approp::bill_meta;
use crate::approp::embeddings::{self, LoadedEmbeddings};
use crate::approp::loading::LoadedBill;
use crate::approp::ontology::Provision;
use crate::approp::query::compute_link_hash;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ─── Types ───────────────────────────────────────────────────────────────────

/// The top-level links file stored at `<dir>/links/links.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinksFile {
    pub schema_version: String,
    pub embedding_model: String,
    pub accepted: Vec<AcceptedLink>,
}

impl LinksFile {
    /// Create a new empty links file.
    pub fn new(embedding_model: &str) -> Self {
        LinksFile {
            schema_version: "1.0".to_string(),
            embedding_model: embedding_model.to_string(),
            accepted: Vec::new(),
        }
    }
}

/// A persisted link between two provisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedLink {
    /// Deterministic 8-char hex hash (same as shown in `relate` output).
    pub hash: String,
    pub source: ProvisionRef,
    pub target: ProvisionRef,
    pub similarity: f32,
    pub relationship: LinkRelationship,
    pub evidence: LinkEvidence,
    pub accepted_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Reference to a specific provision in a specific bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionRef {
    pub bill_dir: String,
    pub provision_index: usize,
    /// Human-readable label (account name or description).
    #[serde(default)]
    pub label: String,
}

/// The type of relationship between two linked provisions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinkRelationship {
    SameAccount,
    Renamed,
    Reclassified,
    Related,
}

impl std::fmt::Display for LinkRelationship {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkRelationship::SameAccount => write!(f, "same_account"),
            LinkRelationship::Renamed => write!(f, "renamed"),
            LinkRelationship::Reclassified => write!(f, "reclassified"),
            LinkRelationship::Related => write!(f, "related"),
        }
    }
}

/// How the link was established.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinkEvidence {
    NameMatch,
    HighSimilarity,
    Manual,
}

// ─── Link Candidates (output of suggest) ─────────────────────────────────────

/// A candidate link produced by `link suggest` — not yet accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkCandidate {
    pub hash: String,
    pub source: ProvisionRef,
    pub target: ProvisionRef,
    pub similarity: f32,
    pub confidence: LinkConfidence,
    pub already_accepted: bool,
    pub source_label: String,
    pub target_label: String,
    pub source_dollars: Option<i64>,
    pub target_dollars: Option<i64>,
}

/// Confidence tier for a link candidate.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinkConfidence {
    /// Name match (case-insensitive, prefix-stripped) — highest confidence.
    Verified,
    /// Similarity >= 0.65 AND same normalized agency.
    High,
    /// Similarity 0.55-0.65, or name mismatch in the 0.65+ zone.
    Uncertain,
}

impl std::fmt::Display for LinkConfidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkConfidence::Verified => write!(f, "verified"),
            LinkConfidence::High => write!(f, "high"),
            LinkConfidence::Uncertain => write!(f, "uncertain"),
        }
    }
}

/// Scope filtering for `link suggest`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkScope {
    /// Compare bills within the same fiscal year (CR ↔ omnibus).
    Intra,
    /// Compare bills across different fiscal years (year-over-year tracking).
    Cross,
    /// Compare all bill pairs regardless of fiscal year.
    All,
}

impl LinkScope {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "intra" => Some(LinkScope::Intra),
            "cross" => Some(LinkScope::Cross),
            "all" => Some(LinkScope::All),
            _ => None,
        }
    }
}

// ─── Suggest Algorithm ───────────────────────────────────────────────────────

/// Compute link candidates across all bill pairs.
///
/// For each top-level budget authority provision in each bill, find the best
/// match in every other bill above the similarity threshold. Candidates are
/// classified by confidence tier using calibrated thresholds from empirical
/// analysis of 6.7M pairwise comparisons.
///
/// # Thresholds (from NEXT_STEPS.md empirical calibration)
/// - Name match (case-insensitive, prefix-stripped) → Verified
/// - sim >= 0.65 AND same normalized agency → High
/// - sim 0.55-0.65 OR name mismatch in 0.65+ zone → Uncertain
/// - sim < 0.55 → excluded
pub fn suggest(
    bills: &[LoadedBill],
    bill_embeddings: &[Option<LoadedEmbeddings>],
    threshold: f32,
    scope: LinkScope,
    existing: &Option<LinksFile>,
    limit: usize,
) -> Vec<LinkCandidate> {
    let accepted_hashes: std::collections::HashSet<&str> = existing
        .as_ref()
        .map(|l| l.accepted.iter().map(|a| a.hash.as_str()).collect())
        .unwrap_or_default();

    // Get embedding model name from the first available embeddings
    let embedding_model = bill_embeddings
        .iter()
        .flatten()
        .next()
        .map(|e| e.metadata.model.as_str())
        .unwrap_or("unknown");

    // Build provision info for quick lookup
    struct ProvInfo {
        bill_pos: usize,
        bill_dir: String,
        bill_id: String,
        bill_fys: Vec<u32>,
        prov_idx: usize,
        canonical_name: String,
        norm_agency: String,
        label: String,
        dollars: Option<i64>,
    }

    let mut all_provisions: Vec<ProvInfo> = Vec::new();

    for (bill_pos, bill) in bills.iter().enumerate() {
        if bill_embeddings[bill_pos].is_none() {
            continue;
        }
        let bill_dir = bill
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let bill_id = bill.extraction.bill.identifier.clone();
        let bill_fys = bill.extraction.bill.fiscal_years.clone();

        for (idx, p) in bill.extraction.provisions.iter().enumerate() {
            // Only consider top-level BA appropriation provisions
            if let Some(amt) = p.amount() {
                if !matches!(
                    amt.semantics,
                    crate::approp::ontology::AmountSemantics::NewBudgetAuthority
                ) {
                    continue;
                }
                if !matches!(p, Provision::Appropriation { .. }) {
                    continue;
                }
                let dl = match p {
                    Provision::Appropriation { detail_level, .. } => detail_level.as_str(),
                    _ => "",
                };
                if dl == "sub_allocation" || dl == "proviso_amount" {
                    continue;
                }

                let acct = p.account_name();
                if acct.is_empty() {
                    continue;
                }

                all_provisions.push(ProvInfo {
                    bill_pos,
                    bill_dir: bill_dir.clone(),
                    bill_id: bill_id.clone(),
                    bill_fys: bill_fys.clone(),
                    prov_idx: idx,
                    canonical_name: bill_meta::normalize_account_name(acct),
                    norm_agency: crate::approp::query::normalize_agency(p.agency()),
                    label: acct.to_string(),
                    dollars: amt.dollars(),
                });
            }
        }
    }

    let mut candidates: Vec<LinkCandidate> = Vec::new();
    // Track best match per (source, target_bill) to avoid duplicates
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for src in &all_provisions {
        let Some(src_emb) = &bill_embeddings[src.bill_pos] else {
            continue;
        };
        if src.prov_idx >= src_emb.count() {
            continue;
        }
        let src_vec = src_emb.vector(src.prov_idx);

        for tgt in &all_provisions {
            // Skip same bill
            if src.bill_pos == tgt.bill_pos {
                continue;
            }
            // Skip same provision (shouldn't happen across bills, but defensive)
            if src.bill_dir == tgt.bill_dir && src.prov_idx == tgt.prov_idx {
                continue;
            }

            // Apply scope filter
            let shares_fy = src.bill_fys.iter().any(|fy| tgt.bill_fys.contains(fy));
            match scope {
                LinkScope::Intra => {
                    if !shares_fy {
                        continue;
                    }
                }
                LinkScope::Cross => {
                    if shares_fy {
                        continue;
                    }
                }
                LinkScope::All => {}
            }

            let Some(tgt_emb) = &bill_embeddings[tgt.bill_pos] else {
                continue;
            };
            if tgt.prov_idx >= tgt_emb.count() {
                continue;
            }
            let tgt_vec = tgt_emb.vector(tgt.prov_idx);

            let sim = embeddings::cosine_similarity(src_vec, tgt_vec);
            if sim < threshold {
                continue;
            }

            // Deduplicate: only keep the best match from src to each target bill
            // Use a directional key to avoid A→B and B→A duplicates
            let dedup_key = if src.bill_dir < tgt.bill_dir
                || (src.bill_dir == tgt.bill_dir && src.prov_idx < tgt.prov_idx)
            {
                format!(
                    "{}:{}→{}:{}",
                    src.bill_dir, src.prov_idx, tgt.bill_dir, tgt.prov_idx
                )
            } else {
                format!(
                    "{}:{}→{}:{}",
                    tgt.bill_dir, tgt.prov_idx, src.bill_dir, src.prov_idx
                )
            };
            if seen.contains(&dedup_key) {
                continue;
            }
            seen.insert(dedup_key);

            // Compute confidence tier
            let name_match =
                !src.canonical_name.is_empty() && src.canonical_name == tgt.canonical_name;
            let same_agency = src.norm_agency == tgt.norm_agency;

            let confidence = if name_match {
                LinkConfidence::Verified
            } else if sim >= 0.65 && same_agency {
                LinkConfidence::High
            } else {
                LinkConfidence::Uncertain
            };

            // Skip uncertain below 0.55
            if confidence == LinkConfidence::Uncertain && sim < 0.55 {
                continue;
            }

            let hash = compute_link_hash(
                &src.bill_dir,
                src.prov_idx,
                &tgt.bill_dir,
                tgt.prov_idx,
                embedding_model,
            );

            let already_accepted = accepted_hashes.contains(hash.as_str());

            candidates.push(LinkCandidate {
                hash,
                source: ProvisionRef {
                    bill_dir: src.bill_dir.clone(),
                    provision_index: src.prov_idx,
                    label: src.label.clone(),
                },
                target: ProvisionRef {
                    bill_dir: tgt.bill_dir.clone(),
                    provision_index: tgt.prov_idx,
                    label: tgt.label.clone(),
                },
                similarity: sim,
                confidence,
                already_accepted,
                source_label: format!("{} — {}", src.bill_id, truncate_str(&src.label, 40)),
                target_label: format!("{} — {}", tgt.bill_id, truncate_str(&tgt.label, 40)),
                source_dollars: src.dollars,
                target_dollars: tgt.dollars,
            });
        }
    }

    // Sort by similarity descending
    candidates.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    candidates.truncate(limit);
    candidates
}

// ─── I/O ─────────────────────────────────────────────────────────────────────

/// Load `links/links.json` from the data root directory.
pub fn load_links(dir: &Path) -> Result<Option<LinksFile>> {
    let path = dir.join("links").join("links.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let links: LinksFile = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(links))
}

/// Save `links/links.json` to the data root directory.
/// Creates the `links/` directory if it doesn't exist.
pub fn save_links(dir: &Path, links: &LinksFile) -> Result<()> {
    let links_dir = dir.join("links");
    std::fs::create_dir_all(&links_dir)
        .with_context(|| format!("Failed to create {}", links_dir.display()))?;
    let path = links_dir.join("links.json");
    let json = serde_json::to_string_pretty(links)?;
    std::fs::write(&path, json).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

// ─── Accept / Remove ─────────────────────────────────────────────────────────

/// Accept link candidates by hash, adding them to the links file.
///
/// Returns the number of newly accepted links.
pub fn accept_links(
    links: &mut LinksFile,
    candidates: &[LinkCandidate],
    hashes: &[&str],
    note: Option<&str>,
    auto: bool,
) -> usize {
    let existing_hashes: std::collections::HashSet<String> =
        links.accepted.iter().map(|l| l.hash.clone()).collect();

    let now = chrono::Utc::now().to_rfc3339();
    let mut accepted_count = 0;

    for candidate in candidates {
        if existing_hashes.contains(&candidate.hash) {
            continue;
        }

        let should_accept = if auto {
            // --auto: accept Verified and High confidence
            matches!(
                candidate.confidence,
                LinkConfidence::Verified | LinkConfidence::High
            )
        } else {
            // Manual: accept only specified hashes
            hashes.contains(&candidate.hash.as_str())
        };

        if !should_accept {
            continue;
        }

        let relationship = if candidate.confidence == LinkConfidence::Verified {
            LinkRelationship::SameAccount
        } else {
            LinkRelationship::Related
        };

        let evidence = if candidate.confidence == LinkConfidence::Verified {
            LinkEvidence::NameMatch
        } else {
            LinkEvidence::HighSimilarity
        };

        links.accepted.push(AcceptedLink {
            hash: candidate.hash.clone(),
            source: candidate.source.clone(),
            target: candidate.target.clone(),
            similarity: candidate.similarity,
            relationship,
            evidence,
            accepted_at: now.clone(),
            note: note.map(|s| s.to_string()),
        });

        accepted_count += 1;
    }

    accepted_count
}

/// Remove accepted links by hash.
///
/// Returns the number of removed links.
pub fn remove_links(links: &mut LinksFile, hashes: &[&str]) -> usize {
    let before = links.accepted.len();
    links
        .accepted
        .retain(|l| !hashes.contains(&l.hash.as_str()));
    before - links.accepted.len()
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max.saturating_sub(1))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}…", &s[..end])
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_links_file_new() {
        let lf = LinksFile::new("test-model");
        assert_eq!(lf.schema_version, "1.0");
        assert_eq!(lf.embedding_model, "test-model");
        assert!(lf.accepted.is_empty());
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut lf = LinksFile::new("test-model");
        lf.accepted.push(AcceptedLink {
            hash: "abcd1234".to_string(),
            source: ProvisionRef {
                bill_dir: "hr4366".to_string(),
                provision_index: 42,
                label: "Test Account".to_string(),
            },
            target: ProvisionRef {
                bill_dir: "hr7148".to_string(),
                provision_index: 99,
                label: "Test Account".to_string(),
            },
            similarity: 0.85,
            relationship: LinkRelationship::SameAccount,
            evidence: LinkEvidence::NameMatch,
            accepted_at: "2026-03-19T00:00:00Z".to_string(),
            note: Some("test note".to_string()),
        });

        save_links(dir.path(), &lf).unwrap();
        let loaded = load_links(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.accepted.len(), 1);
        assert_eq!(loaded.accepted[0].hash, "abcd1234");
        assert_eq!(loaded.accepted[0].note.as_deref(), Some("test note"));
    }

    #[test]
    fn test_load_missing_returns_none() {
        let dir = tempfile::TempDir::new().unwrap();
        let result = load_links(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_links() {
        let mut lf = LinksFile::new("test");
        lf.accepted.push(AcceptedLink {
            hash: "aaaa1111".to_string(),
            source: ProvisionRef {
                bill_dir: "a".to_string(),
                provision_index: 0,
                label: String::new(),
            },
            target: ProvisionRef {
                bill_dir: "b".to_string(),
                provision_index: 0,
                label: String::new(),
            },
            similarity: 0.9,
            relationship: LinkRelationship::SameAccount,
            evidence: LinkEvidence::NameMatch,
            accepted_at: String::new(),
            note: None,
        });
        lf.accepted.push(AcceptedLink {
            hash: "bbbb2222".to_string(),
            source: ProvisionRef {
                bill_dir: "c".to_string(),
                provision_index: 0,
                label: String::new(),
            },
            target: ProvisionRef {
                bill_dir: "d".to_string(),
                provision_index: 0,
                label: String::new(),
            },
            similarity: 0.8,
            relationship: LinkRelationship::Related,
            evidence: LinkEvidence::HighSimilarity,
            accepted_at: String::new(),
            note: None,
        });

        assert_eq!(lf.accepted.len(), 2);
        let removed = remove_links(&mut lf, &["aaaa1111"]);
        assert_eq!(removed, 1);
        assert_eq!(lf.accepted.len(), 1);
        assert_eq!(lf.accepted[0].hash, "bbbb2222");
    }

    #[test]
    fn test_link_confidence_display() {
        assert_eq!(format!("{}", LinkConfidence::Verified), "verified");
        assert_eq!(format!("{}", LinkConfidence::High), "high");
        assert_eq!(format!("{}", LinkConfidence::Uncertain), "uncertain");
    }

    #[test]
    fn test_link_relationship_display() {
        assert_eq!(format!("{}", LinkRelationship::SameAccount), "same_account");
        assert_eq!(format!("{}", LinkRelationship::Renamed), "renamed");
    }

    #[test]
    fn test_link_scope_from_str() {
        assert_eq!(LinkScope::parse("intra"), Some(LinkScope::Intra));
        assert_eq!(LinkScope::parse("cross"), Some(LinkScope::Cross));
        assert_eq!(LinkScope::parse("ALL"), Some(LinkScope::All));
        assert_eq!(LinkScope::parse("invalid"), None);
    }

    #[test]
    fn test_accept_links_manual() {
        let mut lf = LinksFile::new("test");
        let candidates = vec![
            LinkCandidate {
                hash: "aaaa1111".to_string(),
                source: ProvisionRef {
                    bill_dir: "a".to_string(),
                    provision_index: 0,
                    label: "Test".to_string(),
                },
                target: ProvisionRef {
                    bill_dir: "b".to_string(),
                    provision_index: 1,
                    label: "Test".to_string(),
                },
                similarity: 0.9,
                confidence: LinkConfidence::Verified,
                already_accepted: false,
                source_label: "A — Test".to_string(),
                target_label: "B — Test".to_string(),
                source_dollars: Some(100),
                target_dollars: Some(200),
            },
            LinkCandidate {
                hash: "bbbb2222".to_string(),
                source: ProvisionRef {
                    bill_dir: "c".to_string(),
                    provision_index: 0,
                    label: "Other".to_string(),
                },
                target: ProvisionRef {
                    bill_dir: "d".to_string(),
                    provision_index: 1,
                    label: "Other".to_string(),
                },
                similarity: 0.7,
                confidence: LinkConfidence::High,
                already_accepted: false,
                source_label: "C — Other".to_string(),
                target_label: "D — Other".to_string(),
                source_dollars: None,
                target_dollars: None,
            },
        ];

        // Accept only the first one by hash
        let count = accept_links(&mut lf, &candidates, &["aaaa1111"], None, false);
        assert_eq!(count, 1);
        assert_eq!(lf.accepted.len(), 1);
        assert_eq!(lf.accepted[0].hash, "aaaa1111");
    }

    #[test]
    fn test_accept_links_auto() {
        let mut lf = LinksFile::new("test");
        let candidates = vec![
            LinkCandidate {
                hash: "aaaa1111".to_string(),
                source: ProvisionRef {
                    bill_dir: "a".to_string(),
                    provision_index: 0,
                    label: "Test".to_string(),
                },
                target: ProvisionRef {
                    bill_dir: "b".to_string(),
                    provision_index: 1,
                    label: "Test".to_string(),
                },
                similarity: 0.9,
                confidence: LinkConfidence::Verified,
                already_accepted: false,
                source_label: "A".to_string(),
                target_label: "B".to_string(),
                source_dollars: None,
                target_dollars: None,
            },
            LinkCandidate {
                hash: "cccc3333".to_string(),
                source: ProvisionRef {
                    bill_dir: "e".to_string(),
                    provision_index: 0,
                    label: "Uncertain".to_string(),
                },
                target: ProvisionRef {
                    bill_dir: "f".to_string(),
                    provision_index: 1,
                    label: "Uncertain".to_string(),
                },
                similarity: 0.58,
                confidence: LinkConfidence::Uncertain,
                already_accepted: false,
                source_label: "E".to_string(),
                target_label: "F".to_string(),
                source_dollars: None,
                target_dollars: None,
            },
        ];

        // Auto: accepts Verified and High, skips Uncertain
        let count = accept_links(&mut lf, &candidates, &[], None, true);
        assert_eq!(count, 1);
        assert_eq!(lf.accepted[0].hash, "aaaa1111");
    }

    #[test]
    fn test_accept_skips_duplicates() {
        let mut lf = LinksFile::new("test");
        lf.accepted.push(AcceptedLink {
            hash: "aaaa1111".to_string(),
            source: ProvisionRef {
                bill_dir: "a".to_string(),
                provision_index: 0,
                label: String::new(),
            },
            target: ProvisionRef {
                bill_dir: "b".to_string(),
                provision_index: 1,
                label: String::new(),
            },
            similarity: 0.9,
            relationship: LinkRelationship::SameAccount,
            evidence: LinkEvidence::NameMatch,
            accepted_at: String::new(),
            note: None,
        });

        let candidates = vec![LinkCandidate {
            hash: "aaaa1111".to_string(),
            source: ProvisionRef {
                bill_dir: "a".to_string(),
                provision_index: 0,
                label: String::new(),
            },
            target: ProvisionRef {
                bill_dir: "b".to_string(),
                provision_index: 1,
                label: String::new(),
            },
            similarity: 0.9,
            confidence: LinkConfidence::Verified,
            already_accepted: true,
            source_label: String::new(),
            target_label: String::new(),
            source_dollars: None,
            target_dollars: None,
        }];

        // Should not add a duplicate
        let count = accept_links(&mut lf, &candidates, &["aaaa1111"], None, false);
        assert_eq!(count, 0);
        assert_eq!(lf.accepted.len(), 1);
    }
}
