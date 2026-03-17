use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Amount Value ────────────────────────────────────────────────────────────

/// The actual value of a dollar amount — may be a specific number,
/// an open-ended authorization, or absent entirely.
///
/// "Not to exceed" and other ceiling language is captured by
/// `AmountSemantics::Limitation`, not by this enum. A ceiling
/// still has a specific dollar number — the semantics tell you
/// it's a cap.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AmountValue {
    /// A specific dollar amount (e.g., "$51,181,397,000" or "$0").
    /// Includes explicitly zeroed-out amounts.
    Specific {
        /// Whole dollars as integer (can be negative for rescissions)
        dollars: i64,
    },
    /// "such sums as may be necessary" — open-ended, no dollar figure
    SuchSums,
    /// No amount — the provision doesn't carry a dollar value
    /// (directives, riders, extensions without dollar figures)
    None,
}

impl AmountValue {
    /// Get the dollar value if this is a specific amount.
    pub fn dollars(&self) -> Option<i64> {
        match self {
            Self::Specific { dollars } => Some(*dollars),
            _ => Option::None,
        }
    }

    /// True if this is a definite, specific amount.
    pub fn is_definite(&self) -> bool {
        matches!(self, Self::Specific { .. })
    }

    /// Construct a specific dollar amount from an i64.
    pub fn specific(dollars: i64) -> Self {
        Self::Specific { dollars }
    }
}

// ─── Source Location ─────────────────────────────────────────────────────────

/// Structural location of a provision within a bill.
/// Uses human-readable section references, not character offsets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SourceSpan {
    /// Section header, e.g. "SEC. 1401"
    #[serde(default)]
    pub section: String,
    /// Division letter, e.g. "A"
    #[serde(default)]
    pub division: Option<String>,
    /// Title numeral, e.g. "IV"
    #[serde(default)]
    pub title: Option<String>,
}

// ─── Cross-References ────────────────────────────────────────────────────────

/// A reference from one provision to another section or law.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossReference {
    /// Type of reference: "transfer_to", "rescinds_from", "modifies",
    /// "baseline_from", "amends", "references", etc.
    #[serde(default)]
    pub ref_type: String,
    /// Target: "SEC. 1402" or "P.L. 118-47, Division A"
    #[serde(default)]
    pub target: String,
    /// Optional description of the relationship
    #[serde(default)]
    pub description: Option<String>,
}

// ─── Dollar Amounts ──────────────────────────────────────────────────────────

/// Dollar amount with semantics.
///
/// `value` carries the actual amount — may be a specific number,
/// an open-ended authorization ("such sums"), a ceiling, or zero.
/// `semantics` describes what the amount represents in budget terms.
/// `text_as_written` is the verbatim string from the bill for verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DollarAmount {
    /// The amount — may be a specific number, open-ended, a ceiling, etc.
    pub value: AmountValue,
    /// What this amount represents in budget terms.
    pub semantics: AmountSemantics,
    /// Verbatim text from the bill, e.g. "$51,181,397,000" or
    /// "such sums as may be necessary".
    #[serde(default)]
    pub text_as_written: String,
}

impl DollarAmount {
    /// Create a DollarAmount for a specific integer dollar value.
    pub fn from_dollars(dollars: i64, semantics: AmountSemantics, text: impl Into<String>) -> Self {
        Self {
            value: AmountValue::specific(dollars),
            semantics,
            text_as_written: text.into(),
        }
    }

    /// Create a zero-value DollarAmount.
    pub fn zero(semantics: AmountSemantics) -> Self {
        Self {
            value: AmountValue::Specific { dollars: 0 },
            semantics,
            text_as_written: String::new(),
        }
    }

    /// Create a "such sums as may be necessary" amount.
    pub fn such_sums(semantics: AmountSemantics, text: impl Into<String>) -> Self {
        Self {
            value: AmountValue::SuchSums,
            semantics,
            text_as_written: text.into(),
        }
    }

