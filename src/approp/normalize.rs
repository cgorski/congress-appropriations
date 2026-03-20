//! Entity resolution for cross-bill analysis.
//!
//! The `dataset.json` file at the data root stores user-managed entity
//! resolution rules — agency groups and account aliases that enable
//! consistent cross-bill matching. This module provides types, I/O,
//! normalization functions, and the `suggest-text-match` algorithm.
//!
//! `dataset.json` contains **only** knowledge that cannot be derived
//! from scanning per-bill files. No cached or derived data.

use crate::approp::loading::LoadedBill;
use crate::approp::ontology::{AmountSemantics, Provision};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

// ─── Types ───────────────────────────────────────────────────────────────────

/// Top-level dataset file stored at `<dir>/dataset.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetFile {
    pub schema_version: String,
    pub entities: Entities,
}

impl DatasetFile {
    /// Create a new empty dataset file.
    pub fn new() -> Self {
        Self {
            schema_version: "1.0".to_string(),
            entities: Entities {
                agency_groups: Vec::new(),
                account_aliases: Vec::new(),
            },
        }
    }
}

impl Default for DatasetFile {
    fn default() -> Self {
        Self::new()
    }
}

/// Entity resolution rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entities {
    /// Agency equivalence groups. Each group maps variant agency names
    /// to a canonical name for cross-bill matching.
    #[serde(default)]
    pub agency_groups: Vec<AgencyGroup>,
    /// Account name aliases. Each alias maps variant account names
    /// to a canonical name.
    #[serde(default)]
    pub account_aliases: Vec<AccountAlias>,
}

/// A group of agency names that should be treated as equivalent
/// for the purpose of cross-bill comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgencyGroup {
    /// The preferred agency name shown in output.
    pub canonical: String,
    /// Variant names treated as equivalent to the canonical name.
    pub members: Vec<String>,
}

/// A group of account names that should be treated as equivalent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountAlias {
    /// The preferred account name.
    pub canonical: String,
    /// Alternative spellings or phrasings.
    pub aliases: Vec<String>,
}

/// A suggestion produced by `suggest-text-match`.
#[derive(Debug, Clone, Serialize)]
pub struct SuggestedGroup {
    /// The proposed canonical name.
    pub canonical: String,
    /// The variant that should be grouped with the canonical.
    pub members: Vec<String>,
    /// How this suggestion was discovered.
    pub evidence: SuggestionEvidence,
    /// Example account names that appear under both agency variants.
    pub example_accounts: Vec<String>,
    /// Number of orphan pairs this group would resolve.
    pub orphan_pairs_resolved: usize,
}

/// How a normalization suggestion was discovered.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SuggestionEvidence {
    /// Both agencies have provisions with the same account name that appear
    /// as orphans (one in base, one in current) in cross-FY comparison.
    OrphanPair,
    /// Structural regex pattern detected a naming variant.
    RegexPattern { pattern: String },
}

// ─── I/O ─────────────────────────────────────────────────────────────────────

