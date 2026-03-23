# CLI Command Reference

This is the complete reference for every `congress-approp` command and flag. For tutorials and worked examples, see the [Tutorials](../tutorials/find-spending-on-topic.md) section. For task-oriented guides, see [How-To Guides](../how-to/download-bills.md).

## Global Options

These flags can be used with any command:

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Enable verbose (debug-level) logging. Shows detailed progress, file paths, and internal state. |
| `--help` | `-h` | Print help for the command |
| `--version` | `-V` | Print version (top-level only) |

## summary

Show a per-bill overview of all extracted data: provision counts, budget authority, rescissions, and net budget authority.

```text
congress-approp summary [OPTIONS]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./data` | Data directory containing extracted bills. Try `examples` for included FY2024 data. Walks recursively to find all `extraction.json` files. |
| `--format` | string | `table` | Output format: `table`, `json`, `jsonl`, `csv` |
| `--by-agency` | flag | — | Append a second table showing budget authority totals by parent department, sorted descending |
| `--fy` | integer | — | Filter to bills covering this fiscal year (e.g., `2026`). Uses `bill.fiscal_years` from extraction data — works without `enrich`. |
| `--subcommittee` | string | — | Filter by subcommittee jurisdiction (e.g., `defense`, `thud`, `cjs`). Requires `bill_meta.json` — run `enrich` first. See [Enrich Bills with Metadata](../how-to/enrich-data.md) for valid slugs. |

### Examples

```bash
# FY2026 bills only
congress-approp summary --dir data --fy 2026

# FY2026 THUD subcommittee only (requires enrich)
congress-approp summary --dir data --fy 2026 --subcommittee thud
```


```bash
# Basic summary of included example data
congress-approp summary --dir data

# JSON output for scripting
congress-approp summary --dir data --format json

# Show department-level rollup
congress-approp summary --dir data --by-agency

# CSV for spreadsheet import
congress-approp summary --dir data --format csv > bill_summary.csv
```

### Output

The summary table shows one row per loaded bill plus a TOTAL row:

```text
┌───────────┬───────────────────────┬────────────┬─────────────────┬─────────────────┬─────────────────┐
│ Bill      ┆ Classification        ┆ Provisions ┆ Budget Auth ($) ┆ Rescissions ($) ┆      Net BA ($) │
╞═══════════╪═══════════════════════╪════════════╪═════════════════╪═════════════════╪═════════════════╡
│ H.R. 4366 ┆ Omnibus               ┆       2364 ┆ 846,137,099,554 ┆  24,659,349,709 ┆ 821,477,749,845 │
│ H.R. 5860 ┆ Continuing Resolution ┆        130 ┆  16,000,000,000 ┆               0 ┆  16,000,000,000 │
│ H.R. 9468 ┆ Supplemental          ┆          7 ┆   2,882,482,000 ┆               0 ┆   2,882,482,000 │
│ TOTAL     ┆                       ┆       2501 ┆ 865,019,581,554 ┆  24,659,349,709 ┆ 840,360,231,845 │
└───────────┴───────────────────────┴────────────┴─────────────────┴─────────────────┴─────────────────┘

0 dollar amounts unverified across all bills. Run `congress-approp audit` for detailed verification.
```

Budget Authority is computed from provisions (not from any LLM-generated summary). See [Budget Authority Calculation](../explanation/budget-authority.md) for the formula.

The `--by-agency` flag appends a second table with columns: Department, Budget Auth ($), Rescissions ($), Provisions.

---

## search

Search provisions across all extracted bills. Supports filtering by type, agency, account, keyword, division, dollar range, and meaning-based semantic search.

```text
congress-approp search [OPTIONS]
```

### Filter Flags

| Flag | Short | Type | Description |
|------|-------|------|-------------|
| `--dir` | | path | Data directory containing extracted bills. Default: `./data` |
| `--type` | `-t` | string | Filter by provision type. Use `--list-types` to see valid values. |
| `--agency` | `-a` | string | Filter by agency name (case-insensitive substring match) |
| `--account` | | string | Filter by account name (case-insensitive substring match) |
| `--keyword` | `-k` | string | Search in raw_text field (case-insensitive substring match) |
| `--bill` | | string | Filter to a specific bill identifier (e.g., `"H.R. 4366"`) |
| `--division` | | string | Filter by division letter (e.g., `A`, `B`, `C`) |
| `--min-dollars` | | integer | Minimum dollar amount (absolute value) |
| `--max-dollars` | | integer | Maximum dollar amount (absolute value) |
| `--fy` | | integer | Filter to bills covering this fiscal year (e.g., `2026`). Works without `enrich`. |
| `--subcommittee` | | string | Filter by subcommittee jurisdiction (e.g., `thud`, `defense`). Requires `enrich`. |

All filters use **AND logic** — every provision in the result must match every specified filter. Filter order on the command line has no effect on results.

### Semantic Search Flags

| Flag | Type | Description |
|------|------|-------------|
| `--semantic` | string | Rank results by meaning similarity to this query text. Requires pre-computed embeddings and `OPENAI_API_KEY`. |
| `--similar` | string | Find provisions similar to the one specified. Format: `<bill_directory>:<provision_index>` (e.g., `118-hr9468:0`). Uses stored vectors — **no API call needed**. |
| `--top` | integer | Maximum number of results for `--semantic` or `--similar` searches. Default: `20`. Has no effect on non-semantic searches (which return all matching provisions). |

