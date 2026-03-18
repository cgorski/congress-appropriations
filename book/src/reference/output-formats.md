# Output Formats

Every query command (`search`, `summary`, `compare`, `audit`) supports multiple output formats via the `--format` flag. This reference documents each format with examples and usage notes.

## Available Formats

| Format | Flag | Best For |
|--------|------|----------|
| Table | `--format table` (default) | Interactive exploration, quick lookups, terminal display |
| JSON | `--format json` | Programmatic consumption, Python/R/JavaScript, piping to `jq` |
| JSONL | `--format jsonl` | Streaming line-by-line processing, `xargs`, `parallel`, large result sets |
| CSV | `--format csv` | Excel, Google Sheets, R, pandas, any spreadsheet application |

All formats are available on `search`, `summary`, and `compare`. The `audit` command only supports table output.

---

## Table (Default)

Human-readable formatted table with Unicode box-drawing characters. Columns adapt to content width. Long text is truncated with `…`.

```bash
congress-approp search --dir examples/hr9468
```

```text
┌───┬───────────┬───────────────┬───────────────────────────────────────────────┬───────────────┬──────────┬─────┐
│ $ ┆ Bill      ┆ Type          ┆ Description / Account                         ┆    Amount ($) ┆ Section  ┆ Div │
╞═══╪═══════════╪═══════════════╪═══════════════════════════════════════════════╪═══════════════╪══════════╪═════╡
│ ✓ ┆ H.R. 9468 ┆ appropriation ┆ Compensation and Pensions                     ┆ 2,285,513,000 ┆          ┆     │
│ ✓ ┆ H.R. 9468 ┆ appropriation ┆ Readjustment Benefits                         ┆   596,969,000 ┆          ┆     │
│   ┆ H.R. 9468 ┆ rider         ┆ Establishes that each amount appropriated o…  ┆             — ┆ SEC. 101 ┆     │
│   ┆ H.R. 9468 ┆ rider         ┆ Unless otherwise provided, the additional a…  ┆             — ┆ SEC. 102 ┆     │
│   ┆ H.R. 9468 ┆ directive     ┆ Requires the Secretary of Veterans Affairs …  ┆             — ┆ SEC. 103 ┆     │
│   ┆ H.R. 9468 ┆ directive     ┆ Requires the Secretary of Veterans Affairs …  ┆             — ┆ SEC. 103 ┆     │
│   ┆ H.R. 9468 ┆ directive     ┆ Requires the Inspector General of the Depar…  ┆             — ┆ SEC. 104 ┆     │
└───┴───────────┴───────────────┴───────────────────────────────────────────────┴───────────────┴──────────┴─────┘
7 provisions found
```

### Table characteristics

- **Dollar amounts** are formatted with commas (e.g., `2,285,513,000`)
- **Missing amounts** show `—` (em-dash) for provisions without dollar values
- **Long text** is truncated with `…` to fit terminal width
- **Verification symbols** in the `$` column: `✓` (found unique), `≈` (found multiple), `✗` (not found), blank (no amount)
- **Row count** is shown below the table

### Adaptive table layouts

The table changes its column structure depending on what you're searching for:

**Standard search:** `$`, Bill, Type, Description/Account, Amount ($), Section, Div

**CR substitution search (`--type cr_substitution`):** `$`, Bill, Account, New ($), Old ($), Delta ($), Section, Div

**Semantic/similar search (`--semantic` or `--similar`):** Sim, Bill, Type, Description/Account, Amount ($), Div

**Summary table:** Bill, Classification, Provisions, Budget Auth ($), Rescissions ($), Net BA ($)

**Compare table:** Account, Agency, Base ($), Current ($), Delta ($), Δ %, Status

### When to use

- Interactive exploration at the terminal
- Quick spot-checks and lookups
- Sharing results in chat or email (the Unicode formatting renders well in most contexts)
- Any situation where you're reading results directly rather than processing them

---

## JSON

A JSON array of objects. Every matching provision is included with **all available fields** — more data than the table can show.

```bash
congress-approp search --dir examples/hr9468 --type appropriation --format json
```

