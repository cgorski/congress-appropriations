# Architecture

A guide to how `congress-appropriations` works — for new users who want to understand the system, and developers who want to contribute.

---

## What This Is

`congress-appropriations` is a Rust crate (library + CLI binary) that turns U.S. federal appropriations bills into structured, searchable, machine-readable data. It downloads bill XML from Congress.gov, uses an LLM to extract every spending provision, deterministically verifies the extraction against the source text, generates semantic embeddings for meaning-based search, and provides query tools for journalists, staffers, and researchers.

The core principle: **the LLM does the hard part (understanding legal text), but every number is verified by code, every query is deterministic, and every artifact is traceable back to its source.**

---

## The Pipeline

A bill flows through five stages. Each stage produces immutable files. Once a stage completes for a bill, its output is never modified (except in rare deliberate re-runs).

```
                    ┌──────────┐
  Congress.gov ───▶ │ Download │ ───▶ BILLS-*.xml
                    └──────────┘
                         │
                    ┌──────────┐
                    │  Parse   │ ───▶ clean text + chunk boundaries
                    │  + XML   │
                    └──────────┘
                         │
                    ┌──────────┐
  Anthropic API ◀── │ Extract  │ ───▶ extraction.json + verification.json
                    │  (LLM)   │      metadata.json + tokens.json + chunks/
                    └──────────┘
                         │
                    ┌──────────┐
  OpenAI API ◀───── │  Embed   │ ───▶ embeddings.json + vectors.bin
                    └──────────┘
                         │
                    ┌──────────┐
                    │  Query   │ ───▶ search, compare, summary, audit, relate
                    └──────────┘
```

**Only stages 3 (Extract) and 4 (Embed) call external APIs.** Everything else is local, deterministic, and instant.

### Stage 1: Download

The `download` command fetches enrolled bill XML from the Congress.gov API. "Enrolled" means the final version passed by both chambers and sent to the President — the version that becomes law.

**Input:** Congress number + bill type + bill number (e.g., 118th Congress, H.R. 4366)
**Output:** `BILLS-118hr4366enr.xml`
**Requires:** `CONGRESS_API_KEY` (free from Congress.gov)

### Stage 2: Parse

`xml.rs` parses the bill XML using `roxmltree` (pure Rust, no C dependencies). Congressional bill XML has semantic markup — `<division>`, `<title>`, `<appropriations-major>`, `<appropriations-small>`, `<quote>`, `<proviso>` — that the parser uses to extract clean text and identify structural boundaries.

The parser also splits large bills into **chunks** at division and title boundaries. This is critical: an omnibus bill can be 1,500+ pages. Splitting at semantic boundaries (not arbitrary token limits) means each chunk contains a complete legislative section with full context. The FY2024 omnibus splits into 75 chunks.

**Input:** `BILLS-*.xml`
**Output:** Clean text string + vector of `ExtractionChunk` structs
**No API calls.** Pure Rust.

### Stage 3: Extract

`extraction.rs` sends each chunk to Claude (via the Anthropic API) with a detailed system prompt (`prompts.rs`, ~300 lines) that defines every provision type, field, and edge case. The prompt includes real examples from actual bills and explicit instructions for handling sub-allocations, CR substitutions, transfer authority, and mandatory spending extensions.

Chunks are extracted in parallel (default 5 concurrent API calls). Each chunk produces a JSON array of provisions. After all chunks complete:
1. Provisions are merged into a single list
2. Budget totals are computed from the actual provisions (never from LLM self-reported totals)
3. Deterministic verification runs against the source text
4. All artifacts are written to disk

**Input:** Clean text + chunks
**Output:** `extraction.json`, `verification.json`, `metadata.json`, `tokens.json`, `chunks/` directory
**Requires:** `ANTHROPIC_API_KEY`

The `chunks/` directory contains per-chunk LLM artifacts: the model's thinking traces, raw responses, parsed JSON, and conversion reports. These are permanent provenance records kept locally (gitignored) for analysis of how the LLM interpreted each section.

### Stage 4: Embed

The `embed` command generates semantic embedding vectors for every provision using OpenAI's `text-embedding-3-large` model. Each provision is represented by concatenating its meaningful fields:

```
Account: Child Nutrition Programs | Agency: Department of Agriculture | Text: For necessary expenses...
```

This combined text is embedded into a 1024-dimensional vector that captures the provision's meaning. Provisions about similar topics (even with completely different wording) will have vectors pointing in similar directions — enabling semantic search.

**Input:** `extraction.json`
**Output:** `embeddings.json` (metadata) + `vectors.bin` (binary float32 vectors)
**Requires:** `OPENAI_API_KEY`

### Stage 5: Query

All query operations (`search`, `summary`, `compare`, `audit`) run locally against the JSON and binary files on disk. No API calls at query time — except `--semantic` search, which makes one small API call to embed the query text.

---

## Data Directory Layout

Every bill lives in its own directory. Files are discovered by walking recursively for `extraction.json` — that's the anchor file. Everything else is optional.

```
examples/                          ← any --dir path works
├── hr4366/                        ← bill directory
│   ├── BILLS-118hr4366enr.xml     ← source XML from Congress.gov
│   ├── extraction.json            ← structured provisions (REQUIRED)
│   ├── verification.json          ← deterministic verification report
│   ├── metadata.json              ← model, prompt version, hashes
│   ├── tokens.json                ← token usage from extraction
│   ├── embeddings.json            ← embedding metadata (model, dimensions, hashes)
│   ├── vectors.bin                ← raw float32 embedding vectors
│   └── chunks/                    ← per-chunk LLM artifacts (gitignored)
│       ├── 01KKWW9T5RR0JTQ6C9FYYE96A8.json
│       └── ...
├── hr5860/
│   └── ...
└── hr9468/
    └── ...
```

| File | Required | Written by | Read by | Mutated after creation? |
|------|----------|-----------|---------|------------------------|
| `BILLS-*.xml` | For extraction | `download` | `extract`, `upgrade` | Never |
| `extraction.json` | **Yes** | `extract` | All query commands | Never (unless deliberately re-extracted) |
| `verification.json` | No | `extract`, `upgrade` | `audit`, `search` quality | Never |
| `metadata.json` | No | `extract` | Staleness detection | Never |
| `tokens.json` | No | `extract` | Informational | Never |
| `embeddings.json` | No | `embed` | Semantic search | Never (unless re-embedded) |
| `vectors.bin` | No | `embed` | Semantic search | Never (unless re-embedded) |
| `chunks/*.json` | No | `extract` | Analysis/debugging | Never |

**Every file is write-once.** Once a bill is extracted and embedded, its files are never modified. The system is read-dominated: writes happen ~15 times per year (when Congress enacts bills), reads happen hundreds to thousands of times.

Nesting is flexible — `data/congress/118/hr4366/extraction.json` works just as well as `examples/hr4366/extraction.json`. The loader walks recursively from whatever `--dir` you point it at.

---

## The Hash Chain

Each downstream artifact records the SHA-256 hash of its input. This enables **staleness detection**: if someone re-downloads the XML or re-extracts with a new model, all downstream artifacts are detectable as potentially stale.

