use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::debug;

/// A chunk of bill text to be sent as a single LLM extraction call.
/// Produced by `build_chunks()` which adaptively splits the bill at
/// division, title, or paragraph boundaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionChunk {
    /// Human-readable label, e.g. "A-I" or "A-IIa"
    pub label: String,
    /// Division letter (e.g. "A"), or empty for non-omnibus bills
    pub division: String,
    /// Title numeral (e.g. "I", "VII"), or empty
    pub title: String,
    /// Character offset of the chunk start in the source text
    pub start: usize,
    /// Character offset of the chunk end
    pub end: usize,
    /// Estimated token count (chars / 4)
    pub estimated_tokens: usize,
}

/// Build extraction chunks from bill text using adaptive splitting.
///
/// The strategy:
/// 1. If the bill has no divisions, return a single chunk (entire bill).
/// 2. Split into divisions.
/// 3. Within each division, split into titles.
/// 4. If any title exceeds `max_chunk_tokens`, split it at paragraph
///    boundaries (`\n\n`) into roughly equal halves.
///
/// Returns the preamble end position (text before the first chunk) and the chunks.
pub fn build_chunks(text: &str, max_chunk_tokens: usize) -> (usize, Vec<ExtractionChunk>) {
    let divisions = find_divisions(text);

    if divisions.is_empty() {
        // Non-omnibus bill: single chunk
        return (
            0,
            vec![ExtractionChunk {
                label: "full".to_string(),
                division: String::new(),
                title: String::new(),
                start: 0,
                end: text.len(),
                estimated_tokens: text.len() / 4,
            }],
        );
    }

    let preamble_end = divisions[0].start;
    let mut chunks = Vec::new();

    for div in &divisions {
        let div_text = &text[div.start..div.end];
        let div_tokens = div.estimated_tokens;

        // Find TITLE headers within this division
        let titles = find_titles_in(div_text);

        if titles.is_empty() || div_tokens <= max_chunk_tokens {
            // Small division or no titles: one chunk for the whole division
            chunks.push(ExtractionChunk {
                label: div.letter.clone(),
                division: div.letter.clone(),
                title: String::new(),
                start: div.start,
                end: div.end,
                estimated_tokens: div_tokens,
            });
            continue;
        }

        // Split by title
        for (t_start_rel, t_end_rel, numeral) in titles.iter() {
            let abs_start = div.start + t_start_rel;
            let abs_end = div.start + t_end_rel;
            let title_tokens = (abs_end - abs_start) / 4;

            if title_tokens <= max_chunk_tokens {
                chunks.push(ExtractionChunk {
                    label: format!("{}-{}", div.letter, numeral),
                    division: div.letter.clone(),
                    title: numeral.clone(),
                    start: abs_start,
                    end: abs_end,
                    estimated_tokens: title_tokens,
                });
            } else {
                // Title too large: split at paragraph boundaries
                let title_text = &text[abs_start..abs_end];
                let sub_chunks = split_at_paragraphs(title_text, max_chunk_tokens);
                for (j, (sub_start, sub_end)) in sub_chunks.iter().enumerate() {
                    let suffix = (b'a' + j as u8) as char;
                    chunks.push(ExtractionChunk {
                        label: format!("{}-{}{}", div.letter, numeral, suffix),
                        division: div.letter.clone(),
                        title: numeral.clone(),
                        start: abs_start + sub_start,
                        end: abs_start + sub_end,
                        estimated_tokens: (sub_end - sub_start) / 4,
                    });
                }
            }
        }
    }

    debug!(
        "Built {} extraction chunks from {} divisions (max {} tokens/chunk)",
        chunks.len(),
        divisions.len(),
        max_chunk_tokens
    );
    (preamble_end, chunks)
}

