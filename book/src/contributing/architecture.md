# Architecture Overview

This chapter provides a high-level map of how `congress-approp` is structured — for developers who want to understand the codebase, contribute features, or debug issues.

## The Pipeline

Every bill flows through five stages. Each stage is implemented by a distinct set of modules:

```text
Stage 1: Download    →  api/congress/       →  BILLS-*.xml
Stage 2: Parse       →  approp/xml.rs       →  clean text + chunk boundaries
Stage 3: Extract     →  approp/extraction.rs →  extraction.json + verification.json
Stage 4: Embed       →  api/openai/         →  embeddings.json + vectors.bin
Stage 5: Query       →  approp/query.rs     →  search, compare, summary, audit output
```

Only stages 3 (Extract) and 4 (Embed) call external APIs. Everything else is local and deterministic.

## Module Map

```text
src/
  main.rs                    ← CLI entry point, clap definitions, output formatting (~4,200 lines)
  lib.rs                     ← Re-exports: api:: and approp::, plus load_bills and query
  api/
    mod.rs                   ← pub mod anthropic; pub mod congress; pub mod openai;
    anthropic/               ← Claude API client (~660 lines)
      client.rs              ← Message creation with streaming, thinking, caching
      mod.rs
    congress/                ← Congress.gov API client (~850 lines)
      bill.rs                ← Bill listing, metadata, text versions
      client.rs              ← HTTP client with auth
      mod.rs
    openai/                  ← OpenAI API client (~75 lines)
      client.rs              ← Embeddings endpoint only
      mod.rs
  approp/
    mod.rs                   ← pub mod for all submodules
    ontology.rs              ← ALL data types (~960 lines)
    extraction.rs            ← ExtractionPipeline: parallel chunk processing (~840 lines)
    from_value.rs            ← Resilient JSON→Provision parsing (~690 lines)
    xml.rs                   ← Congressional bill XML parsing (~590 lines)
    text_index.rs            ← Dollar amount indexing, section detection (~670 lines)
    prompts.rs               ← System prompt for Claude (~310 lines)
    verification.rs          ← Deterministic verification (~370 lines)
    loading.rs               ← Directory walking, JSON loading (~300 lines)
    query.rs                 ← Library API: search, compare, summarize, audit (~840 lines)
    embeddings.rs            ← Embedding storage, cosine similarity (~260 lines)
    staleness.rs             ← Hash chain checking (~100 lines)
    progress.rs              ← Extraction progress bar (~170 lines)
tests/
  cli_tests.rs               ← 18 integration tests against examples/ data (~411 lines)
```

Total: approximately 9,500 lines of Rust.

## Core Data Types (ontology.rs)

The `Provision` enum is the heart of the data model. It has 11 variants, each representing a different type of legislative provision:

| Variant | Key Fields |
|---------|-----------|
| `Appropriation` | `account_name`, `agency`, `amount`, `detail_level`, `parent_account`, `fiscal_year`, `availability`, `provisos`, `earmarks` |
| `Rescission` | `account_name`, `agency`, `amount`, `reference_law` |
| `TransferAuthority` | `from_scope`, `to_scope`, `limit`, `conditions` |
| `Limitation` | `description`, `amount`, `account_name` |
| `DirectedSpending` | `account_name`, `amount`, `earmark`, `detail_level` |
| `CrSubstitution` | `new_amount`, `old_amount`, `account_name`, `reference_act` |
| `MandatorySpendingExtension` | `program_name`, `statutory_reference`, `amount`, `period` |
| `Directive` | `description`, `deadlines` |
| `Rider` | `description`, `policy_area` |
| `ContinuingResolutionBaseline` | `reference_year`, `reference_laws`, `rate`, `duration` |
| `Other` | `llm_classification`, `description`, `amounts`, `metadata` |

All variants share common fields: `section`, `division`, `title`, `confidence`, `raw_text`, `notes`, `cross_references`.

The enum uses tagged serde: `#[serde(tag = "provision_type", rename_all = "snake_case")]`, so each JSON object self-identifies.

### Supporting Types

