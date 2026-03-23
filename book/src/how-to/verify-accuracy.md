# Verify Extraction Accuracy

> **You will need:** `congress-approp` installed, access to extracted bill data (the `data/` directory works).
>
> **You will learn:** How to run a full verification audit, interpret every metric, trace individual provisions back to source XML, and decide whether extraction quality is sufficient for your use case.

Extraction uses an LLM to classify and structure provisions from bill text. Verification uses deterministic code — no LLM involved — to check every claim the extraction made against the source. This guide walks you through the complete verification workflow.

## Step 1: Run the Audit

The `audit` command is your primary verification tool:

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

Column Guide:
  Verified   Dollar amount string found at exactly one position in source text
  NotFound   Dollar amounts NOT found in source — not present in source, review manually
  Ambig      Dollar amounts found multiple times in source — correct but position uncertain
  Exact      raw_text is byte-identical substring of source — verbatim copy
  NormText   raw_text matches after whitespace/quote/dash normalization — content correct
  Spaceless  raw_text matches only after removing all spaces — PDF artifact, review
  TextMiss   raw_text not found at any tier — may be paraphrased, review manually
  Coverage   Percentage of dollar strings in source text matched to a provision

Key:
  NotFound = 0 and Coverage = 100%   →  All amounts captured and found in source
  NotFound = 0 and Coverage < 100%   →  Extracted amounts correct, but bill has more
  NotFound > 0                       →  Some amounts need manual review
```

This is a lot of information. Let's break it down column by column.

## Step 2: Check for Unverifiable Amounts (The Critical Metric)

**The single most important number in the audit is the NotFound column.** It counts provisions where the extracted dollar string (e.g., `"$2,285,513,000"`) was not found anywhere in the source bill text.

| NotFound Value | Interpretation | Action |
|----------------|---------------|--------|
| **0** | Every extracted dollar amount exists in the source text. | No action needed — this is the ideal result. |
| **1–5** | A small number of amounts couldn't be verified. | Run `audit --verbose` to identify which provisions; manually check them against the source XML. |
| **> 5** | Significant number of unverifiable amounts. | Investigate whether extraction used the wrong source file, the model hallucinated amounts, or the XML is corrupted. Consider re-extracting. |

Across the included example data: **NotFound = 0 for every bill.** 99.995% of extracted dollar amounts were confirmed to exist in the source text. See [Accuracy Metrics](../appendix/accuracy-metrics.md) for the full breakdown.

### Verified vs. Ambiguous

The remaining provisions with dollar amounts fall into two categories:

- **Verified:** The dollar string was found at exactly **one** position in the source. This provides the strongest attribution — you know exactly where in the bill this amount comes from.
- **Ambiguous (Ambig):** The dollar string was found at **multiple** positions. The amount is correct — it's definitely in the bill — but it appears more than once, so you can't automatically pin it to a single location.

Ambiguous matches are common and expected. Round numbers like `$5,000,000` can appear 50+ times in a large omnibus bill. In H.R. 4366, 723 of 1,485 provisions with dollar amounts are ambiguous — mostly because common round-number amounts recur throughout the bill's 2,364 provisions.

**Ambiguous does not mean inaccurate.** The amount is verified to exist in the source; only the precise location is uncertain.

### Provisions without dollar amounts

Not all provisions have dollar amounts. Riders, directives, and some policy provisions carry no dollars. These provisions don't appear in the Verified/NotFound/Ambig counts. In the example data:

- H.R. 4366: 2,364 provisions, 1,485 with dollar amounts (762 verified + 723 ambiguous), 879 without
- H.R. 5860: 130 provisions, 35 with dollar amounts (33 verified + 2 ambiguous), 95 without
- H.R. 9468: 7 provisions, 2 with dollar amounts (2 verified + 0 ambiguous), 5 without

## Step 3: Examine Raw Text Matching

The right side of the audit table checks whether each provision's `raw_text` excerpt (the first ~150 characters of the bill language) is a substring of the source text. This is checked in four tiers:

### Tier 1: Exact (best)

The `raw_text` is a **byte-identical** substring of the source bill text. This means the LLM copied the text perfectly — not a single character was changed.

In the example data: approximately 95.5% of provisions match at the Exact tier across the 13-bill dataset. This is excellent and provides strong evidence that the provision is attributed to the correct location in the bill.

### Tier 2: Normalized

The `raw_text` matches after normalizing whitespace, curly quotes (`"` → `"`), and em-dashes (`—` → `-`). These differences arise from the XML-to-text conversion process — the source XML uses Unicode characters that the LLM may render differently.

In the example data: 71 provisions (2.8%) match at the Normalized tier. The content is correct; only formatting details differ.

### Tier 3: Spaceless

The `raw_text` matches only after removing all spaces. This catches cases where word boundaries differ — for example, `(1)not less than` vs. `(1) not less than`. This is typically caused by XML tags being stripped without inserting spaces.

In the example data: 0 provisions match at the Spaceless tier.

### Tier 4: No Match (TextMiss)

The `raw_text` was not found at any tier. Possible causes:

- **Truncation:** The LLM truncated a very long provision and the truncated text doesn't appear as-is in the source.
- **Paraphrasing:** The LLM rephrased the statutory language (especially common for complex amendments like "Section X is amended by striking Y and inserting Z").
- **Concatenation:** The LLM combined text from adjacent sections into one raw_text string.

In the example data: 38 provisions (1.5%) are TextMiss. Examining them reveals they are all non-dollar provisions — statutory amendments (riders and mandatory spending extensions) where the LLM slightly reformatted section references. **No provision with a dollar amount has a TextMiss in the example data.**

### What TextMiss does and doesn't mean

**TextMiss does NOT mean the provision is fabricated.** The provision's other fields (account_name, description, dollar amounts) may still be correct — it's only the raw_text excerpt that doesn't match. Dollar amounts are verified independently through the amount checks.

**TextMiss DOES mean you should review manually** if the provision is important to your analysis. Use `audit --verbose` to see which provisions are affected.

## Step 4: Use Verbose Mode for Details

When any metric raises a concern, use `--verbose` to see specific problematic provisions:

```bash
congress-approp audit --dir data --verbose
```

This adds a list of individual provisions that didn't pass verification at the highest tier. For each one, you'll see:

- The provision index
- The provision type and account name (if applicable)
- The dollar string (if applicable) and whether it was found
- The raw text preview and which match tier it achieved

This gives you enough information to manually check any provision against the source XML.

## Step 5: Trace a Specific Provision to Source

For any provision you want to verify yourself — perhaps one you plan to cite in a report or story — here's how to trace it back to the source:

### 1. Get the provision details

```bash
congress-approp search --dir data/118-hr9468 --type appropriation --format json
```

Look for the provision you're interested in. Note the `dollars`, `raw_text`, and `provision_index` fields.

For example, provision 0 of H.R. 9468:

```json
{
  "dollars": 2285513000,
  "raw_text": "For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.",
  "provision_index": 0,
  "amount_status": "found",
  "match_tier": "exact"
}
```

### 2. Verify the dollar string in the source XML

Search for the `text_as_written` dollar string in the source file:

```bash
grep '$2,285,513,000' data/118-hr9468/BILLS-118hr9468enr.xml
```

If it's found (and `amount_status` is "found"), the amount is verified. If found exactly once, the attribution is unambiguous.

### 3. Read the surrounding context

To see what the bill actually says around that dollar amount:

```bash
grep -B2 -A5 '2,285,513,000' data/118-hr9468/BILLS-118hr9468enr.xml
```

Or in Python for cleaner output:

```python
import re