### Output Flags

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--format` | string | `table` | Output format: `table`, `json`, `jsonl`, `csv` |
| `--list-types` | flag | — | Print all valid provision types and exit (ignores other flags) |

### Examples

```bash
# All appropriations across all example bills
congress-approp search --dir data --type appropriation

# VA appropriations over $1 billion in Division A
congress-approp search --dir data --type appropriation --agency "Veterans" --division A --min-dollars 1000000000

# FEMA-related provisions by keyword
congress-approp search --dir data --keyword "Federal Emergency Management"

# CR substitutions (table auto-adapts to show New/Old/Delta columns)
congress-approp search --dir data/118-hr5860 --type cr_substitution

# All directives in the VA supplemental
congress-approp search --dir data/118-hr9468 --type directive

# Semantic search — find by meaning, not keywords
congress-approp search --dir data --semantic "school lunch programs for kids" --top 5

# Find provisions similar to a specific one across all bills
congress-approp search --dir data --similar 118-hr9468:0 --top 5

# Combine semantic with hard filters
congress-approp search --dir data --semantic "clean energy" --type appropriation --min-dollars 100000000 --top 10

# Export to CSV for spreadsheet analysis
congress-approp search --dir data --type appropriation --format csv > appropriations.csv

# Export to JSON for programmatic use
congress-approp search --dir data --type rescission --format json

# List all valid provision types
congress-approp search --dir data --list-types
```

### Available Provision Types

```text
  appropriation                    Budget authority grant
  rescission                       Cancellation of prior budget authority
  cr_substitution                  CR anomaly (substituting $X for $Y)
  transfer_authority               Permission to move funds between accounts
  limitation                       Cap or prohibition on spending
  directed_spending                Earmark / community project funding
  mandatory_spending_extension     Amendment to authorizing statute
  directive                        Reporting requirement or instruction
  rider                            Policy provision (no direct spending)
  continuing_resolution_baseline   Core CR funding mechanism
  other                            Unclassified provisions
```

### Table Output Columns

The table adapts its shape based on the provision types in the results.

**Standard search table:**

| Column | Description |
|--------|-------------|
| `$` | Verification status: `✓` (found unique), `≈` (found multiple), `✗` (not found), blank (no dollar amount) |
| `Bill` | Bill identifier |
| `Type` | Provision type |
| `Description / Account` | Account name for appropriations/rescissions, description for other types |
| `Amount ($)` | Dollar amount, or `—` for provisions without amounts |
| `Section` | Section reference from the bill (e.g., `SEC. 101`) |
| `Div` | Division letter for omnibus bills |

**CR substitution table:** Replaces `Amount ($)` with `New ($)`, `Old ($)`, and `Delta ($)`.

**Semantic/similar table:** Adds a `Sim` column at the left showing cosine similarity (0.0–1.0).

### JSON/CSV Output Fields

JSON and CSV output include more fields than the table:

| Field | Type | Description |
|-------|------|-------------|
| `bill` | string | Bill identifier |
| `provision_type` | string | Provision type |
| `account_name` | string | Account name |
| `description` | string | Description |
| `agency` | string | Agency name |
| `dollars` | integer or null | Dollar amount |
| `old_dollars` | integer or null | Old amount (CR substitutions only) |
| `semantics` | string | Amount semantics (e.g., `new_budget_authority`) |
| `section` | string | Section reference |
| `division` | string | Division letter |
| `raw_text` | string | Bill text excerpt |
| `amount_status` | string or null | `found`, `found_multiple`, `not_found`, or null |
| `match_tier` | string | `exact`, `normalized`, `spaceless`, `no_match` |
| `quality` | string | `strong`, `moderate`, `weak`, or `n/a` |
| `provision_index` | integer | Index in the bill's provision array (zero-based) |

---

## compare

Compare provisions between two sets of bills. Matches accounts by `(agency, account_name)` and computes dollar deltas. Account names are matched case-insensitively with em-dash prefix stripping. If a `dataset.json` file exists in the data directory, agency groups and account aliases are applied for cross-bill matching. Use `--exact` to disable all normalization and match on exact lowercased strings only. See [Resolve Agency and Account Name Differences](../how-to/entity-resolution.md) for details.

There are two ways to specify what to compare:

**Directory-based** (compare two specific directories):
```text
congress-approp compare --base <BASE> --current <CURRENT> [OPTIONS]
```

**FY-based** (compare all bills for one fiscal year against another):
```text
congress-approp compare --base-fy <YEAR> --current-fy <YEAR> --dir <DIR> [OPTIONS]
```

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--base` | | path | — | Base directory for comparison (e.g., prior fiscal year) |
| `--current` | | path | — | Current directory for comparison (e.g., current fiscal year) |
| `--base-fy` | | integer | — | Use all bills covering this FY as the base set (alternative to `--base`) |
| `--current-fy` | | integer | — | Use all bills covering this FY as the current set (alternative to `--current`) |
| `--dir` | | path | `./data` | Data directory (required with `--base-fy`/`--current-fy`) |
| `--subcommittee` | | string | — | Scope comparison to one subcommittee jurisdiction. Requires `enrich`. |
| `--agency` | `-a` | string | — | Filter by agency name (case-insensitive substring) |
| `--real` | | flag | — | Add inflation-adjusted "Real Δ %*" column using CPI-U. Shows which programs beat inflation (▲) and which fell behind (▼). |
| `--cpi-file` | | path | — | Path to a custom CPI/deflator JSON file. Overrides the bundled CPI-U data. See [Adjust for Inflation](../how-to/inflation-adjustment.md) for the file format. |
| `--format` | | string | `table` | Output format: `table`, `json`, `csv` |

