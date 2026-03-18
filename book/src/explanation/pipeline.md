# The Extraction Pipeline

A bill flows through six stages on its way from raw XML on Congress.gov to queryable, verified, searchable data on your machine. Each stage produces immutable files. Once a stage completes for a bill, its output is never modified — unless you deliberately re-extract or upgrade.

This chapter explains each stage in detail: what it does, what it produces, and why it's designed the way it is.

## Pipeline Overview

```text
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
                    │ Enrich   │ ───▶ bill_meta.json          (offline, no API)
                    │(optional)│
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

**Only stages 3 (Extract) and 5 (Embed) call external APIs.** Everything else — downloading, parsing, enrichment, verification, linking, querying — runs locally and deterministically.

## Stage 1: Download

The `download` command fetches enrolled bill XML from the Congress.gov API.

**What "enrolled" means:** When a bill passes both the House and Senate in identical form and is sent to the President for signature, that final text is the "enrolled" version. Once signed, it becomes law. This is the authoritative text — the version that actually governs how money is spent.

**What the XML looks like:** Congressional bill XML uses semantic markup defined by the Government Publishing Office (GPO). Tags like `<division>`, `<title>`, `<section>`, `<appropriations-major>`, `<appropriations-small>`, `<quote>`, and `<proviso>` describe the legislative structure, not just formatting. This semantic markup is what makes reliable parsing possible — you can identify account name headings, dollar amounts, proviso clauses, and structural boundaries directly from the XML tree.

**What gets created:**

```text
data/118/hr/9468/
└── BILLS-118hr9468enr.xml     ← Enrolled bill XML from Congress.gov
```

**Requires:** `CONGRESS_API_KEY` (free from [api.congress.gov](https://api.congress.gov/sign-up/))

**No transformation is applied.** The XML is saved exactly as received from Congress.gov.

## Stage 2: Parse

Parsing happens at the beginning of the `extract` command — it's not a separate CLI step. The `xml.rs` module reads the bill XML using `roxmltree` (a pure-Rust XML parser with no C dependencies) and produces two things:

### Clean text extraction

The parser walks the XML tree and extracts human-readable text with two important conventions:

1. **Quote delimiters:** Account names in bill XML are wrapped in `<quote>` tags. The parser renders these as `''Account Name''` (double single-quotes) to match the format the LLM system prompt expects. For example:

   ```xml
   <quote>Compensation and Pensions</quote>
   ```

   becomes:

   ```text
   ''Compensation and Pensions''
   ```

2. **Structural markers:** Division headers, title headers, and section numbers are preserved in the clean text so the LLM can identify structural boundaries.

### Chunk boundaries

Large bills need to be split into smaller pieces for the LLM — you can't send a 1,500-page omnibus as a single prompt. The parser identifies **semantic chunk boundaries** by walking the XML tree structure:

- **Primary splits:** At `<division>` boundaries (Division A, Division B, etc.)
- **Secondary splits:** At `<title>` boundaries within each division
- **Tertiary splits:** If a single title or division still exceeds the maximum chunk token limit (~3,000 tokens), it's further split at paragraph boundaries

This is **semantic chunking**, not arbitrary token-limit splitting. Each chunk contains a complete legislative section — a full title or division — so the LLM sees complete context. This matters because provisions often reference "the amount made available under this heading" or "the previous paragraph," and the LLM needs to see those references in context.

**Chunk counts for the example data:**

| Bill | XML Size | Chunks |
|------|----------|--------|
| H.R. 9468 (supplemental) | 9 KB | 1 |
| H.R. 5860 (CR) | 131 KB | 5 |
| H.R. 4366 (omnibus) | 1.8 MB | 75 |

**No files are written.** The clean text and chunk boundaries exist only in memory, passed directly to the extraction stage.

**No API calls.** Pure Rust computation.

## Stage 3: Extract

This is the core stage — the only one that uses an LLM. Each chunk of bill text is sent to Claude with a detailed system prompt (~300 lines) that defines every provision type, shows real JSON examples, constrains the output format, and includes specific instructions for edge cases. The LLM reads the actual legislative language and produces structured JSON — there is no intermediate regex extraction step.

### The system prompt

The system prompt (defined in `prompts.rs`) is the instruction manual for the LLM. It covers:

- **Reading instructions:** How to interpret `''Account Name''` delimiters, dollar amounts, "Provided, That" provisos, "notwithstanding" clauses, and section numbering
- **Bill type guidance:** How regular appropriations, continuing resolutions, omnibus bills, and supplementals differ
- **Provision type definitions:** All 11 types (appropriation, rescission, transfer_authority, limitation, directed_spending, cr_substitution, mandatory_spending_extension, directive, rider, continuing_resolution_baseline, other) with examples
- **Detail level rules:** When to classify a provision as top_level, line_item, sub_allocation, or proviso_amount
- **Sub-allocation semantics:** Explicit instructions that "of which $X shall be for..." breakdowns are `reference_amount`, not `new_budget_authority`
- **CR substitution requirements:** Both the new and old amounts must be extracted with dollar values, semantics, and text_as_written
- **Output format:** The exact JSON schema the LLM must produce

The prompt is sent with `cache_control` enabled, so subsequent chunks within the same bill benefit from prompt caching — the system prompt tokens are served from cache rather than re-processed, reducing both latency and cost.

### Parallel chunk processing

Chunks are extracted in parallel using bounded concurrency (default 5 simultaneous LLM calls, configurable via `--parallel`). A progress dashboard shows real-time status:

```text
  5/42, 187 provs [4m 23s] 842 tok/s | 📝A-IIb ~8K 180/s | 🤔B-I ~3K | 📝B-III ~1K 95/s
