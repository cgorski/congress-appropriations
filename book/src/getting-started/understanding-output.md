# Understanding the Output

> **You will need:** `congress-approp` installed, access to the `examples/` directory.
>
> **You will learn:** How to read every table the tool produces — what each column means, what the symbols indicate, and how to interpret the numbers.

Before diving into tutorials and specific tasks, let's build a solid understanding of the output formats you'll encounter. Every command in `congress-approp` uses consistent conventions, but the tables adapt their shape depending on what you're looking at.

## The Summary Table

The `summary` command gives you the bird's-eye view:

```bash
congress-approp summary --dir examples
```

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

### Column-by-column

| Column | What It Shows |
|--------|---------------|
| **Bill** | The bill identifier as printed in the legislation (e.g., "H.R. 4366"). The TOTAL row sums across all loaded bills. |
| **Classification** | The type of appropriations bill: `Omnibus`, `Continuing Resolution`, `Supplemental`, `Regular`, `Minibus`, or `Rescissions`. |
| **Provisions** | The total count of extracted provisions of all types — appropriations, rescissions, riders, directives, and everything else. |
| **Budget Auth ($)** | The sum of all provisions where the amount semantics is `new_budget_authority` and the detail level is `top_level` or `line_item`. Sub-allocations and proviso amounts are excluded to prevent double-counting. This number is **computed from individual provisions**, never from an LLM-generated summary. |
| **Rescissions ($)** | The absolute value sum of all provisions of type `rescission` with `rescission` semantics. This is money Congress is canceling from prior appropriations. |
| **Net BA ($)** | Budget Authority minus Rescissions. This is the net new spending authority enacted by the bill. For most reporting purposes, **Net BA is the number you want.** |

### The footer

The line below the table — "0 dollar amounts unverified across all bills" — is a quick trust check. It counts provisions across all loaded bills where the dollar amount string was not found in the source bill text. Zero means every extracted number was confirmed against the source. If this number is ever greater than zero, the `audit` command will show you exactly which provisions need review.

### By-agency view

Add `--by-agency` to see budget authority broken down by parent department:

```bash
congress-approp summary --dir examples --by-agency
```

This appends a second table showing every agency, its total budget authority, rescissions, and provision count, sorted by budget authority descending. For example, Department of Veterans Affairs shows ~$343B (which includes mandatory programs like Compensation and Pensions that appear as appropriation lines in the bill text).

## The Search Table

The `search` command produces tables that **adapt their columns based on what you're searching for**. This is one of the most important things to understand about the output.

### Standard search table

For most searches, you see this layout:

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

| Column | What It Shows |
|--------|---------------|
| **$** | Verification status of the dollar amount (see symbols table below) |
| **Bill** | Which bill this provision comes from |
| **Type** | The provision type: `appropriation`, `rescission`, `rider`, `directive`, `limitation`, `transfer_authority`, `cr_substitution`, `mandatory_spending_extension`, `directed_spending`, `continuing_resolution_baseline`, or `other` |
| **Description / Account** | The account name for appropriations and rescissions, or a description for other provision types. Long text is truncated with `…` |
| **Amount ($)** | The dollar amount. Shows `—` for provisions without a dollar value (riders, directives). |
| **Section** | The section reference from the bill text (e.g., "SEC. 101"). Empty if the provision appears under a heading without a section number. |
| **Div** | The division letter for omnibus bills (e.g., "A" for MilCon-VA in H.R. 4366). Empty for bills without divisions. |

### The $ column — verification symbols

The leftmost column tells you the verification status of each provision's dollar amount:

| Symbol | Meaning | Should You Worry? |
|--------|---------|-------------------|
| **✓** | The exact dollar string (e.g., `$2,285,513,000`) was found at **one unique position** in the source bill text. | No — this is the best result. |
| **≈** | The dollar string was found at **multiple positions** in the source text. The amount is correct, but it can't be pinned to a single location. | No — very common for round numbers like `$5,000,000` which may appear 50 times in an omnibus. |
| **✗** | The dollar string was **not found** in the source text. | **Yes** — this provision needs manual review. Across the included example data, this never occurs (0 of 8,554). |
| *(blank)* | The provision doesn't carry a dollar amount (riders, directives, some policy provisions). | No — nothing to verify. |

