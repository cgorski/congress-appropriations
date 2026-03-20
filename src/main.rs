use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use comfy_table::{Cell, CellAlignment, Color, Table, presets::UTF8_FULL_CONDENSED};
use congress_appropriations::api::congress::bill::BillListItem;
use congress_appropriations::api::congress::{BillId, BillType, Congress, CongressClient};
use congress_appropriations::approp::loading::{self, LoadedBill};
use congress_appropriations::approp::ontology::{AmountSemantics, Provision};
use congress_appropriations::approp::text_index;
use congress_appropriations::approp::verification::{CheckResult, MatchTier};
use congress_appropriations::approp::xml;
use std::collections::HashMap;
use std::time::Instant;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "congress-approp",
    version,
    about = "Download and analyze U.S. appropriations bills",
    after_help = "Quick start: congress-approp summary --dir data\nExplore included bill data without any API keys."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// API interaction commands
    Api {
        #[command(subcommand)]
        action: ApiCommands,
    },
    /// Download appropriations bill XML from Congress.gov
    Download {
        /// Congress number (e.g., 118 for 2023-2024, 119 for 2025-2026)
        #[arg(long)]
        congress: u32,
        /// Bill type: hr (House), s (Senate), hjres (House joint resolution)
        #[arg(long)]
        r#type: Option<String>,
        /// Bill number for single-bill download (used with --type)
        #[arg(long)]
        number: Option<u32>,
        /// Output directory
        #[arg(long, default_value = "./data")]
        output_dir: String,
        /// Only download bills signed into law (filters out introduced/committee versions)
        #[arg(long)]
        enacted_only: bool,
        /// Download format: xml (for extraction), pdf (for reading) [comma-separated]
        #[arg(long, default_value = "xml")]
        format: String,
        /// Bill text version: enr (enrolled/final), ih (introduced), eh (engrossed)
        #[arg(long)]
        version: Option<String>,
        /// Download all text versions (introduced, engrossed, enrolled, etc.) instead of just enrolled
        #[arg(long)]
        all_versions: bool,
        /// Show what would be downloaded without fetching
        #[arg(long)]
        dry_run: bool,
    },
    /// Extract spending provisions from bill text using Claude (requires ANTHROPIC_API_KEY)
    Extract {
        /// Data directory containing downloaded bill XML
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Show what would be extracted without calling LLM
        #[arg(long)]
        dry_run: bool,
        /// Parallel LLM calls — higher is faster but uses more API quota
        #[arg(long, default_value = "5")]
        parallel: usize,
        /// LLM model for extraction (tested with claude-opus-4-6; other models may vary in quality)
        #[arg(long, env = "APPROP_MODEL")]
        model: Option<String>,
        /// Re-extract bills even if extraction.json already exists
        #[arg(long)]
        force: bool,
        /// Save partial results when some chunks fail (default: abort bill on any chunk failure)
        #[arg(long)]
        continue_on_error: bool,
    },
    /// Search provisions across all extracted bills
    Search {
        /// Data directory (try 'examples' for included FY2024 data)
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Filter by agency name (case-insensitive substring)
        #[arg(long, short)]
        agency: Option<String>,
        /// Filter by provision type (e.g. appropriation, rescission, rider)
        #[arg(long, short = 't')]
        r#type: Option<String>,
        /// Filter by account name (case-insensitive substring)
        #[arg(long)]
        account: Option<String>,
        /// Search keyword in raw_text (case-insensitive)
        #[arg(long, short)]
        keyword: Option<String>,
        /// Filter to a specific bill (e.g. "H.R. 9468")
        #[arg(long)]
        bill: Option<String>,
        /// Filter by division letter (e.g., A, B, C)
        #[arg(long)]
        division: Option<String>,
        /// Minimum dollar amount (absolute value)
        #[arg(long)]
        min_dollars: Option<i64>,
        /// Maximum dollar amount (absolute value)
        #[arg(long)]
        max_dollars: Option<i64>,
        /// Output format: table, json, jsonl, csv
        #[arg(long, default_value = "table")]
        format: String,
        /// List all valid provision types and exit
        #[arg(long)]
        list_types: bool,
        /// Semantic search query (ranks results by meaning similarity, requires embeddings)
        #[arg(long)]
        semantic: Option<String>,
        /// Find provisions similar to this one (format: bill_dir:index, e.g. hr4366:42)
        #[arg(long)]
        similar: Option<String>,
        /// Maximum results for semantic/similar search
        #[arg(long, default_value = "20")]
        top: usize,
        /// Filter to bills covering this fiscal year
        #[arg(long)]
        fy: Option<u32>,
        /// Filter by subcommittee jurisdiction (e.g., defense, thud, cjs). Requires `enrich`.
        #[arg(long)]
        subcommittee: Option<String>,
    },
    /// Show summary of all extracted bills
    Summary {
        /// Data directory (try 'examples' for included FY2024 data)
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Output format: table, json, jsonl, csv
        #[arg(long, default_value = "table")]
        format: String,
        /// Show budget authority totals by parent department
        #[arg(long)]
        by_agency: bool,
        /// Filter to bills covering this fiscal year
        #[arg(long)]
        fy: Option<u32>,
        /// Filter by subcommittee jurisdiction (e.g., defense, thud, cjs). Requires `enrich`.
        #[arg(long)]
        subcommittee: Option<String>,
        /// Separate advance appropriations from current-year in the output. Requires `enrich`.
        #[arg(long)]
        show_advance: bool,
    },
    /// Compare provisions between two sets of bills (e.g. two fiscal years)
    Compare {
        /// Base directory for comparison (e.g., data from prior fiscal year)
        #[arg(long)]
        base: Option<String>,
        /// Current directory for comparison (e.g., data from current fiscal year)
        #[arg(long)]
        current: Option<String>,
        /// Use all bills for this FY as the base set (alternative to --base)
        #[arg(long)]
        base_fy: Option<u32>,
        /// Use all bills for this FY as the current set (alternative to --current)
        #[arg(long)]
        current_fy: Option<u32>,
        /// Data directory (required with --base-fy/--current-fy)
        #[arg(long)]
        dir: Option<String>,
        /// Filter by agency name (case-insensitive substring)
        #[arg(long, short)]
        agency: Option<String>,
        /// Scope comparison to one subcommittee jurisdiction. Requires `enrich`.
        #[arg(long)]
        subcommittee: Option<String>,
        /// Use accepted links for matching across renames
        #[arg(long)]
        use_links: bool,
        /// Show inflation-adjusted "Real Δ %" column using CPI-U
        #[arg(long)]
        real: bool,
        /// Path to custom CPI/deflator JSON file (overrides bundled CPI-U data)
        #[arg(long)]
        cpi_file: Option<String>,
        /// Disable all normalization from dataset.json — use exact matching only
        #[arg(long)]
        exact: bool,
        /// Output format: table, json, csv
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Audit data quality across all extracted bills
    #[command(alias = "report")]
    Audit {
        /// Data directory to audit (try 'examples' for included FY2024 data)
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Show individual problematic provisions
        #[arg(long)]
        verbose: bool,
    },
    /// Upgrade extraction data to the latest schema version (re-verifies, no LLM needed)
    Upgrade {
        /// Data directory to upgrade
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Show what would change without writing files
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate embeddings for extracted bills (requires OPENAI_API_KEY)
    Embed {
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Embedding model
        #[arg(long, default_value = "text-embedding-3-large")]
        model: String,
        /// Request this many dimensions from the API
        #[arg(long, default_value = "3072")]
        dimensions: usize,
        /// Provisions per API batch
        #[arg(long, default_value = "100")]
        batch_size: usize,
        /// Preview without calling API
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate bill metadata for FY/subcommittee filtering (no API key needed)
    Enrich {
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Preview without writing files
        #[arg(long)]
        dry_run: bool,
        /// Re-enrich even if bill_meta.json exists
        #[arg(long)]
        force: bool,
    },
    /// Manage cross-bill provision links
    Link {
        #[command(subcommand)]
        action: LinkCommands,
    },
    /// Manage entity resolution rules (agency groups, account aliases)
    Normalize {
        #[command(subcommand)]
        action: NormalizeCommands,
    },
    /// Deep-dive on one provision across all bills (requires embeddings)
    Relate {
        /// Provision reference: bill_directory:index (e.g., hr9468:0)
        source: String,
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Max related provisions per tier
        #[arg(long, default_value = "10")]
        top: usize,
        /// Output format: table, json, hashes
        #[arg(long, default_value = "table")]
        format: String,
        /// Show fiscal year timeline with advance/current/supplemental split
        #[arg(long)]
        fy_timeline: bool,
    },
}

#[derive(Subcommand)]
enum LinkCommands {
    /// Compute link candidates from embeddings
    Suggest {
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Minimum similarity threshold
        #[arg(long, default_value = "0.55")]
        threshold: f32,
        /// Scope: intra (within-FY), cross (across-FY), all
        #[arg(long, default_value = "all")]
        scope: String,
        /// Max candidates
        #[arg(long, default_value = "100")]
        limit: usize,
        /// Output format: table, json, hashes
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Accept link candidates by hash
    Accept {
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Link hashes to accept
        hashes: Vec<String>,
        /// Optional annotation
        #[arg(long)]
        note: Option<String>,
        /// Accept all verified + high-confidence candidates
        #[arg(long)]
        auto: bool,
    },
    /// Remove accepted links by hash
    Remove {
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Link hashes to remove
        hashes: Vec<String>,
    },
    /// Show accepted links
    List {
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Output format: table, json
        #[arg(long, default_value = "table")]
        format: String,
        /// Filter to links involving this bill
        #[arg(long)]
        bill: Option<String>,
    },
}

/// Subcommands for `normalize`.
#[derive(Subcommand)]
enum NormalizeCommands {
    /// Discover agency/account naming variants using orphan-pair analysis and regex patterns
    #[command(name = "suggest-text-match")]
    SuggestTextMatch {
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Preview suggestions without writing dataset.json
        #[arg(long)]
        dry_run: bool,
        /// Output format: table, json
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Discover agency/account naming variants using LLM classification with XML context
    #[command(name = "suggest-llm")]
    SuggestLlm {
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
        /// Preview suggestions without writing dataset.json
        #[arg(long)]
        dry_run: bool,
        /// Maximum clusters per API call
        #[arg(long, default_value = "15")]
        batch_size: usize,
        /// Output format: table, json
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Show current entity resolution rules from dataset.json
    List {
        /// Data directory
        #[arg(long, default_value = "./data")]
        dir: String,
    },
}

#[derive(Subcommand)]
enum ApiCommands {
    /// Test API connectivity
    Test,
    /// Bill-related queries
    Bill {
        #[command(subcommand)]
        action: BillCommands,
    },
}

#[derive(Subcommand)]
enum BillCommands {
    /// List appropriations bills for a given Congress session
    List {
        #[arg(long)]
        congress: u32,
        #[arg(long, default_value = "hr")]
        r#type: String,
        #[arg(long, default_value = "0")]
        offset: u32,
        #[arg(long, default_value = "20")]
        limit: u32,
    },
    /// Get bill detail
    Get {
        #[arg(long)]
        congress: u32,
        #[arg(long, default_value = "hr")]
        r#type: String,
        #[arg(short, long)]
        number: u32,
    },
    /// Get bill text versions with PDF URLs
    Text {
        #[arg(long)]
        congress: u32,
        #[arg(long, default_value = "hr")]
        r#type: String,
        #[arg(short, long)]
        number: u32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Detect interactive terminal before tracing captures stderr
    congress_appropriations::approp::progress::init();

    // Set up tracing
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Api { action } => handle_api(action).await,
        Commands::Download {
            congress,
            r#type,
            number,
            output_dir,
            enacted_only,
            format,
            version,
            all_versions,
            dry_run,
        } => {
            handle_download(DownloadOptions {
                congress,
                bill_type: r#type.as_deref(),
                bill_number: number,
                output_dir: &output_dir,
                enacted_only,
                format: &format,
                version_filter: version.as_deref(),
                all_versions,
                dry_run,
            })
            .await
        }
        Commands::Extract {
            dir,
            dry_run,
            parallel,
            model,
            force,
            continue_on_error,
        } => handle_extract(&dir, dry_run, parallel, model, force, continue_on_error).await,
        Commands::Search {
            dir,
            agency,
            r#type,
            account,
            keyword,
            bill,
            division,
            min_dollars,
            max_dollars,
            format,
            list_types,
            semantic,
            similar,
            top,
            fy,
            subcommittee,
        } => {
            handle_search(
                &dir,
                agency.as_deref(),
                r#type.as_deref(),
                account.as_deref(),
                keyword.as_deref(),
                bill.as_deref(),
                division.as_deref(),
                min_dollars,
                max_dollars,
                &format,
                list_types,
                semantic.as_deref(),
                similar.as_deref(),
                top,
                fy,
                subcommittee.as_deref(),
            )
            .await
        }
        Commands::Summary {
            dir,
            format,
            by_agency,
            fy,
            subcommittee,
            show_advance,
        } => handle_summary(
            &dir,
            &format,
            by_agency,
            fy,
            subcommittee.as_deref(),
            show_advance,
        ),
        Commands::Compare {
            base,
            current,
            base_fy,
            current_fy,
            dir,
            agency,
            subcommittee,
            use_links,
            real,
            cpi_file,
            exact,
            format,
        } => handle_compare(
            base.as_deref(),
            current.as_deref(),
            base_fy,
            current_fy,
            dir.as_deref(),
            agency.as_deref(),
            subcommittee.as_deref(),
            use_links,
            real,
            cpi_file.as_deref(),
            &format,
            exact,
        ),
        Commands::Normalize { action } => handle_normalize(action).await,
        Commands::Audit { dir, verbose } => handle_audit(&dir, verbose),
        Commands::Upgrade { dir, dry_run } => handle_upgrade(&dir, dry_run),
        Commands::Embed {
            dir,
            model,
            dimensions,
            batch_size,
            dry_run,
        } => handle_embed(&dir, &model, dimensions, batch_size, dry_run).await,
        Commands::Link { action } => handle_link(action),
        Commands::Enrich {
            dir,
            dry_run,
            force,
        } => handle_enrich(&dir, dry_run, force),
        Commands::Relate {
            source,
            dir,
            top,
            format,
            fy_timeline,
        } => handle_relate(&source, &dir, top, &format, fy_timeline),
    }
}

// ─── Normalize Handler ───────────────────────────────────────────────────────

async fn handle_normalize(action: NormalizeCommands) -> Result<()> {
    use congress_appropriations::approp::normalize;

    match action {
        NormalizeCommands::SuggestTextMatch {
            dir,
            dry_run,
            format,
        } => {
            let dir_path = std::path::Path::new(&dir);
            let bills = loading::load_bills(dir_path)?;
            if bills.is_empty() {
                anyhow::bail!("No extracted bills found in directory: {dir}");
            }

            let suggestions = normalize::suggest_text_match(&bills);

            if suggestions.is_empty() {
                println!("No agency naming variants detected across bills.");
                println!("All cross-FY account matches use exact agency names.");
                return Ok(());
            }

            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&suggestions)?);
                }
                _ => {
                    println!(
                        "Found {} suggested agency groups ({} orphan pairs resolvable):\n",
                        suggestions.len(),
                        suggestions
                            .iter()
                            .map(|s| s.orphan_pairs_resolved)
                            .sum::<usize>()
                    );

                    for (i, s) in suggestions.iter().enumerate() {
                        let evidence_tag = match &s.evidence {
                            normalize::SuggestionEvidence::OrphanPair => "orphan-pair",
                            normalize::SuggestionEvidence::RegexPattern { pattern } => {
                                pattern.as_str()
                            }
                        };
                        println!("  {}. [{}] \"{}\"", i + 1, evidence_tag, s.canonical);
                        for m in &s.members {
                            println!("     = \"{}\"", m);
                        }
                        if !s.example_accounts.is_empty() {
                            println!(
                                "     Evidence: {} shared accounts (e.g., {})",
                                s.orphan_pairs_resolved,
                                s.example_accounts
                                    .iter()
                                    .take(3)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                        println!();
                    }
                }
            }

            if dry_run {
                eprintln!("Dry run — no changes written.");
                return Ok(());
            }

            // Load existing dataset.json or create new
            let mut dataset =
                normalize::load_dataset(dir_path)?.unwrap_or_else(normalize::DatasetFile::new);

            // Merge suggestions
            normalize::merge_groups(&mut dataset, &suggestions);

            // Write
            normalize::save_dataset(dir_path, &dataset)?;
            eprintln!(
                "Wrote {} agency groups to {}/dataset.json",
                dataset.entities.agency_groups.len(),
                dir
            );
        }

        NormalizeCommands::SuggestLlm {
            dir,
            dry_run,
            batch_size,
            format,
        } => {
            use congress_appropriations::api::anthropic::{AnthropicClient, MessageBuilder};
            use std::collections::HashSet;

            let dir_path = std::path::Path::new(&dir);
            let bills = loading::load_bills(dir_path)?;
            if bills.is_empty() {
                anyhow::bail!("No extracted bills found in directory: {dir}");
            }

            // Step 1: Find unresolved pairs using text-match as the starting point
            let text_suggestions = normalize::suggest_text_match(&bills);
            if text_suggestions.is_empty() {
                println!("No unresolved agency naming variants found.");
                return Ok(());
            }

            // Step 2: Filter to pairs not already in dataset.json
            let existing = normalize::load_dataset(dir_path)?.unwrap_or_default();
            let existing_agencies: HashSet<String> = existing
                .entities
                .agency_groups
                .iter()
                .flat_map(|g| {
                    std::iter::once(g.canonical.to_lowercase())
                        .chain(g.members.iter().map(|m| m.to_lowercase()))
                })
                .collect();

            let unresolved: Vec<&normalize::SuggestedGroup> = text_suggestions
                .iter()
                .filter(|s| {
                    !existing_agencies.contains(&s.canonical.to_lowercase())
                        && !s
                            .members
                            .iter()
                            .any(|m| existing_agencies.contains(&m.to_lowercase()))
                })
                .collect();

            if unresolved.is_empty() {
                println!("All agency variants are already resolved in dataset.json.");
                return Ok(());
            }

            // Step 3: Check API key early — fail fast before expensive cluster building
            let client = AnthropicClient::from_env().map_err(|_| {
                anyhow::anyhow!("ANTHROPIC_API_KEY not set. Required for suggest-llm.")
            })?;

            eprintln!(
                "Found {} unresolved agency pairs. Building clusters with XML context...",
                unresolved.len()
            );

            // Step 4: Build clusters with XML context
            let owned_unresolved: Vec<normalize::SuggestedGroup> =
                unresolved.iter().map(|s| (*s).clone()).collect();
            let clusters = normalize::build_llm_clusters(&bills, &owned_unresolved);

            if clusters.is_empty() {
                println!("No clusters could be built (provisions may lack XML context).");
                return Ok(());
            }

            eprintln!(
                "Built {} clusters. Sending to Claude in batches of {batch_size}...",
                clusters.len()
            );

            // Step 5: Send to Claude in batches

            let mut all_accepted_groups: Vec<normalize::SuggestedGroup> = Vec::new();

            for (batch_idx, batch) in clusters.chunks(batch_size).enumerate() {
                let user_prompt = normalize::format_llm_prompt(batch);

                eprintln!(
                    "  Batch {}/{}: {} clusters, ~{} tokens...",
                    batch_idx + 1,
                    clusters.len().div_ceil(batch_size),
                    batch.len(),
                    user_prompt.len() / 4,
                );

                let request = MessageBuilder::new("claude-sonnet-4-20250514")
                    .system(normalize::LLM_SYSTEM_PROMPT)
                    .user(&user_prompt)
                    .max_tokens(4000)
                    .build();

                let response = client
                    .send_message(&request)
                    .await
                    .map_err(|e| anyhow::anyhow!("LLM API call failed: {e}"))?;

                // Extract text from response
                let response_text: String = response
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        congress_appropriations::api::anthropic::types::ContentBlock::Text {
                            text,
                            ..
                        } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");

                eprintln!(
                    "  Received: {} tokens in, {} tokens out",
                    response.usage.input_tokens, response.usage.output_tokens,
                );

                // Parse JSON from response (handle markdown code block wrapping)
                let json_text = if let Some(start) = response_text.find("```") {
                    let after_ticks = &response_text[start + 3..];
                    // Skip optional language tag
                    let json_start = after_ticks.find('{').unwrap_or(0);
                    if let Some(end) = after_ticks.find("```") {
                        &after_ticks[json_start..end]
                    } else {
                        &after_ticks[json_start..]
                    }
                } else if let Some(start) = response_text.find('{') {
                    &response_text[start..]
                } else {
                    eprintln!("  Warning: Could not find JSON in response. Skipping batch.");
                    continue;
                };

                match serde_json::from_str::<serde_json::Value>(json_text) {
                    Ok(parsed) => {
                        // Extract SAME groups from response
                        if let Some(groups) = parsed.get("groups").and_then(|g| g.as_array()) {
                            for group in groups {
                                let verdict =
                                    group.get("verdict").and_then(|v| v.as_str()).unwrap_or("");
                                if verdict != "SAME" {
                                    continue;
                                }
                                let canonical = group
                                    .get("canonical")
                                    .and_then(|c| c.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let members: Vec<String> = group
                                    .get("members")
                                    .and_then(|m| m.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                            .filter(|s| {
                                                s.to_lowercase() != canonical.to_lowercase()
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let reasoning = group
                                    .get("reasoning")
                                    .and_then(|r| r.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                if !canonical.is_empty() && !members.is_empty() {
                                    println!("  [SAME] \"{}\"", canonical);
                                    for m in &members {
                                        println!("     = \"{}\"", m);
                                    }
                                    println!("     Reasoning: {}", reasoning);
                                    println!();

                                    all_accepted_groups.push(normalize::SuggestedGroup {
                                        canonical,
                                        members,
                                        evidence: normalize::SuggestionEvidence::RegexPattern {
                                            pattern: format!("llm: {}", reasoning),
                                        },
                                        example_accounts: Vec::new(),
                                        orphan_pairs_resolved: 0,
                                    });
                                }
                            }
                        }

                        // Report DIFFERENT verdicts
                        if let Some(separates) = parsed.get("separate").and_then(|s| s.as_array()) {
                            for sep in separates {
                                let agency =
                                    sep.get("agency").and_then(|a| a.as_str()).unwrap_or("?");
                                let reasoning =
                                    sep.get("reasoning").and_then(|r| r.as_str()).unwrap_or("");
                                println!("  [DIFF] \"{}\"", agency);
                                println!("     Reasoning: {}", reasoning);
                                println!();
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  Warning: Failed to parse LLM response as JSON: {e}");
                        eprintln!(
                            "  Raw response (first 500 chars): {}",
                            &json_text[..json_text.len().min(500)]
                        );
                    }
                }
            }

            // Step 5: Output summary
            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&all_accepted_groups)?);
                }
                _ => {
                    println!(
                        "\nSummary: {} SAME groups identified by LLM.",
                        all_accepted_groups.len()
                    );
                }
            }

            if dry_run {
                eprintln!("Dry run — no changes written.");
                return Ok(());
            }

            if all_accepted_groups.is_empty() {
                eprintln!("No SAME groups to write.");
                return Ok(());
            }

            // Write to dataset.json
            let mut dataset =
                normalize::load_dataset(dir_path)?.unwrap_or_else(normalize::DatasetFile::new);
            normalize::merge_groups(&mut dataset, &all_accepted_groups);
            normalize::save_dataset(dir_path, &dataset)?;
            eprintln!(
                "Wrote {} agency groups to {}/dataset.json",
                dataset.entities.agency_groups.len(),
                dir
            );
        }

        NormalizeCommands::List { dir } => {
            let dir_path = std::path::Path::new(&dir);
            let dataset = normalize::load_dataset(dir_path)?;

            match dataset {
                None => {
                    println!("No dataset.json found in {dir}");
                    println!();
                    println!("To create one, run:");
                    println!("  congress-approp normalize suggest-text-match --dir {dir}");
                }
                Some(ds) => {
                    if ds.entities.agency_groups.is_empty()
                        && ds.entities.account_aliases.is_empty()
                    {
                        println!("dataset.json exists but contains no entity resolution rules.");
                        return Ok(());
                    }

                    if !ds.entities.agency_groups.is_empty() {
                        println!("Agency groups ({}):\n", ds.entities.agency_groups.len());
                        for (i, g) in ds.entities.agency_groups.iter().enumerate() {
                            println!("  {}. \"{}\"", i + 1, g.canonical);
                            for m in &g.members {
                                println!("     = \"{}\"", m);
                            }
                            println!();
                        }
                    }

                    if !ds.entities.account_aliases.is_empty() {
                        println!("Account aliases ({}):\n", ds.entities.account_aliases.len());
                        for (i, a) in ds.entities.account_aliases.iter().enumerate() {
                            println!("  {}. \"{}\"", i + 1, a.canonical);
                            for alias in &a.aliases {
                                println!("     = \"{}\"", alias);
                            }
                            println!();
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// ─── Link Handler ────────────────────────────────────────────────────────────

fn handle_link(action: LinkCommands) -> Result<()> {
    use congress_appropriations::approp::embeddings;
    use congress_appropriations::approp::links;

    match action {
        LinkCommands::Suggest {
            dir,
            threshold,
            scope,
            limit,
            format,
        } => {
            let link_scope = links::LinkScope::parse(&scope).ok_or_else(|| {
                anyhow::anyhow!("Invalid scope: '{scope}'. Valid values: intra, cross, all")
            })?;

            let bills = loading::load_bills(std::path::Path::new(&dir))?;
            if bills.is_empty() {
                anyhow::bail!("No extracted bills found in directory: {dir}");
            }

            let mut bill_embeddings: Vec<Option<embeddings::LoadedEmbeddings>> = Vec::new();
            for bill in &bills {
                bill_embeddings.push(embeddings::load(&bill.dir)?);
            }

            let existing = links::load_links(std::path::Path::new(&dir))?;
            let candidates = links::suggest(
                &bills,
                &bill_embeddings,
                threshold,
                link_scope,
                &existing,
                limit,
            );

            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&candidates)?);
                }
                "hashes" => {
                    for c in &candidates {
                        if !c.already_accepted {
                            println!("{}", c.hash);
                        }
                    }
                }
                _ => {
                    if candidates.is_empty() {
                        println!("No link candidates found above threshold {threshold}.");
                        return Ok(());
                    }

                    let mut table = Table::new();
                    table.load_preset(UTF8_FULL_CONDENSED);
                    table.set_header(vec![
                        Cell::new("Hash"),
                        Cell::new("Sim"),
                        Cell::new("Conf"),
                        Cell::new("Source"),
                        Cell::new("Target"),
                        Cell::new("Accepted"),
                    ]);

                    let mut verified = 0usize;
                    let mut high = 0usize;
                    let mut uncertain = 0usize;
                    let mut already = 0usize;

                    for c in &candidates {
                        match c.confidence {
                            links::LinkConfidence::Verified => verified += 1,
                            links::LinkConfidence::High => high += 1,
                            links::LinkConfidence::Uncertain => uncertain += 1,
                        }
                        if c.already_accepted {
                            already += 1;
                        }

                        let accepted_str = if c.already_accepted { "✓" } else { "" };
                        table.add_row(vec![
                            Cell::new(&c.hash),
                            Cell::new(format!("{:.2}", c.similarity)),
                            Cell::new(format!("{}", c.confidence)),
                            Cell::new(truncate(&c.source_label, 35)),
                            Cell::new(truncate(&c.target_label, 35)),
                            Cell::new(accepted_str),
                        ]);
                    }

                    println!("{table}");
                    println!();
                    println!(
                        "{} candidates ({verified} verified, {high} high, {uncertain} uncertain, {already} already accepted)",
                        candidates.len()
                    );
                }
            }
        }

        LinkCommands::Accept {
            dir,
            hashes,
            note,
            auto,
        } => {
            let dir_path = std::path::Path::new(&dir);
            let bills = loading::load_bills(dir_path)?;
            if bills.is_empty() {
                anyhow::bail!("No extracted bills found in directory: {dir}");
            }

            let mut bill_embeddings: Vec<Option<embeddings::LoadedEmbeddings>> = Vec::new();
            for bill in &bills {
                bill_embeddings.push(embeddings::load(&bill.dir)?);
            }

            // Load or create links file
            let mut links_file = links::load_links(dir_path)?.unwrap_or_else(|| {
                let model = bill_embeddings
                    .iter()
                    .flatten()
                    .next()
                    .map(|e| e.metadata.model.as_str())
                    .unwrap_or("unknown");
                links::LinksFile::new(model)
            });

            // Compute candidates to match against hashes
            let existing_for_suggest = Some(links_file.clone());
            let candidates = links::suggest(
                &bills,
                &bill_embeddings,
                0.50,
                links::LinkScope::All,
                &existing_for_suggest,
                10000,
            );

            let hash_refs: Vec<&str> = hashes.iter().map(|s| s.as_str()).collect();
            let accepted = links::accept_links(
                &mut links_file,
                &candidates,
                &hash_refs,
                note.as_deref(),
                auto,
            );

            links::save_links(dir_path, &links_file)?;

            if auto {
                eprintln!(
                    "Auto-accepted {accepted} links ({} total)",
                    links_file.accepted.len()
                );
            } else {
                eprintln!(
                    "Accepted {accepted} links ({} total)",
                    links_file.accepted.len()
                );
            }
        }

        LinkCommands::Remove { dir, hashes } => {
            let dir_path = std::path::Path::new(&dir);
            let mut links_file = links::load_links(dir_path)?
                .ok_or_else(|| anyhow::anyhow!("No links file found in {dir}/links/"))?;

            let hash_refs: Vec<&str> = hashes.iter().map(|s| s.as_str()).collect();
            let removed = links::remove_links(&mut links_file, &hash_refs);

            links::save_links(dir_path, &links_file)?;
            eprintln!(
                "Removed {removed} links ({} remaining)",
                links_file.accepted.len()
            );
        }

        LinkCommands::List { dir, format, bill } => {
            let dir_path = std::path::Path::new(&dir);
            let links_file = links::load_links(dir_path)?;

            let Some(links_file) = links_file else {
                println!(
                    "No links file found. Run `link suggest` then `link accept` to create links."
                );
                return Ok(());
            };

            let filtered: Vec<&links::AcceptedLink> = links_file
                .accepted
                .iter()
                .filter(|l| {
                    if let Some(ref b) = bill {
                        let b_lower = b.to_lowercase();
                        l.source.bill_dir.to_lowercase().contains(&b_lower)
                            || l.target.bill_dir.to_lowercase().contains(&b_lower)
                    } else {
                        true
                    }
                })
                .collect();

            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&filtered)?);
                }
                _ => {
                    if filtered.is_empty() {
                        println!("No accepted links found.");
                        return Ok(());
                    }

                    let mut table = Table::new();
                    table.load_preset(UTF8_FULL_CONDENSED);
                    table.set_header(vec![
                        Cell::new("Hash"),
                        Cell::new("Sim"),
                        Cell::new("Relationship"),
                        Cell::new("Source"),
                        Cell::new("Target"),
                        Cell::new("Note"),
                    ]);

                    for l in &filtered {
                        let src = format!(
                            "{}:{} ({})",
                            l.source.bill_dir,
                            l.source.provision_index,
                            truncate(&l.source.label, 25)
                        );
                        let tgt = format!(
                            "{}:{} ({})",
                            l.target.bill_dir,
                            l.target.provision_index,
                            truncate(&l.target.label, 25)
                        );
                        table.add_row(vec![
                            Cell::new(&l.hash),
                            Cell::new(format!("{:.2}", l.similarity)),
                            Cell::new(format!("{}", l.relationship)),
                            Cell::new(&src),
                            Cell::new(&tgt),
                            Cell::new(l.note.as_deref().unwrap_or("")),
                        ]);
                    }

                    println!("{table}");
                    println!("{} accepted links", filtered.len());
                }
            }
        }
    }

    Ok(())
}

// ─── Relate Handler ──────────────────────────────────────────────────────────

fn handle_relate(
    source_ref: &str,
    dir: &str,
    top_n: usize,
    format: &str,
    fy_timeline: bool,
) -> Result<()> {
    use congress_appropriations::approp::embeddings;
    use congress_appropriations::approp::query;

    // Parse "bill_dir:index" reference
    let parts: Vec<&str> = source_ref.splitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "Invalid provision reference: '{source_ref}'. Expected format: bill_dir:index (e.g., 118-hr9468:0)"
        );
    }
    let source_bill_dir = parts[0];
    let source_idx: usize = parts[1].parse().map_err(|_| {
        anyhow::anyhow!("Invalid provision index: '{}'. Must be a number.", parts[1])
    })?;

    // Load all bills and embeddings
    let bills = loading::load_bills(std::path::Path::new(dir))?;
    if bills.is_empty() {
        anyhow::bail!("No extracted bills found in directory: {dir}");
    }

    let mut bill_embeddings: Vec<Option<embeddings::LoadedEmbeddings>> = Vec::new();
    for bill in &bills {
        let emb = embeddings::load(&bill.dir)?;
        bill_embeddings.push(emb);
    }

    // Run relate
    let report = query::relate(
        source_bill_dir,
        source_idx,
        &bills,
        &bill_embeddings,
        top_n,
        fy_timeline,
    )?;

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "hashes" => {
            // Output just the hashes of same_account matches (for piping to link accept)
            for m in &report.same_account {
                println!("{}", m.hash);
            }
        }
        _ => {
            // Table format
            println!(
                "Provision: {} [{}] — {} ({})",
                report.source_bill,
                report.source_index,
                report.source_account,
                report
                    .source_dollars
                    .map(|d| format!("${}", format_dollars(d)))
                    .unwrap_or_else(|| "no amount".to_string()),
            );
            println!();

            if !report.same_account.is_empty() {
                println!("Same Account:");
                let mut table = Table::new();
                table.load_preset(UTF8_FULL_CONDENSED);
                table.set_header(vec![
                    Cell::new("Hash"),
                    Cell::new("Sim"),
                    Cell::new("Bill"),
                    Cell::new("Type"),
                    Cell::new("Account / Description"),
                    Cell::new("Amount ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Timing"),
                    Cell::new("Conf"),
                ]);

                for m in &report.same_account {
                    let timing_str = match (m.timing.as_deref(), m.available_fy) {
                        (Some("advance"), Some(fy)) => format!("advance(FY{fy})"),
                        (Some("supplemental"), _) => "supplemental".to_string(),
                        (Some("current_year"), _) => "current".to_string(),
                        _ => "—".to_string(),
                    };
                    table.add_row(vec![
                        Cell::new(&m.hash),
                        Cell::new(format!("{:.2}", m.similarity)),
                        Cell::new(&m.bill_identifier),
                        Cell::new(&m.provision_type),
                        Cell::new(truncate(&m.account_name, 40)),
                        Cell::new(
                            m.dollars
                                .map(format_dollars)
                                .unwrap_or_else(|| "—".to_string()),
                        )
                        .set_alignment(CellAlignment::Right),
                        Cell::new(&timing_str),
                        Cell::new(m.confidence),
                    ]);
                }
                println!("{table}");
            }

            if !report.related.is_empty() {
                println!();
                println!("Related:");
                let mut table = Table::new();
                table.load_preset(UTF8_FULL_CONDENSED);
                table.set_header(vec![
                    Cell::new("Hash"),
                    Cell::new("Sim"),
                    Cell::new("Bill"),
                    Cell::new("Type"),
                    Cell::new("Account / Description"),
                    Cell::new("Amount ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Conf"),
                ]);

                for m in &report.related {
                    table.add_row(vec![
                        Cell::new(&m.hash),
                        Cell::new(format!("{:.2}", m.similarity)),
                        Cell::new(&m.bill_identifier),
                        Cell::new(&m.provision_type),
                        Cell::new(truncate(&m.account_name, 40)),
                        Cell::new(
                            m.dollars
                                .map(format_dollars)
                                .unwrap_or_else(|| "—".to_string()),
                        )
                        .set_alignment(CellAlignment::Right),
                        Cell::new(m.confidence),
                    ]);
                }
                println!("{table}");
            }

            if let Some(ref timeline) = report.timeline {
                println!();
                println!("Fiscal Year Timeline:");
                let mut table = Table::new();
                table.load_preset(UTF8_FULL_CONDENSED);
                table.set_header(vec![
                    Cell::new("FY"),
                    Cell::new("Current ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Advance ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Supplemental ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Bills"),
                ]);

                for entry in timeline {
                    table.add_row(vec![
                        Cell::new(entry.fy),
                        Cell::new(format_dollars(entry.current_year_ba))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(entry.advance_ba))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(entry.supplemental_ba))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(entry.source_bills.join(", ")),
                    ]);
                }
                println!("{table}");
            }

            let total_matches = report.same_account.len() + report.related.len();
            println!();
            println!(
                "{} matches ({} same account, {} related)",
                total_matches,
                report.same_account.len(),
                report.related.len()
            );
        }
    }

    Ok(())
}

