# Export Data for Spreadsheets and Scripts

> **You will need:** `congress-approp` installed, access to the `examples/` directory.
>
> **You will learn:** How to get appropriations data into Excel, Google Sheets, Python, R, and shell pipelines using the four output formats: CSV, JSON, JSONL, and table.

The `congress-approp` CLI is great for interactive exploration, but most analysis workflows eventually need the data in another tool — a spreadsheet for a briefing, a pandas DataFrame for statistical analysis, or a `jq` pipeline for automation. Every query command supports four output formats via the `--format` flag, and this tutorial shows you how to use each one effectively.

## CSV for Spreadsheets

CSV is the most portable format for getting data into Excel, Google Sheets, LibreOffice Calc, or any other spreadsheet application.

### Basic export

```bash
congress-approp search --dir examples --type appropriation --format csv > appropriations.csv
```

This writes a file with a header row and one row per matching provision. Here's what the first few lines look like:

```text
bill,provision_type,account_name,description,agency,dollars,old_dollars,semantics,detail_level,section,division,raw_text,amount_status,match_tier,quality,provision_index
H.R. 9468,appropriation,Compensation and Pensions,Compensation and Pensions,Department of Veterans Affairs,2285513000,,new_budget_authority,,,,For an additional amount for ''Compensation and Pensions''...,found,exact,strong,0
H.R. 9468,appropriation,Readjustment Benefits,Readjustment Benefits,Department of Veterans Affairs,596969000,,new_budget_authority,,,,For an additional amount for ''Readjustment Benefits''...,found,exact,strong,1
```

### Columns in CSV output

The CSV includes the same fields as the JSON output, flattened into columns:

| Column | Description |
|--------|-------------|
| `bill` | Bill identifier (e.g., "H.R. 4366") |
| `provision_type` | Type: appropriation, rescission, rider, etc. |
| `account_name` | The appropriations account name |
| `description` | Description of the provision |
| `agency` | Parent department or agency |
| `dollars` | Dollar amount as a plain integer (no commas or $) |
| `old_dollars` | For CR substitutions: the old amount being replaced |
| `semantics` | What the amount means: new_budget_authority, rescission, reference_amount, etc. |
| `section` | Section reference (e.g., "SEC. 101") |
| `division` | Division letter for omnibus bills (e.g., "A") |
| `amount_status` | Verification result: found, found_multiple, not_found |
| `quality` | Overall quality: strong, moderate, weak, n/a |
| `raw_text` | Excerpt of the actual bill language |
| `provision_index` | Position in the bill's provision array (zero-indexed) |
| `match_tier` | How raw_text matched the source: exact, normalized, spaceless, no_match |
| `fiscal_year` | Fiscal year the provision is for (appropriations only) |
| `detail_level` | Structural granularity: top_level, line_item, sub_allocation, proviso_amount |
| `confidence` | LLM confidence score (0.00–1.00) |

> ⚠️ **Don't sum the `dollars` column directly.** The export includes sub-allocations
> and reference amounts that would double-count money already in a parent line item.
> Without filtering, a naive sum can overcount budget authority by **2x or more**.
>
> To compute correct budget authority totals:
> - Filter to `semantics == new_budget_authority`
> - Exclude `detail_level == sub_allocation` and `detail_level == proviso_amount`
>
> Or use `congress-approp summary` which does this correctly and automatically.

### Computing totals correctly

**In Excel or Google Sheets:**
1. Open the CSV
2. Add a filter on the `semantics` column → select only `new_budget_authority`
3. Add a filter on the `detail_level` column → deselect `sub_allocation` and `proviso_amount`
4. Sum the filtered `dollars` column

**With jq (command line):**
```bash
congress-approp search --dir examples --type appropriation --format jsonl \
  | jq -s '[.[] | select(.semantics == "new_budget_authority" and .detail_level != "sub_allocation" and .detail_level != "proviso_amount") | .dollars] | add'
```

