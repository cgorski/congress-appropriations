# Compare Two Bills

> **You will need:** `congress-approp` installed, access to the `examples/` directory.
>
> **You will learn:** How to use the `compare` command to see which accounts gained, lost, or changed funding between two sets of bills.

One of the most common questions in appropriations analysis is: *"What changed?"* Maybe you're comparing a continuing resolution to the full-year omnibus to see which programs got different treatment. Maybe you're comparing this year's omnibus to last year's. Or maybe a supplemental added emergency funding on top of the base bill and you want to see exactly where the money went.

The `compare` command answers these questions by matching accounts across two sets of bills and computing the dollar difference.

## Your First Comparison

Let's compare the FY2024 omnibus (H.R. 4366) to the VA supplemental (H.R. 9468) to see which accounts got additional emergency funding:

```bash
congress-approp compare --base examples/hr4366 --current examples/hr9468
```

The tool first prints a warning:

```text
⚠  Comparing Omnibus to Supplemental. Accounts in one but not the other may be expected
    — this does not necessarily indicate policy changes.
```

This is important context. A supplemental only touches a handful of accounts, so most accounts from the omnibus will show up as "only in base." That's expected — the supplemental didn't eliminate those programs.

The comparison table follows, sorted by largest absolute change first:

```text
┌─────────────────────────────────────┬──────────────────────┬─────────────────┬───────────────┬──────────────────┬─────────┬──────────────┐
│ Account                             ┆ Agency               ┆        Base ($) ┆   Current ($) ┆        Delta ($) ┆     Δ % ┆ Status       │
╞═════════════════════════════════════╪══════════════════════╪═════════════════╪═══════════════╪══════════════════╪═════════╪══════════════╡
│ Compensation and Pensions           ┆ Department of Veter… ┆ 197,382,903,000 ┆ 2,285,513,000 ┆ -195,097,390,000 ┆  -98.8% ┆ changed      │
│ Supplemental Nutrition Assistance … ┆ Department of Agric… ┆ 122,382,521,000 ┆             0 ┆ -122,382,521,000 ┆ -100.0% ┆ only in base │
│ Medical Services                    ┆ Department of Veter… ┆  71,000,000,000 ┆             0 ┆  -71,000,000,000 ┆ -100.0% ┆ only in base │
│ Child Nutrition Programs            ┆ Department of Agric… ┆  33,266,226,000 ┆             0 ┆  -33,266,226,000 ┆ -100.0% ┆ only in base │
│ ...                                                                                                                                       │
│ Readjustment Benefits               ┆ Department of Veter… ┆  13,774,657,000 ┆   596,969,000 ┆  -13,177,688,000 ┆  -95.7% ┆ changed      │
│ ...                                                                                                                                       │
└─────────────────────────────────────┴──────────────────────┴─────────────────┴───────────────┴──────────────────┴─────────┴──────────────┘
```

## Understanding the Columns

| Column | Meaning |
|--------|---------|
| **Account** | The appropriations account name, matched between the two bill sets |
| **Agency** | The parent department or agency |
| **Base ($)** | Total budget authority for this account in the `--base` bills |
| **Current ($)** | Total budget authority for this account in the `--current` bills |
| **Delta ($)** | Current minus Base |
| **Δ %** | Percentage change from base to current |
| **Status** | How the account appears across the two sets (see below) |

### Status values

| Status | Meaning |
|--------|---------|
| `changed` | Account exists in both base and current with different dollar amounts |
| `unchanged` | Account exists in both with the same amount (rare in practice) |
| `only in base` | Account exists in the base bills but not in the current bills |
| `only in current` | Account exists in the current bills but not in the base bills |

## Interpreting Cross-Type Comparisons

The comparison above — omnibus vs. supplemental — is instructive but requires careful interpretation:

**Why "Compensation and Pensions" shows -98.8%:** The omnibus has $197B for Comp & Pensions (which includes mandatory spending). The supplemental has $2.3B. The compare command shows the raw dollar values in each set — it doesn't add them together. The supplemental is *additional* funding on top of the omnibus, but the compare table shows the amounts *within each set*, not cumulative totals.

**Why most accounts show "only in base":** The supplemental only funds two accounts (Comp & Pensions and Readjustment Benefits). Every other account in the omnibus has zero representation in the supplemental. This doesn't mean those programs lost funding — it means the supplemental didn't touch them.

