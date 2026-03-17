use super::ontology::Provision;
use super::text_index::TextIndex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Normalize text for comparison: collapse whitespace, normalize quotes and dashes.
fn normalize_for_comparison(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut last_was_space = false;
    for c in s.chars() {
        let c = match c {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2014}' | '\u{2013}' | '\u{2012}' => '-',
            c if c.is_whitespace() => {
                if !last_was_space {
                    last_was_space = true;
                    result.push(' ');
                }
                continue;
            }
            c => c,
        };
        last_was_space = false;
        result.push(c);
    }
    result.trim().to_string()
}

/// Remove ALL spaces for the most aggressive comparison tier.
fn spaceless(s: &str) -> String {
    normalize_for_comparison(s).replace(' ', "")
}

/// Result of verifying a single provision's dollar amount against source text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmountCheck {
    pub provision_index: usize,
    pub text_as_written: String,
    pub found_in_source: bool,
    pub source_positions: Vec<usize>,
    pub status: CheckResult,
}

/// How closely the raw_text matched the source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchTier {
    Exact,
    Normalized,
    Spaceless,
    NoMatch,
}

/// Result of checking that raw_text is a verbatim substring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTextCheck {
    pub provision_index: usize,
    pub raw_text_preview: String,
    pub is_verbatim_substring: bool,
    pub match_tier: MatchTier,
    pub found_at_position: Option<usize>,
}

/// Arithmetic check result for a group of provisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArithmeticResult {
    pub scope: String,
    pub extracted_sum: i64,
    pub stated_total: Option<i64>,
    pub status: CheckResult,
}

/// Report on dollar amounts in the text that were not extracted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletenessReport {
    pub total_dollar_amounts_in_text: usize,
    pub accounted_for: usize,
    pub unaccounted: Vec<UnaccountedAmount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaccountedAmount {
    pub text: String,
    pub value: i64,
    pub position: usize,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckResult {
    Verified,
    NotFound,
    Ambiguous,
    Mismatch,
    NoReference,
}

/// Full verification report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    /// Schema version for this file format. None = pre-versioned data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    pub amount_checks: Vec<AmountCheck>,
    pub raw_text_checks: Vec<RawTextCheck>,
    pub arithmetic_checks: Vec<ArithmeticResult>,
    pub completeness: CompletenessReport,
    pub summary: VerificationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub total_provisions: usize,
    pub amounts_verified: usize,
    pub amounts_not_found: usize,
    pub amounts_ambiguous: usize,
    pub raw_text_exact: usize,
    pub raw_text_normalized: usize,
    pub raw_text_spaceless: usize,
    pub raw_text_no_match: usize,
    pub completeness_pct: f64,
    #[serde(default)]
    pub provisions_by_detail_level: HashMap<String, usize>,
}

