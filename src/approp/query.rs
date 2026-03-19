//! Query operations over loaded bill data.
//!
//! These functions take `&[LoadedBill]` and return plain data structs
//! suitable for any output format. The CLI layer handles formatting.

use crate::approp::bill_meta::{self, FundingTiming};
use crate::approp::embeddings::{self, LoadedEmbeddings};
use crate::approp::loading::LoadedBill;
use crate::approp::ontology::{AmountSemantics, Provision};
use crate::approp::verification::{CheckResult, MatchTier};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

// ─── Summary ─────────────────────────────────────────────────────────────────

/// High-level per-bill budget summary.
#[derive(Debug, Serialize)]
pub struct BillSummary {
    pub identifier: String,
    pub classification: String,
    pub provisions: usize,
    pub budget_authority: i64,
    pub rescissions: i64,
    pub net_ba: i64,
    pub completeness_pct: Option<f64>,
    /// Current-year budget authority (excluding advance). Present when bill_meta exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_year_ba: Option<i64>,
    /// Advance budget authority (for future FYs). Present when bill_meta exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advance_ba: Option<i64>,
}

/// Produce a summary row for every loaded bill.
pub fn summarize(bills: &[LoadedBill]) -> Vec<BillSummary> {
    bills
        .iter()
        .map(|loaded| {
            let (ba, rescissions) = loaded.extraction.compute_totals();
            let completeness = loaded
                .verification
                .as_ref()
                .map(|v| v.summary.completeness_pct);

            // Prefer enriched bill_nature from bill_meta when available,
            // fall back to the LLM's original classification.
            let classification = loaded
                .bill_meta
                .as_ref()
                .map(|m| format!("{}", m.bill_nature))
                .unwrap_or_else(|| format!("{}", loaded.extraction.bill.classification));

            // Compute advance/current split from bill_meta if available
            let (current_year_ba, advance_ba) = compute_advance_split(loaded);

            BillSummary {
                identifier: loaded.extraction.bill.identifier.clone(),
                classification,
                provisions: loaded.extraction.provisions.len(),
                budget_authority: ba,
                rescissions,
                net_ba: ba - rescissions,
                completeness_pct: completeness,
                current_year_ba,
                advance_ba,
            }
        })
        .collect()
}

