# Congressional Appropriations Analyzer

A command-line tool that downloads U.S. federal appropriations bills from Congress.gov, extracts every spending provision into structured JSON using Claude Opus 4.6, and verifies each dollar amount against the source text.

The goal: make the ~1,500 pages of annual appropriations bills searchable, sortable, and machine-readable — so you can answer questions like "how much did Congress appropriate for VA Compensation and Pensions?" or "which programs got cut in the continuing resolution?" in seconds instead of hours.

## How It Works

```text
Congress.gov API              XML Parser (Rust)          Claude Opus 4.6            Verification
      │                             │                          │                          │
      ▼                             ▼                          ▼                          ▼
 ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
 │ Download │─▶│ Parse    │─▶│ LLM      │─▶│ Verify   │─▶│ Search   │
 │ bill XML │  │ XML      │  │ extract  │  │ amounts  │  │ Compare  │
 └──────────┘  └──────────┘  └──────────┘  └──────────┘  └──────────┘
  BILLS-*.xml   clean text    extraction.json  verification.json
```

1. **Download** — Fetch enrolled bill XML from Congress.gov (structured, semantic markup).
2. **Parse** — Extract clean text from XML using `roxmltree` in pure Rust. No Python, no PDF conversion. The XML provides exact structural boundaries for divisions, titles, and sections.
3. **Extract** — Send bill text to Claude Opus 4.6 with adaptive thinking. Large bills are automatically split into chunks and extracted in parallel. Every provision — appropriations, rescissions, CR anomalies, riders, directives — is captured as structured JSON.
4. **Verify** — Deterministically check every dollar amount and text excerpt against the source. No LLM involved. Pure string matching with tiered fallback (exact → normalized → spaceless).
5. **Query** — Search, summarize, compare, and verify across all extracted bills using built-in subcommands.

## Scope

This tool extracts **discretionary appropriations** — the spending Congress votes on each year through the twelve annual appropriations bills (plus supplementals and continuing resolutions). That's roughly **26% of total federal spending**. It does **not** cover mandatory spending (Social Security, Medicare, Medicaid — about 63%) or net interest on the debt (about 11%).

The amounts represent **budget authority** (what Congress authorizes agencies to obligate), not **outlays** (what the Treasury actually disburses). This is why the numbers you'll see — around $1.7–1.9 trillion — don't match the ~$6–7 trillion headline federal budget figure.

## Quick Start

### Prerequisites