**With Python:**
```python
import csv
with open("provisions.csv") as f:
    rows = list(csv.DictReader(f))
ba = sum(int(r["dollars"]) for r in rows
         if r["dollars"]
         and r["semantics"] == "new_budget_authority"
         and r["detail_level"] not in ("sub_allocation", "proviso_amount"))
print(f"Budget Authority: ${ba:,}")
```

> **Tip:** When you export to CSV/JSON/JSONL, the tool prints a summary to stderr showing how many provisions have each semantics type and the budget authority total. Watch for this — it tells you immediately whether filtering is needed.

### Opening in Excel

1. Open Excel
2. File → Open → navigate to your `.csv` file
3. If Excel doesn't auto-detect columns, use Data → From Text/CSV and select UTF-8 encoding
4. The `dollars` column will be numeric — you can format it as currency or with comma separators

**Gotchas to watch for:**

- **Large numbers:** Excel may display very large dollar amounts in scientific notation (e.g., `8.46E+11`). Format the column as Number with 0 decimal places.
- **Leading zeros:** Not an issue here since bill numbers don't have leading zeros, but be aware that CSV import can strip them in other contexts.
- **UTF-8 characters:** Bill text contains em-dashes (—), curly quotes, and other Unicode characters. Make sure your import specifies UTF-8 encoding. On Windows, this sometimes requires the "From Text/CSV" import wizard rather than a simple File → Open.
- **Commas in text:** The `raw_text` and `description` fields may contain commas. The CSV output properly quotes these fields, but some older CSV parsers may not handle quoted fields correctly.

### Opening in Google Sheets

1. Go to Google Sheets → File → Import → Upload
2. Select your `.csv` file
3. Import location: "Replace current sheet" or "Insert new sheet"
4. Separator type: Comma (should auto-detect)
5. Google Sheets handles UTF-8 natively — no encoding issues

### Useful CSV exports

```bash
# All appropriations across all example bills
congress-approp search --dir examples --type appropriation --format csv > all_appropriations.csv

# Just the VA accounts
congress-approp search --dir examples --agency "Veterans" --format csv > va_provisions.csv

# Rescissions over $100 million
congress-approp search --dir examples --type rescission --min-dollars 100000000 --format csv > big_rescissions.csv

# CR substitutions with old and new amounts
congress-approp search --dir examples --type cr_substitution --format csv > cr_anomalies.csv

# Everything in Division A (MilCon-VA)
congress-approp search --dir examples/hr4366 --division A --format csv > milcon_va.csv

# Summary table as CSV
congress-approp summary --dir examples --format csv > bill_summary.csv
```

## JSON for Programmatic Use

JSON output includes every field for each matching provision as an array of objects. It's the richest output format and the best choice for Python, JavaScript, R, or any other programming language.

### Basic export

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

### Five jq One-Liners Every Analyst Needs