/// Compute current-year vs advance budget authority split from bill_meta timing data.
///
/// Returns `(Some(current), Some(advance))` if bill_meta has provision_timing data,
/// `(None, None)` otherwise.
///
/// Always loads the original extraction from disk when provision_timing indices
/// exceed the current provision count. This handles subcommittee-filtered bills
/// where `loaded.extraction.provisions` is a subset with shifted indices but
/// `bill_meta.provision_timing` stores original extraction indices.
fn compute_advance_split(loaded: &LoadedBill) -> (Option<i64>, Option<i64>) {
    let Some(meta) = &loaded.bill_meta else {
        return (None, None);
    };
    if meta.provision_timing.is_empty() {
        return (None, None);
    }

    // Check if any timing index exceeds the current provision count —
    // this means provisions have been filtered and we need the original.
    let max_timing_idx = meta
        .provision_timing
        .iter()
        .map(|t| t.provision_index)
        .max()
        .unwrap_or(0);

    let original_extraction = if max_timing_idx >= loaded.extraction.provisions.len() {
        // Provision indices from bill_meta exceed current list — load original from disk
        let ext_path = loaded.dir.join("extraction.json");
        std::fs::read_to_string(&ext_path)
            .ok()
            .and_then(|s| serde_json::from_str::<crate::approp::ontology::BillExtraction>(&s).ok())
    } else {
        None
    };
    let provisions = original_extraction
        .as_ref()
        .map(|e| e.provisions.as_slice())
        .unwrap_or(&loaded.extraction.provisions);

    // Determine which divisions are active in the (potentially filtered) bill.
    // If the bill has been subcommittee-filtered, only provisions in the active
    // divisions should be counted. If unfiltered, all divisions are active.
    let active_divisions: Option<std::collections::HashSet<String>> =
        if original_extraction.is_some() {
            // Bill was filtered — collect divisions from the filtered provision list
            let divs: std::collections::HashSet<String> = loaded
                .extraction
                .provisions
                .iter()
                .filter_map(|p| p.division().map(|d| d.to_uppercase()))
                .collect();
            if divs.is_empty() { None } else { Some(divs) }
        } else {
            None // unfiltered — all divisions included
        };

    let mut current = 0i64;
    let mut advance = 0i64;

    for timing_entry in &meta.provision_timing {
        let idx = timing_entry.provision_index;
        if idx >= provisions.len() {
            continue;
        }
        let p = &provisions[idx];

        // Skip provisions not in the active divisions (when subcommittee-filtered)
        if let Some(ref divs) = active_divisions {
            let prov_div = p.division().unwrap_or("").to_uppercase();
            if !divs.contains(&prov_div) {
                continue;
            }
        }

        if let Some(amt) = p.amount() {
            if !matches!(amt.semantics, AmountSemantics::NewBudgetAuthority) {
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
            let dollars = amt.dollars().unwrap_or(0);
            match timing_entry.timing {
                bill_meta::FundingTiming::Advance => advance += dollars,
                _ => current += dollars,
            }
        }
    }

    (Some(current), Some(advance))
}

// ─── Agency Rollup ───────────────────────────────────────────────────────────

/// Budget authority aggregated to the parent-department level.
#[derive(Debug, Serialize)]
pub struct AgencyRollup {
    pub department: String,
    pub budget_authority: i64,
    pub rescissions: i64,
    pub provision_count: usize,
}

/// Compute budget authority by parent department.
///
/// Uses query-time comma-split on the agency field — never modifies stored data.
///
/// For each provision, extract parent department:
///   - If agency contains `,`, take everything before the first comma.
///   - Exception: if it starts with `"Office of Inspector General,"`, take the
///     part after the comma instead.
///   - Otherwise use the agency string as-is.
///
/// Only count provisions where semantics == `NewBudgetAuthority` for BA.
/// Only count provisions where the provision type is `rescission` for rescissions.
///
/// Results are sorted by `budget_authority` descending.
pub fn rollup_by_department(bills: &[LoadedBill]) -> Vec<AgencyRollup> {
    let mut map: HashMap<String, (i64, i64, usize)> = HashMap::new();

    for loaded in bills {
        for p in &loaded.extraction.provisions {
            let agency_raw = p.agency();
            if agency_raw.is_empty() {
                continue;
            }

            let department = extract_parent_department(agency_raw);

            let entry = map.entry(department).or_insert((0, 0, 0));
            entry.2 += 1;

            if let Some(amt) = p.amount() {
                let dollars = amt.dollars().unwrap_or(0);
                if matches!(amt.semantics, AmountSemantics::NewBudgetAuthority) {
                    entry.0 += dollars;
                }
                if matches!(p, Provision::Rescission { .. })
                    && matches!(amt.semantics, AmountSemantics::Rescission)
                {
                    entry.1 += dollars.abs();
                }
            }
        }
    }

    let mut rollups: Vec<AgencyRollup> = map
        .into_iter()
        .map(|(dept, (ba, resc, count))| AgencyRollup {
            department: dept,
            budget_authority: ba,
            rescissions: resc,
            provision_count: count,
        })
        .collect();

    rollups.sort_by(|a, b| b.budget_authority.cmp(&a.budget_authority));
    rollups
}

/// Extract the parent department from an agency string.
///
/// - `"Office of Inspector General, Department of Defense"` → `"Department of Defense"`
/// - `"Corps of Engineers—Civil, Department of Defense"` → `"Corps of Engineers—Civil"`
///   (takes before first comma normally)
/// - `"Department of Defense"` → `"Department of Defense"` (no comma)
fn extract_parent_department(agency: &str) -> String {
    if let Some(comma_pos) = agency.find(',') {
        let before = agency[..comma_pos].trim();
        let after = agency[comma_pos + 1..].trim();

        // Exception: if the text before the comma is "Office of Inspector General",
        // the real department is the part *after* the comma.
        if before.eq_ignore_ascii_case("Office of Inspector General") {
            return after.to_string();
        }

        before.to_string()
    } else {
        agency.to_string()
    }
}

// ─── Audit ───────────────────────────────────────────────────────────────────

/// Per-bill verification audit numbers.
#[derive(Debug, Serialize)]
pub struct AuditRow {
    pub identifier: String,
    pub total_provisions: usize,
    pub amounts_verified: usize,
    pub amounts_not_found: usize,
    pub amounts_ambiguous: usize,
    pub raw_text_exact: usize,
    pub raw_text_normalized: usize,
    pub raw_text_spaceless: usize,
    pub raw_text_no_match: usize,
    pub completeness_pct: f64,
}

/// Produce an audit row for every loaded bill that has a verification report.
///
/// Bills without verification data are included with all counts set to zero
/// and `completeness_pct` of `0.0`.
pub fn audit(bills: &[LoadedBill]) -> Vec<AuditRow> {
    bills
        .iter()
        .map(|loaded| {
            let bill_id = &loaded.extraction.bill.identifier;
            let total_provisions = loaded.extraction.provisions.len();

            if let Some(ref ver) = loaded.verification {
                let s = &ver.summary;
                AuditRow {
                    identifier: bill_id.clone(),
                    total_provisions,
                    amounts_verified: s.amounts_verified,
                    amounts_not_found: s.amounts_not_found,
                    amounts_ambiguous: s.amounts_ambiguous,
                    raw_text_exact: s.raw_text_exact,
                    raw_text_normalized: s.raw_text_normalized,
                    raw_text_spaceless: s.raw_text_spaceless,
                    raw_text_no_match: s.raw_text_no_match,
                    completeness_pct: s.completeness_pct,
                }
            } else {
                AuditRow {
                    identifier: bill_id.clone(),
                    total_provisions,
                    amounts_verified: 0,
                    amounts_not_found: 0,
                    amounts_ambiguous: 0,
                    raw_text_exact: 0,
                    raw_text_normalized: 0,
                    raw_text_spaceless: 0,
                    raw_text_no_match: 0,
                    completeness_pct: 0.0,
                }
            }
        })
        .collect()
}

// ─── Search ──────────────────────────────────────────────────────────────────

/// Filtering criteria for provision search.
#[derive(Debug, Default)]
pub struct SearchFilter<'a> {
    pub provision_type: Option<&'a str>,
    pub agency: Option<&'a str>,
    pub account: Option<&'a str>,
    pub keyword: Option<&'a str>,
    pub bill: Option<&'a str>,
    pub division: Option<&'a str>,
    pub min_dollars: Option<i64>,
    pub max_dollars: Option<i64>,
}

/// A single provision matching the search criteria, with verification metadata.
#[derive(Debug)]
pub struct SearchResult<'a> {
    pub bill_identifier: &'a str,
    pub provision_index: usize,
    pub provision: &'a Provision,
    /// `"found"`, `"found_multiple"`, `"not_found"`, or `None` if no verification.
    pub amount_status: Option<&'static str>,
    /// `"exact"`, `"normalized"`, `"spaceless"`, `"no_match"`, or `None`.
    pub match_tier: Option<&'static str>,
    /// Overall quality indicator: `"strong"`, `"moderate"`, `"weak"`, or `"n/a"`.
    pub quality: &'static str,
}

/// Search all bills for provisions matching every filter in `filter`.
///
/// Filters are ANDed together: a provision must satisfy **all** non-`None`
/// criteria to be included.
pub fn search<'a>(bills: &'a [LoadedBill], filter: &SearchFilter<'_>) -> Vec<SearchResult<'a>> {
    // Pre-build the verification lookup.
    let ver_lookup = build_verification_lookup(bills);

    let mut results: Vec<SearchResult<'a>> = Vec::new();

    for loaded in bills {
        let bill_id = loaded.extraction.bill.identifier.as_str();

        // Bill filter
        if let Some(bill_filter) = filter.bill
            && !bill_id.to_lowercase().contains(&bill_filter.to_lowercase())
        {
            continue;
        }

        for (idx, provision) in loaded.extraction.provisions.iter().enumerate() {
            if !provision_matches(provision, filter) {
                continue;
            }

            let ver_key = (bill_id, idx);
            let (amount_status, match_tier) =
                ver_lookup.get(&ver_key).copied().unwrap_or((None, None));

            let quality = compute_quality(amount_status, match_tier);

            results.push(SearchResult {
                bill_identifier: bill_id,
                provision_index: idx,
                provision,
                amount_status,
                match_tier,
                quality,
            });
        }
    }

    results
}