- **`DollarAmount`** — `value` (AmountValue), `semantics` (AmountSemantics), `text_as_written`
- **`AmountValue`** — `Specific { dollars: i64 }`, `SuchSums`, `None`
- **`AmountSemantics`** — `NewBudgetAuthority`, `Rescission`, `ReferenceAmount`, `Limitation`, `TransferCeiling`, `MandatorySpending`, `Other(String)`
- **`BillExtraction`** — top-level structure: `bill`, `provisions`, `summary`, `chunk_map`, `schema_version`
- **`BillInfo`** — `identifier`, `classification`, `short_title`, `fiscal_years`, `divisions`, `public_law`
- **`ExtractionSummary`** — LLM self-check totals (diagnostic only, never used for computation)

The `BillExtraction::compute_totals()` method deterministically computes budget authority and rescissions from the provisions array, filtering by semantics and detail_level.

## The Extraction Pipeline (extraction.rs)

`ExtractionPipeline` orchestrates the LLM extraction process:

1. **Parse XML** — calls `xml::parse_bill_xml()` to get clean text and chunk boundaries
2. **Build chunks** — each chunk gets the full system prompt plus its section of bill text
3. **Extract in parallel** — sends chunks to Claude via the Anthropic API with bounded concurrency (`--parallel N`)
4. **Parse responses** — `from_value::parse_bill_extraction()` handles LLM output with resilient parsing
5. **Merge** — provisions from all chunks are combined into a single list
6. **Compute totals** — budget authority is summed from provisions (never trusting LLM arithmetic)
7. **Verify** — `verification::verify_extraction()` runs deterministic checks
8. **Write** — all artifacts saved to disk

Progress updates are sent via a channel to a rendering task that displays the live dashboard.

## Resilient Parsing (from_value.rs)

This module bridges the gap between the LLM's JSON output and Rust's strict type system:

- **Missing fields** → defaults (empty string, null, empty array)
- **Wrong types** → coerced (string `"$10,000,000"` → integer `10000000`)
- **Unknown provision types** → wrapped as `Provision::Other` with original classification preserved
- **Extra fields** → silently ignored for known types; preserved in `metadata` map for `Other`
- **Failed provisions** → logged as warnings, skipped

Every compromise is counted in a `ConversionReport` — the tool never silently hides parsing issues.

## Verification (verification.rs)

Three deterministic checks, no LLM involved:

1. **Amount checks** — search for each `text_as_written` dollar string in the source text
2. **Raw text checks** — check if `raw_text` is a substring of source (exact → normalized → spaceless → no_match)
3. **Completeness** — count dollar-sign patterns in source and check how many are accounted for

The `text_index.rs` module builds a positional index of every dollar amount and section header in the source text, used by verification and for chunk boundary computation.

## Library API (query.rs)

Pure functions that take `&[LoadedBill]` and return data structs:

```rust
pub fn summarize(bills: &[LoadedBill]) -> Vec<BillSummary>
pub fn search(bills: &[LoadedBill], filter: &SearchFilter) -> Vec<SearchResult>
pub fn compare(base: &[LoadedBill], current: &[LoadedBill], agency: Option<&str>) -> Vec<AccountDelta>
pub fn audit(bills: &[LoadedBill]) -> Vec<AuditRow>
pub fn rollup_by_department(bills: &[LoadedBill]) -> Vec<AgencyRollup>
pub fn build_embedding_text(provision: &Provision) -> String
```

**Design contract:** No I/O, no formatting, no API calls, no side effects. The CLI layer (`main.rs`) handles all formatting and output.

## Embeddings (embeddings.rs)

Split storage: JSON metadata + binary float32 vectors.

Key functions:
- `load(dir)` → `Option<LoadedEmbeddings>` — loads metadata and binary vectors
- `save(dir, metadata, vectors)` — writes both files atomically
- `cosine_similarity(a, b)` → `f32` — dot product (vectors are L2-normalized)
- `normalize(vec)` — L2-normalize in place

## Loading (loading.rs)

`load_bills(dir)` recursively walks from a path, finds every `extraction.json`, and loads it along with sibling `verification.json` and `metadata.json` into `LoadedBill` structs. Results are sorted by bill identifier.

## CLI Layer (main.rs)

The CLI is built with `clap` derive macros. The `Commands` enum defines all subcommands. Each command has a handler function:

| Command | Handler | Lines | Async? |
|---------|---------|-------|--------|
| `summary` | `handle_summary()` | ~160 | No |
| `search` | `handle_search()` | ~530 | Yes (semantic path) |
| `search --semantic` | `handle_semantic_search()` | ~330 | Yes |
| `compare` | `handle_compare()` | ~210 | No |
| `audit` | `handle_audit()` | ~180 | No |
| `extract` | `handle_extract()` | ~310 | Yes |
| `embed` | `handle_embed()` | ~120 | Yes |
| `download` | `handle_download()` | ~400 | Yes |
| `upgrade` | `handle_upgrade()` | ~150 | No |

> **Known technical debt:** `main.rs` is ~4,200 lines. While the summary and compare handlers have been consolidated to call library functions in `query.rs`, the search handler still contains substantial inline formatting logic. Each provision type has its own table column layout, and the semantic search path has ~200 lines of inline filtering. A future refactor could reduce `main.rs` by extracting the table formatting into a dedicated module.

## Key Design Decisions

### 1. LLM isolation

The LLM touches data exactly once (extraction). Every downstream operation is deterministic. If you don't trust the LLM's classification, the `raw_text` field lets you read the original bill language.

### 2. Budget totals from provisions, not summaries

`compute_totals()` sums individual provisions filtered by semantics and detail_level. The LLM's self-reported `total_budget_authority` is never used for computation.

### 3. Semantic chunking

Bills are split at XML `<division>` and `<title>` boundaries, not at arbitrary token limits. Each chunk contains a complete legislative section, preserving context for the LLM.

### 4. Tagged enum deserialization

`Provision` uses `#[serde(tag = "provision_type")]`. Each JSON object self-identifies. Forward-compatible and human-readable.

### 5. Resilient LLM output parsing

`from_value.rs` manually walks the `serde_json::Value` tree with fallbacks rather than using strict deserialization. An unknown provision type becomes `Other` with the original data preserved. Extraction rarely fails entirely.

### 6. Schema evolution without re-extraction

The `upgrade` command re-deserializes through the current schema, re-runs verification, and updates files — no LLM calls needed. New fields get defaults, renamed fields get mapped.

### 7. Write-once, read-many

All artifacts are immutable after creation. No file locking, no database, no caching needed. The files ARE the cache. Hash checks are ~2ms and run on every load.

## Dependencies

| Crate | Role |
|-------|------|
| `clap` | CLI argument parsing (derive macros) |
| `roxmltree` | XML parsing — pure Rust, read-only |
| `reqwest` | HTTP client for all three APIs (with `rustls-tls`) |
| `tokio` | Async runtime for parallel API calls |
| `serde` / `serde_json` | Serialization for all JSON artifacts |
| `walkdir` | Recursive directory traversal |
| `comfy-table` | Terminal table formatting |
| `csv` | CSV output |
| `sha2` | SHA-256 hashing for the hash chain |
| `chrono` | Timestamps in metadata |
| `ulid` | Unique IDs for chunk artifacts |
| `anyhow` / `thiserror` | Error handling (anyhow for CLI, thiserror for library) |
| `tracing` / `tracing-subscriber` | Structured logging |
| `futures` | Stream processing for parallel extraction |

All API clients use `rustls-tls` — no OpenSSL dependency.

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Load 3 bills (JSON parsing) | ~10ms | |
| Load embeddings (3 bills, binary) | ~2ms | Memory read |
| SHA-256 hash all files (3 bills) | ~2ms | |
| Cosine search (2,500 provisions) | <0.1ms | Dot products |
| **Total cold-start query** | **~15ms** | Load + hash + search |
| Embed query text (OpenAI API) | ~100ms | Network round-trip |
| Full extraction (omnibus, 75 chunks) | ~60 min | Parallel LLM calls |
| Generate embeddings (2,500 provisions) | ~30 sec | Batch API calls |

At 20 congresses (~60 bills, ~15,000 provisions): cold start ~80ms, search <1ms. The system scales linearly and stays interactive at any realistic data volume.

## Next Steps

- **[Code Map](./code-map.md)** — file-by-file guide to the codebase
- **[Adding a New Provision Type](./new-provision-type.md)** — the most common contributor task
- **[Testing Strategy](./testing.md)** — how the test suite is structured
- **[Style Guide and Conventions](./style-guide.md)** — coding standards and practices