/// Find TITLE [ROMAN NUMERAL] headers within a division's text.
/// Returns (start_offset, end_offset, numeral) relative to the input text.
fn find_titles_in(text: &str) -> Vec<(usize, usize, String)> {
    let mut results = Vec::new();
    let mut search_start = 0;

    while let Some(pos) = text[search_start..].find("TITLE ") {
        let abs_pos = search_start + pos;

        // Must be at start of a line
        if abs_pos > 0 && !text[..abs_pos].ends_with('\n') {
            search_start = abs_pos + 6;
            continue;
        }

        // Extract the numeral (roman numerals: I, V, X, L, C, D, M and combinations)
        let after = &text[abs_pos + 6..];
        let num_end = after
            .find(|c: char| !matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'))
            .unwrap_or(after.len());
        let numeral = after[..num_end].to_string();

        if numeral.is_empty() {
            search_start = abs_pos + 6;
            continue;
        }

        // Check that the next char after the numeral is not a letter
        // (avoids matching "TITLE IX SPECIAL" where IX is valid but "TITLED" is not)
        let after_num = after.chars().nth(num_end).unwrap_or(' ');
        if after_num.is_ascii_alphabetic() && !after_num.is_ascii_uppercase() {
            search_start = abs_pos + 6;
            continue;
        }

        results.push((abs_pos, 0, numeral)); // end filled in below
        search_start = abs_pos + 6;
    }

    // Fill in end positions
    for i in 0..results.len() {
        results[i].1 = if i + 1 < results.len() {
            results[i + 1].0
        } else {
            text.len()
        };
    }

    results
}

/// Split text into sub-chunks of approximately `max_tokens` tokens each,
/// breaking at double-newline paragraph boundaries.
fn split_at_paragraphs(text: &str, max_tokens: usize) -> Vec<(usize, usize)> {
    let max_chars = max_tokens * 4;
    let mut chunks = Vec::new();
    let mut chunk_start = 0;

    while chunk_start < text.len() {
        let remaining = text.len() - chunk_start;
        if remaining <= max_chars {
            chunks.push((chunk_start, text.len()));
            break;
        }

        // Find the last paragraph break before the max_chars boundary
        let search_end = (chunk_start + max_chars).min(text.len());
        let search_region = &text[chunk_start..search_end];
        let break_pos = search_region.rfind("\n\n");

        let chunk_end = match break_pos {
            Some(pos) if pos > max_chars / 4 => chunk_start + pos + 2, // after the \n\n
            _ => {
                // No good paragraph break; find any newline
                let nl_pos = search_region.rfind('\n');
                match nl_pos {
                    Some(pos) if pos > max_chars / 4 => chunk_start + pos + 1,
                    _ => search_end, // hard split at max_chars
                }
            }
        };

        chunks.push((chunk_start, chunk_end));
        chunk_start = chunk_end;
    }

    chunks
}

/// A span of text corresponding to one division of an omnibus bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivisionSpan {
    /// Division letter, e.g. "A", "B", "G"
    pub letter: String,
    /// Full heading text, e.g. "DIVISION A—MILITARY CONSTRUCTION, VETERANS AFFAIRS..."
    pub heading: String,
    /// Character offset of the division start in the source text
    pub start: usize,
    /// Character offset of the division end (start of next division, or end of text)
    pub end: usize,
    /// Estimated token count (chars / 4)
    pub estimated_tokens: usize,
}

/// A reference to a dollar amount found in source text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DollarRef {
    /// The dollar string as it appears, e.g. "$51,181,397,000"
    pub text: String,
    /// Parsed value in whole dollars (not cents)
    pub value: i64,
    /// Character offset in source text
    pub position: usize,
    /// ~80 chars of surrounding context
    pub context: String,
}

/// A reference to a section header (SEC. XXXX.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionRef {
    /// e.g. "SEC. 1401"
    pub label: String,
    /// Character offset of the section start
    pub position: usize,
    /// Character offset of the next section (or end of text)
    pub end_position: usize,
}

/// A reference to a clause like "Provided, That"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClauseRef {
    pub clause_type: String,
    pub position: usize,
    pub context: String,
}

/// Complete text index for a bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextIndex {
    pub total_chars: usize,
    pub dollar_amounts: Vec<DollarRef>,
    pub section_headers: Vec<SectionRef>,
    pub proviso_clauses: Vec<ClauseRef>,
}

