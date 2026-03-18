# Filter and Search Provisions

> **You will need:** `congress-approp` installed, access to the `examples/` directory. For semantic search: `OPENAI_API_KEY`.
>
> **You will learn:** Every filter flag available on the `search` command, how to combine them, and practical recipes for common queries.

The `search` command is the most versatile tool in `congress-approp`. It supports ten filter flags that can be combined freely — all filters use AND logic, meaning every provision in the results must match every filter you specify. This guide covers each flag with real examples from the included data.

## Quick Reference: All Search Flags

| Flag | Short | Type | Description |
|------|-------|------|-------------|
| `--dir` | | path | Directory containing extracted bills (required) |
| `--type` | `-t` | string | Filter by provision type |
| `--agency` | `-a` | string | Filter by agency name (case-insensitive substring) |
| `--account` | | string | Filter by account name (case-insensitive substring) |
| `--keyword` | `-k` | string | Search in raw_text (case-insensitive substring) |
| `--bill` | | string | Filter to a specific bill identifier |
| `--division` | | string | Filter by division letter |
| `--min-dollars` | | integer | Minimum dollar amount (absolute value) |
| `--max-dollars` | | integer | Maximum dollar amount (absolute value) |
| `--format` | | string | Output format: table, json, jsonl, csv |
| `--semantic` | | string | Rank by meaning similarity (requires embeddings + OPENAI_API_KEY) |
| `--similar` | | string | Find provisions similar to a specific one (format: `dir:index`) |
| `--top` | | integer | Maximum results for semantic/similar search (default 20) |
| `--list-types` | | flag | List all valid provision types and exit |

## Filter by Provision Type (`--type`)

The most common filter. Restricts results to a single provision type.

```bash
# All appropriations across all bills
congress-approp search --dir examples --type appropriation

# All rescissions
congress-approp search --dir examples --type rescission

# CR substitutions (anomalies) — table auto-adapts to show New/Old/Delta columns
congress-approp search --dir examples --type cr_substitution

# Reporting requirements and instructions to agencies
congress-approp search --dir examples --type directive

# Policy provisions (no direct spending)
congress-approp search --dir examples --type rider
```

### Available provision types

Use `--list-types` to see all valid values:

```bash
congress-approp search --dir examples --list-types
```

```text
Available provision types:
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

### Type distribution by bill

Not every bill contains every type. Here's the distribution across the example data:

| Type | H.R. 4366 (Omnibus) | H.R. 5860 (CR) | H.R. 9468 (Supp) |
|------|---------------------|-----------------|-------------------|
| `appropriation` | 1,216 | 5 | 2 |
| `limitation` | 456 | 4 | — |
| `rider` | 285 | 49 | 2 |
| `directive` | 120 | 2 | 3 |
| `other` | 84 | 12 | — |
| `rescission` | 78 | — | — |
| `transfer_authority` | 77 | — | — |
| `mandatory_spending_extension` | 40 | 44 | — |
| `directed_spending` | 8 | — | — |
| `cr_substitution` | — | 13 | — |
| `continuing_resolution_baseline` | — | 1 | — |

## Filter by Agency (`--agency`)

Matches the `agency` field using a case-insensitive substring search:

```bash
# All provisions from the Department of Veterans Affairs
congress-approp search --dir examples --agency "Veterans"

# All provisions from the Department of Energy
congress-approp search --dir examples --agency "Energy"

# All NASA provisions
congress-approp search --dir examples --agency "Aeronautics"

# All DOJ provisions
congress-approp search --dir examples --agency "Justice"
```

The `--agency` flag matches against the structured `agency` field that the LLM extracted — typically the full department name (e.g., "Department of Veterans Affairs"). You only need to provide a substring; the match is case-insensitive.

**Tip:** Some provisions don't have an agency field (riders, directives, and some other types). These will never appear in agency-filtered results.

### Combine with type for focused results

```bash
# Only VA appropriations
congress-approp search --dir examples --agency "Veterans" --type appropriation

# Only VA rescissions
congress-approp search --dir examples --agency "Veterans" --type rescission

