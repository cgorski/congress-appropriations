//! Account authority registry — the cross-bill "database" of federal budget accounts.
//!
//! Aggregates all `tas_mapping.json` files into a single `authorities.json` at the
//! data root. Each authority represents one federal budget account identified by its
//! Federal Account Symbol (FAS) code, with references to every provision instance
//! across all processed bills.
//!
//! # Pipeline position
//!
//! ```text
//! extract → verify-text → enrich → resolve-tas → **authority build** → query
//! ```
//!
//! # Usage
//!
//! ```text
//! congress-approp authority build --dir data
//! congress-approp authority list --dir data
//! congress-approp trace 070-0400 --dir data
//! ```

use crate::approp::tas::{self, FasReference, TasConfidence, TasMappingFile, TasMethod};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

// ─── Types ───────────────────────────────────────────────────────────────────

/// The complete authority registry — stored at `<dir>/authorities.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityRegistry {
    pub schema_version: String,
    pub generated_at: String,
    /// SHA-256 of `fas_reference.json` used during this build.
    pub fas_reference_hash: String,
    pub authorities: Vec<AccountAuthority>,
    pub summary: RegistrySummary,
}

/// Summary statistics for the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySummary {
    pub total_authorities: usize,
    pub total_provisions: usize,
    pub bills_included: usize,
    pub fiscal_years_covered: Vec<u32>,
    pub authorities_with_name_variants: usize,
    pub authorities_in_multiple_bills: usize,
    /// Number of detected lifecycle events (renames, etc.) across all authorities.
    #[serde(default)]
    pub total_events: usize,
}

/// A single budget account authority — one per FAS code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountAuthority {
    /// Primary identifier — the Federal Account Symbol (e.g., `"070-0400"`).
    pub fas_code: String,
    /// CGAC agency code (e.g., `"070"`).
    pub agency_code: String,
    /// Official title from the FAST Book (if available).
    pub fas_title: String,
    /// Agency name from the FAST Book (if available).
    pub agency_name: String,
    /// All name variants observed across bills for this account.
    pub name_variants: Vec<NameVariant>,
    /// Every provision instance of this account across all bills.
    pub provisions: Vec<AuthorityProvisionRef>,
    /// Number of distinct bills this account appears in.
    pub bill_count: usize,
    /// Fiscal years this account has been seen in.
    pub fiscal_years: Vec<u32>,
    /// Total budget authority across all provisions (may double-count across FYs).
    pub total_dollars: i64,
    /// Detected lifecycle events (renames, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AuthorityEvent>,
}

/// A name variant observed for an authority across bills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameVariant {
    /// The account name as extracted by the LLM.
    pub name: String,
    /// Bill directories where this name was used.
    pub bills: Vec<String>,
    /// Classification of this variant relative to the others.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<VariantClassification>,
    /// Fiscal years where this name was observed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fiscal_years: Vec<u32>,
}

/// How a name variant relates to the authority's canonical name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantClassification {
    /// The canonical/primary name for this account.
    Canonical,
    /// Differs only in capitalization (e.g., "army" vs "Army").
    CaseVariant,
    /// The LLM included or omitted an agency em-dash prefix
    /// (e.g., "USSS—Operations and Support" vs "Operations and Support").
    PrefixVariant,
    /// A genuine name change — Congress renamed this account.
    NameChange,
    /// The LLM used an inconsistent name, but there's no clear temporal boundary.
    InconsistentExtraction,
}

/// A detected event in an authority's lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityEvent {
    /// The fiscal year when this event was first observed.
    pub fiscal_year: u32,
    /// What kind of event occurred.
    pub event_type: AuthorityEventType,
}

/// Types of authority lifecycle events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthorityEventType {
    /// Congress renamed this account.
    Rename { from: String, to: String },
}

/// Reference to a specific provision in a specific bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityProvisionRef {
    pub bill_dir: String,
    pub bill_identifier: String,
    pub provision_index: usize,
    pub fiscal_years: Vec<u32>,
    pub dollars: Option<i64>,
    /// The account name as extracted (may differ from the FAS title).
    pub account_name: String,
    /// How this mapping was established.
    pub confidence: TasConfidence,
    pub method: TasMethod,
}