// ─── Enrich Handler ──────────────────────────────────────────────────────────

fn handle_enrich(dir: &str, dry_run: bool, force: bool) -> Result<()> {
    use congress_appropriations::approp::bill_meta;

    let bills = loading::load_bills(std::path::Path::new(dir))?;

    if bills.is_empty() {
        anyhow::bail!("No extracted bills found in directory: {dir}");
    }

    let mut enriched = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for loaded in &bills {
        let bill_id = &loaded.extraction.bill.identifier;
        let bill_dir = &loaded.dir;

        // Skip if bill_meta.json already exists and !force
        if !force && bill_dir.join("bill_meta.json").exists() {
            skipped += 1;
            eprintln!("  skip {bill_id} (bill_meta.json exists, use --force to re-enrich)");
            continue;
        }

        // Find the XML source file
        let xml_path = bill_meta::find_xml_in_dir(bill_dir);

        let extraction_path = bill_dir.join("extraction.json");

        match bill_meta::generate_bill_meta(
            &loaded.extraction,
            xml_path.as_deref(),
            &extraction_path,
        ) {
            Ok(meta) => {
                let n_divisions = meta.subcommittees.len();
                let n_advance = meta
                    .provision_timing
                    .iter()
                    .filter(|t| t.timing == bill_meta::FundingTiming::Advance)
                    .count();
                let n_supplemental = meta
                    .provision_timing
                    .iter()
                    .filter(|t| t.timing == bill_meta::FundingTiming::Supplemental)
                    .count();
                let n_timing = meta.provision_timing.len();

                if dry_run {
                    eprintln!(
                        "  would enrich {bill_id}: nature={:?}, {} divisions, {n_timing} BA provisions ({n_advance} advance, {n_supplemental} supplemental)",
                        meta.bill_nature, n_divisions
                    );
                } else {
                    bill_meta::save_bill_meta(bill_dir, &meta)?;
                    eprintln!(
                        "  enriched {bill_id}: nature={:?}, {} divisions, {n_timing} BA provisions ({n_advance} advance, {n_supplemental} supplemental)",
                        meta.bill_nature, n_divisions
                    );
                }
                enriched += 1;
            }
            Err(e) => {
                eprintln!("  FAILED {bill_id}: {e}");
                failed += 1;
            }
        }
    }

    eprintln!();
    if dry_run {
        eprintln!("Dry run complete: would enrich {enriched}, skipped {skipped}, failed {failed}");
    } else {
        eprintln!("Enrich complete: enriched {enriched}, skipped {skipped}, failed {failed}");
    }

    Ok(())
}