# DOJ directives
congress-approp search --dir examples --agency "Justice" --type directive
```

## Filter by Account Name (`--account`)

Matches the `account_name` field using a case-insensitive substring search. This is more specific than `--agency` — it targets the individual appropriations account:

```bash
# All provisions for Child Nutrition Programs
congress-approp search --dir examples --account "Child Nutrition"

# All provisions for the FBI
congress-approp search --dir examples --account "Federal Bureau of Investigation"

# All provisions for Disaster Relief
congress-approp search --dir examples --account "Disaster Relief"

# All provisions for Medical Services (VA)
congress-approp search --dir examples --account "Medical Services"
```

The account name is extracted from the bill text — it's usually the text between `''` delimiters in the legislative language (e.g., `''Compensation and Pensions''`).

### Account vs. Agency

| Flag | Matches Against | Granularity | Example |
|------|----------------|-------------|---------|
| `--agency` | Parent department or agency | Broad | "Department of Veterans Affairs" |
| `--account` | Specific appropriations account | Narrow | "Compensation and Pensions" |

Many provisions under the same agency have different account names. Use `--agency` for a department-wide view and `--account` when you know the specific program.

### Gotcha: "Salaries and Expenses"

The account name "Salaries and Expenses" appears under dozens of different agencies. If you search `--account "Salaries and Expenses"` without an agency filter, you'll get results from across the entire government. Combine with `--agency` to narrow:

```bash
congress-approp search --dir examples --account "Salaries and Expenses" --agency "Justice"
```

## Filter by Keyword in Bill Text (`--keyword`)

Searches the `raw_text` field — the actual bill language excerpt stored with each provision. This is a case-insensitive substring match:

```bash
# Find provisions mentioning FEMA
congress-approp search --dir examples --keyword "Federal Emergency Management"

# Find provisions with "notwithstanding" (often signals important policy exceptions)
congress-approp search --dir examples --keyword "notwithstanding"

# Find provisions about transfer authority
congress-approp search --dir examples --keyword "may transfer"

# Find provisions about reporting requirements
congress-approp search --dir examples --keyword "shall submit a report"

# Find provisions referencing a specific public law
congress-approp search --dir examples --keyword "Public Law 118"
```

### Keyword vs. Account vs. Semantic

| Search Method | Searches | Best For | Misses |
|---------------|----------|----------|--------|
| `--keyword` | The raw_text excerpt (~150 chars of bill language) | Exact terms you know appear in the text | Provisions where the term is in the account name but not the raw_text excerpt, or where synonyms are used |
| `--account` | The structured account_name field | Known program names | Provisions that reference the program without naming the account |
| `--semantic` | The full provision meaning (via embeddings) | Concepts and topics, layperson language | Nothing — it searches everything, but scores may be low for weak matches |

For the most thorough search, try all three approaches. Start with `--keyword` or `--account` for precision, then use `--semantic` to find provisions you might have missed.

## Filter by Bill (`--bill`)

Restricts results to a specific bill by its identifier string:

```bash
# Only provisions from H.R. 4366
congress-approp search --dir examples --bill "H.R. 4366"

# Only provisions from H.R. 9468
congress-approp search --dir examples --bill "H.R. 9468"
```

The value must match the bill identifier as it appears in the data (e.g., "H.R. 4366", including the space and period). This is a case-sensitive exact match.

**Alternative: Point `--dir` at a specific bill directory.** Instead of `--bill`, you can scope the search by directory:

```bash
# These are equivalent for single-bill searches:
congress-approp search --dir examples --bill "H.R. 4366"
congress-approp search --dir examples/hr4366
```

The `--dir` approach is simpler for single-bill searches. The `--bill` flag is useful when you have multiple bills loaded via a parent directory and want to filter to one.

## Filter by Division (`--division`)

Omnibus bills are organized into lettered divisions (Division A, Division B, etc.), each covering a different set of agencies. The `--division` flag scopes results to a single division:

```bash
# Division A = MilCon-VA in H.R. 4366
congress-approp search --dir examples/hr4366 --division A

# Division B = Agriculture in H.R. 4366
congress-approp search --dir examples/hr4366 --division B

# Division C = Commerce, Justice, Science in H.R. 4366
congress-approp search --dir examples/hr4366 --division C