You must provide either `--base` + `--current` (directory paths) or `--base-fy` + `--current-fy` + `--dir`.

### Examples

```bash
# Compare omnibus to supplemental (directory-based)
congress-approp compare --base data/118-hr4366 --current data/118-hr9468

# Compare THUD funding: FY2024 → FY2026 (FY-based with subcommittee scope)
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir data

# Compare all FY2024 vs FY2026 (no subcommittee scope)
congress-approp compare --base-fy 2024 --current-fy 2026 --dir data

# Show inflation-adjusted changes (which programs beat inflation?)
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir data --real

# Filter to VA accounts only
congress-approp compare --base data/118-hr4366 --current data/118-hr9468 --agency "Veterans"

# Export comparison to CSV
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir data --format csv > thud_compare.csv
```

### Matching Behavior

Account matching uses several normalization layers:

- **Case-insensitive**: "Grants-In-Aid for Airports" matches "Grants-in-Aid for Airports"
- **Em-dash prefix stripping**: "Department of VA—Compensation and Pensions" matches "Compensation and Pensions"
- **Sub-agency normalization**: "Maritime Administration" matches "Department of Transportation" for the same account name
- **Hierarchical CR name matching**: "Federal Emergency Management Agency—Disaster Relief Fund" matches "Disaster Relief Fund"

### Output Columns

| Column | Description |
|--------|-------------|
| `Account` | Account name, matched between bills |
| `Agency` | Parent department or agency |
| `Base ($)` | Budget authority in the `--base` or `--base-fy` bills |
| `Current ($)` | Budget authority in the `--current` or `--current-fy` bills |
| `Delta ($)` | Current minus Base |
| `Δ %` | Percentage change |
| `Status` | `changed`, `unchanged`, `only in base`, or `only in current` |

Results are sorted by absolute delta, largest changes first. The tool warns when comparing different bill classifications (e.g., Omnibus vs. Supplemental).

---

## audit

Show a detailed verification and quality report for all extracted bills.

```text
congress-approp audit [OPTIONS]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./data` | Data directory to audit. Try `examples` for included FY2024 data. |
| `--verbose` | flag | — | Show individual problematic provisions (those with `not_found` amounts or `no_match` raw text) |

### Examples

```bash
# Standard audit
congress-approp audit --dir data

# Verbose — see individual problematic provisions
congress-approp audit --dir data --verbose
```

### Output

```text
┌───────────┬────────────┬──────────┬──────────┬───────┬───────┬──────────┬───────────┬──────────┬──────────┐
│ Bill      ┆ Provisions ┆ Verified ┆ NotFound ┆ Ambig ┆ Exact ┆ NormText ┆ Spaceless ┆ TextMiss ┆ Coverage │
╞═══════════╪════════════╪══════════╪══════════╪═══════╪═══════╪══════════╪═══════════╪══════════╪══════════╡
│ H.R. 4366 ┆       2364 ┆      762 ┆        0 ┆   723 ┆  2285 ┆       59 ┆         0 ┆       20 ┆    94.2% │
│ H.R. 5860 ┆        130 ┆       33 ┆        0 ┆     2 ┆   102 ┆       12 ┆         0 ┆       16 ┆    61.1% │
│ H.R. 9468 ┆          7 ┆        2 ┆        0 ┆     0 ┆     5 ┆        0 ┆         0 ┆        2 ┆   100.0% │
│ TOTAL     ┆       2501 ┆      797 ┆        0 ┆   725 ┆  2392 ┆       71 ┆         0 ┆       38 ┆          │
└───────────┴────────────┴──────────┴──────────┴───────┴───────┴──────────┴───────────┴──────────┴──────────┘
```

### Column Reference

**Amount verification (left side):**

| Column | Description |
|--------|-------------|
| **Verified** | Dollar amount found at exactly one position in source text |
| **NotFound** | Dollar amount NOT found in source — **should be 0**; review manually if > 0 |
| **Ambig** | Dollar amount found at multiple positions — correct but location is uncertain |

**Raw text verification (right side):**

| Column | Description |
|--------|-------------|
| **Exact** | `raw_text` is byte-identical substring of source text |
| **NormText** | `raw_text` matches after whitespace/quote/dash normalization |
| **Spaceless** | `raw_text` matches only after removing all spaces |
| **TextMiss** | `raw_text` not found at any tier — may be paraphrased or truncated |

**Completeness:**

| Column | Description |
|--------|-------------|
| **Coverage** | Percentage of dollar strings in source text matched to a provision. See [What Coverage Means](../explanation/coverage.md). |

See [Understanding the Output](../getting-started/understanding-output.md) and [Verify Extraction Accuracy](../how-to/verify-accuracy.md) for detailed interpretation guidance.

---

## download

Download appropriations bill XML from Congress.gov.