/// A single entry in a fiscal-year timeline for an authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub fiscal_year: u32,
    pub dollars: i64,
    /// Bills contributing to this FY's total.
    pub bills: Vec<String>,
    /// Account names used in each contributing bill.
    pub account_names: Vec<String>,
}

// ─── Build ───────────────────────────────────────────────────────────────────

/// Build the authority registry by scanning all `tas_mapping.json` files
/// under `dir` and grouping provisions by FAS code.
///
/// For each FAS code, looks up the official title and agency from the
/// provided `fas_reference`. Collects name variants (distinct account names
/// the LLM extracted across different bills) and provision references.
pub fn build_authorities(dir: &Path, fas_reference: &FasReference) -> Result<AuthorityRegistry> {
    // Discover all TAS mapping files
    let mut mapping_files: Vec<(String, TasMappingFile)> = Vec::new();
    let mut bill_fiscal_years: HashMap<String, Vec<u32>> = HashMap::new();
    let mut bill_identifiers: HashMap<String, String> = HashMap::new();

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

    let mut dirs: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    dirs.sort_by_key(|e| e.file_name());

    for entry in dirs {
        let bill_dir = entry.path();
        if !bill_dir.is_dir() {
            continue;
        }
        let bill_dir_name = bill_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Load TAS mapping
        let tm_path = bill_dir.join("tas_mapping.json");
        if !tm_path.exists() {
            continue;
        }
        let tm: TasMappingFile = match tas::load_tas_mapping(&bill_dir)? {
            Some(tm) => tm,
            None => continue,
        };

        // Load fiscal years from extraction.json
        let ext_path = bill_dir.join("extraction.json");
        if ext_path.exists()
            && let Ok(ext_text) = std::fs::read_to_string(&ext_path)
            && let Ok(ext_val) = serde_json::from_str::<serde_json::Value>(&ext_text)
        {
            let fys: Vec<u32> = ext_val
                .get("bill")
                .and_then(|b| b.get("fiscal_years"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u32))
                        .collect()
                })
                .unwrap_or_default();
            bill_fiscal_years.insert(bill_dir_name.clone(), fys);

            let ident = ext_val
                .get("bill")
                .and_then(|b| b.get("identifier"))
                .and_then(|v| v.as_str())
                .unwrap_or(&bill_dir_name)
                .to_string();
            bill_identifiers.insert(bill_dir_name.clone(), ident);
        }

        mapping_files.push((bill_dir_name, tm));
    }

    // Group provisions by FAS code
    let mut by_fas: HashMap<String, Vec<AuthorityProvisionRef>> = HashMap::new();
    let mut all_bills: BTreeSet<String> = BTreeSet::new();
    let mut all_fys: BTreeSet<u32> = BTreeSet::new();

    for (bill_dir_name, tm) in &mapping_files {
        all_bills.insert(bill_dir_name.clone());
        let fys = bill_fiscal_years
            .get(bill_dir_name)
            .cloned()
            .unwrap_or_default();
        for fy in &fys {
            all_fys.insert(*fy);
        }

        let bill_id = bill_identifiers
            .get(bill_dir_name)
            .cloned()
            .unwrap_or_else(|| bill_dir_name.clone());

        for mapping in &tm.mappings {
            let Some(ref fas_code) = mapping.fas_code else {
                continue;
            };

            by_fas
                .entry(fas_code.clone())
                .or_default()
                .push(AuthorityProvisionRef {
                    bill_dir: bill_dir_name.clone(),
                    bill_identifier: bill_id.clone(),
                    provision_index: mapping.provision_index,
                    fiscal_years: fys.clone(),
                    dollars: mapping.dollars,
                    account_name: mapping.account_name.clone(),
                    confidence: mapping.confidence,
                    method: mapping.method,
                });
        }
    }

    // Build authorities
    let mut authorities: Vec<AccountAuthority> = Vec::new();
    let mut total_provisions = 0usize;

    for (fas_code, provisions) in &by_fas {
        total_provisions += provisions.len();

        // Look up official title from FAS reference
        let (fas_title, agency_name) = fas_reference
            .get_by_code(fas_code)
            .map(|a| (a.title.clone(), a.agency_name.clone()))
            .unwrap_or_else(|| {
                // Use the first provision's account name as fallback
                let fallback = provisions
                    .first()
                    .map(|p| p.account_name.clone())
                    .unwrap_or_default();
                (fallback, String::new())
            });

        let agency_code = fas_code.split('-').next().unwrap_or("").to_string();

        // Collect name variants
        let name_variants = collect_name_variants(provisions);

        // Distinct bills and fiscal years
        let bill_dirs: BTreeSet<&str> = provisions.iter().map(|p| p.bill_dir.as_str()).collect();
        let mut seen_fys: BTreeSet<u32> = BTreeSet::new();
        for p in provisions {
            for fy in &p.fiscal_years {
                seen_fys.insert(*fy);
            }
        }

        let total_dollars: i64 = provisions.iter().filter_map(|p| p.dollars).sum();

        // Classify name variants and detect events
        let (classified_variants, events) =
            classify_variants_and_detect_events(&name_variants, &bill_fiscal_years);

        authorities.push(AccountAuthority {
            fas_code: fas_code.clone(),
            agency_code,
            fas_title,
            agency_name,
            name_variants: classified_variants,
            provisions: provisions.clone(),
            bill_count: bill_dirs.len(),
            fiscal_years: seen_fys.into_iter().collect(),
            total_dollars,
            events,
        });
    }

    // Sort by FAS code for deterministic output
    authorities.sort_by(|a, b| a.fas_code.cmp(&b.fas_code));

    let total_events: usize = authorities.iter().map(|a| a.events.len()).sum();

    // Compute summary
    let authorities_with_name_variants = authorities
        .iter()
        .filter(|a| a.name_variants.len() > 1)
        .count();
    let authorities_in_multiple_bills = authorities.iter().filter(|a| a.bill_count > 1).count();

    let ref_hash = tas::fas_reference_hash(&dir.join("fas_reference.json"))
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(AuthorityRegistry {
        schema_version: "1.0".to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        fas_reference_hash: ref_hash,
        summary: RegistrySummary {
            total_authorities: authorities.len(),
            total_provisions,
            bills_included: all_bills.len(),
            fiscal_years_covered: all_fys.into_iter().collect(),
            authorities_with_name_variants,
            authorities_in_multiple_bills,
            total_events,
        },
        authorities,
    })
}

