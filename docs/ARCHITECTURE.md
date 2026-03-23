# Architecture

A guide to how `congress-appropriations` works — for new users who want to understand the system, and developers who want to contribute.

---

## What This Is

`congress-appropriations` is a Rust crate (library + CLI binary) that turns U.S. federal appropriations bills into structured, searchable, machine-readable data. It downloads bill XML from Congress.gov, uses an LLM to extract every spending provision, deterministically verifies the extraction against the source text, maps each account to a Treasury Account Symbol for cross-bill tracking, generates semantic embeddings for meaning-based search, and provides query tools for journalists, staffers, and researchers.

The core principle: **the LLM does the hard part (understanding legal text), but every number is verified by code, every provision is traceable to the exact byte in the enrolled bill, every account is linked by a government-assigned identifier, and every query is deterministic.**

---

## The Pipeline

A bill flows through seven stages. Each stage produces files without modifying previous outputs. The hash chain detects when upstream files change.

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
                    │ Verify   │ ───▶ source_span on each provision     (offline)
                    │  Text    │      3-tier repair: prefix→substr→norm
                    └──────────┘
                         │
                    ┌──────────┐
                    │ Enrich   │ ───▶ bill_meta.json                    (offline)
                    └──────────┘
                         │
              ┌──────────┴──────────┐
              │                     │
         ┌──────────┐         ┌──────────┐
  Claude ◀│ Resolve  │  OpenAI◀│  Embed   │ ───▶ embeddings.json
  Opus    │   TAS    │         └──────────┘       vectors.bin
          └──────────┘
               │
          tas_mapping.json
               │
         ┌──────────┐
         │Authority │ ───▶ authorities.json      (offline)
         │  Build   │      1,051 accounts, 937 cross-bill links
         └──────────┘
                    └──────────┘
                         │
                    ┌──────────┐
                    │  Query   │ ───▶ search, compare, summary, audit, relate
                    └──────────┘
                         │
                    ┌──────────┐
                    │  Link    │ ───▶ links/links.json        (persistent)
                    │(optional)│
                    └──────────┘
