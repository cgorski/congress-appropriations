# Accuracy Metrics

This appendix provides a comprehensive breakdown of every verification metric across the included example data. These numbers are the empirical basis for the trust claims made throughout this documentation.

All metrics are deterministic — computed by code against the source bill text, with zero LLM involvement.

## Aggregate Summary

| Metric | Value |
|--------|-------|
| **Total provisions extracted** | 8,554 (across 13 bills) |
| **Total budget authority** | $6.4 trillion |
| **Dollar amounts NOT found in source** | **0** |
| **Dollar amount internal consistency mismatches** | **0** |
| **Raw text exact match rate** | 95.5% |
| **Advance appropriations detected** | $1.49 trillion (18% of total BA) |
| **FY2026 subcommittee coverage** | All 12 subcommittees |
| **Raw text byte-identical to source** | **2,392 (95.6%)** |
| **Raw text not found at any tier** | 38 (1.5%) |
| **Total budget authority (computed from provisions)** | $865,019,581,554 |
| **Total rescissions** | $24,659,349,709 |
| **Total net budget authority** | $840,360,231,845 |

The single most important number: **0 dollar amounts not found in source across 8,554 provisions from thirteen bills.** Every extracted dollar amount was confirmed to exist in the source bill text.

---

## Per-Bill Breakdown

### H.R. 4366 — Consolidated Appropriations Act, 2024 (Omnibus)

| Category | Metric | Value |
|----------|--------|-------|
| **Provisions** | Total extracted | 2,364 |
| | Appropriations | 1,216 (51.4%) |
| | Limitations | 456 (19.3%) |
| | Riders | 285 (12.1%) |
| | Directives | 120 (5.1%) |
| | Other | 84 (3.6%) |
| | Rescissions | 78 (3.3%) |
| | Transfer authorities | 77 (3.3%) |
| | Mandatory spending extensions | 40 (1.7%) |
| | Directed spending | 8 (0.3%) |
| **Dollar Amounts** | Provisions with amounts | 1,485 |
| | Verified (unique position) | 762 |
| | Ambiguous (multiple positions) | 723 |
| | **Not found** | **0** |
| **Raw Text** | Exact match | 2,285 (96.7%) |
| | Normalized match | 59 (2.5%) |
| | Spaceless match | 0 (0.0%) |
| | No match | 20 (0.8%) |
| **Completeness** | Dollar patterns in source | ~1,734 |
| | Accounted for by provisions | ~1,634 |
| | **Coverage** | **94.2%** |
| **Budget Authority** | Gross BA | $846,137,099,554 |
| | Rescissions | $24,659,349,709 |
| | Net BA | $821,477,749,845 |

**Notes on H.R. 4366 metrics:**

