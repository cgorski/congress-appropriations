//! Parse congressional bill XML and extract structured text + chunk boundaries.
//!
//! The Government Publishing Office publishes enrolled bills as XML with semantic
//! markup: `<division>`, `<title>`, `<appropriations-small>`, `<proviso>`, `<quote>`, etc.
//! This module parses that XML to:
//!
//! 1. Extract clean plain text (with `''quote''` delimiters matching the LLM prompt)
//! 2. Build `ExtractionChunk` boundaries by walking the XML tree structure
//! 3. Provide bill metadata (identifier, short title)
//! 4. Count structural elements for semantic completeness checking

use crate::approp::text_index::ExtractionChunk;
use anyhow::{Context, Result};
use std::path::Path;
use tracing::debug;

/// Parsed bill XML ready for extraction.
#[derive(Debug, Clone)]
pub struct ParsedBillXml {
    /// Bill identifier from `<legis-num>`, e.g. "H. R. 4366"
    pub identifier: String,
    /// Short title from `<short-title>`, if present
    pub short_title: Option<String>,
    /// Clean plain text of the entire bill, with `''quote''` delimiters.
    /// This is the concatenation of preamble + all chunk texts.
    pub full_text: String,
    /// Preamble text (before the first division, or the enacting clause)
    pub preamble: String,
    /// Structural extraction chunks derived from the XML tree.
    /// Each chunk has `start`/`end` offsets into `full_text`.
    pub chunks: Vec<ExtractionChunk>,
    /// Number of `<appropriations-small>` elements per division (for completeness checking)
    pub appropriations_count_by_division: Vec<(String, usize)>,
    /// Total `<appropriations-small>` elements in the bill
    pub total_appropriations_elements: usize,
}

/// Parse a congressional bill XML file and extract text + structure.
///
/// The XML is expected to follow the GPO bill DTD with tags like
/// `<division>`, `<title>`, `<section>`, `<appropriations-small>`, `<quote>`, etc.
pub fn parse_bill_xml(path: &Path, max_chunk_tokens: usize) -> Result<ParsedBillXml> {
    let raw_xml = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read XML file: {}", path.display()))?;

    parse_bill_xml_str(&raw_xml, max_chunk_tokens)
}

/// Parse bill XML from a string.
pub fn parse_bill_xml_str(raw_xml: &str, max_chunk_tokens: usize) -> Result<ParsedBillXml> {
    let opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };

    let doc = roxmltree::Document::parse_with_options(raw_xml, opt)
        .context("Failed to parse bill XML")?;

    let root = doc.root_element();

    // Extract metadata
    let identifier = find_text_content(&root, "legis-num").unwrap_or_default();
    let short_title = find_text_content(&root, "short-title");

    // Find the <legis-body> element — contains all bill content
    let legis_body = root
        .descendants()
        .find(|n| n.has_tag_name("legis-body"))
        .context("No <legis-body> element found in XML")?;

    // Find divisions
    let divisions: Vec<roxmltree::Node> = legis_body
        .children()
        .filter(|n| n.has_tag_name("division"))
        .collect();

    // Count appropriations-small per division
    let mut approp_by_div = Vec::new();
    let mut total_approp = 0usize;

    // Build chunks by extracting text DIRECTLY from XML nodes.
    // We build the full_text by concatenating preamble + each chunk's text,
    // so chunk start/end offsets are guaranteed correct.
    // Build a header from bill metadata so the LLM sees the bill number
    let mut bill_header = String::new();
    if !identifier.is_empty() {
        bill_header.push_str(&identifier);
        bill_header.push('\n');
    }
    if let Some(ref st) = short_title {
        bill_header.push_str(st);
        bill_header.push('\n');
    }
    if !bill_header.is_empty() {
        bill_header.push('\n');
    }

    if divisions.is_empty() {
        // No divisions — single chunk for the whole bill
        let body_text = extract_text_with_quotes(&legis_body);
        let full_text = format!("{bill_header}{body_text}");
        total_approp = count_descendants(&legis_body, "appropriations-small");
        let chunk = ExtractionChunk {
            label: "full".to_string(),
            division: String::new(),
            title: String::new(),
            start: 0,
            end: full_text.len(),
            estimated_tokens: full_text.len() / 4,
        };

        debug!(
            "Parsed XML: 0 divisions, 1 chunk, {} appropriations-small elements, {} chars",
            total_approp,
            full_text.len()
        );

        Ok(ParsedBillXml {
            identifier,
            short_title,
            full_text,
            preamble: String::new(),
            chunks: vec![chunk],
            appropriations_count_by_division: approp_by_div,
            total_appropriations_elements: total_approp,
        })
    } else {
        // Has divisions — extract text per node, build full_text by concatenation
        let (preamble, full_text, chunks) = build_chunks_from_xml_nodes(
            &bill_header,
            &legis_body,
            &divisions,
            max_chunk_tokens,
            &mut approp_by_div,
            &mut total_approp,
        );

        debug!(
            "Parsed XML: {} divisions, {} chunks, {} appropriations-small elements, {} chars",
            divisions.len(),
            chunks.len(),
            total_approp,
            full_text.len()
        );

        Ok(ParsedBillXml {
            identifier,
            short_title,
            full_text,
            preamble,
            chunks,
            appropriations_count_by_division: approp_by_div,
            total_appropriations_elements: total_approp,
        })
    }
}