/// Collect distinct name variants for one authority's provisions.
fn collect_name_variants(provisions: &[AuthorityProvisionRef]) -> Vec<NameVariant> {
    let mut by_name: HashMap<String, (BTreeSet<String>, BTreeSet<u32>)> = HashMap::new();
    for p in provisions {
        let entry = by_name.entry(p.account_name.clone()).or_default();
        entry.0.insert(p.bill_dir.clone());
        for fy in &p.fiscal_years {
            entry.1.insert(*fy);
        }
    }

    let mut variants: Vec<NameVariant> = by_name
        .into_iter()
        .map(|(name, (bills, fys))| NameVariant {
            name,
            bills: bills.into_iter().collect(),
            classification: None, // classified later
            fiscal_years: fys.into_iter().collect(),
        })
        .collect();

    // Sort by name for deterministic output
    variants.sort_by(|a, b| a.name.cmp(&b.name));
    variants
}

/// Normalize a name for comparison: lowercase, strip em-dash prefix, trim.
fn normalize_variant_name(name: &str) -> String {
    let lower = name.to_lowercase();
    // Strip em-dash prefix (e.g., "USSS—Operations" → "operations")
    let parts: Vec<&str> = lower
        .split(&['\u{2014}', '\u{2013}', '—', '–'][..])
        .collect();
    let stripped = if parts.len() > 1 {
        parts.last().unwrap_or(&"").trim()
    } else {
        lower.trim()
    };
    stripped.to_string()
}

