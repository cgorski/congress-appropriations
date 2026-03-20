# Your First Query

> **You will need:** `congress-approp` installed ([Installation](./installation.md)), access to the `data/` directory from the cloned repository.
>
> **You will learn:** How to explore the included FY2024 appropriations data using five core commands — no API keys required.

This chapter is a guided tour. Every command runs against the included example data and produces real results you can verify yourself. By the end, you'll know how to see budget totals, search for provisions, compare bills, and check data quality.

## Step 1: See What Bills You Have

Start with the `summary` command to get an overview:

```bash
congress-approp summary --dir data
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

Here's what each column means:

| Column | Meaning |
|--------|---------|
| **Bill** | The bill identifier (e.g., H.R. 4366) |
| **Classification** | What kind of appropriations bill: Omnibus, Continuing Resolution, or Supplemental |
| **Provisions** | Total number of provisions extracted from the bill |
| **Budget Auth ($)** | Sum of all provisions with `new_budget_authority` semantics — what Congress authorized agencies to spend. Computed from the actual provisions, not from any LLM-generated summary |
| **Rescissions ($)** | Sum of all rescission provisions — money Congress is taking back from prior appropriations |
| **Net BA ($)** | Budget Authority minus Rescissions — the net new spending authority |

The footer line — "0 dollar amounts unverified" — tells you that every extracted dollar amount was confirmed to exist in the source bill text. This is the headline trust metric.

## Step 2: Search for Provisions

The `search` command finds provisions matching your criteria. Let's start broad — all appropriation-type provisions across all bills:

```bash
congress-approp search --dir data --type appropriation
```

This returns a table with hundreds of rows. Let's narrow it down. Find all provisions mentioning FEMA:

```bash
congress-approp search --dir data --keyword "Federal Emergency Management"
```

```text
┌───┬───────────┬───────────────┬───────────────────────────────────────────────┬────────────────┬──────────┬─────┐
│ $ ┆ Bill      ┆ Type          ┆ Description / Account                         ┆     Amount ($) ┆ Section  ┆ Div │
╞═══╪═══════════╪═══════════════╪═══════════════════════════════════════════════╪════════════════╪══════════╪═════╡
│   ┆ H.R. 5860 ┆ other         ┆ Allows FEMA Disaster Relief Fund to be appor… ┆              — ┆ SEC. 128 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ appropriation ┆ Federal Emergency Management Agency—Disast…   ┆ 16,000,000,000 ┆ SEC. 129 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ appropriation ┆ Office of the Inspector General—Operations…   ┆      2,000,000 ┆ SEC. 129 ┆ A   │
└───┴───────────┴───────────────┴───────────────────────────────────────────────┴────────────────┴──────────┴─────┘
3 provisions found

$ = Amount status: ✓ found (unique), ≈ found (multiple matches), ✗ not found
```

Understanding the **$** column — the verification status for each provision's dollar amount:

| Symbol | Meaning |
|--------|---------|
| **✓** | Dollar amount string found at exactly one position in the source text — highest confidence |
| **≈** | Dollar amount found at multiple positions (common for round numbers like $5,000,000) — amount is correct but can't be pinned to a unique location |
| **✗** | Dollar amount not found in the source text — needs manual review |
| (blank) | Provision doesn't carry a dollar amount (riders, directives) |

Now try searching by account name. This matches against the structured `account_name` field rather than searching the full text:

```bash
congress-approp search --dir data --account "Child Nutrition"
```

```text
┌───┬───────────┬───────────────┬─────────────────────────────────────────────┬────────────────┬─────────┬─────┐
│ $ ┆ Bill      ┆ Type          ┆ Description / Account                       ┆     Amount ($) ┆ Section ┆ Div │
╞═══╪═══════════╪═══════════════╪═════════════════════════════════════════════╪════════════════╪═════════╪═════╡
│ ✓ ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆ 33,266,226,000 ┆         ┆ B   │
│ ✓ ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆     18,004,000 ┆         ┆ B   │
│ ...                                                                                                          │
└───┴───────────┴───────────────┴─────────────────────────────────────────────┴────────────────┴─────────┴─────┘
```

The top result — $33.27 billion for Child Nutrition Programs — is the top-level appropriation. The smaller amounts below it are sub-allocations and reference amounts within the same account.

You can combine filters. For example, find all appropriations over $1 billion in Division A (MilCon-VA):

```bash
congress-approp search --dir data/hr4366 --type appropriation --division A --min-dollars 1000000000
```

## Step 3: Look at the VA Supplemental

The smallest bill, H.R. 9468, is a good place to see the full picture. It has only 7 provisions:

```bash
congress-approp search --dir data/hr9468
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