```text
congress-approp download [OPTIONS] --congress <CONGRESS>
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--congress` | integer | *(required)* | Congress number (e.g., `118` for 2023–2024) |
| `--type` | string | — | Bill type code: `hr`, `s`, `hjres`, `sjres` |
| `--number` | integer | — | Bill number (used with `--type` for single-bill download) |
| `--output-dir` | path | `./data` | Output directory. Intermediate directories are created as needed. |
| `--enacted-only` | flag | — | Only download bills signed into law |
| `--format` | string | `xml` | Download format: `xml` (for extraction), `pdf` (for reading). Comma-separated for multiple. |
| `--version` | string | — | Text version filter: `enr` (enrolled/final), `ih` (introduced), `eh` (engrossed). When omitted, only enrolled is downloaded. |
| `--all-versions` | flag | — | Download all text versions (introduced, engrossed, enrolled, etc.) instead of just enrolled |
| `--dry-run` | flag | — | Show what would be downloaded without fetching |

**Requires:** `CONGRESS_API_KEY` environment variable.

### Examples

```bash
# Download a specific bill (enrolled version only, by default)
congress-approp download --congress 118 --type hr --number 4366 --output-dir data

# Download all enacted bills for a congress (enrolled versions only)
congress-approp download --congress 118 --enacted-only --output-dir data

# Preview without downloading
congress-approp download --congress 118 --enacted-only --output-dir data --dry-run

# Download both XML and PDF
congress-approp download --congress 118 --type hr --number 4366 --output-dir data --format xml,pdf

# Download all text versions (introduced, engrossed, enrolled, etc.)
congress-approp download --congress 118 --type hr --number 4366 --output-dir data --all-versions
```

---

## extract

Extract spending provisions from bill XML using Claude. Parses the XML, sends text chunks to the LLM in parallel, merges results, and runs deterministic verification.

```text
congress-approp extract [OPTIONS]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./data` | Data directory containing downloaded bill XML |
| `--dry-run` | flag | — | Show chunk count and estimated tokens without calling the LLM |
| `--parallel` | integer | `5` | Number of concurrent LLM API calls. Higher is faster but uses more API quota. |
| `--model` | string | `claude-opus-4-6` | LLM model for extraction. Can also be set via `APPROP_MODEL` env var. Flag takes precedence. |
| `--force` | flag | — | Re-extract bills even if `extraction.json` already exists. Without this flag, already-extracted bills are skipped. |
| `--continue-on-error` | flag | — | Save partial results when some chunks fail. Without this flag, the tool aborts a bill if any chunk permanently fails and does not write `extraction.json`. |

**Requires:** `ANTHROPIC_API_KEY` environment variable (not required if all bills are already extracted).

**Behavior notes:**
- **Aborts on chunk failure by default.** If any chunk permanently fails (after all retries), the bill's extraction is aborted and no `extraction.json` is written. This prevents garbage partial extractions from being saved to disk. Use `--continue-on-error` to save partial results instead.
- **Per-bill error handling.** In a multi-bill run, a failure on one bill does not abort the entire run. The failed bill is skipped (no files written) and extraction continues with the remaining bills. Re-running the same command retries only the failed bills.
- **Skips already-extracted bills** by default. If every bill in `--dir` already has `extraction.json`, the command exits without requiring an API key. Use `--force` to re-extract.
- **Prefers enrolled XML.** When a directory has multiple `BILLS-*.xml` files, only the enrolled version (`*enr.xml`) is processed. Non-enrolled versions are ignored.
- **Resilient to parse failures.** If an XML file fails to parse (e.g., a non-enrolled version with a different structure), the tool logs a warning and continues to the next bill instead of aborting.

### Examples

```bash
# Preview extraction (no API calls)
congress-approp extract --dir data/118/hr/9468 --dry-run

# Extract a single bill
congress-approp extract --dir data/118/hr/9468

# Extract with higher parallelism for large bills
congress-approp extract --dir data/118/hr/4366 --parallel 8

# Extract all bills under a directory (skips already-extracted bills)
congress-approp extract --dir data --parallel 6

# Re-extract a bill that was already processed
congress-approp extract --dir data/118/hr/9468 --force

# Save partial results even when some chunks fail (rate limiting, etc.)
congress-approp extract --dir data/118/hr/2882 --parallel 6 --continue-on-error

# Use a different model
congress-approp extract --dir data/118/hr/9468 --model claude-sonnet-4-20250514
```

### Output Files

| File | Description |
|------|-------------|
| `extraction.json` | All provisions with structured fields |
| `verification.json` | Deterministic verification against source text |
| `metadata.json` | Model, prompt version, timestamps, source XML hash |
| `tokens.json` | Token usage (input, output, cache) |
| `chunks/` | Per-chunk LLM artifacts (gitignored) |

---

## embed

Generate semantic embedding vectors for extracted provisions using OpenAI's embedding model. Enables `--semantic` and `--similar` on the `search` command.

```text
congress-approp embed [OPTIONS]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./data` | Data directory containing extracted bills |
| `--model` | string | `text-embedding-3-large` | OpenAI embedding model |
| `--dimensions` | integer | `3072` | Number of dimensions to request from the API |
| `--batch-size` | integer | `100` | Provisions per API batch call |
| `--dry-run` | flag | — | Preview token counts without calling the API |

**Requires:** `OPENAI_API_KEY` environment variable.

Bills with up-to-date embeddings are automatically skipped (detected via hash chain).

### Examples

