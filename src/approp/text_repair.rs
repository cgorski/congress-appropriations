//! Raw-text verification and repair — Stage 2 of the extraction pipeline.
//!
//! Every provision produced by the LLM extraction carries a `raw_text` field
//! that is *supposed* to be a verbatim substring of the enrolled bill source
//! text. In practice, the LLM sometimes makes small substitutions:
//!
//! - `"clause"` instead of `"subsection"` (legal term swap)
//! - `"on"` instead of `"in"` (preposition swap)
//! - Straight quotes instead of Unicode curly quotes
//! - Newlines collapsed into spaces
//!
//! This module detects those mismatches and repairs them deterministically,
//! without any LLM calls. The algorithm has three tiers, each a fallback
//! for the previous:
//!
//! 1. **Prefix match** — find the longest prefix of `raw_text` that appears
//!    in the source, then copy `target_len` bytes of actual source from that
//!    position. Handles single-word substitutions that occur after a long
//!    correct prefix (e.g., 80 chars match, then "clause" vs "subsection").
//!
//! 2. **Substring match** — find the longest *internal* substring (not
//!    anchored at position 0) of `raw_text` that appears in the source.
//!    Walk backward by the substring's offset to recover the provision start.
//!    Handles cases where the first few characters are generic (e.g., `"(a) "`)
//!    but a distinctive phrase later in the text is unique.
//!
//! 3. **Normalized position mapping** — build a character-level map between
//!    a normalized version of the source (whitespace/quotes collapsed) and the
//!    original source. Search in normalized space, then map the hit position
//!    back to original byte offsets. Handles curly-quote and newline differences.
//!
//! After repair, every provision's `raw_text` is guaranteed to be a verbatim
//! substring of the source text, and a [`TextSpan`] records the exact byte
//! range for 1-to-1 correspondence.
//!
//! # Pipeline position
//!
//! ```text
//! extract (stage 1) → verify-text (stage 2) → enrich → embed → …
//! ```

use crate::approp::ontology::{TextMatchTier, TextSpan};
use serde_json::Value;
use std::path::Path;
use tracing::{debug, warn};

// ─── Public report types ─────────────────────────────────────────────────────

/// Report from running verify-text on a single bill.
#[derive(Debug, Clone, Default)]
pub struct VerifyTextReport {
    /// Total provisions checked.
    pub total: usize,
    /// Provisions whose `raw_text` was already an exact substring.
    pub exact: usize,
    /// Provisions whose `raw_text` matched after whitespace/quote normalization
    /// (counted by the Rust verification module, not repaired here).
    pub normalized: usize,
    /// Provisions repaired by Tier 1 (prefix match).
    pub repaired_prefix: usize,
    /// Provisions repaired by Tier 2 (substring match).
    pub repaired_substring: usize,
    /// Provisions repaired by Tier 3 (normalized position mapping).
    pub repaired_normalized: usize,
    /// Provisions that could not be located in the source at any tier.
    pub unverified: usize,
    /// Number of [`TextSpan`]s added or updated.
    pub spans_added: usize,
}

impl VerifyTextReport {
    /// Total provisions that are now traceable (exact + all repair tiers).
    pub fn traceable(&self) -> usize {
        self.exact + self.repaired_prefix + self.repaired_substring + self.repaired_normalized
    }
}

impl std::fmt::Display for VerifyTextReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} total: {} exact, {} repaired ({} prefix, {} substr, {} norm), {} unverified",
            self.total,
            self.exact,
            self.repaired_prefix + self.repaired_substring + self.repaired_normalized,
            self.repaired_prefix,
            self.repaired_substring,
            self.repaired_normalized,
            self.unverified,
        )
    }
}

// ─── Normalization helpers (mirrors verification.rs logic) ───────────────────

/// Collapse whitespace, normalize quotes and dashes for comparison.
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

/// Build a position map from normalized character indices to original byte
/// offsets. Returns `(normalized_string, map)` where `map[i]` is the byte
/// offset in the original string corresponding to normalized character `i`.
fn build_position_map(source: &str) -> (String, Vec<usize>) {
    let mut norm_chars = String::with_capacity(source.len());
    let mut norm_to_orig: Vec<usize> = Vec::with_capacity(source.len());
    let mut in_ws = false;

    for (byte_offset, c) in source.char_indices() {
        let nc = match c {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2014}' | '\u{2013}' | '\u{2012}' => '-',
            c if c.is_whitespace() => {
                if !in_ws {
                    in_ws = true;
                    norm_chars.push(' ');
                    norm_to_orig.push(byte_offset);
                }
                continue;
            }
            c => c,
        };
        in_ws = false;
        norm_chars.push(nc);
        norm_to_orig.push(byte_offset);
    }

    (norm_chars, norm_to_orig)
}