// ─── Embed Handler ───────────────────────────────────────────────────────────

async fn handle_embed(
    dir: &str,
    model: &str,
    dimensions: usize,
    batch_size: usize,
    dry_run: bool,
) -> Result<()> {
    let dir_path = std::path::Path::new(dir);
    let bills = loading::load_bills(dir_path)?;

    if bills.is_empty() {
        println!("No extracted bills found in {dir}");
        return Ok(());
    }

    // For each bill, check if embeddings are up to date
    let mut to_embed = Vec::new();
    for bill in &bills {
        let ext_path = bill.dir.join("extraction.json");
        let ext_hash =
            congress_appropriations::approp::staleness::file_sha256(&ext_path).unwrap_or_default();

        // Check if embeddings exist and are current
        let emb_path = bill.dir.join("embeddings.json");
        let needs_embed = if emb_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&emb_path) {
                if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&content) {
                    let stored = meta
                        .get("extraction_sha256")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    stored != ext_hash
                } else {
                    true
                }
            } else {
                true
            }
        } else {
            true
        };

        if needs_embed {
            to_embed.push((bill, ext_hash));
        } else {
            let name = &bill.extraction.bill.identifier;
            println!("{name}: embeddings up to date, skipping");
        }
    }

    if to_embed.is_empty() {
        println!("\nAll embeddings up to date.");
        return Ok(());
    }

    // Create client once (skipped in dry-run)
    let client = if !dry_run {
        Some(congress_appropriations::api::openai::client::OpenAIClient::from_env()?)
    } else {
        None
    };

    // Embed each bill
    for (bill, ext_hash) in &to_embed {
        let name = &bill.extraction.bill.identifier;
        let n = bill.extraction.provisions.len();
        let texts: Vec<String> = bill
            .extraction
            .provisions
            .iter()
            .map(congress_appropriations::approp::query::build_embedding_text)
            .collect();
        let est_tokens: usize = texts.iter().map(|t| t.len() / 4).sum();

        if dry_run {
            println!("{name}: {n} provisions, ~{est_tokens} estimated tokens (dry run)");
            continue;
        }

        println!("{name}: embedding {n} provisions...");

        let client = client.as_ref().unwrap();
        let mut all_vectors: Vec<f32> = Vec::with_capacity(n * dimensions);
        let mut total_tokens = 0u32;

        for (batch_idx, chunk) in texts.chunks(batch_size).enumerate() {
            let request = congress_appropriations::api::openai::types::EmbeddingRequest {
                model: model.to_string(),
                input: chunk.to_vec(),
                dimensions: Some(dimensions),
            };
            let response = client.embed(request).await?;
            total_tokens += response.usage.total_tokens;

            // Sort by index to ensure order matches
            let mut data = response.data;
            data.sort_by_key(|d| d.index);

            for d in data {
                all_vectors.extend_from_slice(&d.embedding);
            }

            let done = (batch_idx + 1) * batch_size;
            let done = done.min(n);
            print!("\r  {done}/{n} provisions");
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
        println!();

        // Save
        congress_appropriations::approp::embeddings::save(
            &bill.dir,
            model,
            dimensions,
            ext_hash,
            &all_vectors,
        )?;

        println!("  Saved: embeddings.json + vectors.bin ({total_tokens} tokens)");
    }

    Ok(())
}

// ─── API Handlers ────────────────────────────────────────────────────────────

async fn handle_api(action: ApiCommands) -> Result<()> {
    match action {
        ApiCommands::Test => {
            tracing::info!("Testing Congress.gov API...");
            let congress_client = CongressClient::from_env()
                .context("Set CONGRESS_API_KEY — free key at https://api.congress.gov/sign-up/")?;
            let bill = congress_client.test_api().await?;
            tracing::info!("✓ Congress.gov API: {} - {}", bill.number, bill.title);

            tracing::info!("Testing Anthropic API...");
            let anthropic_client =
                congress_appropriations::api::anthropic::AnthropicClient::from_env()
                    .context("Set ANTHROPIC_API_KEY — sign up at https://console.anthropic.com/")?;
            let msg = anthropic_client.test_connection().await?;
            tracing::info!(
                "✓ Anthropic API: model={}, tokens={}",
                msg.model,
                msg.total_tokens()
            );

            Ok(())
        }
        ApiCommands::Bill { action } => handle_bill(action).await,
    }
}

async fn handle_bill(action: BillCommands) -> Result<()> {
    let client = CongressClient::from_env()
        .context("Set CONGRESS_API_KEY — free key at https://api.congress.gov/sign-up/")?;

    match action {
        BillCommands::List {
            congress,
            r#type,
            offset,
            limit,
        } => {
            let c = Congress::new(congress).map_err(|e| anyhow::anyhow!("{e}"))?;
            let bt: BillType = r#type
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid bill type: {}", r#type))?;
            let response = client.list_bills(c, bt, offset, limit).await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
            Ok(())
        }
        BillCommands::Get {
            congress,
            r#type,
            number,
        } => {
            let c = Congress::new(congress).map_err(|e| anyhow::anyhow!("{e}"))?;
            let bt: BillType = r#type
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid bill type: {}", r#type))?;
            let id = BillId::new(c, bt, number);
            let detail = client.get_bill(&id).await?;
            println!("{}", serde_json::to_string_pretty(&detail)?);
            Ok(())
        }
        BillCommands::Text {
            congress,
            r#type,
            number,
        } => {
            let c = Congress::new(congress).map_err(|e| anyhow::anyhow!("{e}"))?;
            let bt: BillType = r#type
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid bill type: {}", r#type))?;
            let id = BillId::new(c, bt, number);
            let versions = client.get_bill_text(&id).await?;
            for v in &versions {
                println!(
                    "Version: {} ({})",
                    v.r#type.as_deref().unwrap_or("unknown"),
                    v.date.as_deref().unwrap_or("no date")
                );
                for f in &v.formats {
                    println!("  {}: {}", f.r#type.as_deref().unwrap_or("?"), f.url);
                }
            }
            Ok(())
        }
    }
}

// ─── Extract Handler ─────────────────────────────────────────────────────────

async fn handle_extract(
    dir: &str,
    dry_run: bool,
    max_parallel: usize,
    model: Option<String>,
    force: bool,
    continue_on_error: bool,
) -> Result<()> {
    use congress_appropriations::api::anthropic::AnthropicClient;
    use congress_appropriations::approp::extraction::ExtractionPipeline;
    use congress_appropriations::approp::verification;

    let total_start = Instant::now();

    tracing::info!("═══════════════════════════════════════════════════════");
    tracing::info!("Extracting appropriations data from {dir}");
    tracing::info!("═══════════════════════════════════════════════════════");

    let dir_path = std::path::Path::new(dir);

    // Find bill sources: prefer XML files, fall back to .txt files
    let bill_sources = loading::find_bill_sources(dir_path);

    if bill_sources.is_empty() {
        tracing::warn!("No bill XML or text files found in {dir}");
        return Ok(());
    }

    tracing::info!("");
    tracing::info!("Found {} bill source files", bill_sources.len());

    if dry_run {
        for (label, path) in &bill_sources {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            let ext = path.extension().unwrap_or_default().to_string_lossy();
            tracing::info!(
                "  [DRY RUN] {label}: {} ({ext}, {size} bytes)",
                path.display(),
            );

            // Parse and build chunks to show estimated work
            let is_xml = path.extension().is_some_and(|e| e == "xml");
            let (text_len, chunk_count) = if is_xml {
                let parsed = xml::parse_bill_xml(
                    path,
                    congress_appropriations::approp::extraction::DEFAULT_MAX_CHUNK_TOKENS,
                )?;
                (parsed.full_text.len(), parsed.chunks.len())
            } else {
                let text = std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                let (_pe, ch) = text_index::build_chunks(
                    &text,
                    congress_appropriations::approp::extraction::DEFAULT_MAX_CHUNK_TOKENS,
                );
                (text.len(), ch.len())
            };

            let est_tokens = text_len / 4;
            tracing::info!("           {chunk_count} chunks, ~{est_tokens} estimated input tokens",);
        }
        return Ok(());
    }

    // Check if any bills actually need extraction before requiring API key
    let needs_extraction = bill_sources.iter().any(|(_, source_path)| {
        let bill_dir = source_path.parent().unwrap_or(std::path::Path::new("."));
        let extraction_path = bill_dir.join("extraction.json");
        !extraction_path.exists() || force
    });

    if !needs_extraction {
        tracing::info!("");
        tracing::info!("All bills already extracted. Use --force to re-extract.");
        return Ok(());
    }

    let anthropic = AnthropicClient::from_env()
        .context("Set ANTHROPIC_API_KEY — sign up at https://console.anthropic.com/")?;

    // Set up pipeline
    let model_name = model.as_deref().unwrap_or("claude-opus-4-6");
    tracing::info!("  Model: {model_name}");
    let mut pipeline = ExtractionPipeline::new(anthropic, model.clone());

    let mut total_provisions = 0usize;
    let mut total_verified = 0usize;
    let mut total_not_found = 0usize;

    for (bill_idx, (label, source_path)) in bill_sources.iter().enumerate() {
        let bill_num = bill_idx + 1;
        let bill_total = bill_sources.len();

        tracing::info!("");
        tracing::info!("═══════════════════════════════════════════════════════");
        tracing::info!("[{bill_num}/{bill_total}] Processing: {label}");
        tracing::info!("═══════════════════════════════════════════════════════");

        let bill_dir = source_path.parent().unwrap_or(std::path::Path::new("."));

        // Skip already-extracted bills unless --force is set
        let extraction_path = bill_dir.join("extraction.json");
        if extraction_path.exists() && !force {
            tracing::info!(
                "  Skipping {label}: extraction.json already exists (use --force to re-extract)"
            );
            continue;
        }

        let bill_start = Instant::now();

        // Phase 1: Parse source and build text + chunks
        tracing::info!("");
        let is_xml = source_path.extension().is_some_and(|e| e == "xml");

        let (bill_text, preamble, chunks) = if is_xml {
            tracing::info!("  Phase 1: Parsing XML and building chunks...");
            let parsed = match xml::parse_bill_xml(
                source_path,
                congress_appropriations::approp::extraction::DEFAULT_MAX_CHUNK_TOKENS,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        "  ⚠ Skipping {}: {} (not a parseable bill XML?)",
                        source_path.display(),
                        e
                    );
                    continue;
                }
            };
            tracing::info!(
                "    {} chars, {} chunks, {} appropriations elements",
                parsed.full_text.len(),
                parsed.chunks.len(),
                parsed.total_appropriations_elements,
            );
            // Save the clean text for verification and manual inspection
            let txt_path = source_path.with_extension("txt");
            std::fs::write(&txt_path, &parsed.full_text)?;
            (parsed.full_text, parsed.preamble, parsed.chunks)
        } else {
            tracing::info!("  Phase 1: Reading text and building chunks...");
            let text = std::fs::read_to_string(source_path)
                .with_context(|| format!("Failed to read {}", source_path.display()))?;
            let (pe, ch) = text_index::build_chunks(
                &text,
                congress_appropriations::approp::extraction::DEFAULT_MAX_CHUNK_TOKENS,
            );
            let preamble_str = text[..pe].to_string();
            (text, preamble_str, ch)
        };

        // Build dollar index for verification
        let index = text_index::build_text_index(&bill_text);
        tracing::info!(
            "    {} dollar amounts, {} sections, {} provisos in {} chars",
            index.dollar_amounts.len(),
            index.section_headers.len(),
            index.proviso_clauses.len(),
            index.total_chars,
        );

        // Phase 2: LLM extraction
        let est_tokens = bill_text.len() / 4;

        tracing::info!("");
        tracing::info!(
            "  Phase 2: Extracting ({} chunks, parallel={}, ~{} tokens)...",
            chunks.len(),
            max_parallel,
            est_tokens
        );
        let (extraction, conversion_report) = match pipeline
            .extract_bill_parallel(
                label,
                &bill_text,
                &preamble,
                &chunks,
                max_parallel,
                bill_dir,
                continue_on_error,
            )
            .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("  ✗ {label}: {e}");
                tracing::error!("    No extraction.json written for this bill.");
                tracing::info!("");
                continue;
            }
        };

        let actual_provisions = extraction.provisions.len();
        total_provisions += actual_provisions;

        // Save extraction
        std::fs::write(
            bill_dir.join("extraction.json"),
            serde_json::to_string_pretty(&extraction)?,
        )?;

        // Save conversion report
        std::fs::write(
            bill_dir.join("conversion.json"),
            serde_json::to_string_pretty(&conversion_report)?,
        )?;

        // Save metadata
        let metadata = pipeline.build_metadata(&bill_text, Some(source_path));
        std::fs::write(
            bill_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?,
        )?;

        // Phase 3: Verification
        tracing::info!("");
        tracing::info!("  Phase 3: Verifying against source text...");
        let report = verification::verify_provisions(&extraction.provisions, &bill_text, &index);

        total_verified += report.summary.amounts_verified;
        total_not_found += report.summary.amounts_not_found;

        tracing::info!(
            "    Amounts verified:    {}/{}",
            report.summary.amounts_verified,
            report.summary.amounts_verified
                + report.summary.amounts_not_found
                + report.summary.amounts_ambiguous
        );
        tracing::info!(
            "    Amounts not found:   {}",
            report.summary.amounts_not_found
        );
        let total_raw = report.summary.raw_text_exact
            + report.summary.raw_text_normalized
            + report.summary.raw_text_spaceless
            + report.summary.raw_text_no_match;
        tracing::info!(
            "    Raw text match:      {}/{} exact, {}/{} normalized, {}/{} spaceless, {} no match",
            report.summary.raw_text_exact,
            total_raw,
            report.summary.raw_text_normalized,
            total_raw,
            report.summary.raw_text_spaceless,
            total_raw,
            report.summary.raw_text_no_match
        );
        let completeness = report.summary.completeness_pct;
        if completeness < 50.0 {
            tracing::warn!(
                "    Dollar completeness: {:.1}% ⚠ INCOMPLETE — majority of dollar amounts in source not captured",
                completeness
            );
        } else if completeness < 90.0 {
            tracing::info!(
                "    Dollar completeness: {:.1}% ⚠ some provisions not captured",
                completeness
            );
        } else {
            tracing::info!("    Dollar completeness: {:.1}%", completeness);
        }
        if !report.summary.provisions_by_detail_level.is_empty() {
            let levels: Vec<String> = report
                .summary
                .provisions_by_detail_level
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            tracing::info!("    Detail levels:       {}", levels.join(", "));
        }

        std::fs::write(
            bill_dir.join("verification.json"),
            serde_json::to_string_pretty(&report)?,
        )?;

        // Save token tracking
        std::fs::write(
            bill_dir.join("tokens.json"),
            serde_json::to_string_pretty(&pipeline.tokens)?,
        )?;

        // LLM self-count mismatch check
        let llm_count = extraction.summary.total_provisions;
        let actual_count = extraction.provisions.len();
        if llm_count != actual_count {
            tracing::warn!(
                "    LLM self-count mismatch: model reported {} provisions but {} were parsed (off by {})",
                llm_count,
                actual_count,
                (actual_count as i64 - llm_count as i64).abs()
            );
        }

        // LLM self-check summary
        tracing::info!("");
        tracing::info!("  LLM self-check:");
        tracing::info!(
            "    Budget authority:      ${}",
            extraction.summary.total_budget_authority
        );
        tracing::info!(
            "    Rescissions:           ${}",
            extraction.summary.total_rescissions
        );
        if !extraction.summary.flagged_issues.is_empty() {
            tracing::info!(
                "    Flagged issues:        {}",
                extraction.summary.flagged_issues.len()
            );
            for issue in &extraction.summary.flagged_issues {
                tracing::debug!("      - {}", &issue[..issue.len().min(100)]);
            }
        }

        let bill_elapsed = bill_start.elapsed();
        let complete_indicator = if completeness >= 90.0 { "✓" } else { "⚠" };
        tracing::info!(
            "  {} {}: {} provisions, {:?} ({:.1}% complete) [{bill_elapsed:.1?}]",
            complete_indicator,
            label,
            actual_count,
            extraction.bill.classification,
            completeness,
        );
    }

    // Final summary
    let total_elapsed = total_start.elapsed();
    tracing::info!("");
    tracing::info!("═══════════════════════════════════════════════════════");
    tracing::info!("Extraction complete [{total_elapsed:.1?}]");
    tracing::info!("  Bills processed:     {}", bill_sources.len());
    tracing::info!("  Total provisions:    {total_provisions}");
    tracing::info!("  Amounts verified:    {total_verified}");
    tracing::info!("  Amounts not found:   {total_not_found}");
    tracing::info!("  Token usage:");
    tracing::info!("    LLM calls:         {}", pipeline.tokens.calls);
    tracing::info!("    Input tokens:      {}", pipeline.tokens.total_input);
    tracing::info!("    Output tokens:     {}", pipeline.tokens.total_output);
    tracing::info!(
        "    Cache read:        {}",
        pipeline.tokens.total_cache_read
    );
    tracing::info!(
        "    Cache create:      {}",
        pipeline.tokens.total_cache_create
    );
    tracing::info!("    Total tokens:      {}", pipeline.tokens.total());
    tracing::info!("═══════════════════════════════════════════════════════");

    Ok(())
}