/// Apply every non-bill filter in `SearchFilter` to a single provision.
fn provision_matches(provision: &Provision, filter: &SearchFilter<'_>) -> bool {
    // Type filter
    if let Some(type_filter) = filter.provision_type
        && provision.type_str() != type_filter
    {
        return false;
    }

    // Agency filter (case-insensitive contains)
    if let Some(agency_filter) = filter.agency
        && !provision
            .agency()
            .to_lowercase()
            .contains(&agency_filter.to_lowercase())
    {
        return false;
    }

    // Account filter (case-insensitive contains)
    if let Some(account_filter) = filter.account
        && !provision
            .account_name()
            .to_lowercase()
            .contains(&account_filter.to_lowercase())
    {
        return false;
    }

    // Keyword filter — searches raw_text (case-insensitive contains)
    if let Some(keyword_filter) = filter.keyword
        && !provision
            .raw_text()
            .to_lowercase()
            .contains(&keyword_filter.to_lowercase())
    {
        return false;
    }

    // Division filter (case-insensitive exact match)
    if let Some(div_filter) = filter.division {
        let pdivision = provision.division().unwrap_or("");
        if !pdivision.eq_ignore_ascii_case(div_filter) {
            return false;
        }
    }

    // Dollar-amount range filters
    if filter.min_dollars.is_some() || filter.max_dollars.is_some() {
        let abs_dollars = provision
            .amount()
            .and_then(|a| a.dollars())
            .map(|d| d.abs());

        if let Some(min) = filter.min_dollars {
            match abs_dollars {
                Some(d) if d >= min => {}
                _ => return false,
            }
        }
        if let Some(max) = filter.max_dollars {
            match abs_dollars {
                Some(d) if d <= max => {}
                _ => return false,
            }
        }
    }

    true
}

/// Determine overall quality from amount verification status and raw-text match tier.
///
/// - `"strong"` — amount uniquely found **and** raw text is an exact match.
/// - `"weak"` — amount not found in source, or raw text had no match.
/// - `"moderate"` — everything in between.
/// - `"n/a"` — no verification data available.
pub fn compute_quality(amount_status: Option<&str>, match_tier: Option<&str>) -> &'static str {
    match (amount_status, match_tier) {
        (Some("found"), Some("exact")) => "strong",
        (Some("found"), Some("normalized" | "spaceless")) => "moderate",
        (Some("found_multiple"), Some("exact" | "normalized")) => "moderate",
        (Some("found"), Some("no_match")) => "moderate",
        (Some("found_multiple"), Some("no_match" | "spaceless")) => "weak",
        (Some("not_found"), _) => "weak",
        _ => "n/a",
    }
}

// ─── Compare ─────────────────────────────────────────────────────────────────

/// One row of the comparison table.
#[derive(Debug, Serialize)]
pub struct CompareRow {
    pub account_name: String,
    pub agency: String,
    pub base_dollars: i64,
    pub current_dollars: i64,
    pub delta: i64,
    pub delta_pct: Option<f64>,
    /// One of `"changed"`, `"only in base"`, `"only in current"`, `"unchanged"`, `"reclassified"`.
    pub status: String,
    /// Inflation-adjusted percentage change. Present when `--real` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_delta_pct: Option<f64>,
    /// Inflation flag: real_increase, real_cut, inflation_erosion, unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inflation_flag: Option<String>,
}

/// The full result of comparing two sets of bills.
#[derive(Debug, Serialize)]
pub struct CompareResult {
    pub base_description: String,
    pub current_description: String,
    /// Present when the two bill sets have different classifications.
    pub cross_type_warning: Option<String>,
    /// Rows sorted by absolute delta descending.
    pub rows: Vec<CompareRow>,
}

/// Compare appropriation accounts between a base set and a current set of bills.
///
/// Includes cross-type warning logic when the bill classifications differ.
/// The optional `agency_filter` narrows both sides to matching agencies.
pub fn compare(
    base: &[LoadedBill],
    current: &[LoadedBill],
    agency_filter: Option<&str>,
) -> CompareResult {
    let base_description = describe_bills(base);
    let current_description = describe_bills(current);

    // Cross-type warning
    let cross_type_warning = if !base.is_empty() && !current.is_empty() {
        let base_class = &base[0].extraction.bill.classification;
        let current_class = &current[0].extraction.bill.classification;
        if std::mem::discriminant(base_class) != std::mem::discriminant(current_class) {
            Some(format!(
                "Comparing {base_class} to {current_class}. \
                 Accounts in one but not the other may be expected — \
                 this does not necessarily indicate policy changes."
            ))
        } else {
            None
        }
    } else {
        None
    };

    // Build account maps: (agency, account_name) → total dollars
    let base_accounts = build_account_map(base, agency_filter);
    let current_accounts = build_account_map(current, agency_filter);

    // Collect the union of all keys
    let mut all_keys: Vec<(String, String)> = Vec::new();
    for k in base_accounts.keys() {
        all_keys.push(k.clone());
    }
    for k in current_accounts.keys() {
        if !all_keys.contains(k) {
            // Try suffix matching for hierarchical CR names
            let short = normalize_account_name(&k.1);
            let found = base_accounts
                .keys()
                .any(|bk| normalize_account_name(&bk.1) == short && bk.0 == k.0);
            if !found {
                all_keys.push(k.clone());
            }
        }
    }
    all_keys.sort();
    all_keys.dedup();

    let mut rows: Vec<CompareRow> = Vec::new();

    for key in &all_keys {
        let base_entry = base_accounts.get(key);
        let base_val = base_entry.map(|e| e.dollars).unwrap_or(0);

        // Look up in current — try exact match first, then suffix match
        let current_entry = current_accounts.get(key).or_else(|| {
            let short = normalize_account_name(&key.1);
            current_accounts
                .iter()
                .find(|(k, _)| k.0 == key.0 && normalize_account_name(&k.1) == short)
                .map(|(_, v)| v)
        });
        let current_val = current_entry.map(|e| e.dollars).unwrap_or(0);

        if base_val == 0 && current_val == 0 {
            continue;
        }

        let delta = current_val - base_val;
        let delta_pct = if base_val != 0 {
            Some((delta as f64 / base_val as f64) * 100.0)
        } else {
            None
        };

        let status = if base_val == 0 {
            "only in current"
        } else if current_val == 0 {
            "only in base"
        } else if delta == 0 {
            "unchanged"
        } else {
            "changed"
        };

        // Use original display names — prefer base side, fall back to current
        let (display_account, display_agency) = base_entry
            .or(current_entry)
            .map(|e| (e.display_account.clone(), e.display_agency.clone()))
            .unwrap_or_else(|| (key.1.clone(), key.0.clone()));

        rows.push(CompareRow {
            account_name: display_account,
            agency: display_agency,
            base_dollars: base_val,
            current_dollars: current_val,
            delta,
            delta_pct,
            status: status.to_string(),
            real_delta_pct: None,
            inflation_flag: None,
        });
    }

    // ── Cross-semantics orphan rescue ────────────────────────────────────
    //
    // Scan orphans ("only in base" / "only in current") and check if the
    // account name exists in the other bill under a different semantics
    // (e.g., Transit Formula Grants classified as "limitation" in one bill
    // and "new_budget_authority" in the other). If found, change status to
    // "reclassified" and fill in the missing dollars.
    let base_wide = build_any_semantics_map(base, agency_filter);
    let current_wide = build_any_semantics_map(current, agency_filter);

    for row in &mut rows {
        if row.status == "only in base" {
            // Base has it, current doesn't (under BA semantics).
            // Check if current has it under ANY semantics.
            let key = (
                normalize_agency(&row.agency),
                normalize_account_name(&row.account_name),
            );
            if let Some(wide_entry) = current_wide.get(&key) {
                row.current_dollars = wide_entry.dollars;
                row.delta = row.current_dollars - row.base_dollars;
                row.delta_pct = if row.base_dollars != 0 {
                    Some((row.delta as f64 / row.base_dollars as f64) * 100.0)
                } else {
                    None
                };
                row.status = "reclassified".to_string();
            }
        } else if row.status == "only in current" {
            // Current has it, base doesn't (under BA semantics).
            // Check if base has it under ANY semantics.
            let key = (
                normalize_agency(&row.agency),
                normalize_account_name(&row.account_name),
            );
            if let Some(wide_entry) = base_wide.get(&key) {
                row.base_dollars = wide_entry.dollars;
                row.delta = row.current_dollars - row.base_dollars;
                row.delta_pct = if row.base_dollars != 0 {
                    Some((row.delta as f64 / row.base_dollars as f64) * 100.0)
                } else {
                    None
                };
                row.status = "reclassified".to_string();
            }
        }
    }

    // Sort by absolute delta descending
    rows.sort_by(|a, b| b.delta.unsigned_abs().cmp(&a.delta.unsigned_abs()));

    CompareResult {
        base_description,
        current_description,
        cross_type_warning,
        rows,
    }
}