```

**Only stages 3 (Extract) and 4 (Embed) call external APIs.** Everything else — downloading, parsing, enrichment, verification, linking, querying — runs locally and deterministically.

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

### Stage 3.5: Enrich (Optional)

The `enrich` command generates bill-level metadata by parsing the source XML structure and analyzing the already-extracted provisions. It runs entirely offline — no API calls needed.

**What it produces:**
- **Subcommittee mappings** — division letter → canonical jurisdiction (Defense, THUD, CJS, etc.) via XML parsing and pattern matching on division titles
- **Advance appropriation classification** — each budget authority provision classified as current-year, advance, or supplemental using a fiscal-year-aware algorithm that compares "October 1, YYYY" and "first quarter of fiscal year YYYY" dates to the bill's fiscal year
- **Bill nature** — enriched classification (omnibus, minibus, full-year CR with appropriations, supplemental, authorization) from provision type distribution and subcommittee count
- **Canonical account names** — lowercased, em-dash-prefix-stripped names for case-insensitive cross-bill matching
- **Classification provenance** — every automated decision records its source (XML structure, pattern match, fiscal year comparison, default rule)

**Input:** `extraction.json` + `BILLS-*.xml`
**Output:** `bill_meta.json`
**Requires:** Nothing — no API keys, no network access.

### Stage 4: Embed

The `embed` command generates semantic embedding vectors for every provision using OpenAI's `text-embedding-3-large` model. Each provision is represented by concatenating its meaningful fields:

```
Account: Child Nutrition Programs | Agency: Department of Agriculture | Text: For necessary expenses...
```

This combined text is embedded into a 3,072-dimensional vector that captures the provision's meaning. Provisions about similar topics (even with completely different wording) will have vectors pointing in similar directions — enabling semantic search.

**Input:** `extraction.json`
**Output:** `embeddings.json` (metadata) + `vectors.bin` (binary float32 vectors)
**Requires:** `OPENAI_API_KEY`

### Stage 5: Query

All query operations (`search`, `summary`, `compare`, `audit`, `relate`) run locally against the JSON and binary files on disk. No API calls at query time — except `--semantic` search, which makes one small API call to embed the query text.

The `--fy` and `--subcommittee` flags on `summary`, `search`, and `compare` use `bill_meta.json` for fiscal year filtering and jurisdiction scoping. The `--show-advance` flag on `summary` uses `bill_meta.json` to separate current-year from advance budget authority.

### Stage 6: Link (Optional)

The `link suggest` command computes cross-bill provision relationships using embedding similarity and account name matching. Candidates are classified by confidence tier (verified, high, uncertain) and persisted via `link accept`.

**What it produces:**
- **Link candidates** — pairs of provisions across different bills with similarity scores and confidence tiers
- **Accepted links** — user-reviewed relationships stored in `links/links.json` at the data root
- **Link hashes** — deterministic 8-char hex identifiers (from SHA-256 of source, target, and model) that remain stable across runs

**Input:** `embeddings.json` + `vectors.bin` + `bill_meta.json` (for account normalization)
**Output:** `links/links.json`
**Requires:** Pre-computed embeddings. No API keys at suggest/accept time.

Links are consumed by `compare --use-links` (rescues orphans via accepted links) and `relate` (shows link hashes for future persistence).

---

## Data Directory Layout

Every bill lives in its own directory. Files are discovered by walking recursively for `extraction.json` — that's the anchor file. Everything else is optional. Cross-bill links live at the data root in `links/links.json`.

```
data/                              ← any --dir path works
├── hr4366/                        ← bill directory
│   ├── BILLS-118hr4366enr.xml     ← source XML from Congress.gov
│   ├── extraction.json            ← structured provisions (REQUIRED)
│   ├── verification.json          ← deterministic verification
│   ├── metadata.json              ← model, prompt version, hashes
│   ├── tokens.json                ← token usage from extraction
│   ├── bill_meta.json             ← bill metadata: FY, jurisdictions, advance (enrich)
│   ├── embeddings.json            ← embedding metadata (model, dimensions, hashes)
│   ├── vectors.bin                ← raw float32 embedding vectors
│   └── chunks/                    ← per-chunk LLM artifacts (gitignored)
│       ├── 01KKWW9T5RR0JTQ6C9FYYE96A8.json
│       └── ...
├── hr5860/
│   └── ...
├── hr9468/
│   └── ...
└── links/                         ← cross-bill relationships (link accept)
    └── links.json                 ← append-only via link accept/remove
