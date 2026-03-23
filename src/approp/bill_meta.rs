//! Bill-level metadata: fiscal year scoping, jurisdiction mapping, advance
//! appropriation classification, bill nature enrichment, and account
//! normalization.
//!
//! The `enrich` command generates a `bill_meta.json` file per bill directory
//! containing this metadata. It runs entirely offline — no API calls — using
//! XML parsing and deterministic keyword/date classification.

use crate::approp::ontology::{
    AmountSemantics, BillClassification, BillExtraction, FundAvailability, Provision,
};
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::LazyLock;

// ─── Types ───────────────────────────────────────────────────────────────────

/// Top-level bill metadata written to `bill_meta.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillMeta {
    pub schema_version: String,
    pub congress: Option<u32>,
    pub fiscal_years: Vec<u32>,
    pub bill_nature: BillNature,
    pub subcommittees: Vec<SubcommitteeMapping>,
    pub provision_timing: Vec<ProvisionTiming>,
    pub canonical_accounts: Vec<CanonicalAccount>,
    pub extraction_sha256: String,
}

/// Enriched bill classification with finer distinctions than the LLM's
/// `BillClassification`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BillNature {
    Regular,
    Omnibus,
    Minibus,
    ContinuingResolution,
    FullYearCrWithAppropriations,
    Supplemental,
    Authorization,
    #[serde(untagged)]
    Other(String),
}

impl std::fmt::Display for BillNature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BillNature::Regular => write!(f, "Regular"),
            BillNature::Omnibus => write!(f, "Omnibus"),
            BillNature::Minibus => write!(f, "Minibus"),
            BillNature::ContinuingResolution => write!(f, "Continuing Resolution"),
            BillNature::FullYearCrWithAppropriations => {
                write!(f, "Full-Year CR with Appropriations")
            }
            BillNature::Supplemental => write!(f, "Supplemental"),
            BillNature::Authorization => write!(f, "Authorization"),
            BillNature::Other(s) => write!(f, "{s}"),
        }
    }
}

/// Maps a single division letter to a canonical jurisdiction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcommitteeMapping {
    pub division: String,
    pub jurisdiction: Jurisdiction,
    pub title: String,
    pub source: ClassificationSource,
}

/// The twelve traditional appropriations subcommittee jurisdictions plus
/// common non-subcommittee division types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Jurisdiction {
    Defense,
    LaborHhs,
    Thud,
    FinancialServices,
    Cjs,
    EnergyWater,
    Interior,
    Agriculture,
    LegislativeBranch,
    MilconVa,
    StateForeignOps,
    HomelandSecurity,
    ContinuingResolution,
    Extenders,
    Policy,
    BudgetProcess,
    Other,
}

impl Jurisdiction {
    /// The canonical slug used in `--subcommittee` CLI flag.
    pub fn slug(&self) -> &'static str {
        match self {
            Jurisdiction::Defense => "defense",
            Jurisdiction::LaborHhs => "labor-hhs",
            Jurisdiction::Thud => "thud",
            Jurisdiction::FinancialServices => "financial-services",
            Jurisdiction::Cjs => "cjs",
            Jurisdiction::EnergyWater => "energy-water",
            Jurisdiction::Interior => "interior",
            Jurisdiction::Agriculture => "agriculture",
            Jurisdiction::LegislativeBranch => "legislative-branch",
            Jurisdiction::MilconVa => "milcon-va",
            Jurisdiction::StateForeignOps => "state-foreign-ops",
            Jurisdiction::HomelandSecurity => "homeland-security",
            Jurisdiction::ContinuingResolution => "continuing-resolution",
            Jurisdiction::Extenders => "extenders",
            Jurisdiction::Policy => "policy",
            Jurisdiction::BudgetProcess => "budget-process",
            Jurisdiction::Other => "other",
        }
    }

    /// Parse a jurisdiction from a CLI slug (case-insensitive).
    pub fn from_slug(slug: &str) -> Option<Jurisdiction> {
        match slug.to_lowercase().as_str() {
            "defense" => Some(Jurisdiction::Defense),
            "labor-hhs" => Some(Jurisdiction::LaborHhs),
            "thud" => Some(Jurisdiction::Thud),
            "financial-services" => Some(Jurisdiction::FinancialServices),
            "cjs" => Some(Jurisdiction::Cjs),
            "energy-water" => Some(Jurisdiction::EnergyWater),
            "interior" => Some(Jurisdiction::Interior),
            "agriculture" => Some(Jurisdiction::Agriculture),
            "legislative-branch" => Some(Jurisdiction::LegislativeBranch),
            "milcon-va" | "milconva" => Some(Jurisdiction::MilconVa),
            "state-foreign-ops" => Some(Jurisdiction::StateForeignOps),
            "homeland-security" => Some(Jurisdiction::HomelandSecurity),
            "continuing-resolution" => Some(Jurisdiction::ContinuingResolution),
            "extenders" => Some(Jurisdiction::Extenders),
            "policy" => Some(Jurisdiction::Policy),
            "budget-process" => Some(Jurisdiction::BudgetProcess),
            "other" => Some(Jurisdiction::Other),
            _ => None,
        }
    }
}

