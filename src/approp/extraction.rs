use crate::api::anthropic::{AnthropicClient, MessageBuilder, Usage};
use crate::approp::from_value::{ConversionReport, parse_bill_extraction};
use crate::approp::ontology::*;
use crate::approp::prompts;
use crate::approp::text_index::ExtractionChunk;
use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use ulid::Ulid;

const MODEL: &str = "claude-opus-4-6";
const MAX_TOKENS: u32 = 128000;
/// Default maximum tokens per extraction chunk.
/// Titles/divisions larger than this are split at paragraph boundaries.
pub const DEFAULT_MAX_CHUNK_TOKENS: usize = 3_000;

/// Tracks cumulative token usage across all LLM calls.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct TokenTracker {
    pub total_input: u32,
    pub total_output: u32,
    pub total_cache_read: u32,
    pub total_cache_create: u32,
    pub calls: u32,
}

impl TokenTracker {
    pub fn record(&mut self, usage: &Usage) {
        self.total_input += usage.input_tokens;
        self.total_output += usage.output_tokens;
        self.total_cache_read += usage.cache_read_input_tokens;
        self.total_cache_create += usage.cache_creation_input_tokens;
        self.calls += 1;
    }

    pub fn merge(&mut self, other: &TokenTracker) {
        self.total_input += other.total_input;
        self.total_output += other.total_output;
        self.total_cache_read += other.total_cache_read;
        self.total_cache_create += other.total_cache_create;
        self.calls += other.calls;
    }

    pub fn total(&self) -> u32 {
        self.total_input + self.total_output
    }
}

/// Progress event sent from parallel extraction tasks to the logging task.
#[derive(Debug, Clone)]
pub enum ChunkProgress {
    Started {
        label: String,
        short: String,
    },
    Thinking {
        short: String,
        est_tokens: usize,
    },
    Generating {
        short: String,
        est_tokens: usize,
    },
    Completed {
        label: String,
        short: String,
        provisions: usize,
        elapsed_secs: f64,
        input_tokens: u32,
        output_tokens: u32,
    },
    Failed {
        label: String,
        short: String,
        error: String,
        attempt: u32,
    },
    Retrying {
        label: String,
        short: String,
        attempt: u32,
    },
}

/// Per-chunk artifact saved to .chunks/{ulid}.json for debugging and resume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkArtifact {
    pub chunk_id: String,
    pub label: String,
    pub division: String,
    pub title: String,
    pub chunk_start: usize,
    pub chunk_end: usize,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub elapsed_secs: f64,
    pub thinking: Option<String>,
    pub raw_response: String,
    pub raw_json: serde_json::Value,
    pub conversion_report: ConversionReport,
    pub provisions_extracted: usize,
    pub timestamp: String,
}

/// Entry in the chunk_map field of BillExtraction, linking chunks to provisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMapEntry {
    pub chunk_id: String,
    pub label: String,
    pub provision_indices: Vec<usize>,
}

/// Result of extracting a single chunk (internal, not serialized).
struct ChunkResult {
    label: String,
    chunk_id: String,
    division: String,
    title: String,
    chunk_start: usize,
    chunk_end: usize,
    extraction: BillExtraction,
    report: ConversionReport,
    tokens: TokenTracker,
    thinking: Option<String>,
    raw_response: String,
    raw_json: serde_json::Value,
    elapsed_secs: f64,
}

/// Owned data for a single chunk, used as input to extraction functions.
struct ChunkInput {
    label: String,
    short: String,
    division: String,
    title: String,
    chunk_start: usize,
    chunk_end: usize,
    preamble: String,
    chunk_text: String,
    bill_id: String,
}

/// State of an in-flight chunk for the dashboard display.
#[derive(Clone)]
struct ActiveChunk {
    short: String,
    phase: String, // "🤔" or "📝"
    est_tokens: usize,
    /// Rolling window of (timestamp, cumulative_tokens) for rate calculation.
    token_history: VecDeque<(Instant, usize)>,
}