impl TextIndex {
    /// SHA-256 hash of a text string, for provenance tracking.
    pub fn text_hash(text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Detect division boundaries in an omnibus/minibus bill.
///
/// Returns an empty Vec if the bill has no divisions (supplementals, CRs, etc.).
/// Each DivisionSpan covers from its heading to the start of the next division
/// (or end of text for the last division).
///
/// Skips table-of-contents entries by requiring the heading to appear after
/// position 2500 (past the TOC in all observed bills).
pub fn find_divisions(text: &str) -> Vec<DivisionSpan> {
    let mut results = Vec::new();
    let mut search_start = 0;

    // Find "DIVISION X" at the start of a line, past the table of contents.
    // The letter is followed by a non-lowercase char (em-dash, en-dash, or space).
    while let Some(pos) = text[search_start..].find("DIVISION ") {
        let abs_pos = search_start + pos;

        // Must be at start of a line
        if abs_pos > 0 && !text[..abs_pos].ends_with('\n') {
            search_start = abs_pos + 9;
            continue;
        }

        // Skip TOC entries (they appear in the first ~2500 chars)
        if abs_pos < 2500 {
            search_start = abs_pos + 9;
            continue;
        }

        // Extract the letter (single uppercase char after "DIVISION ")
        let after = &text[abs_pos + 9..];
        let letter = after.chars().next().unwrap_or(' ');
        if !letter.is_ascii_uppercase() {
            search_start = abs_pos + 9;
            continue;
        }

        // The char after the letter should not be lowercase (avoids matching
        // prose like "Division Authority" mid-sentence)
        let after_letter = after.chars().nth(1).unwrap_or(' ');
        if after_letter.is_ascii_lowercase() {
            search_start = abs_pos + 9;
            continue;
        }

        // Extract the heading (to end of line or next 120 chars)
        let heading_end = text[abs_pos..]
            .find('\n')
            .map(|p| abs_pos + p)
            .unwrap_or((abs_pos + 120).min(text.len()));
        let heading = text[abs_pos..heading_end].trim().to_string();

        results.push(DivisionSpan {
            letter: letter.to_string(),
            heading,
            start: abs_pos,
            end: 0,              // filled in below
            estimated_tokens: 0, // filled in below
        });

        search_start = abs_pos + 9;
    }

    // Fill in end positions and token estimates
    for i in 0..results.len() {
        results[i].end = if i + 1 < results.len() {
            results[i + 1].start
        } else {
            text.len()
        };
        results[i].estimated_tokens = (results[i].end - results[i].start) / 4;
    }

    debug!("Found {} divisions", results.len());
    results
}

/// Build a complete text index from bill text.
pub fn build_text_index(text: &str) -> TextIndex {
    let total_chars = text.len();

    // 1. Find all dollar amounts
    let dollar_amounts = find_dollar_amounts(text);
    debug!("Found {} dollar amounts", dollar_amounts.len());

    // 2. Find section headers (SEC. XXXX.)
    let section_headers = find_section_headers(text);
    debug!("Found {} section headers", section_headers.len());

    // 3. Find "Provided, That" clauses
    let proviso_clauses = find_clauses(text, "Provided, That", "proviso");
    debug!("Found {} proviso clauses", proviso_clauses.len());

    TextIndex {
        total_chars,
        dollar_amounts,
        section_headers,
        proviso_clauses,
    }
}

fn get_context(text: &str, pos: usize, radius: usize) -> String {
    let mut start = pos.saturating_sub(radius);
    let mut end = (pos + radius).min(text.len());
    // Snap to valid UTF-8 char boundaries
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[start..end].replace('\n', " ").trim().to_string()
}

fn find_dollar_amounts(text: &str) -> Vec<DollarRef> {
    let mut results = Vec::new();
    let mut search_start = 0;

    while let Some(dollar_pos) = text[search_start..].find('$') {
        let abs_pos = search_start + dollar_pos;
        // Read digits and commas after the $
        let after = &text[abs_pos + 1..];
        let end = after
            .find(|c: char| c != ',' && !c.is_ascii_digit() && c != '.')
            .unwrap_or(after.len());
        let amount_text = text[abs_pos..abs_pos + 1 + end].trim_end_matches([',', '.', ':', ';']);

        // Must have at least one digit
        if amount_text.len() > 1 && amount_text[1..].contains(|c: char| c.is_ascii_digit()) {
            // Parse the value (strip everything except digits)
            let digits: String = amount_text[1..]
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            if let Ok(value) = digits.parse::<i64>() {
                // Keep everything — even small amounts can be significant
                results.push(DollarRef {
                    text: amount_text.to_string(),
                    value,
                    position: abs_pos,
                    context: get_context(text, abs_pos, 80),
                });
            }
        }

        search_start = abs_pos + 1;
    }

    results
}

fn find_section_headers(text: &str) -> Vec<SectionRef> {
    let mut results = Vec::new();
    let mut search_start = 0;

    // Pattern: "SEC." followed by spaces and digits, then a period
    // We search for "\nSEC. " or start-of-text "SEC. " to avoid matching TOC entries
    // that typically look like "Sec. 1234. Title text."
    // Actual sections use uppercase "SEC."
    while let Some(pos) = text[search_start..].find("SEC. ") {
        let abs_pos = search_start + pos;

        // Check that this is at the start of a line (preceded by newline or start of text)
        if abs_pos > 0 && !text[..abs_pos].ends_with('\n') {
            search_start = abs_pos + 5;
            continue;
        }

        // Extract the section number
        let after = &text[abs_pos + 5..];
        let num_end = after
            .find(|c: char| !c.is_ascii_digit() && c != '.' && !c.is_ascii_alphabetic())
            .unwrap_or(after.len());
        let label = format!("SEC. {}", after[..num_end].trim_end_matches('.'));

        results.push(SectionRef {
            label,
            position: abs_pos,
            end_position: 0, // fill in below
        });

        search_start = abs_pos + 5;
    }

    // Fill in end positions
    for i in 0..results.len() {
        results[i].end_position = if i + 1 < results.len() {
            results[i + 1].position
        } else {
            text.len()
        };
    }

    results
}

fn find_clauses(text: &str, pattern: &str, clause_type: &str) -> Vec<ClauseRef> {
    let mut results = Vec::new();
    let mut search_start = 0;

    while let Some(pos) = text[search_start..].find(pattern) {
        let abs_pos = search_start + pos;
        results.push(ClauseRef {
            clause_type: clause_type.to_string(),
            position: abs_pos,
            context: get_context(text, abs_pos, 60),
        });
        search_start = abs_pos + pattern.len();
    }

    results
}

impl TextIndex {
    /// Count dollar amounts within a character range.
    pub fn dollar_amounts_in_range(&self, start: usize, end: usize) -> Vec<&DollarRef> {
        self.dollar_amounts
            .iter()
            .filter(|d| d.position >= start && d.position < end)
            .collect()
    }

    /// Find a dollar amount string in the index, returning its position.
    pub fn find_amount(&self, text_as_written: &str) -> Option<&DollarRef> {
        self.dollar_amounts
            .iter()
            .find(|d| d.text == text_as_written)
    }

    /// Find all occurrences of a dollar amount string.
    pub fn find_all_amounts(&self, text_as_written: &str) -> Vec<&DollarRef> {
        self.dollar_amounts
            .iter()
            .filter(|d| d.text == text_as_written)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_chunks_no_divisions() {
        let text = "SEC. 101. For expenses of $100,000,000.\nSEC. 102. For more.\n";
        let (preamble_end, chunks) = build_chunks(text, 10000);
        assert_eq!(preamble_end, 0);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].label, "full");
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, text.len());
    }