```
BILLS-*.xml ──sha256──▶ metadata.json (source_xml_sha256)
                              │
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
                              │
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

The `staleness.rs` module checks this chain on commands that use embeddings. If a hash mismatches, it prints a warning to stderr but never blocks execution:

```
⚠ H.R. 4366: embeddings are stale (extraction.json has changed)
```

Hashing all files for 3 bills takes ~2ms. At 60 bills, ~12ms. There is no performance reason to skip or cache hash checks.

---

## Embedding Storage Format

Embeddings use a split format: JSON metadata + binary vectors.

**`embeddings.json`** (~200 bytes, human-readable):
```json
{
  "schema_version": "1.0",
  "model": "text-embedding-3-large",
  "dimensions": 1024,
  "count": 2364,
  "extraction_sha256": "ae912e3427b8...",
  "vectors_file": "vectors.bin",
  "vectors_sha256": "7bd7821176bc..."
}
```

**`vectors.bin`** (count × dimensions × 4 bytes, binary):
Raw little-endian float32 array. No header. Dimensions and count come from the JSON metadata. Loaded in Rust via `std::fs::read()` + byte-to-float conversion.

**Why binary for vectors:** At 1024 dimensions × 2,364 provisions, the binary file is 9.7 MB and loads in <2ms. The same data as JSON float arrays would be ~57 MB and take ~175ms to parse in Rust. Since this is a read-heavy system (load once per CLI invocation, query many times), the binary format keeps startup instant.

**Why JSON for metadata:** The metadata is tiny and must be human-inspectable for debugging and provenance. `cat embeddings.json` tells you what model was used, how many provisions are embedded, and what extraction they correspond to.

---

## Semantic Search

Semantic search lets users find provisions by meaning rather than keywords. The query "school lunch programs for kids" finds "Child Nutrition Programs" even though the words don't overlap — because the *meaning* is similar.

### How it works

1. **At embed time:** Each provision's text is sent to OpenAI's `text-embedding-3-large` model, which returns a 1024-dimensional vector representing its meaning. These vectors are stored in `vectors.bin`.

2. **At query time:** The user's search query is embedded using the same model (single API call, ~100ms). The query vector is compared to every provision vector using cosine similarity (dot product of normalized vectors). Results are ranked by similarity and filtered by any hard constraints (--type, --division, --min-dollars, etc.).

3. **Performance:** Cosine similarity over 2,500 vectors takes <0.1ms. The bottleneck is loading the binary file (~2ms) and the single API call to embed the query (~100ms). Total: ~100ms per search.

### Similarity scores

OpenAI embedding vectors are L2-normalized (norm = 1.0), so cosine similarity equals the dot product. Scores range from -1 to 1 in theory, but in practice for this data:

| Score | Interpretation |
|-------|---------------|
| > 0.80 | Same account/program across bills |
| 0.60–0.80 | Related topic, same policy area |
| 0.45–0.60 | Loosely related concepts |
| < 0.45 | Unlikely to be meaningfully related |

### Find-similar

`--similar hr4366:42` takes provision #42's embedding vector and finds the most similar provisions across all loaded bills. This enables:
- **Cross-bill matching:** find the same program in a different bill
- **Year-over-year tracking:** find last year's version of this provision
- **Conference tracking:** match House and Senate versions

---

## Module Map

### Core data types

| Module | Lines | Purpose |
|--------|-------|---------|
| `ontology.rs` | ~960 | All data types. The `Provision` enum has 11 variants (Appropriation, Rescission, TransferAuthority, Limitation, DirectedSpending, CrSubstitution, MandatorySpendingExtension, Directive, Rider, ContinuingResolutionBaseline, Other). Also defines `BillExtraction`, `DollarAmount`, `AmountSemantics`, `BillClassification`, `ExtractionMetadata`, and all supporting types. |
| `from_value.rs` | ~690 | Resilient JSON → Provision deserialization. Handles LLM output variance: missing fields get defaults, unexpected types are coerced, unknown provision types become `Other`. This is why extraction rarely fails even when the LLM returns imperfect JSON. |

### Extraction pipeline

| Module | Lines | Purpose |
|--------|-------|---------|
| `extraction.rs` | ~840 | `ExtractionPipeline` — orchestrates parallel LLM chunk extraction, merges results, builds metadata. Contains `build_metadata()` which computes the source XML hash for the hash chain. |
| `xml.rs` | ~590 | Congressional bill XML parsing via `roxmltree`. Extracts clean text, identifies `<appropriations-major>` headings, and splits into chunks at division/title boundaries. |
| `text_index.rs` | ~670 | Builds a positional index of every dollar amount (`$X,XXX,XXX`), section header, and proviso clause in the source text. Used by verification and for chunk boundary computation. |
| `prompts.rs` | ~310 | The system prompt sent to Claude. Defines every provision type, shows real JSON examples, constrains output format, and includes specific instructions for edge cases (CR substitutions, sub-allocations, mandatory spending extensions). |
| `progress.rs` | ~170 | Extraction progress bar rendering. |

### Verification and quality

| Module | Lines | Purpose |
|--------|-------|---------|
| `verification.rs` | ~370 | Deterministic post-extraction verification. Three checks: (1) dollar amount strings found in source text, (2) raw_text matched via three-tier system (exact → normalized → spaceless), (3) completeness — how many dollar references in the source were accounted for. No LLM involved. |
| `staleness.rs` | ~100 | Hash chain integrity checking. Compares stored SHA-256 hashes to current file contents. Returns warnings for stale artifacts. |

### Query and search

| Module | Lines | Purpose |
|--------|-------|---------|
| `query.rs` | ~840 | The library API. Functions: `summarize()`, `search()`, `compare()`, `audit()`, `rollup_by_department()`, `build_embedding_text()`. All take `&[LoadedBill]` and return plain data structs. No I/O, no formatting. |
| `loading.rs` | ~300 | Directory walking via `walkdir`, JSON deserialization, assembly of `LoadedBill` structs. Finds `extraction.json` recursively, loads sibling artifacts. |
| `embeddings.rs` | ~260 | Embedding storage: `load()` / `save()` for the JSON metadata + binary vectors format. Also provides `cosine_similarity()`, `normalize()`, and `top_n_similar()` functions for vector search. |

### API clients

| Module | Lines | Purpose |
|--------|-------|---------|
| `api/congress/` | ~850 | Congress.gov API client. Bill listing, metadata lookup, text download. |
| `api/anthropic/` | ~660 | Anthropic API client. Message creation with streaming, thinking support, caching. |
| `api/openai/` | ~75 | OpenAI API client. Embeddings endpoint only. Minimal — just enough for `embed` command. |

---

## Library API

The crate exports a library API alongside the CLI binary. The CLI (`main.rs`) is a thin layer that calls library functions and formats output.

```rust
use congress_appropriations::{load_bills, query};
use congress_appropriations::approp::query::SearchFilter;
use std::path::Path;