/// Run deterministic verification of extracted provisions against source text.
pub fn verify_provisions(
    provisions: &[Provision],
    source_text: &str,
    index: &TextIndex,
) -> VerificationReport {
    let mut amount_checks = Vec::new();
    let mut raw_text_checks = Vec::new();

    // Track which dollar amounts in the index are accounted for
    let mut accounted_positions: std::collections::HashSet<usize> =
        std::collections::HashSet::new();

    for (i, provision) in provisions.iter().enumerate() {
        let (amount_text, raw_text) = provision.verification_text();

        // 1. Amount verification
        if let Some(written) = amount_text {
            let matches = index.find_all_amounts(written);
            let status = match matches.len() {
                0 => {
                    // Try searching the raw source text directly (index might have slight differences)
                    if let Some(pos) = source_text.find(written) {
                        // Mark the found position as accounted for
                        // Find the closest dollar ref in the index by position
                        if let Some(closest) = index
                            .dollar_amounts
                            .iter()
                            .min_by_key(|d| (d.position as isize - pos as isize).unsigned_abs())
                        {
                            accounted_positions.insert(closest.position);
                        }
                        CheckResult::Verified
                    } else {
                        warn!("Amount not found in source: {written} (provision {i})");
                        CheckResult::NotFound
                    }
                }
                1 => {
                    accounted_positions.insert(matches[0].position);
                    CheckResult::Verified
                }
                _ => {
                    // Multiple matches — mark all as accounted
                    for m in &matches {
                        accounted_positions.insert(m.position);
                    }
                    debug!(
                        "Amount {} found {} times (provision {})",
                        written,
                        matches.len(),
                        i
                    );
                    CheckResult::Ambiguous
                }
            };

            amount_checks.push(AmountCheck {
                provision_index: i,
                text_as_written: written.to_string(),
                found_in_source: !matches!(status, CheckResult::NotFound),
                source_positions: matches.iter().map(|m| m.position).collect(),
                status,
            });
        }

        // 2. Raw text verification with tiered matching
        let preview: String = if raw_text.len() > 80 {
            // Find a char boundary at or before byte 80 to avoid panicking on multi-byte chars
            let mut end = 80;
            while end > 0 && !raw_text.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &raw_text[..end])
        } else {
            raw_text.to_string()
        };
        if !raw_text.is_empty() {
            let (is_match, tier, found_pos) = if let Some(pos) = source_text.find(raw_text) {
                (true, MatchTier::Exact, Some(pos))
            } else if normalize_for_comparison(raw_text).len() > 10
                && normalize_for_comparison(source_text)
                    .contains(&normalize_for_comparison(raw_text))
            {
                (true, MatchTier::Normalized, None)
            } else if spaceless(raw_text).len() > 10
                && spaceless(source_text).contains(&spaceless(raw_text))
            {
                (true, MatchTier::Spaceless, None)
            } else {
                (false, MatchTier::NoMatch, None)
            };

            let _ = is_match; // suppress unused warning; kept for clarity

            raw_text_checks.push(RawTextCheck {
                provision_index: i,
                raw_text_preview: preview,
                is_verbatim_substring: matches!(tier, MatchTier::Exact),
                match_tier: tier,
                found_at_position: found_pos,
            });
        }
    }

    // 3. Completeness check
    let unaccounted: Vec<UnaccountedAmount> = index
        .dollar_amounts
        .iter()
        .filter(|d| !accounted_positions.contains(&d.position))
        .map(|d| UnaccountedAmount {
            text: d.text.clone(),
            value: d.value,
            position: d.position,
            context: d.context.clone(),
        })
        .collect();

    let completeness = CompletenessReport {
        total_dollar_amounts_in_text: index.dollar_amounts.len(),
        accounted_for: accounted_positions.len(),
        unaccounted,
    };

    // 4. Arithmetic checks (placeholder — needs section grouping info)
    let arithmetic_checks = Vec::new();

    // Summary
    let amounts_verified = amount_checks
        .iter()
        .filter(|c| matches!(c.status, CheckResult::Verified))
        .count();
    let amounts_not_found = amount_checks
        .iter()
        .filter(|c| matches!(c.status, CheckResult::NotFound))
        .count();
    let amounts_ambiguous = amount_checks
        .iter()
        .filter(|c| matches!(c.status, CheckResult::Ambiguous))
        .count();
    let raw_text_exact = raw_text_checks
        .iter()
        .filter(|c| matches!(c.match_tier, MatchTier::Exact))
        .count();
    let raw_text_normalized = raw_text_checks
        .iter()
        .filter(|c| matches!(c.match_tier, MatchTier::Normalized))
        .count();
    let raw_text_spaceless = raw_text_checks
        .iter()
        .filter(|c| matches!(c.match_tier, MatchTier::Spaceless))
        .count();
    let raw_text_no_match = raw_text_checks
        .iter()
        .filter(|c| matches!(c.match_tier, MatchTier::NoMatch))
        .count();
    let completeness_pct = if index.dollar_amounts.is_empty() {
        100.0
    } else {
        (accounted_positions.len() as f64 / index.dollar_amounts.len() as f64) * 100.0
    };

    // Count provisions by detail level
    let mut by_detail = HashMap::new();
    for provision in provisions {
        let level = if let Provision::Appropriation { detail_level, .. } = provision {
            if detail_level.is_empty() {
                "unspecified"
            } else {
                detail_level.as_str()
            }
        } else {
            "n/a"
        };
        *by_detail.entry(level.to_string()).or_insert(0usize) += 1;
    }

    let summary = VerificationSummary {
        total_provisions: provisions.len(),
        amounts_verified,
        amounts_not_found,
        amounts_ambiguous,
        raw_text_exact,
        raw_text_normalized,
        raw_text_spaceless,
        raw_text_no_match,
        completeness_pct,
        provisions_by_detail_level: by_detail,
    };

    VerificationReport {
        schema_version: Some("1.0".to_string()),
        amount_checks,
        raw_text_checks,
        arithmetic_checks,
        completeness,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_for_comparison() {
        // Curly quotes → straight
        assert_eq!(
            normalize_for_comparison("\u{201c}hello\u{201d}"),
            "\"hello\""
        );
        assert_eq!(normalize_for_comparison("\u{2018}world\u{2019}"), "'world'");
        // Dashes → hyphen
        assert_eq!(normalize_for_comparison("a\u{2014}b"), "a-b");
        assert_eq!(normalize_for_comparison("a\u{2013}b"), "a-b");
        assert_eq!(normalize_for_comparison("a\u{2012}b"), "a-b");
        // Collapse whitespace
        assert_eq!(normalize_for_comparison("a  b   c"), "a b c");
        assert_eq!(normalize_for_comparison("  a  b  "), "a b");
        // Tabs and newlines
        assert_eq!(normalize_for_comparison("a\t\nb"), "a b");
    }

    #[test]
    fn test_spaceless() {
        assert_eq!(spaceless("hello world"), "helloworld");
        assert_eq!(spaceless("  a  b  c  "), "abc");
    }

    #[test]
    fn test_normalize_combined() {
        let input = "\u{201c}Public Law 118\u{2013}47\u{201d}";
        assert_eq!(normalize_for_comparison(input), "\"Public Law 118-47\"");
    }
}
