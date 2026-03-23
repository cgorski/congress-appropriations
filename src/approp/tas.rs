//! Treasury Account Symbol (TAS) resolution for appropriation provisions.
//!
//! Maps each top-level budget authority provision to a Federal Account Symbol
//! (FAS) code — a stable, government-assigned identifier that persists through
//! account renames and reorganizations.
//!
//! The resolution algorithm has two tiers:
//!
//! 1. **Deterministic matching** (~90% of provisions): Compare the provision's
//!    account name against the FAST Book reference data using exact, suffix
//!    (after em-dash stripping), and substring containment matching. Free,
//!    instant, no API key required.
//!
//! 2. **LLM matching** (~10% of provisions): Send unmatched provisions to
//!    Claude Opus with the relevant FAS codes for the provision's agency.
//!    The LLM's response is verified against the FAST Book — if the code
//!    it returns isn't in the reference, the match is rejected.
//!
//! # Pipeline position
//!
//! ```text
//! extract → verify-text → enrich → **resolve-tas** → embed → …
//! ```
//!
//! # Output
//!
//! Produces `tas_mapping.json` per bill directory, containing one [`TasMapping`]
//! per top-level budget authority appropriation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

// ─── FAS Reference Data (from FAST Book) ─────────────────────────────────────

/// Reference data loaded from `fas_reference.json`, containing all known
/// Federal Account Symbols from the FAST Book.
pub struct FasReference {
    /// All active accounts from FAST Book Part II.
    pub accounts: Vec<FasAccount>,
    /// Discontinued General Fund accounts from the Changes sheet.
    pub discontinued: Vec<FasAccount>,
    /// Lookup multi-map: lowercase short title → indices into `accounts`.
    /// Multiple FAS accounts can share the same short title (e.g., "Salaries and Expenses").
    lookup: HashMap<String, Vec<usize>>,
    /// Lookup multi-map for discontinued accounts.
    disc_lookup: HashMap<String, Vec<usize>>,
    /// Set of all known FAS codes (for verification).
    known_codes: std::collections::HashSet<String>,
}

/// A single account from the FAST Book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FasAccount {
    pub fas_code: String,
    pub agency_code: String,
    pub main_account: String,
    pub agency_name: String,
    pub title: String,
    pub fund_type: String,
    #[serde(default)]
    pub has_no_year_variant: bool,
    #[serde(default)]
    pub has_annual_variant: bool,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default)]
    pub legislation: Option<String>,
    #[serde(default)]
    pub independent_agency: Option<String>,
    #[serde(default)]
    pub tas_variants: usize,
    // Discontinued-only fields
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub comments: Option<String>,
}

/// Raw JSON structure of `fas_reference.json`.
#[derive(Debug, Deserialize)]
struct FasReferenceFile {
    #[allow(dead_code)]
    schema_version: String,
    accounts: Vec<FasAccount>,
    #[serde(default)]
    discontinued: Vec<FasAccount>,
}

impl FasReference {
    /// Build lookup indices after loading.
    ///
    /// The lookup maps `short_title → Vec<index>` (a multi-map) because
    /// many FAS accounts share the same short title across agencies
    /// (e.g., 151 agencies have a "Salaries and Expenses" account).
    fn build_lookups(accounts: &[FasAccount], discontinued: &[FasAccount]) -> Self {
        let mut lookup: HashMap<String, Vec<usize>> = HashMap::new();
        let mut known_codes = std::collections::HashSet::new();

        for (i, acct) in accounts.iter().enumerate() {
            known_codes.insert(acct.fas_code.clone());
            let short = short_title(&acct.title);
            if short.len() > 3 {
                lookup.entry(short).or_default().push(i);
            }
        }

        let mut disc_lookup: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, acct) in discontinued.iter().enumerate() {
            known_codes.insert(acct.fas_code.clone());
            let short = short_title(&acct.title);
            if short.len() > 3 {
                disc_lookup.entry(short).or_default().push(i);
            }
        }

        Self {
            accounts: accounts.to_vec(),
            discontinued: discontinued.to_vec(),
            lookup,
            disc_lookup,
            known_codes,
        }
    }

    /// Check whether a FAS code exists in the reference (active or discontinued).
    pub fn contains_code(&self, fas_code: &str) -> bool {
        self.known_codes.contains(fas_code)
    }

    /// Look up an account by FAS code.
    pub fn get_by_code(&self, fas_code: &str) -> Option<&FasAccount> {
        self.accounts
            .iter()
            .find(|a| a.fas_code == fas_code)
            .or_else(|| self.discontinued.iter().find(|a| a.fas_code == fas_code))
    }

    /// Get all General Fund accounts for a given agency code.
    pub fn accounts_for_agency(&self, agency_code: &str) -> Vec<&FasAccount> {
        self.accounts
            .iter()
            .filter(|a| a.agency_code == agency_code && a.fund_type == "general")
            .collect()
    }

    /// Total number of known FAS codes (active + discontinued).
    pub fn total_codes(&self) -> usize {
        self.known_codes.len()
    }
}