```bash
# Generate embeddings for all bills
congress-approp embed --dir data

# Preview without calling API
congress-approp embed --dir data --dry-run

# Generate for a single bill
congress-approp embed --dir data/118/hr/9468

# Use fewer dimensions (not recommended — see Generate Embeddings guide)
congress-approp embed --dir data --dimensions 1024
```

### Output Files

| File | Description |
|------|-------------|
| `embeddings.json` | Metadata: model, dimensions, count, SHA-256 hashes |
| `vectors.bin` | Raw little-endian float32 vectors (count × dimensions × 4 bytes) |

---

## enrich

Generate bill metadata for fiscal year filtering, subcommittee scoping, and advance appropriation classification. This command parses the source XML and analyzes the extraction output — **no API keys are required**.

```text
congress-approp enrich [OPTIONS]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./data` | Data directory containing extracted bills |
| `--dry-run` | flag | — | Preview what would be generated without writing files |
| `--force` | flag | — | Re-enrich even if `bill_meta.json` already exists |

### What It Generates

For each bill directory, `enrich` creates a `bill_meta.json` file containing:

- **Congress number** — parsed from the XML filename
- **Subcommittee mappings** — division letter → jurisdiction (e.g., Division A → Defense)
- **Bill nature** — enriched classification (omnibus, minibus, full-year CR with appropriations, etc.)
- **Advance appropriation classification** — each budget authority provision classified as current-year, advance, or supplemental using a fiscal-year-aware algorithm
- **Canonical account names** — case-normalized, prefix-stripped names for cross-bill matching

### Examples

```bash
# Enrich all bills
congress-approp enrich --dir data

# Preview without writing files
congress-approp enrich --dir data --dry-run

# Force re-enrichment
congress-approp enrich --dir data --force
```

### When to Run

Run `enrich` once after extracting bills, before using `--subcommittee` filters. The `--fy` flag on other commands works without `enrich` (it uses fiscal year data already in `extraction.json`), but `--subcommittee` requires the division-to-jurisdiction mapping that only `enrich` provides.

The tool warns when `bill_meta.json` is stale (when `extraction.json` has changed since enrichment). Run `enrich --force` to regenerate.

See [Enrich Bills with Metadata](../how-to/enrich-data.md) for a detailed guide including subcommittee slugs, advance classification algorithm, and provenance tracking.

---

## verify-text

Check that every provision's `raw_text` is a verbatim substring of the enrolled bill source text. Optionally repair mismatches and add `source_span` byte positions. No API key required.

```
congress-approp verify-text [OPTIONS]
  --dir <DIR>       Data directory [default: ./data]
  --repair          Fix broken raw_text and add source_span to every provision
  --bill <BILL>     Single bill directory (e.g., 118-hr2882)
  --format <FMT>    Output format: table, json [default: table]
```

### Examples

```bash
# Analyze all bills (no changes)
congress-approp verify-text --dir data

# Repair and add source spans
congress-approp verify-text --dir data --repair

# Single bill
congress-approp verify-text --dir data --bill 118-hr2882 --repair
```

### Output

Reports the number of provisions at each match tier:

```text
34568 provisions: 34568 exact, 0 repaired (0 prefix, 0 substring, 0 normalized), 0 unverified
Traceable: 34568/34568 (100.000%)

✅ Every provision is traceable to the enrolled bill source text.
```

When `--repair` is used, a backup is created at `extraction.json.pre-repair` before any modifications. Each provision gets a `source_span` field with UTF-8 byte offsets into the source `.txt` file.

See [Verifying Extraction Data](../how-to/verify-data.md) for details on the 3-tier repair algorithm and the source span invariant.

---

## resolve-tas

Map each top-level budget authority provision to a Federal Account Symbol (FAS) code from the Treasury's FAST Book. Uses deterministic string matching for unambiguous names and Claude Opus for the rest.

```
congress-approp resolve-tas [OPTIONS]
  --dir <DIR>              Data directory [default: ./data]
  --bill <BILL>            Single bill directory (e.g., 118-hr2882)
  --dry-run                Show what would be resolved and estimated cost
  --no-llm                 Deterministic matching only (no API key needed)
  --force                  Re-resolve even if tas_mapping.json exists
  --batch-size <N>         Provisions per LLM batch [default: 40]
  --fas-reference <PATH>   Path to FAS reference JSON [default: data/fas_reference.json]
```

Requires `ANTHROPIC_API_KEY` for the LLM tier. With `--no-llm`, no API key is needed (resolves ~56% of provisions).

### Examples

```bash
# Preview cost before running
congress-approp resolve-tas --dir data --dry-run

# Full resolution (deterministic + LLM)
congress-approp resolve-tas --dir data

# Free mode (deterministic only, no API key)
congress-approp resolve-tas --dir data --no-llm

# Single bill
congress-approp resolve-tas --dir data --bill 118-hr2882
```

### Output

Produces `tas_mapping.json` per bill with one mapping per top-level budget authority provision. Reports match rates:

```text
6685 provisions: 6645 matched (99.4%), 40 unmatched
  Deterministic: 3731, LLM: 2914
```

See [Resolving Treasury Account Symbols](../how-to/tas-resolution.md) for details on the two-tier matching algorithm, confidence levels, and the FAST Book reference.

---

## authority build