    #[test]
    fn test_build_chunks_splits_divisions() {
        let mut text = String::new();
        // Preamble
        for _ in 0..100 {
            text.push_str("Padding text to get past the 2500 char TOC threshold.\n");
        }
        let preamble_len = text.len();
        text.push_str("\nDIVISION A—STUFF HERE\n");
        text.push_str("For expenses of $100.\n");
        text.push_str("\nDIVISION B—MORE STUFF\n");
        text.push_str("For more expenses of $200.\n");

        let (preamble_end, chunks) = build_chunks(&text, 50000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].division, "A");
        assert_eq!(chunks[1].division, "B");
        assert!(preamble_end > 2500);
    }

    #[test]
    fn test_build_chunks_splits_titles() {
        let mut text = String::new();
        for _ in 0..100 {
            text.push_str("Padding text to get past the 2500 char TOC threshold.\n");
        }
        text.push_str("\nDIVISION A—BIG DIVISION WITH LOTS OF CONTENT\n");
        // Title I - make it big enough
        text.push_str("TITLE I\n");
        for _ in 0..200 {
            text.push_str("For expenses of $1,000,000 for some program or activity.\n");
        }
        text.push_str("TITLE II\n");
        for _ in 0..200 {
            text.push_str("For more expenses of $2,000,000 for another program.\n");
        }

        // With a high threshold, should be one chunk per division
        let (_, chunks_high) = build_chunks(&text, 100000);
        assert_eq!(chunks_high.len(), 1);
        assert_eq!(chunks_high[0].division, "A");

        // With a low threshold, should split into titles
        let (_, chunks_low) = build_chunks(&text, 3000);
        assert!(chunks_low.len() >= 2);
        assert_eq!(chunks_low[0].title, "I");
    }

    #[test]
    fn test_split_at_paragraphs() {
        let text = "paragraph one\n\nparagraph two\n\nparagraph three\n\nparagraph four";
        let chunks = split_at_paragraphs(text, 10); // ~40 chars max per chunk
        assert!(chunks.len() >= 2);
        // All chunks together should cover the whole text
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks.last().unwrap().1, text.len());
        // No gaps
        for i in 1..chunks.len() {
            assert_eq!(chunks[i].0, chunks[i - 1].1);
        }
    }

    #[test]
    fn test_find_divisions_no_divisions() {
        let text = "SEC. 101. For expenses of $100,000,000.\nSEC. 102. For more.\n";
        let divs = find_divisions(text);
        assert!(divs.is_empty());
    }

    #[test]
    fn test_find_divisions_skips_toc() {
        // TOC entries within first 2500 chars should be skipped
        let mut text = String::new();
        text.push_str("Table of Contents\n");
        text.push_str("DIVISION A—STUFF\n"); // position < 2500, should skip
        text.push_str("DIVISION B—MORE STUFF\n");
        // Pad to get past 2500
        for _ in 0..100 {
            text.push_str("Lorem ipsum dolor sit amet padding text here.\n");
        }
        text.push_str("\nDIVISION A—REAL STUFF STARTS HERE\n");
        text.push_str("For expenses of $100.\n");
        text.push_str("\nDIVISION B—MORE REAL STUFF\n");
        text.push_str("For more expenses of $200.\n");
        let divs = find_divisions(&text);
        assert_eq!(divs.len(), 2);
        assert_eq!(divs[0].letter, "A");
        assert_eq!(divs[1].letter, "B");
        assert!(divs[0].start > 2500);
        assert!(divs[0].end == divs[1].start);
        assert!(divs[1].end == text.len());
    }

    #[test]
    fn test_find_divisions_short_heading() {
        // Division G has a short heading — make sure we catch it
        let mut text = String::new();
        for _ in 0..100 {
            text.push_str("Padding text to get past the 2500 char TOC threshold.\n");
        }
        text.push_str("\nDIVISION A—LONG HEADING WITH LOTS OF DETAIL\n");
        text.push_str("Content of division A.\n");
        text.push_str("\nDIVISION G—OTHER MATTERS\n");
        text.push_str("Content of division G.\n");
        let divs = find_divisions(&text);
        assert_eq!(divs.len(), 2);
        assert_eq!(divs[0].letter, "A");
        assert_eq!(divs[1].letter, "G");
        assert!(divs[1].estimated_tokens > 0);
    }

    #[test]
    fn test_find_dollar_amounts() {
        let text = "For expenses of $51,181,397,000 and also $500,000.";
        let amounts = find_dollar_amounts(text);
        assert_eq!(amounts.len(), 2);
        assert_eq!(amounts[0].text, "$51,181,397,000");
        assert_eq!(amounts[0].value, 51181397000);
        assert_eq!(amounts[1].text, "$500,000"); // trailing period stripped by trim_end_matches
        assert_eq!(amounts[1].value, 500000);
    }

    #[test]
    fn test_find_section_headers() {
        let text = "table of contents\nSec. 101. Blah\n\nSEC. 101. For expenses of...\nSEC. 102. And also...\n";
        let sections = find_section_headers(text);
        // Should find SEC. 101 and SEC. 102 (uppercase, at start of line)
        // Should NOT find "Sec. 101" (lowercase, TOC entry)
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].label, "SEC. 101");
        assert_eq!(sections[1].label, "SEC. 102");
        assert!(sections[0].end_position == sections[1].position);
    }

    #[test]
    fn test_build_text_index() {
        let text = concat!(
            "SEC. 101. For expenses of $100,000,000 for general purposes.\n",
            "SEC. 102. For expenses of $200,000,000: Provided, That not more than $50,000 shall be transferred.\n",
        );
        let index = build_text_index(text);
        assert_eq!(index.section_headers.len(), 2);
        assert_eq!(index.section_headers[0].label, "SEC. 101");
        assert_eq!(index.section_headers[1].label, "SEC. 102");
        assert_eq!(index.dollar_amounts.len(), 3);
        assert_eq!(index.proviso_clauses.len(), 1);
        assert_eq!(index.total_chars, text.len());
    }
}