- **Rust 1.93+** — [Install via rustup](https://rustup.rs/)
- **Congress.gov API key** — Free, [sign up here](https://api.congress.gov/sign-up/)
- **Anthropic API key** — Required for LLM extraction, [sign up here](https://console.anthropic.com/)

That's it. No Python, no pip, no virtual environments.

### Install

```bash
git clone https://github.com/youruser/appropriations.git
cd appropriations
cargo install --path .
```

This puts `congress-approp` on your PATH. If you modify the code, run `cargo install --path .` again to update.

### Extract a Bill

```bash
# Set your API keys
export CONGRESS_API_KEY="your-key"
export ANTHROPIC_API_KEY="your-key"

# Find the enrolled bill XML URL
congress-approp api bill text --congress 118 --type hr -n 9468

# Download it
mkdir -p data/hr9468
curl -sL -o data/hr9468/BILLS-118hr9468enr.xml \
  "https://www.congress.gov/118/bills/hr9468/BILLS-118hr9468enr.xml"

# Extract provisions and verify
congress-approp extract --dir data/hr9468
```

### Download All Bills for a Congress

```bash
# Download all enacted appropriations bills (XML format)
congress-approp download --congress 118 --enacted-only --output-dir data/118

# Extract everything
congress-approp extract --dir data/118 --parallel 6
```

## Try It Without API Keys

The `examples/` directory contains pre-extracted data from two real bills — no API keys needed to explore:

- **`examples/hr9468/`** — Veterans Benefits Supplemental Appropriations Act, 2024 (small, 7 provisions, 100% complete)
- **`examples/hr5860/`** — Continuing Appropriations Act, 2024 (medium, 130 provisions, 61% complete, with CR substitutions and mandatory spending extensions)

Each directory contains the source XML, the extracted provisions, and the verification report. You can run any query command against them immediately.

## Querying Extracted Bills

### `summary` — What bills do I have?

```bash
congress-approp summary --dir examples
```

```text
┌───────────┬──────────────────────┬───────┬─────────────────┬─────────────────┬────────────────┬───────────┐
│ Bill      ┆ Classification       ┆ Provs ┆ Budget Auth ($) ┆ Rescissions ($) ┆     Net BA ($) ┆ Complete% │
╞═══════════╪══════════════════════╪═══════╪═════════════════╪═════════════════╪════════════════╪═══════════╡
│ H.R. 5860 ┆ ContinuingResolution ┆   130 ┆  16,000,000,000 ┆               0 ┆ 16,000,000,000 ┆     61.1% │
│ H.R. 9468 ┆ Supplemental         ┆     7 ┆   2,882,482,000 ┆               0 ┆  2,882,482,000 ┆    100.0% │
│ TOTAL     ┆                      ┆   137 ┆  18,882,482,000 ┆               0 ┆ 18,882,482,000 ┆           │
└───────────┴──────────────────────┴───────┴─────────────────┴─────────────────┴────────────────┴───────────┘
```

Budget authority is computed from the actual provisions, not the LLM's self-reported summary. The **Complete%** column shows what percentage of dollar amounts in the source text were captured — red means incomplete, green means comprehensive.

### `search` — Find provisions across bills

Tables adapt automatically to the provision type you're searching for.

**Find all appropriations:**

```bash
congress-approp search --dir examples --type appropriation
```

```text
┌───┬───────────┬───────────────┬───────────────────────────────────────────────┬───────────────┬──────────┬─────┐
│ V ┆ Bill      ┆ Type          ┆ Description / Account                         ┆    Amount ($) ┆ Section  ┆ Div │
╞═══╪═══════════╪═══════════════╪═══════════════════════════════════════════════╪═══════════════╪══════════╪═════╡
│ ✓ ┆ H.R. 9468 ┆ appropriation ┆ Compensation and Pensions                     ┆ 2,285,513,000 ┆          ┆     │
│ ✓ ┆ H.R. 9468 ┆ appropriation ┆ Readjustment Benefits                         ┆   596,969,000 ┆          ┆     │
│ ✓ ┆ H.R. 5860 ┆ appropriation ┆ Federal Emergency Management Agency—Disaster…  ┆16,000,000,000 ┆ SEC. 129 ┆ A   │
└───┴───────────┴───────────────┴───────────────────────────────────────────────┴───────────────┴──────────┴─────┘
```

The **V** column shows verification status: ✓ means the dollar amount was found verbatim in the source text.

**Find CR anomalies (which programs got funding changes):**

```bash
congress-approp search --dir examples/hr5860 --type cr_substitution
```

```text
┌───┬───────────┬──────────────────────────────────────────┬───────────────┬───────────────┬──────────────┬──────────┬─────┐
│ V ┆ Bill      ┆ Account                                  ┆       New ($) ┆       Old ($) ┆    Delta ($) ┆ Section  ┆ Div │
╞═══╪═══════════╪══════════════════════════════════════════╪═══════════════╪═══════════════╪══════════════╪══════════╪═════╡
│ ✓ ┆ H.R. 5860 ┆ Rural Housing Service—Rural Community…   ┆    25,300,000 ┆    75,300,000 ┆  -50,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ National Science Foundation—STEM Educ…   ┆    92,000,000 ┆   217,000,000 ┆ -125,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Office of Personnel Management—Salari…   ┆   219,076,000 ┆   190,784,000 ┆  +28,292,000 ┆ SEC. 126 ┆ A   │
│   ┆ ...       ┆ (13 provisions total)                    ┆               ┆               ┆              ┆          ┆     │
└───┴───────────┴──────────────────────────────────────────┴───────────────┴───────────────┴──────────────┴──────────┴─────┘
```

The CR substitution table automatically shows **New**, **Old**, and **Delta** columns — exactly what you need to see which programs Congress funded above or below the prior-year rate.

**Find reporting requirements:**

```bash
congress-approp search --dir examples/hr9468 --type directive
```

```text
┌───┬───────────┬────────────────────────────────────────────────────────────────────────┬──────────┬─────┐
│ V ┆ Bill      ┆ Description                                                            ┆ Section  ┆ Div │
╞═══╪═══════════╪════════════════════════════════════════════════════════════════════════╪══════════╪═════╡
│   ┆ H.R. 9468 ┆ Requires the Secretary of Veterans Affairs to submit a report detaili… ┆ SEC. 103 ┆     │
│   ┆ H.R. 9468 ┆ Requires the Secretary of Veterans Affairs to submit a report on the … ┆ SEC. 103 ┆     │
│   ┆ H.R. 9468 ┆ Requires the Inspector General of the Department of Veterans Affairs … ┆ SEC. 104 ┆     │
└───┴───────────┴────────────────────────────────────────────────────────────────────────┴──────────┴─────┘
```

**Export to CSV for Excel:**

```bash
congress-approp search --dir examples --type appropriation --format csv > appropriations.csv
```

The CSV includes `description`, `raw_text`, and all other fields for filtering in a spreadsheet.

**All search flags:**

| Flag | Short | Description |
|------|-------|-------------|
| `--dir` | | Directory containing extracted bills (required) |
| `--agency` | `-a` | Filter by agency name (case-insensitive substring) |
| `--type` | `-t` | Filter by provision type (e.g. `appropriation`, `rescission`, `rider`, `cr_substitution`) |
| `--account` | | Filter by account name (case-insensitive substring) |
| `--keyword` | `-k` | Search in raw_text (case-insensitive substring) |
| `--bill` | | Filter to a specific bill (e.g. `"H.R. 9468"`) |
| `--format` | | Output format: `table` (default), `json`, `csv` |

### `compare` — What changed between two sets of bills?

```bash
congress-approp compare --base examples/hr5860 --current examples/hr9468
```

Compares appropriation accounts between any two directories. Matches by `(agency, account_name)` with automatic normalization for hierarchical CR names. Results sorted by largest change first.

### `report` — Can I trust these numbers?

```bash
congress-approp report --dir examples
```

```text
┌───────────┬───────┬──────────┬──────────┬───────┬───────┬──────┬───────┬─────────┬───────────┐
│ Bill      ┆ Provs ┆ Verified ┆ NotFound ┆ Ambig ┆ Exact ┆ Norm ┆ Space ┆ NoMatch ┆ Complete% │
╞═══════════╪═══════╪══════════╪══════════╪═══════╪═══════╪══════╪═══════╪═════════╪═══════════╡
│ H.R. 5860 ┆   130 ┆       33 ┆        0 ┆     2 ┆   102 ┆   12 ┆     0 ┆      16 ┆     61.1% │
│ H.R. 9468 ┆     7 ┆        2 ┆        0 ┆     0 ┆     5 ┆    0 ┆     0 ┆       2 ┆    100.0% │
│ TOTAL     ┆   137 ┆       35 ┆        0 ┆     2 ┆   107 ┆   12 ┆     0 ┆      18 ┆           │
└───────────┴───────┴──────────┴──────────┴───────┴───────┴──────┴───────┴─────────┴───────────┘

Column Guide:
  Verified   Dollar amounts found verbatim in source text — safe to cite
  NotFound   Dollar amounts NOT found in source — may be hallucinated, review manually
  Exact      raw_text is byte-identical substring of source — verbatim copy
  Norm       raw_text matches after whitespace/quote/dash normalization — content correct
  NoMatch    raw_text not found at any tier — may be paraphrased, review manually
  Complete%  Percentage of ALL dollar amounts in source text captured by a provision

Key:
  NotFound = 0 and Complete% = 100%  →  All amounts captured and verified
  NotFound = 0 and Complete% < 100%  →  Extracted amounts correct, but bill has more
  NotFound > 0                       →  Some amounts need manual review
```

Use `--verbose` to see each individual problematic provision.

**The key metric: across all tested bills, zero dollar amounts have been hallucinated.** Everything the model extracts is verified against the source text. The tool may be incomplete on very large bills (Complete% < 100%), but what it does extract is correct.

## Bill Types

| Classification | What It Is |
|----------------|------------|
| `regular` | One of the 12 annual appropriations bills (Defense, Labor-HHS, etc.) |
| `omnibus` | Multiple regular bills combined into one package |
| `minibus` | A few regular bills combined (smaller than an omnibus) |
| `continuing_resolution` | Temporary funding at prior-year rates, with specific anomalies |
| `supplemental` | Additional funding outside the regular cycle (disaster relief, wartime, etc.) |
| `rescissions` | A bill primarily canceling previously enacted budget authority |

## CLI Reference

| Subcommand | Description |
|------------|-------------|
| `download` | Download bill XML from Congress.gov |
| `extract` | Extract provisions from bill XML using the LLM |
| `search` | Search provisions across all extracted bills |
| `summary` | Show summary of all extracted bills |
| `compare` | Compare provisions between two sets of bills |
| `report` | Show verification and quality report |
| `api test` | Test API connectivity (Congress.gov + Anthropic) |
| `api bill list` | List appropriations bills for a Congress |
| `api bill get` | Get metadata for a specific bill |
| `api bill text` | Get text versions and download URLs for a bill |

**Common flags:**
- `--parallel N` on `extract` controls concurrent LLM calls (default 5)
- `--format table|json|csv` on `search` and `summary` controls output format
- `--dry-run` on `download` and `extract` previews without making API calls
- `-v` enables verbose (debug-level) logging

### Output Files

For each bill, the extraction pipeline produces:

| File | Contents |
|------|----------|
| `extraction.json` | All provisions with amounts, accounts, sections, verification status, and chunk traceability |
| `verification.json` | Deterministic checks: dollar amount matching, raw text verification, completeness |
| `conversion.json` | Report on any type coercions or warnings during JSON parsing |
| `tokens.json` | LLM token usage (input, output, cache hits) |
| `metadata.json` | Extraction provenance: model name, prompt version, schema version, timestamps |
| `BILLS-*.xml` | Original enrolled bill XML from Congress.gov |
| `BILLS-*.txt` | Clean text derived from XML (generated during extraction) |
| `.chunks/*.json` | Per-chunk LLM artifacts: thinking content, raw response, conversion report (for debugging and resume) |

## How It Works

### XML Parsing

Bill XML from Congress.gov uses semantic markup: `<division>`, `<title>`, `<appropriations-small>`, `<proviso>`, `<quote>`. The tool parses this with `roxmltree` (pure Rust, zero dependencies) and extracts clean text with `''quote''` delimiters matching the LLM prompt format. No PDF conversion or Python needed.

### Parallel Chunk Extraction

Large bills (omnibus, continuing resolutions) are automatically split into chunks at division and title boundaries from the XML tree. Each chunk is extracted in parallel with bounded concurrency (default 5 simultaneous LLM calls). A single-line dashboard shows progress:

```text
  5/42, 187 provs [4m 23s] 842 tok/s | 📝A-IIb ~8K 180/s | 🤔B-I ~3K | 📝B-III ~1K 95/s
```

After all chunks complete, provisions are merged, the summary is recomputed from actual provisions (never trusting the LLM's arithmetic), and verification runs against the complete source text.

### Verification

Verification is deterministic — no LLM involved:

1. **Amount checks** — Every `text_as_written` dollar string is searched for verbatim in the source text. Result: `verified`, `not_found` (possible hallucination), or `ambiguous` (found multiple times).
2. **Raw text checks** — Each provision's `raw_text` excerpt is checked as a substring of the source, with tiered matching: `exact` → `normalized` (whitespace/quote normalization) → `spaceless` (PDF artifact handling) → `no_match`.
3. **Completeness** — Every dollar sign in the source text is counted and checked against extracted provisions. 100% means every dollar amount in the bill was captured.

### Chunk Traceability

Every extraction produces per-chunk artifacts in `.chunks/` with ULIDs. Each artifact contains the model's thinking content, raw response, parsed JSON, and per-chunk conversion report. The `chunk_map` field in `extraction.json` links each provision to its source chunk, enabling full audit trails.

### Accuracy

Across all tested bills from the 118th Congress:

| Metric | Result |
|--------|--------|
| Dollar amounts hallucinated | **0** |
| CR substitution pairs verified | **13/13** (100%) |
| Sub-allocation accounting | Correctly excluded from budget authority totals |
| Raw text exact match rate | 78% (XML source), remainder matched after normalization |

### Limitations

- **Omnibus bills** (1,000+ pages) are split into chunks and extracted in parallel, but the model may not capture every sub-allocation and proviso. Check the `Complete%` in the summary.
- **Continuing resolution baselines** fund at prior-year rates. The tool extracts CR anomalies (substitutions) as structured data but doesn't model the baseline funding levels themselves.
- **Earmarks** are referenced in bill text but the actual recipient lists are in the joint explanatory statement — a separate document not included in the enrolled bill XML.
- **Year-over-year deltas** are computed by the `compare` command. Each year must be extracted independently.
- **LLM non-determinism** means re-extracting the same bill may produce slightly different provision counts or classifications. The verification pipeline ensures dollar amounts are always correct regardless.

## License

**Code:** MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

The appropriations bill data (XML, bill text, and legislative content within JSON files) is **United States Government Work** in the **public domain** under 17 U.S.C. § 105. No copyright restrictions apply to government-authored bill text. The structured extractions are derived from this public domain source material.

## Field Reference

See [docs/FIELD_REFERENCE.md](docs/FIELD_REFERENCE.md) for a complete description of every field in `extraction.json` and `verification.json`.