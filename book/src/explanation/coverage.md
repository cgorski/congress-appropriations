# What Coverage Means (and Doesn't)

The `audit` command includes a **Coverage** column that shows the percentage of dollar-sign patterns in the source bill text that were matched to an extracted provision. This metric is frequently misunderstood — it measures extraction **completeness**, not accuracy. A bill can have 0 unverifiable dollar amounts (perfect accuracy) and still show 61% coverage (incomplete extraction). This chapter explains exactly what coverage measures, why it's often below 100%, and when you should (and shouldn't) worry about it.

## The Definition

Coverage is computed by the completeness check in `verification.rs`:

```text
Coverage = (dollar patterns matched to a provision) / (total dollar patterns in source text) × 100%
```

The numerator counts dollar-sign patterns in the source bill text (e.g., `$51,181,397,000`, `$500,000`, `$0`) that were matched to at least one extracted provision's `text_as_written` field.

The denominator counts **every** dollar-sign pattern in the source text — including many that should not be extracted as provisions.

## Coverage in the Example Data

| Bill | Provisions | Coverage | Interpretation |
|------|-----------|----------|---------------|
| H.R. 9468 (supplemental) | 7 | **100.0%** | Every dollar amount in the source was captured |
| H.R. 4366 (omnibus) | 2,364 | **94.2%** | Most captured; 5.8% are dollar strings that aren't independent provisions |
| H.R. 5860 (CR) | 130 | **61.1%** | Many dollar strings are prior-year references in the CR text, not new provisions |

Notice that all fourteen bills have **0 unverifiable dollar amounts** (NotFound = 0 in the audit). Coverage and accuracy are independent metrics:

- **Accuracy** (NotFound) answers: "Are the extracted amounts real?" → Yes, all of them.
- **Coverage** answers: "Did we capture every dollar amount in the bill?" → Not necessarily, and that's often fine.

## Why Coverage Below 100% Is Usually Fine

Many dollar strings in bill text are **not independent provisions** and should not be extracted. Here are the most common categories:

### Statutory cross-references

Bills frequently cite dollar amounts from other laws for context. For example:

> ...pursuant to section 1241(a) of the Food Security Act ($500,000,000 for each fiscal year)...

The $500 million is from a different law being referenced — it's not a new appropriation in this bill. The dollar string appears in the source text but correctly should not be extracted as a provision.

### Loan guarantee ceilings

Agricultural and housing bills contain loan guarantee volumes:

> $3,500,000,000 for guaranteed farm ownership loans and $3,100,000,000 for farm ownership direct loans

These are loan volume limits — how much the government will guarantee in private lending. They're not budget authority (the government isn't spending this money directly). The subsidy cost of the loan guarantee may be extracted as a separate provision, but the face value of the loan volume is correctly excluded.

### Struck amounts in amendments

When a bill amends another law by changing a dollar figure:

> ...by striking "$50,000" and inserting "$75,000"...

The old amount ($50,000) appears in the source text but should not be extracted as a new provision. Only the new amount ($75,000) represents the current-law level.

### Prior-year references in continuing resolutions

This is the main reason H.R. 5860 has only 61.1% coverage. Continuing resolutions reference prior-year appropriations acts extensively:

> ...under the authority and conditions provided in the applicable appropriations Act for fiscal year 2023...

The referenced prior-year act contains hundreds of dollar amounts that appear in the CR's text as part of the legal citation. These are contextual references — they describe the baseline funding level — but they're not new provisions in the CR. Only the 13 CR substitutions (anomalies) and a few standalone appropriations represent new funding decisions in the CR itself.

### Proviso sub-references within already-captured provisions

Some dollar amounts appear within provisos that are already captured as part of a parent provision's context:

> Provided, That of the total amount available under this heading, $7,000,000 shall be for the Urban Agriculture program

If this $7M is captured as a sub-allocation provision, it's accounted for. But if it's part of the parent provision's raw_text and not separately extracted, the $7M appears in the source text but isn't "matched to a provision" in the completeness calculation. This can happen when the proviso amount is too small or too contextual to warrant a separate provision.

### Fee offsets and receipts

Some provisions reference fee amounts that offset spending:

> ...of which not to exceed $520,000,000 shall be derived from fee collections

Fee collections appear as dollar strings in the text but represent revenue, not expenditure. They may or may not be extracted as provisions depending on context.

## When Low Coverage IS Concerning

While coverage below 100% is often fine, certain patterns warrant investigation:

### Coverage below 60% on a regular appropriations bill

CRs routinely have low coverage (lots of prior-year references). But a regular appropriations bill or omnibus should generally be above 80%. If you see 50-60% coverage on a bill that should have hundreds of provisions, significant sections may have been missed.

**What to do:** Run `audit --verbose` to see the unaccounted dollar amounts. Check whether major accounts you expect are present in `search --type appropriation`. Look for gaps — are entire divisions or titles missing?

### Known major accounts not appearing

If you know a bill includes funding for a specific large program and that program doesn't appear in the search results, the extraction may have missed it — even if overall coverage looks acceptable.

**What to do:** Search by keyword: `search --keyword "program name"`. If nothing appears, check the source XML to confirm the program is in the bill, then consider re-extracting.

### Coverage dropping significantly after re-extraction

If you re-extract a bill with a different model and coverage drops from 94% to 75%, the new model may be less capable at identifying provisions.

**What to do:** Compare provision counts between the old and new extractions. Check whether the new extraction missed entire sections. Consider reverting to the original extraction or using a higher-capability model.

### Large unaccounted dollar amounts

The `audit --verbose` output lists every unaccounted dollar string with its context. If you see large amounts ($1 billion+) that aren't captured by any provision, those are worth investigating — they may represent missed appropriations rather than innocent cross-references.