// ─── Search Handler ──────────────────────────────────────────────────────────

fn compute_quality(amount_status: Option<&str>, match_tier: Option<&str>) -> &'static str {
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

#[allow(clippy::too_many_arguments)]
async fn handle_search(
    dir: &str,
    agency: Option<&str>,
    provision_type: Option<&str>,
    account: Option<&str>,
    keyword: Option<&str>,
    bill: Option<&str>,
    division_filter: Option<&str>,
    min_dollars: Option<i64>,
    max_dollars: Option<i64>,
    format: &str,
    list_types: bool,
    semantic: Option<&str>,
    similar: Option<&str>,
    top: usize,
    fy: Option<u32>,
    subcommittee: Option<&str>,
) -> Result<()> {
    if list_types {
        println!("Available provision types:");
        println!("  appropriation                    Budget authority grant");
        println!("  rescission                       Cancellation of prior budget authority");
        println!("  cr_substitution                  CR anomaly (substituting $X for $Y)");
        println!("  transfer_authority               Permission to move funds between accounts");
        println!("  limitation                       Cap or prohibition on spending");
        println!("  directed_spending                Earmark / community project funding");
        println!("  mandatory_spending_extension     Amendment to authorizing statute");
        println!("  directive                        Reporting requirement or instruction");
        println!("  rider                            Policy provision (no direct spending)");
        println!("  continuing_resolution_baseline   Core CR funding mechanism");
        println!("  other                            Unclassified provisions");
        return Ok(());
    }

    let dir_path = std::path::Path::new(dir);
    let all_bills = loading::load_bills(dir_path)?;

    if all_bills.is_empty() {
        println!("No extracted bills found in {dir}");
        return Ok(());
    }

    // Apply FY filter
    let fy_filtered: Vec<_> = if let Some(fiscal_year) = fy {
        all_bills
            .into_iter()
            .filter(|b| b.extraction.bill.fiscal_years.contains(&fiscal_year))
            .collect()
    } else {
        all_bills
    };

    if fy_filtered.is_empty() {
        println!("No bills found matching the specified filters.");
        return Ok(());
    }

    // Semantic/similar search path (early return).
    // Uses FY-filtered bills but NOT subcommittee-filtered, because
    // subcommittee filtering changes provision indices which breaks
    // vector lookups. Subcommittee is passed as a parameter and
    // applied during the scoring loop.
    if semantic.is_some() || similar.is_some() {
        return handle_semantic_search(
            &fy_filtered,
            dir,
            semantic,
            similar,
            top,
            provision_type,
            agency,
            account,
            keyword,
            bill,
            division_filter,
            min_dollars,
            max_dollars,
            format,
            subcommittee,
        )
        .await;
    }

    // For non-semantic search, do NOT use filter_bills_to_subcommittee because it
    // shifts provision indices, causing the output provision_index to not match the
    // original extraction (and thus breaking `relate bill:index` references).
    // Instead, resolve the subcommittee to a set of (bill_dir, division) pairs and
    // filter results after the search.
    let subcommittee_divisions: Option<HashMap<String, Vec<String>>> = if let Some(sub_slug) =
        subcommittee
    {
        use congress_appropriations::approp::bill_meta::Jurisdiction;
        let jurisdiction = Jurisdiction::from_slug(sub_slug).ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown subcommittee: '{sub_slug}'. Valid slugs: defense, labor-hhs, thud, financial-services, cjs, energy-water, interior, agriculture, legislative-branch, milcon-va, state-foreign-ops, homeland-security"
                )
            })?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for bill in &fy_filtered {
            if let Some(meta) = &bill.bill_meta {
                let divs: Vec<String> = meta
                    .subcommittees
                    .iter()
                    .filter(|s| s.jurisdiction == jurisdiction)
                    .map(|s| s.division.to_uppercase())
                    .collect();
                if !divs.is_empty() {
                    let bill_id = bill.extraction.bill.identifier.clone();
                    map.insert(bill_id, divs);
                }
            } else {
                anyhow::bail!(
                    "{}: --subcommittee requires bill metadata. Run `congress-approp enrich --dir {}` first.",
                    bill.extraction.bill.identifier,
                    dir
                );
            }
        }
        if map.is_empty() {
            println!("No bills found matching the specified filters.");
            return Ok(());
        }
        Some(map)
    } else {
        None
    };

    let bills = fy_filtered;

    if bills.is_empty() {
        println!("No bills found matching the specified filters.");
        return Ok(());
    }

    const KNOWN_PROVISION_TYPES: &[&str] = &[
        "appropriation",
        "rescission",
        "transfer_authority",
        "limitation",
        "directed_spending",
        "cr_substitution",
        "mandatory_spending_extension",
        "directive",
        "rider",
        "continuing_resolution_baseline",
        "other",
    ];

    if let Some(t) = provision_type
        && !KNOWN_PROVISION_TYPES.contains(&t)
    {
        eprintln!("Warning: unknown provision type '{t}'.");
        eprintln!("Known types: {}", KNOWN_PROVISION_TYPES.join(", "));
        eprintln!();
    }

    // Build verification lookup: (bill_identifier, provision_index) -> (verified, match_tier)
    let ver_lookup = build_verification_lookup(&bills);

    // Collect matching provisions
    struct Match {
        bill_id: String,
        congress: Option<u32>,
        provision_index: usize,
        provision_type: String,
        account_name: String,
        description: String,
        agency: String,
        dollars: Option<i64>,
        old_dollars: Option<i64>,
        semantics: String,
        section: String,
        division: String,
        raw_text: String,
        verified: Option<String>,
        match_tier: Option<String>,
        quality: String,
        fiscal_year: Option<u32>,
        detail_level: String,
        confidence: f32,
    }

    let mut matches: Vec<Match> = Vec::new();

    for loaded in &bills {
        let bill_id = &loaded.extraction.bill.identifier;
        let bill_congress: Option<u32> = loaded
            .bill_meta
            .as_ref()
            .and_then(|m| m.congress)
            .or_else(|| {
                loaded
                    .dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|name| name.split('-').next().and_then(|s| s.parse::<u32>().ok()))
            });

        // Bill filter
        if let Some(bill_filter) = bill
            && !bill_id.to_lowercase().contains(&bill_filter.to_lowercase())
        {
            continue;
        }

        for (idx, provision) in loaded.extraction.provisions.iter().enumerate() {
            let ptype = provision.type_str();
            let paccount = provision.account_name();
            let pagency = provision.agency();
            let praw = provision.raw_text();
            let (pdollars, psemantics) = prov_amount_strs(provision);
            let psection = provision.section();
            let pdivision = provision.division().unwrap_or("");

            // Apply filters
            if let Some(type_filter) = provision_type
                && ptype != type_filter
            {
                continue;
            }
            if let Some(agency_filter) = agency
                && !pagency
                    .to_lowercase()
                    .contains(&agency_filter.to_lowercase())
            {
                continue;
            }
            if let Some(account_filter) = account
                && !paccount
                    .to_lowercase()
                    .contains(&account_filter.to_lowercase())
            {
                continue;
            }
            if let Some(keyword_filter) = keyword
                && !praw.to_lowercase().contains(&keyword_filter.to_lowercase())
            {
                continue;
            }
            if let Some(div_filter) = division_filter
                && !pdivision.eq_ignore_ascii_case(div_filter)
            {
                continue;
            }
            // Subcommittee filter: check if this provision's division is in the
            // resolved jurisdiction mapping. Applied here (not via
            // filter_bills_to_subcommittee) to preserve original provision indices
            // so that `relate bill:index` references work correctly.
            if let Some(ref sub_divs) = subcommittee_divisions {
                let dominated = sub_divs
                    .get(bill_id.as_str())
                    .is_some_and(|divs| divs.iter().any(|d| d.eq_ignore_ascii_case(pdivision)));
                if !dominated {
                    continue;
                }
            }
            if min_dollars.is_some() || max_dollars.is_some() {
                let abs_dollars = provision
                    .amount()
                    .and_then(|a| a.dollars())
                    .map(|d| d.abs());
                if let Some(min) = min_dollars {
                    match abs_dollars {
                        Some(d) if d >= min => {}
                        _ => continue,
                    }
                }
                if let Some(max) = max_dollars {
                    match abs_dollars {
                        Some(d) if d <= max => {}
                        _ => continue,
                    }
                }
            }

            let ver_key = (bill_id.as_str(), idx);
            let (verified, tier) = ver_lookup.get(&ver_key).cloned().unwrap_or((None, None));

            let pold = provision.old_amount().and_then(|a| a.dollars());
            let pdesc = provision.description();

            let quality_val = compute_quality(verified.as_deref(), tier);

            matches.push(Match {
                bill_id: bill_id.clone(),
                congress: bill_congress,
                provision_index: idx,
                provision_type: ptype.to_string(),
                account_name: paccount.to_string(),
                description: pdesc.to_string(),
                agency: pagency.to_string(),
                dollars: pdollars,
                old_dollars: pold,
                semantics: psemantics.to_string(),
                section: psection.to_string(),
                division: pdivision.to_string(),
                raw_text: praw.to_string(),
                verified,
                match_tier: tier.map(|s| s.to_string()),
                quality: quality_val.to_string(),
                fiscal_year: provision.fiscal_year(),
                detail_level: provision.detail_level().to_string(),
                confidence: provision.confidence(),
            });
        }
    }

    // Output
    match format {
        "json" => {
            let output: Vec<serde_json::Value> = matches
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "bill": format_bill_id(&m.bill_id, m.congress),
                        "congress": m.congress,
                        "provision_index": m.provision_index,
                        "provision_type": m.provision_type,
                        "account_name": m.account_name,
                        "description": m.description,
                        "agency": m.agency,
                        "dollars": m.dollars,
                        "old_dollars": m.old_dollars,
                        "semantics": m.semantics,
                        "section": m.section,
                        "division": m.division,
                        "raw_text": m.raw_text,
                        "amount_status": m.verified,
                        "quality": m.quality,
                        "match_tier": m.match_tier,
                        "fiscal_year": m.fiscal_year,
                        "detail_level": m.detail_level,
                        "confidence": m.confidence,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        "jsonl" => {
            for m in &matches {
                let obj = serde_json::json!({
                    "bill": format_bill_id(&m.bill_id, m.congress),
                    "congress": m.congress,
                    "provision_index": m.provision_index,
                    "provision_type": m.provision_type,
                    "account_name": m.account_name,
                    "description": m.description,
                    "agency": m.agency,
                    "dollars": m.dollars,
                    "old_dollars": m.old_dollars,
                    "semantics": m.semantics,
                    "section": m.section,
                    "division": m.division,
                    "raw_text": m.raw_text,
                    "amount_status": m.verified,
                    "quality": m.quality,
                    "match_tier": m.match_tier,
                    "fiscal_year": m.fiscal_year,
                    "detail_level": m.detail_level,
                    "confidence": m.confidence,
                });
                println!("{}", serde_json::to_string(&obj)?);
            }
        }
        "csv" => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            wtr.write_record([
                "bill",
                "congress",
                "provision_type",
                "account_name",
                "description",
                "agency",
                "dollars",
                "old_dollars",
                "semantics",
                "section",
                "division",
                "amount_status",
                "quality",
                "raw_text",
                "provision_index",
                "match_tier",
                "fiscal_year",
                "detail_level",
                "confidence",
            ])?;
            for m in &matches {
                wtr.write_record([
                    &format_bill_id(&m.bill_id, m.congress),
                    &m.congress.map(|c| c.to_string()).unwrap_or_default(),
                    &m.provision_type,
                    &m.account_name,
                    &m.description,
                    &m.agency,
                    &m.dollars.map(|d| d.to_string()).unwrap_or_default(),
                    &m.old_dollars.map(|d| d.to_string()).unwrap_or_default(),
                    &m.semantics,
                    &m.section,
                    &m.division,
                    &m.verified.clone().unwrap_or_else(|| "n/a".to_string()),
                    &m.quality,
                    &m.raw_text,
                    &m.provision_index.to_string(),
                    &m.match_tier.clone().unwrap_or_default(),
                    &m.fiscal_year.map(|y| y.to_string()).unwrap_or_default(),
                    &m.detail_level,
                    &format!("{:.2}", m.confidence),
                ])?;
            }
            wtr.flush()?;
        }
        _ => {
            // table format
            if matches.is_empty() {
                println!("No matching provisions found.");
                return Ok(());
            }

            let mut table = Table::new();
            table.load_preset(UTF8_FULL_CONDENSED);

            // Determine if all matches are the same type for type-adaptive headers
            let single_type = provision_type;

            match single_type {
                Some("directive") => {
                    table.set_header(vec![
                        Cell::new("$"),
                        Cell::new("Bill"),
                        Cell::new("Description"),
                        Cell::new("Section"),
                        Cell::new("Div"),
                    ]);
                    for m in &matches {
                        let vi = match m.verified.as_deref() {
                            Some("found") => "✓",
                            Some("found_multiple") => "≈",
                            Some("not_found") => "✗",
                            _ => " ",
                        };
                        table.add_row(vec![
                            Cell::new(vi),
                            Cell::new(&m.bill_id),
                            Cell::new(truncate(&m.description, 70)),
                            Cell::new(&m.section),
                            Cell::new(&m.division),
                        ]);
                    }
                }
                Some("rider") => {
                    table.set_header(vec![
                        Cell::new("$"),
                        Cell::new("Bill"),
                        Cell::new("Description"),
                        Cell::new("Section"),
                        Cell::new("Div"),
                    ]);
                    for m in &matches {
                        let vi = match m.verified.as_deref() {
                            Some("found") => "✓",
                            Some("found_multiple") => "≈",
                            Some("not_found") => "✗",
                            _ => " ",
                        };
                        table.add_row(vec![
                            Cell::new(vi),
                            Cell::new(&m.bill_id),
                            Cell::new(truncate(&m.description, 70)),
                            Cell::new(&m.section),
                            Cell::new(&m.division),
                        ]);
                    }
                }
                Some("mandatory_spending_extension") => {
                    table.set_header(vec![
                        Cell::new("$"),
                        Cell::new("Bill"),
                        Cell::new("Program"),
                        Cell::new("Amount ($)").set_alignment(CellAlignment::Right),
                        Cell::new("Section"),
                        Cell::new("Div"),
                    ]);
                    for m in &matches {
                        let vi = match m.verified.as_deref() {
                            Some("found") => "✓",
                            Some("found_multiple") => "≈",
                            Some("not_found") => "✗",
                            _ => " ",
                        };
                        let amt = m
                            .dollars
                            .map(format_dollars)
                            .unwrap_or_else(|| "—".to_string());
                        table.add_row(vec![
                            Cell::new(vi),
                            Cell::new(&m.bill_id),
                            Cell::new(truncate(&m.description, 50)),
                            Cell::new(&amt).set_alignment(CellAlignment::Right),
                            Cell::new(&m.section),
                            Cell::new(&m.division),
                        ]);
                    }
                }
                Some("cr_substitution") => {
                    table.set_header(vec![
                        Cell::new("$"),
                        Cell::new("Bill"),
                        Cell::new("Account"),
                        Cell::new("New ($)").set_alignment(CellAlignment::Right),
                        Cell::new("Old ($)").set_alignment(CellAlignment::Right),
                        Cell::new("Delta ($)").set_alignment(CellAlignment::Right),
                        Cell::new("Section"),
                        Cell::new("Div"),
                    ]);
                    for m in &matches {
                        let vi = match m.verified.as_deref() {
                            Some("found") => "✓",
                            Some("found_multiple") => "≈",
                            Some("not_found") => "✗",
                            _ => " ",
                        };
                        let new_s = m
                            .dollars
                            .map(format_dollars)
                            .unwrap_or_else(|| "—".to_string());
                        let old_s = m
                            .old_dollars
                            .map(format_dollars)
                            .unwrap_or_else(|| "—".to_string());
                        let delta_s = match (m.dollars, m.old_dollars) {
                            (Some(n), Some(o)) => format_dollars_signed(n - o),
                            _ => "—".to_string(),
                        };
                        table.add_row(vec![
                            Cell::new(vi),
                            Cell::new(&m.bill_id),
                            Cell::new(truncate(&m.account_name, 40)),
                            Cell::new(&new_s).set_alignment(CellAlignment::Right),
                            Cell::new(&old_s).set_alignment(CellAlignment::Right),
                            Cell::new(&delta_s).set_alignment(CellAlignment::Right),
                            Cell::new(&m.section),
                            Cell::new(&m.division),
                        ]);
                    }
                }
                Some("limitation") => {
                    table.set_header(vec![
                        Cell::new("$"),
                        Cell::new("Bill"),
                        Cell::new("Description"),
                        Cell::new("Account"),
                        Cell::new("Amount ($)").set_alignment(CellAlignment::Right),
                        Cell::new("Section"),
                        Cell::new("Div"),
                    ]);
                    for m in &matches {
                        let vi = match m.verified.as_deref() {
                            Some("found") => "✓",
                            Some("found_multiple") => "≈",
                            Some("not_found") => "✗",
                            _ => " ",
                        };
                        let amt = m
                            .dollars
                            .map(format_dollars)
                            .unwrap_or_else(|| "—".to_string());
                        table.add_row(vec![
                            Cell::new(vi),
                            Cell::new(&m.bill_id),
                            Cell::new(truncate(&m.description, 50)),
                            Cell::new(truncate(&m.account_name, 30)),
                            Cell::new(&amt).set_alignment(CellAlignment::Right),
                            Cell::new(&m.section),
                            Cell::new(&m.division),
                        ]);
                    }
                }
                _ => {
                    // Default: mixed types or appropriation/rescission
                    table.set_header(vec![
                        Cell::new("$"),
                        Cell::new("Bill"),
                        Cell::new("Type"),
                        Cell::new("Description / Account"),
                        Cell::new("Amount ($)").set_alignment(CellAlignment::Right),
                        Cell::new("Section"),
                        Cell::new("Div"),
                    ]);
                    for m in &matches {
                        let vi = match m.verified.as_deref() {
                            Some("found") => "✓",
                            Some("found_multiple") => "≈",
                            Some("not_found") => "✗",
                            _ => " ",
                        };
                        let amt = m
                            .dollars
                            .map(format_dollars)
                            .unwrap_or_else(|| "—".to_string());
                        // Show description if account is empty, otherwise account
                        let desc_or_acct = if m.account_name.is_empty() {
                            truncate(&m.description, 45)
                        } else {
                            truncate(&m.account_name, 45)
                        };
                        table.add_row(vec![
                            Cell::new(vi),
                            Cell::new(&m.bill_id),
                            Cell::new(&m.provision_type),
                            Cell::new(desc_or_acct),
                            Cell::new(&amt).set_alignment(CellAlignment::Right),
                            Cell::new(&m.section),
                            Cell::new(&m.division),
                        ]);
                    }
                }
            }

            println!("{table}");
            println!("{} provisions found", matches.len());
            println!();
            println!(
                "$ = Amount status: ✓ found (unique), ≈ found (multiple matches), ✗ not found"
            );

            // Warn about incomplete source bills
            let incomplete: Vec<String> = bills
                .iter()
                .filter_map(|b| {
                    b.verification.as_ref().and_then(|v| {
                        if v.summary.completeness_pct < 50.0 {
                            Some(format!(
                                "{} ({:.1}% complete)",
                                b.extraction.bill.identifier, v.summary.completeness_pct
                            ))
                        } else {
                            None
                        }
                    })
                })
                .collect();
            if !incomplete.is_empty() {
                println!();
                println!(
                    "Note: some source bills have incomplete extractions: {}",
                    incomplete.join(", ")
                );
                println!("Run 'report' for full verification details.");
            }
        }
    }

    // Smart stderr footer for non-table formats: warn about mixed semantics
    if format != "table" && !matches.is_empty() {
        let mut ba_count = 0usize;
        let mut ba_total = 0i64;
        let mut ref_count = 0usize;
        let mut other_sem_count = 0usize;
        for m in &matches {
            match m.semantics.as_str() {
                "new_budget_authority" => {
                    ba_count += 1;
                    ba_total += m.dollars.unwrap_or(0);
                }
                "reference_amount" => ref_count += 1,
                _ => other_sem_count += 1,
            }
        }
        if ref_count > 0 || other_sem_count > 0 {
            eprintln!(
                "{} provisions exported: {} new_budget_authority (${:.1}B), {} reference_amount, {} other semantics",
                matches.len(),
                ba_count,
                ba_total as f64 / 1e9,
                ref_count,
                other_sem_count
            );
            eprintln!(
                "⚠ To compute budget authority totals, filter to semantics=new_budget_authority and detail_level!=sub_allocation."
            );
            eprintln!("  Or use `congress-approp summary` which does this automatically.");
        }
    }

    Ok(())
}