Aggregate all `tas_mapping.json` files into a single `authorities.json` account registry at the data root. Groups provisions by FAS code, collects name variants, and detects rename events.

```
congress-approp authority build [OPTIONS]
  --dir <DIR>       Data directory [default: ./data]
  --force           Rebuild even if authorities.json already exists
```

No API key required. Runs in ~1 second.

### Example

```bash
congress-approp authority build --dir data

# Output:
# Built authorities.json:
#   1051 authorities, 6645 provisions, 24 bills, FYs [2019, 2020, ..., 2026]
#   937 in multiple bills, 443 with name variants
```

---

## authority list

Browse the account authority registry. Shows FAS code, bill count, fiscal years, total budget authority, and official title for each authority.

```
congress-approp authority list [OPTIONS]
  --dir <DIR>       Data directory [default: ./data]
  --agency <CODE>   Filter by CGAC agency code (e.g., 070 for DHS)
  --format <FMT>    Output format: table, json [default: table]
```

### Examples

```bash
# List all authorities
congress-approp authority list --dir data

# Filter to DHS accounts
congress-approp authority list --dir data --agency 070

# JSON for programmatic use
congress-approp authority list --dir data --format json
```

---

## trace

Show the funding timeline for a federal budget account across all fiscal years in the dataset. Accepts a FAS code or a name search query.

```
congress-approp trace <QUERY> [OPTIONS]
  <QUERY>           FAS code (e.g., 070-0400) or account name fragment
  --dir <DIR>       Data directory [default: ./data]
  --format <FMT>    Output format: table, json [default: table]
```

Name search splits the query into words and matches authorities where all words appear across the title, agency name, FAS code, and name variants. If multiple authorities match, the command lists candidates and asks you to be more specific.

### Examples

```bash
# By FAS code (exact)
congress-approp trace 070-0400 --dir data

# By name (word-level search)
congress-approp trace "coast guard operations" --dir data
congress-approp trace "disaster relief" --dir data

# JSON output
congress-approp trace 070-0400 --dir data --format json
```

### Output

```text
TAS 070-0400: Operations and Support, United States Secret Service, Homeland Security
  Agency: Department of Homeland Security

┌──────┬──────────────────────┬────────────────┬──────────────────────────────┐
│ FY   ┆ Budget Authority ($) ┆ Bill(s)        ┆ Account Name(s)              │
╞══════╪══════════════════════╪════════════════╪══════════════════════════════╡
│ 2020 ┆        2,336,401,000 ┆ H.R. 1158      ┆ United States Secret Servi…  │
│ 2021 ┆        2,373,109,000 ┆ H.R. 133       ┆ United States Secret Servi…  │
│ 2022 ┆        2,554,729,000 ┆ H.R. 2471      ┆ Operations and Support       │
│ 2024 ┆        3,007,982,000 ┆ H.R. 2882      ┆ Operations and Support       │
│ 2025 ┆          231,000,000 ┆ H.R. 9747 (CR) ┆ United States Secret Servi…  │
└──────┴──────────────────────┴────────────────┴──────────────────────────────┘
```

Bill classification labels — `(CR)`, `(supplemental)`, `(full-year CR)` — are shown when the bill is not a regular or omnibus appropriation. Detected rename events are shown below the timeline. Name variants are listed with their classification type.

See [The Authority System](../explanation/authority-system.md) for details on how account tracking works across fiscal years.

---

## normalize suggest-text-match

Discover agency and account naming variants using orphan-pair analysis and structural regex patterns. Scans all bills for cross-FY orphan pairs (same account name, different agency) and common naming patterns (prefix expansion, preposition variants, abbreviation differences). Results are cached for the `normalize accept` command.

No API calls. No network access. Runs in milliseconds.

```text
congress-approp normalize suggest-text-match [OPTIONS]
  --dir <DIR>            Data directory [default: ./data]
  --format <FORMAT>      Output format: table, json, hashes [default: table]
  --min-accounts <N>     Minimum shared accounts to include a suggestion [default: 1]
```

Use `--format hashes` to output one hash per line for scripting. Use `--min-accounts 3` to filter to stronger suggestions (pairs sharing 3+ account names).

Suggestions are cached in `~/.congress-approp/cache/` and consumed by `normalize accept`.

---

## normalize suggest-llm

Discover agency and account naming variants using LLM classification with XML heading context. Sends unresolved ambiguous account clusters to Claude with the bill's XML organizational structure, dollar amounts, and fiscal year information. The LLM classifies agency pairs as SAME or DIFFERENT.

Requires `ANTHROPIC_API_KEY`. Uses Claude Opus.

```text
congress-approp normalize suggest-llm [OPTIONS]
  --dir <DIR>            Data directory [default: ./data]
  --batch-size <N>       Maximum clusters per API call [default: 15]
  --format <FORMAT>      Output format: table, json, hashes [default: table]
```

Only processes clusters not already resolved by `suggest-text-match` or existing `dataset.json` entries. Results are cached for the `normalize accept` command.

---

## normalize accept

Accept suggested normalizations by hash. Reads from the suggestion cache populated by `suggest-text-match` or `suggest-llm`, matches the specified hashes, and writes the accepted groups to `dataset.json`.

```text
congress-approp normalize accept [OPTIONS] [HASHES]...
  --dir <DIR>            Data directory [default: ./data]
  --auto                 Accept all cached suggestions without specifying hashes
```

