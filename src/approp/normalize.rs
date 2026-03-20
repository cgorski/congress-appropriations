//! Entity resolution for cross-bill analysis.
//!
//! The `dataset.json` file at the data root stores user-managed entity
//! resolution rules — agency groups and account aliases that enable
//! consistent cross-bill matching. This module provides types, I/O,
//! normalization functions, and the `suggest-text-match` algorithm.
//!
//! `dataset.json` contains **only** knowledge that cannot be derived
//! from scanning per-bill files. No cached or derived data.

use crate::approp::bill_meta;
use crate::approp::loading::LoadedBill;
use crate::approp::ontology::{AmountSemantics, Provision};
use anyhow::{Context, Result};
use regex::Regex;
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

// ─── LLM Suggest Support ─────────────────────────────────────────────────────

/// A cluster of provisions for one account, ready to send to the LLM.
#[derive(Debug, Clone, Serialize)]
pub struct LlmCluster {
    /// The account name shared across all provisions in this cluster.
    pub account_name: String,
    /// Agency variants found for this account.
    pub agency_variants: Vec<String>,
    /// Individual provision appearances with context.
    pub provisions: Vec<LlmClusterEntry>,
}

/// One provision appearance within an LLM cluster.
#[derive(Debug, Clone, Serialize)]
pub struct LlmClusterEntry {
    pub bill_identifier: String,
    pub bill_dir: String,
    pub fiscal_years: Vec<u32>,
    pub agency: String,
    pub dollars: i64,
    pub xml_context: String,
}

/// Extract ~800 chars of XML context around a search string, preserving
/// appropriations heading structure as `[MAJOR]` and `[SUBHEADING]` markers.
pub fn get_xml_context(xml_content: &str, search_text: &str, window: usize) -> String {
    if search_text.is_empty() {
        return "(no search text)".to_string();
    }

    let pos = xml_content.find(search_text).or_else(|| {
        let lower_xml = xml_content.to_lowercase();
        let lower_search = search_text.to_lowercase();
        lower_xml.find(&lower_search)
    });

    let pos = match pos {
        Some(p) => p,
        None => return "(not found in XML)".to_string(),
    };

    let start = pos.saturating_sub(window);
    let end = (pos + 400).min(xml_content.len());
    let raw = &xml_content[start..end];

    // Preserve heading structure with markers
    let re_major_open = Regex::new(r"<appropriations-major[^>]*>").unwrap();
    let re_major_close = Regex::new(r"</appropriations-major>").unwrap();
    let re_inter_open = Regex::new(r"<appropriations-intermediate[^>]*>").unwrap();
    let re_inter_close = Regex::new(r"</appropriations-intermediate>").unwrap();
    let re_header_open = Regex::new(r"<header[^>]*>").unwrap();
    let re_header_close = Regex::new(r"</header>").unwrap();
    let re_any_tag = Regex::new(r"<[^>]+>").unwrap();
    let re_whitespace = Regex::new(r"[ \t]+").unwrap();
    let re_blank_lines = Regex::new(r"\n\s*\n").unwrap();

    let mut cleaned = raw.to_string();
    cleaned = re_major_open
        .replace_all(&cleaned, "\n[MAJOR] ")
        .to_string();
    cleaned = re_major_close
        .replace_all(&cleaned, " [/MAJOR]")
        .to_string();
    cleaned = re_inter_open
        .replace_all(&cleaned, "\n  [SUBHEADING] ")
        .to_string();
    cleaned = re_inter_close
        .replace_all(&cleaned, " [/SUBHEADING]")
        .to_string();
    cleaned = re_header_open.replace_all(&cleaned, "").to_string();
    cleaned = re_header_close.replace_all(&cleaned, "").to_string();
    cleaned = re_any_tag.replace_all(&cleaned, "").to_string();
    cleaned = re_whitespace.replace_all(&cleaned, " ").to_string();
    cleaned = re_blank_lines.replace_all(&cleaned, "\n").to_string();

    let lines: Vec<&str> = cleaned
        .trim()
        .split('\n')
        .filter(|l| !l.trim().is_empty())
        .collect();
    lines
        .iter()
        .rev()
        .take(8)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build LLM clusters from unresolved orphan pairs.
///
/// Takes the output of `suggest_text_match` and enriches each suggestion
/// with XML context from the bill files. Groups by department for efficient
/// batching.
pub fn build_llm_clusters(
    bills: &[LoadedBill],
    unresolved_pairs: &[SuggestedGroup],
) -> Vec<LlmCluster> {
    // Cache XML content per bill directory to avoid re-reading
    let mut xml_cache: HashMap<String, String> = HashMap::new();

    // Build provision lookup: (account_lower, agency) → Vec<provision info>
    struct ProvInfo {
        bill_id: String,
        bill_dir: String,
        fiscal_years: Vec<u32>,
        agency: String,
        account: String,
        dollars: i64,
        text_as_written: String,
    }

    let mut all_provs: Vec<ProvInfo> = Vec::new();
    for bill in bills {
        let bill_dir = bill
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let bill_id = bill.extraction.bill.identifier.clone();
        let fys = bill.extraction.bill.fiscal_years.clone();

        // Load XML if not cached
        if !xml_cache.contains_key(&bill_dir)
            && let Some(xml_path) = bill_meta::find_xml_in_dir(&bill.dir)
            && let Ok(content) = std::fs::read_to_string(&xml_path)
        {
            xml_cache.insert(bill_dir.clone(), content);
        }

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
                all_provs.push(ProvInfo {
                    bill_id: bill_id.clone(),
                    bill_dir: bill_dir.clone(),
                    fiscal_years: fys.clone(),
                    agency: agency.to_string(),
                    account: acct.to_string(),
                    dollars: amt.dollars().unwrap_or(0),
                    text_as_written: amt.text_as_written.clone(),
                });
            }
        }
    }

    let mut clusters: Vec<LlmCluster> = Vec::new();

    for suggestion in unresolved_pairs {
        let target_agencies: HashSet<String> = std::iter::once(suggestion.canonical.to_lowercase())
            .chain(suggestion.members.iter().map(|m| m.to_lowercase()))
            .collect();

        // Find all provisions under any of the target agencies for shared accounts
        let mut cluster_entries: Vec<LlmClusterEntry> = Vec::new();
        let mut seen_accounts: HashSet<String> = HashSet::new();

        for prov in &all_provs {
            if !target_agencies.contains(&prov.agency.to_lowercase()) {
                continue;
            }
            // Only include accounts from the suggestion's example list
            let acct_lower = prov.account.to_lowercase();
            if !suggestion.example_accounts.iter().any(|e| e == &acct_lower) {
                // Also include if any example account is a substring
                if !suggestion
                    .example_accounts
                    .iter()
                    .any(|e| acct_lower.contains(e) || e.contains(&acct_lower))
                {
                    continue;
                }
            }

            seen_accounts.insert(acct_lower);

            // Get XML context
            let xml_ctx = xml_cache
                .get(&prov.bill_dir)
                .map(|xml| {
                    let search = if !prov.text_as_written.is_empty() {
                        &prov.text_as_written
                    } else {
                        &prov.account
                    };
                    get_xml_context(xml, search, 800)
                })
                .unwrap_or_else(|| "(no XML available)".to_string());

            cluster_entries.push(LlmClusterEntry {
                bill_identifier: prov.bill_id.clone(),
                bill_dir: prov.bill_dir.clone(),
                fiscal_years: prov.fiscal_years.clone(),
                agency: prov.agency.clone(),
                dollars: prov.dollars,
                xml_context: xml_ctx,
            });
        }

        if cluster_entries.len() >= 2 {
            let agency_variants: Vec<String> = cluster_entries
                .iter()
                .map(|e| e.agency.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            clusters.push(LlmCluster {
                account_name: suggestion
                    .example_accounts
                    .first()
                    .cloned()
                    .unwrap_or_default(),
                agency_variants,
                provisions: cluster_entries,
            });
        }
    }

    clusters
}