// ─── Semantic Search Handler ─────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn handle_semantic_search(
    bills: &[loading::LoadedBill],
    dir: &str,
    semantic: Option<&str>,
    similar: Option<&str>,
    top_n: usize,
    type_filter: Option<&str>,
    agency: Option<&str>,
    account: Option<&str>,
    keyword: Option<&str>,
    bill_filter: Option<&str>,
    division_filter: Option<&str>,
    min_dollars: Option<i64>,
    max_dollars: Option<i64>,
    format: &str,
    subcommittee: Option<&str>,
) -> Result<()> {
    use congress_appropriations::approp::embeddings;

    // Load embeddings for all bills
    let mut bill_embeddings: Vec<Option<embeddings::LoadedEmbeddings>> = Vec::new();
    let mut has_any = false;
    for bill in bills {
        match embeddings::load(&bill.dir)? {
            Some(emb) => {
                has_any = true;
                bill_embeddings.push(Some(emb));
            }
            None => {
                eprintln!(
                    "⚠ {}: no embeddings found, excluded from semantic search",
                    bill.extraction.bill.identifier
                );
                bill_embeddings.push(None);
            }
        }
    }
    if !has_any {
        anyhow::bail!("No embeddings found. Run `congress-approp embed --dir {dir}` first.");
    }

    // Get the query vector
    let query_vec: Vec<f32> = if let Some(query_text) = semantic {
        // Embed the query text
        let client = congress_appropriations::api::openai::client::OpenAIClient::from_env()?;
        // Determine dimensions from first available embeddings
        let first_emb = bill_embeddings.iter().flatten().next().unwrap();
        let dims = first_emb.dimensions();
        let model = first_emb.metadata.model.clone();
        let request = congress_appropriations::api::openai::types::EmbeddingRequest {
            model,
            input: vec![query_text.to_string()],
            dimensions: Some(dims),
        };
        // Need to run async from sync context
        let response = client.embed(request).await?;
        response.data.into_iter().next().unwrap().embedding
    } else if let Some(similar_ref) = similar {
        // Parse "bill_dir:index"
        let parts: Vec<&str> = similar_ref.splitn(2, ':').collect();
        anyhow::ensure!(
            parts.len() == 2,
            "Invalid --similar format. Use bill_dir:index (e.g., 118-hr4366:42)"
        );
        let target_dir = parts[0];
        let target_idx: usize = parts[1]
            .parse()
            .context("Invalid provision index in --similar")?;

        // Find the bill and get the vector
        let mut found = None;
        for (i, bill) in bills.iter().enumerate() {
            let dir_name = bill
                .dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if dir_name == target_dir {
                if let Some(emb) = &bill_embeddings[i] {
                    anyhow::ensure!(
                        target_idx < emb.count(),
                        "Provision index {target_idx} out of range (bill has {} provisions)",
                        emb.count()
                    );
                    found = Some(emb.vector(target_idx).to_vec());
                } else {
                    anyhow::bail!("No embeddings for {target_dir}");
                }
                break;
            }
        }
        found.context(format!("Bill directory '{target_dir}' not found"))?
    } else {
        unreachable!()
    };

    // Score all provisions
    struct ScoredProvision<'a> {
        bill_id: &'a str,
        congress: Option<u32>,
        #[allow(dead_code)]
        bill_dir_name: String,
        provision_index: usize,
        provision: &'a Provision,
        similarity: f32,
    }

    let mut scored: Vec<ScoredProvision<'_>> = Vec::new();
    for (i, bill) in bills.iter().enumerate() {
        let Some(emb) = &bill_embeddings[i] else {
            continue;
        };
        let bill_id = bill.extraction.bill.identifier.as_str();
        let bill_congress: Option<u32> =
            bill.bill_meta
                .as_ref()
                .and_then(|m| m.congress)
                .or_else(|| {
                    bill.dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|name| name.split('-').next().and_then(|s| s.parse::<u32>().ok()))
                });
        let bill_dir_name = bill
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        for (idx, provision) in bill.extraction.provisions.iter().enumerate() {
            // Skip the source provision for --similar
            if let Some(similar_ref) = similar {
                let parts: Vec<&str> = similar_ref.splitn(2, ':').collect();
                if parts.len() == 2
                    && bill_dir_name == parts[0]
                    && idx == parts[1].parse::<usize>().unwrap_or(usize::MAX)
                {
                    continue;
                }
            }

            // Apply hard filters
            if let Some(tf) = type_filter
                && provision.type_str() != tf
            {
                continue;
            }
            if let Some(af) = agency
                && !provision
                    .agency()
                    .to_lowercase()
                    .contains(&af.to_lowercase())
            {
                continue;
            }
            if let Some(ac) = account
                && !provision
                    .account_name()
                    .to_lowercase()
                    .contains(&ac.to_lowercase())
            {
                continue;
            }
            if let Some(kw) = keyword
                && !provision
                    .raw_text()
                    .to_lowercase()
                    .contains(&kw.to_lowercase())
            {
                continue;
            }
            if let Some(bf) = bill_filter
                && !bill_id.to_lowercase().contains(&bf.to_lowercase())
            {
                continue;
            }
            if let Some(df) = division_filter {
                let pdiv = provision.division().unwrap_or("");
                if !pdiv.eq_ignore_ascii_case(df) {
                    continue;
                }
            }
            if let Some(min) = min_dollars {
                let d = provision
                    .amount()
                    .and_then(|a| a.dollars())
                    .map(|d| d.abs());
                match d {
                    Some(d) if d >= min => {}
                    _ => continue,
                }
            }
            if let Some(max) = max_dollars {
                let d = provision
                    .amount()
                    .and_then(|a| a.dollars())
                    .map(|d| d.abs());
                match d {
                    Some(d) if d <= max => {}
                    _ => continue,
                }
            }

            // Subcommittee filter: check provision's division against bill_meta jurisdiction.
            // This is applied here (not via filter_bills_to_subcommittee) to preserve
            // original provision indices for correct vector lookups.
            if let Some(sub_slug) = subcommittee {
                use congress_appropriations::approp::bill_meta::Jurisdiction;
                if let Some(target_j) = Jurisdiction::from_slug(sub_slug) {
                    let dominated = if let Some(meta) = &bill.bill_meta {
                        let prov_div = provision.division().unwrap_or("");
                        meta.subcommittees.iter().any(|s| {
                            s.division.eq_ignore_ascii_case(prov_div) && s.jurisdiction == target_j
                        })
                    } else {
                        false // no bill_meta → can't resolve subcommittee → skip
                    };
                    if !dominated {
                        continue;
                    }
                }
            }

            let sim = embeddings::cosine_similarity(&query_vec, emb.vector(idx));
            scored.push(ScoredProvision {
                bill_id,
                congress: bill_congress,
                bill_dir_name: bill_dir_name.clone(),
                provision_index: idx,
                provision,
                similarity: sim,
            });
        }
    }

    // Sort and truncate
    scored.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(top_n);

    if scored.is_empty() {
        println!("No matching provisions found.");
        return Ok(());
    }

    // Output
    match format {
        "json" => {
            let output: Vec<serde_json::Value> = scored
                .iter()
                .map(|s| {
                    let dollars = s.provision.amount().and_then(|a| a.dollars());
                    serde_json::json!({
                        "bill": format_bill_id(s.bill_id, s.congress),
                        "congress": s.congress,
                        "provision_index": s.provision_index,
                        "similarity": (s.similarity * 1000.0).round() / 1000.0,
                        "provision_type": s.provision.type_str(),
                        "account_name": s.provision.account_name(),
                        "agency": s.provision.agency(),
                        "dollars": dollars,
                        "division": s.provision.division(),
                        "section": s.provision.section(),
                        "description": s.provision.description(),
                        "raw_text": s.provision.raw_text(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        "jsonl" => {
            for s in &scored {
                let dollars = s.provision.amount().and_then(|a| a.dollars());
                let obj = serde_json::json!({
                    "bill": format_bill_id(s.bill_id, s.congress),
                    "congress": s.congress,
                    "provision_index": s.provision_index,
                    "similarity": (s.similarity * 1000.0).round() / 1000.0,
                    "provision_type": s.provision.type_str(),
                    "account_name": s.provision.account_name(),
                    "agency": s.provision.agency(),
                    "dollars": dollars,
                    "division": s.provision.division(),
                    "section": s.provision.section(),
                    "raw_text": s.provision.raw_text(),
                });
                println!("{}", serde_json::to_string(&obj)?);
            }
        }
        "csv" => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            wtr.write_record([
                "bill",
                "provision_index",
                "similarity",
                "provision_type",
                "account_name",
                "agency",
                "dollars",
                "division",
                "section",
                "description",
                "raw_text",
            ])?;
            for s in &scored {
                let dollars = s.provision.amount().and_then(|a| a.dollars());
                wtr.write_record([
                    &format_bill_id(s.bill_id, s.congress),
                    &s.provision_index.to_string(),
                    &format!("{:.3}", s.similarity),
                    s.provision.type_str(),
                    s.provision.account_name(),
                    s.provision.agency(),
                    &dollars.map(|d| d.to_string()).unwrap_or_default(),
                    s.provision.division().unwrap_or(""),
                    s.provision.section(),
                    s.provision.description(),
                    s.provision.raw_text(),
                ])?;
            }
            wtr.flush()?;
        }
        _ => {
            // Table format
            let mut table = Table::new();
            table.load_preset(UTF8_FULL_CONDENSED);
            table.set_header(vec![
                Cell::new("Sim"),
                Cell::new("Bill"),
                Cell::new("Type"),
                Cell::new("Description / Account").set_alignment(CellAlignment::Left),
                Cell::new("Amount ($)").set_alignment(CellAlignment::Right),
                Cell::new("Div"),
            ]);

            for s in &scored {
                let dollars = s.provision.amount().and_then(|a| a.dollars());
                let dollars_str = dollars
                    .map(format_dollars)
                    .unwrap_or_else(|| "—".to_string());
                let desc = if !s.provision.account_name().is_empty() {
                    truncate(s.provision.account_name(), 45)
                } else if !s.provision.description().is_empty() {
                    truncate(s.provision.description(), 45)
                } else {
                    truncate(s.provision.raw_text(), 45)
                };
                let div = s.provision.division().unwrap_or("");
                table.add_row(vec![
                    Cell::new(format!("{:.2}", s.similarity)),
                    Cell::new(format_bill_id(s.bill_id, s.congress)),
                    Cell::new(s.provision.type_str()),
                    Cell::new(desc),
                    Cell::new(dollars_str).set_alignment(CellAlignment::Right),
                    Cell::new(div),
                ]);
            }
            println!("{table}");
            println!("\n{} provisions found", scored.len());
        }
    }

    Ok(())
}

// ─── Summary Handler ─────────────────────────────────────────────────────────

/// Filter bills to only include provisions from divisions matching the given jurisdiction.
/// Creates new LoadedBill copies with filtered provision lists.
fn filter_bills_to_subcommittee(
    bills: &[loading::LoadedBill],
    jurisdiction: &congress_appropriations::approp::bill_meta::Jurisdiction,
) -> Result<Vec<loading::LoadedBill>> {
    let mut filtered = Vec::new();
    for bill in bills {
        let meta = bill.bill_meta.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "{}: --subcommittee requires bill metadata. Run `congress-approp enrich --dir <DIR>` first.",
                bill.extraction.bill.identifier
            )
        })?;

        // Find division letters for this jurisdiction
        let matching_divisions: Vec<&str> = meta
            .subcommittees
            .iter()
            .filter(|s| s.jurisdiction == *jurisdiction)
            .map(|s| s.division.as_str())
            .collect();

        if matching_divisions.is_empty() {
            continue; // This bill doesn't contain this subcommittee
        }

        // Filter provisions to only those in matching divisions
        let filtered_provisions: Vec<_> = bill
            .extraction
            .provisions
            .iter()
            .filter(|p| {
                if let Some(div) = p.division() {
                    matching_divisions
                        .iter()
                        .any(|d| d.eq_ignore_ascii_case(div))
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        if filtered_provisions.is_empty() {
            continue;
        }

        let mut filtered_extraction = bill.extraction.clone();
        filtered_extraction.provisions = filtered_provisions;

        filtered.push(loading::LoadedBill {
            dir: bill.dir.clone(),
            extraction: filtered_extraction,
            verification: bill.verification.clone(),
            metadata: bill.metadata.clone(),
            bill_meta: bill.bill_meta.clone(),
        });
    }
    Ok(filtered)
}

fn handle_summary(
    dir: &str,
    format: &str,
    by_agency: bool,
    fy: Option<u32>,
    subcommittee: Option<&str>,
    show_advance: bool,
) -> Result<()> {
    let dir_path = std::path::Path::new(dir);
    let all_bills = loading::load_bills(dir_path)?;

    if all_bills.is_empty() {
        println!("No extracted bills found in {dir}");
        return Ok(());
    }

    // Apply FY filter
    let fy_filtered: Vec<_> = if let Some(fiscal_year) = fy {
        all_bills
            .into_iter()
            .filter(|b| b.extraction.bill.fiscal_years.contains(&fiscal_year))
            .collect()
    } else {
        all_bills
    };

    // Apply subcommittee filter
    let bills = if let Some(sub_slug) = subcommittee {
        use congress_appropriations::approp::bill_meta::Jurisdiction;
        let jurisdiction = Jurisdiction::from_slug(sub_slug).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown subcommittee: '{sub_slug}'. Valid slugs: defense, labor-hhs, thud, financial-services, cjs, energy-water, interior, agriculture, legislative-branch, milcon-va, state-foreign-ops, homeland-security"
            )
        })?;
        filter_bills_to_subcommittee(&fy_filtered, &jurisdiction)?
    } else {
        fy_filtered
    };

    if bills.is_empty() {
        println!("No bills found matching the specified filters.");
        // Show what IS available to help the user
        let all_for_hint = loading::load_bills(dir_path).unwrap_or_default();
        if !all_for_hint.is_empty() {
            let mut available_fys: Vec<u32> = all_for_hint
                .iter()
                .flat_map(|b| b.extraction.bill.fiscal_years.iter().copied())
                .collect();
            available_fys.sort();
            available_fys.dedup();
            if !available_fys.is_empty() {
                let fy_strs: Vec<String> = available_fys.iter().map(|y| y.to_string()).collect();
                eprintln!("  Available fiscal years: {}", fy_strs.join(", "));
            }
            if subcommittee.is_some() {
                let mut available_subs: Vec<String> = all_for_hint
                    .iter()
                    .filter_map(|b| b.bill_meta.as_ref())
                    .flat_map(|m| {
                        m.subcommittees
                            .iter()
                            .map(|s| s.jurisdiction.slug().to_string())
                    })
                    .collect();
                available_subs.sort();
                available_subs.dedup();
                // Remove generic ones for cleaner output
                available_subs.retain(|s| {
                    s != "other" && s != "extenders" && s != "policy" && s != "budget-process"
                });
                if !available_subs.is_empty() {
                    eprintln!("  Available subcommittees: {}", available_subs.join(", "));
                }
            }
        }
        return Ok(());
    }

    use congress_appropriations::approp::query;

    // Use the library function for the core computation.
    // This replaces ~130 lines of inline reimplementation that existed before consolidation.
    let mut summaries = query::summarize(&bills);

    // If --show-advance is NOT requested, strip the advance fields to keep output clean.
    // The library function always computes them when bill_meta is available.
    if !show_advance {
        for s in &mut summaries {
            s.current_year_ba = None;
            s.advance_ba = None;
        }
    } else {
        // Warn about bills missing bill_meta when --show-advance is requested
        for s in &summaries {
            if s.current_year_ba.is_none() {
                let bill = bills
                    .iter()
                    .find(|b| b.extraction.bill.identifier == s.identifier);
                if let Some(bill) = bill
                    && bill.bill_meta.is_none()
                {
                    eprintln!(
                        "  hint: {}: --show-advance requires bill metadata. Run `congress-approp enrich --dir {}` first.",
                        s.identifier, dir
                    );
                }
            }
        }
    }

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&summaries)?);
        }
        "jsonl" => {
            for s in &summaries {
                println!("{}", serde_json::to_string(&s)?);
            }
        }
        _ => {
            let mut table = Table::new();
            table.load_preset(UTF8_FULL_CONDENSED);
            if show_advance && summaries.iter().any(|s| s.current_year_ba.is_some()) {
                table.set_header(vec![
                    Cell::new("Bill"),
                    Cell::new("FYs"),
                    Cell::new("Classification"),
                    Cell::new("Provisions").set_alignment(CellAlignment::Right),
                    Cell::new("Current ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Advance ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Total BA ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Rescissions ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Net BA ($)").set_alignment(CellAlignment::Right),
                ]);
            } else {
                table.set_header(vec![
                    Cell::new("Bill"),
                    Cell::new("FYs"),
                    Cell::new("Classification"),
                    Cell::new("Provisions").set_alignment(CellAlignment::Right),
                    Cell::new("Budget Auth ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Rescissions ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Net BA ($)").set_alignment(CellAlignment::Right),
                ]);
            }

            let mut total_provs = 0usize;
            let mut total_ba = 0i64;
            let mut total_resc = 0i64;

            let has_advance_data =
                show_advance && summaries.iter().any(|s| s.current_year_ba.is_some());

            let mut total_current = 0i64;
            let mut total_advance = 0i64;

            for s in &summaries {
                total_provs += s.provisions;
                total_ba += s.budget_authority;
                total_resc += s.rescissions;
                if let Some(c) = s.current_year_ba {
                    total_current += c;
                }
                if let Some(a) = s.advance_ba {
                    total_advance += a;
                }

                let fy_str = s
                    .fiscal_years
                    .iter()
                    .map(|y| y.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                if has_advance_data {
                    table.add_row(vec![
                        Cell::new(format_bill_id(&s.identifier, s.congress)),
                        Cell::new(&fy_str),
                        Cell::new(&s.classification),
                        Cell::new(s.provisions).set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(
                            s.current_year_ba.unwrap_or(s.budget_authority),
                        ))
                        .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(s.advance_ba.unwrap_or(0)))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(s.budget_authority))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(s.rescissions))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(s.net_ba)).set_alignment(CellAlignment::Right),
                    ]);
                } else {
                    table.add_row(vec![
                        Cell::new(format_bill_id(&s.identifier, s.congress)),
                        Cell::new(&fy_str),
                        Cell::new(&s.classification),
                        Cell::new(s.provisions).set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(s.budget_authority))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(s.rescissions))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(s.net_ba)).set_alignment(CellAlignment::Right),
                    ]);
                }
            }

            // Totals row
            if has_advance_data {
                table.add_row(vec![
                    Cell::new("TOTAL").fg(Color::White),
                    Cell::new(""),
                    Cell::new(""),
                    Cell::new(total_provs)
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                    Cell::new(format_dollars(total_current))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                    Cell::new(format_dollars(total_advance))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                    Cell::new(format_dollars(total_ba))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                    Cell::new(format_dollars(total_resc))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                    Cell::new(format_dollars(total_ba - total_resc))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                ]);
            } else {
                table.add_row(vec![
                    Cell::new("TOTAL").fg(Color::White),
                    Cell::new(""),
                    Cell::new(""),
                    Cell::new(total_provs)
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                    Cell::new(format_dollars(total_ba))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                    Cell::new(format_dollars(total_resc))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                    Cell::new(format_dollars(total_ba - total_resc))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::White),
                ]);
            }

            println!("{table}");
            println!();
            println!(
                "Budget Auth = sum of new_budget_authority provisions (computed from provisions, not LLM summary)"
            );
            println!("Rescissions = sum of rescission provisions (absolute value)");
            println!("Net BA = Budget Auth − Rescissions");

            let mut total_not_found = 0usize;
            let mut bills_with_not_found = 0usize;
            for loaded in &bills {
                let nf = loaded
                    .verification
                    .as_ref()
                    .map(|v| v.summary.amounts_not_found)
                    .unwrap_or(0);
                total_not_found += nf;
                if nf > 0 {
                    bills_with_not_found += 1;
                }
            }
            if total_not_found == 0 {
                println!(
                    "\n0 dollar amounts unverified across all bills. Run `congress-approp audit` for detailed verification."
                );
            } else {
                println!(
                    "\n{} dollar amounts not found in source text across {} bill(s). Run `congress-approp audit` for details.",
                    total_not_found, bills_with_not_found
                );
            }

            if by_agency {
                use congress_appropriations::approp::query;
                let rollups = query::rollup_by_department(&bills);
                if !rollups.is_empty() {
                    println!();
                    let mut agency_table = Table::new();
                    agency_table.load_preset(UTF8_FULL_CONDENSED);
                    agency_table.set_header(vec![
                        Cell::new("Department"),
                        Cell::new("Budget Auth ($)").set_alignment(CellAlignment::Right),
                        Cell::new("Rescissions ($)").set_alignment(CellAlignment::Right),
                        Cell::new("Provisions").set_alignment(CellAlignment::Right),
                    ]);
                    for r in &rollups {
                        agency_table.add_row(vec![
                            Cell::new(&r.department),
                            Cell::new(format_dollars(r.budget_authority))
                                .set_alignment(CellAlignment::Right),
                            Cell::new(format_dollars(r.rescissions))
                                .set_alignment(CellAlignment::Right),
                            Cell::new(r.provision_count.to_string())
                                .set_alignment(CellAlignment::Right),
                        ]);
                    }
                    println!("{agency_table}");
                }
            }
        }
    }

    Ok(())
}

