# Work with CR Substitutions

> **You will need:** `congress-approp` installed, access to the `data/` directory.
>
> **You will learn:** What CR substitutions are in legislative context, how to find and interpret them, how to match them to their omnibus counterparts, and how to export them for analysis.

Continuing resolutions (CRs) fund the government at prior-year rates — but not uniformly. Specific programs get different treatment through **anomalies**, formally known as CR substitutions. These are provisions that say "substitute $X for $Y," replacing one dollar amount with another. They're politically significant because they reveal which programs Congress chose to fund above or below the default rate.

The tool extracts CR substitutions as structured data with both the new and old amounts, making them easy to find, compare, and analyze.

## What a CR Substitution Looks Like

In bill text, a CR substitution looks like this:

> ...shall be applied by substituting "$25,300,000" for "$75,300,000"...

This means: instead of continuing the Rural Community Facilities Program at its prior-year level of $75.3 million, fund it at $25.3 million — a $50 million cut.

The tool captures both sides:

```json
{
  "provision_type": "cr_substitution",
  "account_name": "Rural Housing Service—Rural Community Facilities Program Account",
  "new_amount": {
    "value": { "kind": "specific", "dollars": 25300000 },
    "semantics": "new_budget_authority",
    "text_as_written": "$25,300,000"
  },
  "old_amount": {
    "value": { "kind": "specific", "dollars": 75300000 },
    "semantics": "new_budget_authority",
    "text_as_written": "$75,300,000"
  },
  "raw_text": "except section 521(a)(2) shall be applied by substituting ''$25,300,000'' for ''$75,300,000''",
  "section": "SEC. 101",
  "division": "A"
}
```

Both dollar amounts — the new and the old — are independently verified against the source bill text.

## Find All CR Substitutions

The `--type cr_substitution` filter finds every anomaly in a continuing resolution:

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

$ = Amount status: ✓ found (unique), ≈ found (multiple matches), ✗ not found
```

Notice how the table automatically changes shape when you search for CR substitutions — instead of a single **Amount** column, you get three:

| Column | Meaning |
|--------|---------|
| **New ($)** | The new dollar amount the CR substitutes in (the "X" in "substituting X for Y") |
| **Old ($)** | The old dollar amount being replaced (the "Y") |
| **Delta ($)** | New minus Old. **Negative means a cut**, positive means an increase. |

Every dollar amount has ✓ verification — both the new and old amounts were found in the source bill text. All 13 CR substitutions in H.R. 5860 are fully verified.

## Interpret the Results

### Which programs were cut?

Eleven of the thirteen CR substitutions are negative deltas — Congress funded these programs below the prior-year level during the temporary spending period. The largest cuts:

| Account | New | Old | Delta | Cut % |
|---------|-----|-----|-------|-------|
| Migration and Refugee Assistance | $915M | $1,535M | -$620M | -40.4% |
| *(section 521(d)(1) reference)* | $123M | $706M | -$583M | -82.6% |
| Bilateral Economic Assistance | $638M | $938M | -$300M | -32.0% |
| Int'l Narcotics Control | $75M | $375M | -$300M | -80.0% |
| Rural Water and Waste Disposal | $60M | $325M | -$265M | -81.5% |

### Which programs got more?

Only two programs received increases:

| Account | New | Old | Delta | Increase % |
|---------|-----|-----|-------|------------|
| OPM Salaries and Expenses | $219M | $191M | +$28M | +14.8% |
| FAA Facilities and Equipment | $617M | $570M | +$47M | +8.2% |

### Missing account names

The third row in the table has no account name — just `$122,572,000 / $705,768,000`. This happens when the CR language references a section of law rather than naming an account directly:

```text
except section 521(d)(1) shall be applied by substituting ''$122,572,000'' for ''$705,768,000''
```

Section 521(d)(1) refers to the rental assistance voucher program under the Housing Act of 1949. The tool captures the amounts and the raw text but can't always infer the account name when the bill text uses a statutory reference instead.

You can see the full details in JSON:

```bash
congress-approp search --dir data/hr5860 --type cr_substitution --format json
```

The `raw_text` field will show the full excerpt for each provision, including the statutory reference.

## Export CR Substitutions

### CSV for spreadsheets

```bash
congress-approp search --dir data/hr5860 --type cr_substitution --format csv > cr_anomalies.csv
```

The CSV includes the `dollars` column (new amount), `old_dollars` column (old amount), and all other fields. You can compute the delta in Excel as `=A2-B2` or use the `dollars` and `old_dollars` columns directly.

### JSON for scripts

```bash
congress-approp search --dir data/hr5860 --type cr_substitution --format json > cr_anomalies.json
```

JSON output includes every field:

```json
{
  "account_name": "Rural Housing Service—Rural Community Facilities Program Account",
  "amount_status": "found",
  "bill": "H.R. 5860",
  "description": "Rural Housing Service—Rural Community Facilities Program Account",
  "division": "A",
  "dollars": 25300000,
  "match_tier": "exact",
  "old_dollars": 75300000,
  "provision_index": 3,
  "provision_type": "cr_substitution",
  "quality": "strong",
  "raw_text": "except section 521(a)(2) shall be applied by substituting ''$25,300,000'' for ''$75,300,000''",
  "section": "SEC. 101",
  "semantics": "new_budget_authority"
}
```

### Sort by largest cut using jq

```bash
congress-approp search --dir data/hr5860 --type cr_substitution --format json | \
  jq 'map(. + {delta: (.dollars - .old_dollars)}) | sort_by(.delta) | .[] |
    "\(.delta)\t\(.account_name // "unnamed")"'