// ─── Text Extraction ─────────────────────────────────────────────────────────

/// Extract text content from an XML node, replacing `<quote>` tags with `''` delimiters.
/// This produces text matching the format expected by the LLM extraction prompt.
fn extract_text_with_quotes(node: &roxmltree::Node) -> String {
    let mut result = String::new();
    collect_text_recursive(node, &mut result);
    // Clean up: normalize whitespace runs but preserve paragraph breaks
    clean_extracted_text(&result)
}

/// Recursively collect text from an XML node tree, inserting `''` around `<quote>` content.
fn collect_text_recursive(node: &roxmltree::Node, out: &mut String) {
    for child in node.children() {
        if child.is_text() {
            if let Some(text) = child.text() {
                out.push_str(text);
            }
        } else if child.is_element() {
            if child.has_tag_name("quote") {
                out.push_str("''");
                collect_text_recursive(&child, out);
                out.push_str("''");
            } else if child.has_tag_name("header") {
                // Headers get their own line
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                collect_text_recursive(&child, out);
                out.push('\n');
            } else if child.has_tag_name("enum") {
                // Section enums (SEC. 101.) get a prefix
                let parent_tag = node.tag_name().name();
                if parent_tag == "section" {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("SEC. ");
                }
                collect_text_recursive(&child, out);
                out.push(' ');
            } else if child.has_tag_name("text")
                || child.has_tag_name("subsection")
                || child.has_tag_name("paragraph")
                || child.has_tag_name("subparagraph")
            {
                collect_text_recursive(&child, out);
            } else if child.has_tag_name("proviso") {
                // Provisos contain <italic>Provided,</italic> — extract inline
                collect_text_recursive(&child, out);
            } else if child.has_tag_name("italic") || child.has_tag_name("bold") {
                collect_text_recursive(&child, out);
            } else if child.has_tag_name("appropriations-major")
                || child.has_tag_name("appropriations-intermediate")
                || child.has_tag_name("appropriations-small")
            {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                collect_text_recursive(&child, out);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            } else if child.has_tag_name("division")
                || child.has_tag_name("title")
                || child.has_tag_name("section")
            {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                collect_text_recursive(&child, out);
            } else {
                // For any other element, just recurse
                collect_text_recursive(&child, out);
            }
        }
    }
}

/// Clean up extracted text: normalize whitespace runs, remove excessive blank lines.
fn clean_extracted_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_was_newline = false;
    let mut blank_line_count = 0;

    for ch in text.chars() {
        if ch == '\n' {
            if blank_line_count < 2 {
                result.push('\n');
            }
            blank_line_count += 1;
            last_was_newline = true;
        } else if ch.is_whitespace() {
            if !last_was_newline && !result.ends_with(' ') && !result.is_empty() {
                result.push(' ');
            }
        } else {
            blank_line_count = 0;
            last_was_newline = false;
            result.push(ch);
        }
    }

    result.trim().to_string()
}

// ─── Chunk Building ──────────────────────────────────────────────────────────

