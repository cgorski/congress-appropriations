# Architecture

`congress-appropriations` is a Rust crate (library + CLI binary) that downloads,
parses, extracts, verifies, and queries U.S. federal appropriations bills. The
extraction step uses an LLM (Claude); every other stage is deterministic. The
crate publishes a library API (`congress_appropriations::{load_bills, query}`) so
downstream tools can query extracted data without the CLI.

---

## Data Directory Layout

A **data root** (e.g. `examples/`, `data/`) contains one or more **bill
directories** at any nesting depth. Discovery walks the tree looking for
`extraction.json` files; each directory that contains one is treated as a bill.

```
<data-root>/
  <bill-dir>/                     # e.g. hr4366/
    BILLS-*.xml                   # Source XML from Congress.gov
    extraction.json               # Required — provisions, summary, bill info
    verification.json             # Optional — deterministic quality report
    metadata.json                 # Optional — model, prompt version, timestamps
    tokens.json                   # Optional — token usage stats
    chunks/                       # Per-chunk LLM artifacts (thinking, raw response, conversion report)
```

| File                | Required | Producer         | Consumer                |
|---------------------|----------|------------------|-------------------------|
| `BILLS-*.xml`       | For extraction | `download` cmd | `extract` cmd           |
| `extraction.json`   | **Yes**  | `extract` cmd    | All query/verify stages |
| `verification.json` | No       | `extract` / `upgrade` | `audit`, `search` quality |
| `metadata.json`     | No       | `extract` cmd    | `upgrade`, diagnostics  |
| `tokens.json`       | No       | `extract` cmd    | Cost tracking           |

Nesting is arbitrary — `data/fy2024/defense/hr4366/extraction.json` works fine.
The loader (`loading.rs`) uses `walkdir` to find every `extraction.json`
recursively, then deserializes the sibling artifacts.

---

## Pipeline Stages

```
1. Download ──→ 2. Parse ──→ 3. Extract ──→ 4. Verify ──→ 5. Query
   (API)          (XML)        (LLM)         (determ.)     (determ.)
```

### 1. Download
Fetches enrolled-bill XML from the Congress.gov API. Requires a
`CONGRESS_API_KEY`. Output: `BILLS-*.xml`.

### 2. Parse
`xml.rs` parses the XML with `roxmltree`, extracts clean text, and computes
chunk boundaries at division/title breaks so each chunk preserves semantic
context. `text_index.rs` builds a dollar-reference index over the full text
for verification.

### 3. Extract
`extraction.rs` sends chunks to Claude (Anthropic API) in parallel, guided by
the system prompt in `prompts.rs` (~300 lines of structured instructions). Each
chunk produces a list of `Provision` values. Results are merged, deduplicated,
and written to `extraction.json`. **This is the only stage that calls an LLM.**

### 4. Verify
`verification.rs` runs deterministic checks against the source XML text — no
LLM involved. It validates dollar amounts, matches `raw_text` fields back to
source, and computes a completeness metric. Output: `verification.json`.

### 5. Query
`query.rs` operates entirely on the loaded JSON artifacts. Provides search,
compare, summarize, rollup-by-agency, and audit functions. The CLI formats
results as tables or CSV; the library returns plain data structs.

---

## Module Map

| Module            | Purpose |
|-------------------|---------|
| `ontology.rs`     | All data types: `Provision` enum (11 variants), `BillExtraction`, `DollarAmount`, `AmountSemantics`, `BillClassification`, etc. |
| `extraction.rs`   | `ExtractionPipeline` — LLM interaction, parallel chunk processing, result merging |
| `from_value.rs`   | Resilient `serde_json::Value` → `Provision` deserialization; handles LLM output variance (missing fields, unexpected types) |
| `text_index.rs`   | Dollar-reference indexing over source text, section detection, chunk boundary computation |
| `xml.rs`          | Congressional bill XML parsing via `roxmltree` |
| `verification.rs` | Deterministic post-extraction verification (amount checks, raw-text matching, completeness) |
| `prompts.rs`      | System prompt and extraction instructions (~300 lines) |
| `loading.rs`      | Directory walking, JSON deserialization, `LoadedBill` assembly |
| `query.rs`        | Search, compare, summarize, audit — the library API surface |
| `progress.rs`     | Extraction progress display |

### Provision Variants

`Appropriation`, `Rescission`, `TransferAuthority`, `Limitation`,
`DirectedSpending`, `CrSubstitution`, `MandatorySpendingExtension`,
`Directive`, `Rider`, `ContinuingResolutionBaseline`, `Other`.