// ─── Private helpers ─────────────────────────────────────────────────────────

/// Map of known sub-agency names to their parent department.
///
/// Used by `normalize_agency` to resolve cases where the LLM uses different
/// granularities across bills — e.g., "Maritime Administration" in one bill
/// and "Department of Transportation" in another for the same account.
///
/// This is a static lookup because the agency hierarchy is stable across
/// congresses. The trade-off: within-bill entries under different sub-agencies
/// of the same department with the same account name (e.g., two different
/// "Salaries and Expenses" lines under DOT) will be merged. In practice this
/// affects ~7 entries across the 13-bill dataset, all for administrative
/// accounts where the merged total is still directionally correct.
const SUB_AGENCY_TO_PARENT: &[(&str, &str)] = &[
    // Department of Transportation
    ("maritime administration", "department of transportation"),
    (
        "federal highway administration",
        "department of transportation",
    ),
    (
        "federal aviation administration",
        "department of transportation",
    ),
    (
        "federal transit administration",
        "department of transportation",
    ),
    (
        "federal railroad administration",
        "department of transportation",
    ),
    (
        "national highway traffic safety administration",
        "department of transportation",
    ),
    (
        "great lakes st. lawrence seaway development corporation",
        "department of transportation",
    ),
    (
        "pipeline and hazardous materials safety administration",
        "department of transportation",
    ),
    // Department of the Interior
    ("national park service", "department of the interior"),
    ("bureau of land management", "department of the interior"),
    (
        "u.s. fish and wildlife service",
        "department of the interior",
    ),
    (
        "united states fish and wildlife service",
        "department of the interior",
    ),
    ("bureau of indian affairs", "department of the interior"),
    ("bureau of indian education", "department of the interior"),
    ("bureau of reclamation", "department of the interior"),
    ("u.s. geological survey", "department of the interior"),
    (
        "united states geological survey",
        "department of the interior",
    ),
    (
        "office of surface mining reclamation and enforcement",
        "department of the interior",
    ),
    (
        "bureau of ocean energy management",
        "department of the interior",
    ),
    (
        "bureau of safety and environmental enforcement",
        "department of the interior",
    ),
    (
        "bureau of trust funds administration",
        "department of the interior",
    ),
    // Department of Housing and Urban Development
    (
        "government national mortgage association",
        "department of housing and urban development",
    ),
    (
        "federal housing administration",
        "department of housing and urban development",
    ),
    (
        "neighborhood reinvestment corporation",
        "department of housing and urban development",
    ),
    // Department of Justice
    ("federal bureau of investigation", "department of justice"),
    ("drug enforcement administration", "department of justice"),
    (
        "bureau of alcohol, tobacco, firearms and explosives",
        "department of justice",
    ),
    ("u.s. marshals service", "department of justice"),
    ("federal bureau of prisons", "department of justice"),
    ("bureau of justice assistance", "department of justice"),
    // Department of Commerce
    (
        "national oceanic and atmospheric administration",
        "department of commerce",
    ),
    (
        "national institute of standards and technology",
        "department of commerce",
    ),
    ("u.s. patent and trademark office", "department of commerce"),
    (
        "international trade administration",
        "department of commerce",
    ),
    ("bureau of industry and security", "department of commerce"),
    (
        "economic development administration",
        "department of commerce",
    ),
    (
        "minority business development agency",
        "department of commerce",
    ),
    // Department of Defense (civil)
    (
        "department of the army, corps of engineers—civil",
        "department of defense—civil",
    ),
];