```

| File | Required | Written by | Read by | Mutated after creation? |
|------|----------|-----------|---------|------------------------|
| `BILLS-*.xml` | For extraction | `download` | `extract`, `upgrade`, `enrich` | Never |
| `extraction.json` | **Yes** | `extract` | All query commands | Never (unless deliberately re-extracted) |
| `verification.json` | No | `extract`, `upgrade` | `audit`, `search` quality | Never |
| `metadata.json` | No | `extract` | Staleness detection | Never |
| `tokens.json` | No | `extract` | Informational | Never |
| `bill_meta.json` | No | `enrich` | `--subcommittee`, `--show-advance`, `relate` | Only by re-enrich |
| `embeddings.json` | No | `embed` | Semantic search, `link suggest` | Never (unless re-embedded) |
| `vectors.bin` | No | `embed` | Semantic search, `link suggest`, `relate` | Never (unless re-embedded) |
| `chunks/*.json` | No | `extract` | Analysis/debugging | Never |
| `links/links.json` | No | `link accept` | `compare --use-links`, `link list` | Append-only (accept adds, remove deletes) |

**Every file is write-once.** Once a bill is extracted and embedded, its files are never modified. The system is read-dominated: writes happen ~15 times per year (when Congress enacts bills), reads happen hundreds to thousands of times.

Nesting is flexible — `data/congress/118/hr4366/extraction.json` works just as well as `data/118-hr4366/extraction.json`. The loader walks recursively from whatever `--dir` you point it at.

---

## The Hash Chain

Each downstream artifact records the SHA-256 hash of its input. This enables **staleness detection**: if someone re-downloads the XML or re-extracts with a new model, all downstream artifacts are detectable as potentially stale.

```
BILLS-*.xml ──sha256──▶ metadata.json (source_xml_sha256)
                              │
extraction.json ──sha256──▶ bill_meta.json (extraction_sha256)
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
                              │
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

The `bill_meta.json` link was added in v4.0 — when the extraction changes, provision indices in `bill_meta.json` may no longer be valid. The staleness check warns: "bill metadata is stale (extraction.json has changed). Run `enrich --force`."

The `staleness.rs` module checks this chain on commands that use embeddings. If a hash mismatches, it prints a warning to stderr but never blocks execution:

```
⚠ H.R. 4366: embeddings are stale (extraction.json has changed)
```

Hashing all files for 25 bills takes ~8ms. At 60 bills, ~40ms. There is no performance reason to skip or cache hash checks.

---

## Embedding Storage Format

Embeddings use a split format: JSON metadata + binary vectors.

**`embeddings.json`** (~200 bytes, human-readable):
```json
{
  "schema_version": "1.0",
  "model": "text-embedding-3-large",
  "dimensions": 3072,
  "count": 2364,
  "extraction_sha256": "ae912e3427b8...",
  "vectors_file": "vectors.bin",
  "vectors_sha256": "7bd7821176bc..."
}
```

**`vectors.bin`** (count × dimensions × 4 bytes, binary):
Raw little-endian float32 array. No header. Dimensions and count come from the JSON metadata. Loaded in Rust via `std::fs::read()` + byte-to-float conversion.

**Why binary for vectors:** At 3,072 dimensions × 2,364 provisions, the binary file is 29 MB and loads in <3ms. The same data as JSON float arrays would be ~170 MB and take ~500ms to parse in Rust. Since this is a read-heavy system (load once per CLI invocation, query many times), the binary format keeps startup instant.

**Why JSON for metadata:** The metadata is tiny and must be human-inspectable for debugging and provenance. `cat embeddings.json` tells you what model was used, how many provisions are embedded, and what extraction they correspond to.

---

## Semantic Search

Semantic search lets users find provisions by meaning rather than keywords. The query "school lunch programs for kids" finds "Child Nutrition Programs" even though the words don't overlap — because the *meaning* is similar.

### How it works

1. **At embed time:** Each provision's text is sent to OpenAI's `text-embedding-3-large` model, which returns a 3,072-dimensional vector representing its meaning. These vectors are stored in `vectors.bin`.

2. **At query time:** The user's search query is embedded using the same model (single API call, ~100ms). The query vector is compared to every provision vector using cosine similarity (dot product of normalized vectors). Results are ranked by similarity and filtered by any hard constraints (--type, --division, --min-dollars, --fy, --subcommittee, etc.).

3. **Performance:** Cosine similarity over 11,000+ vectors takes <0.5ms. The bottleneck is loading the binary files (~8ms for 25 bills) and the single API call to embed the query (~100ms). Total: ~110ms per search.

### Similarity scores

OpenAI embedding vectors are L2-normalized (norm = 1.0), so cosine similarity equals the dot product. Scores range from -1 to 1 in theory, but in practice for this data:

| Score | Interpretation |
|-------|---------------|
| > 0.80 | Same account/program across bills |
| 0.60–0.80 | Related topic, same policy area |
| 0.45–0.60 | Loosely related concepts |
| < 0.45 | Unlikely to be meaningfully related |

### Find-similar

`--similar 118-hr4366:42` takes provision #42's embedding vector and finds the most similar provisions across all loaded bills. This enables:
- **Cross-bill matching:** find the same program in a different bill
- **Year-over-year tracking:** find last year's version of this provision
- **Conference tracking:** match House and Senate versions

---

## Module Map

### Core data types

| Module | Lines | Purpose |
|--------|-------|---------|
| `ontology.rs` | ~900 | All data types. The `Provision` enum has 11 variants (Appropriation, Rescission, TransferAuthority, Limitation, DirectedSpending, CrSubstitution, MandatorySpendingExtension, Directive, Rider, ContinuingResolutionBaseline, Other). Also defines `BillExtraction`, `TextSpan`, `TextMatchTier`, `DollarAmount`, `AmountSemantics`, `BillClassification`, `ExtractionMetadata`, and all supporting types. Dead types removed in v6.0.0 (Relationship, RelationType, ValidationReport, etc.). |
| `from_value.rs` | ~690 | Resilient JSON → Provision deserialization. Handles LLM output variance: missing fields get defaults, unexpected types are coerced, unknown provision types become `Other`. This is why extraction rarely fails even when the LLM returns imperfect JSON. |
| `bill_meta.rs` | ~1360 | Bill-level metadata types and classification functions. `BillMeta`, `BillNature`, `Jurisdiction`, `SubcommitteeMapping`, `ProvisionTiming`, `FundingTiming`, `CanonicalAccount`, `ClassificationSource`. Includes XML division parsing via `roxmltree`, jurisdiction classification via pattern matching, fiscal-year-aware advance appropriation detection, account name normalization, and bill nature classification. No external dependencies — runs entirely offline. 33 unit tests. |
| `links.rs` | ~790 | Persistent cross-bill provision links. `LinksFile`, `AcceptedLink`, `LinkCandidate`, `LinkConfidence`, `LinkRelationship`, `LinkEvidence`, `ProvisionRef`. Core `suggest()` algorithm computes cross-bill similarity for all BA provisions using calibrated thresholds (0.55/0.65). `accept_links()` and `remove_links()` for link management. `load_links()` / `save_links()` for I/O at `<dir>/links/links.json`. 10 unit tests. |

### TAS resolution and authority system (NEW in v6.0.0)

| Module | Lines | Purpose |
|--------|-------|---------|
| `tas.rs` | ~1190 | Treasury Account Symbol resolution. `FasReference` loads `fas_reference.json` (2,768 FAS codes from the FAST Book). Two-tier matching: `match_deterministic()` tries 4 strategies (direct, short-title, suffix after em-dash strip, agency disambiguation with DOD service branch detection); LLM tier sends unmatched provisions to Claude Opus in agency-scoped batches. `apply_llm_results()` verifies every LLM response against the FAST Book. `agency_name_to_code()` maps ~80 LLM-extracted agency names to CGAC codes. Produces `tas_mapping.json` per bill. 16 unit tests. |
| `authority.rs` | ~1050 | Account authority registry. `build_authorities()` aggregates all `tas_mapping.json` files into `authorities.json` — one `AccountAuthority` per FAS code with provision references, name variants, fiscal year coverage, and dollar totals. `classify_variants_and_detect_events()` classifies name differences (case, prefix, name_change, inconsistent) and detects rename events with fiscal year boundaries. `build_timeline()` groups provisions by FY for the `trace` command. `search_authorities()` does word-level matching across title, agency, and variants. 11 unit tests. |
| `text_repair.rs` | ~675 | Source text verification and repair (the `verify-text` command). 3-tier deterministic algorithm: `try_prefix_match()` → `try_substring_match()` → `try_normalized_match()`. Each tier copies directly from the source text, guaranteeing the result is verbatim. `build_position_map()` creates a normalized-to-original byte offset map for Tier 3. `verify_and_repair_bill_json()` works at the `serde_json::Value` level to write inline `source_span` on each provision. 9 unit tests. |

### Extraction pipeline

| Module | Lines | Purpose |
|--------|-------|---------|
| `extraction.rs` | ~860 | `ExtractionPipeline` — orchestrates parallel LLM chunk extraction, merges results, builds metadata. Contains `build_metadata()` which computes the source XML hash for the hash chain. `chunks_total` and `chunks_completed` are mandatory fields in metadata (v6.0.0). |
| `xml.rs` | ~590 | Congressional bill XML parsing via `roxmltree`. Extracts clean text, identifies `<appropriations-major>` headings, and splits into chunks at division/title boundaries. |
| `text_index.rs` | ~670 | Builds a positional index of every dollar amount (`$X,XXX,XXX`), section header, and proviso clause in the source text. Used by verification and for chunk boundary computation. |
| `prompts.rs` | ~310 | The system prompt sent to Claude for extraction. Also contains `TAS_SYSTEM_PROMPT` for TAS resolution. Defines every provision type, shows real JSON examples, constrains output format. |
| `progress.rs` | ~170 | Extraction progress bar rendering. |

### Verification and quality

| Module | Lines | Purpose |
|--------|-------|---------|
| `verification.rs` | ~370 | Deterministic post-extraction verification. Three checks: (1) dollar amount strings found in source text, (2) raw_text matched via three-tier system (exact → normalized → spaceless), (3) completeness — how many dollar references in the source were accounted for. No LLM involved. Note: `text_repair.rs` provides a separate, more thorough verification with repair capability. |
| `staleness.rs` | ~165 | Hash chain integrity checking. Compares stored SHA-256 hashes to current file contents. Variants: `ExtractionStale`, `EmbeddingsStale`, `BillMetaStale`. Returns warnings for stale artifacts — never blocks execution. |

### Entity resolution

| Module | Lines | Purpose |
|--------|-------|---------|
| `normalize.rs` | ~1100 | Entity resolution types and algorithms. `DatasetFile` / `AgencyGroup` / `AccountAlias` for `dataset.json`. `suggest_text_match()` discovers agency naming variants via orphan-pair analysis. `get_xml_context()` extracts structural XML headings for LLM disambiguation. `merge_groups()` for the suggest/accept pattern. 15 unit tests. Note: for cross-bill account matching, TAS resolution (`tas.rs`) is now the preferred approach; `normalize` remains useful for `compare` output formatting. |
| `cache.rs` | ~420 | Suggest/accept workflow caching at `~/.congress-approp/cache/`. `write_suggestions()` / `read_suggestions()` with SHA-256-based auto-invalidation when extraction files change. Used by both `normalize` and `link` suggest/accept commands. |

### Query and search

| Module | Lines | Purpose |
|--------|-------|---------|
| `query.rs` | ~1600 | The library API. Functions: `summarize()`, `search()`, `compare()`, `audit()`, `rollup_by_department()`, `build_embedding_text()`, `relate()`, `compute_link_hash()`. Also contains `normalize_agency()` and `normalize_account_name()`. The `compare()` function includes cross-semantics orphan rescue. All functions take `&[LoadedBill]` and return plain data structs. No I/O, no formatting. |
| `loading.rs` | ~340 | Directory walking via `walkdir`, JSON deserialization, assembly of `LoadedBill` structs. Finds `extraction.json` recursively, loads sibling artifacts including `bill_meta.json`. |
| `embeddings.rs` | ~260 | Embedding storage: `load()` / `save()` for the JSON metadata + binary vectors format. `embeddings.json` contains metadata (model, dimensions, count, hashes); `vectors.bin` contains packed float32 vectors. Also provides `cosine_similarity()`, `normalize()`, and `top_n_similar()` functions for vector search. |
| `inflation.rs` | ~430 | CPI data loading from bundled `cpi.json`, fiscal-year-weighted average computation, real delta calculation, and inflation flags (real_increase, real_cut, inflation_erosion). Used by `compare --real`. |

### API clients

| Module | Lines | Purpose |
|--------|-------|---------|
| `api/congress/` | ~850 | Congress.gov API client. Bill listing, metadata lookup, text download. |
| `api/anthropic/` | ~660 | Anthropic API client. Message creation with streaming, thinking support, caching. Used by both `extract` and `resolve-tas`. |
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

All artifacts except `links/links.json` are immutable after creation. The links file is append-only (`link accept` adds entries, `link remove` deletes them). This means:
- No file locking needed
- No database needed — JSON files on disk are the right abstraction
- No caching needed — the files ARE the cache
- Hash checks are free (~2ms) and should run on every load

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Load 25 bills (extraction.json) | ~40ms | JSON parsing |
| Load embeddings (25 bills, binary) | ~8ms | Raw byte read |
| Hash all files (25 bills) | ~8ms | SHA-256 |
| Cosine search (8,500 provisions) | <0.5ms | Dot product |
| Enrich all 25 bills | <1s | XML parsing + classification |
| Link suggest (25 bills, all scope) | ~4s | O(n²) pairwise comparison |
| **Total cold-start query** | **~50ms** | Load + hash + search |
| Embed query text (OpenAI API) | ~100ms | Network round-trip |
| Full extraction (omnibus, 75 chunks) | ~60 min | Parallel LLM calls |
| Generate embeddings (2,500 provisions) | ~30 sec | Batch API calls |

At 20 congresses (~60 bills, ~15,000 provisions): cold start ~100ms, search <1ms, link suggest ~50s. The system scales linearly for query operations and quadratically for link suggest (which is run infrequently).

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