```

Each chunk produces a JSON array of provisions. The LLM's response is captured along with its "thinking" content (internal reasoning) and saved to the `chunks/` directory as a permanent provenance record.

### Resilient JSON parsing

The LLM doesn't always produce perfect JSON. Missing fields, wrong types, unexpected enum values, extra fields — all of these can occur. The `from_value.rs` module handles this with a resilient parsing strategy:

- **Missing fields** get defaults (empty string, null, empty array)
- **Wrong types** are coerced where possible (string `"$10,000,000"` → integer `10000000`)
- **Unknown provision types** become `Provision::Other` with the LLM's original classification preserved
- **Extra fields** on known types are silently ignored
- **Failed provisions** are logged but don't abort the extraction

Every compromise is counted in a `ConversionReport` — you can see exactly how many null-to-default conversions, type coercions, and unknown types occurred.

### Merge and compute

After all chunks complete:

1. **Provisions are merged** into a single flat array, ordered by chunk sequence
2. **Budget authority totals are computed** from the individual provisions — summing `new_budget_authority` provisions at `top_level` and `line_item` detail levels. The LLM also produces a summary with totals, but these are **never used for computation** — only for diagnostics. This design means a bug in the LLM's arithmetic can't corrupt budget totals.
3. **Chunk provenance** is recorded — the `chunk_map` field in `extraction.json` links each provision back to the chunk it came from

### Deterministic verification

Verification runs immediately after extraction, with no LLM involvement. It answers three questions:

1. **"Are the dollar amounts real?"** — For every provision with a `text_as_written` dollar string (e.g., `"$2,285,513,000"`), search for that exact string in the source bill text. Result: `verified` (found once), `ambiguous` (found multiple times), or `not_found`.

2. **"Is the quoted text actually from the bill?"** — For every provision's `raw_text` excerpt, check if it's a substring of the source text using tiered matching:
   - **Exact:** Byte-identical substring (95.6% of provisions in example data)
   - **Normalized:** Matches after collapsing whitespace and normalizing Unicode quotes/dashes (2.8%)
   - **Spaceless:** Matches after removing all spaces (0%)
   - **No match:** Not found at any tier (1.5% — all non-dollar statutory amendments)

3. **"Did we miss anything?"** — Count every dollar-sign pattern in the source text and check how many are accounted for by extracted provisions. This produces the coverage percentage.

See [How Verification Works](./verification.md) for the complete technical details.

### What gets created

```text
data/118/hr/9468/
├── BILLS-118hr9468enr.xml     ← Source XML (unchanged)
├── extraction.json            ← All provisions, bill info, summary, chunk map
├── verification.json          ← Amount checks, raw text checks, completeness
├── metadata.json              ← Model name, prompt version, timestamps, source hash
├── tokens.json                ← Input/output/cache token counts per chunk
└── chunks/                    ← Per-chunk LLM artifacts (gitignored)
    ├── 01JRWN9T5RR0JTQ6C9FYYE96A8.json
    └── ...