**The classification warning:** The tool detects when you're comparing different bill types (Omnibus vs. Supplemental, CR vs. Regular, etc.) and prints a warning. These cross-type comparisons can be misleading if you interpret "only in base" as "program eliminated."

## A More Natural Comparison: Filtering by Agency

To focus on just the accounts that matter, use `--agency` to narrow the comparison:

```bash
congress-approp compare --base examples/hr4366 --current examples/hr9468 --agency "Veterans"
```

This filters both sides to only show accounts from the Department of Veterans Affairs, making the comparison much easier to read. You'll see the two "changed" accounts (Comp & Pensions and Readjustment Benefits) plus the VA accounts that are "only in base."

## When Compare Shines: Same-Type Comparisons

The compare command is most powerful when comparing bills of the same type:

- **FY2023 omnibus → FY2024 omnibus:** See which programs gained or lost funding year over year
- **House version → Senate version:** Track differences during the conference process
- **FY2024 omnibus → FY2025 omnibus:** Year-over-year trend analysis

To do this, extract both bills into separate directories, then:

```bash
# Example: comparing two fiscal years (requires extracting both bills first)
congress-approp compare --base data/117/hr/2471 --current data/118/hr/4366
```

Accounts are matched by `(agency, account_name)` with automatic normalization. Results are sorted by the absolute value of the delta, so the biggest changes appear first.

## Handling Account Name Mismatches

The compare command matches accounts by exact normalized name. If Congress renames an account between fiscal years — say, "Cybersecurity and Infrastructure Security Agency" becomes "CISA Operations and Support" — the compare command will show the old name as "only in base" and the new name as "only in current" rather than matching them.

For accounts with different names that represent the same program, use the `--similar` flag on `search` to find the semantic match:

```bash
congress-approp search --dir examples --similar hr9468:0 --top 5
```

This uses embedding vectors to match by meaning rather than account name. See [Track a Program Across Bills](./track-program-across-bills.md) for details.

The `compare --use-links` flag uses persistent cross-bill relationships (created via `link accept`) to inform the matching, handling renames automatically. See [Track a Program Across Bills](./track-program-across-bills.md) for the full link workflow.

## Export Comparisons

Like all query commands, `compare` supports multiple output formats:

```bash
# CSV for Excel analysis
congress-approp compare --base examples/hr4366 --current examples/hr9468 --format csv > comparison.csv

# JSON for programmatic processing
congress-approp compare --base examples/hr4366 --current examples/hr9468 --format json
```

The JSON output includes every field for each account delta:

```json
[
  {
    "account_name": "Compensation and Pensions",
    "agency": "Department of Veterans Affairs",
    "base_dollars": 197382903000,
    "current_dollars": 2285513000,
    "delta": -195097390000,
    "delta_pct": -98.84,
    "status": "changed"
  }
]
```

This is useful for building year-over-year tracking dashboards or automated change reports.

## Practical Examples

### Which programs got the biggest increases?

```bash
congress-approp compare --base data/fy2023 --current data/fy2024 --format json | \
  jq '[.[] | select(.delta > 0)] | sort_by(-.delta) | .[:10]'
```

### Which programs were eliminated?

```bash
congress-approp compare --base data/fy2023 --current data/fy2024 --format json | \
  jq '[.[] | select(.status == "only in base")] | sort_by(-.base_dollars)'
```

### What's new this year?

```bash
congress-approp compare --base data/fy2023 --current data/fy2024 --format json | \
  jq '[.[] | select(.status == "only in current")] | sort_by(-.current_dollars)'
```

## Summary

The compare command is your tool for answering "what changed?" at the account level:

- Use `--base` and `--current` to point at any two directories containing extracted bills
- Results are sorted by the absolute value of the change — biggest impacts first
- The `--agency` filter helps focus on specific departments
- Pay attention to the classification warning when comparing different bill types
- Export to CSV or JSON for further analysis
- For accounts that change names between bills, use `--similar` semantic matching

## Next Steps

- **[Track a Program Across Bills](./track-program-across-bills.md)** — use embedding-based matching when account names differ
- **[Export Data for Spreadsheets and Scripts](./export-data.md)** — advanced export recipes
- **[Why the Numbers Might Not Match Headlines](../explanation/numbers-vs-headlines.md)** — understand why budget authority figures may differ from public reports