// Load all bills under a directory (recursively finds extraction.json files)
let bills = load_bills(Path::new("examples"))?;

// Per-bill budget summary
let summaries = query::summarize(&bills);
for s in &summaries {
    println!("{}: ${} BA", s.identifier, s.budget_authority);
}

// Search with filters (all fields optional, ANDed together)
let results = query::search(&bills, &SearchFilter {
    provision_type: Some("appropriation"),
    division: Some("A"),
    min_dollars: Some(1_000_000_000),
    ..Default::default()
});

// Budget authority by parent department (query-time grouping, never stored)
let agencies = query::rollup_by_department(&bills);

// Cross-bill comparison
let diff = query::compare(&fy2023_bills, &fy2024_bills, None);

// Verification audit
let audit_rows = query::audit(&bills);

// Build embedding text (deterministic, for use with any embedding API)
let text = query::build_embedding_text(&some_provision);
```

### Design principles

- **All query functions are pure.** They take `&[LoadedBill]` and return data. No side effects, no I/O, no API calls.
- **The CLI formats; the library computes.** `main.rs` handles table/JSON/CSV/JSONL rendering. The library returns structs.
- **Semantic search is separate.** Embedding loading and cosine similarity live in `embeddings.rs`, not `query.rs`. The CLI wires them together. This keeps the library usable without OpenAI.

---

## Verification Design

Verification answers two questions with zero LLM involvement:

### "Are the extracted amounts real?"

For each provision with a dollar amount, the verifier searches for the `text_as_written` string (e.g., `"$2,285,513,000"`) in the original bill text.

| Result | Meaning |
|--------|---------|
| `found` | Amount string found at exactly one position — high confidence |
| `found_multiple` | Amount string found at multiple positions — correct but ambiguous (common for round numbers like "$5,000,000") |
| `not_found` | Amount string not in source text — needs manual review |

Across all example data: **0 amounts not found.**

### "Is extraction complete?"

The `text_index` counts every dollar-sign pattern in the source text. The completeness percentage is: (dollar refs matched to provisions) / (total dollar refs). This can legitimately be below 100%:

- **Statutory references** — amounts from other laws cited in the text
- **Loan guarantee ceilings** — not budget authority
- **Struck amounts** — "striking '$50,000' and inserting '$75,000'" has an old amount that shouldn't be extracted
- **Proviso sub-allocations** — "of which $X shall be for..." may or may not be captured as separate provisions

The completeness metric lives in `audit`, not in the default `summary` display, because it requires this context to interpret correctly.

### Raw text matching tiers

| Tier | Method | What it handles |
|------|--------|-----------------|
| **Exact** | Byte-identical substring | Clean extractions (96.7% of provisions) |
| **Normalized** | Collapse whitespace, normalize curly quotes and em-dashes to ASCII | Unicode formatting differences (2.5%) |
| **Spaceless** | Remove all spaces then compare | Line-break artifacts (0%) |
| **NoMatch** | None of the above | Truncated LLM output (0.8% — all are truncated statutory amendments) |

---

## Key Design Decisions

### 1. LLM isolation

The LLM touches the data exactly once: during extraction. Every downstream operation — verification, querying, budget arithmetic, semantic search ranking — is deterministic. If you don't trust the LLM's classification of a provision, the `raw_text` field lets you read the original bill language yourself.

### 2. Budget totals from provisions, not summaries

`BillExtraction::compute_totals()` sums individual provision dollar amounts filtered by `semantics == "new_budget_authority"`. The LLM also produces an `ExtractionSummary` with totals, but these are never used for computation — only for diagnostics. This means a bug in the LLM's arithmetic can't corrupt budget totals.

### 3. Semantic chunking

Bills are split at XML `<division>` and `<title>` boundaries, not at arbitrary token limits. Each chunk contains a complete legislative section. This reduces extraction errors at boundaries and preserves context (e.g., a proviso that references "the amount made available under this heading" needs to see the heading).

### 4. Tagged enum deserialization

`Provision` uses `#[serde(tag = "provision_type", rename_all = "snake_case")]`. Each JSON object self-identifies: `{"provision_type": "appropriation", "account_name": "...", ...}`. This makes `extraction.json` human-readable, forward-compatible, and robust against field variations across provision types.