/// Normalize an agency name to its parent department for cross-bill matching.
///
/// First applies comma-based extraction (e.g., "Office of Inspector General,
/// Department of Defense" → "Department of Defense"), then looks up the result
/// in the sub-agency-to-parent table.
pub fn normalize_agency(agency: &str) -> String {
    let mut lower = agency.to_lowercase();
    lower = lower.trim().to_string();

    // Step 1: separator-based parent extraction
    // Handle both comma-separated ("Office of Inspector General, DOT") and
    // slash-separated ("Department of Energy / NNSA") agency names.
    if let Some(comma_pos) = lower.find(',') {
        let before = lower[..comma_pos].trim().to_string();
        let after = lower[comma_pos + 1..].trim().to_string();
        if before == "office of inspector general" {
            lower = after;
        } else {
            lower = before;
        }
    } else if let Some(slash_pos) = lower.find(" / ") {
        // "Department of Energy / National Nuclear Security Administration"
        // → try the part after the slash as a sub-agency lookup first
        let after = lower[slash_pos + 3..].trim().to_string();
        let before = lower[..slash_pos].trim().to_string();
        // Check if the part after the slash is a known sub-agency
        for (sub, parent) in SUB_AGENCY_TO_PARENT {
            if after == *sub {
                return parent.to_string();
            }
        }
        // If not in table, use the part before the slash (the parent)
        lower = before;
    }

    // Step 2: sub-agency lookup
    for (sub, parent) in SUB_AGENCY_TO_PARENT {
        if lower == *sub {
            return parent.to_string();
        }
    }

    lower
}

/// An entry in the account comparison map, holding the aggregated dollar total
/// and the original (non-normalized) names for display purposes.
struct AccountMapEntry {
    dollars: i64,
    /// Original agency name as extracted (before lowercasing).
    display_agency: String,
    /// Original account name as extracted (before normalization).
    display_account: String,
}

/// Build a map of `(normalized_agency, normalized_account) → AccountMapEntry`
/// for appropriation provisions with `NewBudgetAuthority` semantics.
///
/// Keys are lowercased and account names are em-dash-stripped for case-insensitive
/// matching. The `AccountMapEntry` preserves the original names for display.
fn build_account_map(
    bills: &[LoadedBill],
    agency_filter: Option<&str>,
) -> HashMap<(String, String), AccountMapEntry> {
    let mut accounts: HashMap<(String, String), AccountMapEntry> = HashMap::new();
    for loaded in bills {
        for p in &loaded.extraction.provisions {
            if let Some(amt) = p.amount() {
                if !matches!(amt.semantics, AmountSemantics::NewBudgetAuthority) {
                    continue;
                }
                if !matches!(p, Provision::Appropriation { .. }) {
                    continue;
                }
                let ag = p.agency();
                let ag = if ag.is_empty() { "(unknown)" } else { ag };
                if let Some(filter) = agency_filter
                    && !ag.to_lowercase().contains(&filter.to_lowercase())
                {
                    continue;
                }
                let key = (
                    normalize_agency(ag),
                    normalize_account_name(p.account_name()),
                );
                let entry = accounts.entry(key).or_insert_with(|| AccountMapEntry {
                    dollars: 0,
                    display_agency: ag.to_string(),
                    display_account: p.account_name().to_string(),
                });
                entry.dollars += amt.dollars().unwrap_or(0);
            }
        }
    }
    accounts
}

/// Build a map of `(normalized_agency, normalized_account) → AccountMapEntry`
/// for appropriation AND limitation provisions with ANY dollar semantics.
///
/// Used by the cross-semantics orphan rescue in `compare()` to detect
/// accounts that exist in both bills but with different semantics
/// (e.g., Transit Formula Grants as "limitation" in one bill and
/// "new_budget_authority" in another).
fn build_any_semantics_map(
    bills: &[LoadedBill],
    agency_filter: Option<&str>,
) -> HashMap<(String, String), AccountMapEntry> {
    let mut accounts: HashMap<(String, String), AccountMapEntry> = HashMap::new();
    for loaded in bills {
        for p in &loaded.extraction.provisions {
            if let Some(amt) = p.amount() {
                // Include Appropriation and Limitation provisions with any semantics
                if !matches!(
                    p,
                    Provision::Appropriation { .. } | Provision::Limitation { .. }
                ) {
                    continue;
                }
                let ag = p.agency();
                let ag = if ag.is_empty() { "(unknown)" } else { ag };
                if let Some(filter) = agency_filter
                    && !ag.to_lowercase().contains(&filter.to_lowercase())
                {
                    continue;
                }
                let key = (
                    normalize_agency(ag),
                    normalize_account_name(p.account_name()),
                );
                let entry = accounts.entry(key).or_insert_with(|| AccountMapEntry {
                    dollars: 0,
                    display_agency: ag.to_string(),
                    display_account: p.account_name().to_string(),
                });
                entry.dollars += amt.dollars().unwrap_or(0);
            }
        }
    }
    accounts
}

/// Normalize account name for fuzzy cross-bill matching.
///
/// Lowercases, strips hierarchical prefixes separated by em-dash or en-dash,
/// and trims whitespace. This ensures that "Grants-In-Aid for Airports",
/// "Grants-in-Aid for Airports", and "Department of VA—Grants-in-Aid for Airports"
/// all normalize to the same string.
pub fn normalize_account_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let parts: Vec<&str> = lower.split(&['\u{2014}', '\u{2013}'][..]).collect();
    if parts.len() > 1 {
        return parts.last().unwrap_or(&"").trim().to_string();
    }
    lower.trim().to_string()
}

/// Create a short human-readable description of a set of loaded bills.
fn describe_bills(bills: &[LoadedBill]) -> String {
    if bills.is_empty() {
        return String::new();
    }
    if bills.len() == 1 {
        return bills[0].extraction.bill.identifier.clone();
    }
    let ids: Vec<&str> = bills
        .iter()
        .map(|b| b.extraction.bill.identifier.as_str())
        .collect();
    if ids.len() <= 3 {
        ids.join(", ")
    } else {
        format!("{} bills ({}, {}, ...)", ids.len(), ids[0], ids[1])
    }
}

/// Lookup type: `(bill_identifier, provision_index) → (amount_status, match_tier)`.
///
/// Both inner `Option`s are `&'static str` so callers don't need to manage
/// owned strings.
type VerificationLookup<'a> =
    HashMap<(&'a str, usize), (Option<&'static str>, Option<&'static str>)>;

/// Build a lookup of verification status by `(bill_identifier, provision_index)`.
fn build_verification_lookup(bills: &[LoadedBill]) -> VerificationLookup<'_> {
    let mut lookup: VerificationLookup<'_> = HashMap::new();

    for loaded in bills {
        let bill_id = loaded.extraction.bill.identifier.as_str();

        if let Some(ref ver) = loaded.verification {
            let mut amount_status: HashMap<usize, &'static str> = HashMap::new();
            for check in &ver.amount_checks {
                let status_str: &'static str = match check.status {
                    CheckResult::Verified => "found",
                    CheckResult::Ambiguous => "found_multiple",
                    CheckResult::NotFound => "not_found",
                    _ => continue,
                };
                amount_status.insert(check.provision_index, status_str);
            }

            let mut tier_status: HashMap<usize, &'static str> = HashMap::new();
            for check in &ver.raw_text_checks {
                let tier_str: &'static str = match check.match_tier {
                    MatchTier::Exact => "exact",
                    MatchTier::Normalized => "normalized",
                    MatchTier::Spaceless => "spaceless",
                    MatchTier::NoMatch => "no_match",
                };
                tier_status.insert(check.provision_index, tier_str);
            }

            for i in 0..loaded.extraction.provisions.len() {
                let verified = amount_status.get(&i).copied();
                let tier = tier_status.get(&i).copied();
                lookup.insert((bill_id, i), (verified, tier));
            }
        }
    }

    lookup
}