/// Build chunks by extracting text directly from XML nodes.
/// Returns (preamble_text, full_text, chunks) where full_text = preamble + all chunk texts
/// and chunk start/end offsets are guaranteed correct because we built full_text ourselves.
fn build_chunks_from_xml_nodes(
    bill_header: &str,
    legis_body: &roxmltree::Node,
    divisions: &[roxmltree::Node],
    max_chunk_tokens: usize,
    approp_by_div: &mut Vec<(String, usize)>,
    total_approp: &mut usize,
) -> (String, String, Vec<ExtractionChunk>) {
    let mut full_text = String::new();
    let mut chunks = Vec::new();

    // Start with bill header (identifier + short title)
    full_text.push_str(bill_header);

    // Extract preamble (content before first division)
    let mut preamble = String::new();
    for child in legis_body.children() {
        if child.is_element() && child.has_tag_name("division") {
            break;
        }
        let text = extract_text_with_quotes(&child);
        if !text.is_empty() {
            preamble.push_str(&text);
            preamble.push('\n');
        }
    }
    let preamble = clean_extracted_text(&preamble);
    full_text.push_str(&preamble);
    if !preamble.is_empty() {
        full_text.push('\n');
    }

    // Process each division
    for div_node in divisions {
        let div_letter = find_child_text(div_node, "enum").unwrap_or_default();
        let div_approp = count_descendants(div_node, "appropriations-small");
        approp_by_div.push((div_letter.clone(), div_approp));
        *total_approp += div_approp;

        // Find titles within this division
        let titles: Vec<roxmltree::Node> = div_node
            .children()
            .filter(|n| n.has_tag_name("title"))
            .collect();

        let div_text = extract_text_with_quotes(div_node);
        let div_tokens = div_text.len() / 4;

        if titles.is_empty() || div_tokens <= max_chunk_tokens {
            // Small division or no titles — one chunk
            let start = full_text.len();
            full_text.push_str(&div_text);
            full_text.push('\n');
            let end = full_text.len();
            chunks.push(ExtractionChunk {
                label: div_letter.clone(),
                division: div_letter.clone(),
                title: String::new(),
                start,
                end,
                estimated_tokens: (end - start) / 4,
            });
        } else {
            // Split by titles — extract text from each title node
            build_title_chunks_from_nodes(
                &mut full_text,
                &div_letter,
                div_node,
                &titles,
                max_chunk_tokens,
                &mut chunks,
            );
        }
    }

    (preamble.clone(), full_text, chunks)
}

/// Build chunks from title-level XML nodes within a division.
fn build_title_chunks_from_nodes(
    full_text: &mut String,
    div_letter: &str,
    div_node: &roxmltree::Node,
    titles: &[roxmltree::Node],
    max_chunk_tokens: usize,
    chunks: &mut Vec<ExtractionChunk>,
) {
    // First, extract any content in the division that comes before the first title
    for child in div_node.children() {
        if child.is_element() && child.has_tag_name("title") {
            break;
        }
        // Division header, enum, etc.
        if child.is_element() && (child.has_tag_name("enum") || child.has_tag_name("header")) {
            let text = extract_text_with_quotes(&child);
            if !text.is_empty() {
                full_text.push_str(&text);
                full_text.push('\n');
            }
        }
    }

    for title_node in titles {
        let numeral = find_child_text(title_node, "enum").unwrap_or_default();
        let title_text = extract_text_with_quotes(title_node);
        let title_tokens = title_text.len() / 4;

        if title_tokens <= max_chunk_tokens {
            let start = full_text.len();
            full_text.push_str(&title_text);
            full_text.push('\n');
            let end = full_text.len();
            chunks.push(ExtractionChunk {
                label: format!("{div_letter}-{numeral}"),
                division: div_letter.to_string(),
                title: numeral.clone(),
                start,
                end,
                estimated_tokens: (end - start) / 4,
            });
        } else {
            // Title too large — split the extracted text at section boundaries
            split_text_into_chunks(
                full_text,
                &title_text,
                div_letter,
                &numeral,
                max_chunk_tokens,
                chunks,
            );
        }
    }
}

