# Code Map

A file-by-file guide to the codebase — where each module lives, what it does, how many lines it contains, and when you'd need to edit it.

## Source Layout

```text
src/
├── main.rs                          ← CLI entry point (~4,200 lines)
├── lib.rs                           ← Library re-exports (5 lines)
├── api/
│   ├── mod.rs                       ← pub mod anthropic; pub mod congress; pub mod openai;
│   ├── anthropic/
│   │   ├── mod.rs                   ← Re-exports
│   │   └── client.rs               ← Claude API client (~340 lines)
│   ├── congress/
│   │   ├── mod.rs                   ← Types and re-exports
│   │   ├── client.rs               ← Congress.gov HTTP client
│   │   └── bill.rs                 ← Bill listing, metadata, text versions
│   └── openai/
│       ├── mod.rs                   ← Re-exports
│       └── client.rs               ← Embeddings endpoint (~45 lines)
└── approp/
    ├── mod.rs                       ← pub mod for all submodules
    ├── ontology.rs                  ← All data types (~960 lines)
    ├── bill_meta.rs                 ← Bill metadata + classification (~1,280 lines)
    ├── extraction.rs                ← Extraction pipeline (~840 lines)
    ├── from_value.rs                ← Resilient JSON parsing (~690 lines)
    ├── xml.rs                       ← Congressional XML parser (~590 lines)
    ├── text_index.rs                ← Dollar amount indexing (~670 lines)
    ├── prompts.rs                   ← LLM system prompt (~310 lines)
    ├── verification.rs              ← Deterministic verification (~370 lines)
    ├── links.rs                     ← Cross-bill link persistence (~790 lines)
    ├── loading.rs                   ← Directory walking, bill loading (~340 lines)
    ├── query.rs                     ← Library API (~1,300 lines)
    ├── embeddings.rs                ← Embedding storage (~260 lines)
    ├── staleness.rs                 ← Hash chain checking incl bill_meta (~165 lines)
    └── progress.rs                  ← Extraction progress bar (~170 lines)
```

## Supporting Files

```text
tests/
└── cli_tests.rs                     ← 42 integration tests (~1,200 lines)

docs/
├── ARCHITECTURE.md                  ← Architecture doc (~416 lines)
└── FIELD_REFERENCE.md               ← JSON field reference (~348 lines)

book/
└── src/                             ← This mdbook documentation

data/
├── hr4366/                          ← FY2024 omnibus (2,364 provisions)
├── hr5860/                          ← FY2024 continuing resolution (130 provisions)
└── hr9468/                          ← VA supplemental (7 provisions)
```

## File-by-File Reference

### Core: CLI and Library Entry Points

| File | Lines | Purpose | When to Edit |
|------|-------|---------|-------------|
| `src/main.rs` | ~4,200 | CLI entry point. Clap argument definitions, command handlers, output formatting (table/JSON/CSV/JSONL). Contains handlers for all commands: `handle_search`, `handle_summary`, `handle_compare`, `handle_audit`, `handle_extract`, `handle_embed`, `handle_download`, `handle_upgrade`, `handle_enrich`, `handle_relate`, `handle_link`, and helper functions including `filter_bills_to_subcommittee`. | Adding new CLI commands or flags; changing output formatting; wiring new library functions to the CLI. |
| `src/lib.rs` | 5 | Library re-exports: `pub mod api; pub mod approp;` plus `pub use approp::loading::{LoadedBill, load_bills}; pub use approp::query;` | Adding new top-level re-exports for library consumers. |

### Core: Data Types