// ─── Embedding Text ──────────────────────────────────────────────────────────

/// Build the text to embed for a provision. Deterministic.
pub fn build_embedding_text(provision: &Provision) -> String {
    let mut parts = Vec::new();
    let acct = provision.account_name();
    if !acct.is_empty() {
        parts.push(format!("Account: {acct}"));
    }
    let agency = provision.agency();
    if !agency.is_empty() {
        parts.push(format!("Agency: {agency}"));
    }
    let desc = provision.description();
    if !desc.is_empty() {
        parts.push(format!("Description: {desc}"));
    }
    let raw = provision.raw_text();
    if !raw.is_empty() {
        parts.push(format!("Text: {raw}"));
    }
    parts.join(" | ")
}

// ─── Relate ──────────────────────────────────────────────────────────────────

/// A match found by `relate()` — one provision similar to the source.
#[derive(Debug, Serialize)]
pub struct RelateMatch {
    /// Deterministic 8-char hex hash for future link persistence.
    pub hash: String,
    pub bill_identifier: String,
    pub bill_dir: String,
    pub provision_index: usize,
    pub similarity: f32,
    pub account_name: String,
    pub agency: String,
    pub dollars: Option<i64>,
    pub provision_type: String,
    /// "verified" (name match), "high" (sim>=0.65 + same normalized agency),
    /// "uncertain" (0.55-0.65 or name mismatch)
    pub confidence: &'static str,
    /// Funding timing from bill_meta, if available.
    pub timing: Option<String>,
    /// The FY the money becomes available, if advance.
    pub available_fy: Option<u32>,
}

/// One row in the fiscal year timeline produced by `relate --fy-timeline`.
#[derive(Debug, Serialize)]
pub struct FyTimelineEntry {
    pub fy: u32,
    pub current_year_ba: i64,
    pub advance_ba: i64,
    pub supplemental_ba: i64,
    pub source_bills: Vec<String>,
}

/// Full report from the `relate` command.
#[derive(Debug, Serialize)]
pub struct RelateReport {
    pub source_bill: String,
    pub source_index: usize,
    pub source_account: String,
    pub source_dollars: Option<i64>,
    /// High-confidence matches (name match or high similarity + same agency).
    pub same_account: Vec<RelateMatch>,
    /// Lower-confidence matches (uncertain zone).
    pub related: Vec<RelateMatch>,
    /// Fiscal year timeline, if requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Vec<FyTimelineEntry>>,
}

/// Compute a deterministic 8-char hex hash for a link candidate.
///
/// The hash is derived from source bill, source index, target bill, target
/// index, and the embedding model name. This ensures:
/// - Same inputs always produce the same hash (deterministic)
/// - Re-embedding with a different model invalidates old hashes
pub fn compute_link_hash(
    src_bill_dir: &str,
    src_idx: usize,
    tgt_bill_dir: &str,
    tgt_idx: usize,
    model: &str,
) -> String {
    let data = format!("{src_bill_dir}:{src_idx}\u{2192}{tgt_bill_dir}:{tgt_idx}:{model}");
    let digest = Sha256::digest(data.as_bytes());
    format!("{:x}", digest)[..8].to_string()
}