// ─── Tier 1: Prefix match ────────────────────────────────────────────────────

/// Try to repair `raw_text` by finding its longest matching prefix in `source`.
///
/// Tries prefix lengths from `max_prefix` down to `min_prefix`. When found,
/// copies `target_len` bytes of actual source text from the match position.
///
/// Returns `Some((corrected, start, tier))` or `None`.
fn try_prefix_match(
    raw_text: &str,
    source: &str,
    target_len: usize,
    max_prefix: usize,
    min_prefix: usize,
) -> Option<(String, usize, TextMatchTier)> {
    let max_p = max_prefix.min(raw_text.len());
    for prefix_len in (min_prefix..=max_p).rev() {
        // Ensure we're on a char boundary
        if !raw_text.is_char_boundary(prefix_len) {
            continue;
        }
        let prefix = &raw_text[..prefix_len];
        if let Some(pos) = source.find(prefix) {
            let end = source.len().min(pos + target_len);
            // Ensure we end on a char boundary
            let mut end = end;
            while end > pos && !source.is_char_boundary(end) {
                end -= 1;
            }
            let corrected = &source[pos..end];
            return Some((corrected.to_string(), pos, TextMatchTier::RepairedPrefix));
        }
    }
    None
}

// ─── Tier 2: Substring match ─────────────────────────────────────────────────

/// Try to repair `raw_text` by finding its longest internal substring in `source`,
/// then walking backward to recover the provision start.
///
/// Scans substrings starting from offsets 0..`max_start_offset` in `raw_text`.
/// For each offset, tries substring lengths from `max_sub_len` down to `min_sub_len`.
///
/// Returns `Some((corrected, start, tier))` or `None`.
fn try_substring_match(
    raw_text: &str,
    source: &str,
    target_len: usize,
    max_start_offset: usize,
    max_sub_len: usize,
    min_sub_len: usize,
) -> Option<(String, usize, TextMatchTier)> {
    let mut best_pos: Option<usize> = None;
    let mut best_len: usize = 0;
    let mut best_offset: usize = 0;

    let max_offset = max_start_offset.min(raw_text.len().saturating_sub(min_sub_len));

    for start_offset in 0..=max_offset {
        if !raw_text.is_char_boundary(start_offset) {
            continue;
        }
        let remaining = raw_text.len() - start_offset;
        let max_sl = max_sub_len.min(remaining);
        for sub_len in (min_sub_len..=max_sl).rev() {
            let end = start_offset + sub_len;
            if !raw_text.is_char_boundary(end) {
                continue;
            }
            let sub = &raw_text[start_offset..end];
            if let Some(pos) = source.find(sub) {
                if sub_len > best_len {
                    best_len = sub_len;
                    best_pos = Some(pos);
                    best_offset = start_offset;
                }
                break; // Found best for this offset
            }
        }
        if best_len >= 40 {
            break; // Good enough, stop early
        }
    }

    if let Some(match_pos) = best_pos.filter(|_| best_len >= min_sub_len) {
        // Walk backward from match_pos by the substring's offset in raw_text
        let prov_start = match_pos.saturating_sub(best_offset);
        let end = source.len().min(prov_start + target_len);
        // Ensure char boundaries
        let mut end = end;
        while end > prov_start && !source.is_char_boundary(end) {
            end -= 1;
        }
        let corrected = &source[prov_start..end];
        // Verify the corrected text is actually in source (should be by construction)
        debug_assert!(source.contains(corrected));
        return Some((
            corrected.to_string(),
            prov_start,
            TextMatchTier::RepairedSubstring,
        ));
    }

    None
}

// ─── Tier 3: Normalized position mapping ─────────────────────────────────────