---

## Library API

The crate re-exports `load_bills` and the `query` module for programmatic use:

```rust
use congress_appropriations::{load_bills, query};
use congress_appropriations::query::SearchFilter;
use std::path::Path;

// Load all bills under a data directory
let bills = load_bills(Path::new("examples"))?;

// Per-bill budget summary
let summaries = query::summarize(&bills);

// Filtered provision search (filters are ANDed)
let results = query::search(&bills, &SearchFilter {
    division: Some("A"),
    ..Default::default()
});

// Agency-level rollup
let agencies = query::rollup_by_department(&bills);

// Cross-bill comparison
let diff = query::compare(&base_bills, &current_bills);

// Data-quality audit
let audit_rows = query::audit(&bills);
```

All query functions accept `&[LoadedBill]` and return plain structs (`Vec<BillSummary>`,
`Vec<SearchResult>`, etc.) with no side effects. The CLI layer handles
table/CSV formatting.

---

## Verification Design

Verification (`verification.rs`) is fully deterministic — it never calls an LLM.
It answers two questions: *Are the extracted amounts real?* and *Is extraction
complete?*

### Three-Tier Raw-Text Matching

Each provision's `raw_text` is matched against the bill source text:

| Tier         | Method | Handles |
|--------------|--------|---------|
| **Exact**    | Byte-identical substring search | Clean extractions |
| **Normalized** | Collapse whitespace, normalize quotes (`''""`→ASCII) and dashes (`—–‐`→`-`) | XML/Unicode formatting differences |
| **Spaceless** | Strip all whitespace then search | Line-break and indent variations |

Result is one of `Exact`, `Normalized`, `Spaceless`, or `NoMatch` (stored in
`MatchTier`).

### Amount Verification

For each provision with a dollar amount, the verifier checks whether that
amount appears in the source text at the expected location. Results:
`found`, `found_multiple` (ambiguous), or `not_found`.

### Completeness Metric

`text_index.rs` counts every dollar reference (`$NNN`) in the source XML.
The completeness percentage is: (dollar refs accounted for by provisions) /
(total dollar refs). This metric can legitimately be below 100% even when
extraction is correct, because:

- **Statutory references** cite amounts from other laws (e.g. "section 101(a)
  of Public Law 118-XX provided $X")
- **Loan/guarantee ceilings** repeat the same dollar figure in different
  semantic contexts
- **Struck amounts** — text like "strike '$50,000' and insert '$75,000'"
  contains old amounts that aren't current provisions

---

## Key Design Decisions

1. **LLM isolation.** The LLM is used *only* in the extraction step. Verification,
   querying, and budget arithmetic are all deterministic. This means you can
   trust `query::summarize()` totals — they are computed from provision data,
   never from LLM-generated summaries.

2. **Budget totals from provisions.** `BillExtraction::compute_totals()` sums
   individual provision amounts. The LLM's `ExtractionSummary` totals are
   informational only and never used for computation.

3. **Semantic chunking.** XML is split at division/title boundaries
   (`text_index.rs`), not at arbitrary token limits. Each chunk carries full
   context for its legislative section, reducing extraction errors at boundaries.

4. **Tagged enum deserialization.** `Provision` uses
   `#[serde(tag = "provision_type", rename_all = "snake_case")]` so each
   JSON object self-identifies its variant. This makes `extraction.json`
   human-readable and forward-compatible.

5. **Resilient parsing.** `from_value.rs` converts `serde_json::Value` →
   `Provision` with fallback handling for missing fields, unexpected types,
   and variant names the enum doesn't know yet (→ `Other`). This absorbs
   LLM output variance without hard failures.

6. **Schema evolution via `upgrade`.** The `upgrade` command re-deserializes
   and re-verifies existing `extraction.json` files against the current schema
   without re-running extraction. This enables schema changes (new fields, new
   variants) without burning API credits.

---

## Dependencies

| Crate             | Role |
|-------------------|------|
| `clap`            | CLI argument parsing |
| `roxmltree`       | XML parsing (read-only, no allocation of a DOM) |
| `reqwest`         | HTTP client (Congress.gov API, Anthropic API) |
| `tokio`           | Async runtime |
| `serde` / `serde_json` | Serialization for all JSON artifacts |
| `walkdir`         | Recursive directory traversal |
| `comfy-table`     | Terminal table formatting |
| `csv`             | CSV output |
| `sha2`            | Content hashing for metadata |
| `chrono`          | Timestamps in metadata |