/// Compute output tokens/sec from a rolling history window.
/// Returns None if insufficient data (< 1 second of history)
/// or if the most recent observation is stale (> 10 seconds old).
fn compute_rate(history: &VecDeque<(Instant, usize)>, now: Instant) -> Option<usize> {
    if history.len() < 2 {
        return None;
    }
    let oldest = history.front()?;
    let newest = history.back()?;
    // If no data in the last 10 seconds, rate is effectively zero
    if now.duration_since(newest.0).as_secs() > 10 {
        return None;
    }
    let dt = newest.0.duration_since(oldest.0).as_secs_f64();
    if dt < 1.0 {
        return None;
    }
    let dtok = newest.1.saturating_sub(oldest.1);
    Some((dtok as f64 / dt) as usize)
}

pub struct ExtractionPipeline {
    client: Arc<AnthropicClient>,
    pub tokens: TokenTracker,
}

impl ExtractionPipeline {
    pub fn new(client: AnthropicClient) -> Self {
        Self {
            client: Arc::new(client),
            tokens: TokenTracker::default(),
        }
    }

    /// Extract a bill using chunk-level extraction.
    ///
    /// Splits the bill into chunks (by division/title), extracts each chunk
    /// in parallel with bounded concurrency, then merges all results.
    /// Progress events are sent through a channel for live logging.
    pub async fn extract_bill_parallel(
        &mut self,
        bill_id: &str,
        bill_text: &str,
        preamble: &str,
        chunks: &[ExtractionChunk],
        max_parallel: usize,
        bill_dir: &Path,
    ) -> Result<(BillExtraction, ConversionReport)> {
        let total_chunks = chunks.len();

        info!(
            "    {} chunks, concurrency={}, preamble={} chars",
            total_chunks,
            max_parallel,
            preamble.len()
        );

        // Channel for progress events from parallel tasks
        let (tx, mut rx) = mpsc::unbounded_channel::<ChunkProgress>();
        let is_interactive = crate::approp::progress::is_interactive();

        // Spawn a task that logs progress events as a single-line dashboard
        let progress_handle = tokio::spawn(async move {
            let mut completed = 0usize;
            let mut total_provs = 0usize;
            let start = std::time::Instant::now();
            let mut active: HashMap<String, ActiveChunk> = HashMap::new();

            while let Some(event) = rx.recv().await {
                match event {
                    ChunkProgress::Started { short, .. } => {
                        active.insert(
                            short.clone(),
                            ActiveChunk {
                                short: short.clone(),
                                phase: "⏳".to_string(),
                                est_tokens: 0,
                                token_history: VecDeque::new(),
                            },
                        );
                    }
                    ChunkProgress::Thinking {
                        ref short,
                        est_tokens,
                    } => {
                        if let Some(a) = active.get_mut(short) {
                            a.phase = "🤔".to_string();
                            a.est_tokens = est_tokens;
                        }
                    }
                    ChunkProgress::Generating {
                        ref short,
                        est_tokens,
                    } => {
                        if let Some(a) = active.get_mut(short) {
                            a.phase = "📝".to_string();
                            a.est_tokens = est_tokens;
                            let now = Instant::now();
                            a.token_history.push_back((now, est_tokens));
                            // Drain entries older than 10 seconds
                            while a
                                .token_history
                                .front()
                                .is_some_and(|(t, _)| now.duration_since(*t).as_secs() > 10)
                            {
                                a.token_history.pop_front();
                            }
                        }
                    }
                    ChunkProgress::Completed {
                        ref label,
                        ref short,
                        provisions,
                        elapsed_secs,
                        input_tokens,
                        output_tokens,
                    } => {
                        completed += 1;
                        total_provs += provisions;
                        active.remove(short);
                        // Clear the interactive line before logging
                        if is_interactive {
                            eprint!("\r{:>120}\r", "");
                        }
                        info!(
                            "    ✓ {label}: {provisions} provs [{elapsed_secs:.0}s, in={input_tokens} out={output_tokens}] | {completed}/{total_chunks}, {total_provs} provs [{:.0?}]",
                            start.elapsed()
                        );
                    }
                    ChunkProgress::Failed {
                        ref label,
                        ref short,
                        ref error,
                        attempt,
                    } => {
                        active.remove(short);
                        if is_interactive {
                            eprint!("\r{:>120}\r", "");
                        }
                        warn!("    ✗ {label} failed (attempt {attempt}): {error}");
                    }
                    ChunkProgress::Retrying {
                        ref label,
                        ref short,
                        attempt,
                    } => {
                        active.remove(short);
                        if is_interactive {
                            eprint!("\r{:>120}\r", "");
                        }
                        warn!("    ↻ {label} retrying (attempt {attempt}/3)");
                    }
                }

                // Redraw the single-line dashboard (interactive mode only)
                if is_interactive && !active.is_empty() {
                    let elapsed = start.elapsed();
                    let now = Instant::now();
                    let mut parts: Vec<String> = Vec::new();
                    let mut aggregate_rate: usize = 0;
                    let mut sorted_active: Vec<&ActiveChunk> = active.values().collect();
                    sorted_active.sort_by(|a, b| a.short.cmp(&b.short));
                    for a in sorted_active.iter().take(8) {
                        let rate = compute_rate(&a.token_history, now);
                        if let Some(r) = rate {
                            aggregate_rate += r;
                        }
                        match (&a.phase as &str, a.est_tokens, rate) {
                            ("📝", tok, Some(r)) if tok > 0 => {
                                parts.push(format!("📝{} ~{}K {}/s", a.short, tok / 1000, r));
                            }
                            ("📝", tok, None) if tok > 0 => {
                                parts.push(format!("📝{} ~{}K", a.short, tok / 1000));
                            }
                            _ => {
                                parts.push(format!("{}{}", a.phase, a.short));
                            }
                        }
                    }
                    let active_str = parts.join(" | ");
                    let rate_str = if aggregate_rate > 0 {
                        format!("{aggregate_rate} tok/s")
                    } else {
                        String::new()
                    };
                    eprint!(
                        "\r    {completed}/{total_chunks}, {total_provs} provs [{:.0?}] {rate_str} | {active_str}",
                        elapsed
                    );
                    let _ = std::io::Write::flush(&mut std::io::stderr());
                }
            }

            // Clear the dashboard line at the end
            if is_interactive {
                eprint!("\r{:>120}\r", "");
                let _ = std::io::Write::flush(&mut std::io::stderr());
            }
        });

        // Build owned data for each chunk so futures are 'static
        let inputs: Vec<ChunkInput> = chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| ChunkInput {
                label: format!("[{}/{}] {}", i + 1, total_chunks, chunk.label),
                short: chunk.label.clone(),
                division: chunk.division.clone(),
                title: chunk.title.clone(),
                chunk_start: chunk.start,
                chunk_end: chunk.end,
                preamble: preamble.to_string(),
                chunk_text: bill_text[chunk.start..chunk.end].to_string(),
                bill_id: bill_id.to_string(),
            })
            .collect();

