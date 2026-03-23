# Running the Complete Pipeline

This guide walks through every step to process appropriations bills from raw XML to a queryable account registry. Each step adds data without modifying previous outputs. You can stop at any step and still get value from the data produced so far.

## Prerequisites

```bash
cargo install --path .    # Build the tool (Rust 1.93+)
```

API keys (only needed for specific steps):

| Key | Environment Variable | Required For |
|-----|---------------------|-------------|
| Congress.gov | `CONGRESS_API_KEY` | `download` (free at api.congress.gov) |
| Anthropic | `ANTHROPIC_API_KEY` | `extract`, `resolve-tas` (LLM tier) |
| OpenAI | `OPENAI_API_KEY` | `embed` (text-embedding-3-large) |

No API keys are needed for `verify-text`, `enrich`, `authority build`, or any query command when working with pre-processed data.

## The Pipeline

```text
Step 1: download       → BILLS-*.xml
Step 2: extract        → extraction.json, verification.json, metadata.json
Step 3: verify-text    → source_span on every provision (modifies extraction.json)
Step 4: enrich         → bill_meta.json
Step 5: resolve-tas    → tas_mapping.json
Step 6: embed          → embeddings.json, vectors.bin
Step 7: authority build → authorities.json
```

### Step 1: Download bill XML

```bash
# Download all enacted bills for a congress
congress-approp download --congress 119 --enacted-only

# Or download a specific bill
congress-approp download --congress 119 --type hr --number 7148
```

This fetches the enrolled (signed-into-law) XML from Congress.gov into `data/{congress}-{type}{number}/`. Each bill gets its own directory.

**Cost:** Free (Congress.gov API is free).
**Time:** ~30 seconds per congress.
**Needs:** `CONGRESS_API_KEY`

You can skip this step entirely if you already have bill XML files — just place them in the expected directory structure.

### Step 2: Extract provisions

```bash
congress-approp extract --dir data --parallel 5
```

Sends bill text to Claude Opus 4.6 for structured extraction. Large bills are split into chunks and processed in parallel. Every provision — appropriations, rescissions, CR anomalies, riders, directives — is captured as typed JSON.

The command skips bills that already have `extraction.json`. Use `--force` to re-extract.

**Cost:** ~$0.10 per chunk. Small bills: $0.10–0.50. Omnibus bills: $5–15.
**Time:** Small bills: 1–2 minutes. Omnibus: 30–60 minutes.
**Needs:** `ANTHROPIC_API_KEY`

**This is the expensive step.** Once done, you do not need to re-extract unless the model or prompt improves significantly.

**Produces per bill:**

| File | Content |
|------|---------|
| `extraction.json` | Structured provisions (the main output) |
| `verification.json` | Dollar amount and raw text verification |
| `metadata.json` | Provenance (model, timestamps, chunk completion) |
| `conversion.json` | LLM JSON parsing report |
| `tokens.json` | API token usage for cost tracking |
| `BILLS-*.txt` | Clean text extracted from XML (used for verification) |

### Step 3: Verify and repair raw text

```bash
congress-approp verify-text --dir data --repair
```

Deterministically checks that every provision's `raw_text` field is a verbatim substring of the enrolled bill source text. Repairs LLM copying errors (word substitutions like "clause" instead of "subsection", whitespace differences, quote character mismatches) using a 3-tier algorithm:

1. **Prefix match** — find the longest matching prefix, copy source bytes
2. **Substring match** — find a distinctive internal phrase, walk backward to the provision start
3. **Normalized position mapping** — search in whitespace/quote-normalized space, map back to original byte positions

After repair, every provision carries a `source_span` with exact UTF-8 byte offsets into the source `.txt` file.

**Cost:** Free (no API calls).
**Time:** ~10 seconds for all 32 bills.
**Needs:** Nothing.

Without `--repair`, the command analyzes but does not modify any files. A backup (`extraction.json.pre-repair`) is created before any modifications.

**Invariant:** After this step, for every provision `p`:
```
source_file_bytes[p.source_span.start .. p.source_span.end] == p.raw_text
```