If you have [`jq`](https://jqlang.github.io/jq/) installed (a lightweight JSON processor), you can do powerful filtering and aggregation directly from the command line:

**1. Total budget authority across all appropriations:**

```bash
congress-approp search --dir examples --type appropriation --format json | \
  jq '[.[] | select(.semantics == "new_budget_authority") | .dollars] | add'
```

```text
862137099554
```

**2. Top 10 accounts by dollar amount:**

```bash
congress-approp search --dir examples --type appropriation --format json | \
  jq '[.[] | select(.dollars != null)] | sort_by(-.dollars) | .[:10] | .[] | "\(.dollars)\t\(.account_name)"'
```

**3. Group by agency and sum budget authority:**

```bash
congress-approp search --dir examples --type appropriation --format json | \
  jq 'group_by(.agency) | map({
    agency: .[0].agency,
    total: [.[] | .dollars // 0] | add,
    count: length
  }) | sort_by(-.total) | .[:10]'
```

**4. Find all provisions in Division A over $1 billion:**

```bash
congress-approp search --dir examples --format json | \
  jq '[.[] | select(.division == "A" and (.dollars // 0) > 1000000000)]'
```

**5. Extract just account names (unique, sorted):**

```bash
congress-approp search --dir examples --type appropriation --format json | \
  jq '[.[].account_name] | unique | sort | .[]'
```

### Loading JSON in Python

```python
import json

# Method 1: From a file
with open("appropriations.json") as f:
    provisions = json.load(f)

# Method 2: From subprocess
import subprocess
result = subprocess.run(
    ["congress-approp", "search", "--dir", "examples",
     "--type", "appropriation", "--format", "json"],
    capture_output=True, text=True
)
provisions = json.loads(result.stdout)

# Work with the data
for p in provisions:
    if p["dollars"] and p["dollars"] > 1_000_000_000:
        print(f"{p['account_name']}: ${p['dollars']:,.0f}")
```

### Loading JSON in pandas

```python
import pandas as pd
import json

# Load search output
df = pd.read_json("appropriations.json")

# Basic analysis
print(f"Total provisions: {len(df)}")
print(f"Total BA: ${df[df['semantics'] == 'new_budget_authority']['dollars'].sum():,.0f}")
print(f"\nBy agency:")
print(df.groupby("agency")["dollars"].sum().sort_values(ascending=False).head(10))
```

### Loading JSON in R

```r
library(jsonlite)

provisions <- fromJSON("appropriations.json")

# Filter to appropriations with budget authority
ba <- provisions[provisions$semantics == "new_budget_authority" & !is.na(provisions$dollars), ]

# Top 10 by dollars
head(ba[order(-ba$dollars), c("account_name", "agency", "dollars")], 10)
```

## JSONL for Streaming

JSONL (JSON Lines) outputs one JSON object per line, with no enclosing array brackets. This is ideal for:

- Streaming processing (each line is independently parseable)
- Piping to `while read` loops in shell scripts
- Processing very large result sets without loading everything into memory
- Tools like `xargs` and `parallel`

### Basic usage

```bash
congress-approp search --dir examples --type appropriation --format jsonl
```

Each line is a complete JSON object:

```text
{"account_name":"Compensation and Pensions","agency":"Department of Veterans Affairs","amount_status":"found","bill":"H.R. 9468","description":"Compensation and Pensions","division":"","dollars":2285513000,...}
{"account_name":"Readjustment Benefits","agency":"Department of Veterans Affairs","amount_status":"found","bill":"H.R. 9468","description":"Readjustment Benefits","division":"","dollars":596969000,...}
...
```

### Shell processing examples

```bash
# Count provisions per bill
congress-approp search --dir examples --format jsonl | \
  jq -r '.bill' | sort | uniq -c | sort -rn

# Extract account names line by line
congress-approp search --dir examples --type appropriation --format jsonl | \
  while IFS= read -r line; do
    echo "$line" | jq -r '.account_name'
  done

# Filter and reformat in one pipeline
congress-approp search --dir examples --type rescission --format jsonl | \
  jq -r 'select(.dollars > 1000000000) | "\(.bill)\t$\(.dollars)\t\(.account_name)"'
```

### When to use JSONL vs. JSON

| Format | Use When |
|--------|----------|
| **JSON** | Loading the full result set into memory (Python, R, JavaScript). Result is a single parseable array. |
| **JSONL** | Streaming line-by-line processing, very large result sets, piping to `jq`/`xargs`/`parallel`. Each line is independent. |

## Working with extraction.json Directly

Sometimes the CLI search output doesn't give you exactly what you need. The raw `extraction.json` file contains the complete data with nested structures that the CLI flattens.

### Structure

```json
{
  "schema_version": "1.0",
  "bill": {
    "identifier": "H.R. 9468",
    "classification": "supplemental",
    "short_title": "Veterans Benefits Continuity and Accountability Supplemental Appropriations Act, 2024",
    "fiscal_years": [2024],
    "divisions": [],
    "public_law": null
  },
  "provisions": [
    {
      "provision_type": "appropriation",
      "account_name": "Compensation and Pensions",
      "agency": "Department of Veterans Affairs",
      "amount": {
        "value": { "kind": "specific", "dollars": 2285513000 },
        "semantics": "new_budget_authority",
        "text_as_written": "$2,285,513,000"
      },
      "detail_level": "top_level",
      "availability": "to remain available until expended",
      "fiscal_year": 2024,
      "confidence": 0.99,
      "raw_text": "For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.",
      "notes": ["Supplemental appropriation under Veterans Benefits Administration heading", "No-year funding"],
      "cross_references": [],
      "section": "",
      "division": null,
      "title": null,
      "provisos": [],
      "earmarks": [],
      "parent_account": null,
      "program": null
    }
  ],
  "summary": { ... },
  "chunk_map": []
}
```

Key differences from CLI JSON output:
- **Nested `amount` object** with `value`, `semantics`, and `text_as_written` sub-fields
- **`notes` array** — explanatory annotations the LLM added
- **`cross_references` array** — references to other laws and sections
- **`provisos` array** — "Provided, That" conditions
- **`earmarks` array** — community project funding items
- **`confidence` float** — LLM self-assessed confidence (0.0–1.0)
- **`availability` string** — fund availability period

### Flattening nested data in Python

```python
import json
import pandas as pd

with open("examples/hr9468/extraction.json") as f:
    data = json.load(f)

# Flatten provisions with nested amounts
rows = []
for p in data["provisions"]:
    row = {
        "provision_type": p["provision_type"],
        "account_name": p.get("account_name", ""),
        "agency": p.get("agency", ""),
        "section": p.get("section", ""),
        "division": p.get("division", ""),
        "confidence": p.get("confidence", 0),
        "raw_text": p.get("raw_text", ""),
        "notes": "; ".join(p.get("notes", [])),
    }

    # Flatten the amount
    amt = p.get("amount")
    if amt:
        val = amt.get("value", {})
        row["dollars"] = val.get("dollars") if val.get("kind") == "specific" else None
        row["semantics"] = amt.get("semantics", "")
        row["text_as_written"] = amt.get("text_as_written", "")

    rows.append(row)

df = pd.DataFrame(rows)
print(df[["provision_type", "account_name", "dollars", "semantics"]].to_string())
```

### Finding provisions with specific notes

The `notes` field contains useful annotations that the CLI doesn't display:

```python
import json

with open("examples/hr4366/extraction.json") as f:
    data = json.load(f)

# Find all provisions noted as advance appropriations
for i, p in enumerate(data["provisions"]):
    notes = p.get("notes", [])
    for note in notes:
        if "advance" in note.lower():
            acct = p.get("account_name", "unknown")
            amt = p.get("amount", {}).get("value", {}).get("dollars", "N/A")
            print(f"[{i}] {acct}: ${amt:,} — {note}")
```

## Summary: Choosing the Right Format

| Format | Flag | Best For | Preserves Nested Data? |
|--------|------|----------|----------------------|
| **Table** | `--format table` (default) | Interactive exploration, quick lookups | No — truncates long fields |
| **CSV** | `--format csv` | Excel, Google Sheets, R, simple tabular analysis | No — flattened columns |
| **JSON** | `--format json` | Python, JavaScript, `jq`, programmatic processing | Partially — CLI flattens some fields |
| **JSONL** | `--format jsonl` | Streaming, piping, line-by-line processing | Partially — same as JSON per line |
| **extraction.json** (direct) | Read the file directly | Full nested data, notes, cross-references, provisos | **Yes** — complete data |

For most analysis tasks, start with `--format json` or `--format csv`. Only read `extraction.json` directly when you need nested fields like `notes`, `cross_references`, or `provisos` that the CLI output flattens away.

## Next Steps

- **[Filter and Search Provisions](../how-to/filter-and-search.md)** — all search flags for narrowing results before export
- **[extraction.json Fields](../reference/extraction-json.md)** — complete field reference for the raw JSON
- **[Output Formats](../reference/output-formats.md)** — format reference with full column lists