        let client = Arc::clone(&self.client);

        // Run extractions with bounded concurrency
        let results: Vec<Result<ChunkResult>> = stream::iter(inputs)
            .map(|input| {
                let client = Arc::clone(&client);
                let tx = tx.clone();
                async move { extract_chunk_with_retry(&client, &input, tx).await }
            })
            .buffer_unordered(max_parallel)
            .collect()
            .await;

        // Drop the sender so the progress task finishes
        drop(tx);
        let _ = progress_handle.await;

        // Collect results, sort by division+title for deterministic order
        let mut all_provisions: Vec<Provision> = Vec::new();
        let mut merged_report = ConversionReport::default();
        let mut merged_tokens = TokenTracker::default();
        let mut all_flagged_issues: Vec<String> = Vec::new();
        let mut all_sections_empty: Vec<String> = Vec::new();
        let mut first_bill_info: Option<BillInfo> = None;
        let mut chunk_labels_ok: Vec<String> = Vec::new();
        let mut chunk_labels_failed: Vec<String> = Vec::new();

        // Sort results by the original chunk order (label starts with [N/M])
        let mut ok_results: Vec<ChunkResult> = Vec::new();
        for result in results {
            match result {
                Ok(cr) => ok_results.push(cr),
                Err(e) => {
                    warn!("    Chunk failed permanently: {e}");
                    chunk_labels_failed.push(format!("{e}"));
                }
            }
        }
        // Sort by label to restore original order
        ok_results.sort_by(|a, b| a.label.cmp(&b.label));

        let mut chunk_map: Vec<ChunkMapEntry> = Vec::new();
        let mut provision_offset = 0usize;