// ─── Compare Handler ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn handle_compare(
    base_dir: Option<&str>,
    current_dir: Option<&str>,
    base_fy: Option<u32>,
    current_fy: Option<u32>,
    dir: Option<&str>,
    agency_filter: Option<&str>,
    subcommittee: Option<&str>,
    use_links: bool,
    real: bool,
    cpi_file: Option<&str>,
    format: &str,
    exact: bool,
) -> Result<()> {
    use congress_appropriations::approp::bill_meta::Jurisdiction;
    use congress_appropriations::approp::inflation;
    use congress_appropriations::approp::normalize;
    use congress_appropriations::approp::query;

    // Resolve which bills to compare: either --base/--current dirs or --base-fy/--current-fy
    let (base_bills, current_bills) = if let (Some(bfy), Some(cfy)) = (base_fy, current_fy) {
        let data_dir = dir.unwrap_or("./data");
        let all_bills = loading::load_bills(std::path::Path::new(data_dir))?;
        if all_bills.is_empty() {
            anyhow::bail!("No extracted bills found in directory: {data_dir}");
        }

        let base: Vec<_> = all_bills
            .iter()
            .filter(|b| b.extraction.bill.fiscal_years.contains(&bfy))
            .cloned()
            .collect();
        let current: Vec<_> = all_bills
            .iter()
            .filter(|b| b.extraction.bill.fiscal_years.contains(&cfy))
            .cloned()
            .collect();

        if base.is_empty() {
            anyhow::bail!("No bills found covering FY{bfy}");
        }
        if current.is_empty() {
            anyhow::bail!("No bills found covering FY{cfy}");
        }

        (base, current)
    } else if let (Some(bd), Some(cd)) = (base_dir, current_dir) {
        let base = loading::load_bills(std::path::Path::new(bd))?;
        let current = loading::load_bills(std::path::Path::new(cd))?;
        if base.is_empty() {
            anyhow::bail!("No extracted bills found in base directory: {bd}");
        }
        if current.is_empty() {
            anyhow::bail!("No extracted bills found in current directory: {cd}");
        }
        (base, current)
    } else {
        anyhow::bail!(
            "Provide either --base and --current directories, or --base-fy and --current-fy with --dir"
        );
    };

    // If --subcommittee is specified, filter provisions to matching divisions
    // by resolving jurisdiction → division letter per bill via bill_meta
    let (base_filtered, current_filtered) = if let Some(sub_slug) = subcommittee {
        let jurisdiction = Jurisdiction::from_slug(sub_slug).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown subcommittee: '{sub_slug}'. Valid slugs: defense, labor-hhs, thud, financial-services, cjs, energy-water, interior, agriculture, legislative-branch, milcon-va, state-foreign-ops, homeland-security"
            )
        })?;

        (
            filter_bills_to_subcommittee(&base_bills, &jurisdiction)?,
            filter_bills_to_subcommittee(&current_bills, &jurisdiction)?,
        )
    } else {
        (base_bills, current_bills)
    };

    // Load entity resolution rules from dataset.json (unless --exact)
    let dataset = if exact {
        None
    } else {
        let data_dir = dir.or(base_dir).unwrap_or("./data");
        normalize::load_dataset(std::path::Path::new(data_dir)).unwrap_or(None)
    };
    let agency_groups = dataset
        .as_ref()
        .map(|d| d.entities.agency_groups.as_slice())
        .unwrap_or(&[]);
    let account_aliases = dataset
        .as_ref()
        .map(|d| d.entities.account_aliases.as_slice())
        .unwrap_or(&[]);

    let mut result = query::compare(
        &base_filtered,
        &current_filtered,
        agency_filter,
        agency_groups,
        account_aliases,
    );

    // If --real, compute inflation-adjusted deltas
    let inflation_ctx = if real {
        let cpi_path = cpi_file.map(std::path::Path::new);
        let cpi_data = inflation::load_cpi(cpi_path)?;

        // Check staleness of bundled data
        if cpi_file.is_none()
            && let Some(warning) = inflation::check_staleness(&cpi_data)
        {
            eprintln!("⚠  {warning}");
        }

        // Determine fiscal years for the comparison
        let effective_base_fy = base_fy.or_else(|| {
            base_filtered
                .first()
                .and_then(|b| b.extraction.bill.fiscal_years.first().copied())
        });
        let effective_current_fy = current_fy.or_else(|| {
            current_filtered
                .first()
                .and_then(|b| b.extraction.bill.fiscal_years.first().copied())
        });

        if let (Some(bfy), Some(cfy)) = (effective_base_fy, effective_current_fy) {
            if let Some(ctx) = inflation::compute_inflation_context(&cpi_data, bfy, cfy) {
                // Apply inflation adjustment to each row
                for row in &mut result.rows {
                    if let Some(nominal_pct) = row.delta_pct {
                        let real_pct = inflation::real_delta_pct(nominal_pct, ctx.rate);
                        let flag = inflation::compute_flag(Some(nominal_pct), ctx.rate);
                        row.real_delta_pct = Some((real_pct * 10.0).round() / 10.0);
                        row.inflation_flag = Some(flag.slug().to_string());
                    } else if row.status == "only in base" || row.status == "only in current" {
                        row.inflation_flag = Some("n/a".to_string());
                    }
                }
                Some(ctx)
            } else {
                eprintln!(
                    "⚠  Could not compute inflation rate: CPI data not available for FY{bfy} or FY{cfy}"
                );
                None
            }
        } else {
            eprintln!("⚠  Could not determine fiscal years for inflation adjustment");
            None
        }
    } else {
        None
    };

    // If --use-links, load accepted links and rescue orphans that have a link
    // connecting them across bills (handles renames and reorganizations).
    if use_links {
        let data_dir = dir.unwrap_or("./data");
        if let Ok(Some(links_file)) =
            congress_appropriations::approp::links::load_links(std::path::Path::new(data_dir))
        {
            // Collect bill directories for each side of the comparison
            let base_dirs: std::collections::HashSet<String> = base_filtered
                .iter()
                .filter_map(|b| b.dir.file_name().map(|n| n.to_string_lossy().to_string()))
                .collect();
            let current_dirs: std::collections::HashSet<String> = current_filtered
                .iter()
                .filter_map(|b| b.dir.file_name().map(|n| n.to_string_lossy().to_string()))
                .collect();

            // For each "only in base" orphan, check if any provision in the base
            // bills has a link to a provision in the current bills
            // This is a best-effort match — we check link targets against current dirs
            let link_rescued: usize = result
                .rows
                .iter_mut()
                .filter(|r| r.status == "only in base" || r.status == "only in current")
                .filter_map(|r| {
                    // Try to find the provision in the link map
                    // We check by account name match in the link targets
                    let _is_base_orphan = r.status == "only in base";
                    for link in &links_file.accepted {
                        let src_name = link.source.label.to_lowercase();
                        let tgt_name = link.target.label.to_lowercase();
                        let row_name = r.account_name.to_lowercase();

                        if (src_name.contains(&row_name) || row_name.contains(&src_name))
                            && (base_dirs.contains(&link.source.bill_dir)
                                || current_dirs.contains(&link.source.bill_dir))
                            && (base_dirs.contains(&link.target.bill_dir)
                                || current_dirs.contains(&link.target.bill_dir))
                        {
                            r.status = format!("linked ({})", link.relationship);
                            return Some(());
                        }
                        if (tgt_name.contains(&row_name) || row_name.contains(&tgt_name))
                            && (base_dirs.contains(&link.source.bill_dir)
                                || current_dirs.contains(&link.source.bill_dir))
                            && (base_dirs.contains(&link.target.bill_dir)
                                || current_dirs.contains(&link.target.bill_dir))
                        {
                            r.status = format!("linked ({})", link.relationship);
                            return Some(());
                        }
                    }
                    None
                })
                .count();

            if link_rescued > 0 {
                eprintln!("  {link_rescued} orphan(s) rescued via accepted links");
            }
        } else {
            eprintln!("  hint: no links file found. Run `link suggest` then `link accept` first.");
        }
    }

    if let Some(ref warning) = result.cross_type_warning {
        eprintln!("⚠  {warning}");
        eprintln!();
    }

    match format {
        "json" => {
            if let Some(ref ctx) = inflation_ctx {
                #[derive(serde::Serialize)]
                struct InflationCompareOutput<'a> {
                    inflation: &'a inflation::InflationContext,
                    rows: &'a [query::CompareRow],
                    summary: InflationSummary,
                }
                #[derive(serde::Serialize)]
                struct InflationSummary {
                    beat_inflation: usize,
                    fell_behind: usize,
                    inflation_rate_pct: f64,
                }
                let beat = result
                    .rows
                    .iter()
                    .filter(|r| r.inflation_flag.as_deref() == Some("real_increase"))
                    .count();
                let behind = result
                    .rows
                    .iter()
                    .filter(|r| {
                        matches!(
                            r.inflation_flag.as_deref(),
                            Some("real_cut") | Some("inflation_erosion")
                        )
                    })
                    .count();
                let output = InflationCompareOutput {
                    inflation: ctx,
                    rows: &result.rows,
                    summary: InflationSummary {
                        beat_inflation: beat,
                        fell_behind: behind,
                        inflation_rate_pct: (ctx.rate * 1000.0).round() / 10.0,
                    },
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&result.rows)?);
            }
        }
        "csv" => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            if inflation_ctx.is_some() {
                wtr.write_record([
                    "agency",
                    "account_name",
                    "base_dollars",
                    "current_dollars",
                    "delta",
                    "delta_pct",
                    "status",
                    "normalized",
                    "real_delta_pct",
                    "inflation_flag",
                ])?;
            } else {
                wtr.write_record([
                    "agency",
                    "account_name",
                    "base_dollars",
                    "current_dollars",
                    "delta",
                    "delta_pct",
                    "status",
                    "normalized",
                ])?;
            }
            for d in &result.rows {
                if inflation_ctx.is_some() {
                    wtr.write_record([
                        &d.agency,
                        &d.account_name,
                        &d.base_dollars.to_string(),
                        &d.current_dollars.to_string(),
                        &d.delta.to_string(),
                        &d.delta_pct.map(|p| format!("{p:.1}")).unwrap_or_default(),
                        &d.status,
                        &if d.normalized {
                            "true".to_string()
                        } else {
                            "false".to_string()
                        },
                        &d.real_delta_pct
                            .map(|p| format!("{p:.1}"))
                            .unwrap_or_default(),
                        d.inflation_flag.as_deref().unwrap_or(""),
                    ])?;
                } else {
                    wtr.write_record([
                        &d.agency,
                        &d.account_name,
                        &d.base_dollars.to_string(),
                        &d.current_dollars.to_string(),
                        &d.delta.to_string(),
                        &d.delta_pct.map(|p| format!("{p:.1}")).unwrap_or_default(),
                        &d.status,
                        &if d.normalized {
                            "true".to_string()
                        } else {
                            "false".to_string()
                        },
                    ])?;
                }
            }
            wtr.flush()?;
        }
        _ => {
            println!(
                "Comparing: {}  →  {}",
                result.base_description, result.current_description
            );
            println!();

            if result.rows.is_empty() {
                println!("No matching appropriation accounts found.");
                return Ok(());
            }

            let mut table = Table::new();
            table.load_preset(UTF8_FULL_CONDENSED);
            if inflation_ctx.is_some() {
                table.set_header(vec![
                    Cell::new("Account"),
                    Cell::new("Agency"),
                    Cell::new("Base ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Current ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Delta ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Δ %").set_alignment(CellAlignment::Right),
                    Cell::new("Real Δ %*").set_alignment(CellAlignment::Right),
                    Cell::new(""),
                    Cell::new("Status"),
                ]);
            } else {
                table.set_header(vec![
                    Cell::new("Account"),
                    Cell::new("Agency"),
                    Cell::new("Base ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Current ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Delta ($)").set_alignment(CellAlignment::Right),
                    Cell::new("Δ %").set_alignment(CellAlignment::Right),
                    Cell::new("Status"),
                ]);
            }

            for d in &result.rows {
                let delta_color = if d.delta > 0 {
                    Color::Green
                } else if d.delta < 0 {
                    Color::Red
                } else {
                    Color::Reset
                };

                let pct_str = d
                    .delta_pct
                    .map(|p| format!("{p:+.1}%"))
                    .unwrap_or_else(|| "—".to_string());

                if inflation_ctx.is_some() {
                    let real_pct_str = d
                        .real_delta_pct
                        .map(|p| format!("{p:+.1}%"))
                        .unwrap_or_else(|| "—".to_string());
                    let flag_str = d
                        .inflation_flag
                        .as_deref()
                        .map(|f| match f {
                            "real_increase" => "▲",
                            "real_cut" | "inflation_erosion" => "▼",
                            _ => "—",
                        })
                        .unwrap_or("—");
                    let real_color = match d.inflation_flag.as_deref() {
                        Some("real_increase") => Color::Green,
                        Some("real_cut") | Some("inflation_erosion") => Color::Red,
                        _ => Color::Reset,
                    };
                    table.add_row(vec![
                        Cell::new(truncate(&d.account_name, 35)),
                        Cell::new(truncate(&d.agency, 20)),
                        Cell::new(format_dollars(d.base_dollars))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(d.current_dollars))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars_signed(d.delta))
                            .set_alignment(CellAlignment::Right)
                            .fg(delta_color),
                        Cell::new(&pct_str)
                            .set_alignment(CellAlignment::Right)
                            .fg(delta_color),
                        Cell::new(&real_pct_str)
                            .set_alignment(CellAlignment::Right)
                            .fg(real_color),
                        Cell::new(flag_str).fg(real_color),
                        Cell::new(if d.normalized {
                            format!("{} (normalized)", d.status)
                        } else {
                            d.status.clone()
                        }),
                    ]);
                } else {
                    table.add_row(vec![
                        Cell::new(truncate(&d.account_name, 35)),
                        Cell::new(truncate(&d.agency, 20)),
                        Cell::new(format_dollars(d.base_dollars))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars(d.current_dollars))
                            .set_alignment(CellAlignment::Right),
                        Cell::new(format_dollars_signed(d.delta))
                            .set_alignment(CellAlignment::Right)
                            .fg(delta_color),
                        Cell::new(&pct_str)
                            .set_alignment(CellAlignment::Right)
                            .fg(delta_color),
                        Cell::new(if d.normalized {
                            format!("{} (normalized)", d.status)
                        } else {
                            d.status.clone()
                        }),
                    ]);
                }
            }

            println!("{table}");

            // Inflation footer
            if let Some(ref ctx) = inflation_ctx {
                let beat = result
                    .rows
                    .iter()
                    .filter(|r| r.inflation_flag.as_deref() == Some("real_increase"))
                    .count();
                let behind = result
                    .rows
                    .iter()
                    .filter(|r| {
                        matches!(
                            r.inflation_flag.as_deref(),
                            Some("real_cut") | Some("inflation_erosion")
                        )
                    })
                    .count();
                println!(
                    "{beat} beat inflation, {behind} fell behind | {} FY{}→FY{}: {:.1}% ({})",
                    ctx.source.split(',').next().unwrap_or(&ctx.source),
                    ctx.base_fy,
                    ctx.current_fy,
                    ctx.rate * 100.0,
                    ctx.note
                );
                println!(
                    "* Real Δ % is computed from an external price index, not verified against bill text."
                );
                println!();
            }

            println!(
                "{} accounts compared ({} changed, {} only in current, {} only in base, {} unchanged)",
                result.rows.len(),
                result.rows.iter().filter(|d| d.status == "changed").count(),
                result
                    .rows
                    .iter()
                    .filter(|d| d.status == "only in current")
                    .count(),
                result
                    .rows
                    .iter()
                    .filter(|d| d.status == "only in base")
                    .count(),
                result
                    .rows
                    .iter()
                    .filter(|d| d.status == "unchanged")
                    .count(),
            );

            // Orphan-pair hint: suggest normalize when unresolved orphans exist
            let orphan_count = result
                .rows
                .iter()
                .filter(|d| d.status == "only in base" || d.status == "only in current")
                .count();
            if orphan_count > 0 {
                let data_dir = dir.or(base_dir).unwrap_or("./data");
                let has_dataset = std::path::Path::new(data_dir).join("dataset.json").exists();
                if has_dataset {
                    eprintln!(
                        "\n{orphan_count} unresolved orphan pairs remain. Run `normalize suggest-text-match --dir {data_dir}` to discover more agency naming variants."
                    );
                } else {
                    eprintln!(
                        "\n{orphan_count} orphan pairs detected. Run `normalize suggest-text-match --dir {data_dir}` to discover agency naming variants."
                    );
                }
            }
        }
    }

    Ok(())
}