| File | Lines | Purpose | When to Edit |
|------|-------|---------|-------------|
| `src/approp/ontology.rs` | ~960 | **All data types.** The `Provision` enum (11 variants), `BillExtraction`, `BillInfo`, `DollarAmount`, `AmountValue`, `AmountSemantics`, `ExtractionSummary`, `ExtractionMetadata`, `Proviso`, `Earmark`, `CrossReference`, `CrAnomaly`, `TransferLimit`, `FundAvailability`, `BillClassification`, `SourceSpan`, and all accessor methods on `Provision`. Also contains `BillExtraction::compute_totals()`. | Adding new provision types; adding new fields to existing types; changing budget authority calculation logic. |
| `src/approp/from_value.rs` | ~690 | **Resilient JSON → Provision parsing.** Manually walks `serde_json::Value` trees with fallbacks for missing fields, wrong types, and unknown enum variants. Contains `parse_bill_extraction()`, `parse_provision()`, `parse_dollar_amount()`, and dozens of helper functions. Produces `ConversionReport` documenting every compromise. | Adding new provision types (must add a match arm in `parse_provision()`); handling new LLM output quirks; adding new fields that need special parsing. |

### Core: Extraction Pipeline

| File | Lines | Purpose | When to Edit |
|------|-------|---------|-------------|
| `src/approp/extraction.rs` | ~840 | **ExtractionPipeline.** Orchestrates the full extraction process: XML parsing → chunk splitting → parallel LLM calls → response parsing → merge → compute totals → verify → write artifacts. Contains `TokenTracker`, `ChunkProgress`, `build_metadata()`, and the parallel streaming logic using `futures::stream`. | Changing the extraction flow; adding new artifact types; modifying chunk processing logic. Rarely edited — extraction is stable. |
| `src/approp/xml.rs` | ~590 | **Congressional bill XML parsing** via `roxmltree`. Extracts clean text with `''quote''` delimiters, identifies `<appropriations-major>` headings, and splits into chunks at `<division>` and `<title>` boundaries. Contains `parse_bill_xml()`, `parse_bill_xml_str()`, and the recursive XML tree walker. | Handling new XML element types; fixing text extraction edge cases; changing chunk splitting logic. |
| `src/approp/text_index.rs` | ~670 | **Dollar amount indexing.** Builds a positional index of every `$X,XXX,XXX` pattern, section header, and proviso clause in the source text. Used by verification for amount checking and by extraction for chunk boundary computation. Contains `TextIndex`, `ExtractionChunk`. | Adding new text patterns to index; changing how chunks are bounded. |
| `src/approp/prompts.rs` | ~310 | **System prompt for Claude.** The `EXTRACTION_SYSTEM` constant (~300 lines) defines every provision type, shows real JSON examples, constrains output format, and includes specific instructions for edge cases (CR substitutions, sub-allocations, mandatory spending extensions). | Improving extraction quality; adding new provision type definitions; fixing edge case handling. **Caution:** Changes invalidate all existing extractions — re-extraction is needed for affected bills. |
| `src/approp/progress.rs` | ~170 | **Extraction progress bar rendering.** Displays the live dashboard during multi-chunk extraction. | Changing the progress display format. |

### Core: Verification and Quality

| File | Lines | Purpose | When to Edit |
|------|-------|---------|-------------|
| `src/approp/verification.rs` | ~370 | **Deterministic verification.** Three checks: (1) dollar amount strings searched in source text, (2) raw_text matched via three-tier system (exact → normalized → spaceless → no_match), (3) completeness — percentage of dollar strings in source matched to provisions. Contains `verify_extraction()`, `AmountCheck`, `RawTextCheck`, `MatchTier`, `CheckResult`, `VerificationReport`. | Adding new verification checks (e.g., arithmetic checks); changing match tier logic. |
| `src/approp/staleness.rs` | ~165 | **Hash chain checking.** Computes SHA-256 of files, compares to stored hashes, returns `StaleWarning` if mismatched. Contains `check()`, `file_sha256()`, `StaleWarning` enum with `ExtractionStale`, `EmbeddingsStale`, and `BillMetaStale` variants. | Adding new staleness checks for additional pipeline artifacts. |

### Core: Query and Search