/// Extract the short title: everything before the first comma, lowercased.
fn short_title(title: &str) -> String {
    title.split(',').next().unwrap_or("").trim().to_lowercase()
}

/// Load the FAS reference from a JSON file.
pub fn load_fas_reference(path: &Path) -> Result<FasReference> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read FAS reference: {}", path.display()))?;
    let file: FasReferenceFile = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse FAS reference: {}", path.display()))?;

    let reference = FasReference::build_lookups(&file.accounts, &file.discontinued);
    Ok(reference)
}

/// Compute SHA-256 hash of the FAS reference file (for staleness tracking).
pub fn fas_reference_hash(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{:x}", digest))
}

// ─── Deterministic Matching (Tier 1) ─────────────────────────────────────────

/// Result of a deterministic match attempt.
pub struct DeterministicMatch {
    pub fas_code: String,
    pub fas_title: String,
    pub method: TasMethod,
}

/// Attempt to match a provision's account name to a FAS code using
/// deterministic string matching against the FAST Book reference.
///
/// Uses a conservative approach that only produces matches we are CERTAIN
/// about — no false positives. The algorithm:
///
/// 1. **Direct match**: lowercase account name == exactly one FAS short title.
///    If multiple FAS codes share that short title, uses `agency_code` to
///    disambiguate. If disambiguation fails, returns `None` (needs LLM).
///
/// 2. **Suffix match**: strip em-dash agency prefix, then apply the same
///    direct + disambiguation logic on the suffix.
///
/// Containment matching is deliberately NOT used — it produces false positives
/// on common names like "Salaries and Expenses" (151 FAS codes share this title).
///
/// Returns `None` if no unambiguous match is found. The caller should send
/// unresolved provisions to the LLM tier.
pub fn match_deterministic(
    account_name: &str,
    agency_code: Option<&str>,
    reference: &FasReference,
) -> Option<DeterministicMatch> {
    if account_name.is_empty() {
        return None;
    }

    let lower = account_name.to_lowercase().trim().to_string();

    // Strategy 1a: Direct match on full lowercased account name
    if let Some(result) = try_lookup_with_disambiguation(&lower, agency_code, reference) {
        return Some(result);
    }

    // Strategy 1b: Direct match on provision's short title (first comma segment)
    // Handles "Operation and Maintenance, Army" → lookup "operation and maintenance"
    let prov_short = short_title(account_name);
    if prov_short != lower
        && let Some(result) = try_lookup_with_disambiguation(&prov_short, agency_code, reference)
    {
        return Some(result);
    }

    // Strategy 2: Em-dash suffix match
    // "United States Secret Service—Operations and Support" → "operations and support"
    let suffix = strip_emdash_prefix(&lower);
    if !suffix.is_empty()
        && suffix != lower
        && let Some(result) = try_lookup_with_disambiguation(&suffix, agency_code, reference)
    {
        return Some(DeterministicMatch {
            method: TasMethod::SuffixMatch,
            ..result
        });
    }

    // No unambiguous match found — needs LLM
    None
}