### CR substitution table

When you search for `cr_substitution` type provisions, the table automatically changes shape to show the old and new amounts:

```bash
congress-approp search --dir examples/hr5860 --type cr_substitution
```

```text
┌───┬───────────┬──────────────────────────────────────────┬───────────────┬───────────────┬──────────────┬──────────┬─────┐
│ $ ┆ Bill      ┆ Account                                  ┆       New ($) ┆       Old ($) ┆    Delta ($) ┆ Section  ┆ Div │
╞═══╪═══════════╪══════════════════════════════════════════╪═══════════════╪═══════════════╪══════════════╪══════════╪═════╡
│ ✓ ┆ H.R. 5860 ┆ Rural Housing Service—Rural Community…   ┆    25,300,000 ┆    75,300,000 ┆  -50,000,000 ┆ SEC. 101 ┆ A   │
│ ...                                                                                                                      │
│ ✓ ┆ H.R. 5860 ┆ Office of Personnel Management—Salari…   ┆   219,076,000 ┆   190,784,000 ┆  +28,292,000 ┆ SEC. 126 ┆ A   │
└───┴───────────┴──────────────────────────────────────────┴───────────────┴───────────────┴──────────────┴──────────┴─────┘
13 provisions found
```

Instead of a single **Amount** column, you get:

| Column | Meaning |
|--------|---------|
| **New ($)** | The new dollar amount the CR substitutes in |
| **Old ($)** | The old dollar amount being replaced |
| **Delta ($)** | New minus Old. Negative means a cut, positive means an increase |

### Semantic search table

When you use `--semantic` or `--similar`, a **Sim** (similarity) column appears at the left:

```text
┌──────┬───────────┬───────────────┬───────────────────────────────────────┬────────────────┬─────┐
│ Sim  ┆ Bill      ┆ Type          ┆ Description / Account                 ┆     Amount ($) ┆ Div │
╞══════╪═══════════╪═══════════════╪═══════════════════════════════════════╪════════════════╪═════╡
│ 0.51 ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs              ┆ 33,266,226,000 ┆ B   │
│ 0.46 ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs              ┆     10,000,000 ┆ B   │
└──────┴───────────┴───────────────┴───────────────────────────────────────┴────────────────┴─────┘
```

The **Sim** score is the cosine similarity between your query and the provision's embedding vector, ranging from 0 to 1:

| Score Range | Interpretation |
|-------------|---------------|
| **> 0.80** | Almost certainly the same program (when comparing across bills) |
| **0.60 – 0.80** | Related topic, same policy area |
| **0.45 – 0.60** | Loosely related |
| **< 0.45** | Probably not meaningfully related |

Results are sorted by similarity descending and limited to `--top N` (default 20).

## The Audit Table

The `audit` command provides the most detailed quality view:

```bash
congress-approp audit --dir examples
```

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

The audit table has two groups of columns: **amount verification** (left side) and **text verification** (right side).

### Amount verification columns

These check whether the dollar amount string (e.g., `"$2,285,513,000"`) exists in the source bill text:

| Column | What It Counts | Ideal Value |
|--------|---------------|-------------|
| **Verified** | Provisions whose dollar string was found at exactly one position in the source | Higher is better |
| **NotFound** | Provisions whose dollar string was **not found anywhere** in the source text | **Must be 0** — any value above 0 means you should investigate |
| **Ambig** | Provisions whose dollar string was found at multiple positions (ambiguous location but correct amount) | Not a problem — common for round numbers |

The sum of Verified + Ambig equals the total number of provisions that have dollar amounts. NotFound should always be zero. Across the included example data, it is.

### Text verification columns

These check whether the `raw_text` excerpt (the first ~150 characters of the bill language for each provision) is a substring of the source text:

| Column | Match Method | What It Means |
|--------|-------------|---------------|
| **Exact** | Byte-identical substring match | The raw text was copied verbatim from the source — best case. 95.5% of provisions across the 13-bill dataset. |
| **NormText** | Matches after normalizing whitespace, curly quotes (`"` → `"`), and em-dashes (`—` → `-`) | Minor formatting differences from XML-to-text conversion. Content is correct. |
| **Spaceless** | Matches only after removing all spaces | Catches word-joining artifacts. Zero occurrences in the example data. |
| **TextMiss** | Not found at any matching tier | The raw text may be paraphrased or truncated. In the example data, all 38 TextMiss cases are non-dollar provisions (statutory amendments) where the LLM slightly reformatted section references. |

### Coverage column

**Coverage** is the percentage of all dollar-sign patterns found in the source bill text that were matched to an extracted provision. This measures **completeness**, not accuracy.

- **100%** (H.R. 9468): Every dollar amount in the source was captured — perfect.
- **94.2%** (H.R. 4366): Most dollar amounts were captured. The remaining 5.8% are typically statutory cross-references, loan guarantee ceilings, or old amounts being struck by amendments — dollar figures that appear in the text but aren't independent provisions.
- **61.1%** (H.R. 5860): Lower coverage is expected for continuing resolutions because most of the bill text consists of references to prior-year appropriations acts, which contain many dollar amounts that are contextual references, not new provisions.

**Coverage below 100% does not mean the extracted numbers are wrong.** It means the bill text contains dollar strings that aren't captured as provisions. See [What Coverage Means (and Doesn't)](../explanation/coverage.md) for a detailed explanation.

### Quick decision guide

After running `audit`, here's how to interpret the results:

| Situation | Interpretation | Action |
|-----------|---------------|--------|
| NotFound = 0, Coverage ≥ 90% | Excellent — all extracted amounts verified, high completeness | Use with confidence |
| NotFound = 0, Coverage 60–90% | Good — all extracted amounts verified, some dollar strings in source uncaptured | Fine for most purposes; check unaccounted amounts if completeness matters |
| NotFound = 0, Coverage < 60% | Amounts are correct but extraction may be incomplete | Consider re-extracting; review with `audit --verbose` |
| NotFound > 0 | **Some amounts need review** | Run `audit --verbose` to see which provisions failed; verify manually against the source XML |

## The Compare Table

The `compare` command shows account-level differences between two sets of bills:

```bash
congress-approp compare --base examples/hr4366 --current examples/hr9468
```

```text
┌─────────────────────────────────────┬──────────────────────┬─────────────────┬───────────────┬──────────────────┬─────────┬──────────────┐
│ Account                             ┆ Agency               ┆        Base ($) ┆   Current ($) ┆        Delta ($) ┆     Δ % ┆ Status       │
╞═════════════════════════════════════╪══════════════════════╪═════════════════╪═══════════════╪══════════════════╪═════════╪══════════════╡
│ Compensation and Pensions           ┆ Department of Veter… ┆ 197,382,903,000 ┆ 2,285,513,000 ┆ -195,097,390,000 ┆  -98.8% ┆ changed      │
│ Readjustment Benefits               ┆ Department of Veter… ┆  13,774,657,000 ┆   596,969,000 ┆  -13,177,688,000 ┆  -95.7% ┆ changed      │
│ ...                                                                                                                                       │
│ Supplemental Nutrition Assistance … ┆ Department of Agric… ┆ 122,382,521,000 ┆             0 ┆ -122,382,521,000 ┆ -100.0% ┆ only in base │
└─────────────────────────────────────┴──────────────────────┴─────────────────┴───────────────┴──────────────────┴─────────┴──────────────┘
```

| Column | Meaning |
|--------|---------|
| **Account** | The account name, matched between bills |
| **Agency** | The parent agency or department |
| **Base ($)** | Total budget authority for this account in the `--base` bills |
| **Current ($)** | Total budget authority in the `--current` bills |
| **Delta ($)** | Current minus Base |
| **Δ %** | Percentage change |
| **Status** | `changed` (in both, different amounts), `unchanged` (in both, same amount), `only in base` (not in current), or `only in current` (not in base) |

Results are sorted by the absolute value of Delta, largest changes first.

> **Interpreting cross-type comparisons:** When comparing an omnibus to a supplemental (as above), most accounts will show "only in base" because the supplemental only touches a few accounts. The tool warns you about this: "Comparing Omnibus to Supplemental. Accounts in one but not the other may be expected." The compare command is most informative when comparing bills of the same type — for example, an FY2023 omnibus to an FY2024 omnibus.