### 5. Resilient LLM output parsing

`from_value.rs` doesn't deserialize LLM output with strict `serde`. Instead, it manually walks the `serde_json::Value` tree with fallbacks for missing fields, wrong types, and unknown enum variants. An appropriation missing `fiscal_year` gets `None`. An unknown provision type becomes `Other` with the LLM's original classification preserved. This absorbs LLM variance without hard failures.

### 6. Schema evolution without re-extraction

The `upgrade` command re-deserializes and re-verifies existing data against the current code's schema. New fields get defaults. Renamed fields get mapped. Verification is re-run against the source XML. This means schema changes (new provision types, new fields, new verification checks) can be applied to historical data without re-running the expensive LLM extraction.

### 7. Write-once, read-many

All artifacts except link files (future) are immutable after creation. This means:
- No file locking needed
- No database needed — JSON files on disk are the right abstraction
- No caching needed — the files ARE the cache
- Hash checks are free (~2ms) and should run on every load

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Load 3 bills (extraction.json) | ~10ms | JSON parsing |
| Load embeddings (3 bills, binary) | ~2ms | Memory-mapped read |
| Hash all files (3 bills) | ~2ms | SHA-256 |
| Cosine search (2,500 provisions) | <0.1ms | Numpy-equivalent dot product |
| **Total cold-start query** | **~15ms** | Load + hash + search |
| Embed query text (OpenAI API) | ~100ms | Network round-trip |
| Full extraction (omnibus, 75 chunks) | ~60 min | Parallel LLM calls |
| Generate embeddings (2,500 provisions) | ~30 sec | Batch API calls |

At 20 congresses (~60 bills, ~15,000 provisions): cold start ~80ms, search <1ms. The system scales linearly and stays interactive at any realistic data volume.

---

## Dependencies

| Crate | Role |
|-------|------|
| `clap` | CLI argument parsing (derive macros) |
| `roxmltree` | XML parsing — pure Rust, read-only, zero-copy where possible |
| `reqwest` | HTTP client for Congress.gov, Anthropic, and OpenAI APIs (with `rustls-tls`) |
| `tokio` | Async runtime for parallel API calls |
| `serde` / `serde_json` | Serialization for all JSON artifacts |
| `walkdir` | Recursive directory traversal |
| `comfy-table` | Terminal table formatting |
| `csv` | CSV output |
| `sha2` | SHA-256 hashing for the hash chain |
| `chrono` | Timestamps in metadata |
| `ulid` | Unique IDs for chunk artifacts |
| `anyhow` / `thiserror` | Error handling (anyhow for CLI, thiserror for library errors) |
| `tracing` / `tracing-subscriber` | Structured logging |