// ─── Report Handler ──────────────────────────────────────────────────────────

fn handle_audit(dir: &str, verbose: bool) -> Result<()> {
    let dir_path = std::path::Path::new(dir);
    let bills = loading::load_bills(dir_path)?;

    if bills.is_empty() {
        println!("No extracted bills found in {dir}");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec![
        Cell::new("Bill"),
        Cell::new("Provisions").set_alignment(CellAlignment::Right),
        Cell::new("Verified").set_alignment(CellAlignment::Right),
        Cell::new("NotFound").set_alignment(CellAlignment::Right),
        Cell::new("Ambig").set_alignment(CellAlignment::Right),
        Cell::new("Exact").set_alignment(CellAlignment::Right),
        Cell::new("NormText").set_alignment(CellAlignment::Right),
        Cell::new("Spaceless").set_alignment(CellAlignment::Right),
        Cell::new("TextMiss").set_alignment(CellAlignment::Right),
        Cell::new("Coverage").set_alignment(CellAlignment::Right),
    ]);

    let mut total_provs = 0usize;
    let mut total_verified = 0usize;
    let mut total_not_found = 0usize;
    let mut total_ambiguous = 0usize;
    let mut total_exact = 0usize;
    let mut total_normalized = 0usize;
    let mut total_spaceless = 0usize;
    let mut total_no_match = 0usize;

    for loaded in &bills {
        let bill_id = &loaded.extraction.bill.identifier;
        let provs = loaded.extraction.provisions.len();
        total_provs += provs;

        if let Some(ref ver) = loaded.verification {
            let s = &ver.summary;
            total_verified += s.amounts_verified;
            total_not_found += s.amounts_not_found;
            total_ambiguous += s.amounts_ambiguous;
            total_exact += s.raw_text_exact;
            total_normalized += s.raw_text_normalized;
            total_spaceless += s.raw_text_spaceless;
            total_no_match += s.raw_text_no_match;

            let not_found_color = if s.amounts_not_found > 0 {
                Color::Red
            } else {
                Color::Green
            };
            let no_match_color = if s.raw_text_no_match > 0 {
                Color::Yellow
            } else {
                Color::Green
            };
            let completeness_color = if s.completeness_pct >= 90.0 {
                Color::Green
            } else if s.completeness_pct >= 50.0 {
                Color::Yellow
            } else {
                Color::Red
            };

            table.add_row(vec![
                Cell::new(bill_id),
                Cell::new(provs).set_alignment(CellAlignment::Right),
                Cell::new(s.amounts_verified)
                    .set_alignment(CellAlignment::Right)
                    .fg(Color::Green),
                Cell::new(s.amounts_not_found)
                    .set_alignment(CellAlignment::Right)
                    .fg(not_found_color),
                Cell::new(s.amounts_ambiguous).set_alignment(CellAlignment::Right),
                Cell::new(s.raw_text_exact).set_alignment(CellAlignment::Right),
                Cell::new(s.raw_text_normalized).set_alignment(CellAlignment::Right),
                Cell::new(s.raw_text_spaceless).set_alignment(CellAlignment::Right),
                Cell::new(s.raw_text_no_match)
                    .set_alignment(CellAlignment::Right)
                    .fg(no_match_color),
                Cell::new(format!("{:.1}%", s.completeness_pct))
                    .set_alignment(CellAlignment::Right)
                    .fg(completeness_color),
            ]);

            // Verbose: show individual problems
            if verbose {
                for check in &ver.amount_checks {
                    if matches!(check.status, CheckResult::NotFound) {
                        println!(
                            "  ✗ {bill_id} provision[{}]: amount {} NOT FOUND in source",
                            check.provision_index, check.text_as_written
                        );
                    }
                }
                for check in &ver.raw_text_checks {
                    if matches!(check.match_tier, MatchTier::NoMatch) {
                        println!(
                            "  ~ {bill_id} provision[{}]: raw_text NO MATCH: {}",
                            check.provision_index, check.raw_text_preview
                        );
                    }
                }
            }
        } else {
            table.add_row(vec![
                Cell::new(bill_id),
                Cell::new(provs).set_alignment(CellAlignment::Right),
                Cell::new("—"),
                Cell::new("—"),
                Cell::new("—"),
                Cell::new("—"),
                Cell::new("—"),
                Cell::new("—"),
                Cell::new("—"),
                Cell::new("no verification"),
            ]);
        }
    }

    // Totals
    table.add_row(vec![
        Cell::new("TOTAL").fg(Color::White),
        Cell::new(total_provs)
            .set_alignment(CellAlignment::Right)
            .fg(Color::White),
        Cell::new(total_verified)
            .set_alignment(CellAlignment::Right)
            .fg(Color::White),
        Cell::new(total_not_found)
            .set_alignment(CellAlignment::Right)
            .fg(Color::White),
        Cell::new(total_ambiguous)
            .set_alignment(CellAlignment::Right)
            .fg(Color::White),
        Cell::new(total_exact)
            .set_alignment(CellAlignment::Right)
            .fg(Color::White),
        Cell::new(total_normalized)
            .set_alignment(CellAlignment::Right)
            .fg(Color::White),
        Cell::new(total_spaceless)
            .set_alignment(CellAlignment::Right)
            .fg(Color::White),
        Cell::new(total_no_match)
            .set_alignment(CellAlignment::Right)
            .fg(Color::White),
        Cell::new("").fg(Color::White),
    ]);

    println!("{table}");
    println!();
    println!("Column Guide:");
    println!("  Verified   Dollar amount string found at exactly one position in source text");
    println!(
        "  NotFound   Dollar amounts NOT found in source — not present in source, review manually"
    );
    println!(
        "  Ambig      Dollar amounts found multiple times in source — correct but position uncertain"
    );
    println!("  Exact      raw_text is byte-identical substring of source — verbatim copy");
    println!(
        "  NormText  raw_text matches after whitespace/quote/dash normalization — content correct"
    );
    println!("  Spaceless raw_text matches only after removing all spaces — PDF artifact, review");
    println!("  TextMiss raw_text not found at any tier — may be paraphrased, review manually");
    println!("  Coverage  Percentage of dollar strings in source text matched to a provision");
    println!();
    println!("Key:");
    println!("  NotFound = 0 and Coverage = 100%   →  All amounts captured and found in source");
    println!(
        "  NotFound = 0 and Coverage < 100%   →  Extracted amounts correct, but bill has more"
    );
    println!("  NotFound > 0                       →  Some amounts need manual review");

    Ok(())
}

// ─── Upgrade Handler ─────────────────────────────────────────────────────────

fn handle_upgrade(dir: &str, dry_run: bool) -> Result<()> {
    use congress_appropriations::approp::text_index::{TextIndex, build_text_index};
    use congress_appropriations::approp::verification;
    use congress_appropriations::approp::xml;
    use sha2::{Digest, Sha256};

    let dir_path = std::path::Path::new(dir);

    // Find all extraction.json files
    let mut ext_files = Vec::new();
    for entry in walkdir::WalkDir::new(dir_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() == "extraction.json" {
            ext_files.push(entry.into_path());
        }
    }
    ext_files.sort();

    if ext_files.is_empty() {
        println!("No extraction.json files found in {dir}");
        return Ok(());
    }

    println!("Found {} bill(s) to check", ext_files.len());
    println!();

    for ext_path in &ext_files {
        let bill_dir = ext_path.parent().unwrap_or(std::path::Path::new("."));
        let bill_name = bill_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Load raw JSON for patching
        let ext_text = std::fs::read_to_string(ext_path)?;
        let mut ext_json: serde_json::Value = serde_json::from_str(&ext_text)?;

        let current_version = ext_json
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or("0");

        if current_version == "1.0" {
            println!("{bill_name}: already at schema v1.0, skipping");
            continue;
        }

        println!("Upgrading {bill_name}...");
        println!("  Schema: {current_version} → 1.0");

        // Apply v0 → v1.0 migrations
        let mut fixed_count = 0usize;
        ext_json["schema_version"] = serde_json::Value::String("1.0".to_string());

        if let Some(provisions) = ext_json["provisions"].as_array_mut() {
            for prov in provisions.iter_mut() {
                for field in &["amount", "new_amount", "old_amount"] {
                    if let Some(amount) = prov.get_mut(*field)
                        && fix_such_sums_amount(amount)
                    {
                        fixed_count += 1;
                    }
                }
                if let Some(amounts) = prov.get_mut("amounts")
                    && let Some(arr) = amounts.as_array_mut()
                {
                    for amt in arr.iter_mut() {
                        if fix_such_sums_amount(amt) {
                            fixed_count += 1;
                        }
                    }
                }
            }
        }

        println!("  Migrated: {fixed_count} provisions fixed");

        if dry_run {
            println!("  [DRY RUN] Would write extraction.json and re-verify");
            println!();
            continue;
        }

        // Write patched extraction.json
        std::fs::write(ext_path, serde_json::to_string_pretty(&ext_json)?)?;

        // Re-verify against source XML
        let xml_files: Vec<_> = std::fs::read_dir(bill_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let p = e.path();
                p.extension().is_some_and(|x| x == "xml")
                    && p.file_stem()
                        .is_some_and(|n| n.to_string_lossy().starts_with("BILLS-"))
            })
            .map(|e| e.path())
            .collect();

        if let Some(xml_path) = xml_files.first() {
            let parsed = xml::parse_bill_xml(xml_path, 3000)?;
            let index = build_text_index(&parsed.full_text);

            // Load the patched extraction via serde
            let ext_text = std::fs::read_to_string(ext_path)?;
            let extraction: congress_appropriations::approp::ontology::BillExtraction =
                serde_json::from_str(&ext_text)?;

            let mut report =
                verification::verify_provisions(&extraction.provisions, &parsed.full_text, &index);
            report.schema_version = Some("1.0".to_string());

            let ver_path = bill_dir.join("verification.json");
            std::fs::write(&ver_path, serde_json::to_string_pretty(&report)?)?;

            println!(
                "  Re-verified: {} provisions, {} not_found, {:.1}% coverage",
                report.summary.total_provisions,
                report.summary.amounts_not_found,
                report.summary.completeness_pct
            );

            // Update metadata.json
            let text_hash = TextIndex::text_hash(&parsed.full_text);
            let xml_bytes = std::fs::read(xml_path)?;
            let source_xml_sha256 = format!("{:x}", Sha256::digest(&xml_bytes));
            let meta_path = bill_dir.join("metadata.json");
            let metadata = serde_json::json!({
                "extraction_version": env!("CARGO_PKG_VERSION"),
                "prompt_version": "v3",
                "model": "claude-opus-4-6",
                "schema_version": "1.0",
                "source_pdf_sha256": null,
                "source_xml_sha256": source_xml_sha256,
                "extracted_text_sha256": text_hash,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            std::fs::write(&meta_path, serde_json::to_string_pretty(&metadata)?)?;

            println!("  Updated: extraction.json, verification.json, metadata.json");
        } else {
            println!("  WARNING: No source XML found, skipping re-verification");
        }
        println!();
    }

    println!("Upgrade complete.");
    Ok(())
}

/// Fix a dollar amount object: if kind=specific, dollars=0, semantics=missing → SuchSums + indefinite
fn fix_such_sums_amount(amount: &mut serde_json::Value) -> bool {
    if !amount.is_object() {
        return false;
    }

    let semantics_is_missing = amount
        .get("semantics")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s == "missing");

    if !semantics_is_missing {
        return false;
    }

    let value_obj = amount.get("value");
    let kind_is_specific = value_obj
        .and_then(|v| v.get("kind"))
        .and_then(|v| v.as_str())
        .is_some_and(|s| s == "specific");
    let dollars_is_zero = value_obj
        .and_then(|v| v.get("dollars"))
        .and_then(|v| v.as_i64())
        .is_some_and(|d| d == 0);
    let text_is_empty = amount
        .get("text_as_written")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.is_empty());

    if kind_is_specific && dollars_is_zero && text_is_empty {
        amount["value"] = serde_json::json!({"kind": "such_sums"});
    }

    amount["semantics"] = serde_json::Value::String("indefinite".to_string());
    true
}