/// Per-provision advance/current/supplemental classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionTiming {
    pub provision_index: usize,
    pub timing: FundingTiming,
    pub available_fy: Option<u32>,
    pub source: ClassificationSource,
}

/// Whether a BA provision is current-year, advance, or supplemental.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FundingTiming {
    CurrentYear,
    Advance,
    Supplemental,
    Unknown,
}

/// Normalized account name for cross-bill matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalAccount {
    pub provision_index: usize,
    pub canonical_name: String,
}

/// How a classification was determined — provenance for every automated decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClassificationSource {
    XmlStructure,
    PatternMatch { pattern: String },
    FiscalYearComparison { availability_fy: u32, bill_fy: u32 },
    NoteText,
    DefaultRule,
    LlmClassification { model: String, confidence: f32 },
    Manual,
}

// ─── Jurisdiction Classification ─────────────────────────────────────────────

/// Patterns for classifying division titles to jurisdictions.
/// Each entry is (jurisdiction, list of case-insensitive substring/regex patterns).
const JURISDICTION_PATTERNS: &[(Jurisdiction, &[&str])] = &[
    (
        Jurisdiction::Defense,
        &["department of defense", "defense appropriations"],
    ),
    (
        Jurisdiction::LaborHhs,
        &["departments of labor", "labor, health"],
    ),
    (
        Jurisdiction::Thud,
        &[
            "transportation, housing and urban development",
            "transportation-housing",
        ],
    ),
    (
        Jurisdiction::FinancialServices,
        &["financial services", "general government"],
    ),
    (
        Jurisdiction::Cjs,
        &[
            "commerce, justice, science",
            "science, and related agencies",
        ],
    ),
    (Jurisdiction::EnergyWater, &["energy and water"]),
    (
        Jurisdiction::Interior,
        &["interior, environment", "department of the interior"],
    ),
    (
        Jurisdiction::Agriculture,
        &["agriculture, rural development"],
    ),
    (Jurisdiction::LegislativeBranch, &["legislative branch"]),
    (
        Jurisdiction::MilconVa,
        &[
            "military construction, veterans",
            "military construction and veterans",
        ],
    ),
    (
        Jurisdiction::StateForeignOps,
        &["state, foreign", "department of state"],
    ),
    (Jurisdiction::HomelandSecurity, &["homeland security"]),
    (
        Jurisdiction::ContinuingResolution,
        &[
            "continuing appropriations",
            "further additional continuing",
            "additional continuing",
            "further continuing",
            "extension of continuing",
        ],
    ),
];

/// Additional patterns for common non-subcommittee divisions.
const GENERIC_PATTERNS: &[(&str, Jurisdiction)] = &[
    ("other matters", Jurisdiction::Other),
    ("miscellaneous", Jurisdiction::Other),
    ("budgetary effects", Jurisdiction::BudgetProcess),
    ("extensions", Jurisdiction::Extenders),
    ("extender", Jurisdiction::Extenders),
    ("health care extender", Jurisdiction::Extenders),
];

/// Classify a division title to a jurisdiction using pattern matching.
pub fn classify_jurisdiction(title: &str) -> (Jurisdiction, ClassificationSource) {
    let lower = title.to_lowercase();

    // Try subcommittee patterns first
    for (jurisdiction, patterns) in JURISDICTION_PATTERNS {
        for pattern in *patterns {
            if lower.contains(pattern) {
                return (
                    jurisdiction.clone(),
                    ClassificationSource::PatternMatch {
                        pattern: (*pattern).to_string(),
                    },
                );
            }
        }
    }

    // Try generic patterns
    for (pattern, jurisdiction) in GENERIC_PATTERNS {
        if lower.contains(pattern) {
            return (
                jurisdiction.clone(),
                ClassificationSource::PatternMatch {
                    pattern: (*pattern).to_string(),
                },
            );
        }
    }

    // Health-related titles → labor-hhs
    if lower == "health" || lower.starts_with("health ") {
        return (
            Jurisdiction::LaborHhs,
            ClassificationSource::PatternMatch {
                pattern: "health".to_string(),
            },
        );
    }

    // Unclassified — default to Other
    (Jurisdiction::Other, ClassificationSource::DefaultRule)
}

// ─── Advance Appropriation Classification ────────────────────────────────────

/// Regex for "October 1, YYYY" dates in availability text.
static OCTOBER_1_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)October\s+1\s*,?\s*(\d{4})").unwrap());

/// Regex for "first quarter of fiscal year YYYY" in availability text.
static FIRST_QUARTER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)first\s+quarter\s+of\s+fiscal\s+year\s+(\d{4})").unwrap());

/// Regex for "fiscal year YYYY" references (broader catch).
static FISCAL_YEAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)fiscal\s+year\s+(\d{4})").unwrap());

/// Extract availability fiscal year from text using known patterns.
///
/// Returns `(availability_fy, pattern_description)` if found.
fn extract_availability_fy(text: &str) -> Option<(u32, String)> {
    // Pattern 1: "October 1, YYYY" → availability FY = YYYY + 1
    if let Some(caps) = OCTOBER_1_RE.captures(text)
        && let Ok(year) = caps[1].parse::<u32>()
    {
        return Some((year + 1, format!("October 1, {year}")));
    }

    // Pattern 2: "first quarter of fiscal year YYYY" → availability FY = YYYY
    if let Some(caps) = FIRST_QUARTER_RE.captures(text)
        && let Ok(year) = caps[1].parse::<u32>()
    {
        return Some((year, format!("first quarter of fiscal year {year}")));
    }

    None
}