If no cache exists, prints an error suggesting to run `suggest-text-match` first.

---

## normalize list

Display current entity resolution rules from `dataset.json`.

```text
congress-approp normalize list [OPTIONS]
  --dir <DIR>            Data directory [default: ./data]
```

Shows all agency groups and account aliases. If no `dataset.json` exists, shows a helpful message suggesting how to create one.

---

## relate

Deep-dive on one provision across all bills. Finds similar provisions by embedding similarity, groups them by confidence tier, and optionally builds a fiscal year timeline with advance/current/supplemental split. Requires pre-computed embeddings but **no API keys** (uses stored vectors).

```text
congress-approp relate <SOURCE> [OPTIONS]
```

The `<SOURCE>` argument is a provision reference in the format `bill_directory:index` (e.g., `118-hr9468:0`). Use the `provision_index` from search output.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./examples` | Data directory |
| `--top` | integer | `10` | Max related provisions per confidence tier |
| `--format` | string | `table` | Output format: `table`, `json`, `hashes` |
| `--fy-timeline` | flag | — | Show fiscal year timeline with advance/current/supplemental split |

### Output

The table output shows two sections:

- **Same Account** — high-confidence matches (verified name match or high similarity + same agency). Each row includes a deterministic 8-char hash, similarity score, bill, account name, dollar amount, funding timing, and confidence label.
- **Related** — lower-confidence matches (uncertain zone, 0.55–0.65 similarity or name mismatch).

With `--fy-timeline`, a third section shows the fiscal year timeline: current-year BA, advance BA, supplemental BA, and contributing bills for each fiscal year.

### Examples

```bash
# Deep-dive on VA Compensation and Pensions
congress-approp relate 118-hr9468:0 --dir data --fy-timeline

# Get just the link hashes for piping to `link accept`
congress-approp relate 118-hr9468:0 --dir data --format hashes

# JSON output with timeline
congress-approp relate 118-hr9468:0 --dir data --format json --fy-timeline
```

### Link Hashes

Each match includes a deterministic 8-character hex hash (e.g., `b7e688d7`). These hashes are computed from the source provision, target provision, and embedding model — the same inputs always produce the same hash. Use `--format hashes` to output just the hashes of same-account matches, suitable for piping to `link accept`:

```bash
congress-approp relate 118-hr9468:0 --dir data --format hashes | \
  xargs congress-approp link accept --dir data
```

---

## link suggest

Compute cross-bill link candidates from embeddings. For each top-level budget authority provision, finds the best match in every other bill above the similarity threshold and classifies by confidence tier.

```text
congress-approp link suggest [OPTIONS]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./examples` | Data directory |
| `--threshold` | float | `0.55` | Minimum similarity for candidates |
| `--scope` | string | `all` | Which bill pairs to compare: `intra` (within same FY), `cross` (across FYs), `all` |
| `--limit` | integer | `100` | Max candidates to output |
| `--format` | string | `table` | Output format: `table`, `json`, `hashes` |

### Confidence Tiers

Based on empirically calibrated thresholds from analysis of 6.7M pairwise comparisons:

| Tier | Criteria | Meaning |
|------|----------|---------|
| **verified** | Canonical account name match (case-insensitive, prefix-stripped) | Almost certainly the same account |
| **high** | Similarity ≥ 0.65 AND same normalized agency | Very likely the same account |
| **uncertain** | Similarity 0.55–0.65, or name mismatch above 0.65 | Needs manual review |

### Examples

```bash
# Cross-fiscal-year candidates (year-over-year tracking)
congress-approp link suggest --dir data --scope cross --limit 20

# All candidates above 0.65 similarity
congress-approp link suggest --dir data --threshold 0.65 --limit 50

# Output just the hashes of new (un-accepted) candidates
congress-approp link suggest --dir data --format hashes
```

---

## link accept

Persist link candidates by accepting them into `links/links.json` at the data root.

```text
congress-approp link accept [OPTIONS] [HASHES...]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./examples` | Data directory |
| `--note` | string | — | Optional annotation (e.g., "Account renamed from X to Y") |
| `--auto` | flag | — | Accept all verified + high-confidence candidates without specifying hashes |
| `HASHES` | positional | — | One or more 8-char link hashes to accept |

### Examples

```bash
# Accept specific links by hash
congress-approp link accept --dir data a3f7b2c4 e5d1c8a9

# Accept with a note
congress-approp link accept --dir data a3f7b2c4 --note "Same VA account, different bill vehicles"

# Auto-accept all verified and high-confidence candidates
congress-approp link accept --dir data --auto

# Pipe from relate output
congress-approp relate 118-hr9468:0 --dir data --format hashes | \
  xargs congress-approp link accept --dir data
```

---

## link remove

Remove accepted links by hash.

```text
congress-approp link remove --dir <DIR> <HASHES...>
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./examples` | Data directory |
| `HASHES` | positional | *(required)* | One or more 8-char link hashes to remove |

### Example

```bash
congress-approp link remove --dir data a3f7b2c4
```

---

## link list

Show accepted links, optionally filtered by bill.

```text
congress-approp link list [OPTIONS]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./examples` | Data directory |
| `--format` | string | `table` | Output format: `table`, `json` |
| `--bill` | string | — | Filter to links involving this bill (case-insensitive substring) |

### Examples

```bash
# Show all accepted links
congress-approp link list --dir data