        for cr in ok_results {
            chunk_labels_ok.push(cr.label.clone());
            merged_report.merge(&cr.report);
            merged_tokens.merge(&cr.tokens);
            all_flagged_issues.extend(cr.extraction.summary.flagged_issues.clone());
            all_sections_empty.extend(cr.extraction.summary.sections_with_no_provisions.clone());
            if first_bill_info.is_none() {
                first_bill_info = Some(cr.extraction.bill.clone());
            }

            let prov_count = cr.extraction.provisions.len();
            let indices: Vec<usize> = (provision_offset..provision_offset + prov_count).collect();

            chunk_map.push(ChunkMapEntry {
                chunk_id: cr.chunk_id.clone(),
                label: cr.label.clone(),
                provision_indices: indices,
            });

            // Save chunk artifact to .chunks/ directory
            if let Err(e) = save_chunk_artifact(bill_dir, &cr) {
                warn!("Failed to save chunk artifact for {}: {e}", cr.label);
            }

            all_provisions.extend(cr.extraction.provisions);
            provision_offset += prov_count;
        }

        if first_bill_info.is_none() {
            anyhow::bail!("All chunks failed to extract for {bill_id}");
        }

        self.tokens.merge(&merged_tokens);

        // Build merged bill info
        let mut bill_info = first_bill_info.unwrap();
        // Collect unique division letters
        let mut div_letters: Vec<String> = chunks.iter().map(|c| c.division.clone()).collect();
        div_letters.sort();
        div_letters.dedup();
        div_letters.retain(|d| !d.is_empty());
        bill_info.divisions = div_letters;

        // Recompute summary from merged provisions
        let total_provisions = all_provisions.len();
        let mut by_type: HashMap<String, usize> = HashMap::new();
        let mut by_division: HashMap<String, usize> = HashMap::new();
        for p in &all_provisions {
            *by_type.entry(p.type_str().to_string()).or_insert(0) += 1;
            let div = p.division().unwrap_or("?");
            *by_division.entry(div.to_string()).or_insert(0) += 1;
        }

        let mut final_extraction = BillExtraction {
            schema_version: None,
            bill: bill_info,
            provisions: all_provisions,
            summary: ExtractionSummary {
                total_provisions,
                by_division,
                by_type,
                total_budget_authority: 0,
                total_rescissions: 0,
                sections_with_no_provisions: all_sections_empty,
                flagged_issues: all_flagged_issues,
            },
            chunk_map,
        };
        final_extraction.schema_version = Some("1.0".to_string());

        let (ba, rescissions) = final_extraction.compute_totals();
        final_extraction.summary.total_budget_authority = ba;
        final_extraction.summary.total_rescissions = rescissions;

        info!(
            "    Merged: {} provisions from {}/{} chunks",
            total_provisions,
            chunk_labels_ok.len(),
            total_chunks
        );
        if !chunk_labels_failed.is_empty() {
            warn!("    {} chunks failed", chunk_labels_failed.len());
        }

        info!(
            "    Conversion: {} parsed, {} failed, {} coercions, {} warnings",
            merged_report.provisions_parsed,
            merged_report.provisions_failed,
            merged_report.null_to_default + merged_report.type_coercions,
            merged_report.warnings.len()
        );