/// Classify a single provision's funding timing using fiscal-year-aware logic.
///
/// Algorithm:
/// 1. Extract October 1 or first-quarter-of-FY year from availability + raw_text
/// 2. Compare to bill fiscal year: if availability_fy > bill_fy → Advance
/// 3. Check notes for "supplemental" → Supplemental
/// 4. Explicit "advance appropriation" text → Advance
/// 5. Default → CurrentYear
pub fn classify_provision_timing(
    provision: &Provision,
    bill_fiscal_years: &[u32],
) -> (FundingTiming, Option<u32>, ClassificationSource) {
    let bill_fy = match bill_fiscal_years.iter().max() {
        Some(&fy) => fy,
        None => {
            return (
                FundingTiming::CurrentYear,
                None,
                ClassificationSource::DefaultRule,
            );
        }
    };

    // Get availability text + raw_text
    let (avail_text, raw_text, notes) = match provision {
        Provision::Appropriation {
            availability,
            raw_text,
            notes,
            ..
        } => {
            let avail_str = match availability {
                Some(FundAvailability::Other(s)) => s.as_str(),
                _ => "",
            };
            (avail_str, raw_text.as_str(), notes.as_slice())
        }
        _ => ("", "", &[] as &[String]),
    };

    let combined = format!("{avail_text} {raw_text}");

    // Step 1: Extract availability FY from known patterns
    // Check availability text first (more specific), then raw_text
    if let Some((availability_fy, _pattern)) =
        extract_availability_fy(avail_text).or_else(|| extract_availability_fy(raw_text))
    {
        if availability_fy > bill_fy {
            return (
                FundingTiming::Advance,
                Some(availability_fy),
                ClassificationSource::FiscalYearComparison {
                    availability_fy,
                    bill_fy,
                },
            );
        } else {
            // availability_fy <= bill_fy → current year (start of funded FY or prior reference)
            return (
                FundingTiming::CurrentYear,
                Some(availability_fy),
                ClassificationSource::FiscalYearComparison {
                    availability_fy,
                    bill_fy,
                },
            );
        }
    }

    // Step 2: Check notes for "supplemental"
    for note in notes {
        if note.to_lowercase().contains("supplemental") {
            return (
                FundingTiming::Supplemental,
                None,
                ClassificationSource::NoteText,
            );
        }
    }

    // Step 3: Explicit "advance appropriation" text
    if combined.to_lowercase().contains("advance appropriation") {
        return (
            FundingTiming::Advance,
            None,
            ClassificationSource::PatternMatch {
                pattern: "advance appropriation".to_string(),
            },
        );
    }

    // Step 4: Check for future FY references not caught by patterns 1-2
    // Log-worthy but classified as CurrentYear (unknown pattern)
    if let Some(caps) = FISCAL_YEAR_RE.captures(&combined)
        && let Ok(ref_fy) = caps[1].parse::<u32>()
        && ref_fy > bill_fy
    {
        // Future FY reference but not matching our known advance patterns.
        // This is a potential advance appropriation we can't confirm.
        tracing::warn!(
            "Provision references FY{ref_fy} (bill is FY{bill_fy}) but no advance pattern matched — defaulting to CurrentYear"
        );
        return (
            FundingTiming::Unknown,
            Some(ref_fy),
            ClassificationSource::DefaultRule,
        );
    }

    // Step 5: Default — current year
    (
        FundingTiming::CurrentYear,
        None,
        ClassificationSource::DefaultRule,
    )
}

// ─── Bill Nature Classification ──────────────────────────────────────────────