This is the complete bill: two appropriations ($2.3B for Comp & Pensions, $597M for Readjustment Benefits), two policy riders (SEC. 101 and 102 establishing that these amounts are additional to regular appropriations), and three directives requiring the VA Secretary and Inspector General to submit reports about the funding shortfall that necessitated this supplemental.

Notice how the two appropriations have ✓ in the dollar column, while the riders and directives show no symbol — they don't carry dollar amounts, so there's nothing to verify.

## Step 4: See What the CR Changed

Continuing resolutions normally fund agencies at prior-year rates, but specific programs can get different treatment through "anomalies" — formally called CR substitutions. These are provisions that say "substitute $X for $Y," setting a new level instead of continuing the old one.

```bash
congress-approp search --dir data/hr5860 --type cr_substitution
```

```text
┌───┬───────────┬──────────────────────────────────────────┬───────────────┬───────────────┬──────────────┬──────────┬─────┐
│ $ ┆ Bill      ┆ Account                                  ┆       New ($) ┆       Old ($) ┆    Delta ($) ┆ Section  ┆ Div │
╞═══╪═══════════╪══════════════════════════════════════════╪═══════════════╪═══════════════╪══════════════╪══════════╪═════╡
│ ✓ ┆ H.R. 5860 ┆ Rural Housing Service—Rural Community…   ┆    25,300,000 ┆    75,300,000 ┆  -50,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Rural Utilities Service—Rural Water a…   ┆    60,000,000 ┆   325,000,000 ┆ -265,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆                                          ┆   122,572,000 ┆   705,768,000 ┆ -583,196,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ National Science Foundation—STEM Educ…   ┆    92,000,000 ┆   217,000,000 ┆ -125,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ National Oceanic and Atmospheric Admini… ┆    42,000,000 ┆    62,000,000 ┆  -20,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ National Science Foundation—Research …   ┆   608,162,000 ┆   818,162,000 ┆ -210,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Department of State—Administration of…   ┆    87,054,000 ┆   147,054,000 ┆  -60,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Bilateral Economic Assistance—Funds A…   ┆   637,902,000 ┆   937,902,000 ┆ -300,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Bilateral Economic Assistance—Departm…   ┆   915,048,000 ┆ 1,535,048,000 ┆ -620,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ International Security Assistance—Dep…   ┆    74,996,000 ┆   374,996,000 ┆ -300,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Office of Personnel Management—Salari…   ┆   219,076,000 ┆   190,784,000 ┆  +28,292,000 ┆ SEC. 126 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Department of Transportation—Federal …   ┆   617,000,000 ┆   570,000,000 ┆  +47,000,000 ┆ SEC. 137 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Department of Transportation—Federal …   ┆ 2,174,200,000 ┆ 2,221,200,000 ┆  -47,000,000 ┆ SEC. 137 ┆ A   │
└───┴───────────┴──────────────────────────────────────────┴───────────────┴───────────────┴──────────────┴──────────┴─────┘
13 provisions found
```

Notice how the table automatically changes shape for CR substitutions — it shows **New**, **Old**, and **Delta** columns instead of a single Amount. This tells you exactly which programs Congress funded above or below the prior-year rate:

- Most programs were **cut**: Migration and Refugee Assistance lost $620 million (-40.4%), NSF Research lost $210 million (-25.7%)
- Two programs **increased**: OPM Salaries and Expenses gained $28 million (+14.8%) and FAA Facilities and Equipment gained $47 million (+8.2%)
- Every dollar amount has ✓ — both the new and old amounts were verified in the source text