    /// Convenience: get the dollar value if specific, else None.
    pub fn dollars(&self) -> Option<i64> {
        self.value.dollars()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AmountSemantics {
    NewBudgetAuthority,
    TransferCeiling,
    Rescission,
    Limitation,
    ReferenceAmount,
    MandatorySpending,
    #[serde(untagged)]
    Other(String),
}

// ─── Fund Availability ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FundAvailability {
    OneYear {
        fiscal_year: u32,
    },
    MultiYear {
        through: u32,
    },
    NoYear,
    #[serde(untagged)]
    Other(String),
}

// ─── Provisos ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proviso {
    pub proviso_type: ProvisoType,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub amount: Option<DollarAmount>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub raw_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProvisoType {
    Limitation,
    Transfer,
    Reporting,
    Condition,
    Prohibition,
    #[serde(untagged)]
    Other(String),
}

// ─── Earmarks ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Earmark {
    #[serde(default)]
    pub recipient: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub requesting_member: Option<String>,
}

// ─── CR Anomaly ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrAnomaly {
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub modification: String,
    #[serde(default)]
    pub delta: Option<i64>,
    #[serde(default)]
    pub raw_text: String,
}

// ─── Transfer Limit ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TransferLimit {
    Percentage(f64),
    FixedAmount(DollarAmount),
    #[serde(untagged)]
    Other(String),
}

// ─── Provisions (core extraction type) ───────────────────────────────────────

/// A single provision extracted from a bill.
/// Tagged by provision_type for serde. Every variant carries common fields:
/// section, division, title, confidence, raw_text, notes, cross_references.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provision_type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Provision {
    Appropriation {
        #[serde(default)]
        account_name: String,
        #[serde(default)]
        agency: Option<String>,
        #[serde(default)]
        program: Option<String>,
        amount: DollarAmount,
        #[serde(default)]
        fiscal_year: Option<u32>,
        #[serde(default)]
        availability: Option<FundAvailability>,
        #[serde(default)]
        provisos: Vec<Proviso>,
        #[serde(default)]
        earmarks: Vec<Earmark>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
        /// "top_level", "line_item", "sub_allocation", "proviso_amount", or ""
        #[serde(default)]
        detail_level: String,
        /// Parent account for sub-allocations
        #[serde(default)]
        parent_account: Option<String>,
    },
    Rescission {
        #[serde(default)]
        account_name: String,
        #[serde(default)]
        agency: Option<String>,
        amount: DollarAmount,
        #[serde(default)]
        reference_law: Option<String>,
        #[serde(default)]
        fiscal_years: Option<String>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
    },
    TransferAuthority {
        #[serde(default)]
        from_scope: String,
        #[serde(default)]
        to_scope: String,
        limit: TransferLimit,
        #[serde(default)]
        conditions: Vec<String>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
    },
    Limitation {
        #[serde(default)]
        description: String,
        #[serde(default)]
        amount: Option<DollarAmount>,
        #[serde(default)]
        account_name: Option<String>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
        #[serde(default)]
        parent_account: Option<String>,
    },
    DirectedSpending {
        #[serde(default)]
        account_name: Option<String>,
        amount: DollarAmount,
        earmark: Earmark,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
        /// "top_level", "line_item", "sub_allocation", "proviso_amount", or ""
        #[serde(default)]
        detail_level: String,
        /// Parent account for sub-allocations
        #[serde(default)]
        parent_account: Option<String>,
    },
    CrSubstitution {
        #[serde(default)]
        reference_act: String,
        #[serde(default)]
        reference_section: String,
        new_amount: DollarAmount,
        old_amount: DollarAmount,
        #[serde(default)]
        account_name: Option<String>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
    },
    MandatorySpendingExtension {
        #[serde(default)]
        program_name: String,
        #[serde(default)]
        statutory_reference: String,
        #[serde(default)]
        amount: Option<DollarAmount>,
        #[serde(default)]
        period: Option<String>,
        #[serde(default)]
        extends_through: Option<String>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
    },
    Directive {
        #[serde(default)]
        description: String,
        #[serde(default)]
        deadlines: Vec<String>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
    },
    Rider {
        #[serde(default)]
        description: String,
        #[serde(default)]
        policy_area: Option<String>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
    },
    ContinuingResolutionBaseline {
        reference_year: u32,
        #[serde(default)]
        reference_laws: Vec<String>,
        #[serde(default)]
        rate: String,
        #[serde(default)]
        duration: Option<String>,
        #[serde(default)]
        anomalies: Vec<CrAnomaly>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
    },
    Other {
        #[serde(default)]
        llm_classification: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        amounts: Vec<DollarAmount>,
        #[serde(default)]
        references: Vec<String>,
        #[serde(default)]
        metadata: HashMap<String, serde_json::Value>,
        // Common fields
        #[serde(default)]
        section: String,
        #[serde(default)]
        division: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        confidence: f32,
        #[serde(default)]
        raw_text: String,
        #[serde(default)]
        notes: Vec<String>,
        #[serde(default)]
        cross_references: Vec<CrossReference>,
    },
}