```json
[
  {
    "account_name": "Compensation and Pensions",
    "agency": "Department of Veterans Affairs",
    "amount_status": "found",
    "bill": "H.R. 9468",
    "description": "Compensation and Pensions",
    "division": "",
    "dollars": 2285513000,
    "match_tier": "exact",
    "old_dollars": null,
    "provision_index": 0,
    "provision_type": "appropriation",
    "quality": "strong",
    "raw_text": "For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.",
    "section": "",
    "semantics": "new_budget_authority"
  },
  {
    "account_name": "Readjustment Benefits",
    "agency": "Department of Veterans Affairs",
    "amount_status": "found",
    "bill": "H.R. 9468",
    "description": "Readjustment Benefits",
    "division": "",
    "dollars": 596969000,
    "match_tier": "exact",
    "old_dollars": null,
    "provision_index": 1,
    "provision_type": "appropriation",
    "quality": "strong",
    "raw_text": "For an additional amount for ''Readjustment Benefits'', $596,969,000, to remain available until expended.",
    "section": "",
    "semantics": "new_budget_authority"
  }
]
```

### JSON fields (search output)

| Field | Type | Description |
|-------|------|-------------|
| `bill` | string | Bill identifier (e.g., `"H.R. 9468"`) |
| `provision_type` | string | Provision type (e.g., `"appropriation"`) |
| `provision_index` | integer | Zero-based index in the bill's provision array |
| `account_name` | string | Account name (empty string if not applicable) |
| `description` | string | Description of the provision |
| `agency` | string | Agency name (empty string if not applicable) |
| `dollars` | integer or null | Dollar amount as plain integer, or null if no amount |
| `old_dollars` | integer or null | Old amount for CR substitutions, null for other types |
| `semantics` | string | Amount semantics: `new_budget_authority`, `rescission`, `reference_amount`, `limitation`, `transfer_ceiling`, `mandatory_spending` |
| `section` | string | Section reference (e.g., `"SEC. 101"`) |
| `division` | string | Division letter (empty string if none) |
| `raw_text` | string | Bill text excerpt (~150 characters) |
| `amount_status` | string or null | `"found"`, `"found_multiple"`, `"not_found"`, or null (no amount) |
| `match_tier` | string | `"exact"`, `"normalized"`, `"spaceless"`, `"no_match"` |
| `quality` | string | `"strong"`, `"moderate"`, `"weak"`, or `"n/a"` |

### JSON fields (summary output)

```bash
congress-approp summary --dir examples --format json
```

```json
[
  {
    "identifier": "H.R. 4366",
    "classification": "Omnibus",
    "provisions": 2364,
    "budget_authority": 846137099554,
    "rescissions": 24659349709,
    "net_ba": 821477749845,
    "completeness_pct": 94.23298731257208
  }
]
```

| Field | Type | Description |
|-------|------|-------------|
| `identifier` | string | Bill identifier |
| `classification` | string | Bill classification |
| `provisions` | integer | Total provision count |
| `budget_authority` | integer | Total budget authority (computed from provisions) |
| `rescissions` | integer | Total rescissions (absolute value) |
| `net_ba` | integer | Budget authority minus rescissions |
| `completeness_pct` | float | Coverage percentage from verification |

### JSON fields (compare output)

```bash
congress-approp compare --base examples/hr4366 --current examples/hr9468 --format json
```

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | Account name |
| `agency` | string | Agency name |
| `base_dollars` | integer | Budget authority in `--base` bills |
| `current_dollars` | integer | Budget authority in `--current` bills |
| `delta` | integer | Current minus base |
| `delta_pct` | float | Percentage change |
| `status` | string | `"changed"`, `"unchanged"`, `"only in base"`, `"only in current"` |

### Piping to jq