# Division D = Energy and Water in H.R. 4366
congress-approp search --dir examples/hr4366 --division D
```

The division letter is a single character (A, B, C, etc.). Bills without divisions (like the VA supplemental H.R. 9468) have no division field, so `--division` effectively returns no results for those bills.

### Combine with type for division-level analysis

```bash
# All appropriations in MilCon-VA (Division A) over $1 billion
congress-approp search --dir examples/hr4366 --division A --type appropriation --min-dollars 1000000000

# All rescissions in Commerce-Justice-Science (Division C)
congress-approp search --dir examples/hr4366 --division C --type rescission

# All riders in Agriculture (Division B)
congress-approp search --dir examples/hr4366 --division B --type rider
```

## Filter by Dollar Range (`--min-dollars`, `--max-dollars`)

Filters provisions by the absolute value of their dollar amount:

```bash
# Provisions of $1 billion or more
congress-approp search --dir examples --min-dollars 1000000000

# Provisions between $100 million and $500 million
congress-approp search --dir examples --min-dollars 100000000 --max-dollars 500000000

# Small provisions under $1 million
congress-approp search --dir examples --max-dollars 1000000

# Large rescissions
congress-approp search --dir examples --type rescission --min-dollars 1000000000
```

The filter uses the **absolute value** of the dollar amount, so rescissions (which may be stored as negative values internally) are compared by their magnitude.

Provisions without dollar amounts (riders, directives, etc.) are excluded from results when `--min-dollars` or `--max-dollars` is specified.

## Combining Multiple Filters

All filters use **AND logic** — every filter must match for a provision to appear. This lets you build very specific queries:

```bash
# VA appropriations over $1 billion in Division A
congress-approp search --dir examples \
  --agency "Veterans" \
  --type appropriation \
  --division A \
  --min-dollars 1000000000

# DOJ rescissions in Division C
congress-approp search --dir examples \
  --agency "Justice" \
  --type rescission \
  --division C

# Provisions mentioning "notwithstanding" in the omnibus under $10 million
congress-approp search --dir examples/hr4366 \
  --keyword "notwithstanding" \
  --max-dollars 10000000

# Energy-related appropriations in Division D between $100M and $1B
congress-approp search --dir examples/hr4366 \
  --division D \
  --type appropriation \
  --min-dollars 100000000 \
  --max-dollars 1000000000
```

### Filter order doesn't matter

The tool applies filters in the order that's most efficient internally. The command-line order of flags has no effect on results — these two commands produce identical output:

```bash
congress-approp search --dir examples --type appropriation --agency "Veterans"
congress-approp search --dir examples --agency "Veterans" --type appropriation
```

## Semantic Search (`--semantic`)

Semantic search ranks provisions by meaning similarity instead of keyword matching. It requires pre-computed embeddings and an `OPENAI_API_KEY`:

```bash
export OPENAI_API_KEY="your-key"

# Find provisions about school lunch programs (no keyword overlap with "Child Nutrition Programs")
congress-approp search --dir examples --semantic "school lunch programs for kids" --top 5

# Find provisions about road and bridge infrastructure
congress-approp search --dir examples --semantic "money for fixing roads and bridges" --top 5
```

### Combining semantic search with hard filters

Hard filters apply first (constraining which provisions are eligible), then semantic ranking orders the remaining results:

```bash
# Appropriations about clean energy, at least $100M
congress-approp search --dir examples \
  --semantic "clean energy research" \
  --type appropriation \
  --min-dollars 100000000 \
  --top 10
```

For a full tutorial on semantic search, see [Use Semantic Search](../tutorials/semantic-search.md).

## Find Similar Provisions (`--similar`)

Find provisions most similar to a specific one across all loaded bills. The syntax is `--similar <bill_directory>:<provision_index>`:

```bash
# Find provisions similar to VA Supplemental provision 0 (Comp & Pensions)
congress-approp search --dir examples --similar hr9468:0 --top 5

# Find provisions similar to omnibus provision 620 (FBI Salaries and Expenses)
congress-approp search --dir examples --similar hr4366:620 --top 5
```

Unlike `--semantic`, the `--similar` flag does **not** make any API calls — it uses pre-computed vectors directly. This makes it instant and free.

You can also combine `--similar` with hard filters:

```bash
# Find appropriations similar to a specific provision
congress-approp search --dir examples --similar hr9468:0 --type appropriation --top 5
```

For a full tutorial, see [Track a Program Across Bills](../tutorials/track-program-across-bills.md).

## Controlling the Number of Results (`--top`)

The `--top` flag limits results for semantic and similar searches (default 20). It has no effect on non-semantic searches (which return all matching provisions):

```bash
# Top 3 results
congress-approp search --dir examples --semantic "veterans health care" --top 3