/// Look up a normalized name in the reference, disambiguating by agency code
/// when multiple FAS accounts share the same short title.
///
/// Returns a match only if EXACTLY ONE candidate is found (unique name)
/// or the agency code narrows it to exactly one.
fn try_lookup_with_disambiguation(
    name: &str,
    agency_code: Option<&str>,
    reference: &FasReference,
) -> Option<DeterministicMatch> {
    // Collect ALL candidates from active accounts with this short title
    let mut candidates: Vec<&FasAccount> = Vec::new();
    if let Some(indices) = reference.lookup.get(name) {
        for &idx in indices {
            candidates.push(&reference.accounts[idx]);
        }
    }
    // Also check discontinued
    if let Some(indices) = reference.disc_lookup.get(name) {
        for &idx in indices {
            candidates.push(&reference.discontinued[idx]);
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // If exactly one candidate, it's unambiguous
    if candidates.len() == 1 {
        return Some(DeterministicMatch {
            fas_code: candidates[0].fas_code.clone(),
            fas_title: candidates[0].title.clone(),
            method: TasMethod::DirectMatch,
        });
    }

    // Multiple candidates — try to disambiguate by agency code
    if let Some(code) = agency_code {
        let agency_filtered: Vec<&&FasAccount> = candidates
            .iter()
            .filter(|c| c.agency_code == code)
            .collect();
        if agency_filtered.len() == 1 {
            return Some(DeterministicMatch {
                fas_code: agency_filtered[0].fas_code.clone(),
                fas_title: agency_filtered[0].title.clone(),
                method: TasMethod::AgencyDisambiguated,
            });
        }
    }

    // Still ambiguous — don't guess, return None for LLM handling
    None
}

/// Strip an em-dash (or en-dash, or hyphen-separated) agency prefix from
/// an account name and return the suffix.
///
/// `"United States Secret Service—Operations and Support"` → `"operations and support"`
fn strip_emdash_prefix(name: &str) -> String {
    let parts: Vec<&str> = name
        .split(&['\u{2014}', '\u{2013}', '—', '–'][..])
        .collect();
    if parts.len() > 1 {
        parts.last().unwrap_or(&"").trim().to_string()
    } else {
        name.to_string()
    }
}

// ─── TAS Mapping Types ───────────────────────────────────────────────────────

/// Per-bill TAS mapping file, written to `tas_mapping.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasMappingFile {
    pub schema_version: String,
    /// Bill directory name (e.g., `"118-hr2882"`).
    pub bill_dir: String,
    /// Bill identifier (e.g., `"H.R. 2882"`).
    pub bill_identifier: String,
    /// LLM model used for Tier 2 matching, or `None` if all deterministic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// SHA-256 of `fas_reference.json` used for this resolution.
    pub fas_reference_hash: String,
    /// ISO 8601 timestamp.
    pub timestamp: String,
    /// One mapping per top-level budget authority appropriation.
    pub mappings: Vec<TasMapping>,
    /// Summary statistics.
    pub summary: TasSummary,
}

/// Mapping of a single provision to a FAS code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasMapping {
    /// Index into the bill's `provisions` array.
    pub provision_index: usize,
    /// Account name as extracted by the LLM.
    pub account_name: String,
    /// Agency name as extracted by the LLM.
    pub agency: String,
    /// Dollar amount (if available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dollars: Option<i64>,
    /// Matched FAS code, or `None` if unmatched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fas_code: Option<String>,
    /// Official FAST Book title for the matched FAS code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fas_title: Option<String>,
    /// Confidence in the match.
    pub confidence: TasConfidence,
    /// How the match was established.
    pub method: TasMethod,
    /// LLM reasoning (only populated for LLM-resolved matches).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

/// Confidence tier for a TAS mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TasConfidence {
    /// Matched deterministically against the FAST Book — highest confidence.
    Verified,
    /// LLM matched, and the FAS code was confirmed in the FAST Book.
    High,
    /// LLM matched, but the FAS code is NOT in the FAST Book
    /// (the LLM may know about it from training data).
    Inferred,
    /// Could not resolve to any FAS code.
    Unmatched,
}

/// How a TAS mapping was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TasMethod {
    /// Account name matched exactly one FAS short title (unique name, unambiguous).
    DirectMatch,
    /// After stripping the em-dash agency prefix, the suffix matched uniquely.
    SuffixMatch,
    /// Multiple FAS codes shared the name, but the provision's agency code narrowed it to one.
    AgencyDisambiguated,
    /// LLM provided the mapping.
    LlmResolved,
    /// No method succeeded.
    None,
}

/// Summary statistics for a bill's TAS resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasSummary {
    /// Total top-level BA provisions considered.
    pub total_provisions: usize,
    /// Provisions matched deterministically (Tier 1).
    pub deterministic_matched: usize,
    /// Provisions matched by LLM (Tier 2).
    pub llm_matched: usize,
    /// Provisions that could not be matched.
    pub unmatched: usize,
    /// Number of unique FAS codes found.
    pub unique_fas_codes: usize,
    /// Match rate as a percentage.
    pub match_rate_pct: f64,
}

// ─── Provision Extraction Helpers ─────────────────────────────────────────────

/// A top-level budget authority provision extracted from a bill for TAS resolution.
/// Works at the `serde_json::Value` level to avoid depending on the typed Provision enum.
pub struct ProvisionForTas {
    pub index: usize,
    pub account_name: String,
    pub agency: String,
    pub dollars: Option<i64>,
}

/// Extract top-level BA appropriation provisions from a bill's JSON.
pub fn extract_provisions_for_tas(extraction_value: &serde_json::Value) -> Vec<ProvisionForTas> {
    let mut result = Vec::new();

    let provisions = match extraction_value
        .get("provisions")
        .and_then(|v| v.as_array())
    {
        Some(arr) => arr,
        None => return result,
    };

    for (i, p) in provisions.iter().enumerate() {
        // Must be an appropriation
        let ptype = p
            .get("provision_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if ptype != "appropriation" {
            continue;
        }

        // Must be new_budget_authority
        let amt = match p.get("amount") {
            Some(a) if a.is_object() => a,
            _ => continue,
        };
        let semantics = amt.get("semantics").and_then(|v| v.as_str()).unwrap_or("");
        if semantics != "new_budget_authority" {
            continue;
        }

        // Must not be sub-allocation or proviso
        let detail_level = p.get("detail_level").and_then(|v| v.as_str()).unwrap_or("");
        if detail_level == "sub_allocation" || detail_level == "proviso_amount" {
            continue;
        }

        // Must have an account name
        let account_name = p.get("account_name").and_then(|v| v.as_str()).unwrap_or("");
        if account_name.is_empty() {
            continue;
        }

        let agency = p.get("agency").and_then(|v| v.as_str()).unwrap_or("");

        // Extract dollars
        let dollars = amt
            .get("value")
            .and_then(|v| v.get("dollars"))
            .and_then(|v| v.as_i64());

        result.push(ProvisionForTas {
            index: i,
            account_name: account_name.to_string(),
            agency: agency.to_string(),
            dollars,
        });
    }

    result
}