        Ok((final_extraction, merged_report))
    }

    /// Build extraction metadata for provenance tracking.
    pub fn build_metadata(&self, text: &str) -> ExtractionMetadata {
        use crate::approp::text_index::TextIndex;
        ExtractionMetadata {
            extraction_version: env!("CARGO_PKG_VERSION").to_string(),
            prompt_version: "v3".to_string(),
            model: MODEL.to_string(),
            schema_version: "0.3.0".to_string(),
            source_pdf_sha256: None,
            extracted_text_sha256: TextIndex::text_hash(text),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

// ─── Free Functions for Parallel Extraction ──────────────────────────────────

/// Extract provisions from a single chunk, with up to 3 retries.
/// This is a free async function (no &mut self) so it can be run in parallel.
async fn extract_chunk_with_retry(
    client: &AnthropicClient,
    input: &ChunkInput,
    tx: mpsc::UnboundedSender<ChunkProgress>,
) -> Result<ChunkResult> {
    let _ = tx.send(ChunkProgress::Started {
        label: input.label.clone(),
        short: input.short.clone(),
    });
    let timer_start = std::time::Instant::now();
    let chunk_id = Ulid::new().to_string();

    let mut last_err = None;

    for attempt in 0..3u32 {
        if attempt > 0 {
            let _ = tx.send(ChunkProgress::Retrying {
                label: input.label.clone(),
                short: input.short.clone(),
                attempt: attempt + 1,
            });
            tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
        }

        match extract_single_chunk(client, input, &tx).await {
            Ok(output) => {
                let mut tokens = TokenTracker::default();
                tokens.record(&output.usage);
                let elapsed = timer_start.elapsed().as_secs_f64();

                let _ = tx.send(ChunkProgress::Completed {
                    label: input.label.clone(),
                    short: input.short.clone(),
                    provisions: output.extraction.provisions.len(),
                    elapsed_secs: elapsed,
                    input_tokens: output.usage.input_tokens,
                    output_tokens: output.usage.output_tokens,
                });

                return Ok(ChunkResult {
                    label: input.label.clone(),
                    chunk_id,
                    division: input.division.clone(),
                    title: input.title.clone(),
                    chunk_start: input.chunk_start,
                    chunk_end: input.chunk_end,
                    extraction: output.extraction,
                    report: output.report,
                    tokens,
                    thinking: output.thinking,
                    raw_response: output.raw_response,
                    raw_json: output.raw_json,
                    elapsed_secs: elapsed,
                });
            }
            Err(e) => {
                let _ = tx.send(ChunkProgress::Failed {
                    label: input.label.clone(),
                    short: input.short.clone(),
                    error: format!("{e:#}"),
                    attempt: attempt + 1,
                });
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap())
}

/// Output from a single chunk extraction call.
struct ChunkOutput {
    extraction: BillExtraction,
    report: ConversionReport,
    usage: Usage,
    thinking: Option<String>,
    raw_response: String,
    raw_json: serde_json::Value,
}

/// Extract provisions from a single chunk (one LLM call, no retry).
async fn extract_single_chunk(
    client: &AnthropicClient,
    input: &ChunkInput,
    tx: &mpsc::UnboundedSender<ChunkProgress>,
) -> Result<ChunkOutput> {
    let label = &input.label;
    let short = &input.short;
    let division = &input.division;
    let title = &input.title;
    let bill_id = &input.bill_id;
    let preamble = &input.preamble;
    let chunk_text = &input.chunk_text;

    let div_instruction = if !division.is_empty() {
        if !title.is_empty() {
            format!(
                "Extract all provisions from Division {division}, Title {title} of {bill_id}. \
                 Set division=\"{division}\" and title=\"{title}\" on every provision."
            )
        } else {
            format!(
                "Extract all provisions from Division {division} of {bill_id}. \
                 Set division=\"{division}\" on every provision."
            )
        }
    } else {
        format!("Extract all provisions from {bill_id}.")
    };

    let user_message = format!(
        "BILL PREAMBLE:\n{preamble}\n\nBILL TEXT:\n{chunk_text}\n\n{div_instruction} \
         Return a single JSON object matching the schema described in the system prompt."
    );

    debug!(
        label,
        text_len = chunk_text.len(),
        "Sending chunk for extraction"
    );

    let req = MessageBuilder::new(MODEL)
        .system_cached(prompts::EXTRACTION_SYSTEM)
        .user(user_message)
        .max_tokens(MAX_TOKENS)
        .thinking_adaptive()
        .build();

    // In parallel mode, send streaming progress through the channel
    // instead of writing directly to stderr
    let short_owned = short.to_string();
    let tx_clone = tx.clone();
    let mut thinking_chars = 0usize;
    let mut text_chars = 0usize;

    let message = client
        .send_message_streaming(
            &req,
            move |event: &crate::api::anthropic::StreamEvent| match event {
                crate::api::anthropic::StreamEvent::ThinkingDelta(t) => {
                    thinking_chars += t.len();
                    let est = thinking_chars / 4;
                    let _ = tx_clone.send(ChunkProgress::Thinking {
                        short: short_owned.clone(),
                        est_tokens: est,
                    });
                }
                crate::api::anthropic::StreamEvent::TextDelta(t) => {
                    text_chars += t.len();
                    let est = text_chars / 4;
                    let _ = tx_clone.send(ChunkProgress::Generating {
                        short: short_owned.clone(),
                        est_tokens: est,
                    });
                }
                _ => {}
            },
        )
        .await
        .with_context(|| format!("LLM extraction failed for {label}"))?;

    let thinking = message.thinking().map(|s| s.to_string());

    // Check for output truncation
    if matches!(
        message.stop_reason,
        Some(crate::api::anthropic::StopReason::MaxTokens)
    ) {
        warn!(
            "{label}: LLM output was truncated (hit max_tokens). \
             Response JSON is likely incomplete."
        );
    }

    let raw_response = message.all_text();
    let json_str = extract_json(&raw_response)?;

    let raw_json: serde_json::Value = serde_json::from_str(&json_str)
        .with_context(|| format!("JSON parse failed for {label}"))?;

    let (extraction, report) = parse_bill_extraction(&raw_json)
        .with_context(|| format!("Value→BillExtraction failed for {label}"))?;

    Ok(ChunkOutput {
        extraction,
        report,
        usage: message.usage,
        thinking,
        raw_response,
        raw_json,
    })
}

/// Save a chunk artifact to the .chunks/ directory.
fn save_chunk_artifact(bill_dir: &Path, result: &ChunkResult) -> Result<PathBuf> {
    let chunks_dir = bill_dir.join(".chunks");
    std::fs::create_dir_all(&chunks_dir)?;

    let artifact = ChunkArtifact {
        chunk_id: result.chunk_id.clone(),
        label: result.label.clone(),
        division: result.division.clone(),
        title: result.title.clone(),
        chunk_start: result.chunk_start,
        chunk_end: result.chunk_end,
        input_tokens: result.tokens.total_input,
        output_tokens: result.tokens.total_output,
        elapsed_secs: result.elapsed_secs,
        thinking: result.thinking.clone(),
        raw_response: result.raw_response.clone(),
        raw_json: result.raw_json.clone(),
        conversion_report: result.report.clone(),
        provisions_extracted: result.extraction.provisions.len(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let path = chunks_dir.join(format!("{}.json", result.chunk_id));
    std::fs::write(&path, serde_json::to_string_pretty(&artifact)?)?;
    debug!("Saved chunk artifact: {}", path.display());
    Ok(path)
}

/// Extract JSON from a response that may contain markdown code blocks.
fn extract_json(text: &str) -> anyhow::Result<String> {
    let trimmed = text.trim();

    // Try ```json ... ``` blocks
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return Ok(trimmed[json_start..json_start + end].trim().to_string());
        }
        return Ok(trimmed[json_start..].trim().to_string());
    }

    // Try ``` ... ``` blocks
    if let Some(start) = trimmed.find("```") {
        let after_fence = start + 3;
        let json_start = if let Some(nl) = trimmed[after_fence..].find('\n') {
            after_fence + nl + 1
        } else {
            after_fence
        };
        if let Some(end) = trimmed[json_start..].find("```") {
            return Ok(trimmed[json_start..json_start + end].trim().to_string());
        }
        return Ok(trimmed[json_start..].trim().to_string());
    }

    // Raw JSON (starts with { or [)
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Ok(trimmed.to_string());
    }

    anyhow::bail!(
        "Could not extract JSON from response (first 300 chars): {}",
        &trimmed[..trimmed.len().min(300)]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_tracker_default() {
        let t = TokenTracker::default();
        assert_eq!(t.total(), 0);
        assert_eq!(t.calls, 0);
    }

    #[test]
    fn token_tracker_record() {
        let mut t = TokenTracker::default();
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 10,
            cache_creation_input_tokens: 20,
        };
        t.record(&usage);
        assert_eq!(t.total(), 150);
        assert_eq!(t.calls, 1);
    }

    #[test]
    fn extract_json_raw() {
        let input = r#"{"bill": {}}"#;
        assert_eq!(extract_json(input).unwrap(), input);
    }

    #[test]
    fn extract_json_from_code_block() {
        let input = "```json\n{\"a\": 1}\n```";
        assert_eq!(extract_json(input).unwrap(), "{\"a\": 1}");
    }

    #[test]
    fn extract_json_fails_on_plain_text() {
        assert!(extract_json("hello world").is_err());
    }
}