/// Classify name variants and detect lifecycle events (renames).
///
/// For each authority, examines how the account name changes across fiscal years:
/// - **Case variant**: same name after lowercasing
/// - **Prefix variant**: same name after stripping em-dash agency prefix
/// - **Name change**: meaningfully different name with a clear temporal boundary
/// - **Inconsistent extraction**: different names with no clear pattern
///
/// Returns classified variants and any detected events.
fn classify_variants_and_detect_events(
    variants: &[NameVariant],
    bill_fiscal_years: &HashMap<String, Vec<u32>>,
) -> (Vec<NameVariant>, Vec<AuthorityEvent>) {
    if variants.len() <= 1 {
        // Single variant — it's the canonical name
        let mut classified = variants.to_vec();
        if let Some(v) = classified.first_mut() {
            v.classification = Some(VariantClassification::Canonical);
        }
        return (classified, Vec::new());
    }

    let mut classified = variants.to_vec();
    let mut events = Vec::new();

    // Step 1: Collect normalized names for comparison
    let normalized: Vec<String> = classified
        .iter()
        .map(|v| normalize_variant_name(&v.name))
        .collect();
    let unique_normalized: BTreeSet<&str> = normalized.iter().map(|s| s.as_str()).collect();

    // Step 2: If all normalized names are the same, classify as case/prefix variants
    if unique_normalized.len() == 1 {
        // All variants are the same after normalization
        // Pick the most common as canonical
        let longest_idx = classified
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| v.bills.len())
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Extract canonical name before mutable iteration (avoids borrow conflict)
        let canon_lower_early = classified[longest_idx].name.to_lowercase();

        for (i, v) in classified.iter_mut().enumerate() {
            if i == longest_idx {
                v.classification = Some(VariantClassification::Canonical);
            } else {
                let lower_match = v.name.to_lowercase() == canon_lower_early;
                if lower_match {
                    v.classification = Some(VariantClassification::CaseVariant);
                } else {
                    v.classification = Some(VariantClassification::PrefixVariant);
                }
            }
        }
        return (classified, events);
    }

    // Step 3: Multiple distinct normalized names — look for temporal boundaries
    // Group fiscal years by normalized name
    let mut fy_by_norm_name: HashMap<String, BTreeSet<u32>> = HashMap::new();
    for v in classified.iter() {
        let norm = normalize_variant_name(&v.name);
        let entry = fy_by_norm_name.entry(norm).or_default();
        // Get fiscal years from the variant's bills
        for bill_dir in &v.bills {
            if let Some(fys) = bill_fiscal_years.get(bill_dir) {
                for fy in fys {
                    entry.insert(*fy);
                }
            }
        }
        // Also use the variant's own fiscal_years field
        for fy in &v.fiscal_years {
            entry.insert(*fy);
        }
    }

    // Check if there's a clear temporal boundary: one name used before FY X,
    // another name used from FY X onward
    let norm_names: Vec<String> = fy_by_norm_name.keys().cloned().collect();
    let mut detected_rename = false;

    if norm_names.len() == 2 {
        let fys_a = &fy_by_norm_name[&norm_names[0]];
        let fys_b = &fy_by_norm_name[&norm_names[1]];

        let max_a = fys_a.iter().max().copied().unwrap_or(0);
        let min_b = fys_b.iter().min().copied().unwrap_or(9999);
        let max_b = fys_b.iter().max().copied().unwrap_or(0);
        let min_a = fys_a.iter().min().copied().unwrap_or(9999);

        // Check if A is entirely before B or B is entirely before A
        if max_a < min_b {
            // A came first, B is the new name
            let old_name = classified
                .iter()
                .find(|v| normalize_variant_name(&v.name) == norm_names[0])
                .map(|v| v.name.clone())
                .unwrap_or_default();
            let new_name = classified
                .iter()
                .find(|v| normalize_variant_name(&v.name) == norm_names[1])
                .map(|v| v.name.clone())
                .unwrap_or_default();

            events.push(AuthorityEvent {
                fiscal_year: min_b,
                event_type: AuthorityEventType::Rename {
                    from: old_name,
                    to: new_name,
                },
            });
            detected_rename = true;
        } else if max_b < min_a {
            // B came first, A is the new name
            let old_name = classified
                .iter()
                .find(|v| normalize_variant_name(&v.name) == norm_names[1])
                .map(|v| v.name.clone())
                .unwrap_or_default();
            let new_name = classified
                .iter()
                .find(|v| normalize_variant_name(&v.name) == norm_names[0])
                .map(|v| v.name.clone())
                .unwrap_or_default();

            events.push(AuthorityEvent {
                fiscal_year: min_a,
                event_type: AuthorityEventType::Rename {
                    from: old_name,
                    to: new_name,
                },
            });
            detected_rename = true;
        }
    }

    // Step 4: Classify each variant
    let most_common_idx = classified
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| v.bills.len())
        .map(|(i, _)| i)
        .unwrap_or(0);

    // Extract canonical name info before mutable iteration (avoids borrow conflict)
    let canon_name = classified[most_common_idx].name.clone();
    let canon_lower = canon_name.to_lowercase();
    let canon_norm = normalize_variant_name(&canon_name);

    for (i, v) in classified.iter_mut().enumerate() {
        if i == most_common_idx {
            v.classification = Some(VariantClassification::Canonical);
        } else if detected_rename {
            v.classification = Some(VariantClassification::NameChange);
        } else {
            // No clear temporal boundary — might be LLM inconsistency
            let norm_self = normalize_variant_name(&v.name);
            if norm_self == canon_norm {
                // Same after normalization — prefix or case variant
                if v.name.to_lowercase() == canon_lower {
                    v.classification = Some(VariantClassification::CaseVariant);
                } else {
                    v.classification = Some(VariantClassification::PrefixVariant);
                }
            } else {
                v.classification = Some(VariantClassification::InconsistentExtraction);
            }
        }
    }

    (classified, events)
}