/// The system prompt for LLM-based entity resolution.
pub const LLM_SYSTEM_PROMPT: &str = r#"You are an expert on U.S. federal government organizational structure and appropriations.

You will see CLUSTERS of provisions — each cluster groups appearances of an account across multiple appropriations bills from different fiscal years. Each appearance shows the agency name extracted by an LLM, the dollar amount, and the XML heading context from the enrolled bill.

Your task: For each cluster, determine which agency name variants refer to the SAME organizational entity for the purpose of year-over-year budget comparison.

Key considerations:
- The XML heading hierarchy ([MAJOR] and [SUBHEADING] markers) is ground truth from Congress
- Similar dollar amounts across fiscal years suggest the same budget line
- Sub-agencies within the same department may have SEPARATE budget lines — do not merge them unless the XML evidence shows they are the same line
- "Department of Defense—Army" and "Department of Defense—Department of the Army" are naming variants for the SAME entity
- But "Department of the Army" RDT&E and "Department of the Navy" RDT&E are DIFFERENT entities

Return JSON:
{"groups": [{"canonical": "preferred agency name", "members": ["variant1", "variant2"], "verdict": "SAME", "reasoning": "brief explanation citing XML/dollar evidence"}], "separate": [{"agency": "name", "verdict": "DIFFERENT", "reasoning": "brief"}]}

CRITICAL: When in doubt, say DIFFERENT. False merges silently corrupt budget totals. False orphans are visible and fixable."#;

/// Format clusters into a user prompt for the LLM.
pub fn format_llm_prompt(clusters: &[LlmCluster]) -> String {
    let mut parts = Vec::new();
    parts.push("Analyze these account clusters and classify agency pairs:\n".to_string());

    for (i, cluster) in clusters.iter().enumerate() {
        let sep = "=".repeat(20);
        parts.push(format!(
            "\n{sep} CLUSTER {}: \"{}\" ({} agencies) {sep}\n",
            i + 1,
            cluster.account_name,
            cluster.agency_variants.len(),
        ));

        for entry in &cluster.provisions {
            let dollars_str = {
                let s = entry.dollars.to_string();
                let mut result = String::new();
                for (i, c) in s.chars().rev().enumerate() {
                    if i > 0 && i % 3 == 0 {
                        result.insert(0, ',');
                    }
                    result.insert(0, c);
                }
                result
            };
            parts.push(format!(
                "  {} FY{:?}  agency=\"{}\"\n  ${dollars_str}\n  XML context:\n",
                entry.bill_identifier, entry.fiscal_years, entry.agency,
            ));
            for line in entry.xml_context.split('\n') {
                parts.push(format!("    {}\n", line));
            }
            parts.push("\n".to_string());
        }
    }

    parts.join("")
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