// ─── Download Handler ────────────────────────────────────────────────────────

struct DownloadOptions<'a> {
    congress: u32,
    bill_type: Option<&'a str>,
    bill_number: Option<u32>,
    output_dir: &'a str,
    enacted_only: bool,
    format: &'a str,
    version_filter: Option<&'a str>,
    all_versions: bool,
    dry_run: bool,
}

async fn handle_download(opts: DownloadOptions<'_>) -> Result<()> {
    let total_start = Instant::now();

    let client = CongressClient::from_env()
        .context("Set CONGRESS_API_KEY — free key at https://api.congress.gov/sign-up/")?;
    let c = Congress::new(opts.congress).map_err(|e| anyhow::anyhow!("{e}"))?;

    let formats: Vec<&str> = opts.format.split(',').map(|s| s.trim()).collect();
    let versions: Option<Vec<&str>> = if let Some(v) = opts.version_filter {
        // Explicit --version flag: use exactly what the user specified
        Some(v.split(',').map(|s| s.trim()).collect())
    } else if opts.all_versions {
        // --all-versions: no filter, download everything
        None
    } else {
        // Default: enrolled only
        Some(vec!["Enrolled"])
    };
    let output_dir = opts.output_dir;
    let dry_run = opts.dry_run;
    let enacted_only = opts.enacted_only;

    // Single-bill download: skip the scan and download one bill directly
    if let (Some(bt_str), Some(num)) = (opts.bill_type, opts.bill_number) {
        let bt: BillType = bt_str
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid bill type: {bt_str}"))?;
        let id = BillId::new(c, bt, num);

        tracing::info!("═══════════════════════════════════════════════════════");
        tracing::info!("Downloading {id}");
        tracing::info!("═══════════════════════════════════════════════════════");

        let tvs = client.get_bill_text(&id).await?;
        let mut downloaded = 0u32;

        for tv in &tvs {
            let ver_name = tv.r#type.as_deref().unwrap_or("unknown");
            if let Some(ref allowed) = versions {
                let ver_lower = ver_name.to_lowercase();
                if !allowed
                    .iter()
                    .any(|a| ver_lower.contains(&a.to_lowercase()))
                {
                    continue;
                }
            }
            for fmt in &tv.formats {
                let fmt_type = fmt.r#type.as_deref().unwrap_or("").to_lowercase();
                if !formats.iter().any(|f| fmt_type.contains(*f)) {
                    continue;
                }
                let filename = fmt.url.split('/').next_back().unwrap_or("file");
                let dir = format!("{}/{}-{}{}", output_dir, c.number(), bt.api_slug(), num);
                std::fs::create_dir_all(&dir)?;
                let out_path = format!("{dir}/{filename}");

                if std::path::Path::new(&out_path).exists() {
                    tracing::info!("  Already exists: {filename}");
                    continue;
                }

                if dry_run {
                    tracing::info!("  [DRY RUN] Would download: {filename} ({ver_name})");
                    continue;
                }

                tracing::info!("  Downloading {filename} ({ver_name})...");
                let http = reqwest::Client::builder()
                    .user_agent("congress-approp/1.0.0")
                    .timeout(std::time::Duration::from_secs(60))
                    .build()?;
                let resp = http.get(&fmt.url).send().await?;
                if resp.status().is_success() {
                    let bytes = resp.bytes().await?;
                    std::fs::write(&out_path, &bytes)?;
                    tracing::info!("  ✓ {} ({})", filename, human_bytes(bytes.len()));
                    downloaded += 1;
                } else {
                    tracing::warn!("  ✗ HTTP {}", resp.status());
                }
            }
        }

        let elapsed = total_start.elapsed();
        tracing::info!("Download complete: {downloaded} files [{elapsed:.1?}]");
        tracing::info!(
            "  Output: {output_dir}/{}-{}{}",
            c.number(),
            bt.api_slug(),
            num
        );
        return Ok(());
    }

    // Validate: if only one of --type/--number given, error
    if opts.bill_type.is_some() || opts.bill_number.is_some() {
        anyhow::bail!("Both --type and --number are required for single-bill download");
    }

    tracing::info!("═══════════════════════════════════════════════════════");
    tracing::info!("Scanning {} for appropriations bills", c);
    tracing::info!(
        "  Filters: enacted_only={enacted_only} formats={}",
        opts.format
    );
    if let Some(ref v) = versions {
        tracing::info!("  Version filter: {}", v.join(", "));
    }
    if dry_run {
        tracing::info!("  *** DRY RUN — nothing will be downloaded ***");
    }
    tracing::info!("═══════════════════════════════════════════════════════");

    // ── Phase 1: Scan for matching bills
    tracing::info!("");
    tracing::info!("Phase 1: Scanning bill lists...");

    let bill_types = [BillType::Hr, BillType::S];
    let mut matched_bills: Vec<(BillId, String)> = Vec::new();
    let mut total_scanned = 0u32;
    let mut total_title_matches = 0u32;
    let mut total_skipped_not_enacted = 0u32;

    for bt in &bill_types {
        let mut offset = 0u32;
        let mut page = 0u32;
        let type_start = Instant::now();

        tracing::info!("  Scanning {} bills...", bt.label());

        loop {
            page += 1;
            let page_start = Instant::now();
            let response = client.list_bills(c, *bt, offset, 250).await?;
            let page_count = response.bills.len();
            let page_elapsed = page_start.elapsed();

            if page_count == 0 {
                tracing::info!("    Page {page}: empty — done with {} bills", bt.label());
                break;
            }

            total_scanned += page_count as u32;

            let mut page_matches = 0u32;
            let mut page_skipped = 0u32;

            for bill_item in &response.bills {
                let title = &bill_item.title;

                if !title_matches_appropriations(title) {
                    continue;
                }
                total_title_matches += 1;

                if enacted_only && !is_enacted(bill_item) {
                    total_skipped_not_enacted += 1;
                    page_skipped += 1;
                    tracing::debug!(
                        "    Skip (not enacted): {} {} - {}",
                        bt.label(),
                        bill_item.number,
                        &title[..title.len().min(60)]
                    );
                    continue;
                }

                let id = BillId::new(c, *bt, bill_item.number);
                let action = bill_item
                    .latest_action
                    .as_ref()
                    .and_then(|la| la.text.as_deref())
                    .unwrap_or("(no action)");

                tracing::info!("    ✓ Match: {} - {}", id, &title[..title.len().min(70)]);
                tracing::debug!("      Latest action: {}", &action[..action.len().min(80)]);

                matched_bills.push((id, title.clone()));
                page_matches += 1;
            }

            tracing::info!(
                "    Page {page}: {page_count} bills scanned, {page_matches} matched, {page_skipped} skipped (not enacted) [{page_elapsed:.1?}]",
            );

            offset += 250;
            if response.pagination.next.is_none() || page_count < 250 {
                break;
            }
        }

        let type_elapsed = type_start.elapsed();
        tracing::info!("  Done scanning {} bills [{type_elapsed:.1?}]", bt.label());
    }

    tracing::info!("");
    tracing::info!("Phase 1 summary:");
    tracing::info!("  Total bills scanned:       {total_scanned}");
    tracing::info!("  Title keyword matches:     {total_title_matches}");
    tracing::info!("  Skipped (not enacted):     {total_skipped_not_enacted}");
    tracing::info!("  Bills to process:          {}", matched_bills.len());

    if matched_bills.is_empty() {
        tracing::warn!("No matching bills found. Nothing to download.");
        return Ok(());
    }

    // ── Phase 2: Fetch text versions
    tracing::info!("");
    tracing::info!(
        "Phase 2: Fetching text versions for {} bills...",
        matched_bills.len()
    );

    struct DownloadItem {
        id: BillId,
        version_name: String,
        format_type: String,
        url: String,
    }

    let mut download_queue: Vec<DownloadItem> = Vec::new();

    for (i, (id, _title)) in matched_bills.iter().enumerate() {
        let bill_num = i + 1;
        let bill_total = matched_bills.len();

        tracing::info!("  [{bill_num}/{bill_total}] Fetching text versions for {id}...");

        match client.get_bill_text(id).await {
            Ok(tvs) => {
                let version_count = tvs.len();
                let mut added = 0u32;
                let mut filtered_out = 0u32;

                for tv in &tvs {
                    let ver_name = tv.r#type.as_deref().unwrap_or("unknown");

                    if let Some(ref allowed) = versions {
                        let ver_lower = ver_name.to_lowercase();
                        if !allowed
                            .iter()
                            .any(|a| ver_lower.contains(&a.to_lowercase()))
                        {
                            filtered_out += 1;
                            tracing::debug!("    Skip version '{ver_name}' (not in filter)");
                            continue;
                        }
                    }

                    for fmt in &tv.formats {
                        let fmt_type = fmt.r#type.as_deref().unwrap_or("").to_lowercase();
                        if formats.iter().any(|f| fmt_type.contains(*f)) {
                            download_queue.push(DownloadItem {
                                id: id.clone(),
                                version_name: ver_name.to_string(),
                                format_type: fmt.r#type.as_deref().unwrap_or("?").to_string(),
                                url: fmt.url.clone(),
                            });
                            added += 1;
                        }
                    }
                }

                tracing::info!(
                    "    {version_count} versions available, {added} files queued, {filtered_out} filtered out"
                );
            }
            Err(e) => {
                tracing::warn!("    ✗ Failed to get text versions: {e}");
            }
        }
    }

    tracing::info!("");
    tracing::info!("Phase 2 summary:");
    tracing::info!("  Files to download: {}", download_queue.len());

    if download_queue.is_empty() {
        tracing::warn!("No files to download after filtering.");
        return Ok(());
    }

    // ── Phase 3: Download
    if dry_run {
        tracing::info!("");
        tracing::info!("Phase 3: DRY RUN — listing what would be downloaded:");
        for (i, item) in download_queue.iter().enumerate() {
            println!(
                "  [{:>3}/{}] {} | {} | {} → {}",
                i + 1,
                download_queue.len(),
                item.id,
                item.version_name,
                item.format_type,
                item.url
            );
        }
        tracing::info!("");
        tracing::info!(
            "Would download {} files. Run without --dry-run to fetch them.",
            download_queue.len()
        );
        return Ok(());
    }

    tracing::info!("");
    tracing::info!("Phase 3: Downloading {} files...", download_queue.len());

    let http = reqwest::Client::builder()
        .user_agent("congress-approp/0.1.0")
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let mut downloaded = 0u32;
    let mut skipped_existing = 0u32;
    let mut errors = 0u32;
    let mut total_bytes = 0usize;
    let download_start = Instant::now();

    for (i, item) in download_queue.iter().enumerate() {
        let file_num = i + 1;
        let file_total = download_queue.len();
        let filename = item.url.split('/').next_back().unwrap_or("file");

        let dir = format!(
            "{}/{}/{}/{}",
            output_dir,
            c.number(),
            item.id.bill_type.api_slug(),
            item.id.number
        );
        std::fs::create_dir_all(&dir)?;
        let out_path = format!("{dir}/{filename}");

        if std::path::Path::new(&out_path).exists() {
            let size = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
            tracing::debug!(
                "  [{file_num}/{file_total}] Skip (exists, {}): {filename}",
                human_bytes(size as usize)
            );
            skipped_existing += 1;
            continue;
        }

        let elapsed = download_start.elapsed().as_secs_f64();
        let rate = if downloaded > 0 {
            downloaded as f64 / elapsed
        } else {
            0.0
        };
        let remaining = file_total as u32 - file_num as u32;
        let eta_secs = if rate > 0.0 {
            remaining as f64 / rate
        } else {
            0.0
        };

        tracing::info!(
            "  [{file_num}/{file_total}] {filename} ({}) [ETA: {:.0}s]",
            item.version_name,
            eta_secs
        );

        let dl_start = Instant::now();
        match http.get(&item.url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let bytes = resp.bytes().await?;
                let size = bytes.len();
                total_bytes += size;
                std::fs::write(&out_path, &bytes)?;
                let dl_elapsed = dl_start.elapsed();
                tracing::info!("    ✓ {} [{dl_elapsed:.1?}]", human_bytes(size));
                downloaded += 1;
            }
            Ok(resp) => {
                let status = resp.status();
                tracing::warn!("    ✗ HTTP {status}");
                errors += 1;
            }
            Err(e) => {
                tracing::warn!("    ✗ {e}");
                errors += 1;
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // ── Summary
    let total_elapsed = total_start.elapsed();
    tracing::info!("");
    tracing::info!("═══════════════════════════════════════════════════════");
    tracing::info!("Download complete [{total_elapsed:.1?}]");
    tracing::info!(
        "  Downloaded:      {downloaded} files ({})",
        human_bytes(total_bytes)
    );
    tracing::info!("  Already existed: {skipped_existing}");
    tracing::info!("  Errors:          {errors}");
    tracing::info!("  Output dir:      {output_dir}");
    tracing::info!("═══════════════════════════════════════════════════════");

    Ok(())
}

// ─── Utility Functions ───────────────────────────────────────────────────────

/// Check if a bill item looks like it was enacted.
fn is_enacted(item: &BillListItem) -> bool {
    item.latest_action
        .as_ref()
        .and_then(|la| la.text.as_ref())
        .map(|t| {
            let low = t.to_lowercase();
            low.contains("became public law") || low.contains("became law")
        })
        .unwrap_or(false)
}

/// Check if a bill title matches appropriations keywords.
fn title_matches_appropriations(title: &str) -> bool {
    let low = title.to_lowercase();
    low.contains("appropriation")
        || low.contains("continuing resolution")
        || low.contains("omnibus")
}

/// Format a bill identifier with congress number for display.
/// "H.R. 7148" + Some(119) → "H.R. 7148 (119th)"
/// "H.R. 7148" + None → "H.R. 7148"
fn format_bill_id(identifier: &str, congress: Option<u32>) -> String {
    match congress {
        Some(c) => format!("{identifier} ({c}th)"),
        None => identifier.to_string(),
    }
}

/// Human-readable byte size.
fn human_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    }
}

/// Format an integer as comma-separated dollars.
fn format_dollars(n: i64) -> String {
    let abs = n.unsigned_abs();
    let s = abs.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    let formatted: String = result.chars().rev().collect();
    if n < 0 {
        format!("-{formatted}")
    } else {
        formatted
    }
}

/// Format a signed dollar amount with +/- prefix.
fn format_dollars_signed(n: i64) -> String {
    if n > 0 {
        format!("+{}", format_dollars(n))
    } else if n < 0 {
        format_dollars(n)
    } else {
        "0".to_string()
    }
}

/// Truncate a string to max_len, appending "…" if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut end = max_len.saturating_sub(1);
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

/// Get (dollars, semantics_str) from a provision for display purposes.
fn prov_amount_strs(p: &Provision) -> (Option<i64>, &str) {
    match p.amount() {
        Some(amt) => {
            let sem = match amt.semantics {
                AmountSemantics::NewBudgetAuthority => "new_budget_authority",
                AmountSemantics::TransferCeiling => "transfer_ceiling",
                AmountSemantics::Rescission => "rescission",
                AmountSemantics::Limitation => "limitation",
                AmountSemantics::ReferenceAmount => "reference_amount",
                AmountSemantics::MandatorySpending => "mandatory_spending",
                AmountSemantics::Other(_) | _ => "other",
            };
            (amt.dollars(), sem)
        }
        None => (None, ""),
    }
}

/// Lookup from (bill_identifier, provision_index) to (verified, match_tier).
type VerificationLookup<'a> = HashMap<(&'a str, usize), (Option<String>, Option<&'a str>)>;

/// Build a lookup of verification status by (bill_identifier, provision_index).
fn build_verification_lookup(bills: &[LoadedBill]) -> VerificationLookup<'_> {
    let mut lookup = HashMap::new();
    for loaded in bills {
        let bill_id = loaded.extraction.bill.identifier.as_str();
        if let Some(ref ver) = loaded.verification {
            let mut amount_status: HashMap<usize, &str> = HashMap::new();
            for check in &ver.amount_checks {
                let status_str = match check.status {
                    CheckResult::Verified => "found",
                    CheckResult::Ambiguous => "found_multiple",
                    CheckResult::NotFound => "not_found",
                    _ => continue,
                };
                amount_status.insert(check.provision_index, status_str);
            }

            let mut tier_status: HashMap<usize, &str> = HashMap::new();
            for check in &ver.raw_text_checks {
                tier_status.insert(
                    check.provision_index,
                    match check.match_tier {
                        MatchTier::Exact => "exact",
                        MatchTier::Normalized => "normalized",
                        MatchTier::Spaceless => "spaceless",
                        MatchTier::NoMatch => "no_match",
                    },
                );
            }

            for i in 0..loaded.extraction.provisions.len() {
                let verified = amount_status.get(&i).map(|s| s.to_string());
                let tier = tier_status.get(&i).copied();
                lookup.insert((bill_id, i), (verified, tier));
            }
        }
    }
    lookup
}