/// Classify the bill nature from the extraction data and subcommittee mappings.
///
/// Uses provision type distribution and subcommittee count to distinguish:
/// - Omnibus (5+ subcommittees with appropriations)
/// - Minibus (2-4 subcommittees with appropriations)
/// - Full-year CR with appropriations (CR baseline + many appropriations)
/// - Regular, Supplemental, Authorization (from LLM classification)
pub fn classify_bill_nature(
    extraction: &BillExtraction,
    subcommittees: &[SubcommitteeMapping],
) -> BillNature {
    let provisions = &extraction.provisions;

    // Count provision types
    let mut appropriation_count = 0usize;
    let mut cr_baseline_count = 0usize;

    for p in provisions {
        match p {
            Provision::Appropriation { .. } => appropriation_count += 1,
            Provision::ContinuingResolutionBaseline { .. } => cr_baseline_count += 1,
            _ => {}
        }
    }

    // Count real appropriations subcommittees (exclude CR, extenders, other)
    let real_subcommittees: usize = subcommittees
        .iter()
        .filter(|s| {
            !matches!(
                s.jurisdiction,
                Jurisdiction::ContinuingResolution
                    | Jurisdiction::Extenders
                    | Jurisdiction::Policy
                    | Jurisdiction::BudgetProcess
                    | Jurisdiction::Other
            )
        })
        .count();

    // Detect omnibus vs minibus based on subcommittee count FIRST.
    // An omnibus that contains an embedded CR division (like H.R. 7148 with
    // Division H "Further Continuing Appropriations Act") is still an omnibus,
    // not a "full-year CR with appropriations." The subcommittee count is the
    // stronger signal.
    if real_subcommittees >= 5 {
        return BillNature::Omnibus;
    }
    if real_subcommittees >= 2 {
        // 2-4 real subcommittees. Could be a minibus, or a minibus+CR.
        // If it also has a CR baseline, it's a minibus with a CR division —
        // still a minibus. The CR is just covering the un-funded subcommittees.
        return BillNature::Minibus;
    }

    // No real subcommittees (or just 1). If there's a CR baseline AND many
    // appropriations, this is a full-year CR with embedded appropriations
    // (like H.R. 1968 which funds Defense, Homeland, Labor-HHS etc. through
    // appropriation provisions even though it's structured as a CR).
    if cr_baseline_count > 0 && appropriation_count > 50 {
        return BillNature::FullYearCrWithAppropriations;
    }

    // Fall back to the LLM's classification, normalized
    match &extraction.bill.classification {
        BillClassification::Regular => BillNature::Regular,
        BillClassification::ContinuingResolution => {
            // Already checked for full-year CR above
            BillNature::ContinuingResolution
        }
        BillClassification::Omnibus => {
            // Already checked subcommittee count above — if we got here with <2
            // subcommittees, trust the LLM but keep its label
            BillNature::Omnibus
        }
        BillClassification::Supplemental => BillNature::Supplemental,
        BillClassification::Rescissions => BillNature::Other("rescissions".to_string()),
        BillClassification::Minibus => BillNature::Minibus,
        BillClassification::Other(s) => {
            let lower = s.to_lowercase();
            if lower.contains("supplemental") {
                BillNature::Supplemental
            } else if lower.contains("authorization") {
                BillNature::Authorization
            } else {
                BillNature::Other(s.clone())
            }
        }
    }
}

// ─── Account Normalization ───────────────────────────────────────────────────

/// Normalize an account name for cross-bill matching.
///
/// Lowercases, strips hierarchical em-dash/en-dash prefixes, and trims whitespace.
/// For example:
/// - "Grants-In-Aid for Airports" → "grants-in-aid for airports"
/// - "Department of VA\u{2014}Compensation and Pensions" → "compensation and pensions"
pub fn normalize_account_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let parts: Vec<&str> = lower.split(&['\u{2014}', '\u{2013}'][..]).collect();
    if parts.len() > 1 {
        return parts.last().unwrap_or(&"").trim().to_string();
    }
    lower.trim().to_string()
}

// ─── XML Parsing ─────────────────────────────────────────────────────────────

/// A division parsed from XML: its letter and title.
#[derive(Debug, Clone)]
pub struct ParsedDivision {
    pub letter: String,
    pub title: String,
}

/// Parse congress number from the XML filename.
///
/// Expected format: `BILLS-{congress}{type}{number}enr.xml`
/// Returns `None` if the filename doesn't match.
pub fn parse_congress_from_filename(filename: &str) -> Option<u32> {
    static CONGRESS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"BILLS-(\d+)").unwrap());

    CONGRESS_RE
        .captures(filename)
        .and_then(|caps| caps[1].parse::<u32>().ok())
}

/// Parse division letters and titles from a bill XML file using `roxmltree`.
///
/// Tries two approaches:
/// 1. `<division><enum>A</enum><header>Title</header>` in the body
/// 2. `<toc-entry level="division">Division A—Title</toc-entry>` in the TOC
///
/// Returns an empty Vec for bills without divisions (supplementals, etc.).
pub fn parse_divisions_from_xml(xml_path: &Path) -> Result<Vec<ParsedDivision>> {
    let raw = std::fs::read_to_string(xml_path)
        .with_context(|| format!("Failed to read {}", xml_path.display()))?;

    let opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };

    let doc = roxmltree::Document::parse_with_options(&raw, opt)
        .with_context(|| format!("Failed to parse XML: {}", xml_path.display()))?;

    let root = doc.root_element();

    // Approach 1: Find <division> elements with <enum> + <header> children
    let mut divisions = Vec::new();
    for node in root.descendants() {
        if node.tag_name().name() == "division" {
            let mut enum_text = None;
            let mut header_text = None;

            for child in node.children() {
                if !child.is_element() {
                    continue;
                }
                match child.tag_name().name() {
                    "enum" if enum_text.is_none() => {
                        enum_text = Some(collect_text(&child).trim().to_string());
                    }
                    "header" if header_text.is_none() => {
                        let text = collect_text(&child).trim().to_string();
                        // Normalize whitespace
                        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                        header_text = Some(text);
                    }
                    _ => {}
                }
                if enum_text.is_some() && header_text.is_some() {
                    break;
                }
            }

            if let (Some(letter), Some(title)) = (enum_text, header_text)
                && !letter.is_empty()
                && !title.is_empty()
            {
                divisions.push(ParsedDivision { letter, title });
            }
        }
    }

    if !divisions.is_empty() {
        return Ok(divisions);
    }

    // Approach 2: Fall back to <toc-entry level="division">
    static TOC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)Division\s+([A-Z](?:-[A-Z])?)\s*[\u{2014}\u{2013}\-]\s*(.+)").unwrap()
    });

    for node in root.descendants() {
        if node.tag_name().name() == "toc-entry" {
            let level = node.attribute("level").unwrap_or("");
            if level == "division" {
                let text = collect_text(&node).trim().to_string();
                let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if let Some(caps) = TOC_RE.captures(&text) {
                    divisions.push(ParsedDivision {
                        letter: caps[1].to_string(),
                        title: caps[2].trim().to_string(),
                    });
                }
            }
        }
    }

    Ok(divisions)
}