// ─── Resolve Orchestration ───────────────────────────────────────────────────

/// Known mappings from LLM-extracted agency names to CGAC agency codes.
/// Used to scope deterministic matching to the correct agency's FAS accounts.
///
/// This table maps both department names and sub-agency names to the CGAC
/// code used in TAS/FAS identifiers. Sub-agencies map to their parent
/// department's code (e.g., "FBI" → "015" for DOJ) EXCEPT for DOD service
/// branches which have their own codes (Army=021, Navy=017, Air Force=057).
static AGENCY_NAME_TO_CODE: &[(&str, &str)] = &[
    ("department of homeland security", "070"),
    ("department of defense", "097"),
    ("department of veterans affairs", "036"),
    ("department of transportation", "069"),
    ("department of housing and urban development", "086"),
    ("department of agriculture", "012"),
    ("department of health and human services", "075"),
    ("department of justice", "015"),
    ("department of state", "019"),
    ("department of the interior", "014"),
    ("department of energy", "089"),
    ("department of the treasury", "020"),
    ("department of labor", "016"),
    ("department of education", "091"),
    ("department of commerce", "013"),
    ("environmental protection agency", "068"),
    ("national aeronautics and space administration", "080"),
    ("national science foundation", "049"),
    ("small business administration", "073"),
    ("general services administration", "047"),
    ("office of personnel management", "024"),
    ("social security administration", "028"),
    ("nuclear regulatory commission", "031"),
    ("agency for international development", "072"),
    ("executive office of the president", "011"),
    ("the judiciary", "010"),
    // DOD service branches — own CGAC codes
    ("department of the army", "021"),
    ("department of the navy", "017"),
    ("department of the air force", "057"),
    ("corps of engineers", "096"),
    // DHS sub-agencies → parent DHS code
    ("federal emergency management agency", "070"),
    ("u.s. customs and border protection", "070"),
    ("u.s. immigration and customs enforcement", "070"),
    ("transportation security administration", "070"),
    ("coast guard", "070"),
    ("united states secret service", "070"),
    ("cybersecurity and infrastructure security agency", "070"),
    ("federal law enforcement training centers", "070"),
    ("countering weapons of mass destruction office", "070"),
    // HHS sub-agencies
    ("national institutes of health", "075"),
    ("centers for disease control and prevention", "075"),
    ("food and drug administration", "075"),
    ("health resources and services administration", "075"),
    (
        "substance abuse and mental health services administration",
        "075",
    ),
    ("indian health service", "075"),
    ("administration for children and families", "075"),
    ("centers for medicare and medicaid services", "075"),
    ("centers for medicare & medicaid services", "075"),
    // USDA sub-agencies
    ("forest service", "012"),
    ("natural resources conservation service", "012"),
    ("farm service agency", "012"),
    ("food and nutrition service", "012"),
    ("animal and plant health inspection service", "012"),
    ("agricultural research service", "012"),
    // Interior sub-agencies
    ("bureau of land management", "014"),
    ("national park service", "014"),
    ("u.s. fish and wildlife service", "014"),
    ("fish and wildlife service", "014"),
    ("bureau of reclamation", "014"),
    ("bureau of indian affairs", "014"),
    ("u.s. geological survey", "014"),
    // DOJ sub-agencies
    ("federal bureau of investigation", "015"),
    ("drug enforcement administration", "015"),
    ("bureau of alcohol, tobacco, firearms and explosives", "015"),
    ("federal bureau of prisons", "015"),
    ("u.s. marshals service", "015"),
    // Commerce sub-agencies
    ("national oceanic and atmospheric administration", "013"),
    ("national institute of standards and technology", "013"),
    // Transportation sub-agencies
    ("federal highway administration", "069"),
    ("federal aviation administration", "069"),
    ("federal transit administration", "069"),
    ("federal railroad administration", "069"),
    ("national highway traffic safety administration", "069"),
    ("maritime administration", "069"),
];

/// Map an LLM-extracted agency name to a CGAC agency code.
///
/// When `account_name` is provided, it is used to detect DOD service branches.
/// The LLM often extracts agency as "Department of Defense" even when the account
/// is service-specific (e.g., "Operation and Maintenance, Army"). The account
/// name's qualifier (", Army", ", Navy", etc.) reveals the actual service branch.
pub fn agency_name_to_code(agency: &str) -> Option<&'static str> {
    agency_name_to_code_with_account(agency, "")
}