/// Try to repair `raw_text` by searching in normalized space and mapping back
/// to original byte positions.
///
/// This handles cases where the only differences are whitespace (newlines vs
/// spaces) and quote characters (curly vs straight). The normalized search
/// finds the position, and the position map recovers the original bytes.
///
/// Returns `Some((corrected, start, tier))` or `None`.
fn try_normalized_match(
    raw_text: &str,
    source: &str,
    norm_source: &str,
    norm_to_orig: &[usize],
    target_len: usize,
) -> Option<(String, usize, TextMatchTier)> {
    let norm_raw = normalize_for_comparison(raw_text);
    if norm_raw.is_empty() {
        return None;
    }

    // Search for the normalized raw_text in the normalized source
    let npos = norm_source.find(&norm_raw)?;

    // Map back to the original byte offset
    if npos >= norm_to_orig.len() {
        return None;
    }
    let orig_start = norm_to_orig[npos];

    // Find the original end position
    let norm_end = npos + norm_raw.len();
    let orig_end = if norm_end < norm_to_orig.len() {
        norm_to_orig[norm_end]
    } else {
        source.len()
    };

    // Copy a chunk of original source, using the larger of raw_text length or mapped range
    let copy_len = target_len.max(orig_end - orig_start);
    let end = source.len().min(orig_start + copy_len);

    // Ensure char boundary
    let mut end = end;
    while end > orig_start && !source.is_char_boundary(end) {
        end -= 1;
    }

    let corrected = &source[orig_start..end];

    // Verify the corrected text is in source
    if source.contains(corrected) {
        Some((
            corrected.to_string(),
            orig_start,
            TextMatchTier::RepairedNormalized,
        ))
    } else {
        None
    }
}

// ─── Main repair function ────────────────────────────────────────────────────

/// Attempt to locate `raw_text` in `source` and produce a verified [`TextSpan`].
///
/// If `raw_text` is already verbatim in the source, returns a span immediately.
/// Otherwise, tries the 3-tier repair algorithm and returns a corrected
/// `raw_text` along with its span.
///
/// Returns `(maybe_corrected_raw_text, span, tier)`. If the first element is
/// `Some`, the caller should replace `raw_text` with it. The span is always
/// populated on success.
pub fn locate_raw_text(
    raw_text: &str,
    source: &str,
    source_filename: &str,
    norm_source: &str,
    norm_to_orig: &[usize],
) -> (Option<String>, Option<TextSpan>, TextMatchTier) {
    if raw_text.is_empty() || raw_text.trim().is_empty() {
        return (None, None, TextMatchTier::Exact);
    }

    let target_len = raw_text.len().max(150);

    // ── Exact match ──────────────────────────────────────────────────────
    if let Some(pos) = source.find(raw_text) {
        let span = TextSpan {
            start: pos,
            end: pos + raw_text.len(),
            file: source_filename.to_string(),
            verified: true,
            match_tier: TextMatchTier::Exact,
        };
        return (None, Some(span), TextMatchTier::Exact);
    }

    // ── Tier 1: Prefix match ─────────────────────────────────────────────
    if let Some((corrected, start, tier)) = try_prefix_match(raw_text, source, target_len, 80, 15) {
        let span = TextSpan {
            start,
            end: start + corrected.len(),
            file: source_filename.to_string(),
            verified: true,
            match_tier: tier.clone(),
        };
        return (Some(corrected), Some(span), tier);
    }

    // ── Tier 2: Substring match ──────────────────────────────────────────
    if let Some((corrected, sub_start, tier)) =
        try_substring_match(raw_text, source, target_len, 40, 60, 15)
    {
        let span = TextSpan {
            start: sub_start,
            end: sub_start + corrected.len(),
            file: source_filename.to_string(),
            verified: true,
            match_tier: tier.clone(),
        };
        return (Some(corrected), Some(span), tier);
    }

    // ── Tier 3: Normalized position mapping ──────────────────────────────
    if let Some((corrected, _start, tier)) =
        try_normalized_match(raw_text, source, norm_source, norm_to_orig, target_len)
    {
        // Final verification: the corrected text must be findable as exact substring
        if source.contains(corrected.as_str()) {
            let verify_pos = source.find(corrected.as_str()).unwrap();
            let span = TextSpan {
                start: verify_pos,
                end: verify_pos + corrected.len(),
                file: source_filename.to_string(),
                verified: true,
                match_tier: tier.clone(),
            };
            return (Some(corrected), Some(span), tier);
        }
    }

    // ── All tiers failed ─────────────────────────────────────────────────
    (None, None, TextMatchTier::Exact) // tier is irrelevant on failure
}

// ─── Bill-level verify and repair ────────────────────────────────────────────

/// Load the source `.txt` file for a bill directory.
///
/// Prefers `BILLS-*.txt` (generated during extraction from XML).
/// Returns `None` if no source text is available.
pub fn load_source_text(bill_dir: &Path) -> Option<String> {
    // Look for .txt file first
    let entries = std::fs::read_dir(bill_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "txt")
            && path
                .file_stem()
                .is_some_and(|n| n.to_string_lossy().starts_with("BILLS-"))
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            return Some(text);
        }
    }
    None
}