This is mechanically verifiable. The `start` and `end` values are UTF-8 byte offsets (matching Rust's native `str` indexing). Languages that use character-based indexing (Python, JavaScript) must use byte-level slicing:

```python
raw_bytes = open("BILLS-118hr2882enr.txt", "rb").read()
actual = raw_bytes[span["start"]:span["end"]].decode("utf-8")
assert actual == provision["raw_text"]
```

### Step 4: Enrich with metadata

```bash
congress-approp enrich --dir data
```

Generates `bill_meta.json` per bill with fiscal year metadata, subcommittee/jurisdiction mappings, advance appropriation classification, and enriched bill nature (omnibus, minibus, full-year CR, etc.). Uses XML parsing and deterministic keyword matching — no LLM calls.

**Cost:** Free.
**Time:** ~30 seconds for all bills.
**Needs:** Nothing.

Enables `--fy`, `--subcommittee`, and `--show-advance` flags on query commands.

### Step 5: Resolve Treasury Account Symbols

```bash
# Full resolution (deterministic + LLM)
congress-approp resolve-tas --dir data

# Deterministic only (free, no API key, ~56% resolution)
congress-approp resolve-tas --dir data --no-llm

# Preview cost before running
congress-approp resolve-tas --dir data --dry-run
```

Maps each top-level budget authority provision to a Federal Account Symbol (FAS) — a stable identifier assigned by the Treasury that persists through account renames and reorganizations.

**Two tiers:**
- **Deterministic** (~56%): Matches provision account names against the bundled FAST Book reference (`fas_reference.json`). Free, instant, zero false positives.
- **LLM** (~44%): Sends ambiguous provisions to Claude Opus with the relevant FAS codes for the provision's agency. Verifies each returned code against the FAST Book.

**Cost:** Free with `--no-llm`. ~$85 for the full 32-bill dataset with LLM tier (~$2–4 per omnibus).
**Time:** Instant for `--no-llm`. ~5 minutes per omnibus with LLM.
**Needs:** `ANTHROPIC_API_KEY` for LLM tier.

**This is a one-time cost per bill.** The FAS code assignment does not need to be repeated unless the bill is re-extracted.

### Step 6: Generate embeddings

```bash
congress-approp embed --dir data
```

Generates OpenAI embedding vectors (text-embedding-3-large, 3072 dimensions) for every provision. Enables semantic search (`--semantic`), similar-provision matching (`--similar`), the `relate` command, and `link suggest`.

**Cost:** ~$14 for 34,568 provisions.
**Time:** ~10–15 minutes for all bills.
**Needs:** `OPENAI_API_KEY`

**Optional.** If you only need TAS-based account tracking, keyword search, and fiscal year comparisons, you can skip this step.

### Step 7: Build the authority registry

```bash
congress-approp authority build --dir data
```

Aggregates all `tas_mapping.json` files into a single `authorities.json` at the data root. Groups provisions by FAS code into account authorities with name variants, provision references, fiscal year coverage, dollar totals, and detected lifecycle events (renames).

**Cost:** Free.
**Time:** ~1 second.
**Needs:** At least one `tas_mapping.json` from Step 5.

## Querying the Data

After the pipeline completes, all query commands work:

```bash
# What bills do I have?
congress-approp summary --dir data

# Filter to one fiscal year
congress-approp summary --dir data --fy 2026

# Track an account across fiscal years
congress-approp trace 070-0400 --dir data
congress-approp trace "coast guard operations" --dir data

# Browse the account registry
congress-approp authority list --dir data --agency 070

# Search by meaning
congress-approp search --dir data --semantic "disaster relief funding" --top 5

# Compare fiscal years with TAS matching
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud \
    --dir data --use-authorities

# Audit data quality
congress-approp audit --dir data

# Verify source traceability
congress-approp verify-text --dir data
```

## Adding a New Bill

When Congress enacts a new bill, add it to the dataset:

```bash
congress-approp download --congress 119 --type hr --number 9999
congress-approp extract --dir data/119-hr9999 --parallel 5
congress-approp verify-text --dir data --bill 119-hr9999 --repair
congress-approp enrich --dir data/119-hr9999
congress-approp resolve-tas --dir data --bill 119-hr9999
congress-approp embed --dir data/119-hr9999
congress-approp authority build --dir data --force
```

The `--force` on the last command rebuilds `authorities.json` to include the new bill. All existing data is unchanged.

## Rebuilding From Scratch

If you have only the XML files, you can rebuild everything:

```bash
congress-approp extract --dir data --parallel 5      # ~$100, ~4 hours
congress-approp verify-text --dir data --repair       # free, ~10 seconds
congress-approp enrich --dir data                     # free, ~30 seconds
congress-approp resolve-tas --dir data                # ~$85, ~1 hour
congress-approp embed --dir data                      # ~$14, ~15 minutes
congress-approp authority build --dir data             # free, ~1 second
```

Total cost to rebuild from scratch: ~$200. Total time: ~6 hours (mostly waiting for LLM responses). The XML files themselves are permanent government records available from Congress.gov.

## Pipeline Dependencies

```text
download (1) ─────────┐
                       ▼
extract (2) ──────► verify-text (3) ──────┐
     │                                     │
     ├──────────► enrich (4) ◄────────────┘
     │                │
     ├──────────► resolve-tas (5) ◄── fas_reference.json
     │                │
     └──────────► embed (6)
                      │
                      ├──► link suggest
                      │
authority build (7) ◄─── resolve-tas outputs from all bills
```

Steps 4, 5, and 6 are independent of each other — they all read from `extraction.json` and can run in any order after Step 3. Step 7 requires Step 5 to have run on all bills you want included.

## Output File Reference

### Per-bill files

| File | Step | Size (typical) | Content |
|------|------|---------------|---------|
| `BILLS-*.xml` | 1 | 12K–9.4MB | Enrolled bill XML (source of truth) |
| `BILLS-*.txt` | 2 | 3K–3MB | Clean text from XML |
| `extraction.json` | 2+3 | 20K–2MB | Provisions + source spans |
| `verification.json` | 2 | 5K–500K | Verification report |
| `metadata.json` | 2 | 500B | Provenance |
| `bill_meta.json` | 4 | 2K–20K | FY, subcommittee, timing |
| `tas_mapping.json` | 5 | 5K–200K | FAS codes per provision |
| `embeddings.json` | 6 | 1K–50K | Embedding metadata |
| `vectors.bin` | 6 | 100K–35MB | Binary float32 vectors |

### Cross-bill files (at data root)

| File | Step | Content |
|------|------|---------|
| `fas_reference.json` | bundled | 2,768 FAS codes from the FAST Book |
| `authorities.json` | 7 | Account registry with timelines and events |
| `dataset.json` | normalize accept | Entity resolution rules (optional) |
| `links/links.json` | link accept | Embedding-based cross-bill links (optional) |