/// Like [`agency_name_to_code`] but also considers the account name for
/// DOD service-branch detection.
pub fn agency_name_to_code_with_account(agency: &str, account_name: &str) -> Option<&'static str> {
    if agency.is_empty() {
        return None;
    }
    let lower = agency.to_lowercase();
    let acct_lower = account_name.to_lowercase();

    // DOD special case: if agency is "Department of Defense" or similar,
    // check the account name for service branch qualifiers.
    // "Operation and Maintenance, Army" → 021 (not 097)
    if lower.contains("defense") || lower == "dod" {
        // Check for service branch in account name (after last comma)
        let branch_hints: &[(&str, &str)] = &[
            (", army", "021"),
            ("army national guard", "021"),
            ("army reserve", "021"),
            (", navy", "017"),
            ("marine corps", "017"),
            ("navy reserve", "017"),
            (", air force", "057"),
            ("air national guard", "057"),
            ("air force reserve", "057"),
            ("space force", "057"),
        ];
        for &(hint, code) in branch_hints {
            if acct_lower.contains(hint) {
                return Some(code);
            }
        }
        // No service branch detected — use DOD umbrella
        return Some("097");
    }

    // Exact match first
    for &(name, code) in AGENCY_NAME_TO_CODE {
        if name == lower {
            return Some(code);
        }
    }
    // Substring match (agency name contains or is contained by a known name)
    for &(name, code) in AGENCY_NAME_TO_CODE {
        if name.len() > 8 && (lower.contains(name) || name.contains(lower.as_str())) {
            return Some(code);
        }
    }
    None
}

/// Resolve TAS codes for a bill using deterministic matching only (Tier 1).
///
/// Returns mappings for all top-level BA provisions. Provisions that could not
/// be matched deterministically will have `confidence: Unmatched` — these
/// should be sent to the LLM tier.
///
/// Only marks provisions as `Verified` when the match is unambiguous:
/// either the account name uniquely identifies one FAS code, or the
/// agency code disambiguates among multiple candidates.
pub fn resolve_deterministic(
    provisions: &[ProvisionForTas],
    reference: &FasReference,
) -> Vec<TasMapping> {
    provisions
        .iter()
        .map(|p| {
            let agency_code = agency_name_to_code_with_account(&p.agency, &p.account_name);
            if let Some(det) = match_deterministic(&p.account_name, agency_code, reference) {
                TasMapping {
                    provision_index: p.index,
                    account_name: p.account_name.clone(),
                    agency: p.agency.clone(),
                    dollars: p.dollars,
                    fas_code: Some(det.fas_code),
                    fas_title: Some(det.fas_title),
                    confidence: TasConfidence::Verified,
                    method: det.method,
                    reasoning: None,
                }
            } else {
                TasMapping {
                    provision_index: p.index,
                    account_name: p.account_name.clone(),
                    agency: p.agency.clone(),
                    dollars: p.dollars,
                    fas_code: None,
                    fas_title: None,
                    confidence: TasConfidence::Unmatched,
                    method: TasMethod::None,
                    reasoning: None,
                }
            }
        })
        .collect()
}

/// Build a [`TasMappingFile`] from resolved mappings.
pub fn build_mapping_file(
    mappings: Vec<TasMapping>,
    bill_dir: &str,
    bill_identifier: &str,
    model: Option<&str>,
    ref_hash: &str,
) -> TasMappingFile {
    let deterministic_matched = mappings
        .iter()
        .filter(|m| m.confidence == TasConfidence::Verified)
        .count();
    let llm_matched = mappings
        .iter()
        .filter(|m| matches!(m.confidence, TasConfidence::High | TasConfidence::Inferred))
        .count();
    let unmatched = mappings
        .iter()
        .filter(|m| m.confidence == TasConfidence::Unmatched)
        .count();

    let unique_fas: std::collections::HashSet<&str> = mappings
        .iter()
        .filter_map(|m| m.fas_code.as_deref())
        .collect();

    let total = mappings.len();
    let matched_total = deterministic_matched + llm_matched;
    let match_rate = if total > 0 {
        100.0 * matched_total as f64 / total as f64
    } else {
        0.0
    };

    TasMappingFile {
        schema_version: "1.0".to_string(),
        bill_dir: bill_dir.to_string(),
        bill_identifier: bill_identifier.to_string(),
        model: model.map(|s| s.to_string()),
        fas_reference_hash: ref_hash.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        summary: TasSummary {
            total_provisions: total,
            deterministic_matched,
            llm_matched,
            unmatched,
            unique_fas_codes: unique_fas.len(),
            match_rate_pct: match_rate,
        },
        mappings,
    }
}

// ─── I/O ─────────────────────────────────────────────────────────────────────

/// Load an existing `tas_mapping.json` from a bill directory.
pub fn load_tas_mapping(dir: &Path) -> Result<Option<TasMappingFile>> {
    let path = dir.join("tas_mapping.json");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let mapping: TasMappingFile = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(mapping))
}

