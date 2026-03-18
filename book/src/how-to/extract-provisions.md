# Extract Provisions from a Bill

> **You will need:** `congress-approp` installed, downloaded bill XML (see [Download Bills](./download-bills.md)), `ANTHROPIC_API_KEY` environment variable set.
>
> **You will learn:** How to run the extraction pipeline, control parallelism and model selection, interpret the output files, and handle common issues.

Extraction is the core step of the pipeline — it sends bill text to Claude, which identifies and classifies every spending provision, then deterministic verification checks every dollar amount against the source. This guide covers all the options and considerations.

## Prerequisites

1. **Downloaded bill XML.** You need at least one `BILLS-*.xml` file in a bill directory. See [Download Bills from Congress.gov](./download-bills.md).
2. **Anthropic API key.** Set it in your environment:

```bash
export ANTHROPIC_API_KEY="your-key-here"
```

## Preview Before Extracting

Always start with a dry run to see what the extraction will involve:

```bash
congress-approp extract --dir data/118/hr/9468 --dry-run
```

The dry run shows you:

- **Bill identifier** parsed from the XML
- **Chunk count** — how many pieces the bill will be split into for parallel processing
- **Estimated input tokens** — helps you estimate API cost before committing

Typical chunk counts by bill size:

| Bill Type | XML Size | Chunks | Est. Input Tokens |
|-----------|----------|--------|-------------------|
| Supplemental (small) | ~10 KB | 1 | ~1,200 |
| Continuing Resolution | ~130 KB | 3–5 | ~25,000 |
| Individual regular bill | ~200–500 KB | 10–20 | ~50,000–100,000 |
| Omnibus (large) | ~1–2 MB | 50–75 | ~200,000–315,000 |

No API calls are made during a dry run.

## Run Extraction

### Single bill

```bash
congress-approp extract --dir data/118/hr/9468
```

For a small bill (like the VA supplemental), this completes in under a minute. The tool:

1. **Parses** the XML to extract clean text and identify structural boundaries (divisions, titles)
2. **Splits** large bills into chunks at division and title boundaries
3. **Sends** each chunk to Claude with a ~300-line system prompt defining every provision type
4. **Merges** provisions from all chunks into a single list
5. **Computes** budget authority totals from the individual provisions (never trusting the LLM's arithmetic)
6. **Verifies** every dollar amount and text excerpt against the source XML
7. **Writes** all artifacts to disk

### Multiple bills

Point `--dir` at a parent directory to extract all bills found underneath:

```bash
congress-approp extract --dir data
```

The tool walks recursively, finds every directory containing a `BILLS-*.xml` file, and extracts each one. **Bills that already have `extraction.json` are automatically skipped** — you can safely re-run the same command after a partial failure and it picks up where it left off. To force re-extraction of already-processed bills, use `--force`:

```bash
# Re-extract everything, even bills that already have extraction.json
congress-approp extract --dir data --force
```

### Enrolled versions only

When a bill directory contains multiple XML versions (enrolled, introduced, engrossed, etc.), the extract command **automatically uses only the enrolled version** (`*enr.xml`). Non-enrolled versions are ignored. If no enrolled version exists, all available versions are processed.

This means you don't need to worry about cleaning up extra XML files — the tool picks the right one automatically.

### Resilient processing

If an XML file fails to parse (for example, a non-enrolled version with a different XML structure), the tool **logs a warning and continues** to the next bill instead of aborting the entire run:

```text
⚠ Skipping data/118/hr/2872/BILLS-118hr2872eas.xml: Failed to parse ... (not a parseable bill XML?)
```

This means one bad file won't kill a multi-bill extraction run.

### Extract all downloaded bills with parallelism

```bash
congress-approp extract --dir data --parallel 6
```

## Controlling Parallelism

The `--parallel` flag controls how many LLM API calls run simultaneously. This affects both speed and API rate limit usage:

```bash
# Default: 5 concurrent calls
congress-approp extract --dir data/118/hr/4366

# Faster — good for large bills if your API quota allows
congress-approp extract --dir data/118/hr/4366 --parallel 8

# Conservative — avoids rate limits, good for debugging
congress-approp extract --dir data/118/hr/4366 --parallel 1
```

| Parallelism | Speed | Rate Limit Risk | Best For |
|-------------|-------|-----------------|----------|
| 1 | Slowest | None | Debugging, small bills |
| 3 | Moderate | Low | Conservative extraction |
| 5 (default) | Good | Moderate | Most use cases |
| 8–10 | Fast | Higher | Large bills with high API quota |

For the FY2024 omnibus (75 chunks), `--parallel 6` completes in approximately 60 minutes. At `--parallel 1`, it would take several hours.

### Progress display

For multi-chunk bills, a live progress dashboard shows extraction status:

```text
  5/42, 187 provs [4m 23s] 842 tok/s | 📝A-IIb ~8K 180/s | 🤔B-I ~3K | 📝B-III ~1K 95/s
```

Reading left to right:
- `5/42` — 5 of 42 chunks complete
- `187 provs` — 187 provisions extracted so far
- `[4m 23s]` — elapsed time
- `842 tok/s` — average token throughput
- The remaining items show currently active chunks: 📝 = receiving response, 🤔 = model is thinking

## Choosing a Model

By default, extraction uses `claude-opus-4-6`, which produces the highest quality results. You can override this:

```bash
# Via command-line flag
congress-approp extract --dir data/118/hr/9468 --model claude-sonnet-4-20250514

# Via environment variable (useful for scripting)
export APPROP_MODEL="claude-sonnet-4-20250514"
congress-approp extract --dir data/118/hr/9468
```

The command-line flag takes precedence over the environment variable.

> **Quality warning:** The system prompt and expected output format are specifically tuned for Claude Opus. Other models may produce:
> - More classification errors (e.g., marking an appropriation as a rider)
> - Missing provisions (especially sub-allocations and proviso amounts)
> - Inconsistent JSON formatting (handled by `from_value.rs` resilient parsing, but still)
> - Lower coverage scores in the audit
>
> Always check `audit` output after extracting with a non-default model.

The model name is recorded in `metadata.json` so you always know which model produced a given extraction.

## Output Files

After extraction, the bill directory contains:

```text
data/118/hr/9468/
├── BILLS-118hr9468enr.xml     ← Source XML (unchanged)
├── extraction.json            ← All provisions with amounts, accounts, sections
├── verification.json          ← Deterministic checks against source text
├── metadata.json              ← Model name, prompt version, timestamps, source hash
├── tokens.json                ← LLM token usage (input, output, cache hits)
└── chunks/                    ← Per-chunk LLM artifacts (gitignored)
    ├── 01JRWN9T5RR0JTQ6C9FYYE96A8.json
    └── ...
```

### extraction.json

The main output. Contains:

- **`bill`** — Identifier, classification, short title, fiscal years, divisions
- **`provisions`** — Array of every extracted provision with full structured data
- **`summary`** — LLM-generated summary statistics (used for diagnostics, never for computation)
- **`chunk_map`** — Links each provision to the chunk it was extracted from
- **`schema_version`** — Version of the extraction schema

This is the file all query commands (`search`, `summary`, `compare`, `audit`) read.

### verification.json

Deterministic verification of every provision against the source text. No LLM involved:

- **`amount_checks`** — Was each dollar string found in the source?
- **`raw_text_checks`** — Is each raw text excerpt a substring of the source?
- **`completeness`** — How many dollar strings in the source were captured?
- **`summary`** — Roll-up metrics (verified, not_found, ambiguous, match tiers)

### metadata.json

Extraction provenance:

- **`model`** — Which LLM model was used
- **`prompt_version`** — Hash of the system prompt
- **`extraction_timestamp`** — When the extraction ran
- **`source_xml_sha256`** — SHA-256 hash of the source XML (for the hash chain)

### tokens.json

API token usage:

- **`total_input`** — Total input tokens across all chunks
- **`total_output`** — Total output tokens
- **`total_cache_read`** — Tokens served from prompt cache (reduces cost)
- **`total_cache_create`** — Tokens added to prompt cache
- **`calls`** — Number of API calls made

### chunks/ directory

Per-chunk LLM artifacts stored with ULID filenames. Each file contains:

- The model's **thinking content** (internal reasoning)
- The **raw JSON response** before parsing
- The **parsed provisions** for that chunk
- A **conversion report** showing any type coercions or missing fields

These are permanent provenance records — useful for debugging why a particular provision was classified a certain way. They are gitignored by default (not part of the hash chain, not needed for downstream operations).

## Verify After Extraction

Always run the audit after extracting:

```bash
congress-approp audit --dir data/118/hr/9468
```

What to check:

| Metric | Good Value | Action if Bad |
|--------|-----------|---------------|
| **NotFound** | 0 | Run `audit --verbose` to see which provisions failed; check source XML manually |
| **Exact** | > 90% of provisions | Minor formatting differences are handled by NormText tier; only worry if TextMiss is high |
| **Coverage** | > 80% for regular bills | Review unaccounted amounts — many are legitimately excluded (statutory refs, loan ceilings) |
| **Provisions count** | Reasonable for bill size | A small bill with 500+ provisions or a large bill with <50 may indicate extraction issues |

For a detailed verification procedure, see [Verify Extraction Accuracy](./verify-accuracy.md).

## Re-Extracting a Bill

To re-extract (for example, with a newer model or after prompt improvements), use the `--force` flag:

```bash
# Re-extract even though extraction.json already exists
congress-approp extract --dir data/118/hr/9468 --force
```

Without `--force`, the extract command skips bills that already have `extraction.json`. This makes it safe to re-run `extract --dir data` after a partial failure — only unprocessed bills will be extracted.

After re-extraction:
- `extraction.json` and `verification.json` are overwritten
- `metadata.json` and `tokens.json` are overwritten
- A new set of chunk artifacts is created in `chunks/`
- **Embeddings become stale** — the tool will warn you, and you'll need to run `embed` again

### Upgrade without re-extracting

If you only need to re-verify against a newer schema (no LLM calls), use `upgrade` instead:

```bash
congress-approp upgrade --dir data/118/hr/9468
```

This re-deserializes the existing extraction through the current code's schema, re-runs verification, and updates the files. Much faster and free. See [Upgrade Extraction Data](./upgrade-data.md).

## Handling Large Bills

Omnibus bills (1,000+ pages) require special attention:

### Chunk splitting

Large bills are automatically split into chunks at XML `<division>` and `<title>` boundaries. This is semantic chunking — each chunk contains a complete legislative section with full context. The FY2024 omnibus (H.R. 4366) splits into approximately 75 chunks.

If a single title or division exceeds the maximum chunk token limit (~3,000 tokens), it's further split at paragraph boundaries. This is rare but happens for very long sections.

### Time estimates

| Bill | Chunks | --parallel 5 | --parallel 8 |
|------|--------|-------------|-------------|
| Small supplemental | 1 | ~30 seconds | ~30 seconds |
| Continuing resolution | 5 | ~3 minutes | ~2 minutes |
| Regular bill | 15–20 | ~15 minutes | ~10 minutes |
| Omnibus | 75 | ~75 minutes | ~50 minutes |

### Handling interruptions

If extraction is interrupted (network error, rate limit, crash), you'll need to re-run it from the beginning. There is no checkpoint/resume mechanism — the tool extracts all chunks and merges them atomically.

## Troubleshooting

### "All bills already extracted"

This means every bill directory already has `extraction.json`. Use `--force` to re-extract:

```bash
congress-approp extract --dir data/118/hr/9468 --force
```

### "No XML files found"

Make sure you downloaded the bill first. The `extract` command looks for files matching `BILLS-*.xml` in the specified directory.

```bash
ls data/118/hr/9468/BILLS-*.xml
```

### "Rate limited" or 429 errors

Reduce parallelism:

```bash
congress-approp extract --dir data/118/hr/4366 --parallel 2
```

Anthropic's API has per-minute token limits. High concurrency on large bills can exceed these limits.

### Low provision count

If a large bill produces surprisingly few provisions, check:

1. **The XML file** — is it the correct version? Some partial texts are available on Congress.gov.
2. **The audit output** — low coverage combined with low provision count suggests the extraction missed sections.
3. **The chunk artifacts** — look in `chunks/` for any chunks that produced zero provisions or error responses.

### "Unexpected token" or JSON parsing errors

The `from_value.rs` resilient parser handles most LLM output quirks automatically. If you see parsing warnings in the verbose output, they're usually minor (a missing field defaulting to empty, a string where a number was expected being coerced). The `conversion.json` report in each chunk directory shows exactly what was adjusted.

If extraction fails entirely, try with `--parallel 1` to isolate which chunk is problematic, then examine that chunk's artifacts in `chunks/`.

## Quick Reference

```bash
# Set API key
export ANTHROPIC_API_KEY="your-key"

# Preview extraction (no API calls)
congress-approp extract --dir data/118/hr/9468 --dry-run

# Extract a single bill
congress-approp extract --dir data/118/hr/9468

# Extract with higher parallelism
congress-approp extract --dir data/118/hr/4366 --parallel 8

# Extract all bills under a directory (skips already-extracted bills)
congress-approp extract --dir data --parallel 6

# Re-extract a bill that was already extracted
congress-approp extract --dir data/118/hr/9468 --force

# Verify after extraction
congress-approp audit --dir data/118/hr/9468
```

## Full Command Reference

```text
congress-approp extract [OPTIONS]

Options:
    --dir <DIR>            Data directory containing downloaded bill XML [default: ./data]
    --dry-run              Show what would be extracted without calling LLM
    --parallel <PARALLEL>  Parallel LLM calls [default: 5]
    --model <MODEL>        LLM model override [env: APPROP_MODEL=]
    --force                Re-extract bills even if extraction.json already exists
```

## Next Steps

- **[Verify Extraction Accuracy](./verify-accuracy.md)** — detailed audit and verification guide
- **[Generate Embeddings](./generate-embeddings.md)** — enable semantic search for extracted bills
- **[Filter and Search Provisions](./filter-and-search.md)** — query your newly extracted data