// ─── Query ───────────────────────────────────────────────────────────────────

/// Find an authority by exact FAS code.
pub fn get_authority<'a>(
    registry: &'a AuthorityRegistry,
    fas_code: &str,
) -> Option<&'a AccountAuthority> {
    registry.authorities.iter().find(|a| a.fas_code == fas_code)
}

/// Search authorities by name fragment (case-insensitive).
///
/// Uses word-level matching: splits the query into words and checks that
/// ALL words appear somewhere across the authority's combined text (FAS title,
/// agency name, FAS code, and all name variants concatenated). This allows
/// queries like `"fema disaster"` to match even though "fema" is in the agency
/// name and "disaster" is in the account title.
///
/// Falls back to single-string containment if word matching finds nothing,
/// so `"070-0400"` still works as a direct search.
pub fn search_authorities<'a>(
    registry: &'a AuthorityRegistry,
    query: &str,
) -> Vec<&'a AccountAuthority> {
    let lower = query.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().filter(|w| w.len() >= 2).collect();

    if words.is_empty() {
        return Vec::new();
    }

    // Strategy 1: All query words appear somewhere in the authority's combined text
    let word_matches: Vec<&AccountAuthority> = registry
        .authorities
        .iter()
        .filter(|a| {
            // Build a combined searchable string from all fields
            let combined = format!(
                "{} {} {} {}",
                a.fas_title.to_lowercase(),
                a.agency_name.to_lowercase(),
                a.fas_code.to_lowercase(),
                a.name_variants
                    .iter()
                    .map(|v| v.name.to_lowercase())
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            words.iter().all(|w| combined.contains(w))
        })
        .collect();

    if !word_matches.is_empty() {
        return word_matches;
    }

    // Strategy 2: Fallback to single-string containment on individual fields
    registry
        .authorities
        .iter()
        .filter(|a| {
            a.fas_title.to_lowercase().contains(&lower)
                || a.agency_name.to_lowercase().contains(&lower)
                || a.fas_code.to_lowercase().contains(&lower)
                || a.name_variants
                    .iter()
                    .any(|v| v.name.to_lowercase().contains(&lower))
        })
        .collect()
}

/// Build a fiscal-year timeline for one authority.
///
/// Groups provisions by fiscal year, sums dollar amounts, and collects
/// the bill identifiers and account names for each FY.
pub fn build_timeline(authority: &AccountAuthority) -> Vec<TimelineEntry> {
    let mut by_fy: HashMap<u32, TimelineEntry> = HashMap::new();

    for p in &authority.provisions {
        for &fy in &p.fiscal_years {
            let entry = by_fy.entry(fy).or_insert_with(|| TimelineEntry {
                fiscal_year: fy,
                dollars: 0,
                bills: Vec::new(),
                account_names: Vec::new(),
            });

            if let Some(d) = p.dollars {
                entry.dollars += d;
            }

            if !entry.bills.contains(&p.bill_identifier) {
                entry.bills.push(p.bill_identifier.clone());
            }

            if !entry.account_names.contains(&p.account_name) {
                entry.account_names.push(p.account_name.clone());
            }
        }
    }

    let mut timeline: Vec<TimelineEntry> = by_fy.into_values().collect();
    timeline.sort_by_key(|e| e.fiscal_year);
    timeline
}