/// Deep-dive on one provision: find similar provisions across all bills,
/// group by confidence, and optionally build a fiscal year timeline.
///
/// This is the library function behind the `relate` CLI command. It takes
/// pre-loaded bills and embeddings to avoid I/O.
///
/// # Arguments
/// - `source_bill_dir`: directory name of the source bill (e.g., "hr9468")
/// - `source_idx`: provision index within that bill
/// - `bills`: all loaded bills
/// - `bill_embeddings`: embeddings for each bill (parallel to `bills`), `None` if unavailable
/// - `top_n`: max results per confidence tier
/// - `build_timeline`: whether to compute the FY timeline
pub fn relate(
    source_bill_dir: &str,
    source_idx: usize,
    bills: &[LoadedBill],
    bill_embeddings: &[Option<LoadedEmbeddings>],
    top_n: usize,
    build_timeline: bool,
) -> anyhow::Result<RelateReport> {
    // Find the source bill and provision
    let source_bill_pos = bills
        .iter()
        .position(|b| {
            b.dir
                .file_name()
                .is_some_and(|n| n.to_string_lossy() == source_bill_dir)
        })
        .ok_or_else(|| anyhow::anyhow!("Bill directory '{source_bill_dir}' not found"))?;

    let source_bill = &bills[source_bill_pos];
    let source_provision = source_bill
        .extraction
        .provisions
        .get(source_idx)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Provision index {source_idx} out of range (bill has {} provisions)",
                source_bill.extraction.provisions.len()
            )
        })?;

    let source_account = source_provision.account_name().to_string();
    let source_dollars = source_provision.amount().and_then(|a| a.dollars());
    let source_bill_id = source_bill.extraction.bill.identifier.clone();

    // Get the source embedding vector
    let source_emb = bill_embeddings[source_bill_pos]
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No embeddings found for {source_bill_dir}"))?;
    anyhow::ensure!(
        source_idx < source_emb.count(),
        "Provision index {source_idx} out of range for embeddings (count={})",
        source_emb.count()
    );
    let source_vec = source_emb.vector(source_idx);
    let embedding_model = source_emb.metadata.model.clone();

    // Compute similarity against every provision in every bill
    let source_canonical = bill_meta::normalize_account_name(&source_account);
    let source_norm_agency = normalize_agency(source_provision.agency());

    let mut all_scored: Vec<RelateMatch> = Vec::new();

    for (bill_pos, bill) in bills.iter().enumerate() {
        let Some(emb) = &bill_embeddings[bill_pos] else {
            continue;
        };

        let bill_id = bill.extraction.bill.identifier.as_str();
        let bill_dir_name = bill
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Build timing lookup for this bill
        let timing_map: HashMap<usize, (&FundingTiming, Option<u32>)> = bill
            .bill_meta
            .as_ref()
            .map(|m| {
                m.provision_timing
                    .iter()
                    .map(|t| (t.provision_index, (&t.timing, t.available_fy)))
                    .collect()
            })
            .unwrap_or_default();

        for (idx, provision) in bill.extraction.provisions.iter().enumerate() {
            // Skip the source provision itself
            if bill_pos == source_bill_pos && idx == source_idx {
                continue;
            }

            if idx >= emb.count() {
                break;
            }

            let sim = embeddings::cosine_similarity(source_vec, emb.vector(idx));
            if sim < 0.50 {
                continue;
            }

            let acct = provision.account_name().to_string();
            let canonical = bill_meta::normalize_account_name(&acct);
            let norm_agency = normalize_agency(provision.agency());

            // Determine confidence tier
            let name_match = !canonical.is_empty()
                && !source_canonical.is_empty()
                && canonical == source_canonical;
            let same_agency = norm_agency == source_norm_agency;

            let confidence = if name_match {
                "verified"
            } else if sim >= 0.65 && same_agency {
                "high"
            } else {
                "uncertain"
            };

            // Only include if above threshold for the tier
            if confidence == "uncertain" && sim < 0.55 {
                continue;
            }

            let dollars = provision.amount().and_then(|a| a.dollars());

            let (timing, available_fy) = timing_map
                .get(&idx)
                .map(|(t, fy)| (Some(format!("{t:?}").to_lowercase()), *fy))
                .unwrap_or((None, None));

            let hash = compute_link_hash(
                source_bill_dir,
                source_idx,
                &bill_dir_name,
                idx,
                &embedding_model,
            );

            all_scored.push(RelateMatch {
                hash,
                bill_identifier: bill_id.to_string(),
                bill_dir: bill_dir_name.clone(),
                provision_index: idx,
                similarity: sim,
                account_name: acct,
                agency: provision.agency().to_string(),
                dollars,
                provision_type: provision.type_str().to_string(),
                confidence,
                timing,
                available_fy,
            });
        }
    }

    // Sort by similarity descending
    all_scored.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Split into tiers
    let mut same_account: Vec<RelateMatch> = Vec::new();
    let mut related: Vec<RelateMatch> = Vec::new();

    for m in all_scored {
        match m.confidence {
            "verified" | "high" => {
                if same_account.len() < top_n {
                    same_account.push(m);
                }
            }
            _ => {
                if related.len() < top_n {
                    related.push(m);
                }
            }
        }
    }

    // Build FY timeline if requested
    let timeline = if build_timeline {
        let mut fy_map: HashMap<u32, (i64, i64, i64, Vec<String>)> = HashMap::new();

        // Include the source provision in the timeline
        if let Some(dollars) = source_dollars {
            let source_fys = &source_bill.extraction.bill.fiscal_years;
            let source_timing = source_bill
                .bill_meta
                .as_ref()
                .and_then(|m| {
                    m.provision_timing
                        .iter()
                        .find(|t| t.provision_index == source_idx)
                })
                .map(|t| &t.timing);

            for &fy in source_fys {
                let entry = fy_map.entry(fy).or_insert((0, 0, 0, Vec::new()));
                match source_timing {
                    Some(FundingTiming::Advance) => entry.1 += dollars,
                    Some(FundingTiming::Supplemental) => entry.2 += dollars,
                    _ => entry.0 += dollars,
                }
                let bill_id = &source_bill.extraction.bill.identifier;
                if !entry.3.contains(bill_id) {
                    entry.3.push(bill_id.clone());
                }
            }
        }

        // Include same_account matches in the timeline
        for m in &same_account {
            let bill = bills.iter().find(|b| {
                b.dir
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy() == m.bill_dir)
            });
            if let Some(bill) = bill {
                let fys = &bill.extraction.bill.fiscal_years;
                if let Some(dollars) = m.dollars {
                    for &fy in fys {
                        let entry = fy_map.entry(fy).or_insert((0, 0, 0, Vec::new()));
                        let timing_str = m.timing.as_deref().unwrap_or("current_year");
                        match timing_str {
                            "advance" => entry.1 += dollars,
                            "supplemental" => entry.2 += dollars,
                            _ => entry.0 += dollars,
                        }
                        if !entry.3.contains(&m.bill_identifier) {
                            entry.3.push(m.bill_identifier.clone());
                        }
                    }
                }
            }
        }

        let mut timeline: Vec<FyTimelineEntry> = fy_map
            .into_iter()
            .map(
                |(fy, (current, advance, supplemental, bills))| FyTimelineEntry {
                    fy,
                    current_year_ba: current,
                    advance_ba: advance,
                    supplemental_ba: supplemental,
                    source_bills: bills,
                },
            )
            .collect();
        timeline.sort_by_key(|e| e.fy);
        Some(timeline)
    } else {
        None
    };

    Ok(RelateReport {
        source_bill: source_bill_id,
        source_index: source_idx,
        source_account,
        source_dollars,
        same_account,
        related,
        timeline,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_quality_strong() {
        assert_eq!(compute_quality(Some("found"), Some("exact")), "strong");
    }

    #[test]
    fn test_compute_quality_moderate() {
        assert_eq!(
            compute_quality(Some("found"), Some("normalized")),
            "moderate"
        );
        assert_eq!(
            compute_quality(Some("found"), Some("spaceless")),
            "moderate"
        );
        assert_eq!(
            compute_quality(Some("found_multiple"), Some("exact")),
            "moderate"
        );
        assert_eq!(compute_quality(Some("found"), Some("no_match")), "moderate");
    }

    #[test]
    fn test_compute_quality_weak() {
        assert_eq!(compute_quality(Some("not_found"), Some("exact")), "weak");
        assert_eq!(compute_quality(Some("not_found"), None), "weak");
        assert_eq!(
            compute_quality(Some("found_multiple"), Some("no_match")),
            "weak"
        );
        assert_eq!(
            compute_quality(Some("found_multiple"), Some("spaceless")),
            "weak"
        );
    }

    #[test]
    fn test_compute_quality_na() {
        assert_eq!(compute_quality(None, None), "n/a");
        assert_eq!(compute_quality(None, Some("exact")), "n/a");
    }

    // ── Agency Normalization ──

    #[test]
    fn test_normalize_agency_parent_department() {
        assert_eq!(
            normalize_agency("Maritime Administration"),
            "department of transportation"
        );
    }

    #[test]
    fn test_normalize_agency_already_parent() {
        assert_eq!(
            normalize_agency("Department of Transportation"),
            "department of transportation"
        );
    }

    #[test]
    fn test_normalize_agency_comma_oig() {
        assert_eq!(
            normalize_agency("Office of Inspector General, Department of Transportation"),
            "department of transportation"
        );
    }

    #[test]
    fn test_normalize_agency_comma_secretary() {
        // "Department of Transportation, Office of the Secretary" →
        // comma extraction takes "Department of Transportation" →
        // not in sub-agency table → stays as is
        assert_eq!(
            normalize_agency("Department of Transportation, Office of the Secretary"),
            "department of transportation"
        );
    }

    #[test]
    fn test_normalize_agency_interior_sub() {
        assert_eq!(
            normalize_agency("National Park Service"),
            "department of the interior"
        );
        assert_eq!(
            normalize_agency("Bureau of Land Management"),
            "department of the interior"
        );
    }

    #[test]
    fn test_normalize_agency_unknown_passthrough() {
        assert_eq!(
            normalize_agency("Environmental Protection Agency"),
            "environmental protection agency"
        );
    }

    // ── Account Normalization ──

    #[test]
    fn test_normalize_account_name_plain() {
        assert_eq!(
            normalize_account_name("Salaries and Expenses"),
            "salaries and expenses"
        );
    }

    #[test]
    fn test_normalize_account_name_with_em_dash() {
        assert_eq!(
            normalize_account_name("Department of Defense\u{2014}Salaries and Expenses"),
            "salaries and expenses"
        );
    }

    #[test]
    fn test_normalize_account_name_with_en_dash() {
        assert_eq!(
            normalize_account_name("DoD\u{2013}Operations"),
            "operations"
        );
    }

    #[test]
    fn test_normalize_account_name_case_insensitive() {
        // These three variants should all normalize to the same string
        assert_eq!(
            normalize_account_name("Grants-In-Aid for Airports"),
            normalize_account_name("Grants-in-Aid for Airports")
        );
        assert_eq!(
            normalize_account_name("Grants-In-Aid for Airports"),
            normalize_account_name("Grants-in-aid for Airports")
        );
    }

    #[test]
    fn test_extract_parent_department_simple() {
        assert_eq!(
            extract_parent_department("Department of Defense"),
            "Department of Defense"
        );
    }

    #[test]
    fn test_extract_parent_department_with_comma() {
        assert_eq!(
            extract_parent_department("Corps of Engineers\u{2014}Civil, Department of Defense"),
            "Corps of Engineers\u{2014}Civil"
        );
    }

    #[test]
    fn test_extract_parent_department_oig_exception() {
        assert_eq!(
            extract_parent_department("Office of Inspector General, Department of Defense"),
            "Department of Defense"
        );
    }

    #[test]
    fn test_describe_bills_empty() {
        assert_eq!(describe_bills(&[]), "");
    }

    #[test]
    fn test_search_filter_default_is_permissive() {
        let filter = SearchFilter::default();
        assert!(filter.provision_type.is_none());
        assert!(filter.agency.is_none());
        assert!(filter.account.is_none());
        assert!(filter.keyword.is_none());
        assert!(filter.bill.is_none());
        assert!(filter.division.is_none());
        assert!(filter.min_dollars.is_none());
        assert!(filter.max_dollars.is_none());
    }

    #[test]
    fn test_summarize_empty() {
        let bills: Vec<LoadedBill> = Vec::new();
        let summaries = summarize(&bills);
        assert!(summaries.is_empty());
    }

    #[test]
    fn test_audit_empty() {
        let bills: Vec<LoadedBill> = Vec::new();
        let rows = audit(&bills);
        assert!(rows.is_empty());
    }

    #[test]
    fn test_rollup_by_department_empty() {
        let bills: Vec<LoadedBill> = Vec::new();
        let rollups = rollup_by_department(&bills);
        assert!(rollups.is_empty());
    }

    #[test]
    fn test_search_empty() {
        let bills: Vec<LoadedBill> = Vec::new();
        let filter = SearchFilter::default();
        let results = search(&bills, &filter);
        assert!(results.is_empty());
    }

    #[test]
    fn test_compare_empty() {
        let base: Vec<LoadedBill> = Vec::new();
        let current: Vec<LoadedBill> = Vec::new();
        let result = compare(&base, &current, None);
        assert!(result.rows.is_empty());
        assert!(result.cross_type_warning.is_none());
    }

    #[test]
    fn test_build_embedding_text_appropriation() {
        let provision = Provision::Appropriation {
            account_name: "Salaries and Expenses".to_string(),
            agency: Some("Department of Defense".to_string()),
            program: None,
            amount: crate::approp::ontology::DollarAmount::from_dollars(
                1_000_000,
                AmountSemantics::NewBudgetAuthority,
                "$1,000,000",
            ),
            fiscal_year: None,
            availability: None,
            provisos: vec![],
            earmarks: vec![],
            section: String::new(),
            division: None,
            title: None,
            confidence: 0.9,
            raw_text: "For salaries and expenses, $1,000,000.".to_string(),
            notes: vec![],
            cross_references: vec![],
            detail_level: String::new(),
            parent_account: None,
        };
        let text = build_embedding_text(&provision);
        assert!(text.contains("Account: Salaries and Expenses"));
        assert!(text.contains("Agency: Department of Defense"));
        assert!(text.contains("Text: For salaries and expenses"));
        assert!(text.contains(" | "));
    }

    #[test]
    fn test_build_embedding_text_directive_no_account() {
        let provision = Provision::Directive {
            description: "Report on spending".to_string(),
            deadlines: vec![],
            section: String::new(),
            division: None,
            title: None,
            confidence: 0.8,
            raw_text: "The Secretary shall report...".to_string(),
            notes: vec![],
            cross_references: vec![],
        };
        let text = build_embedding_text(&provision);
        // Directives have no account_name or agency
        assert!(!text.contains("Account:"));
        assert!(!text.contains("Agency:"));
        assert!(text.contains("Description: Report on spending"));
        assert!(text.contains("Text: The Secretary shall report..."));
    }
}