| File | Lines | Purpose | When to Edit |
|------|-------|---------|-------------|
| `src/approp/query.rs` | ~1,300 | **Library API.** Pure functions: `summarize()`, `search()`, `compare()`, `audit()`, `relate()`, `rollup_by_department()`, `build_embedding_text()`, `compute_link_hash()`. Also contains `normalize_agency()` (35-entry sub-agency lookup) and `normalize_account_name()`. The `compare()` function includes cross-semantics orphan rescue. All functions take `&[LoadedBill]` and return plain data structs. No I/O, no formatting, no side effects. | Adding new query functions; adding new search filter fields; changing budget authority logic; adding new output fields. |
| `src/approp/loading.rs` | ~340 | **Directory walking and bill loading.** `load_bills()` recursively finds `extraction.json` files, deserializes them along with sibling `verification.json`, `metadata.json`, and `bill_meta.json`, and returns `Vec<LoadedBill>`. | Adding new artifact types to load; changing discovery logic. |
| `src/approp/embeddings.rs` | ~260 | **Embedding storage.** `load()` / `save()` for the JSON metadata + binary vectors format. `cosine_similarity()`, `normalize()`, `top_n_similar()`. The split JSON+binary format is optimized for fast loading (~2ms for 29 MB). | Adding new similarity functions; changing storage format; adding batch operations. |

### API Clients

| File | Lines | Purpose | When to Edit |
|------|-------|---------|-------------|
| `src/api/anthropic/client.rs` | ~340 | **Anthropic API client.** Message creation with streaming response handling, thinking/extended thinking support, prompt caching. Uses `reqwest` with `rustls-tls`. | Adding retry logic; supporting new API features; handling new response formats. |
| `src/api/congress/` | ~850 (total) | **Congress.gov API client.** Bill listing, metadata lookup, text version discovery, XML download. Rate limit handling. | Adding new API endpoints; handling pagination edge cases. |
| `src/api/openai/client.rs` | ~45 | **OpenAI API client.** Embeddings endpoint only — minimal implementation. Sends batches of text, receives float32 vectors. | Adding retry logic; supporting new embedding models; adding new endpoints. |

### Tests

| File | Lines | Purpose | When to Edit |
|------|-------|---------|-------------|
| `tests/cli_tests.rs` | ~1,200 | **42 integration tests.** Runs the actual `congress-approp` binary against `data/` data and checks stdout/stderr. Includes budget authority total pinning, search output validation, format tests, enrich/relate/link workflow tests, FY/subcommittee filtering tests, --show-advance verification, and case-insensitive compare tests. | Adding tests for new CLI commands or flags; updating expected output when behavior changes intentionally. |

In addition to integration tests, most modules contain inline unit tests in `#[cfg(test)] mod tests { }` blocks at the bottom of the file.

## Data Flow Diagrams

### How `search --semantic` flows through the code

```text
main.rs: main()
  → match Commands::Search
  → handle_search()           [detects semantic.is_some()]
  → handle_semantic_search()  [async]
    → loading::load_bills()   [finds extraction.json files]
    → embeddings::load()      [for each bill directory]
    → OpenAIClient::embed()   [embeds query text — single API call, ~100ms]
    → for each provision:
        apply hard filters (type, division, dollars, etc.)
        cosine_similarity(query_vector, provision_vector)
    → sort by similarity descending
    → truncate to --top N
    → format output (table/json/jsonl/csv)
```

### How `extract` flows through the code

```text
main.rs: main()
  → match Commands::Extract
  → handle_extract()                     [async]
    → xml::parse_bill_xml()              [parse XML, get clean text + chunks]
    → ExtractionPipeline::new()
    → pipeline.extract_parallel()        [sends chunks to Claude in parallel]
      → for each chunk (bounded concurrency):
          AnthropicClient::create_message()
          from_value::parse_bill_extraction()
          save chunk artifacts to chunks/
    → merge provisions from all chunks
    → BillExtraction::compute_totals()   [sums provisions, never LLM arithmetic]
    → verification::verify_extraction()  [deterministic string matching]
    → write extraction.json, verification.json, metadata.json, tokens.json
```