/// Load `dataset.json` from the data root directory. Returns `None` if not present.
pub fn load_dataset(dir: &Path) -> Result<Option<DatasetFile>> {
    let path = dir.join("dataset.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let dataset: DatasetFile = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(dataset))
}

/// Save `dataset.json` to the data root directory.
pub fn save_dataset(dir: &Path, dataset: &DatasetFile) -> Result<()> {
    let path = dir.join("dataset.json");
    let json = serde_json::to_string_pretty(dataset)?;
    std::fs::write(&path, json).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

// ─── Normalization Functions ─────────────────────────────────────────────────

/// Normalize an agency name using explicit groups from `dataset.json`.
///
/// Returns `(normalized_name, was_normalized)`. If the agency matches a
/// canonical name or any member of a group, returns the canonical name
/// and `true`. Otherwise returns the lowercased agency name and `false`.
///
/// **No implicit normalization.** If the agency is not in any group,
/// it passes through unchanged (lowercased only).
pub fn normalize_agency(agency: &str, groups: &[AgencyGroup]) -> (String, bool) {
    let lower = agency.to_lowercase().trim().to_string();
    if lower.is_empty() {
        return (lower, false);
    }

    for group in groups {
        if group.canonical.to_lowercase() == lower {
            return (group.canonical.to_lowercase(), true);
        }
        for member in &group.members {
            if member.to_lowercase() == lower {
                return (group.canonical.to_lowercase(), true);
            }
        }
    }

    (lower, false)
}

/// Normalize an account name using explicit aliases from `dataset.json`,
/// plus standard lowercasing and em-dash prefix stripping.
///
/// Returns `(normalized_name, was_alias_matched)`.
pub fn normalize_account(account: &str, aliases: &[AccountAlias]) -> (String, bool) {
    let lower = account.to_lowercase();
    // Strip em-dash and en-dash prefixes (e.g., "Department of VA—Account" → "account")
    let stripped = {
        let parts: Vec<&str> = lower.split(&['\u{2014}', '\u{2013}'][..]).collect();
        if parts.len() > 1 {
            parts.last().unwrap_or(&"").trim().to_string()
        } else {
            lower.trim().to_string()
        }
    };

    // Check aliases
    for alias in aliases {
        if alias.canonical.to_lowercase() == stripped {
            return (stripped, false); // Already canonical
        }
        for alt in &alias.aliases {
            if alt.to_lowercase() == stripped {
                return (alias.canonical.to_lowercase(), true);
            }
        }
    }

    (stripped, false)
}

// ─── Suggest Text Match ──────────────────────────────────────────────────────

/// Discover agency normalization suggestions by analyzing orphan pairs
/// in cross-FY comparison and applying structural regex patterns.
///
/// This is the algorithm behind `normalize suggest-text-match`:
/// 1. For each pair of fiscal years, build account maps and find orphans
/// 2. Look for orphan pairs — same account name, different sides, different agency
/// 3. Apply regex patterns for common naming variants
/// 4. Group suggestions by agency pair and return
///
/// No API calls. No embeddings. Pure string analysis.
pub fn suggest_text_match(bills: &[LoadedBill]) -> Vec<SuggestedGroup> {
    // Collect all top-level BA provisions by (fiscal_year, account_name_lower, agency)
    struct ProvEntry {
        agency: String,
        account: String,
        _dollars: i64,
        fiscal_years: Vec<u32>,
    }

    let mut entries: Vec<ProvEntry> = Vec::new();

    for bill in bills {
        let fys = &bill.extraction.bill.fiscal_years;
        for p in &bill.extraction.provisions {
            if !matches!(p, Provision::Appropriation { .. }) {
                continue;
            }
            if let Some(amt) = p.amount() {
                if !matches!(amt.semantics, AmountSemantics::NewBudgetAuthority) {
                    continue;
                }
                let dl = p.detail_level();
                if dl == "sub_allocation" || dl == "proviso_amount" {
                    continue;
                }
                let acct = p.account_name();
                let agency = p.agency();
                if acct.is_empty() || agency.is_empty() {
                    continue;
                }
                entries.push(ProvEntry {
                    agency: agency.to_string(),
                    account: acct.to_lowercase().trim().to_string(),
                    _dollars: amt.dollars().unwrap_or(0),
                    fiscal_years: fys.clone(),
                });
            }
        }
    }

    // Group by account name → list of (agency, fiscal_years, dollars)
    let mut by_account: HashMap<String, Vec<&ProvEntry>> = HashMap::new();
    for entry in &entries {
        by_account
            .entry(entry.account.clone())
            .or_default()
            .push(entry);
    }

    // Find accounts where:
    // - The same account name appears in different fiscal years
    // - Under different agency names
    // These are orphan-pair candidates
    let mut agency_pair_evidence: HashMap<(String, String), Vec<String>> = HashMap::new();

    for (account, provs) in &by_account {
        // Collect unique agencies for this account
        let agencies: HashSet<&str> = provs.iter().map(|p| p.agency.as_str()).collect();
        if agencies.len() < 2 {
            continue;
        }

        // Check if different agencies appear in different fiscal years
        // (which is the orphan-pair pattern)
        let mut agency_fys: HashMap<&str, HashSet<u32>> = HashMap::new();
        for p in provs {
            for fy in &p.fiscal_years {
                agency_fys.entry(p.agency.as_str()).or_default().insert(*fy);
            }
        }

        // For each pair of agencies, check if they appear in different FYs
        let agency_list: Vec<&str> = agencies.into_iter().collect();
        for i in 0..agency_list.len() {
            for j in (i + 1)..agency_list.len() {
                let a = agency_list[i];
                let b = agency_list[j];
                let fys_a = &agency_fys[a];
                let fys_b = &agency_fys[b];

                // If they appear in different fiscal years, this is a cross-FY orphan pair
                let a_only = fys_a.difference(fys_b).count();
                let b_only = fys_b.difference(fys_a).count();
                if a_only > 0 || b_only > 0 {
                    let key = if a < b {
                        (a.to_string(), b.to_string())
                    } else {
                        (b.to_string(), a.to_string())
                    };
                    agency_pair_evidence
                        .entry(key)
                        .or_default()
                        .push(account.clone());
                }
            }
        }
    }

    // Also apply regex patterns for common naming variants
    let unique_agencies: Vec<&str> = entries
        .iter()
        .map(|e| e.agency.as_str())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let regex_variants = find_agency_regex_variants(&unique_agencies);
    for (a, b, pattern) in &regex_variants {
        let key = if a < b {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        };
        // Find accounts that appear under both agencies
        let accounts_a: HashSet<&String> = by_account
            .iter()
            .filter(|(_, provs)| provs.iter().any(|p| p.agency == *a))
            .map(|(acct, _)| acct)
            .collect();
        let accounts_b: HashSet<&String> = by_account
            .iter()
            .filter(|(_, provs)| provs.iter().any(|p| p.agency == *b))
            .map(|(acct, _)| acct)
            .collect();
        let shared: Vec<&String> = accounts_a.intersection(&accounts_b).copied().collect();
        for acct in shared {
            agency_pair_evidence
                .entry(key.clone())
                .or_default()
                .push(format!("{acct} [regex: {pattern}]"));
        }
    }

    // Convert to SuggestedGroup — one per agency PAIR (no transitive closure).
    //
    // Transitive closure would merge all agencies that share any account name,
    // which creates mega-groups through generic names like "Salaries and Expenses."
    // Instead, each pair is a separate suggestion. The user decides which to accept.
    let mut suggestions: Vec<SuggestedGroup> = Vec::new();

    for ((a, b), accounts) in &agency_pair_evidence {
        // Skip pairs connected only through very generic account names
        // (accounts that appear under 5+ different agencies are too generic
        // to be evidence of agency equivalence)
        let non_generic_accounts: Vec<&String> = accounts
            .iter()
            .filter(|acct| {
                let clean = if let Some(pos) = acct.find(" [regex:") {
                    &acct[..pos]
                } else {
                    acct.as_str()
                };
                // Count how many distinct agencies have this account
                let agency_count = by_account
                    .get(clean)
                    .map(|provs| {
                        provs
                            .iter()
                            .map(|p| p.agency.as_str())
                            .collect::<HashSet<_>>()
                            .len()
                    })
                    .unwrap_or(0);
                agency_count < 5
            })
            .collect();

        if non_generic_accounts.is_empty() {
            continue;
        }

        // Pick canonical: prefer the longer name (usually more specific)
        let (canonical, member) = if a.len() >= b.len() {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        };

        // Determine evidence type
        let has_regex = accounts.iter().any(|acct| acct.contains("[regex:"));
        let evidence = if has_regex {
            SuggestionEvidence::RegexPattern {
                pattern: "mixed (orphan-pair + regex)".to_string(),
            }
        } else {
            SuggestionEvidence::OrphanPair
        };

        // Clean account names for display
        let example_accounts: Vec<String> = non_generic_accounts
            .iter()
            .map(|a| {
                if let Some(pos) = a.find(" [regex:") {
                    a[..pos].to_string()
                } else {
                    (*a).clone()
                }
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .take(5)
            .collect();

        suggestions.push(SuggestedGroup {
            canonical,
            members: vec![member],
            evidence,
            example_accounts,
            orphan_pairs_resolved: non_generic_accounts.len(),
        });
    }

    // Sort by orphan pairs resolved (most impactful first)
    suggestions.sort_by(|a, b| b.orphan_pairs_resolved.cmp(&a.orphan_pairs_resolved));

    suggestions
}

/// Find agency naming variants using structural regex patterns.
///
/// Patterns detected:
/// - "Department of X—Y" ↔ "Department of X—Department of Y"
///   (prefix expansion)
/// - "X" ↔ "Department of X"
///   (department prefix addition)
/// - "United States X" ↔ "U.S. X"
///   (abbreviation)
/// - "Office of X" ↔ "Office for X"
///   (preposition variant)
///
/// Scan unique agency names for regex-matchable variant pairs.
///
/// Returns `(agency_a, agency_b, pattern_description)` for each detected pair.
pub fn find_agency_regex_variants(agencies: &[&str]) -> Vec<(String, String, String)> {
    let mut variants: Vec<(String, String, String)> = Vec::new();

    for i in 0..agencies.len() {
        for j in (i + 1)..agencies.len() {
            let a = agencies[i];
            let b = agencies[j];
            let al = a.to_lowercase();
            let bl = b.to_lowercase();

            // Pattern 1: "Department of X—Y" ↔ "Department of X—Department of Y"
            if let Some(pattern) = check_prefix_expansion(&al, &bl) {
                variants.push((a.to_string(), b.to_string(), pattern));
                continue;
            }

            // Pattern 2: "Office of X" ↔ "Office for X"
            if check_preposition_variant(&al, &bl) {
                variants.push((
                    a.to_string(),
                    b.to_string(),
                    "preposition variant (of/for)".to_string(),
                ));
                continue;
            }

            // Pattern 3: "United States X" ↔ "U.S. X"
            if check_us_abbreviation(&al, &bl) {
                variants.push((a.to_string(), b.to_string(), "US abbreviation".to_string()));
            }
        }
    }

    variants
}

fn check_prefix_expansion(a: &str, b: &str) -> Option<String> {
    // "department of defense—army" ↔ "department of defense—department of the army"
    if let (Some(a_pos), Some(b_pos)) = (a.find('\u{2014}'), b.find('\u{2014}')) {
        let a_prefix = &a[..a_pos];
        let b_prefix = &b[..b_pos];
        let a_suffix = a[a_pos + 3..].trim(); // skip em-dash (3 bytes UTF-8)
        let b_suffix = b[b_pos + 3..].trim();

        if a_prefix == b_prefix {
            // Same prefix, different suffix — check if one is "department of X" of the other
            if b_suffix.starts_with("department of the ") && b_suffix.ends_with(a_suffix) {
                return Some(format!("prefix expansion: {a_suffix} ↔ {b_suffix}"));
            }
            if a_suffix.starts_with("department of the ") && a_suffix.ends_with(b_suffix) {
                return Some(format!("prefix expansion: {b_suffix} ↔ {a_suffix}"));
            }
            if b_suffix.starts_with("department of ") && b_suffix.ends_with(a_suffix) {
                return Some(format!("prefix expansion: {a_suffix} ↔ {b_suffix}"));
            }
            if a_suffix.starts_with("department of ") && a_suffix.ends_with(b_suffix) {
                return Some(format!("prefix expansion: {b_suffix} ↔ {a_suffix}"));
            }
        }
    }
    None
}

fn check_preposition_variant(a: &str, b: &str) -> bool {
    // "office of civil rights" ↔ "office for civil rights"
    let a_normalized = a.replace(" of ", " for ");
    let b_normalized = b.replace(" of ", " for ");
    a != b && a_normalized == b_normalized
}

fn check_us_abbreviation(a: &str, b: &str) -> bool {
    // "united states X" ↔ "u.s. X"
    let a_expanded = a
        .replace("u.s. ", "united states ")
        .replace("u.s.", "united states");
    let b_expanded = b
        .replace("u.s. ", "united states ")
        .replace("u.s.", "united states");
    a != b && a_expanded == b_expanded
}

// ─── Merge Logic ─────────────────────────────────────────────────────────────

/// Merge new agency groups into an existing dataset file.
/// Deduplicates members and avoids creating conflicting groups.
pub fn merge_groups(dataset: &mut DatasetFile, new_groups: &[SuggestedGroup]) {
    for suggestion in new_groups {
        // Check if any existing group already contains the canonical or a member
        let mut found_existing = false;
        for existing in &mut dataset.entities.agency_groups {
            let existing_lower: HashSet<String> =
                std::iter::once(existing.canonical.to_lowercase())
                    .chain(existing.members.iter().map(|m| m.to_lowercase()))
                    .collect();

            let new_lower: HashSet<String> = std::iter::once(suggestion.canonical.to_lowercase())
                .chain(suggestion.members.iter().map(|m| m.to_lowercase()))
                .collect();

            if existing_lower.intersection(&new_lower).count() > 0 {
                // Merge into existing group
                for member in &suggestion.members {
                    let member_lower = member.to_lowercase();
                    if member_lower != existing.canonical.to_lowercase()
                        && !existing
                            .members
                            .iter()
                            .any(|m| m.to_lowercase() == member_lower)
                    {
                        existing.members.push(member.clone());
                    }
                }
                // Also add the suggestion's canonical as a member if different
                let canon_lower = suggestion.canonical.to_lowercase();
                if canon_lower != existing.canonical.to_lowercase()
                    && !existing
                        .members
                        .iter()
                        .any(|m| m.to_lowercase() == canon_lower)
                {
                    existing.members.push(suggestion.canonical.clone());
                }
                found_existing = true;
                break;
            }
        }

        if !found_existing {
            dataset.entities.agency_groups.push(AgencyGroup {
                canonical: suggestion.canonical.clone(),
                members: suggestion.members.clone(),
            });
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_agency_with_group() {
        let groups = vec![AgencyGroup {
            canonical: "Department of Defense".to_string(),
            members: vec![
                "Department of the Army".to_string(),
                "Department of Defense—Army".to_string(),
            ],
        }];

        let (name, normalized) = normalize_agency("Department of the Army", &groups);
        assert_eq!(name, "department of defense");
        assert!(normalized);

        let (name, normalized) = normalize_agency("Department of Defense", &groups);
        assert_eq!(name, "department of defense");
        assert!(normalized);

        let (name, normalized) = normalize_agency("Department of Defense—Army", &groups);
        assert_eq!(name, "department of defense");
        assert!(normalized);
    }

    #[test]
    fn test_normalize_agency_no_group() {
        let groups = vec![AgencyGroup {
            canonical: "Department of Defense".to_string(),
            members: vec!["Department of the Army".to_string()],
        }];

        let (name, normalized) = normalize_agency("Environmental Protection Agency", &groups);
        assert_eq!(name, "environmental protection agency");
        assert!(!normalized);
    }

    #[test]
    fn test_normalize_agency_empty_groups() {
        let (name, normalized) = normalize_agency("Department of Defense", &[]);
        assert_eq!(name, "department of defense");
        assert!(!normalized);
    }

    #[test]
    fn test_normalize_account_with_alias() {
        let aliases = vec![AccountAlias {
            canonical: "Office for Civil Rights".to_string(),
            aliases: vec!["Office of Civil Rights".to_string()],
        }];

        let (name, was_alias) = normalize_account("Office of Civil Rights", &aliases);
        assert_eq!(name, "office for civil rights");
        assert!(was_alias);

        let (name, was_alias) = normalize_account("Office for Civil Rights", &aliases);
        assert_eq!(name, "office for civil rights");
        assert!(!was_alias); // Already canonical
    }

    #[test]
    fn test_normalize_account_em_dash_stripping() {
        let (name, _) = normalize_account("Department of VA\u{2014}Compensation and Pensions", &[]);
        assert_eq!(name, "compensation and pensions");
    }

    #[test]
    fn test_dataset_file_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut dataset = DatasetFile::new();
        dataset.entities.agency_groups.push(AgencyGroup {
            canonical: "Department of Defense".to_string(),
            members: vec!["Department of the Army".to_string()],
        });
        dataset.entities.account_aliases.push(AccountAlias {
            canonical: "Office for Civil Rights".to_string(),
            aliases: vec!["Office of Civil Rights".to_string()],
        });

        save_dataset(dir.path(), &dataset).unwrap();
        let loaded = load_dataset(dir.path()).unwrap().unwrap();

        assert_eq!(loaded.entities.agency_groups.len(), 1);
        assert_eq!(
            loaded.entities.agency_groups[0].canonical,
            "Department of Defense"
        );
        assert_eq!(loaded.entities.agency_groups[0].members.len(), 1);
        assert_eq!(loaded.entities.account_aliases.len(), 1);
    }

    #[test]
    fn test_load_missing_returns_none() {
        let dir = tempfile::TempDir::new().unwrap();
        let result = load_dataset(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_groups_new() {
        let mut dataset = DatasetFile::new();
        let suggestions = vec![SuggestedGroup {
            canonical: "Department of Defense".to_string(),
            members: vec!["Department of the Army".to_string()],
            evidence: SuggestionEvidence::OrphanPair,
            example_accounts: vec!["RDT&E, Army".to_string()],
            orphan_pairs_resolved: 3,
        }];

        merge_groups(&mut dataset, &suggestions);
        assert_eq!(dataset.entities.agency_groups.len(), 1);
        assert_eq!(dataset.entities.agency_groups[0].members.len(), 1);
    }

    #[test]
    fn test_merge_groups_into_existing() {
        let mut dataset = DatasetFile::new();
        dataset.entities.agency_groups.push(AgencyGroup {
            canonical: "Department of Defense".to_string(),
            members: vec!["Department of the Army".to_string()],
        });

        let suggestions = vec![SuggestedGroup {
            canonical: "Department of Defense".to_string(),
            members: vec!["Department of the Navy".to_string()],
            evidence: SuggestionEvidence::OrphanPair,
            example_accounts: vec![],
            orphan_pairs_resolved: 2,
        }];

        merge_groups(&mut dataset, &suggestions);
        assert_eq!(dataset.entities.agency_groups.len(), 1);
        assert_eq!(dataset.entities.agency_groups[0].members.len(), 2);
        assert!(
            dataset.entities.agency_groups[0]
                .members
                .contains(&"Department of the Navy".to_string())
        );
    }

    #[test]
    fn test_check_preposition_variant() {
        assert!(check_preposition_variant(
            "office of civil rights",
            "office for civil rights"
        ));
        assert!(!check_preposition_variant(
            "office of civil rights",
            "office of civil rights"
        ));
        assert!(!check_preposition_variant(
            "department of defense",
            "department of state"
        ));
    }

    #[test]
    fn test_check_us_abbreviation() {
        assert!(check_us_abbreviation(
            "united states fish and wildlife service",
            "u.s. fish and wildlife service"
        ));
        assert!(!check_us_abbreviation(
            "united states army",
            "united states navy"
        ));
    }

    #[test]
    fn test_check_prefix_expansion() {
        let result = check_prefix_expansion(
            "department of defense\u{2014}army",
            "department of defense\u{2014}department of the army",
        );
        assert!(result.is_some());

        let result = check_prefix_expansion(
            "department of defense\u{2014}navy",
            "department of defense\u{2014}air force",
        );
        assert!(result.is_none());
    }
}