/// Split a large text block into chunks at section boundaries, appending to full_text.
fn split_text_into_chunks(
    full_text: &mut String,
    text: &str,
    div_letter: &str,
    numeral: &str,
    max_chunk_tokens: usize,
    chunks: &mut Vec<ExtractionChunk>,
) {
    let max_chars = max_chunk_tokens * 4;

    // Find section boundaries (SEC. NNN.)
    let mut section_starts: Vec<usize> = vec![0];
    let mut search_from = 0;
    while let Some(pos) = text[search_from..].find("\nSEC. ") {
        section_starts.push(search_from + pos + 1); // +1 to skip the \n
        search_from = search_from + pos + 6;
    }

    // Group sections into chunks that don't exceed max_chars
    let mut chunk_start = 0usize;
    let mut suffix = b'a';

    for &sec_start in section_starts.iter().skip(1) {
        if sec_start - chunk_start > max_chars && sec_start > chunk_start {
            let chunk_text = &text[chunk_start..sec_start];
            let start = full_text.len();
            full_text.push_str(chunk_text);
            let end = full_text.len();
            chunks.push(ExtractionChunk {
                label: format!("{div_letter}-{numeral}{}", suffix as char),
                division: div_letter.to_string(),
                title: numeral.to_string(),
                start,
                end,
                estimated_tokens: (end - start) / 4,
            });
            suffix += 1;
            chunk_start = sec_start;
        }
    }

    // Emit the remaining content
    let remaining_text = &text[chunk_start..];
    if !remaining_text.is_empty() {
        let label = if suffix > b'a' {
            format!("{div_letter}-{numeral}{}", suffix as char)
        } else {
            format!("{div_letter}-{numeral}")
        };
        let start = full_text.len();
        full_text.push_str(remaining_text);
        full_text.push('\n');
        let end = full_text.len();
        chunks.push(ExtractionChunk {
            label,
            division: div_letter.to_string(),
            title: numeral.to_string(),
            start,
            end,
            estimated_tokens: (end - start) / 4,
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Find the text content of the first descendant element with the given tag name.
fn find_text_content(node: &roxmltree::Node, tag: &str) -> Option<String> {
    node.descendants()
        .find(|n| n.has_tag_name(tag))
        .map(|n| {
            n.descendants()
                .filter(|c| c.is_text())
                .filter_map(|c| c.text())
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
}

/// Find the direct text content of the first child element with the given tag name.
fn find_child_text(node: &roxmltree::Node, tag: &str) -> Option<String> {
    node.children()
        .find(|n| n.has_tag_name(tag))
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Count the number of descendant elements with the given tag name.
fn count_descendants(node: &roxmltree::Node, tag: &str) -> usize {
    node.descendants().filter(|n| n.has_tag_name(tag)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_with_dtd() {
        let xml = r#"<?xml version="1.0"?>
<!DOCTYPE bill PUBLIC "-//US Congress//DTDs/bill.dtd//EN" "bill.dtd">
<bill><text>hello</text></bill>"#;
        let opt = roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        };
        let doc = roxmltree::Document::parse_with_options(xml, opt).unwrap();
        let root = doc.root_element();
        assert!(root.has_tag_name("bill"));
    }

    #[test]
    fn test_extract_text_with_quotes() {
        let xml = r#"<?xml version="1.0"?>
<root><text>For <quote>Compensation and Pensions</quote>, $2,285,513,000.</text></root>"#;
        let opt = roxmltree::ParsingOptions::default();
        let doc = roxmltree::Document::parse_with_options(xml, opt).unwrap();
        let root = doc.root_element();
        let text = extract_text_with_quotes(&root);
        assert!(text.contains("''Compensation and Pensions''"));
        assert!(text.contains("$2,285,513,000"));
    }

    #[test]
    fn test_clean_extracted_text() {
        let messy = "  hello   world  \n\n\n\n  foo  \n  bar  ";
        let clean = clean_extracted_text(messy);
        assert!(!clean.contains("   "));
        assert!(!clean.contains("\n\n\n"));
    }

    #[test]
    fn test_find_text_content() {
        let xml = r#"<?xml version="1.0"?><root><legis-num>H. R. 1234</legis-num></root>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let root = doc.root_element();
        let num = find_text_content(&root, "legis-num");
        assert_eq!(num, Some("H. R. 1234".to_string()));
    }

    #[test]
    fn test_parse_minimal_xml() {
        let xml = r#"<?xml version="1.0"?>
<bill>
<metadata><dublinCore><dc:title xmlns:dc="http://purl.org/dc/elements/1.1/">Test</dc:title></dublinCore></metadata>
<form><legis-num>H. R. 999</legis-num></form>
<legis-body style="appropriations">
<section><text>For expenses, $100,000.</text></section>
</legis-body>
</bill>"#;
        let result = parse_bill_xml_str(xml, 3000).unwrap();
        assert_eq!(result.identifier, "H. R. 999");
        assert!(result.full_text.contains("$100,000"));
        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].label, "full");
    }

    #[test]
    fn test_parse_xml_with_divisions() {
        let xml = r#"<?xml version="1.0"?>
<bill>
<form><legis-num>H. R. 1</legis-num></form>
<legis-body style="OLC">
<section><text>Preamble text here.</text></section>
<division><enum>A</enum><header>Agriculture and Stuff</header>
<title><enum>I</enum><header>Programs</header>
<section><enum>101.</enum><text>For <quote>Farm Programs</quote>, $500,000.</text></section>
</title>
</division>
<division><enum>B</enum><header>Defense and Things</header>
<title><enum>I</enum><header>Military</header>
<section><enum>201.</enum><text>For <quote>Army Ops</quote>, $1,000,000.</text></section>
</title>
</division>
</legis-body>
</bill>"#;
        let result = parse_bill_xml_str(xml, 50000).unwrap();
        assert_eq!(result.identifier, "H. R. 1");
        assert!(result.full_text.contains("''Farm Programs''"));
        assert!(result.full_text.contains("''Army Ops''"));
        assert_eq!(result.chunks.len(), 2);
        assert_eq!(result.chunks[0].division, "A");
        assert_eq!(result.chunks[1].division, "B");
    }
}