```

## Match CR Substitutions to Omnibus Provisions

A natural follow-up question is: *"This CR cut Rural Water from $325M to $60M. What did the full-year omnibus give it?"*

### Using --similar

If embeddings are available, use `--similar` to find the omnibus counterpart. First, find the CR substitution's provision index:

```bash
congress-approp search --dir data/hr5860 --type cr_substitution --format json | \
  jq '.[] | select(.account_name | test("Rural.*Water"; "i")) | .provision_index'
```

Then find similar provisions across all bills:

```bash
congress-approp search --dir data --similar hr5860:<INDEX> --type appropriation --top 3
```

Even though the CR names accounts differently than the omnibus (e.g., "Rural Utilities Service—Rural Water and Waste Disposal Program Account" vs. "Rural Water and Waste Disposal Program Account"), the embedding similarity is typically in the 0.75–0.80 range — well above the threshold for confident matching.

### Using --account

If the names are close enough, a substring search works:

```bash
congress-approp search --dir data/hr4366 --account "Rural Water" --type appropriation
```

This will find the omnibus appropriation for the same program, letting you compare the CR anomaly level to the full-year funding.

## Understanding the CR Structure

Not all provisions in a CR are substitutions. The full structure of H.R. 5860 includes:

| Type | Count | Role |
|------|-------|------|
| `rider` | 49 | Policy provisions extending or modifying existing authorities |
| `mandatory_spending_extension` | 44 | Extensions of mandatory programs that would otherwise expire |
| `cr_substitution` | 13 | Anomalies — programs funded at different-than-prior-year rates |
| `other` | 12 | Miscellaneous provisions |
| `appropriation` | 5 | Standalone new appropriations (FEMA disaster relief, IG funding) |
| `limitation` | 4 | Spending caps and prohibitions |
| `directive` | 2 | Reporting requirements |
| `continuing_resolution_baseline` | 1 | The core mechanism (SEC. 101) establishing prior-year rates |

The `continuing_resolution_baseline` provision (usually SEC. 101) establishes the default rule: fund everything at the prior fiscal year's rate. The CR substitutions are exceptions to that rule. Everything else — riders, mandatory extensions, limitations — modifies or supplements the baseline.

To see the full picture:

```bash
# All provisions in the CR
congress-approp search --dir data/hr5860

# The baseline mechanism
congress-approp search --dir data/hr5860 --type continuing_resolution_baseline

# Mandatory programs extended
congress-approp search --dir data/hr5860 --type mandatory_spending_extension

# Standalone appropriations (FEMA, etc.)
congress-approp search --dir data/hr5860 --type appropriation
```

## Verify CR Substitution Amounts

Both dollar amounts in each CR substitution are independently verified. You can confirm this in the audit:

```bash
congress-approp audit --dir data/hr5860
```

The audit shows `NotFound = 0` for H.R. 5860, meaning every dollar string — including both the "new" and "old" amounts in all 13 CR substitutions — was found in the source bill text.

To verify a specific pair manually:

```bash
# Check that both amounts from the Migration and Refugee Assistance anomaly exist
grep '915,048,000' data/118-hr5860/BILLS-118hr5860enr.xml
grep '1,535,048,000' data/118-hr5860/BILLS-118hr5860enr.xml
```

Both should return matches. The source text will show them adjacent to each other in a "substituting X for Y" pattern.

## Tips for CR Analysis

1. **CRs don't show the full funding picture.** Programs not mentioned in CR substitutions are funded at the prior-year rate. The CR itself doesn't state what that rate is — you need the prior year's appropriations bill to know the baseline.

2. **Watch for paired substitutions.** The two FAA provisions at the bottom of the table (SEC. 137) have opposite deltas: +$47M for Facilities and Equipment and -$47M for the same agency's account. This is a reallocation within the same agency — not a net change in FAA funding.

3. **Some substitutions reference statute sections, not accounts.** When the bill says "section 521(d)(1) shall be applied by substituting X for Y," the tool captures both amounts but may not identify the account name. Check the `raw_text` field for the statutory reference and look it up in the U.S. Code.

4. **Export and sort by delta for the narrative.** The story is always "which programs got cut, which got more, and by how much." Export to CSV, sort by delta, and you have the outline for a briefing or article.

5. **Use `--similar` to find the regular appropriation.** Every CR anomaly corresponds to a regular appropriation in an omnibus or annual bill. The `--similar` command finds that correspondence even when naming conventions differ between bills.

## Quick Reference

```bash
# Find all CR substitutions
congress-approp search --dir data/hr5860 --type cr_substitution

# Export to CSV
congress-approp search --dir data/hr5860 --type cr_substitution --format csv > cr_subs.csv

# Export to JSON
congress-approp search --dir data/hr5860 --type cr_substitution --format json

# Find the full-year omnibus equivalent of a CR account
congress-approp search --dir data --similar hr5860:<INDEX> --type appropriation --top 3

# See all CR provisions (not just substitutions)
congress-approp search --dir data/hr5860

# Audit CR verification
congress-approp audit --dir data/hr5860
```

## Next Steps

- **[Compare Two Bills](../tutorials/compare-two-bills.md)** — account-level comparison between a CR and an omnibus
- **[Track a Program Across Bills](../tutorials/track-program-across-bills.md)** — use `--similar` to match CR accounts to their omnibus counterparts
- **[The Provision Type System](../explanation/provision-types.md)** — detailed documentation of all 11 provision types including `cr_substitution`