**What to do:** Look at the context for each large unaccounted amount. If it starts with "For necessary expenses of..." or similar appropriation language, it's a genuine miss. If it's in the middle of a statutory reference or amendment language, it's correctly excluded.

## Why Coverage Was Removed from the Summary Table

In version 2.1.0, the coverage column was removed from the default `summary` table output. The reason: it was routinely misinterpreted as an accuracy metric.

Users would see "94.2% coverage" and think "5.8% of the data is wrong." In reality, 0% of the extracted data is wrong (NotFound = 0) — the 5.8% represents dollar strings in the source text that weren't captured, most of which are correctly excluded.

Coverage is still available in:

- **`audit` command** — shown as the rightmost column with the full column guide
- **`summary --format json`** — available as the `completeness_pct` field
- **`verification.json`** — available as `summary.completeness_pct`

The decision to keep coverage in `audit` but remove it from `summary` reflects the difference in audience: `summary` is for quick overview (journalists, analysts), while `audit` is for detailed quality assessment (auditors, developers).

## How Coverage Is Computed: Technical Details

The completeness check in `verification.rs` works as follows:

### Step 1: Build the dollar pattern index

The `text_index` module scans the entire source bill text (extracted from XML) for every pattern matching a dollar sign followed by digits and commas: `$X`, `$X,XXX`, `$X,XXX,XXX`, etc.

For H.R. 4366, this finds approximately 1,734 dollar patterns (with 1,046 unique strings, since round numbers like `$5,000,000` appear multiple times).

### Step 2: Match against extracted provisions

For each dollar pattern found in the source, the tool checks whether any extracted provision has a `text_as_written` field matching that dollar string.

A dollar pattern is "accounted for" if at least one provision claims it. Multiple provisions can claim the same dollar string (common for ambiguous amounts like `$5,000,000`).

### Step 3: Compute the percentage

```text
Coverage = (accounted dollar patterns) / (total dollar patterns) × 100%
```

For H.R. 4366: approximately 1,634 of 1,734 dollar patterns are accounted for → 94.2%.

### Step 4: List unaccounted amounts

The `verification.json` file includes a `completeness.unaccounted` array listing every dollar string that wasn't matched to a provision. Each entry includes:

- `text` — the dollar string (e.g., `"$500,000"`)
- `value` — parsed dollar value
- `position` — character offset in the source text
- `context` — surrounding text for identification

The `audit --verbose` command displays these unaccounted amounts, making it easy to review whether they're legitimate exclusions or genuine misses.

## A Decision Framework for Coverage

| Situation | Coverage | Action |
|-----------|----------|--------|
| Small simple bill (supplemental, single purpose) | 100% | No action needed — perfect |
| Omnibus, regular bill | 85–100% | Good — spot-check any unaccounted amounts >$1B |
| Omnibus, regular bill | 60–85% | Review — some provisions may be missed; run `audit --verbose` |
| Omnibus, regular bill | <60% | Investigate — likely missing entire sections; consider re-extracting |
| Continuing resolution | 50–70% | **Expected** — most dollar strings are prior-year references |
| Continuing resolution | <50% | Review — even for a CR, this is unusually low |

The key insight: **Coverage is a completeness heuristic, not an accuracy measure.** It tells you how much of the bill's dollar content was captured. NotFound (which should be 0) tells you whether the captured content is trustworthy.

## Improving Coverage

If coverage is lower than expected, consider these approaches:

### Re-extract with --parallel 1

Higher parallelism is faster but can occasionally cause issues with API rate limits or token budget allocation. Running with `--parallel 1` ensures each chunk gets full attention:

```bash
congress-approp extract --dir data/118/hr/4366 --parallel 1
```

This is much slower for large bills but may capture provisions that were missed with higher parallelism.

### Use the default model

If you extracted with a non-default model (e.g., Claude Sonnet instead of Claude Opus), the lower-capability model may have missed provisions. Re-extracting with the default model often improves coverage:

```bash
congress-approp extract --dir data/118/hr/4366
```

### Check chunk artifacts

The `chunks/` directory contains per-chunk LLM artifacts. If a specific section of the bill seems to have missing provisions, find the chunk that covers that section and examine its raw response to see what the LLM produced.

### Accept the gap

For many use cases, 94% coverage is more than sufficient. If the unaccounted amounts are all statutory references, loan ceilings, and struck amounts, the extraction is correct — it just doesn't capture every dollar string in the text, which is the right behavior.

## Summary

| Question | Answer |
|----------|--------|
| What does coverage measure? | The percentage of dollar strings in the source text matched to an extracted provision |
| Does low coverage mean the data is wrong? | **No** — accuracy (NotFound) and coverage are independent metrics |
| Why is coverage below 100%? | Many dollar strings in bill text are cross-references, loan ceilings, struck amounts, or prior-year citations — not independent provisions |
| Why is CR coverage especially low? | CRs reference prior-year acts extensively, creating many dollar strings that aren't new provisions |
| When should I worry about low coverage? | When a regular bill (not CR) is below 60%, or when known major accounts are missing |
| Where can I see coverage? | `audit` command, `summary --format json`, `verification.json` |
| Why isn't coverage in the summary table? | Removed in v2.1.0 because it was routinely misinterpreted as an accuracy metric |

## Next Steps

- **[Verify Extraction Accuracy](../how-to/verify-accuracy.md)** — the full verification workflow including coverage interpretation
- **[How Verification Works](./verification.md)** — technical details of all three verification checks
- **[Budget Authority Calculation](./budget-authority.md)** — how provisions (the numerator of coverage) feed into budget totals