/// Collect all text content from an XML node recursively.
fn collect_text(node: &roxmltree::Node) -> String {
    let mut out = String::new();
    for child in node.children() {
        if child.is_text() {
            if let Some(text) = child.text() {
                out.push_str(text);
            }
        } else if child.is_element() {
            out.push_str(&collect_text(&child));
        }
    }
    out
}

// ─── Enrichment (the main function) ──────────────────────────────────────────

/// Generate `BillMeta` for a single bill.
///
/// This is the core logic behind the `enrich` command.
///
/// - `extraction`: the loaded `BillExtraction`
/// - `xml_path`: path to the `BILLS-*.xml` file (for division parsing)
/// - `extraction_path`: path to `extraction.json` (for hash chain)
pub fn generate_bill_meta(
    extraction: &BillExtraction,
    xml_path: Option<&Path>,
    extraction_path: &Path,
) -> Result<BillMeta> {
    // Congress number from XML filename
    let congress = xml_path.and_then(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .and_then(parse_congress_from_filename)
    });

    // Fiscal years from extraction
    let fiscal_years = extraction.bill.fiscal_years.clone();

    // Parse divisions from XML and classify jurisdictions
    let subcommittees = if let Some(xml) = xml_path {
        let divisions = parse_divisions_from_xml(xml).unwrap_or_else(|e| {
            tracing::warn!("Could not parse divisions from XML: {e}");
            Vec::new()
        });

        divisions
            .into_iter()
            .map(|div| {
                let (jurisdiction, source) = classify_jurisdiction(&div.title);
                SubcommitteeMapping {
                    division: div.letter,
                    jurisdiction,
                    title: div.title,
                    source,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    // Classify bill nature
    let bill_nature = classify_bill_nature(extraction, &subcommittees);

    // Classify provision timing for all BA appropriation provisions
    let mut provision_timing = Vec::new();
    for (i, p) in extraction.provisions.iter().enumerate() {
        // Only classify appropriation provisions with budget authority
        if let Some(amt) = p.amount()
            && matches!(amt.semantics, AmountSemantics::NewBudgetAuthority)
            && matches!(p, Provision::Appropriation { .. })
        {
            let (timing, available_fy, source) = classify_provision_timing(p, &fiscal_years);
            provision_timing.push(ProvisionTiming {
                provision_index: i,
                timing,
                available_fy,
                source,
            });
        }
    }

    // Normalize account names for all provisions that have one
    let mut canonical_accounts = Vec::new();
    for (i, p) in extraction.provisions.iter().enumerate() {
        let name = p.account_name();
        if !name.is_empty() {
            canonical_accounts.push(CanonicalAccount {
                provision_index: i,
                canonical_name: normalize_account_name(name),
            });
        }
    }

    // Compute extraction hash for hash chain
    let extraction_sha256 = {
        let bytes = std::fs::read(extraction_path)
            .with_context(|| format!("Failed to read {}", extraction_path.display()))?;
        format!("{:x}", Sha256::digest(&bytes))
    };

    Ok(BillMeta {
        schema_version: "1.0".to_string(),
        congress,
        fiscal_years,
        bill_nature,
        subcommittees,
        provision_timing,
        canonical_accounts,
        extraction_sha256,
    })
}

// ─── I/O ─────────────────────────────────────────────────────────────────────

/// Load `bill_meta.json` from a bill directory. Returns `None` if not present.
pub fn load_bill_meta(dir: &Path) -> Option<BillMeta> {
    let path = dir.join("bill_meta.json");
    if !path.exists() {
        return None;
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(meta) => Some(meta),
            Err(e) => {
                tracing::debug!("Could not parse {}: {e}", path.display());
                None
            }
        },
        Err(e) => {
            tracing::debug!("Could not read {}: {e}", path.display());
            None
        }
    }
}

/// Save `bill_meta.json` to a bill directory.
pub fn save_bill_meta(dir: &Path, meta: &BillMeta) -> Result<()> {
    let path = dir.join("bill_meta.json");
    let json = serde_json::to_string_pretty(meta)?;
    std::fs::write(&path, json).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

// ─── Helpers for Query Filtering ─────────────────────────────────────────────

/// Find the XML source file in a bill directory.
/// Prefers enrolled versions (filename ends with "enr").
pub fn find_xml_in_dir(dir: &Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "xml")
            && path
                .file_stem()
                .is_some_and(|n| n.to_string_lossy().starts_with("BILLS-"))
        {
            candidates.push(path);
        }
    }

    // Prefer enrolled version
    if let Some(enr) = candidates.iter().find(|p| {
        p.file_stem()
            .is_some_and(|n| n.to_string_lossy().ends_with("enr"))
    }) {
        return Some(enr.clone());
    }

    candidates.into_iter().next()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Congress Parsing ──

    #[test]
    fn test_parse_congress_from_filename() {
        assert_eq!(
            parse_congress_from_filename("BILLS-119hr7148enr.xml"),
            Some(119)
        );
        assert_eq!(
            parse_congress_from_filename("BILLS-118hr4366enr.xml"),
            Some(118)
        );
        assert_eq!(parse_congress_from_filename("random.xml"), None);
    }

    // ── Jurisdiction Classification ──

    #[test]
    fn test_classify_jurisdiction_defense() {
        let (j, _) = classify_jurisdiction("DEPARTMENT OF DEFENSE APPROPRIATIONS ACT, 2026");
        assert_eq!(j, Jurisdiction::Defense);
    }

    #[test]
    fn test_classify_jurisdiction_thud() {
        let (j, _) = classify_jurisdiction(
            "TRANSPORTATION, HOUSING AND URBAN DEVELOPMENT, AND RELATED AGENCIES APPROPRIATIONS ACT, 2026",
        );
        assert_eq!(j, Jurisdiction::Thud);
    }

    #[test]
    fn test_classify_jurisdiction_cjs() {
        let (j, _) = classify_jurisdiction(
            "COMMERCE, JUSTICE, SCIENCE, AND RELATED AGENCIES APPROPRIATIONS ACT, 2026",
        );
        assert_eq!(j, Jurisdiction::Cjs);
    }

    #[test]
    fn test_classify_jurisdiction_labor_hhs() {
        let (j, _) =
            classify_jurisdiction("DEPARTMENTS OF LABOR, HEALTH AND HUMAN SERVICES, AND EDUCATION");
        assert_eq!(j, Jurisdiction::LaborHhs);
    }

    #[test]
    fn test_classify_jurisdiction_milcon_va() {
        let (j, _) =
            classify_jurisdiction("Military Construction, Veterans Affairs, and Related Agencies");
        assert_eq!(j, Jurisdiction::MilconVa);
    }

    #[test]
    fn test_classify_jurisdiction_agriculture() {
        let (j, _) =
            classify_jurisdiction("Agriculture, Rural Development, Food and Drug Administration");
        assert_eq!(j, Jurisdiction::Agriculture);
    }

    #[test]
    fn test_classify_jurisdiction_cr() {
        let (j, _) = classify_jurisdiction("Continuing Appropriations Act, 2026");
        assert_eq!(j, Jurisdiction::ContinuingResolution);
    }

    #[test]
    fn test_classify_jurisdiction_other_matters() {
        let (j, _) = classify_jurisdiction("Other Matters");
        assert_eq!(j, Jurisdiction::Other);
    }

    #[test]
    fn test_classify_jurisdiction_unknown_supplemental() {
        let (j, source) = classify_jurisdiction("FEND Off Fentanyl Act");
        assert_eq!(j, Jurisdiction::Other);
        assert!(matches!(source, ClassificationSource::DefaultRule));
    }

    #[test]
    fn test_classify_jurisdiction_energy_water() {
        let (j, _) = classify_jurisdiction(
            "ENERGY AND WATER DEVELOPMENT AND RELATED AGENCIES APPROPRIATIONS ACT, 2026",
        );
        assert_eq!(j, Jurisdiction::EnergyWater);
    }

    #[test]
    fn test_classify_jurisdiction_interior() {
        let (j, _) =
            classify_jurisdiction("DEPARTMENT OF THE INTERIOR, ENVIRONMENT, AND RELATED AGENCIES");
        assert_eq!(j, Jurisdiction::Interior);
    }

    #[test]
    fn test_classify_jurisdiction_financial_services() {
        let (j, _) = classify_jurisdiction(
            "Financial Services and General Government Appropriations Act, 2026",
        );
        assert_eq!(j, Jurisdiction::FinancialServices);
    }

    #[test]
    fn test_classify_jurisdiction_state_foreign_ops() {
        let (j, _) =
            classify_jurisdiction("National Security, Department of State, and Related Programs");
        assert_eq!(j, Jurisdiction::StateForeignOps);
    }

    #[test]
    fn test_classify_jurisdiction_legislative_branch() {
        let (j, _) = classify_jurisdiction("Legislative Branch Appropriations Act, 2026");
        assert_eq!(j, Jurisdiction::LegislativeBranch);
    }

    #[test]
    fn test_classify_jurisdiction_extenders() {
        let (j, _) = classify_jurisdiction("Health Extenders");
        assert_eq!(j, Jurisdiction::Extenders);
    }

    #[test]
    fn test_classify_jurisdiction_health_standalone() {
        let (j, _) = classify_jurisdiction("Health");
        assert_eq!(j, Jurisdiction::LaborHhs);
    }

    // ── Advance Classification ──

    #[test]
    fn test_extract_availability_fy_october_1() {
        let (fy, _) = extract_availability_fy(
            "shall become available on October 1, 2024, to remain available until expended",
        )
        .unwrap();
        assert_eq!(fy, 2025); // October 1, 2024 = start of FY2025
    }

    #[test]
    fn test_extract_availability_fy_first_quarter() {
        let (fy, _) =
            extract_availability_fy("for the first quarter of fiscal year 2027, $316,514,725,000")
                .unwrap();
        assert_eq!(fy, 2027);
    }

    #[test]
    fn test_extract_availability_fy_no_match() {
        assert!(extract_availability_fy("to remain available until expended").is_none());
    }

    #[test]
    fn test_classify_timing_advance() {
        // October 1, 2024 in a FY2024 bill → FY2025 → advance
        let provision = Provision::Appropriation {
            account_name: "Test Account".to_string(),
            agency: None,
            program: None,
            amount: crate::approp::ontology::DollarAmount::from_dollars(
                100,
                AmountSemantics::NewBudgetAuthority,
                "$100",
            ),
            fiscal_year: None,
            availability: Some(FundAvailability::Other(
                "shall become available on October 1, 2024".to_string(),
            )),
            provisos: vec![],
            earmarks: vec![],
            section: String::new(),
            division: None,
            title: None,
            confidence: 0.9,
            raw_text: String::new(),
            notes: vec![],
            cross_references: vec![],
            detail_level: String::new(),
            parent_account: None,
        };

        let (timing, avail_fy, source) = classify_provision_timing(&provision, &[2024]);
        assert_eq!(timing, FundingTiming::Advance);
        assert_eq!(avail_fy, Some(2025));
        assert!(matches!(
            source,
            ClassificationSource::FiscalYearComparison { .. }
        ));
    }

    #[test]
    fn test_classify_timing_current_year_start_of_fy() {
        // October 1, 2025 in a FY2026 bill → FY2026 → current year (start of funded FY)
        let provision = Provision::Appropriation {
            account_name: "Test".to_string(),
            agency: None,
            program: None,
            amount: crate::approp::ontology::DollarAmount::from_dollars(
                100,
                AmountSemantics::NewBudgetAuthority,
                "$100",
            ),
            fiscal_year: None,
            availability: Some(FundAvailability::Other(
                "available on October 1, 2025".to_string(),
            )),
            provisos: vec![],
            earmarks: vec![],
            section: String::new(),
            division: None,
            title: None,
            confidence: 0.9,
            raw_text: String::new(),
            notes: vec![],
            cross_references: vec![],
            detail_level: String::new(),
            parent_account: None,
        };

        let (timing, avail_fy, _) = classify_provision_timing(&provision, &[2026]);
        assert_eq!(timing, FundingTiming::CurrentYear);
        assert_eq!(avail_fy, Some(2026));
    }

    #[test]
    fn test_classify_timing_first_quarter_advance() {
        // "first quarter of fiscal year 2027" in a FY2026 bill → advance
        let provision = Provision::Appropriation {
            account_name: "Medicaid".to_string(),
            agency: None,
            program: None,
            amount: crate::approp::ontology::DollarAmount::from_dollars(
                100,
                AmountSemantics::NewBudgetAuthority,
                "$100",
            ),
            fiscal_year: None,
            availability: None,
            provisos: vec![],
            earmarks: vec![],
            section: String::new(),
            division: None,
            title: None,
            confidence: 0.9,
            raw_text: "for the first quarter of fiscal year 2027, $316,514,725,000".to_string(),
            notes: vec![],
            cross_references: vec![],
            detail_level: String::new(),
            parent_account: None,
        };

        let (timing, avail_fy, _) = classify_provision_timing(&provision, &[2026]);
        assert_eq!(timing, FundingTiming::Advance);
        assert_eq!(avail_fy, Some(2027));
    }

    #[test]
    fn test_classify_timing_supplemental_from_notes() {
        let provision = Provision::Appropriation {
            account_name: "Test".to_string(),
            agency: None,
            program: None,
            amount: crate::approp::ontology::DollarAmount::from_dollars(
                100,
                AmountSemantics::NewBudgetAuthority,
                "$100",
            ),
            fiscal_year: None,
            availability: None,
            provisos: vec![],
            earmarks: vec![],
            section: String::new(),
            division: None,
            title: None,
            confidence: 0.9,
            raw_text: String::new(),
            notes: vec!["supplemental funding for disaster relief".to_string()],
            cross_references: vec![],
            detail_level: String::new(),
            parent_account: None,
        };

        let (timing, _, source) = classify_provision_timing(&provision, &[2024]);
        assert_eq!(timing, FundingTiming::Supplemental);
        assert!(matches!(source, ClassificationSource::NoteText));
    }

    #[test]
    fn test_classify_timing_default() {
        let provision = Provision::Appropriation {
            account_name: "Test".to_string(),
            agency: None,
            program: None,
            amount: crate::approp::ontology::DollarAmount::from_dollars(
                100,
                AmountSemantics::NewBudgetAuthority,
                "$100",
            ),
            fiscal_year: None,
            availability: Some(FundAvailability::Other(
                "to remain available until expended".to_string(),
            )),
            provisos: vec![],
            earmarks: vec![],
            section: String::new(),
            division: None,
            title: None,
            confidence: 0.9,
            raw_text: String::new(),
            notes: vec![],
            cross_references: vec![],
            detail_level: String::new(),
            parent_account: None,
        };

        let (timing, _, source) = classify_provision_timing(&provision, &[2024]);
        assert_eq!(timing, FundingTiming::CurrentYear);
        assert!(matches!(source, ClassificationSource::DefaultRule));
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
    fn test_normalize_account_name_em_dash() {
        assert_eq!(
            normalize_account_name("Department of VA\u{2014}Compensation and Pensions"),
            "compensation and pensions"
        );
    }

    #[test]
    fn test_normalize_account_name_en_dash() {
        assert_eq!(
            normalize_account_name("DoD\u{2013}Operations"),
            "operations"
        );
    }

    #[test]
    fn test_normalize_account_name_case_variants() {
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
    fn test_normalize_account_name_multi_prefix() {
        assert_eq!(
            normalize_account_name(
                "Department of Veterans Affairs\u{2014}Veterans Benefits Administration\u{2014}Compensation and Pensions"
            ),
            "compensation and pensions"
        );
    }

    // ── Bill Nature ──

    #[test]
    fn test_bill_nature_full_year_cr() {
        // A bill with a CR baseline + 260 appropriations → FullYearCrWithAppropriations
        let mut provisions = Vec::new();
        provisions.push(Provision::ContinuingResolutionBaseline {
            reference_year: 2024,
            reference_laws: vec![],
            rate: "the rate for operations".to_string(),
            duration: None,
            anomalies: vec![],
            section: String::new(),
            division: None,
            title: None,
            confidence: 0.9,
            raw_text: String::new(),
            notes: vec![],
            cross_references: vec![],
        });
        for _ in 0..260 {
            provisions.push(Provision::Appropriation {
                account_name: "Test".to_string(),
                agency: None,
                program: None,
                amount: crate::approp::ontology::DollarAmount::zero(
                    AmountSemantics::NewBudgetAuthority,
                ),
                fiscal_year: None,
                availability: None,
                provisos: vec![],
                earmarks: vec![],
                section: String::new(),
                division: None,
                title: None,
                confidence: 0.9,
                raw_text: String::new(),
                notes: vec![],
                cross_references: vec![],
                detail_level: String::new(),
                parent_account: None,
            });
        }

        let extraction = BillExtraction {
            schema_version: Some("1.0".to_string()),
            bill: crate::approp::ontology::BillInfo {
                identifier: "H.R. 1968".to_string(),
                classification: BillClassification::ContinuingResolution,
                short_title: None,
                fiscal_years: vec![2025],
                divisions: vec!["A".to_string()],
                public_law: None,
            },
            provisions,
            summary: crate::approp::ontology::ExtractionSummary {
                total_provisions: 261,
                by_division: std::collections::HashMap::new(),
                by_type: std::collections::HashMap::new(),
                total_budget_authority: 0,
                total_rescissions: 0,
                sections_with_no_provisions: vec![],
                flagged_issues: vec![],
            },
            chunk_map: vec![],
        };

        let nature = classify_bill_nature(&extraction, &[]);
        assert_eq!(nature, BillNature::FullYearCrWithAppropriations);
    }

    // ── Jurisdiction Slug Roundtrip ──

    #[test]
    fn test_jurisdiction_slug_roundtrip() {
        let jurisdictions = [
            Jurisdiction::Defense,
            Jurisdiction::LaborHhs,
            Jurisdiction::Thud,
            Jurisdiction::FinancialServices,
            Jurisdiction::Cjs,
            Jurisdiction::EnergyWater,
            Jurisdiction::Interior,
            Jurisdiction::Agriculture,
            Jurisdiction::LegislativeBranch,
            Jurisdiction::MilconVa,
            Jurisdiction::StateForeignOps,
            Jurisdiction::HomelandSecurity,
        ];

        for j in &jurisdictions {
            let slug = j.slug();
            let parsed = Jurisdiction::from_slug(slug);
            assert_eq!(parsed.as_ref(), Some(j), "Roundtrip failed for {slug}");
        }
    }

    // ── Save/Load Roundtrip ──

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let meta = BillMeta {
            schema_version: "1.0".to_string(),
            congress: Some(119),
            fiscal_years: vec![2026],
            bill_nature: BillNature::Omnibus,
            subcommittees: vec![SubcommitteeMapping {
                division: "A".to_string(),
                jurisdiction: Jurisdiction::Defense,
                title: "Department of Defense".to_string(),
                source: ClassificationSource::PatternMatch {
                    pattern: "department of defense".to_string(),
                },
            }],
            provision_timing: vec![ProvisionTiming {
                provision_index: 0,
                timing: FundingTiming::CurrentYear,
                available_fy: None,
                source: ClassificationSource::DefaultRule,
            }],
            canonical_accounts: vec![CanonicalAccount {
                provision_index: 0,
                canonical_name: "military personnel, army".to_string(),
            }],
            extraction_sha256: "abc123".to_string(),
        };

        save_bill_meta(dir.path(), &meta).unwrap();
        let loaded = load_bill_meta(dir.path()).unwrap();
        assert_eq!(loaded.congress, Some(119));
        assert_eq!(loaded.bill_nature, BillNature::Omnibus);
        assert_eq!(loaded.subcommittees.len(), 1);
        assert_eq!(loaded.subcommittees[0].jurisdiction, Jurisdiction::Defense);
    }
}