/// Find the `.txt` source filename in a bill directory.
pub fn source_filename(bill_dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(bill_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "txt")
            && path
                .file_stem()
                .is_some_and(|n| n.to_string_lossy().starts_with("BILLS-"))
        {
            return path.file_name().map(|n| n.to_string_lossy().to_string());
        }
    }
    None
}

/// Verify and optionally repair all provisions in a bill's extraction.
///
/// Works at the `serde_json::Value` level so it can write inline `source_span`
/// directly on each provision object — without needing to modify the typed
/// `Provision` enum (which has 11 variants). The `source_span` field is
/// ignored by the Rust deserializer (Serde skips unknown fields) but is
/// available to Python, JavaScript, and other consumers reading the JSON.
///
/// When `repair` is true, broken `raw_text` fields are replaced with verbatim
/// source excerpts and `source_span` objects are written on each provision.
///
/// When `repair` is false, only analysis is performed — no data is modified.
///
/// Returns a report summarizing what was found and fixed.
pub fn verify_and_repair_bill_json(
    extraction_value: &mut Value,
    source: &str,
    source_file: &str,
    repair: bool,
) -> VerifyTextReport {
    let mut report = VerifyTextReport::default();

    let provisions = match extraction_value
        .get_mut("provisions")
        .and_then(|v| v.as_array_mut())
    {
        Some(arr) => arr,
        None => return report,
    };

    let n = provisions.len();
    report.total = n;

    // Pre-compute normalized source and position map (once, shared across all provisions)
    let (norm_source, norm_to_orig) = build_position_map(source);

    for (i, prov) in provisions.iter_mut().enumerate() {
        let raw_text = prov
            .get("raw_text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if raw_text.is_empty() || raw_text.trim().is_empty() {
            report.exact += 1;
            continue;
        }

        let (maybe_corrected, maybe_span, tier) =
            locate_raw_text(&raw_text, source, source_file, &norm_source, &norm_to_orig);

        match (&maybe_corrected, &maybe_span) {
            // Already exact — no repair needed, just record the span
            (None, Some(span)) => {
                report.exact += 1;
                if repair {
                    write_inline_span(prov, span);
                    report.spans_added += 1;
                }
            }
            // Repaired — update raw_text and record span
            (Some(corrected), Some(span)) => {
                match tier {
                    TextMatchTier::RepairedPrefix => report.repaired_prefix += 1,
                    TextMatchTier::RepairedSubstring => report.repaired_substring += 1,
                    TextMatchTier::RepairedNormalized => report.repaired_normalized += 1,
                    TextMatchTier::Exact => report.exact += 1,
                }
                if repair {
                    prov["raw_text"] = Value::String(corrected.clone());
                    write_inline_span(prov, span);
                    report.spans_added += 1;
                }
            }
            // Could not locate at any tier
            _ => {
                report.unverified += 1;
                debug!(
                    "Provision {i}: raw_text not found in source (first 60: {:?})",
                    &raw_text[..raw_text.len().min(60)]
                );
            }
        }
    }

    if report.unverified > 0 {
        warn!(
            "{} provisions could not be located in source text",
            report.unverified
        );
    }

    report
}