JSON output is designed for piping to [`jq`](https://jqlang.github.io/jq/):

```bash
# Total budget authority
congress-approp search --dir examples --type appropriation --format json | \
  jq '[.[] | select(.semantics == "new_budget_authority") | .dollars] | add'

# Top 5 by dollars
congress-approp search --dir examples --type appropriation --format json | \
  jq 'sort_by(-.dollars) | .[:5] | .[] | "\(.dollars)\t\(.account_name)"'

# Unique account names
congress-approp search --dir examples --type appropriation --format json | \
  jq '[.[].account_name] | unique | sort | .[]'

# Group by agency
congress-approp search --dir examples --type appropriation --format json | \
  jq 'group_by(.agency) | map({agency: .[0].agency, count: length, total: [.[].dollars // 0] | add}) | sort_by(-.total)'
```

### Loading in Python

```python
import json
import subprocess

# From a file
with open("provisions.json") as f:
    data = json.load(f)

# From subprocess
result = subprocess.run(
    ["congress-approp", "search", "--dir", "examples",
     "--type", "appropriation", "--format", "json"],
    capture_output=True, text=True
)
provisions = json.loads(result.stdout)

# With pandas
import pandas as pd
df = pd.read_json("provisions.json")
```

### Loading in R

```r
library(jsonlite)
provisions <- fromJSON("provisions.json")
```

### When to use

- Any programmatic consumption (Python, R, JavaScript, shell scripts)
- Piping to `jq` for ad-hoc filtering and aggregation
- When you need fields that the table truncates or hides
- When you need the `provision_index` for `--similar` searches

---

## JSONL (JSON Lines)

One JSON object per line, with no enclosing array brackets. Each line is independently parseable.

```bash
congress-approp search --dir examples/hr9468 --type appropriation --format jsonl
```

```text
{"account_name":"Compensation and Pensions","agency":"Department of Veterans Affairs","amount_status":"found","bill":"H.R. 9468","description":"Compensation and Pensions","division":"","dollars":2285513000,"match_tier":"exact","old_dollars":null,"provision_index":0,"provision_type":"appropriation","quality":"strong","raw_text":"For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.","section":"","semantics":"new_budget_authority"}
{"account_name":"Readjustment Benefits","agency":"Department of Veterans Affairs","amount_status":"found","bill":"H.R. 9468","description":"Readjustment Benefits","division":"","dollars":596969000,"match_tier":"exact","old_dollars":null,"provision_index":1,"provision_type":"appropriation","quality":"strong","raw_text":"For an additional amount for ''Readjustment Benefits'', $596,969,000, to remain available until expended.","section":"","semantics":"new_budget_authority"}
```

### JSONL characteristics

- **Same fields as JSON** — each line contains the same fields as a JSON array element
- **No array wrapper** — no `[` at the start or `]` at the end
- **Each line is self-contained** — can be parsed independently without reading the entire output
- **No trailing comma issues** — each line is a complete JSON object

### Shell processing

```bash
# Count provisions per bill
congress-approp search --dir examples --format jsonl | \
  jq -r '.bill' | sort | uniq -c | sort -rn

# Line-by-line processing
congress-approp search --dir examples --type appropriation --format jsonl | \
  while IFS= read -r line; do
    echo "$line" | jq -r '"\(.bill)\t\(.account_name)\t\(.dollars)"'
  done

# Filter with jq (works identically to JSON since jq handles JSONL natively)
congress-approp search --dir examples --format jsonl | \
  jq -r 'select(.dollars > 1000000000) | "\(.bill)\t$\(.dollars)\t\(.account_name)"'
```

### When to use JSONL vs. JSON

| Scenario | Use JSON | Use JSONL |
|----------|----------|-----------|
| Loading into Python/R/JavaScript | ✓ | |
| Piping to `jq` | Either works | ✓ (slightly more natural for streaming) |
| Line-by-line shell processing | | ✓ |
| `xargs` or `parallel` pipelines | | ✓ |
| Very large result sets | | ✓ (no need to load entire array into memory) |
| Appending to a log file | | ✓ |
| Need a single parseable document | ✓ | |

---

## CSV

Comma-separated values with a header row. Suitable for import into any spreadsheet application or data analysis tool.

```bash
congress-approp search --dir examples/hr9468 --type appropriation --format csv
```

```text
bill,provision_type,account_name,description,agency,dollars,old_dollars,semantics,detail_level,section,division,raw_text,amount_status,match_tier,quality,provision_index
H.R. 9468,appropriation,Compensation and Pensions,Compensation and Pensions,Department of Veterans Affairs,2285513000,,new_budget_authority,,,,For an additional amount for ''Compensation and Pensions''...,found,exact,strong,0
H.R. 9468,appropriation,Readjustment Benefits,Readjustment Benefits,Department of Veterans Affairs,596969000,,new_budget_authority,,,,For an additional amount for ''Readjustment Benefits''...,found,exact,strong,1
```

### CSV columns

The CSV output includes all the same fields as JSON, flattened into columns:

| Column | Type | Description |
|--------|------|-------------|
| `bill` | string | Bill identifier |
| `provision_type` | string | Provision type |
| `account_name` | string | Account name |
| `description` | string | Description |
| `agency` | string | Agency name |
| `dollars` | integer or empty | Dollar amount (no formatting, no `$` sign) |
| `old_dollars` | integer or empty | Old amount for CR substitutions |
| `semantics` | string | Amount semantics |
| `detail_level` | string | Detail level (appropriation types only) |
| `section` | string | Section reference |
| `division` | string | Division letter |
| `raw_text` | string | Bill text excerpt |
| `amount_status` | string or empty | Verification status |
| `match_tier` | string | Raw text match tier |
| `quality` | string | Quality assessment |
| `provision_index` | integer | Provision index |

### Opening in Excel

1. Save the output to a file: `congress-approp search --dir examples --format csv > provisions.csv`
2. Open Excel → File → Open → navigate to `provisions.csv`
3. If columns aren't detected automatically, use Data → From Text/CSV and select:
   - **Encoding:** UTF-8 (important for em-dashes and other Unicode characters)
   - **Delimiter:** Comma
   - **Data type detection:** Based on entire file

**Common gotchas:**

| Issue | Cause | Fix |
|-------|-------|-----|
| Large numbers in scientific notation (e.g., `8.46E+11`) | Excel auto-formatting | Format the `dollars` column as Number with 0 decimal places |
| Garbled characters (em-dashes, curly quotes) | Wrong encoding | Import with UTF-8 encoding explicitly |
| Extra line breaks in rows | `raw_text` or `description` contains newlines | The CSV properly quotes these fields; use the Import Wizard if simple Open doesn't handle them |

### Opening in Google Sheets

1. File → Import → Upload → select your `.csv` file
2. Import location: "Replace current sheet" or "Insert new sheet"
3. Separator type: Comma (should auto-detect)
4. Google Sheets handles UTF-8 natively

### Loading in pandas

```python
import pandas as pd

df = pd.read_csv("provisions.csv")

# Basic analysis
print(f"Total provisions: {len(df)}")
print(f"Total BA: ${df[df['semantics'] == 'new_budget_authority']['dollars'].sum():,.0f}")
print(df.groupby("agency")["dollars"].sum().sort_values(ascending=False).head(10))
```

### Loading in R

```r
provisions <- read.csv("provisions.csv", stringsAsFactors = FALSE)
```

### When to use

- Importing into Excel or Google Sheets
- Loading into R or pandas when you prefer CSV to JSON
- Any tabular data tool that doesn't support JSON
- Sharing data with non-technical colleagues who work in spreadsheets

---

## Summary: Choosing the Right Format

| I want to... | Use |
|--------------|-----|
| Explore data interactively at the terminal | `--format table` (default) |
| Process data in Python, R, or JavaScript | `--format json` |
| Pipe to `jq` for quick filtering | `--format json` or `--format jsonl` |
| Stream results line by line in shell | `--format jsonl` |
| Import into Excel or Google Sheets | `--format csv` |
| Get all available fields | `--format json` or `--format csv` (table truncates) |
| Append to a log file incrementally | `--format jsonl` |
| Share results with non-technical colleagues | `--format csv` (for spreadsheets) or `--format table` (for email/chat) |

### Field availability comparison

| Field | Table | JSON | JSONL | CSV |
|-------|:-----:|:----:|:-----:|:---:|
| bill | ✓ | ✓ | ✓ | ✓ |
| provision_type | ✓ | ✓ | ✓ | ✓ |
| account_name / description | ✓ (truncated) | ✓ (full) | ✓ (full) | ✓ (full) |
| dollars | ✓ (formatted) | ✓ (integer) | ✓ (integer) | ✓ (integer) |
| old_dollars | ✓ (CR subs only) | ✓ | ✓ | ✓ |
| section | ✓ | ✓ | ✓ | ✓ |
| division | ✓ | ✓ | ✓ | ✓ |
| agency | — | ✓ | ✓ | ✓ |
| semantics | — | ✓ | ✓ | ✓ |
| detail_level | — | ✓ | ✓ | ✓ |
| raw_text | — | ✓ (full) | ✓ (full) | ✓ (full) |
| amount_status | ✓ (as symbol) | ✓ (as string) | ✓ (as string) | ✓ (as string) |
| match_tier | — | ✓ | ✓ | ✓ |
| quality | — | ✓ | ✓ | ✓ |
| provision_index | — | ✓ | ✓ | ✓ |

---

## Redirecting Output to Files

All formats can be redirected to a file using standard shell redirection:

```bash
# Save table output (includes Unicode characters)
congress-approp search --dir examples --type appropriation > results.txt

# Save JSON
congress-approp search --dir examples --type appropriation --format json > results.json

# Save JSONL
congress-approp search --dir examples --type appropriation --format jsonl > results.jsonl

# Save CSV
congress-approp search --dir examples --type appropriation --format csv > results.csv
```

> **Note:** The tool writes output to stdout and warnings/errors to stderr. Redirecting with `>` captures only stdout, so warnings (like "embeddings are stale") still appear on the terminal. To capture everything: `congress-approp search --dir examples --format json > results.json 2> warnings.txt`

---

## Next Steps

- **[Export Data for Spreadsheets and Scripts](../tutorials/export-data.md)** — tutorial with practical export recipes
- **[Filter and Search Provisions](../how-to/filter-and-search.md)** — all search flags for narrowing results before export
- **[CLI Command Reference](./cli.md)** — complete reference for all commands and flags