## Output Formats

Every query command supports four output formats via `--format`:

### Table (default)

```bash
congress-approp search --dir examples/hr9468 --format table
```

Human-readable formatted table. Best for interactive use and quick exploration. Column widths adapt to content. Long text is truncated.

### JSON

```bash
congress-approp search --dir examples/hr9468 --format json
```

A JSON array of objects. **Includes every field** for each matching provision — more data than the table shows. Best for programmatic consumption, piping to `jq`, or loading into scripts.

### JSONL (JSON Lines)

```bash
congress-approp search --dir examples/hr9468 --format jsonl
```

One JSON object per line, no enclosing array. Best for streaming processing, piping to `while read`, or working with very large result sets. Each line is independently parseable.

### CSV

```bash
congress-approp search --dir examples/hr9468 --format csv > provisions.csv
```

Comma-separated values suitable for importing into Excel, Google Sheets, R, or pandas. Includes a header row. Dollar amounts are plain integers (not formatted with commas).

> **Tip:** When exporting to CSV for Excel, make sure to import the file with UTF-8 encoding. Some bill text contains em-dashes (—) and other Unicode characters that may display incorrectly with the default Windows encoding.

For a detailed guide with examples and recipes for each format, see [Output Formats](../reference/output-formats.md).

## Provision Types at a Glance

You'll encounter these provision types throughout the tool. Use `--list-types` for a quick reference:

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

The distribution varies by bill type. In the FY2024 omnibus (H.R. 4366), the breakdown is:

| Type | Count | What These Are |
|------|-------|----------------|
| `appropriation` | 1,216 | Grant of budget authority — the core spending provisions |
| `limitation` | 456 | Caps and prohibitions ("not more than", "none of the funds") |
| `rider` | 285 | Policy provisions that don't directly spend or limit money |
| `directive` | 120 | Reporting requirements and instructions to agencies |
| `other` | 84 | Provisions that don't fit neatly into the standard types |
| `rescission` | 78 | Cancellations of previously appropriated funds |
| `transfer_authority` | 77 | Permission to move funds between accounts |
| `mandatory_spending_extension` | 40 | Amendments to authorizing statutes |
| `directed_spending` | 8 | Earmarks and community project funding |

The continuing resolution (H.R. 5860) has a very different profile: 49 riders, 44 mandatory spending extensions, 13 CR substitutions, and only 5 standalone appropriations. This reflects the CR's structure — it mostly continues prior-year funding rather than setting new levels.

For detailed documentation of each provision type including all fields and real examples, see [Provision Types](../reference/provision-types.md).

## Enriched Output

When you run `congress-approp enrich --dir examples` (no API key needed), the tool generates bill metadata that enhances the output:

- **Enriched classifications** — the summary table shows "Full-Year CR with Appropriations" instead of "Continuing Resolution" for hybrid bills like H.R. 1968, and "Minibus" instead of "Omnibus" for bills covering only 2–4 subcommittees.
- **Advance appropriation split** — use `--show-advance` on `summary` to separate current-year spending from advance appropriations (money enacted now but available in a future fiscal year). This is critical for VA accounts where 79.5% of MilCon-VA budget authority is advance.
- **Fiscal year and subcommittee filtering** — use `--fy 2026` and `--subcommittee thud` to scope any command to a specific year and jurisdiction, automatically resolving division letters across bills.

See [Enrich Bills with Metadata](../how-to/enrich-data.md) for the full guide.

## Next Steps

You now know how to read every type of output the tool produces. Time to put it to use:

- **[Enrich Bills with Metadata](../how-to/enrich-data.md)** — enable FY filtering, subcommittee scoping, and advance splits
- **[Find How Much Congress Spent on a Topic](../tutorials/find-spending-on-topic.md)** — your first real research task
- **[Compare Two Bills](../tutorials/compare-two-bills.md)** — see what changed between bills
- **[Track a Program Across Bills](../tutorials/track-program-across-bills.md)** — trace one account across fiscal years
- **[Filter and Search Provisions](../how-to/filter-and-search.md)** — all the search flags in one place