/// Write a [`TextSpan`] as an inline `source_span` JSON object on a provision.
fn write_inline_span(provision: &mut Value, span: &TextSpan) {
    let span_value = serde_json::json!({
        "start": span.start,
        "end": span.end,
        "file": span.file,
        "verified": span.verified,
        "match_tier": span.match_tier,
    });
    provision["source_span"] = span_value;
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let source = "For necessary expenses of the Secret Service, $3,007,982,000.";
        let raw = "For necessary expenses of the Secret Service, $3,007,982,000.";
        let (norm, map) = build_position_map(source);
        let (corrected, span, tier) = locate_raw_text(raw, source, "test.txt", &norm, &map);
        assert!(corrected.is_none(), "should not correct an exact match");
        assert!(span.is_some());
        assert_eq!(tier, TextMatchTier::Exact);
        let span = span.unwrap();
        assert!(span.verified);
        assert_eq!(&source[span.start..span.end], raw);
    }

    #[test]
    fn test_prefix_repair_clause_vs_subsection() {
        let source =
            "SEC. 104. Section 3 of the Act is amended\u{2014} (1) in subsection (b)\u{2014}";
        let raw = "SEC. 104. Section 3 of the Act is amended\u{2014} (1) on clause (b)\u{2014}";
        let (norm, map) = build_position_map(source);
        let (corrected, span, tier) = locate_raw_text(raw, source, "test.txt", &norm, &map);
        assert!(corrected.is_some(), "should produce a corrected raw_text");
        assert!(span.is_some());
        let span = span.unwrap();
        assert!(span.verified);
        // The corrected text must be in the source
        assert!(source.contains(corrected.as_ref().unwrap().as_str()));
        // Tier should be prefix or substring
        assert!(matches!(
            tier,
            TextMatchTier::RepairedPrefix | TextMatchTier::RepairedSubstring
        ));
    }

    #[test]
    fn test_substring_repair_generic_prefix() {
        let source = "blah blah (a) Subtitle A of title IV of the Homeland Security Act more text here and there and so on and so forth extending to a reasonable length";
        let raw =
            "(a) Subtitle A of title IV of the Homeland Security Act of 2002 is amended by adding";
        // The prefix "(a) " is too common, but "Subtitle A of title IV" is unique
        let (norm, map) = build_position_map(source);
        let (corrected, span, _tier) = locate_raw_text(raw, source, "test.txt", &norm, &map);
        assert!(span.is_some(), "should find via substring match");
        let span = span.unwrap();
        assert!(span.verified);
        if let Some(ref c) = corrected {
            assert!(source.contains(c.as_str()));
        }
    }

    #[test]
    fn test_normalized_repair_curly_quotes() {
        let source = "shall use \u{2018}\u{2018}$425,000,000\u{2019}\u{2019} of funds available";
        let raw = "shall use ''$425,000,000'' of funds available";
        let (norm, map) = build_position_map(source);
        let (corrected, span, _tier) = locate_raw_text(raw, source, "test.txt", &norm, &map);
        assert!(span.is_some(), "should find via some tier");
        let span = span.unwrap();
        assert!(span.verified);
        // The span must point to real source bytes
        assert!(span.start < source.len());
        assert!(span.end <= source.len());
        // Either the original was found or a corrected version was
        if let Some(ref c) = corrected {
            assert!(source.contains(c.as_str()));
        }
    }

    #[test]
    fn test_normalized_repair_newline_vs_space() {
        let source = "SEC. 755.\nSection 313(b) of the Rural Electrification Act";
        let raw = "SEC. 755. Section 313(b) of the Rural Electrification Act";
        let (norm, map) = build_position_map(source);
        let (_corrected, span, _tier) = locate_raw_text(raw, source, "test.txt", &norm, &map);
        assert!(span.is_some(), "should find despite newline vs space");
        let span = span.unwrap();
        assert!(span.verified);
    }

    #[test]
    fn test_empty_raw_text() {
        let source = "some source text";
        let raw = "";
        let (norm, map) = build_position_map(source);
        let (corrected, span, _) = locate_raw_text(raw, source, "test.txt", &norm, &map);
        assert!(corrected.is_none());
        assert!(span.is_none());
    }

    #[test]
    fn test_unfindable_text() {
        let source = "The quick brown fox jumps over the lazy dog.";
        let raw =
            "Completely unrelated text that is nowhere in the source document at all whatsoever.";
        let (norm, map) = build_position_map(source);
        let (corrected, span, _) = locate_raw_text(raw, source, "test.txt", &norm, &map);
        assert!(corrected.is_none());
        assert!(span.is_none());
    }

    #[test]
    fn test_build_position_map_preserves_length() {
        let source = "Hello\n  world\t\tfoo";
        let (norm, map) = build_position_map(source);
        assert_eq!(norm, "Hello world foo");
        assert_eq!(norm.len(), map.len());
    }

    #[test]
    fn test_span_invariant() {
        // The fundamental invariant: source[span.start..span.end] == raw_text (after repair)
        let source = "PREFIX SEC. 104. Section 3 of the Act is amended\u{2014} (1) in subsection (b)\u{2014} (A) by striking and inserting SUFFIX";
        let raw = "SEC. 104. Section 3 of the Act is amended\u{2014} (1) on clause (b)\u{2014} (A) by striking";
        let (norm, map) = build_position_map(source);
        let (corrected, span, _) = locate_raw_text(raw, source, "test.txt", &norm, &map);
        let span = span.expect("should find a span");
        let final_text = corrected.as_deref().unwrap_or(raw);
        assert_eq!(
            &source[span.start..span.end],
            final_text,
            "span invariant violated"
        );
    }
}