// ─── Provision Accessors ─────────────────────────────────────────────────────

impl Provision {
    /// The provision type as a string slug (matches the serde tag).
    pub fn type_str(&self) -> &str {
        match self {
            Provision::Appropriation { .. } => "appropriation",
            Provision::Rescission { .. } => "rescission",
            Provision::TransferAuthority { .. } => "transfer_authority",
            Provision::Limitation { .. } => "limitation",
            Provision::DirectedSpending { .. } => "directed_spending",
            Provision::CrSubstitution { .. } => "cr_substitution",
            Provision::MandatorySpendingExtension { .. } => "mandatory_spending_extension",
            Provision::Directive { .. } => "directive",
            Provision::Rider { .. } => "rider",
            Provision::ContinuingResolutionBaseline { .. } => "continuing_resolution_baseline",
            Provision::Other { .. } => "other",
        }
    }

    /// The account name, if this provision type carries one.
    pub fn account_name(&self) -> &str {
        match self {
            Provision::Appropriation { account_name, .. }
            | Provision::Rescission { account_name, .. } => account_name,
            Provision::DirectedSpending { account_name, .. } => {
                account_name.as_deref().unwrap_or("")
            }
            Provision::Limitation { account_name, .. } => account_name.as_deref().unwrap_or(""),
            Provision::CrSubstitution { account_name, .. } => account_name.as_deref().unwrap_or(""),
            _ => "",
        }
    }

    /// The agency name, if this provision type carries one.
    pub fn agency(&self) -> &str {
        match self {
            Provision::Appropriation { agency, .. } | Provision::Rescission { agency, .. } => {
                agency.as_deref().unwrap_or("")
            }
            _ => "",
        }
    }

    /// The raw_text excerpt (present on all provision types).
    pub fn raw_text(&self) -> &str {
        match self {
            Provision::Appropriation { raw_text, .. }
            | Provision::Rescission { raw_text, .. }
            | Provision::TransferAuthority { raw_text, .. }
            | Provision::Limitation { raw_text, .. }
            | Provision::DirectedSpending { raw_text, .. }
            | Provision::CrSubstitution { raw_text, .. }
            | Provision::MandatorySpendingExtension { raw_text, .. }
            | Provision::Directive { raw_text, .. }
            | Provision::Rider { raw_text, .. }
            | Provision::ContinuingResolutionBaseline { raw_text, .. }
            | Provision::Other { raw_text, .. } => raw_text,
        }
    }

    /// The section header (e.g. "SEC. 101"). Empty string if none.
    pub fn section(&self) -> &str {
        match self {
            Provision::Appropriation { section, .. }
            | Provision::Rescission { section, .. }
            | Provision::TransferAuthority { section, .. }
            | Provision::Limitation { section, .. }
            | Provision::DirectedSpending { section, .. }
            | Provision::CrSubstitution { section, .. }
            | Provision::MandatorySpendingExtension { section, .. }
            | Provision::Directive { section, .. }
            | Provision::Rider { section, .. }
            | Provision::ContinuingResolutionBaseline { section, .. }
            | Provision::Other { section, .. } => section,
        }
    }

    /// The division letter (e.g. "A"). None if the bill has no divisions.
    pub fn division(&self) -> Option<&str> {
        let opt = match self {
            Provision::Appropriation { division, .. }
            | Provision::Rescission { division, .. }
            | Provision::TransferAuthority { division, .. }
            | Provision::Limitation { division, .. }
            | Provision::DirectedSpending { division, .. }
            | Provision::CrSubstitution { division, .. }
            | Provision::MandatorySpendingExtension { division, .. }
            | Provision::Directive { division, .. }
            | Provision::Rider { division, .. }
            | Provision::ContinuingResolutionBaseline { division, .. }
            | Provision::Other { division, .. } => division,
        };
        opt.as_deref()
    }

