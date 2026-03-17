//! Query operations over loaded bill data.
//!
//! These functions take `&[LoadedBill]` and return plain data structs
//! suitable for any output format. The CLI layer handles formatting.

use crate::approp::loading::LoadedBill;
use crate::approp::ontology::{AmountSemantics, Provision};
use crate::approp::verification::{CheckResult, MatchTier};
use serde::Serialize;
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
            BillSummary {
                identifier: loaded.extraction.bill.identifier.clone(),
                classification: format!("{}", loaded.extraction.bill.classification),
                provisions: loaded.extraction.provisions.len(),
                budget_authority: ba,
                rescissions,
                net_ba: ba - rescissions,
                completeness_pct: completeness,
            }
        })
        .collect()
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
    pub account: String,
    pub agency: String,
    pub base_amount: i64,
    pub current_amount: i64,
    pub delta: i64,
    pub delta_pct: Option<f64>,
    /// One of `"changed"`, `"only in base"`, `"only in current"`, `"unchanged"`.
    pub status: String,
}

/// The full result of comparing two sets of bills.
#[derive(Debug)]
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
        let base_val = base_accounts.get(key).copied().unwrap_or(0);

        // Look up in current — try exact match first, then suffix match
        let current_val = current_accounts
            .get(key)
            .copied()
            .or_else(|| {
                let short = normalize_account_name(&key.1);
                current_accounts
                    .iter()
                    .find(|(k, _)| k.0 == key.0 && normalize_account_name(&k.1) == short)
                    .map(|(_, v)| *v)
            })
            .unwrap_or(0);

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

        rows.push(CompareRow {
            account: key.1.clone(),
            agency: key.0.clone(),
            base_amount: base_val,
            current_amount: current_val,
            delta,
            delta_pct,
            status: status.to_string(),
        });
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

/// Build a map of `(agency, account_name) → total dollars` for appropriation
/// provisions with `NewBudgetAuthority` semantics.
fn build_account_map(
    bills: &[LoadedBill],
    agency_filter: Option<&str>,
) -> HashMap<(String, String), i64> {
    let mut accounts: HashMap<(String, String), i64> = HashMap::new();
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
                let key = (ag.to_string(), p.account_name().to_string());
                *accounts.entry(key).or_insert(0) += amt.dollars().unwrap_or(0);
            }
        }
    }
    accounts
}

/// Normalize account name for fuzzy cross-bill matching.
///
/// Strips hierarchical prefixes separated by em-dash or en-dash.
fn normalize_account_name(name: &str) -> String {
    let parts: Vec<&str> = name.split(&['\u{2014}', '\u{2013}'][..]).collect();
    if parts.len() > 1 {
        return parts.last().unwrap_or(&name).trim().to_string();
    }
    name.trim().to_string()
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

    #[test]
    fn test_normalize_account_name_plain() {
        assert_eq!(
            normalize_account_name("Salaries and Expenses"),
            "Salaries and Expenses"
        );
    }

    #[test]
    fn test_normalize_account_name_with_em_dash() {
        assert_eq!(
            normalize_account_name("Department of Defense\u{2014}Salaries and Expenses"),
            "Salaries and Expenses"
        );
    }

    #[test]
    fn test_normalize_account_name_with_en_dash() {
        assert_eq!(
            normalize_account_name("DoD\u{2013}Operations"),
            "Operations"
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