// ─── I/O ─────────────────────────────────────────────────────────────────────

/// Load `authorities.json` from the data root directory.
pub fn load_authorities(dir: &Path) -> Result<Option<AuthorityRegistry>> {
    let path = dir.join("authorities.json");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let registry: AuthorityRegistry = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(registry))
}

/// Save `authorities.json` to the data root directory.
pub fn save_authorities(dir: &Path, registry: &AuthorityRegistry) -> Result<()> {
    let path = dir.join("authorities.json");
    let json = serde_json::to_string_pretty(registry)?;
    std::fs::write(&path, json).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provision(
        bill_dir: &str,
        bill_id: &str,
        index: usize,
        account: &str,
        dollars: i64,
        fys: &[u32],
    ) -> AuthorityProvisionRef {
        AuthorityProvisionRef {
            bill_dir: bill_dir.to_string(),
            bill_identifier: bill_id.to_string(),
            provision_index: index,
            fiscal_years: fys.to_vec(),
            dollars: Some(dollars),
            account_name: account.to_string(),
            confidence: TasConfidence::Verified,
            method: TasMethod::DirectMatch,
        }
    }

    #[test]
    fn test_build_timeline_groups_by_fy() {
        let authority = AccountAuthority {
            fas_code: "070-0400".to_string(),
            agency_code: "070".to_string(),
            fas_title: "Operations and Support, USSS".to_string(),
            agency_name: "DHS".to_string(),
            name_variants: vec![],
            provisions: vec![
                make_provision(
                    "116-hr1158",
                    "H.R. 1158",
                    42,
                    "USSS Ops",
                    2_336_401_000,
                    &[2020],
                ),
                make_provision(
                    "116-hr133",
                    "H.R. 133",
                    87,
                    "USSS Ops",
                    2_373_109_000,
                    &[2021],
                ),
                make_provision(
                    "118-hr2882",
                    "H.R. 2882",
                    15,
                    "Ops and Support",
                    3_007_982_000,
                    &[2024],
                ),
            ],
            bill_count: 3,
            fiscal_years: vec![2020, 2021, 2024],
            total_dollars: 7_717_492_000,
            events: vec![],
        };

        let timeline = build_timeline(&authority);
        assert_eq!(timeline.len(), 3);
        assert_eq!(timeline[0].fiscal_year, 2020);
        assert_eq!(timeline[0].dollars, 2_336_401_000);
        assert_eq!(timeline[1].fiscal_year, 2021);
        assert_eq!(timeline[2].fiscal_year, 2024);
        assert_eq!(timeline[2].dollars, 3_007_982_000);
    }

    #[test]
    fn test_build_timeline_sums_same_fy() {
        let authority = AccountAuthority {
            fas_code: "070-0702".to_string(),
            agency_code: "070".to_string(),
            fas_title: "Disaster Relief Fund".to_string(),
            agency_name: "DHS".to_string(),
            name_variants: vec![],
            provisions: vec![
                make_provision(
                    "118-hr4366",
                    "H.R. 4366",
                    100,
                    "DRF",
                    16_000_000_000,
                    &[2024],
                ),
                make_provision(
                    "118-hr9468",
                    "H.R. 9468",
                    5,
                    "DRF Supplemental",
                    2_000_000_000,
                    &[2024],
                ),
            ],
            bill_count: 2,
            fiscal_years: vec![2024],
            total_dollars: 18_000_000_000,
            events: vec![],
        };

        let timeline = build_timeline(&authority);
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].fiscal_year, 2024);
        assert_eq!(timeline[0].dollars, 18_000_000_000);
        assert_eq!(timeline[0].bills.len(), 2);
    }

    #[test]
    fn test_name_variants_detected() {
        let provisions = vec![
            make_provision(
                "116-hr1158",
                "H.R. 1158",
                0,
                "United States Secret Service—Operations and Support",
                1,
                &[2020],
            ),
            make_provision(
                "118-hr2882",
                "H.R. 2882",
                0,
                "Operations and Support",
                1,
                &[2024],
            ),
        ];

        let variants = collect_name_variants(&provisions);
        assert_eq!(variants.len(), 2, "should detect 2 distinct name variants");

        let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"United States Secret Service—Operations and Support"));
        assert!(names.contains(&"Operations and Support"));
    }

    #[test]
    fn test_name_variants_same_name_one_variant() {
        let provisions = vec![
            make_provision(
                "116-hr1158",
                "H.R. 1158",
                0,
                "Salaries and Expenses",
                1,
                &[2020],
            ),
            make_provision(
                "116-hr133",
                "H.R. 133",
                0,
                "Salaries and Expenses",
                2,
                &[2021],
            ),
        ];

        let variants = collect_name_variants(&provisions);
        assert_eq!(variants.len(), 1, "same name across bills = 1 variant");
        assert_eq!(variants[0].bills.len(), 2, "both bills should be listed");
    }

    #[test]
    fn test_classify_prefix_variants() {
        let variants = vec![
            NameVariant {
                name: "United States Secret Service—Operations and Support".to_string(),
                bills: vec!["116-hr1158".to_string()],
                classification: None,
                fiscal_years: vec![2020],
            },
            NameVariant {
                name: "Operations and Support".to_string(),
                bills: vec!["118-hr2882".to_string()],
                classification: None,
                fiscal_years: vec![2024],
            },
        ];
        let bill_fys: HashMap<String, Vec<u32>> = [
            ("116-hr1158".to_string(), vec![2020]),
            ("118-hr2882".to_string(), vec![2024]),
        ]
        .into_iter()
        .collect();

        let (classified, events) = classify_variants_and_detect_events(&variants, &bill_fys);
        assert_eq!(classified.len(), 2);
        // Both normalize to "operations and support" — so prefix variant, not rename
        assert!(
            events.is_empty(),
            "em-dash prefix difference should not be a rename event"
        );
        let has_prefix = classified
            .iter()
            .any(|v| v.classification == Some(VariantClassification::PrefixVariant));
        assert!(has_prefix, "should detect prefix variant");
    }

    #[test]
    fn test_classify_real_rename() {
        let variants = vec![
            NameVariant {
                name: "Allowances and Expenses".to_string(),
                bills: vec!["117-hr2471".to_string()],
                classification: None,
                fiscal_years: vec![2021],
            },
            NameVariant {
                name: "Members' Representational Allowances".to_string(),
                bills: vec!["119-hr1968".to_string()],
                classification: None,
                fiscal_years: vec![2025],
            },
        ];
        let bill_fys: HashMap<String, Vec<u32>> = [
            ("117-hr2471".to_string(), vec![2021]),
            ("119-hr1968".to_string(), vec![2025]),
        ]
        .into_iter()
        .collect();

        let (classified, events) = classify_variants_and_detect_events(&variants, &bill_fys);
        assert_eq!(events.len(), 1, "should detect one rename event");
        assert_eq!(events[0].fiscal_year, 2025);
        match &events[0].event_type {
            AuthorityEventType::Rename { from, to } => {
                assert_eq!(from, "Allowances and Expenses");
                assert_eq!(to, "Members' Representational Allowances");
            }
        }
        let has_name_change = classified
            .iter()
            .any(|v| v.classification == Some(VariantClassification::NameChange));
        assert!(has_name_change);
    }

    #[test]
    fn test_classify_case_only() {
        let variants = vec![
            NameVariant {
                name: "Operation and Maintenance, Army".to_string(),
                bills: vec!["116-hr133".to_string()],
                classification: None,
                fiscal_years: vec![2021],
            },
            NameVariant {
                name: "Operation and maintenance, army".to_string(),
                bills: vec!["117-hr2471".to_string()],
                classification: None,
                fiscal_years: vec![2022],
            },
        ];
        let bill_fys: HashMap<String, Vec<u32>> = [
            ("116-hr133".to_string(), vec![2021]),
            ("117-hr2471".to_string(), vec![2022]),
        ]
        .into_iter()
        .collect();

        let (classified, events) = classify_variants_and_detect_events(&variants, &bill_fys);
        assert!(
            events.is_empty(),
            "case-only difference should not be an event"
        );
        let has_case = classified
            .iter()
            .any(|v| v.classification == Some(VariantClassification::CaseVariant));
        assert!(has_case, "should detect case variant");
    }

    #[test]
    fn test_normalize_variant_name() {
        assert_eq!(
            normalize_variant_name("United States Secret Service\u{2014}Operations and Support"),
            "operations and support"
        );
        assert_eq!(
            normalize_variant_name("Operations and Support"),
            "operations and support"
        );
        assert_eq!(
            normalize_variant_name("Salaries and Expenses"),
            "salaries and expenses"
        );
    }

    #[test]
    fn test_search_authorities() {
        let registry = AuthorityRegistry {
            schema_version: "1.0".to_string(),
            generated_at: String::new(),
            fas_reference_hash: String::new(),
            authorities: vec![
                AccountAuthority {
                    fas_code: "070-0400".to_string(),
                    agency_code: "070".to_string(),
                    fas_title: "Operations and Support, United States Secret Service".to_string(),
                    agency_name: "Department of Homeland Security".to_string(),
                    name_variants: vec![NameVariant {
                        name: "USSS—Operations and Support".to_string(),
                        bills: vec!["116-hr1158".to_string()],
                        classification: None,
                        fiscal_years: vec![2020],
                    }],
                    provisions: vec![],
                    bill_count: 1,
                    fiscal_years: vec![2020],
                    total_dollars: 0,
                    events: vec![],
                },
                AccountAuthority {
                    fas_code: "015-0339".to_string(),
                    agency_code: "015".to_string(),
                    fas_title: "Executive Office for Immigration Review".to_string(),
                    agency_name: "Department of Justice".to_string(),
                    name_variants: vec![],
                    provisions: vec![],
                    bill_count: 1,
                    fiscal_years: vec![2020],
                    total_dollars: 0,
                    events: vec![],
                },
            ],
            summary: RegistrySummary {
                total_authorities: 2,
                total_provisions: 0,
                bills_included: 1,
                fiscal_years_covered: vec![2020],
                authorities_with_name_variants: 1,
                authorities_in_multiple_bills: 0,
                total_events: 0,
            },
        };

        // Search by agency name
        let results = search_authorities(&registry, "secret service");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fas_code, "070-0400");

        // Search by FAS code
        let results = search_authorities(&registry, "015-0339");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fas_code, "015-0339");

        // Search by name variant
        let results = search_authorities(&registry, "USSS");
        assert_eq!(results.len(), 1);

        // Search with no match
        let results = search_authorities(&registry, "NASA");
        assert_eq!(results.len(), 0);

        // Search by department
        let results = search_authorities(&registry, "homeland");
        assert_eq!(results.len(), 1);

        // Word-level search across fields: "secret" in title + no other constraint
        let results = search_authorities(&registry, "secret service operations");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fas_code, "070-0400");
    }

    #[test]
    fn test_get_authority_exact() {
        let registry = AuthorityRegistry {
            schema_version: "1.0".to_string(),
            generated_at: String::new(),
            fas_reference_hash: String::new(),
            authorities: vec![AccountAuthority {
                fas_code: "070-0400".to_string(),
                agency_code: "070".to_string(),
                fas_title: "Test".to_string(),
                agency_name: "Test".to_string(),
                name_variants: vec![],
                provisions: vec![],
                bill_count: 0,
                fiscal_years: vec![],
                total_dollars: 0,
                events: vec![],
            }],
            summary: RegistrySummary {
                total_authorities: 1,
                total_provisions: 0,
                bills_included: 0,
                fiscal_years_covered: vec![],
                authorities_with_name_variants: 0,
                authorities_in_multiple_bills: 0,
                total_events: 0,
            },
        };

        assert!(get_authority(&registry, "070-0400").is_some());
        assert!(get_authority(&registry, "999-9999").is_none());
    }

    #[test]
    fn test_empty_timeline() {
        let authority = AccountAuthority {
            fas_code: "999-0000".to_string(),
            agency_code: "999".to_string(),
            fas_title: "Empty".to_string(),
            agency_name: "None".to_string(),
            name_variants: vec![],
            provisions: vec![],
            bill_count: 0,
            fiscal_years: vec![],
            total_dollars: 0,
            events: vec![],
        };

        let timeline = build_timeline(&authority);
        assert!(timeline.is_empty());
    }
}