# Top 50 results
congress-approp search --dir examples --semantic "veterans health care" --top 50
```

## Output Formats (`--format`)

All search results can be output in four formats:

```bash
# Human-readable table (default)
congress-approp search --dir examples --type appropriation --format table

# JSON array (full fields, for programmatic use)
congress-approp search --dir examples --type appropriation --format json

# JSON Lines (one object per line, for streaming)
congress-approp search --dir examples --type appropriation --format jsonl

# CSV (for spreadsheets)
congress-approp search --dir examples --type appropriation --format csv > provisions.csv
```

JSON and CSV include **more fields** than the table view — notably `raw_text`, `semantics`, `detail_level`, `amount_status`, `match_tier`, `quality`, and `provision_index`.

For detailed format documentation and recipes, see [Export Data for Spreadsheets and Scripts](../tutorials/export-data.md) and [Output Formats](../reference/output-formats.md).

## Practical Recipes

Here are battle-tested queries for common analysis tasks:

### Find the biggest appropriations in a bill

```bash
congress-approp search --dir examples/hr4366 --type appropriation --min-dollars 10000000000 --format table
```

### Find all provisions for a specific agency

```bash
congress-approp search --dir examples --agency "Department of Energy" --format table
```

### Export all rescissions to a spreadsheet

```bash
congress-approp search --dir examples --type rescission --format csv > rescissions.csv
```

### Find reporting requirements for the VA

```bash
congress-approp search --dir examples --keyword "Veterans Affairs" --type directive
```

### Find all provisions that override other law

```bash
congress-approp search --dir examples --keyword "notwithstanding"
```

### Find which mandatory programs were extended in the CR

```bash
congress-approp search --dir examples/hr5860 --type mandatory_spending_extension --format json
```

### Find provisions in a specific dollar range

```bash
# "Small" appropriations: $1M to $10M
congress-approp search --dir examples --type appropriation --min-dollars 1000000 --max-dollars 10000000

# "Large" appropriations: over $10B
congress-approp search --dir examples --type appropriation --min-dollars 10000000000
```

### Count provisions by type across all bills

```bash
congress-approp search --dir examples --format json | \
  jq 'group_by(.provision_type) | map({type: .[0].provision_type, count: length}) | sort_by(-.count)'
```

### Export everything and filter later

If you're not sure what you need yet, export all provisions and filter in your analysis tool:

```bash
# All provisions, all fields, all bills
congress-approp search --dir examples --format json > all_provisions.json

# Or as CSV for Excel
congress-approp search --dir examples --format csv > all_provisions.csv
```

## Tips

1. **Start broad, then narrow.** Begin with `--type` or `--agency` alone, see how many results you get, then add more filters to focus.

2. **Use `--format json` to see all fields.** The table view truncates long text and hides some fields. JSON shows everything.

3. **Use `--dir` scoping for single-bill searches.** Instead of `--bill "H.R. 4366"`, use `--dir examples/hr4366` — it's simpler and slightly faster.

4. **Combine keyword and account searches.** An account name search finds provisions *named* after a program. A keyword search finds provisions that *mention* a program in their text. Use both for completeness.

5. **Try semantic search as a second pass.** After keyword/account search gives you the obvious results, run a semantic search on the same topic to find provisions you might have missed because the bill uses different terminology.

6. **Check `--list-types` when unsure.** If you can't remember the exact type name, `--list-types` shows all valid values with descriptions.

## Next Steps

- **[Find How Much Congress Spent on a Topic](../tutorials/find-spending-on-topic.md)** — tutorial combining multiple search techniques
- **[Use Semantic Search](../tutorials/semantic-search.md)** — deep dive into meaning-based search
- **[Output Formats](../reference/output-formats.md)** — detailed format reference
- **[CLI Command Reference](../reference/cli.md)** — complete reference for all commands