## Step 5: Check Data Quality

The `audit` command shows how well the extraction held up against the source text:

```bash
congress-approp audit --dir data
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

The key number: **NotFound = 0** for every bill. Every dollar amount the tool extracted actually exists in the source bill text. Here's a quick guide to the other columns:

| Column | What It Means | Good Value |
|--------|--------------|------------|
| **Verified** | Dollar amount found at exactly one position in source | Higher is better |
| **NotFound** | Dollar amounts NOT found in source | **Should be 0** |
| **Ambig** | Dollar amount found at multiple positions (e.g., "$5,000,000" appears 50 times) | Not a problem — amount is correct |
| **Exact** | `raw_text` excerpt is byte-identical to source | Higher is better |
| **NormText** | `raw_text` matches after whitespace/quote normalization | Minor formatting difference |
| **TextMiss** | `raw_text` not found at any matching tier | Review manually |
| **Coverage** | Percentage of dollar strings in source text matched to a provision | 100% is ideal, <100% is often fine |

For a deeper dive into what these numbers mean, see [Verify Extraction Accuracy](../how-to/verify-accuracy.md) and [What Coverage Means](../explanation/coverage.md).

## Step 6: Export to JSON

Every command supports `--format json` for machine-readable output. This is useful for piping to `jq`, loading into Python, or just seeing the full data:

```bash
congress-approp search --dir data/hr9468 --type appropriation --format json
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

The JSON output includes every field for each provision — more detail than the table can show. Key fields to know:

- **`dollars`**: The dollar amount as an integer (no formatting)
- **`semantics`**: What the amount means — `new_budget_authority` counts toward budget totals
- **`raw_text`**: The verbatim excerpt from the bill text
- **`match_tier`**: How closely `raw_text` matched the source — `exact` means byte-identical
- **`quality`**: Overall quality assessment — `strong`, `moderate`, or `weak`
- **`provision_index`**: Position in the bill's provision list (useful for `--similar` searches)

Other output formats are also available: `--format csv` for spreadsheets, `--format jsonl` for streaming one-object-per-line output. See [Output Formats](../reference/output-formats.md) for details.

## Enrich for Fiscal Year and Subcommittee Filtering

The example data includes pre-enriched metadata, but if you extract your own bills, run `enrich` to enable fiscal year and subcommittee filtering:

```bash
congress-approp enrich --dir data      # No API key needed — runs offline
```

Once enriched, you can scope any command to a specific fiscal year and subcommittee:

```bash
# FY2026 THUD subcommittee only
congress-approp summary --dir data --fy 2026 --subcommittee thud

# See advance vs current-year spending
congress-approp summary --dir data --fy 2026 --subcommittee milcon-va --show-advance

# Compare THUD across fiscal years
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir data

# Trace one provision across all bills
congress-approp relate 118-hr9468:0 --dir data --fy-timeline
```

See [Enrich Bills with Metadata](../how-to/enrich-data.md) for the full guide.

## What's Next

Now that you know the basics, choose your path:

- **Want to filter by fiscal year or subcommittee?** → [Enrich Bills with Metadata](../how-to/enrich-data.md)
- **Want to find specific spending?** → [Find How Much Congress Spent on a Topic](../tutorials/find-spending-on-topic.md)
- **Want to compare bills across fiscal years?** → [Compare Two Bills](../tutorials/compare-two-bills.md)
- **Want to track a program across all bills?** → [Track a Program Across Bills](../tutorials/track-program-across-bills.md)
- **Want to export data to Excel or Python?** → [Export Data for Spreadsheets and Scripts](../tutorials/export-data.md)
- **Want to understand the output better?** → [Understanding the Output](./understanding-output.md) (next chapter)
- **Want to extract your own bills?** → [Extract Your Own Bill](../tutorials/extract-your-own-bill.md)
- **Want to search by meaning instead of keywords?** → [Use Semantic Search](../tutorials/semantic-search.md)