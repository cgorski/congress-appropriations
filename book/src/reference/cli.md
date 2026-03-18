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

### Examples

```bash
# Basic summary of included example data
congress-approp summary --dir examples

# JSON output for scripting
congress-approp summary --dir examples --format json

# Show department-level rollup
congress-approp summary --dir examples --by-agency

# CSV for spreadsheet import
congress-approp summary --dir examples --format csv > bill_summary.csv
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

All filters use **AND logic** — every provision in the result must match every specified filter. Filter order on the command line has no effect on results.

### Semantic Search Flags

| Flag | Type | Description |
|------|------|-------------|
| `--semantic` | string | Rank results by meaning similarity to this query text. Requires pre-computed embeddings and `OPENAI_API_KEY`. |
| `--similar` | string | Find provisions similar to the one specified. Format: `<bill_directory>:<provision_index>` (e.g., `hr9468:0`). Uses stored vectors — **no API call needed**. |
| `--top` | integer | Maximum number of results for `--semantic` or `--similar` searches. Default: `20`. Has no effect on non-semantic searches (which return all matching provisions). |

### Output Flags

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--format` | string | `table` | Output format: `table`, `json`, `jsonl`, `csv` |
| `--list-types` | flag | — | Print all valid provision types and exit (ignores other flags) |

### Examples

```bash
# All appropriations across all example bills
congress-approp search --dir examples --type appropriation

# VA appropriations over $1 billion in Division A
congress-approp search --dir examples --type appropriation --agency "Veterans" --division A --min-dollars 1000000000

# FEMA-related provisions by keyword
congress-approp search --dir examples --keyword "Federal Emergency Management"

# CR substitutions (table auto-adapts to show New/Old/Delta columns)
congress-approp search --dir examples/hr5860 --type cr_substitution

# All directives in the VA supplemental
congress-approp search --dir examples/hr9468 --type directive

# Semantic search — find by meaning, not keywords
congress-approp search --dir examples --semantic "school lunch programs for kids" --top 5

# Find provisions similar to a specific one across all bills
congress-approp search --dir examples --similar hr9468:0 --top 5

# Combine semantic with hard filters
congress-approp search --dir examples --semantic "clean energy" --type appropriation --min-dollars 100000000 --top 10

# Export to CSV for spreadsheet analysis
congress-approp search --dir examples --type appropriation --format csv > appropriations.csv

# Export to JSON for programmatic use
congress-approp search --dir examples --type rescission --format json

# List all valid provision types
congress-approp search --dir examples --list-types
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

Compare provisions between two sets of bills. Matches accounts by `(agency, account_name)` and computes dollar deltas.

```text
congress-approp compare [OPTIONS] --base <BASE> --current <CURRENT>
```

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--base` | | path | *(required)* | Base directory for comparison (e.g., prior fiscal year) |
| `--current` | | path | *(required)* | Current directory for comparison (e.g., current fiscal year) |
| `--agency` | `-a` | string | — | Filter by agency name (case-insensitive substring) |
| `--format` | | string | `table` | Output format: `table`, `json`, `csv` |

### Examples

```bash
# Compare omnibus to supplemental
congress-approp compare --base examples/hr4366 --current examples/hr9468

# Filter to VA accounts only
congress-approp compare --base examples/hr4366 --current examples/hr9468 --agency "Veterans"

# Export comparison to CSV
congress-approp compare --base examples/hr4366 --current examples/hr9468 --format csv > comparison.csv
```

### Output Columns

| Column | Description |
|--------|-------------|
| `Account` | Account name, matched between bills |
| `Agency` | Parent department or agency |
| `Base ($)` | Budget authority in the `--base` bills |
| `Current ($)` | Budget authority in the `--current` bills |
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
congress-approp audit --dir examples

# Verbose — see individual problematic provisions
congress-approp audit --dir examples --verbose
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

**Requires:** `ANTHROPIC_API_KEY` environment variable (not required if all bills are already extracted).

**Behavior notes:**
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
congress-approp summary --dir examples
congress-approp search --dir examples --type appropriation
congress-approp audit --dir examples
congress-approp compare --base examples/hr4366 --current examples/hr9468
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
congress-approp search --dir examples --type appropriation --format csv > all.csv

# JSON for jq processing
congress-approp search --dir examples --format json | jq '.[].account_name' | sort -u

# JSONL for streaming
congress-approp search --dir examples --format jsonl | while IFS= read -r line; do echo "$line" | jq '.dollars'; done
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