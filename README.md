# Congressional Appropriations Analyzer

[![Documentation](https://img.shields.io/badge/docs-mdbook-blue)](https://cgorski.github.io/congress-appropriations/) [![Crates.io](https://img.shields.io/crates/v/congress-appropriations)](https://crates.io/crates/congress-appropriations) [![CI](https://github.com/cgorski/congress-appropriations/actions/workflows/ci.yml/badge.svg)](https://github.com/cgorski/congress-appropriations/actions/workflows/ci.yml)

A command-line tool that downloads U.S. federal appropriations bills from Congress.gov, extracts every spending provision into structured JSON using Claude Opus 4.6, and checks each dollar amount against the source text.

> 📖 **[Read the full documentation →](https://cgorski.github.io/congress-appropriations/)**
>
> The documentation book includes tutorials, how-to guides, detailed explanations of the extraction pipeline and verification system, a complete CLI reference, and contributor guides.

The goal: make the ~1,500 pages of annual appropriations bills searchable, sortable, and machine-readable — so you can quickly answer questions like "how much did Congress appropriate for VA Compensation and Pensions?" or "which programs got cut in the continuing resolution?"

**Pre-processed data available:** The [`data/`](data/) directory includes completed extractions for fourteen enacted appropriations bills across the 118th and 119th Congresses (FY2024–FY2026), covering all twelve appropriations subcommittees. 11,136 provisions, $8.9 trillion in budget authority, pre-enriched metadata, and pre-computed embeddings. No API keys required to query these.

## Quick Start — Try It Now

### Install

```bash
git clone https://github.com/cgorski/congress-appropriations.git
cd congress-appropriations
cargo install --path .
```

This places the `congress-approp` binary on your PATH. You need **Rust 1.93+** ([install via rustup](https://rustup.rs/)). After modifying the source, run `cargo install --path .` again to rebuild.

### Explore the Example Data (No API Keys Required)

The `data/` directory contains pre-extracted data from fourteen bills, with pre-computed embeddings for semantic search and pre-enriched metadata for fiscal year and subcommittee filtering. Try these commands right away:

```bash
# See what bills are available and their budget authority totals
congress-approp summary --dir data

# Semantic search — find provisions by meaning, not just keywords.
# "school lunch programs for kids" has zero keyword overlap with the result,
# but semantic search finds it instantly:
congress-approp search --dir data --semantic "school lunch programs for kids" --top 5
```

```text
┌──────┬───────────┬───────────────┬─────────────────────────────┬────────────────┬─────┐
│ Sim  ┆ Bill      ┆ Type          ┆ Description / Account       ┆     Amount ($) ┆ Div │
╞══════╪═══════════╪═══════════════╪═════════════════════════════╪════════════════╪═════╡
│ 0.51 ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs    ┆ 33,266,226,000 ┆ B   │
│ 0.46 ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs    ┆     10,000,000 ┆ B   │
│ 0.45 ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs    ┆     18,004,000 ┆ B   │
└──────┴───────────┴───────────────┴─────────────────────────────┴────────────────┴─────┘
```

Semantic search requires pre-computed embeddings (included for example data) and `OPENAI_API_KEY` at query time. See [Semantic Search](#semantic-search) below.

```bash
# Find all appropriations across every bill
congress-approp search --dir data --type appropriation

# What programs got funding changes in the continuing resolution?
congress-approp search --dir data/118-hr5860 --type cr_substitution

# Find all FEMA-related provisions
congress-approp search --dir data --keyword "Federal Emergency Management"

# Find a provision in the supplemental, then see what matches in the omnibus
congress-approp search --dir data --similar 118-hr9468:0 --top 5

# Export everything to CSV for Excel
congress-approp search --dir data --type appropriation --format csv > appropriations.csv
```

### Enrich Bills for Fiscal Year and Subcommittee Filtering

The `enrich` command generates metadata that enables fiscal year and subcommittee filtering. It requires no API keys — it parses the bill XML and uses deterministic classification rules.

```bash
# Generate bill metadata (no API keys needed)
congress-approp enrich --dir data

# Now you can filter by fiscal year
congress-approp summary --dir data --fy 2026

# Filter by subcommittee jurisdiction
congress-approp search --dir data --semantic "housing assistance" --fy 2026 --subcommittee thud --top 5

# Compare across fiscal years for a specific subcommittee
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir data
```

Without `enrich`, the `--fy` flag still works for basic filtering (using fiscal year data from the extraction). The `--subcommittee` flag requires `enrich` because it needs the division-to-jurisdiction mapping that `enrich` generates.

The `enrich` command also classifies each budget authority provision as current-year or advance appropriation using a fiscal-year-aware algorithm, and enriches bill classifications (e.g., identifying H.R. 1968 as a "full-year CR with appropriations" rather than just a "continuing resolution").

### Included Bills

**118th Congress (FY2024/FY2025):**

| Directory | Bill | Type | Provisions | Budget Auth |
|-----------|------|------|-----------|------------|
| `data/118-hr4366/` | H.R. 4366 | FY2024 omnibus (MilCon-VA, Ag, CJS, E&W, Interior, THUD) | 2,364 | $846B |
| `data/118-hr5860/` | H.R. 5860 | FY2024 initial CR + 13 anomalies | 130 | $16B |
| `data/118-hr9468/` | H.R. 9468 | VA supplemental | 7 | $2.9B |
| `data/118-hr815/` | H.R. 815 | Ukraine/Israel/Taiwan supplemental | 303 | $95B |
| `data/118-hr2872/` | H.R. 2872 | Further CR (FY2024) | 31 | $0 |
| `data/118-hr6363/` | H.R. 6363 | Further CR + extensions | 74 | ~$0 |
| `data/118-hr7463/` | H.R. 7463 | CR extension | 10 | $0 |
| `data/118-hr9747/` | H.R. 9747 | CR + extensions (FY2025) | 114 | $383M |
| `data/118-s870/` | S. 870 | Fire Admin authorization | 49 | $0 |

**119th Congress (FY2025/FY2026):**

| Directory | Bill | Type | Provisions | Budget Auth |
|-----------|------|------|-----------|------------|
| `data/119-hr1968/` | H.R. 1968 | Full-year CR with appropriations (FY2025) | 526 | $1,786B |
| `data/119-hr5371/` | H.R. 5371 | Minibus: CR + Ag + LegBranch + MilCon-VA | 1,048 | $681B |
| `data/119-hr6938/` | H.R. 6938 | Minibus: CJS + Energy-Water + Interior | 1,061 | $196B |
| `data/119-hr7148/` | H.R. 7148 | Omnibus: Defense + Labor-HHS + THUD + FinServ + State | 2,837 | $2,788B |

**Totals:** 11,136 provisions, $8.9 trillion in budget authority, 0 unverifiable dollar amounts. All twelve appropriations subcommittees are covered for FY2026.

Each directory contains the source XML, extracted provisions, verification report, bill metadata (`bill_meta.json` from `enrich`), and pre-computed embeddings. All query commands (`search`, `summary`, `compare`, `audit`, `relate`) work against these directories. Embedding vectors (`vectors.bin`) are included in the git repository but excluded from the crates.io package — run `congress-approp embed --dir data` to regenerate them if you installed via `cargo install`.

## How Federal Appropriations Work

Congress funds the federal government through **annual appropriations bills** — legislation that grants agencies the legal authority to spend money. This tool extracts and structures those bills.

### The Fiscal Year

The federal fiscal year runs **October 1 to September 30**. FY2024 = October 2023 – September 2024. Bills are typically labeled by the fiscal year they fund, not the calendar year they're enacted in.

### Bill Types

| Classification | What It Is |
|----------------|------------|
| `regular` | One of the 12 annual appropriations bills (Defense, Labor-HHS, etc.) |
| `omnibus` | Multiple regular bills combined into one package |
| `minibus` | A few regular bills combined (smaller than an omnibus) |
| `continuing_resolution` | Temporary funding at prior-year rates, with specific anomalies |
| `supplemental` | Additional funding outside the regular cycle (disaster relief, wartime, etc.) |
| `rescissions` | A bill primarily canceling previously enacted budget authority |

### Scope: What This Covers (and Doesn't)

This tool extracts **discretionary appropriations** — the spending Congress votes on each year through the twelve annual appropriations bills (plus supplementals and continuing resolutions). That's roughly **26% of total federal spending**. It does **not** cover mandatory spending (Social Security, Medicare, Medicaid — about 63%) or net interest on the debt (about 11%).

The amounts represent **budget authority** (what Congress authorizes agencies to obligate), not **outlays** (what the Treasury actually disburses). This is why the numbers you'll see — around $1.7–1.9 trillion — don't match the ~$6–7 trillion headline federal budget figure.

### Congress Numbers

Each Congress lasts two years. The **118th Congress** covered **2023–2024**. The **119th Congress** covers **2025–2026**. Bills are identified by Congress number — for example, H.R. 4366 from the 118th Congress is a different bill than H.R. 4366 from any other Congress.

### Glossary

> **Enacted** — Signed into law by the President (or veto overridden).
>
> **Enrolled** — The final version of a bill passed by both chambers, sent to the President. This is the version the tool downloads.
>
> **Omnibus** — A single bill packaging multiple (often all twelve) annual appropriations bills together. Congress frequently uses omnibuses when individual bills stall.
>
> **Continuing Resolution (CR)** — Temporary legislation that funds the government at prior-year rates, usually with specific exceptions called "anomalies" that raise or lower particular accounts.
>
> **Supplemental** — Additional appropriations enacted outside the normal cycle, typically for emergencies (disaster relief, wartime funding, pandemic response).
>
> **Budget Authority** — The legal authority Congress grants to agencies to enter into obligations (contracts, grants, salaries). Distinct from outlays, which are the actual cash disbursements.

## How It Works

```text
Congress.gov   XML Parser    Claude Opus 4.6   Verification      Query
     │              │               │               │               │
     ▼              ▼               ▼               ▼               ▼
┌──────────┐  ┌──────────┐  ┌─────────────┐  ┌──────────┐  ┌──────────┐
│ Download │─▶│ Parse    │─▶│ LLM extract │─▶│ Verify   │─▶│ Search   │
│ bill XML │  │ XML      │  │ (parallel)  │  │ amounts  │  │ Compare  │
└──────────┘  └──────────┘  └─────────────┘  └──────────┘  └──────────┘
 BILLS-*.xml   clean text   extraction.json  verification.json
```

1. **Download** — Fetch enrolled bill XML from Congress.gov (structured, semantic markup).
2. **Parse** — Extract clean text from XML using `roxmltree` in pure Rust. The XML provides exact structural boundaries for divisions, titles, and sections.
3. **Extract** — Send bill text to Claude Opus 4.6 with adaptive thinking. Large bills are automatically split into chunks and extracted in parallel. Every provision — appropriations, rescissions, CR anomalies, riders, directives — is captured as structured JSON.
4. **Verify** — Deterministically check every dollar amount and text excerpt against the source. No LLM involved. Pure string matching with tiered fallback (exact → normalized → spaceless).
5. **Query** — Search, summarize, compare, and verify across all extracted bills using built-in subcommands.

## Download and Extract Your Own Bills

### Prerequisites

| Requirement | Description | Where to Get It |
|-------------|-------------|-----------------|
| **Rust 1.93+** | Build toolchain | [Install via rustup](https://rustup.rs/) |
| **Congress.gov API key** | Access to bill metadata and XML | Free — [sign up here](https://api.congress.gov/sign-up/) |
| **Anthropic API key** | LLM extraction of provisions | [Sign up here](https://console.anthropic.com/) |

> **Note:** The Anthropic key is only needed for extraction — exploring pre-extracted data (the `data/` directory or any previously extracted bills) is completely free.

```bash
# Set your API keys
export CONGRESS_API_KEY="your-key"
export ANTHROPIC_API_KEY="your-key"
```

### Discover Available Bills

Use `api bill list` to see what appropriations bills exist for a given Congress:

```bash
# List all appropriations bills for the 118th Congress
congress-approp api bill list --congress 118

# List only enacted appropriations bills
congress-approp api bill list --congress 118 --enacted-only
```

### Bill Type Codes

When downloading specific bills, you need the bill type code:

| Code | Meaning |
|------|---------|
| `hr` | House bill (e.g., H.R. 4366) |
| `s` | Senate bill |
| `hjres` | House joint resolution |
| `sjres` | Senate joint resolution |

Most enacted appropriations bills originate in the House (`hr`), since the Constitution requires revenue and spending bills to originate there.

### Download a Single Bill

```bash
# Download one bill (enrolled version XML from Congress.gov)
congress-approp download --congress 118 --type hr --number 9468 --output-dir data
```

The `download` command fetches the enrolled XML and creates the directory structure.

### Download All Enacted Bills for a Congress

```bash
# Scan for all enacted appropriations bills and download XML
congress-approp download --congress 118 --enacted-only --output-dir data
```

### Extract Provisions

```bash
# Extract provisions from a single bill
congress-approp extract --dir data/118/hr/9468

# Extract all downloaded bills with parallel chunk processing
congress-approp extract --dir data --parallel 6
```

The `extract` command parses the XML, sends text to the LLM, and runs deterministic verification. Large bills (omnibus, continuing resolutions) are automatically split into chunks at division and title boundaries. Use `--parallel N` to control concurrent LLM calls (default 5). Use `--dry-run` to preview without making API calls.

## Querying Extracted Bills

### `summary` — What bills do I have?

```bash
# See all FY2026 bills (uses --fy to filter; run `enrich` first for enriched classifications)
congress-approp summary --dir data --fy 2026
```

```text
┌───────────┬────────────────┬────────────┬───────────────────┬─────────────────┬───────────────────┐
│ Bill      ┆ Classification ┆ Provisions ┆   Budget Auth ($) ┆ Rescissions ($) ┆        Net BA ($) │
╞═══════════╪════════════════╪════════════╪═══════════════════╪═════════════════╪═══════════════════╡
│ H.R. 5371 ┆ Minibus        ┆       1048 ┆   681,142,644,860 ┆  16,999,000,000 ┆   664,143,644,860 │
│ H.R. 6938 ┆ Minibus        ┆       1061 ┆   196,377,983,000 ┆   5,874,200,000 ┆   190,503,783,000 │
│ H.R. 7148 ┆ Omnibus        ┆       2837 ┆ 2,787,914,783,135 ┆  34,581,747,670 ┆ 2,753,333,035,465 │
│ S. 870    ┆ Authorization  ┆         49 ┆                 0 ┆               0 ┆                 0 │
│ TOTAL     ┆                ┆       4995 ┆ 3,665,435,410,995 ┆  57,454,947,670 ┆ 3,607,980,463,325 │
└───────────┴────────────────┴────────────┴───────────────────┴─────────────────┴───────────────────┘

0 dollar amounts unverified across all bills. Run `congress-approp audit` for detailed verification.
```

Without `--fy`, the summary shows all 14 bills across FY2024–FY2026 ($8.9 trillion total). Use `--subcommittee thud` to narrow further to a specific jurisdiction, and `--show-advance` to separate current-year from advance appropriations.

Budget authority is computed from the actual provisions, not the LLM's self-reported summary.

The `audit` command provides a detailed verification breakdown per bill, including a coverage metric showing what percentage of dollar strings in the source text were matched to extracted provisions. Every extracted dollar amount is independently verified against the source text — 0 amounts were unverified across all example data.

**Understanding budget authority totals:** The Budget Auth column includes all provisions with `new_budget_authority` semantics at the `top_level` or `line_item` detail level. This includes both discretionary appropriations and mandatory spending programs that appear as appropriation lines in the bill text (e.g., SNAP at $122B in the Agriculture division). It also includes advance appropriations — funds enacted in this bill but available in the next fiscal year. The tool faithfully extracts what the bill text says; distinguishing mandatory from discretionary requires authorizing-law context beyond the bill itself. Advance appropriations are identified in the `notes` field.

### `search` — Find provisions across bills

Tables adapt automatically to the provision type you're searching for.

**Find all appropriations:**

```bash
congress-approp search --dir data --type appropriation
```

```text
┌───┬───────────┬───────────────┬───────────────────────────────────────────────┬───────────────┬──────────┬─────┐
│ $ ┆ Bill      ┆ Type          ┆ Description / Account                         ┆    Amount ($) ┆ Section  ┆ Div │
╞═══╪═══════════╪═══════════════╪═══════════════════════════════════════════════╪═══════════════╪══════════╪═════╡
│ ✓ ┆ H.R. 9468 ┆ appropriation ┆ Compensation and Pensions                     ┆ 2,285,513,000 ┆          ┆     │
│ ✓ ┆ H.R. 9468 ┆ appropriation ┆ Readjustment Benefits                         ┆   596,969,000 ┆          ┆     │
│ ✓ ┆ H.R. 5860 ┆ appropriation ┆ Federal Emergency Management Agency—Disaster…  ┆16,000,000,000 ┆ SEC. 129 ┆ A   │
└───┴───────────┴───────────────┴───────────────────────────────────────────────┴───────────────┴──────────┴─────┘
```

The **$** column shows verification status: ✓ means the dollar amount string was found at exactly one position in the source text.

**Find CR anomalies (which programs got funding changes):**

```bash
congress-approp search --dir data/118-hr5860 --type cr_substitution
```

```text
┌───┬───────────┬──────────────────────────────────────────┬───────────────┬───────────────┬──────────────┬──────────┬─────┐
│ $ ┆ Bill      ┆ Account                                  ┆       New ($) ┆       Old ($) ┆    Delta ($) ┆ Section  ┆ Div │
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
congress-approp search --dir data/118-hr9468 --type directive
```

```text
┌───┬───────────┬────────────────────────────────────────────────────────────────────────┬──────────┬─────┐
│ $ ┆ Bill      ┆ Description                                                            ┆ Section  ┆ Div │
╞═══╪═══════════╪════════════════════════════════════════════════════════════════════════╪══════════╪═════╡
│   ┆ H.R. 9468 ┆ Requires the Secretary of Veterans Affairs to submit a report detaili… ┆ SEC. 103 ┆     │
│   ┆ H.R. 9468 ┆ Requires the Secretary of Veterans Affairs to submit a report on the … ┆ SEC. 103 ┆     │
│   ┆ H.R. 9468 ┆ Requires the Inspector General of the Department of Veterans Affairs … ┆ SEC. 104 ┆     │
└───┴───────────┴────────────────────────────────────────────────────────────────────────┴──────────┴─────┘
```

**Export to CSV for Excel:**

```bash
congress-approp search --dir data --type appropriation --format csv > appropriations.csv
```

```text
bill,provision_type,account_name,description,agency,dollars,old_dollars,semantics,...,raw_text
H.R. 9468,appropriation,Compensation and Pensions,Compensation and Pensions,Department of Veterans Affairs,2285513000,,new_budget_authority,...
H.R. 9468,appropriation,Readjustment Benefits,Readjustment Benefits,Department of Veterans Affairs,596969000,,new_budget_authority,...
```

The CSV includes `description`, `raw_text`, and all other fields for filtering in a spreadsheet.

**Export to JSON for programmatic use:**

```bash
congress-approp search --dir data/118-hr9468 --type directive --format json
```

```json
[
  {
    "bill": "H.R. 9468",
    "provision_type": "directive",
    "description": "Requires the Secretary of Veterans Affairs to submit a report detailing corrections...",
    "section": "SEC. 103",
    "raw_text": "SEC. 103. (a) Not later than 30 days after the date of enactment...",
    "verified": null,
    "dollars": null
  }
]
```

JSON output includes every field for each matching provision, suitable for piping to `jq` or loading in scripts.

**Realistic use cases:**

```bash
# What did Congress cut in the CR? (shows old and new amounts with delta)
congress-approp search --dir data --type cr_substitution

# All FEMA-related provisions across all bills
congress-approp search --dir data --keyword "Federal Emergency Management"

# Export all rescissions to a spreadsheet
congress-approp search --dir data --type rescission --format csv > rescissions.csv

# What reporting requirements apply to the VA?
congress-approp search --dir data --keyword "Veterans Affairs" --type directive

# Which mandatory programs were extended in the CR?
congress-approp search --dir data --type mandatory_spending_extension --format json | jq '.[].description'
```

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
congress-approp compare --base data/118-hr5860 --current data/118-hr9468
```

Compares appropriation accounts between any two directories. Matches by `(agency, account_name)` with automatic normalization for hierarchical CR names. Results sorted by largest change first.

### `audit` — Can I trust these numbers?

```bash
congress-approp audit --dir data
```

The audit table shows verification metrics for all 14 bills — Verified (unique attribution), Ambiguous (multiple positions), NotFound, and raw text match tiers (Exact, Normalized, Spaceless, NoMatch). Across all 11,136 provisions: **0 NotFound** amounts.

```text
Column Guide:
  Verified   Dollar amount string found at exactly one position in source text
  NotFound   Dollar amounts NOT found in source — review manually
  Exact      raw_text is byte-identical substring of source — verbatim copy
  NormText   raw_text matches after whitespace/quote/dash normalization — content correct
  Spaceless  raw_text matches after removing all spaces — catches text artifacts
  TextMiss   raw_text not found at any tier — may be paraphrased, review manually
  Coverage   Percentage of ALL dollar amounts in source text captured by a provision

Key:
  NotFound = 0 and Coverage = 100%  →  All amounts captured and found in source
  NotFound = 0 and Coverage < 100%  →  Extracted amounts found in source, but bill has more
  NotFound > 0                      →  Some amounts need manual review
```

Use `--verbose` to see each individual problematic provision.

**The key metric: across 11,136 provisions from fourteen bills, every extracted dollar amount was found in the source bill text** (NotFound = 0 for every bill). Verification confirms amounts exist in the bill, not that they are attributed to the correct provision — for 95.5% of provisions, the raw text excerpt also matches verbatim, providing strong attribution confidence. The tool may be incomplete on large bills (Coverage < 100%), but what it does extract checks out against the source.

## Export Data

Every query command supports `--format csv`, `--format json`, and `--format jsonl` for getting data into other tools.

```bash
# Export appropriations to a spreadsheet
congress-approp search --dir data --type appropriation --format csv > provisions.csv

# Budget totals as JSON
congress-approp summary --dir data --format json

# Full nested data via jq
cat data/118-hr7148/extraction.json | jq '.provisions[] | select(.provision_type=="appropriation")'
```

> ⚠️ **CSV includes sub-allocations and reference amounts.** Don't sum the `dollars` column directly — filter to `semantics=new_budget_authority` and exclude `detail_level=sub_allocation` for correct totals. Or use `congress-approp summary` which does this automatically. See [Export Data for Spreadsheets and Scripts](https://cgorski.github.io/congress-appropriations/tutorials/export-data.html) for details.

## Entity Resolution

When comparing bills across fiscal years, the same program may appear under different agency names (e.g., "Department of Defense—Army" vs "Department of Defense—Department of the Army"). The tool matches by exact name by default — no implicit normalization.

To discover and resolve naming differences:

```bash
# Find obvious naming variants (free, instant, no API key)
congress-approp normalize suggest-text-match --dir data

# Review suggestions — each has an 8-char hash
# Accept specific ones:
congress-approp normalize accept a3f7b201 c92de445 --dir data

# Or accept all:
congress-approp normalize accept --auto --dir data

# For ambiguous cases, use LLM analysis (requires ANTHROPIC_API_KEY)
congress-approp normalize suggest-llm --dir data

# View current rules:
congress-approp normalize list --dir data

# Re-run comparison — orphans are now matched:
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee defense --dir data
```

Normalizations are stored in `dataset.json` — a small, human-editable JSON file at the data root. Use `--exact` on compare to disable all normalization and see raw matching results. See [Resolving Agency and Account Name Differences](https://cgorski.github.io/congress-appropriations/how-to/entity-resolution.html) for the full workflow.

## CLI Reference

| Subcommand | Description |
|------------|-------------|
| `download` | Download bill XML from Congress.gov |
| `extract` | Extract provisions from bill XML using the LLM |
| `enrich` | Generate bill metadata for FY/subcommittee filtering (no API key needed) |
| `embed` | Generate embeddings for semantic search (requires `OPENAI_API_KEY`) |
| `search` | Search provisions across all extracted bills |
| `summary` | Show summary of all extracted bills |
| `compare` | Compare provisions between two sets of bills (use `--exact` to disable normalization) |
| `audit` | Show verification and quality report |
| `relate` | Deep-dive on one provision across all bills (requires embeddings) |
| `normalize suggest-text-match` | Discover agency naming variants using local analysis |
| `normalize suggest-llm` | Discover agency naming variants using LLM with XML context |
| `normalize accept` | Accept suggestions by hash, write to `dataset.json` |
| `normalize list` | Show current entity resolution rules |
| `upgrade` | Upgrade extraction data to the latest schema version (re-verifies, no LLM needed) |
| `api test` | Test API connectivity (Congress.gov + Anthropic) |
| `api bill list` | List appropriations bills for a Congress |
| `api bill get` | Get metadata for a specific bill |
| `api bill text` | Get text versions and download URLs for a bill |

**Common flags:**
- `--parallel N` on `extract` controls concurrent LLM calls (default 5)
- `--format table|json|jsonl|csv` on `search` and `summary` controls output format
- `--semantic <query>` on `search` ranks results by meaning similarity (requires embeddings)
- `--similar <bill:index>` on `search` finds provisions similar to a specific one (e.g., `118-hr9468:0`)
- `--exact` on `compare` disables all normalization from `dataset.json`
- `--min-accounts N` on `normalize suggest-text-match` filters to stronger suggestions
- `--by-agency` on `summary` shows budget authority by parent department
- `--fy <YEAR>` on `summary`, `search`, `compare` filters to bills covering that fiscal year
- `--subcommittee <SLUG>` on `summary`, `search`, `compare` filters by jurisdiction (requires `enrich`)
- `--base-fy <YEAR>` and `--current-fy <YEAR>` on `compare` for FY-based comparison
- `--division`, `--min-dollars`, `--max-dollars` on `search` for filtering
- `--dry-run` on `download`, `extract`, `embed`, `enrich`, and `upgrade` previews without making changes
- `-v` enables verbose (debug-level) logging

### Semantic Search

Semantic search finds provisions by meaning, not just keywords. It uses OpenAI embeddings to understand that "school lunch programs for kids" means "Child Nutrition Programs" even though the words don't match.

**Setup:**

```bash
# Generate embeddings (one-time, ~30 seconds per bill)
export OPENAI_API_KEY="your-key"
congress-approp embed --dir data

# Embeddings are pre-generated for the example data if you cloned the git repo.
# If you installed via `cargo install`, run `embed` to generate them (~30 sec per bill).
```

**Search by meaning:**

```bash
# Find provisions about a topic — works even when keywords don't match
congress-approp search --dir data --semantic "opioid crisis drug treatment"

# Combine semantic search with filters
congress-approp search --dir data --semantic "clean energy" --type appropriation --min-dollars 100000000

# Find provisions similar to a specific one across all bills
congress-approp search --dir data --similar 118-hr9468:0 --top 5
```

The `embed` command writes `embeddings.json` (metadata) and `vectors.bin` (binary float32 vectors) to each bill directory. It skips bills whose embeddings are already up to date. Use `--dry-run` to preview token counts before calling the API.

> **Note on embedding availability:** The `vectors.bin` files are included in the git repository so that `git clone` users can use semantic search immediately. However, they are excluded from the crates.io package (they exceed the 10 MB upload limit). If you installed via `cargo install`, run `congress-approp embed --dir data` to generate embeddings for the example data. This takes approximately 30 seconds per bill and requires an `OPENAI_API_KEY`.

### Output Files

For each bill, the extraction and enrichment pipeline produces:

| File | Contents |
|------|----------|
| `extraction.json` | All provisions with amounts, accounts, sections, verification status, and chunk traceability |
| `verification.json` | Deterministic checks: dollar amount matching, raw text verification, completeness |
| `bill_meta.json` | Bill metadata: fiscal years, subcommittee jurisdictions, advance classification, canonical accounts (generated by `enrich`) |
| `conversion.json` | Report on any type coercions or warnings during JSON parsing |
| `tokens.json` | LLM token usage (input, output, cache hits) |
| `metadata.json` | Extraction provenance: model name, prompt version, schema version, timestamps |
| `BILLS-*.xml` | Original enrolled bill XML from Congress.gov |
| `BILLS-*.txt` | Clean text derived from XML (generated during extraction) |
| `chunks/*.json` | Per-chunk LLM artifacts: thinking content, raw response, conversion report (provenance and analysis) |

## Technical Details

### XML Parsing

Bill XML from Congress.gov uses semantic markup: `<division>`, `<title>`, `<appropriations-small>`, `<proviso>`, `<quote>`. The tool parses this with `roxmltree` (pure Rust) and extracts clean text with `''quote''` delimiters matching the LLM prompt format.

### Parallel Chunk Extraction

Large bills (omnibus, continuing resolutions) are automatically split into chunks at division and title boundaries from the XML tree. Each chunk is extracted in parallel with bounded concurrency (default 5 simultaneous LLM calls). A single-line dashboard shows progress:

```text
  5/42, 187 provs [4m 23s] 842 tok/s | 📝A-IIb ~8K 180/s | 🤔B-I ~3K | 📝B-III ~1K 95/s
```

After all chunks complete, provisions are merged, the summary is recomputed from actual provisions (never trusting the LLM's arithmetic), and verification runs against the complete source text.

### Verification

Verification is deterministic — no LLM involved:

1. **Amount checks** — Every `text_as_written` dollar string is searched for verbatim in the source text. Result: `verified` (found at unique position), `not_found` (not present in source), or `ambiguous` (found at multiple positions).
2. **Raw text checks** — Each provision's `raw_text` excerpt is checked as a substring of the source, with tiered matching: `exact` → `normalized` (whitespace/quote normalization) → `spaceless` (all spaces removed) → `no_match`.
3. **Completeness** — Every dollar sign in the source text is counted and checked against extracted provisions. 100% means every dollar amount in the bill was captured.

### Chunk Traceability

Every extraction produces per-chunk artifacts in `chunks/` with ULIDs. Each artifact contains the model's thinking content, raw response, parsed JSON, and per-chunk conversion report — permanent provenance records that enable analysis of how the LLM interpreted each section of the bill. The `chunk_map` field in `extraction.json` links each provision to its source chunk, enabling full audit trails.

### Accuracy

Across 11,136 provisions from fourteen enacted appropriations bills (118th and 119th Congress, FY2024–FY2026):

| Metric | Result |
|--------|--------|
| Provisions extracted | 11,136 across 14 bills |
| Dollar amounts not found in source | **0** |
| Dollar amount internal consistency | **0 mismatches** across all provisions with parsed amounts |
| CR substitution pairs verified | **100%** |
| Sub-allocation accounting | Correctly excluded from budget authority totals |
| Raw text exact match rate | 95.5% |
| Advance appropriations detected | $1.49 trillion (FY-aware classification, 100% accuracy) |

The `audit` command shows a detailed verification breakdown including a coverage metric (percentage of dollar strings in the source text matched to an extracted provision). Coverage below 100% does not indicate errors — many dollar strings in bill text are statutory references, loan guarantee ceilings, or old amounts being struck by amendments, all of which are correctly excluded from extraction.

### Token Usage

Extraction sends bill text to the LLM in parallel chunks. Use `extract --dry-run` to preview chunk counts and estimated tokens before running.

| Bill | Chunks | Estimated Input Tokens |
|------|--------|----------------------|
| H.R. 4366 (omnibus, 1.8 MB XML) | 75 | ~315,000 |
| H.R. 5860 (CR, 131 KB XML) | 5 | ~25,000 |
| H.R. 9468 (supplemental, 9 KB XML) | 1 | ~1,200 |

Output tokens vary by bill complexity. The `tokens.json` file in each bill directory records exact input, output, and cache-read token counts after extraction.

### Limitations

- **Omnibus bills** (1,000+ pages) are split into chunks and extracted in parallel. The FY2024 omnibus (H.R. 4366) extracted 2,364 provisions in approximately 60 minutes using `--parallel 6`. Use `audit` for detailed verification metrics.
- **Continuing resolution baselines** fund at prior-year rates. The tool extracts CR anomalies (substitutions) as structured data but doesn't model the baseline funding levels themselves.
- **Earmarks** are referenced in bill text but the actual recipient lists are in the joint explanatory statement — a separate document not included in the enrolled bill XML.
- **Year-over-year deltas** are computed by the `compare` command. Each year must be extracted independently.
- **LLM non-determinism** means re-extracting the same bill may produce slightly different provision counts or classifications. The verification pipeline ensures dollar amounts are always correct regardless.

## License

**Code:** MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

The appropriations bill data (XML, bill text, and legislative content within JSON files) is **United States Government Work** in the **public domain** under 17 U.S.C. § 105. No copyright restrictions apply to government-authored bill text. The structured extractions are derived from this public domain source material.

## Field Reference

See [docs/FIELD_REFERENCE.md](docs/FIELD_REFERENCE.md) for a complete description of every field in `extraction.json` and `verification.json`.