with open("data/118-hr9468/BILLS-118hr9468enr.xml") as f:
    text = f.read()

idx = text.find("2,285,513,000")
if idx >= 0:
    # Get surrounding context, strip XML tags
    start = max(0, idx - 200)
    end = min(len(text), idx + 200)
    context = re.sub(r'<[^>]+>', ' ', text[start:end])
    context = re.sub(r'\s+', ' ', context).strip()
    print(f"Context: ...{context}...")
```

### 4. Compare to the extracted data

Does the context match what the provision claims? Is the account name correct? Is the amount attributed to the right program? The structured `raw_text` field should be recognizable in the source context.

For the VA Supplemental example, the source text reads:

> For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.

And the extracted `raw_text` is identical — byte-for-byte.

## Step 6: Interpret Coverage

The **Coverage** column shows the percentage of dollar-sign patterns in the source bill text that were matched to an extracted provision. This measures extraction **completeness**, not accuracy.

### 100% coverage (H.R. 9468)

Every dollar amount in the source was captured by a provision. This is ideal and common for small, simple bills.

### 94.2% coverage (H.R. 4366)

Most dollar amounts were captured, but 5.8% were not. For a 1,500-page omnibus, this is expected. The unmatched dollar strings are typically:

- **Statutory cross-references**: Dollar amounts from other laws cited in the bill text (e.g., "as authorized under section 1241(a)" where the referenced section contains a dollar amount)
- **Loan guarantee ceilings**: "$3,500,000,000 for guaranteed farm ownership loans" — these are loan volume limits, not budget authority
- **Struck amounts**: "Striking '$50,000' and inserting '$75,000'" — the old amount being struck shouldn't be an independent provision
- **Proviso sub-references**: Amounts in conditions that don't constitute independent provisions

### 61.1% coverage (H.R. 5860)

Continuing resolutions have inherently lower coverage because most of the bill text consists of references to prior-year appropriations acts. Those referenced acts contain many dollar amounts that appear in the CR's text but aren't new provisions — they're contextual citations. Only the 13 CR substitutions and a few standalone appropriations are genuine new provisions in this bill.

### When low coverage IS concerning

Coverage below 60% on a regular appropriations bill (not a CR) may indicate that the extraction missed entire sections. Investigate by:

1. Running `audit --verbose` to see which dollar amounts are unaccounted for
2. Checking whether major accounts you expect are present in `search --type appropriation`
3. Comparing the provision count to what you'd expect for a bill of that size

See [What Coverage Means (and Doesn't)](../explanation/coverage.md) for a detailed explanation.

## Step 7: Cross-Check with External Sources

For high-stakes analysis, cross-check the tool's totals against independent sources:

### CBO cost estimates

The Congressional Budget Office publishes cost estimates for most appropriations bills. These aggregate numbers can serve as a sanity check for the tool's budget authority totals. Note that CBO estimates may use slightly different accounting conventions (e.g., including or excluding advance appropriations differently).

### Committee reports

The House and Senate Appropriations Committees publish detailed reports accompanying each bill. These contain account-level funding tables that can be compared to the tool's per-account breakdowns.

### Known sources of discrepancy

Even with perfect extraction, the tool's totals may differ from external sources because:

- **Mandatory spending lines** (SNAP, VA Comp & Pensions) appear as appropriation provisions in the bill text but are not "discretionary" in the budget sense
- **Advance appropriations** are enacted in the current bill but available in a future fiscal year
- **Sub-allocations** use `reference_amount` semantics and are excluded from budget authority totals, while some external sources include them
- **Transfer authorities** have dollar ceilings that are not new spending

See [Why the Numbers Might Not Match Headlines](../explanation/numbers-vs-headlines.md) for a comprehensive explanation.

## Step 8: Decide Whether to Re-Extract

Based on your audit results, here's a decision framework:

| Situation | Recommendation |
|-----------|---------------|
| NotFound = 0, Coverage > 80%, TextMiss < 5% | **Use as-is.** Quality is high. |
| NotFound = 0, Coverage 60–80%, TextMiss < 10% | **Use with awareness.** Extraction is accurate but may be incomplete. Check specific accounts you care about. |
| NotFound = 0, Coverage < 60% (non-CR bill) | **Consider re-extracting.** Major sections may be missing. Try `--parallel 1` for more reliable extraction of tricky sections. |
| NotFound > 0 | **Investigate and possibly re-extract.** Some dollar amounts weren't found in the source. Run `audit --verbose`, manually verify the flagged provisions, and re-extract if the issues are systemic. |
| TextMiss > 10% on dollar-bearing provisions | **Re-extract.** The LLM may have been paraphrasing rather than quoting the bill text. |

### Re-extraction vs. upgrade

- **Re-extract** (`congress-approp extract --dir <path>`): Makes new LLM API calls. Use when you want a fresh extraction, possibly with a different model or after prompt improvements.
- **Upgrade** (`congress-approp upgrade --dir <path>`): No LLM calls. Re-deserializes existing data through the current schema and re-runs verification. Use when the schema or verification logic has been updated but the extraction itself is fine.

## Automated Verification in Scripts

For CI/CD or automated pipelines, you can check verification programmatically:

```bash
# Check that no dollar amounts are unverifiable across all bills
congress-approp summary --dir data --format json | python3 -c "
import sys, json
bills = json.load(sys.stdin)
# The summary footer reports unverified count
# Check budget authority totals as a regression guard
expected = {'H.R. 4366': 846137099554, 'H.R. 5860': 16000000000, 'H.R. 9468': 2882482000}
for b in bills:
    assert b['budget_authority'] == expected[b['identifier']], \
        f\"{b['identifier']} budget authority mismatch: {b['budget_authority']} != {expected[b['identifier']]}\"
print('All budget authority totals match expected values')
"
```

This is the same check used in the project's integration test suite to guard against data regressions.

## Quick Decision Table

| I need to... | Command |
|--------------|---------|
| Run a full audit | `audit --dir data` |
| See individual problematic provisions | `audit --dir data --verbose` |
| Check a specific provision's dollar amount | `grep '$AMOUNT' data/118-hr4366/BILLS-*.xml` |
| Verify a provision's raw text | Compare `raw_text` from JSON output to source XML |
| Check budget authority totals | `summary --dir data --format json` |
| Compare to external sources | `summary --dir data --by-agency` for department-level totals |

## Next Steps

- **[What Coverage Means (and Doesn't)](../explanation/coverage.md)** — detailed explanation of the coverage metric
- **[How Verification Works](../explanation/verification.md)** — the technical design of the verification pipeline
- **[LLM Reliability and Guardrails](../explanation/llm-reliability.md)** — understanding the trust model and known failure modes