```

**Requires:** `ANTHROPIC_API_KEY`

## Stage 3.5: Enrich (Optional)

The `enrich` command generates bill-level metadata by parsing the source XML structure and analyzing the already-extracted provisions. It bridges the gap between raw extraction and informed querying — adding structural knowledge that the LLM extraction doesn't capture.

**Why this stage exists:** The LLM extracts provisions faithfully — every dollar amount, every account name, every section reference. But it doesn't know that Division A in H.R. 7148 covers Defense while Division A in H.R. 6938 covers CJS. It doesn't know that "shall become available on October 1, 2024" in a FY2024 bill means the money is for FY2025 (an advance appropriation). It doesn't know that "Grants-In-Aid for Airports" and "Grants-in-Aid for Airports" are the same account. The `enrich` command adds this structural and normalization knowledge.

**What it does:**

1. **Parses division titles from XML.** The enrolled bill XML contains `<division><enum>A</enum><header>Department of Defense Appropriations Act, 2026</header>` elements. The enrich command extracts each division's letter and title, then classifies the title to a jurisdiction using case-insensitive pattern matching against known subcommittee names.

2. **Classifies advance vs current-year.** For each budget authority provision, the command checks the `availability` field and `raw_text` for "October 1, YYYY" or "first quarter of fiscal year YYYY" patterns. It compares the referenced year to the bill's fiscal year: if the money becomes available after the bill's FY ends, it's advance.

3. **Normalizes account names.** Each account name is lowercased and stripped of hierarchical em-dash prefixes (e.g., "Department of VA—Compensation and Pensions" → "compensation and pensions") for cross-bill matching.

4. **Classifies bill nature.** The provision type distribution and subcommittee count determine whether the bill is an omnibus (5+ subcommittees), minibus (2-4), full-year CR with appropriations (CR baseline + hundreds of regular appropriations), or other type.

**Input:** `extraction.json` + `BILLS-*.xml`
**Output:** `bill_meta.json`
**Requires:** Nothing — no API keys, no network access.

This stage is optional. All commands from v3.x continue to work without it. It is required for `--subcommittee` filtering, `--show-advance` display, and enriched bill classification display. See [Enrich Bills with Metadata](../how-to/enrich-data.md) for a complete guide.

## Stage 4: Embed

The `embed` command generates semantic embedding vectors for every provision using OpenAI's `text-embedding-3-large` model. This is the foundation for meaning-based search and cross-bill matching.

### How provision text is built

Each provision is represented as a concatenation of its meaningful fields:

```text
Account: Child Nutrition Programs | Agency: Department of Agriculture | Text: For necessary expenses of the Food and Nutrition Service...
```

This construction is deterministic — the same provision always produces the same embedding text, computed by `query::build_embedding_text()`. The exact fields included depend on the provision type:

- **Appropriations/Rescissions:** Account name, agency, program, raw text
- **CR Substitutions:** Account name, reference act, reference section, raw text
- **Directives/Riders:** Description, raw text
- **Other types:** Description or LLM classification, raw text

### Batch processing

Provisions are sent to the OpenAI API in batches (default 100 provisions per call). Each call returns a vector of 3,072 floating-point numbers per provision — the embedding that captures the provision's meaning in high-dimensional space.

All vectors are L2-normalized (unit length), which means cosine similarity equals the simple dot product — a fast computation.

### Binary storage

Embeddings are stored in a split format for efficiency:

- **`embeddings.json`** (~200 bytes): Human-readable metadata — model name, dimensions, count, and SHA-256 hashes for the hash chain
- **`vectors.bin`** (count × 3,072 × 4 bytes): Raw little-endian float32 array with no header

For the FY2024 omnibus (2,364 provisions), `vectors.bin` is 29 MB and loads in under 2 milliseconds. The same data as JSON float arrays would be ~57 MB and take ~175ms to parse. Since this is a read-heavy system — load once per CLI invocation, query many times — the binary format keeps startup instant.

### What gets created

```text
data/118/hr/9468/
├── ...existing files...
├── embeddings.json            ← Metadata: model, dimensions, count, hashes
└── vectors.bin                ← Raw float32 vectors [count × 3072]
```

**Requires:** `OPENAI_API_KEY`

## Stage 5: Query

All query operations — `search`, `summary`, `compare`, `audit` — run locally against the JSON and binary files on disk. There are no API calls at query time, with one exception: `search --semantic` makes a single API call to embed your query text (~100ms).

### How queries work

1. **Load:** `loading.rs` recursively walks the `--dir` path, finds every `extraction.json`, and deserializes it along with sibling files (`verification.json`, `metadata.json`) into `LoadedBill` structs.

2. **Filter:** For `search` queries, each provision is tested against the specified filters (type, agency, account, keyword, division, dollar range). All filters use AND logic.

3. **Rank:** For semantic searches, the query text is embedded via OpenAI, and cosine similarity is computed against every matching provision's pre-stored vector. For `--similar`, the source provision's stored vector is used directly (no API call).

4. **Compute:** For `summary`, budget authority and rescissions are computed from provisions. For `compare`, accounts are matched by `(agency, account_name)` and deltas are calculated. For `audit`, verification metrics are aggregated.

5. **Format:** The CLI layer (`main.rs`) renders results as tables, JSON, JSONL, or CSV depending on the `--format` flag.

### Performance

All of this is fast:

| Operation | Time | Notes |
|-----------|------|-------|
| Load 13 bills (extraction.json) | ~40ms | JSON parsing |
| Load embeddings (13 bills, binary) | ~8ms | Memory read |
| Hash all files (13 bills) | ~8ms | SHA-256 |
| Cosine search (8,500 provisions) | <0.5ms | Dot products |
| **Total cold-start query** | **~50ms** | Load + hash + search |
| Embed query text (OpenAI API) | ~100ms | Network round-trip |

At 20 congresses (~60 bills, ~15,000 provisions): cold start ~100ms, search <1ms. The system scales linearly and stays interactive at any realistic data volume.

**No API calls at query time** unless you use `--semantic` (one call to embed the query). The `--similar` command uses only stored vectors — completely offline.

## The Write-Once Principle

Every file in the pipeline is **write-once**. After a bill is extracted and embedded, its files are never modified (unless you deliberately re-extract or upgrade). This design has several advantages:

- **No file locking needed.** Multiple processes can read simultaneously without coordination.
- **No database needed.** JSON files on disk are the right abstraction for a read-dominated workload with ~15 writes per year (when Congress enacts bills) and thousands of reads.
- **No caching needed.** The files ARE the cache. There's nothing to invalidate.
- **Git-friendly.** All files are diffable JSON (except `vectors.bin`, which is gitattributed as binary).
- **Trivially relocatable.** Copy a bill directory anywhere and it works — no registry, no config, no state files outside the directory.

The one exception to strict immutability is the `links/links.json` file, which is append-only for accepted cross-bill relationships. Links are added via `link accept` and removed via `link remove`, but the file is never overwritten — only updated.

## The Hash Chain

Each downstream artifact records the SHA-256 hash of its input, forming a chain that enables staleness detection:

```text
BILLS-*.xml ──sha256──▶ metadata.json (source_xml_sha256)
                              │
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
                              │
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

