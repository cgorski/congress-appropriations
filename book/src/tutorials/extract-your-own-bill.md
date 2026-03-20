# Extract Your Own Bill

> **You will need:** `congress-approp` installed, `CONGRESS_API_KEY` (free), `ANTHROPIC_API_KEY`. Optionally: `OPENAI_API_KEY` for embeddings.
>
> **You will learn:** How to go from zero to queryable data — downloading a bill from Congress.gov, extracting provisions with Claude, verifying the results, and optionally generating embeddings for semantic search.

The included example data covers three FY2024 bills, but there are dozens of enacted appropriations bills across recent congresses. This tutorial walks you through the full pipeline for extracting any bill you want.

## Step 1: Get Your API Keys

You need two keys to run the full pipeline. A third is optional for semantic search.

| Key | Purpose | Cost | Sign Up |
|-----|---------|------|---------|
| `CONGRESS_API_KEY` | Download bill XML from Congress.gov | Free | [api.congress.gov/sign-up](https://api.congress.gov/sign-up/) |
| `ANTHROPIC_API_KEY` | Extract provisions using Claude | Pay-per-use | [console.anthropic.com](https://console.anthropic.com/) |
| `OPENAI_API_KEY` | Generate embeddings for semantic search (optional) | Pay-per-use | [platform.openai.com](https://platform.openai.com/) |

Set them in your shell:

```bash
export CONGRESS_API_KEY="your-congress-key"
export ANTHROPIC_API_KEY="your-anthropic-key"
# Optional:
export OPENAI_API_KEY="your-openai-key"
```

## Step 2: Test Connectivity

Verify that your API keys work before spending time on a full extraction:

```bash
congress-approp api test
```

This checks both the Congress.gov and Anthropic APIs. You should see confirmation that both are reachable and your keys are valid.

## Step 3: Discover Available Bills

Use the `api bill list` command to see what appropriations bills exist for a given congress:

```bash
# List all appropriations bills for the 118th Congress (2023-2024)
congress-approp api bill list --congress 118

# List only enacted appropriations bills
congress-approp api bill list --congress 118 --enacted-only
```

The `--enacted-only` flag filters to bills that were signed into law — these are the ones that actually became binding spending authority. You'll see a list with bill type, number, title, and status.

### Congress numbers

Each Congress spans two years:

| Congress | Years | Example |
|----------|-------|---------|
| 117th | 2021–2022 | FY2022 and FY2023 bills |
| 118th | 2023–2024 | FY2024 and FY2025 bills |
| 119th | 2025–2026 | FY2026 bills |

### Bill type codes

When downloading a specific bill, you need the bill type code:

| Code | Meaning | Example |
|------|---------|---------|
| `hr` | House bill | H.R. 4366 |
| `s` | Senate bill | S. 1234 |
| `hjres` | House joint resolution | H.J.Res. 100 |
| `sjres` | Senate joint resolution | S.J.Res. 50 |

Most enacted appropriations bills originate in the House (`hr`), since the Constitution requires revenue and spending bills to originate there.

## Step 4: Download the Bill

### Download a single bill

If you know the specific bill you want:

```bash
congress-approp download --congress 118 --type hr --number 9468 --output-dir data
```

This fetches the enrolled (final, signed into law) XML from Congress.gov and saves it to `data/118/hr/9468/BILLS-118hr9468enr.xml`.

### Download all enacted bills for a congress

To get everything at once:

```bash
congress-approp download --congress 118 --enacted-only --output-dir data
```

This scans for all enacted appropriations bills in the 118th Congress and downloads their enrolled XML. It may take a minute or two depending on how many bills there are.

### Preview without downloading

Use `--dry-run` to see what would be downloaded without actually fetching anything:

```bash
congress-approp download --congress 118 --enacted-only --output-dir data --dry-run
```

## Step 5: Preview the Extraction (Dry Run)

Before making any LLM API calls, preview what the extraction will look like:

```bash
congress-approp extract --dir data/118/hr/9468 --dry-run
```

The dry run shows you:

- **Chunk count:** How many chunks the bill will be split into. Small bills (like the VA supplemental) are a single chunk. The FY2024 omnibus splits into 75 chunks.
- **Estimated input tokens:** How many tokens will be sent to the LLM. This helps you estimate cost before committing.

Here's what to expect for different bill sizes:

| Bill Type | Typical XML Size | Chunks | Estimated Input Tokens |
|-----------|-----------------|--------|----------------------|
| Supplemental (small) | ~10 KB | 1 | ~1,200 |
| Continuing Resolution | ~130 KB | 5 | ~25,000 |
| Omnibus (large) | ~1.8 MB | 75 | ~315,000 |

## Step 6: Run the Extraction

Now run the actual extraction:

```bash
congress-approp extract --dir data/118/hr/9468
```

For the small VA supplemental, this completes in under a minute. Here's what happens:

1. **Parse:** The XML is parsed to extract clean text and identify chunk boundaries
2. **Extract:** Each chunk is sent to Claude with a detailed system prompt defining every provision type
3. **Merge:** Provisions from all chunks are combined into a single list
4. **Compute:** Budget authority totals are computed from the individual provisions (never trusting the LLM's arithmetic)
5. **Verify:** Every dollar amount and text excerpt is checked against the source XML
6. **Write:** All artifacts are saved to disk

### Controlling parallelism

For large bills with many chunks, you can control how many LLM calls run simultaneously:

```bash
# Default: 5 concurrent calls
congress-approp extract --dir data/118/hr/4366

# Faster but uses more API quota
congress-approp extract --dir data/118/hr/4366 --parallel 8

# Conservative — one at a time
congress-approp extract --dir data/118/hr/4366 --parallel 1
```

Higher parallelism is faster but may hit API rate limits. The default of 5 is a good balance.

### Using a different model

By default, extraction uses `claude-opus-4-6`. You can override this:

```bash
# Via flag
congress-approp extract --dir data/118/hr/9468 --model claude-sonnet-4-20250514

# Via environment variable
export APPROP_MODEL="claude-sonnet-4-20250514"
congress-approp extract --dir data/118/hr/9468
```

> **Caution:** The system prompt and expected output format are tuned for Claude Opus. Other models may produce lower-quality extractions with more classification errors or missing provisions. Always check the `audit` output after extracting with a non-default model.

### Progress display

For multi-chunk bills, a progress dashboard shows real-time status:

```text
  5/42, 187 provs [4m 23s] 842 tok/s | 📝A-IIb ~8K 180/s | 🤔B-I ~3K | 📝B-III ~1K 95/s
```

This tells you: 5 of 42 chunks complete, 187 provisions extracted so far, running for 4 minutes 23 seconds, with three chunks currently being processed.

## Step 7: Check the Output Files

After extraction, your bill directory contains several new files:

```text
data/118/hr/9468/
├── BILLS-118hr9468enr.xml     ← Source XML (downloaded in Step 4)
├── extraction.json            ← All provisions with amounts, accounts, sections
├── verification.json          ← Deterministic checks against source text
├── metadata.json              ← Model name, prompt version, timestamps, source hash
├── tokens.json                ← LLM token usage (input, output, cache hits)
└── chunks/                    ← Per-chunk LLM artifacts (thinking traces, raw responses)
```

| File | What It Contains |
|------|-----------------|
| `extraction.json` | The main output: every extracted provision with structured fields. This is the file all query commands read. |
| `verification.json` | Deterministic verification: dollar amount checks, raw text matching, completeness analysis. No LLM involved. |
| `metadata.json` | Provenance: which model was used, prompt version, extraction timestamp, SHA-256 of the source XML. |
| `tokens.json` | Token usage: input tokens, output tokens, cache read/create tokens, total API calls. |
| `chunks/` | Per-chunk artifacts: the model's thinking content, raw response, parsed JSON, and conversion report for each chunk. These are local provenance records, gitignored by default. |

## Step 8: Verify the Extraction

Run the audit command to check quality:

```bash
congress-approp audit --dir data/118/hr/9468
```

```text
┌───────────┬────────────┬──────────┬──────────┬───────┬───────┬──────────┬───────────┬──────────┬──────────┐
│ Bill      ┆ Provisions ┆ Verified ┆ NotFound ┆ Ambig ┆ Exact ┆ NormText ┆ Spaceless ┆ TextMiss ┆ Coverage │
╞═══════════╪════════════╪══════════╪══════════╪═══════╪═══════╪══════════╪═══════════╪══════════╪══════════╡
│ H.R. 9468 ┆          7 ┆        2 ┆        0 ┆     0 ┆     5 ┆        0 ┆         0 ┆        2 ┆   100.0% │
└───────────┴────────────┴──────────┴──────────┴───────┴───────┴──────────┴───────────┴──────────┴──────────┘
```

What to check:

1. **NotFound should be 0.** If any dollar amounts weren't found in the source text, investigate with `audit --verbose`.
2. **Exact should be high.** This means the raw text excerpts are byte-identical to the source — the LLM copied the text faithfully.
3. **Coverage ideally ≥ 90%.** Coverage below 100% isn't necessarily a problem — see [What Coverage Means](../explanation/coverage.md).

If NotFound > 0, run the verbose audit to see which provisions failed:

```bash
congress-approp audit --dir data/118/hr/9468 --verbose
```

This lists each problematic provision with its dollar string, allowing you to manually check against the source XML.

## Step 9: Query Your Data

All the same commands you used with the example data now work on your extracted bill:

```bash
# Summary
congress-approp summary --dir data/118/hr/9468

# Search for specific provisions
congress-approp search --dir data/118/hr/9468 --type appropriation

# Compare with the examples
congress-approp compare --base data/118-hr4366 --current data/118/hr/9468
```

You can also point `--dir` at a parent directory to load multiple bills at once:

```bash
# Load everything under data/
congress-approp summary --dir data

# Search across all extracted bills
congress-approp search --dir data --keyword "Veterans Affairs"
```

The loader walks recursively from whatever `--dir` you specify, finding every `extraction.json` file.

## Step 10 (Optional): Generate Embeddings

If you want semantic search and `--similar` matching for your newly extracted bill, generate embeddings:

```bash
export OPENAI_API_KEY="your-key"
congress-approp embed --dir data/118/hr/9468
```

This sends each provision's text to OpenAI's `text-embedding-3-large` model and saves the vectors locally. For a small bill (7 provisions), this takes a few seconds. For the omnibus (2,364 provisions), about 30 seconds.

### Preview token usage

```bash
congress-approp embed --dir data/118/hr/9468 --dry-run
```

Shows how many provisions would be embedded and estimated token count without making any API calls.

### After embedding

Now semantic search works on your bill:

```bash
congress-approp search --dir data --semantic "school lunch programs" --top 5
congress-approp search --dir data --similar 118-hr9468:0 --top 5
```

The `embed` command writes two files:

- `embeddings.json` — Metadata: model name, dimensions, provision count, SHA-256 of the extraction it was built from
- `vectors.bin` — Binary float32 vectors (count × dimensions × 4 bytes)

See [Generate Embeddings](../how-to/generate-embeddings.md) for detailed options.

## Re-Extracting a Bill

If you want to re-extract a bill — perhaps with a newer model or after a schema update — simply run `extract` again. It will overwrite the existing `extraction.json` and `verification.json`.

After re-extracting, the embeddings become stale. The tool detects this via the hash chain and warns you:

```text
⚠ H.R. 9468: embeddings are stale (extraction.json has changed)
```

Run `embed` again to regenerate them.

If you only need to re-verify without re-extracting (for example, after a schema upgrade), use the `upgrade` command instead:

```bash
congress-approp upgrade --dir data/118/hr/9468
```

This re-deserializes the existing extraction through the current code's schema, re-runs verification, and updates the files — no LLM calls needed. See [Upgrade Extraction Data](../how-to/upgrade-data.md) for details.

## Estimating Costs

The `tokens.json` file records exact token usage after extraction. Here are typical numbers from the example bills:

| Bill | Type | Chunks | Input Tokens | Output Tokens |
|------|------|--------|-------------|--------------|
| H.R. 9468 | Supplemental (9 KB XML) | 1 | ~1,200 | ~1,500 |
| H.R. 5860 | CR (131 KB XML) | 5 | ~25,000 | ~15,000 |
| H.R. 4366 | Omnibus (1.8 MB XML) | 75 | ~315,000 | ~200,000 |

Embedding costs are much lower — approximately $0.01 per bill for `text-embedding-3-large`.

Use `extract --dry-run` and `embed --dry-run` to preview token counts before committing to API calls.

## Quick Reference: Full Pipeline

Here's the complete sequence for extracting a bill from scratch:

```bash
# 1. Set API keys
export CONGRESS_API_KEY="..."
export ANTHROPIC_API_KEY="..."
export OPENAI_API_KEY="..."  # optional, for embeddings

# 2. Find the bill
congress-approp api bill list --congress 118 --enacted-only

# 3. Download
congress-approp download --congress 118 --type hr --number 4366 --output-dir data

# 4. Preview extraction
congress-approp extract --dir data/118/hr/4366 --dry-run

# 5. Extract
congress-approp extract --dir data/118/hr/4366 --parallel 6

# 6. Verify
congress-approp audit --dir data/118/hr/4366

# 7. Generate embeddings (optional)
congress-approp embed --dir data/118/hr/4366

# 8. Query
congress-approp summary --dir data
congress-approp search --dir data --type appropriation
```

## Troubleshooting

### "No XML files found"

Make sure you downloaded the bill first (`congress-approp download`). The `extract` command looks for `BILLS-*.xml` files in the specified directory.

### "Rate limited" errors during extraction

Reduce parallelism: `extract --parallel 2`. Anthropic's API has per-minute token limits that can be exceeded with high concurrency on large bills.

### Low coverage after extraction

Run `audit --verbose` to see which dollar amounts in the source text weren't captured. Common causes:

- **Statutory cross-references:** Dollar amounts from other laws cited in the bill text — correctly excluded
- **Struck amounts:** "Striking '$50,000' and inserting '$75,000'" — the old amount shouldn't be extracted
- **Loan guarantee ceilings:** Not budget authority — correctly excluded

If legitimate provisions are missing, consider re-extracting with a higher-capability model.

### Stale embeddings warning

After re-extracting, the hash chain detects that `extraction.json` has changed but `embeddings.json` still references the old version. Run `congress-approp embed --dir <path>` to regenerate.

## Next Steps

- **[Verify Extraction Accuracy](../how-to/verify-accuracy.md)** — detailed guide for auditing results
- **[Generate Embeddings](../how-to/generate-embeddings.md)** — embedding options and configuration
- **[Filter and Search Provisions](../how-to/filter-and-search.md)** — all search flags for querying your new data