### How `--similar` flows through the code

```text
main.rs: main()
  → match Commands::Search
  → handle_search()
  → handle_semantic_search()  [same entry point as --semantic]
    → loading::load_bills()
    → embeddings::load()      [for each bill]
    → look up source provision vector from stored vectors.bin  [NO API call]
    → cosine_similarity against all other provisions
    → sort, filter, truncate, format
```

## Key Patterns to Follow

### 1. Library function first, CLI second

New logic goes in `query.rs` (or a new module). The CLI handler in `main.rs` calls the library function and formats output. Never put business logic in `main.rs`.

### 2. All query functions take `&[LoadedBill]` and return structs

No I/O, no formatting, no side effects in library code. All output structs derive `Serialize` for JSON output.

```rust
// Good:
pub fn my_query(bills: &[LoadedBill]) -> Vec<MyResult> { ... }

// Bad:
pub fn my_query(dir: &Path) -> Result<()> { ... }  // Does I/O
```

### 3. Serde for everything

All data types derive `Serialize` and `Deserialize`. This enables JSON, JSONL, and CSV output for free.

### 4. Tests in the same file

Unit tests go in `#[cfg(test)] mod tests { }` at the bottom of each module. Integration tests go in `tests/cli_tests.rs`.

### 5. Clippy clean with `-D warnings`

Clippy treats warnings as errors in CI. Fix all clippy suggestions at the root cause — don't suppress with `#[allow]` unless absolutely necessary. Use `#[allow(clippy::too_many_arguments)]` sparingly.

### 6. Format with `cargo fmt` before committing

The CI rejects improperly formatted code.

## Existing CLI Command Definitions

For reference when adding new commands, here are the existing command patterns in `main.rs`:

```text
congress-approp download   --congress N --type T --number N --output-dir DIR [--enacted-only] [--format F] [--version V] [--dry-run]
congress-approp extract    --dir DIR [--parallel N] [--model M] [--dry-run]
congress-approp embed      --dir DIR [--model M] [--dimensions D] [--batch-size N] [--dry-run]
congress-approp search     --dir DIR [-t TYPE] [-a AGENCY] [--account A] [-k KW] [--bill B] [--division D] [--min-dollars N] [--max-dollars N] [--semantic Q] [--similar S] [--top N] [--format F] [--list-types]
congress-approp summary    --dir DIR [--format F] [--by-agency]
congress-approp compare    --base DIR --current DIR [-a AGENCY] [--format F]
congress-approp audit      --dir DIR [--verbose]
congress-approp upgrade    --dir DIR [--dry-run]
congress-approp api test
congress-approp api bill list --congress N [--type T] [--offset N] [--limit N] [--enacted-only]
congress-approp api bill get --congress N --type T --number N
congress-approp api bill text --congress N --type T --number N
```

## Files That Don't Exist Yet

These modules are designed but not implemented — they appear in the roadmap:

| File | Purpose | Status |
|------|---------|--------|
| `src/approp/bill_meta.rs` | Bill metadata types, XML parsing, jurisdiction classification, FY-aware advance detection, account normalization (33 unit tests) | Shipped in v4.0 |
| `src/approp/links.rs` | Cross-bill link types, suggest algorithm, accept/remove, load/save for `links/links.json` (10 unit tests) | Shipped in v4.0 |
| `relate` command | Deep-dive on one provision across all bills with FY timeline, confidence tiers, and deterministic link hashes | Shipped in v4.0 |

See `NEXT_STEPS.md` (gitignored) for detailed implementation plans.

## Next Steps

- **[Adding a New Provision Type](./new-provision-type.md)** — the most common contributor task
- **[Adding a New CLI Command](./new-command.md)** — how to add a new subcommand
- **[Testing Strategy](./testing.md)** — how the test suite works
- **[Architecture Overview](./architecture.md)** — the big-picture design