If you re-download the XML (producing a new file), `metadata.json` still references the old hash. If you re-extract (producing a new `extraction.json`), `embeddings.json` still references the old extraction hash. The `staleness.rs` module checks these hashes on commands that use embeddings and prints warnings:

```text
⚠ H.R. 4366: embeddings are stale (extraction.json has changed)
```

Warnings are advisory — they never block execution. Hashing all files for 13 bills takes ~8ms, so there's no performance reason to skip checks.

See [Data Integrity and the Hash Chain](./hash-chain.md) for more details.

## Dependencies

The pipeline uses a minimal set of Rust crates:

| Stage | Key Crate | Role |
|-------|-----------|------|
| Download | `reqwest` | HTTP client for Congress.gov API |
| Parse | `roxmltree` | Pure-Rust XML parsing, zero-copy where possible |
| Extract | `reqwest` + `tokio` | Async HTTP for Anthropic API with parallel chunk processing |
| Parse LLM output | `serde_json` | JSON deserialization with custom resilient parsing |
| Verify | `sha2` | SHA-256 hashing for the hash chain |
| Embed | `reqwest` | HTTP client for OpenAI API |
| Query | `walkdir` | Recursive directory traversal to find bill data |
| Output | `comfy-table` + `csv` | Terminal table formatting and CSV export |

All API clients use `rustls-tls` (pure Rust TLS) — no OpenSSL dependency.

## What Can Go Wrong

Understanding the pipeline helps you diagnose issues:

| Symptom | Likely Stage | Investigation |
|---------|-------------|---------------|
| "No XML files found" | Download | Check that `BILLS-*.xml` exists in the directory |
| Low provision count | Extract | Check `audit` coverage; examine chunk artifacts in `chunks/` |
| NotFound > 0 in audit | Extract + Verify | Run `audit --verbose`; check if the LLM hallucinated an amount |
| "Embeddings are stale" | Embed | Run `embed` to regenerate after re-extraction |
| Semantic search returns no results | Embed | Check that `embeddings.json` and `vectors.bin` exist |
| Budget authority doesn't match expectations | Extract | Check detail_level and semantics; see [Budget Authority Calculation](./budget-authority.md) |

## Next Steps

- **[How Verification Works](./verification.md)** — deep dive into the three verification checks
- **[How Semantic Search Works](./semantic-search.md)** — embeddings, cosine similarity, and vector storage
- **[Budget Authority Calculation](./budget-authority.md)** — exactly how totals are computed from provisions
- **[Data Integrity and the Hash Chain](./hash-chain.md)** — staleness detection across the pipeline