# Filter to links involving H.R. 4366
congress-approp link list --dir data --bill hr4366

# JSON output for programmatic use
congress-approp link list --dir data --format json
```

---

## compare --use-authorities

The `compare` command accepts a `--use-authorities` flag that rescues orphan provisions by matching on FAS code instead of account name. When two provisions have the same FAS code but different names or agency attributions, they are recognized as the same account.

```bash
congress-approp compare --base-fy 2024 --current-fy 2026 \
    --subcommittee thud --dir data --use-authorities
```

Requires `tas_mapping.json` files for the bills being compared (run `resolve-tas` first). Orphan provisions rescued via TAS matching are labeled with their FAS code in the status column (e.g., `matched (TAS 069-1775)`).

This flag can be combined with `--use-links`, `--real`, and `--exact`. Entity resolution via `dataset.json` still applies unless `--exact` is specified.

---

## upgrade

Upgrade extraction data to the latest schema version. Re-deserializes existing data through the current parsing logic and re-runs verification. **No LLM API calls.**

```text
congress-approp upgrade [OPTIONS]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dir` | path | `./data` | Data directory to upgrade |
| `--dry-run` | flag | — | Show what would change without writing files |

### Examples

```bash
# Preview changes
congress-approp upgrade --dir data --dry-run

# Upgrade all bills
congress-approp upgrade --dir data

# Upgrade a single bill
congress-approp upgrade --dir data/118/hr/9468
```

---

## api test

Test API connectivity for Congress.gov and Anthropic.

```text
congress-approp api test
```

Verifies that `CONGRESS_API_KEY` and `ANTHROPIC_API_KEY` are set and that both APIs are reachable. No flags.

---

## api bill list

List appropriations bills for a given congress.

```text
congress-approp api bill list [OPTIONS]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--congress` | integer | *(required)* | Congress number |
| `--type` | string | — | Filter by bill type (`hr`, `s`, `hjres`, `sjres`) |
| `--offset` | integer | `0` | Pagination offset |
| `--limit` | integer | `20` | Maximum results per page |
| `--enacted-only` | flag | — | Only show enacted (signed into law) bills |

**Requires:** `CONGRESS_API_KEY`

### Examples

```bash
# All appropriations bills for the 118th Congress
congress-approp api bill list --congress 118

# Only enacted bills
congress-approp api bill list --congress 118 --enacted-only
```

---

## api bill get

Get metadata for a specific bill.

```text
congress-approp api bill get --congress <N> --type <TYPE> --number <N>
```

| Flag | Type | Description |
|------|------|-------------|
| `--congress` | integer | Congress number |
| `--type` | string | Bill type (`hr`, `s`, `hjres`, `sjres`) |
| `--number` | integer | Bill number |

**Requires:** `CONGRESS_API_KEY`

---

## api bill text

Get text versions and download URLs for a bill.

```text
congress-approp api bill text --congress <N> --type <TYPE> --number <N>
```

| Flag | Type | Description |
|------|------|-------------|
| `--congress` | integer | Congress number |
| `--type` | string | Bill type (`hr`, `s`, `hjres`, `sjres`) |
| `--number` | integer | Bill number |

**Requires:** `CONGRESS_API_KEY`

Lists every text version (introduced, engrossed, enrolled, etc.) with available formats (XML, PDF, HTML) and download URLs.

### Example

```bash
congress-approp api bill text --congress 118 --type hr --number 4366
```

---

## Common Patterns

### Query pre-extracted example data (no API keys needed)

```bash
congress-approp summary --dir data
congress-approp search --dir data --type appropriation
congress-approp audit --dir data
congress-approp compare --base data/118-hr4366 --current data/118-hr9468
```

### Full extraction pipeline

```bash
export CONGRESS_API_KEY="..."
export ANTHROPIC_API_KEY="..."
export OPENAI_API_KEY="..."

congress-approp download --congress 118 --enacted-only --output-dir data
congress-approp extract --dir data --parallel 6
congress-approp audit --dir data
congress-approp embed --dir data
congress-approp summary --dir data
```

### Export workflows

```bash
# All appropriations to CSV
congress-approp search --dir data --type appropriation --format csv > all.csv

# JSON for jq processing
congress-approp search --dir data --format json | jq '.[].account_name' | sort -u

# JSONL for streaming
congress-approp search --dir data --format jsonl | while IFS= read -r line; do echo "$line" | jq '.dollars'; done
```

## Environment Variables

| Variable | Used By | Description |
|----------|---------|-------------|
| `CONGRESS_API_KEY` | `download`, `api` commands | Congress.gov API key ([free signup](https://api.congress.gov/sign-up/)) |
| `ANTHROPIC_API_KEY` | `extract` | Anthropic API key for Claude |
| `OPENAI_API_KEY` | `embed`, `search --semantic` | OpenAI API key for embeddings |
| `APPROP_MODEL` | `extract` | Override default LLM model (flag takes precedence) |

See [Environment Variables and API Keys](./environment-variables.md) for details.

## Next Steps

- **[Filter and Search Provisions](../how-to/filter-and-search.md)** — detailed guide with practical recipes for the `search` command
- **[Understanding the Output](../getting-started/understanding-output.md)** — how to read every table the tool produces
- **[Provision Types](./provision-types.md)** — reference for all 11 provision types and their fields