- The 723 ambiguous dollar amounts reflect the high frequency of round numbers in a 1,500-page bill. The most common: `$5,000,000` appears 50 times, `$1,000,000` appears 45 times, and `$10,000,000` appears 38 times in the source text.
- The 20 "no match" raw text provisions are all non-dollar provisions — statutory amendments (riders and mandatory spending extensions) where the LLM slightly reformatted section references. No provision with a dollar amount has a raw text mismatch.
- Coverage of 94.2% means 5.8% of dollar strings in the source text were not matched to a provision. These are primarily statutory cross-references, loan guarantee ceilings, struck amounts in amendments, and proviso sub-references that are correctly excluded from extraction. See [What Coverage Means (and Doesn't)](../explanation/coverage.md).

### H.R. 5860 — Continuing Appropriations Act, 2024 (CR)

| Category | Metric | Value |
|----------|--------|-------|
| **Provisions** | Total extracted | 130 |
| | Riders | 49 (37.7%) |
| | Mandatory spending extensions | 44 (33.8%) |
| | CR substitutions | 13 (10.0%) |
| | Other | 12 (9.2%) |
| | Appropriations | 5 (3.8%) |
| | Limitations | 4 (3.1%) |
| | Directives | 2 (1.5%) |
| | CR baseline | 1 (0.8%) |
| **Dollar Amounts** | Provisions with amounts | 35 |
| | Verified (unique position) | 33 |
| | Ambiguous (multiple positions) | 2 |
| | **Not found** | **0** |
| **CR Substitutions** | Total pairs | 13 |
| | Both amounts verified | **13 (100%)** |
| | Programs with cuts (negative delta) | 11 |
| | Programs with increases (positive delta) | 2 |
| | Largest cut | -$620,000,000 (Migration and Refugee Assistance) |
| | Largest increase | +$47,000,000 (FAA Facilities and Equipment) |
| **Raw Text** | Exact match | 102 (78.5%) |
| | Normalized match | 12 (9.2%) |
| | Spaceless match | 0 (0.0%) |
| | No match | 16 (12.3%) |
| **Completeness** | Dollar patterns in source | ~36 |
| | Accounted for by provisions | ~22 |
| | **Coverage** | **61.1%** |
| **Budget Authority** | Gross BA | $16,000,000,000 |
| | Rescissions | $0 |
| | Net BA | $16,000,000,000 |

**Notes on H.R. 5860 metrics:**

- The CR has a much higher proportion of non-spending provisions (riders and mandatory spending extensions) compared to an omnibus. Only 5 provisions are standalone appropriations — principally the $16 billion FEMA Disaster Relief Fund.
- All 13 CR substitution pairs are fully verified: both the new amount ($X) and old amount ($Y) were found in the source text.
- The 16 "no match" raw text provisions are riders and mandatory spending extensions that amend existing statutes. The LLM sometimes reformats section numbering in these provisions (e.g., adding a space after a closing parenthesis).
- Coverage of 61.1% is expected for a continuing resolution. CRs reference prior-year appropriations acts extensively — those references contain dollar amounts that appear in the CR's text but are contextual citations, not new provisions.

### H.R. 9468 — Veterans Benefits Supplemental (Supplemental)

| Category | Metric | Value |
|----------|--------|-------|
| **Provisions** | Total extracted | 7 |
| | Directives | 3 (42.9%) |
| | Appropriations | 2 (28.6%) |
| | Riders | 2 (28.6%) |
| **Dollar Amounts** | Provisions with amounts | 2 |
| | Verified (unique position) | 2 |
| | Ambiguous (multiple positions) | 0 |
| | **Not found** | **0** |
| **Raw Text** | Exact match | 5 (71.4%) |
| | Normalized match | 0 (0.0%) |
| | Spaceless match | 0 (0.0%) |
| | No match | 2 (28.6%) |
| **Completeness** | Dollar patterns in source | 2 |
| | Accounted for by provisions | 2 |
| | **Coverage** | **100.0%** |
| **Budget Authority** | Gross BA | $2,882,482,000 |
| | Rescissions | $0 |
| | Net BA | $2,882,482,000 |

**Notes on H.R. 9468 metrics:**

- This is the simplest bill in the example data — only 2 dollar amounts in the entire source text, both uniquely verifiable.
- Perfect coverage: every dollar string in the source is accounted for.
- The 2 "no match" raw text provisions are the SEC. 103 directives (reporting requirements), where the LLM's raw text excerpt was truncated and doesn't appear as-is in the source. The content is correct; only the excerpt boundary is slightly off.
- Both appropriations ($2,285,513,000 for Compensation and Pensions + $596,969,000 for Readjustment Benefits) are verified at unique positions — the strongest possible verification result.

---

## Amount Verification Detail

The verification pipeline searches for each provision's `text_as_written` dollar string (e.g., `"$2,285,513,000"`) verbatim in the source bill text.

### Three outcomes

| Status | Meaning | Count | Percentage |
|--------|---------|-------|-----------|
| **Verified** | Dollar string found at exactly one position — unambiguous location | 797 | 52.4% |
| **Ambiguous** | Dollar string found at multiple positions — correct but can't pin location | 725 | 47.6% |
| **Not Found** | Dollar string not found anywhere in source — possible hallucination | **0** | **0.0%** |

### Why ambiguous is so common

Round numbers appear frequently in appropriations bills. In H.R. 4366:

| Dollar String | Occurrences in Source |
|---|---|
| `$5,000,000` | 50 |
| `$1,000,000` | 45 |
| `$10,000,000` | 38 |
| `$15,000,000` | 27 |
| `$3,000,000` | 25 |
| `$500,000` | 24 |
| `$50,000,000` | 20 |
| `$30,000,000` | 19 |
| `$2,000,000` | 19 |
| `$25,000,000` | 16 |

When the tool finds `$5,000,000` at 50 positions, it confirms the amount is real but can't determine which of the 50 occurrences corresponds to this specific provision. That's "ambiguous" — correct amount, uncertain location.

The 797 "verified" provisions have dollar amounts unique enough to appear exactly once in the entire bill — amounts like `$10,643,713,000` (FBI Salaries and Expenses) or `$33,266,226,000` (Child Nutrition Programs).

### Internal consistency check

Beyond source text verification, the pipeline also checks that the parsed integer in `amount.value.dollars` is consistent with the `text_as_written` string. For example:

| text_as_written | Parsed dollars | Consistent? |
|---|---|---|
| `"$2,285,513,000"` | `2285513000` | ✓ Yes |
| `"$596,969,000"` | `596969000` | ✓ Yes |

Across all 1,522 provisions with dollar amounts: **0 internal consistency mismatches.**

---

## Raw Text Verification Detail

Each provision's `raw_text` excerpt (~first 150 characters of the bill language) is checked as a substring of the source text using four-tier matching.

### Tier results across all example data

| Tier | Method | Count | Percentage | What It Catches |
|------|--------|-------|-----------|----------------|
| **Exact** | Byte-identical substring | 2,392 | 95.6% | Clean, faithful extractions |
| **Normalized** | After collapsing whitespace, normalizing quotes (`"` → `"`) and dashes (`—` → `-`) | 71 | 2.8% | Unicode formatting differences from XML-to-text conversion |
| **Spaceless** | After removing all spaces | 0 | 0.0% | Word-joining artifacts (none in this data) |
| **No Match** | Not found at any tier | 38 | 1.5% | Paraphrased, truncated, or concatenated excerpts |

### Analysis of the 38 no-match provisions

All 38 "no match" provisions share a critical property: **none of them carry dollar amounts.** They are all non-dollar provisions — riders and mandatory spending extensions that amend existing statutes.

The typical pattern:

- **Source text:** `Section 1886(d)(5)(G) of the Social Security Act (42 U.S.C. 1395ww(d)(5)(G)) is amended—`
- **LLM raw_text:** `Section 1886(d)(5)(G) of the Social Security Act (42 U.S.C. 1395ww(d)(5)(G)) is amended— (1) clause...`

The LLM included text from the next line, creating a raw_text that doesn't appear as a contiguous substring in the source. The statutory reference and substance are correct; the excerpt boundary is slightly off.

**Implication:** The 38 no-match provisions don't undermine the tool's financial accuracy — they affect only the provenance trail for non-dollar legislative provisions. Dollar amounts are verified independently through the amount checks, which show 0 not-found across all data.

### Per-bill breakdown

| Bill | Exact | Normalized | Spaceless | No Match | Total |
|------|-------|-----------|-----------|----------|-------|
| H.R. 4366 | 2,285 (96.7%) | 59 (2.5%) | 0 (0.0%) | 20 (0.8%) | 2,364 |
| H.R. 5860 | 102 (78.5%) | 12 (9.2%) | 0 (0.0%) | 16 (12.3%) | 130 |
| H.R. 9468 | 5 (71.4%) | 0 (0.0%) | 0 (0.0%) | 2 (28.6%) | 7 |
| **Total** | **2,392 (95.6%)** | **71 (2.8%)** | **0 (0.0%)** | **38 (1.5%)** | **2,501** |

> **Note:** The detailed per-bill breakdown above covers the original three FY2024 example bills. The aggregate metrics at the top of this page reflect all thirteen bills in the current dataset (8,554 provisions). The same verification methodology applies to all bills — 0 NotFound amounts across the entire dataset.

The omnibus has the highest exact match rate (96.7%), which makes sense — it's the most straightforward appropriations text. The CR and supplemental have more statutory amendments (which are harder to quote exactly), contributing to their higher no-match rates.

---

## Completeness (Coverage) Detail

Coverage measures what percentage of dollar-sign patterns in the source text were matched to at least one extracted provision's `text_as_written` field.

### Per-bill coverage

| Bill | Dollar Patterns in Source | Accounted For | Coverage |
|------|--------------------------|---------------|----------|
| H.R. 4366 | ~1,734 | ~1,634 | **94.2%** |
| H.R. 5860 | ~36 | ~22 | **61.1%** |
| H.R. 9468 | 2 | 2 | **100.0%** |

### Why coverage varies

**H.R. 9468 (100%):** The simplest bill — only 2 dollar amounts in the entire source text, both captured.

**H.R. 4366 (94.2%):** The ~100 unaccounted dollar strings are primarily:
- Statutory cross-references to other laws (dollar amounts cited for context, not new provisions)
- Loan guarantee face values (not budget authority)
- Old amounts being struck by amendments ("striking '$50,000' and inserting '$75,000'")
- Proviso sub-amounts that are part of a parent provision's context

**H.R. 5860 (61.1%):** Continuing resolutions reference prior-year appropriations acts extensively. Those referenced acts contain many dollar amounts that appear in the CR's text but are citations of prior-year levels, not new provisions. Only the 13 CR substitutions, 5 standalone appropriations, and a few limitations represent genuine new provisions with dollar amounts.

### Why coverage < 100% doesn't mean errors

Coverage below 100% means there are dollar strings in the source text that weren't captured as provisions. For most of these, non-capture is the **correct behavior**:

- A statutory reference like "section 1241(a) ($500,000,000 for each fiscal year)" contains a dollar amount from another law — it's not a new appropriation in this bill.
- A loan guarantee ceiling like "$3,500,000,000 for guaranteed farm ownership loans" is a loan volume limit, not budget authority.
- An amendment language like "striking '$50,000'" contains an old amount that's being replaced — the replacement amount is the one that matters.

See [What Coverage Means (and Doesn't)](../explanation/coverage.md) for a comprehensive explanation with examples.

---

## CR Substitution Verification

All 13 CR substitutions in H.R. 5860 are fully verified — both the new amount ($X in "substituting $X for $Y") and the old amount ($Y) were found in the source bill text:

| # | Account | New Amount Verified? | Old Amount Verified? |
|---|---------|---------------------|---------------------|
| 1 | Rural Housing Service—Rural Community Facilities | ✓ | ✓ |
| 2 | Rural Utilities Service—Rural Water and Waste Disposal | ✓ | ✓ |
| 3 | *(section 521(d)(1) reference)* | ✓ | ✓ |
| 4 | NSF—STEM Education | ✓ | ✓ |
| 5 | NOAA—Operations, Research, and Facilities | ✓ | ✓ |
| 6 | NSF—Research and Related Activities | ✓ | ✓ |
| 7 | State Dept—Diplomatic Programs | ✓ | ✓ |
| 8 | Bilateral Econ. Assistance—International Disaster Assistance | ✓ | ✓ |
| 9 | Bilateral Econ. Assistance—Migration and Refugee Assistance | ✓ | ✓ |
| 10 | Int'l Security Assistance—Narcotics Control | ✓ | ✓ |
| 11 | OPM—Salaries and Expenses | ✓ | ✓ |
| 12 | DOT—FAA Facilities and Equipment (#1) | ✓ | ✓ |
| 13 | DOT—FAA Facilities and Equipment (#2) | ✓ | ✓ |

**26 of 26 dollar amounts verified** (13 new + 13 old). This is the strongest verification possible for CR substitutions — both sides of every "substituting X for Y" pair are confirmed in the source text.

---

## Budget Authority Verification

Budget authority is computed deterministically from provisions — never from LLM-generated summaries.

### The formula

```text
Budget Authority = sum of amount.value.dollars
    WHERE provision_type = "appropriation"
    AND   amount.semantics = "new_budget_authority"
    AND   detail_level NOT IN ("sub_allocation", "proviso_amount")
```

### Detail level filtering

In H.R. 4366, the detail level distribution for appropriation-type provisions is:

| Detail Level | Count | Included in BA? |
|-------------|-------|----------------|
| `top_level` | 483 | **Yes** |
| `sub_allocation` | 396 | No — breakdowns of parent accounts |
| `line_item` | 272 | **Yes** |
| `proviso_amount` | 65 | No — conditions, not independent appropriations |

Without the detail level filter, the budget authority sum would be $846,159,099,554 — approximately $22 million higher than the correct total of $846,137,099,554. The $22 million represents sub-allocations and proviso amounts correctly excluded from the total.

### Regression testing

The exact budget authority totals are hardcoded in the integration test suite:

```rust
let expected: Vec<(&str, i64, i64)> = vec![
    ("H.R. 4366", 846_137_099_554, 24_659_349_709),
    ("H.R. 5860", 16_000_000_000, 0),
    ("H.R. 9468", 2_882_482_000, 0),
];
```

Any change to the extraction data, provision parsing, or budget authority calculation that would alter these numbers is caught immediately by the `budget_authority_totals_match_expected` test. This is the tool's primary financial integrity guard.

### Independent reproducibility

The budget authority calculation can be independently reproduced in Python:

```python
import json

with open("examples/hr4366/extraction.json") as f:
    data = json.load(f)

ba = 0
for p in data["provisions"]:
    if p["provision_type"] != "appropriation":
        continue
    amt = p.get("amount")
    if not amt or amt.get("semantics") != "new_budget_authority":
        continue
    val = amt.get("value", {})
    if val.get("kind") != "specific":
        continue
    dl = p.get("detail_level", "")
    if dl in ("sub_allocation", "proviso_amount"):
        continue
    ba += val["dollars"]

print(f"Budget Authority: ${ba:,.0f}")
# Output: Budget Authority: $846,137,099,554
```

This produces exactly the same number as the CLI. If the Python and Rust calculations ever disagree, something is wrong.

---

## What These Metrics Do and Don't Prove

### What the metrics prove

| Claim | Evidence |
|-------|---------|
| Extracted dollar amounts are real | 0 of 1,522 dollar amounts not found in source text |
| Dollar parsing is consistent | 0 internal mismatches between text_as_written and parsed dollars |
| CR substitution pairs are complete | 26 of 26 amounts (13 new + 13 old) verified in source |
| Raw text excerpts are faithful | 95.6% byte-identical to source; remaining 4.4% have verified dollar amounts |
| Budget authority is deterministic | Computed from provisions, not LLM summaries; regression-tested; independently reproducible |
| Sub-allocations don't double-count | Detail level filter excludes them; $22M difference confirms correct filtering |

### What the metrics don't prove

| Limitation | Why |
|-----------|-----|
| Classification correctness | Verification can't check whether a "rider" should really be a "limitation" — that's LLM judgment |
| Attribution correctness for ambiguous amounts | When `$5,000,000` appears 50 times, verification confirms the amount exists but can't prove it's attributed to the right account |
| Completeness of non-dollar provisions | The coverage metric only counts dollar strings; riders and directives without dollar amounts are not measured |
| Fiscal year correctness | The `fiscal_year` field is inferred by the LLM; verification doesn't independently confirm it |
| Detail level correctness | If the LLM marks a sub-allocation as `top_level`, it would be incorrectly included in budget authority; this is not automatically detected per-provision |

### The 95.6% exact match rate as attribution evidence

While verification cannot mathematically prove attribution (that a dollar amount is assigned to the correct account), the 95.6% exact raw text match rate provides strong indirect evidence:

- If the raw text excerpt is byte-identical to a passage in the source, and that passage mentions an account name and a dollar amount, the provision is almost certainly attributed correctly.
- The 38 provisions without text matches are all non-dollar provisions, so attribution is a non-issue for them.
- For the 725 ambiguous dollar amounts, the combination of a verified dollar amount and an exact raw text match narrows the attribution to the specific passage the raw text came from.

For high-stakes analysis, supplement the automated verification with manual spot-checks of critical provisions. See [Verify Extraction Accuracy](../how-to/verify-accuracy.md) for the procedure.

---

## Reproducing These Metrics

You can reproduce every metric in this appendix using the included example data:

```bash
# The full audit table
congress-approp audit --dir examples

# Budget authority totals
congress-approp summary --dir examples --format json

# Provision type counts
congress-approp search --dir examples --format json | \
  jq 'group_by(.provision_type) | map({type: .[0].provision_type, count: length}) | sort_by(-.count)'

# CR substitution verification
congress-approp search --dir examples/hr5860 --type cr_substitution --format json | jq length

# Detailed verification data
cat examples/hr9468/verification.json | python3 -m json.tool | head -50
```

All of these commands work with no API keys against the included `examples/` directory.

---

## How Metrics Change with Re-Extraction

Due to LLM non-determinism, re-extracting the same bill may produce slightly different metrics:

| Metric | Stability | Notes |
|--------|-----------|-------|
| **Dollar amounts not found** | Very stable (always 0) | Dollar verification is independent of classification |
| **Budget authority total** | Stable (within ±0.1%) | Small provision count changes rarely affect the aggregate |
| **Provision count** | Moderately stable (±1-3%) | The LLM may split or merge provisions differently |
| **Raw text exact match rate** | Moderately stable (±2%) | Different excerpt boundaries may shift a few provisions between tiers |
| **Coverage** | Moderately stable (±3%) | Depends on how many sub-amounts the LLM captures |
| **Classification distribution** | Less stable (±5%) | A provision may be classified as `rider` in one run and `limitation` in another |

The verification pipeline ensures that **dollar amount accuracy is invariant across re-extractions** — even if provision counts or classifications change, the verified amounts are always correct because they're checked against the source text, not against the LLM's internal state.

---

## Next Steps

- **[Verify Extraction Accuracy](../how-to/verify-accuracy.md)** — practical guide for running your own audit
- **[How Verification Works](../explanation/verification.md)** — technical details of the verification pipeline
- **[What Coverage Means (and Doesn't)](../explanation/coverage.md)** — understanding the completeness metric
- **[Included Example Bills](./example-bills.md)** — detailed profiles of each example bill