    /// The dollar amount and its semantics, if this provision carries one.
    pub fn amount(&self) -> Option<&DollarAmount> {
        match self {
            Provision::Appropriation { amount, .. }
            | Provision::Rescission { amount, .. }
            | Provision::DirectedSpending { amount, .. } => Some(amount),
            Provision::CrSubstitution { new_amount, .. } => Some(new_amount),
            Provision::Limitation { amount, .. }
            | Provision::MandatorySpendingExtension { amount, .. } => amount.as_ref(),
            _ => None,
        }
    }

    /// The confidence score (0.0–1.0).
    pub fn confidence(&self) -> f32 {
        match self {
            Provision::Appropriation { confidence, .. }
            | Provision::Rescission { confidence, .. }
            | Provision::TransferAuthority { confidence, .. }
            | Provision::Limitation { confidence, .. }
            | Provision::DirectedSpending { confidence, .. }
            | Provision::CrSubstitution { confidence, .. }
            | Provision::MandatorySpendingExtension { confidence, .. }
            | Provision::Directive { confidence, .. }
            | Provision::Rider { confidence, .. }
            | Provision::ContinuingResolutionBaseline { confidence, .. }
            | Provision::Other { confidence, .. } => *confidence,
        }
    }

    /// The most relevant descriptive text for this provision.
    /// For directives/riders/limitations: the description field.
    /// For mandatory_spending_extension: the program_name.
    /// For appropriations/rescissions: the account_name.
    /// For other: the description or llm_classification.
    pub fn description(&self) -> &str {
        match self {
            Provision::Directive { description, .. }
            | Provision::Rider { description, .. }
            | Provision::Limitation { description, .. }
            | Provision::Other { description, .. } => description,
            Provision::MandatorySpendingExtension { program_name, .. } => program_name,
            Provision::Appropriation { account_name, .. }
            | Provision::Rescission { account_name, .. } => account_name,
            Provision::DirectedSpending { account_name, .. } => {
                account_name.as_deref().unwrap_or("")
            }
            Provision::CrSubstitution { account_name, .. } => account_name.as_deref().unwrap_or(""),
            Provision::ContinuingResolutionBaseline { rate, .. } => rate,
            Provision::TransferAuthority { from_scope, .. } => from_scope,
        }
    }

    /// The old_amount for CR substitutions (the amount being replaced).
    /// Returns None for all other provision types.
    pub fn old_amount(&self) -> Option<&DollarAmount> {
        match self {
            Provision::CrSubstitution { old_amount, .. } => Some(old_amount),
            _ => None,
        }
    }

    /// For verification: returns (text_as_written if applicable, raw_text).
    pub fn verification_text(&self) -> (Option<&str>, &str) {
        match self {
            Provision::Appropriation {
                amount, raw_text, ..
            }
            | Provision::Rescission {
                amount, raw_text, ..
            }
            | Provision::DirectedSpending {
                amount, raw_text, ..
            } => (Some(amount.text_as_written.as_str()), raw_text),
            Provision::CrSubstitution {
                new_amount,
                raw_text,
                ..
            } => (Some(new_amount.text_as_written.as_str()), raw_text),
            Provision::Limitation {
                amount, raw_text, ..
            }
            | Provision::MandatorySpendingExtension {
                amount, raw_text, ..
            } => (
                amount.as_ref().map(|a| a.text_as_written.as_str()),
                raw_text,
            ),
            Provision::TransferAuthority { raw_text, .. }
            | Provision::Directive { raw_text, .. }
            | Provision::Rider { raw_text, .. }
            | Provision::ContinuingResolutionBaseline { raw_text, .. }
            | Provision::Other { raw_text, .. } => (None, raw_text),
        }
    }
}

// ─── BillExtraction Methods ──────────────────────────────────────────────────

impl BillExtraction {
    /// Compute (total_budget_authority, total_rescissions) from the actual provisions.
    /// This is deterministic — does not use the LLM's self-reported summary.
    pub fn compute_totals(&self) -> (i64, i64) {
        let mut ba = 0i64;
        let mut rescissions = 0i64;
        for p in &self.provisions {
            if let Some(amt) = p.amount() {
                match p {
                    Provision::Appropriation { detail_level, .. } => {
                        if matches!(amt.semantics, AmountSemantics::NewBudgetAuthority) {
                            // Exclude sub-allocations and proviso amounts — they are
                            // breakdowns of a parent account, not additional money.
                            let dl = detail_level.as_str();
                            if dl != "sub_allocation" && dl != "proviso_amount" {
                                ba += amt.dollars().unwrap_or(0);
                            }
                        }
                    }
                    Provision::Rescission { .. } => {
                        if matches!(amt.semantics, AmountSemantics::Rescission) {
                            rescissions += amt.dollars().unwrap_or(0).abs();
                        }
                    }
                    _ => {}
                }
            }
        }
        (ba, rescissions)
    }
}