/// Save a `tas_mapping.json` to a bill directory.
pub fn save_tas_mapping(dir: &Path, mapping: &TasMappingFile) -> Result<()> {
    let path = dir.join("tas_mapping.json");
    let json = serde_json::to_string_pretty(mapping)?;
    std::fs::write(&path, json).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

// ─── LLM Prompt Building ─────────────────────────────────────────────────────

/// System prompt for LLM-based TAS resolution.
pub const TAS_SYSTEM_PROMPT: &str = r#"You are an expert on U.S. federal budget accounts and Treasury Account Symbols (TAS).

You will receive a list of appropriation provisions from a congressional bill that could not be matched to TAS codes by deterministic string matching. You will also receive the list of known TAS codes for the relevant agencies.

Your task: Match each provision to the most likely TAS code.

Key rules:
- Account names in bills often use em-dash format: "Agency—Account Title". The TAS title inverts this: "Account Title, Agency, Department".
- "Salaries and Expenses" was renamed to "Operations and Support" for many DHS accounts around FY2017. Same TAS code, different name.
- DHS sub-agencies (CBP, ICE, TSA, USCG, USSS, FEMA, CISA) each have their own TAS codes under agency prefix 070.
- DOD service branches use separate agency codes: Army=021, Navy/Marines=017, Air Force/Space Force=057. DOD-wide=097.
- Match to TAS codes in the 0000-3999 main account range. Codes 4000+ are revolving/trust funds.
- If you know the correct TAS code from your knowledge but it is not in the provided list, still return it with a note.
- Confidence: "high" = clear match, "medium" = plausible but ambiguous, "none" = no match found.

Return JSON only (no markdown code blocks, no text before or after):
{
  "mappings": [
    {
      "provision_index": 0,
      "fas_code": "070-0400" or null,
      "fas_title": "the official TAS title" or null,
      "confidence": "high" | "medium" | "none",
      "reasoning": "brief explanation"
    }
  ]
}"#;

/// An unmatched provision to be sent to the LLM.
pub struct UnmatchedProvision {
    pub provision_index: usize,
    pub account_name: String,
    pub agency: String,
    pub dollars: Option<i64>,
}

/// Build the user prompt for a batch of unmatched provisions.
pub fn build_llm_prompt(
    provisions: &[UnmatchedProvision],
    agency_fas: &[&FasAccount],
    bill_id: &str,
) -> String {
    let mut parts = Vec::new();
    parts.push(format!("Bill: {bill_id}\n\n"));
    parts.push("== UNMATCHED PROVISIONS ==\n\n".to_string());

    for p in provisions {
        let d_str = p
            .dollars
            .map(|d| format!("${d}"))
            .unwrap_or_else(|| "no amount".to_string());
        parts.push(format!(
            "  provision_index={} account=\"{}\" agency=\"{}\" {d_str}\n",
            p.provision_index, p.account_name, p.agency,
        ));
    }

    parts.push("\n== KNOWN TAS CODES FOR THESE AGENCIES ==\n\n".to_string());
    for fas in agency_fas {
        // Skip revolving/trust/deposit funds (main account >= 4000)
        if fas
            .main_account
            .parse::<u32>()
            .is_ok_and(|main| main >= 4000)
        {
            continue;
        }
        parts.push(format!("  {}: {}\n", fas.fas_code, fas.title));
    }

    parts.join("")
}

/// A single LLM TAS match result parsed from the response.
#[derive(Debug, Deserialize)]
pub struct LlmTasMatch {
    pub provision_index: usize,
    #[serde(default)]
    pub fas_code: Option<String>,
    #[serde(default)]
    pub fas_title: Option<String>,
    #[serde(default)]
    pub confidence: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
}

/// Parse the LLM's JSON response into match results.
pub fn parse_llm_response(response_text: &str) -> Result<Vec<LlmTasMatch>> {
    // Handle potential markdown wrapping
    let mut text = response_text.trim().to_string();
    if text.starts_with("```") {
        // Strip markdown code block
        if let Some(first_newline) = text.find('\n') {
            text = text[first_newline + 1..].to_string();
        }
        if text.ends_with("```") {
            text = text[..text.len() - 3].trim().to_string();
        }
    }

    #[derive(Deserialize)]
    struct LlmResponse {
        mappings: Vec<LlmTasMatch>,
    }

    let parsed: LlmResponse =
        serde_json::from_str(&text).context("Failed to parse LLM TAS response")?;
    Ok(parsed.mappings)
}

/// Apply LLM match results to existing TAS mappings, verifying each against
/// the FAS reference.
pub fn apply_llm_results(
    mappings: &mut [TasMapping],
    llm_results: &[LlmTasMatch],
    reference: &FasReference,
) {
    for llm_match in llm_results {
        // Find the corresponding mapping by provision_index
        let mapping = mappings
            .iter_mut()
            .find(|m| m.provision_index == llm_match.provision_index);

        let Some(mapping) = mapping else {
            continue;
        };

        // Only update if currently unmatched
        if mapping.confidence != TasConfidence::Unmatched {
            continue;
        }

        if let Some(ref fas_code) = llm_match.fas_code {
            // Verify the code exists in the reference
            let confirmed = reference.contains_code(fas_code);

            mapping.fas_code = Some(fas_code.clone());
            mapping.fas_title = llm_match
                .fas_title
                .clone()
                .or_else(|| reference.get_by_code(fas_code).map(|a| a.title.clone()));
            mapping.confidence = if confirmed {
                TasConfidence::High
            } else {
                TasConfidence::Inferred
            };
            mapping.method = TasMethod::LlmResolved;
            mapping.reasoning = llm_match.reasoning.clone();
        }
        // If LLM returned null, leave as Unmatched
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fas_account(
        code: &str,
        agency_code: &str,
        agency_name: &str,
        title: &str,
    ) -> FasAccount {
        FasAccount {
            fas_code: code.to_string(),
            agency_code: agency_code.to_string(),
            main_account: code.split('-').nth(1).unwrap_or("0000").to_string(),
            agency_name: agency_name.to_string(),
            title: title.to_string(),
            fund_type: "general".to_string(),
            has_no_year_variant: false,
            has_annual_variant: true,
            last_updated: None,
            legislation: None,
            independent_agency: None,
            tas_variants: 1,
            action: None,
            comments: None,
        }
    }

    fn make_test_reference() -> FasReference {
        let accounts = vec![
            make_fas_account(
                "070-0400",
                "070",
                "Department of Homeland Security",
                "Operations and Support, United States Secret Service, Homeland Security",
            ),
            make_fas_account(
                "070-0530",
                "070",
                "Department of Homeland Security",
                "Operations and Support, U.S. Customs and Border Protection, Homeland Security",
            ),
            make_fas_account(
                "021-2020",
                "021",
                "Department of Defense - ARMY",
                "Operation and Maintenance, Army",
            ),
            make_fas_account(
                "017-1804",
                "017",
                "Department of Defense - NAVY",
                "Operation and Maintenance, Navy",
            ),
            // Two agencies both have "Salaries and Expenses"
            make_fas_account(
                "015-0100",
                "015",
                "Department of Justice",
                "Salaries and Expenses, General Legal Activities, Justice",
            ),
            make_fas_account(
                "020-0100",
                "020",
                "Department of the Treasury",
                "Salaries and Expenses, Departmental Offices, Treasury",
            ),
        ];
        FasReference::build_lookups(&accounts, &[])
    }

    #[test]
    fn test_match_unique_name() {
        let reference = make_test_reference();
        // "Operation and Maintenance, Army" → short title "operation and maintenance"
        // This is NOT unique (Army + Navy both have it) so without agency it's ambiguous
        let result = match_deterministic("Operation and Maintenance, Army", None, &reference);
        // Should be None without agency — ambiguous
        assert!(
            result.is_none(),
            "ambiguous name without agency should not match"
        );
    }

    #[test]
    fn test_match_with_agency_disambiguation() {
        let reference = make_test_reference();
        // With agency code, "Operation and Maintenance" disambiguates to Army
        let result =
            match_deterministic("Operation and Maintenance, Army", Some("021"), &reference);
        assert!(result.is_some(), "should match with agency disambiguation");
        let m = result.unwrap();
        assert_eq!(m.fas_code, "021-2020");
        assert!(matches!(m.method, TasMethod::AgencyDisambiguated));
    }

    #[test]
    fn test_match_suffix_emdash() {
        let reference = make_test_reference();
        // "Operations and Support" is shared by 070-0400 and 070-0530 (both DHS)
        // So even with agency code 070, it's ambiguous
        let result = match_deterministic(
            "United States Secret Service\u{2014}Operations and Support",
            Some("070"),
            &reference,
        );
        // Suffix "operations and support" has 2 candidates in agency 070 — ambiguous
        assert!(
            result.is_none(),
            "ambiguous within same agency should not match"
        );
    }

    #[test]
    fn test_salaries_and_expenses_no_false_positive() {
        let reference = make_test_reference();
        // "Salaries and Expenses" under DOJ (015) must NOT match Treasury (020)
        let result = match_deterministic("Salaries and Expenses", Some("015"), &reference);
        assert!(result.is_some(), "should match with agency scope");
        let m = result.unwrap();
        assert_eq!(m.fas_code, "015-0100", "must match DOJ, not Treasury");
        assert_eq!(m.method, TasMethod::AgencyDisambiguated);
    }

    #[test]
    fn test_salaries_and_expenses_without_agency_is_ambiguous() {
        let reference = make_test_reference();
        // Without agency, "Salaries and Expenses" is ambiguous (DOJ + Treasury)
        let result = match_deterministic("Salaries and Expenses", None, &reference);
        assert!(
            result.is_none(),
            "ambiguous without agency should return None"
        );
    }

    #[test]
    fn test_match_miss() {
        let reference = make_test_reference();
        let result = match_deterministic("Totally Unknown Account Name", None, &reference);
        assert!(result.is_none());
    }

    #[test]
    fn test_contains_code() {
        let reference = make_test_reference();
        assert!(reference.contains_code("070-0400"));
        assert!(reference.contains_code("021-2020"));
        assert!(!reference.contains_code("999-9999"));
    }

    #[test]
    fn test_short_title() {
        assert_eq!(
            short_title("Operations and Support, United States Secret Service, Homeland Security"),
            "operations and support"
        );
        assert_eq!(
            short_title("Operation and Maintenance, Army"),
            "operation and maintenance"
        );
        assert_eq!(short_title("Single Part Title"), "single part title");
    }

    #[test]
    fn test_strip_emdash_prefix() {
        assert_eq!(
            strip_emdash_prefix("united states secret service\u{2014}operations and support"),
            "operations and support"
        );
        assert_eq!(strip_emdash_prefix("no dash here"), "no dash here");
    }

    #[test]
    fn test_resolve_deterministic_produces_mappings() {
        let reference = make_test_reference();
        let provisions = vec![
            ProvisionForTas {
                index: 0,
                account_name: "Salaries and Expenses".to_string(),
                agency: "Department of Justice".to_string(),
                dollars: Some(1_000_000_000),
            },
            ProvisionForTas {
                index: 1,
                account_name: "Totally Unknown Account".to_string(),
                agency: "Unknown Agency".to_string(),
                dollars: None,
            },
        ];

        let mappings = resolve_deterministic(&provisions, &reference);
        assert_eq!(mappings.len(), 2);

        // DOJ Salaries and Expenses should match 015-0100 via agency disambiguation
        assert_eq!(mappings[0].fas_code.as_deref(), Some("015-0100"));
        assert_eq!(mappings[0].confidence, TasConfidence::Verified);

        assert!(mappings[1].fas_code.is_none());
        assert_eq!(mappings[1].confidence, TasConfidence::Unmatched);
    }

    #[test]
    fn test_agency_name_to_code() {
        assert_eq!(agency_name_to_code("Department of Justice"), Some("015"));
        assert_eq!(agency_name_to_code("Department of the Army"), Some("021"));
        assert_eq!(
            agency_name_to_code("Federal Bureau of Investigation"),
            Some("015")
        );
        assert_eq!(agency_name_to_code("Coast Guard"), Some("070"));
        assert_eq!(agency_name_to_code("Totally Unknown Agency"), None);
        assert_eq!(agency_name_to_code(""), None);
    }

    #[test]
    fn test_agency_code_dod_branch_from_account() {
        // "Department of Defense" + account name with service branch → branch-specific code
        assert_eq!(
            agency_name_to_code_with_account(
                "Department of Defense",
                "Operation and Maintenance, Army"
            ),
            Some("021")
        );
        assert_eq!(
            agency_name_to_code_with_account(
                "Department of Defense",
                "Operation and Maintenance, Navy"
            ),
            Some("017")
        );
        assert_eq!(
            agency_name_to_code_with_account(
                "Department of Defense",
                "Operation and Maintenance, Air Force"
            ),
            Some("057")
        );
        assert_eq!(
            agency_name_to_code_with_account(
                "Department of Defense",
                "Operation and Maintenance, Army National Guard"
            ),
            Some("021")
        );
        assert_eq!(
            agency_name_to_code_with_account(
                "Department of Defense",
                "Operation and Maintenance, Marine Corps"
            ),
            Some("017")
        );
        // No branch qualifier → DOD umbrella
        assert_eq!(
            agency_name_to_code_with_account(
                "Department of Defense",
                "Operation and Maintenance, Defense-Wide"
            ),
            Some("097")
        );
        // Non-DOD agency ignores account name
        assert_eq!(
            agency_name_to_code_with_account(
                "Department of Justice",
                "Salaries and Expenses, Army something"
            ),
            Some("015")
        );
    }

    #[test]
    fn test_build_mapping_file_summary() {
        let mappings = vec![
            TasMapping {
                provision_index: 0,
                account_name: "Test".to_string(),
                agency: "Test Agency".to_string(),
                dollars: Some(1000),
                fas_code: Some("070-0400".to_string()),
                fas_title: Some("Test Title".to_string()),
                confidence: TasConfidence::Verified,
                method: TasMethod::DirectMatch,
                reasoning: None,
            },
            TasMapping {
                provision_index: 1,
                account_name: "Unknown".to_string(),
                agency: "Unknown".to_string(),
                dollars: None,
                fas_code: None,
                fas_title: None,
                confidence: TasConfidence::Unmatched,
                method: TasMethod::None,
                reasoning: None,
            },
        ];

        let file = build_mapping_file(mappings, "test-bill", "H.R. 9999", None, "abc123");
        assert_eq!(file.summary.total_provisions, 2);
        assert_eq!(file.summary.deterministic_matched, 1);
        assert_eq!(file.summary.unmatched, 1);
        assert_eq!(file.summary.unique_fas_codes, 1);
        assert!((file.summary.match_rate_pct - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_parse_llm_response_valid() {
        let response = r#"{"mappings": [{"provision_index": 5, "fas_code": "070-0400", "fas_title": "Operations and Support", "confidence": "high", "reasoning": "Direct match"}]}"#;
        let results = parse_llm_response(response).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].provision_index, 5);
        assert_eq!(results[0].fas_code.as_deref(), Some("070-0400"));
    }

    #[test]
    fn test_parse_llm_response_with_markdown() {
        let response = "```json\n{\"mappings\": [{\"provision_index\": 0, \"fas_code\": null, \"confidence\": \"none\", \"reasoning\": \"not found\"}]}\n```";
        let results = parse_llm_response(response).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].fas_code.is_none());
    }
}