// ─── Relationships ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub from_id: String,
    pub to_id: String,
    pub rel_type: RelationType,
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
    pub source: SourceSpan,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RelationType {
    FundsAccount,
    AdministeredBy,
    JurisdictionOf,
    EnactedAs,
    Modifies,
    RequestedBy,
    AmendedBy,
    ContainsEarmark,
    SubjectToProviso,
    HasTransferAuthorityTo,
    References,
    CrossReferences,
    Supersedes,
    Other(String),
}

// ─── Bill-Level Output ───────────────────────────────────────────────────────

/// Unified output from a single extraction call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillExtraction {
    /// Schema version for this file format. None = pre-versioned data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    pub bill: BillInfo,
    #[serde(default)]
    pub provisions: Vec<Provision>,
    pub summary: ExtractionSummary,
    /// Maps chunk IDs to provision indices for traceability.
    /// Populated during multi-pass extraction; empty for single-pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunk_map: Vec<crate::approp::extraction::ChunkMapEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillInfo {
    #[serde(default)]
    pub identifier: String,
    pub classification: BillClassification,
    #[serde(default)]
    pub short_title: Option<String>,
    #[serde(default)]
    pub fiscal_years: Vec<u32>,
    #[serde(default)]
    pub divisions: Vec<String>,
    #[serde(default)]
    pub public_law: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BillClassification {
    Regular,
    ContinuingResolution,
    Omnibus,
    Supplemental,
    Rescissions,
    Minibus,
    #[serde(untagged)]
    Other(String),
}

impl std::fmt::Display for BillClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BillClassification::Regular => write!(f, "Regular"),
            BillClassification::ContinuingResolution => write!(f, "Continuing Resolution"),
            BillClassification::Omnibus => write!(f, "Omnibus"),
            BillClassification::Supplemental => write!(f, "Supplemental"),
            BillClassification::Rescissions => write!(f, "Rescissions"),
            BillClassification::Minibus => write!(f, "Minibus"),
            BillClassification::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Self-check summary produced by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionSummary {
    #[serde(default)]
    pub total_provisions: usize,
    #[serde(default)]
    pub by_division: HashMap<String, usize>,
    #[serde(default)]
    pub by_type: HashMap<String, usize>,
    #[serde(default)]
    pub total_budget_authority: i64,
    #[serde(default)]
    pub total_rescissions: i64,
    #[serde(default)]
    pub sections_with_no_provisions: Vec<String>,
    #[serde(default)]
    pub flagged_issues: Vec<String>,
}

// ─── Provenance & How a Fact Was Determined ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProvenanceLevel {
    LlmExtraction {
        model: String,
        confidence: f32,
    },
    Computed {
        inputs: Vec<String>,
        method: String,
    },
    ExternalReference {
        source_name: String,
        url: Option<String>,
    },
}

// ─── Verification (Rust-produced, not LLM-produced) ──────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    #[serde(default)]
    pub arithmetic_checks: Vec<ArithmeticCheck>,
    #[serde(default)]
    pub missing_sections: Vec<String>,
    #[serde(default)]
    pub low_confidence_items: Vec<String>,
    #[serde(default)]
    pub corrections: Vec<Correction>,
    pub overall_quality: Quality,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArithmeticCheck {
    pub scope: String,
    #[serde(default)]
    pub expected_total: Option<i64>,
    pub extracted_sum: i64,
    pub discrepancy: i64,
    pub status: CheckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail,
    NoReferenceTotal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correction {
    pub field: String,
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    High,
    Medium,
    Low,
}

// ─── Extraction Metadata ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionMetadata {
    pub extraction_version: String,
    pub prompt_version: String,
    pub model: String,
    pub schema_version: String,
    #[serde(default)]
    pub source_pdf_sha256: Option<String>,
    pub extracted_text_sha256: String,
    pub timestamp: String,
}
