# How Verification Works

Extraction uses an LLM to understand legislative language and classify provisions. Verification uses deterministic code — with zero LLM involvement — to check every claim the extraction made against the source bill text. This chapter explains the three verification checks in detail: amount verification, raw text matching, and completeness analysis.

## The Core Principle

The verification pipeline answers three independent questions:

1. **"Are the extracted dollar amounts real?"** — Does the dollar string actually exist in the source bill text?
2. **"Is the quoted text actually from the bill?"** — Is the raw text excerpt a verbatim substring of the source?
3. **"Did we miss anything?"** — How many dollar amounts in the source text were captured by extracted provisions?

Each question is answered by a different check. All three are deterministic string operations — no language model, no heuristics, no probabilistic matching. The code in `verification.rs` runs pure string searches against the source text extracted from the bill XML.

## Amount Verification

For every provision that carries a dollar amount, the verifier takes the `text_as_written` field (e.g., `"$2,285,513,000"`) and searches for that exact string in the source bill text.

### How it works

1. The `text_index` module builds a positional index of every dollar-sign pattern (`$X,XXX,XXX`) in the source text
2. For each provision with a `text_as_written` value, the verifier searches the index for that string
3. It counts how many times the string appears and records the character positions

### Three possible outcomes

| Result | Meaning | Count in Example Data |
|--------|---------|----------------------|
| **Verified** (`found`) | The dollar string was found at exactly **one** position in the source text. This is the strongest result — the amount exists, and its location is unambiguous. | 797 of 1,522 provisions with amounts |
| **Ambiguous** (`found_multiple`) | The dollar string was found at **multiple** positions. The amount is correct — it's definitely in the bill — but the same string appears more than once, so we can't automatically pin it to a specific location. | 725 of 1,522 |
| **Not Found** (`not_found`) | The dollar string was **not found anywhere** in the source text. This means the LLM may have hallucinated the amount, or the `text_as_written` field has formatting differences from the source. | **0 of 1,522** |

### Why ambiguous is common and acceptable

Round numbers appear frequently throughout appropriations bills. In the FY2024 omnibus (H.R. 4366):

| Dollar String | Occurrences in Source |
|---|---|
| `$5,000,000` | 50 |
| `$1,000,000` | 45 |
| `$10,000,000` | 38 |
| `$15,000,000` | 27 |
| `$3,000,000` | 25 |

When the tool finds `$5,000,000` in 50 places, it can confirm the amount is real but can't determine which of the 50 occurrences corresponds to this specific provision. That's an "ambiguous" result — correct amount, uncertain location.

The 762 "verified" provisions in H.R. 4366 are the ones with unique dollar amounts — numbers specific enough (like `$10,643,713,000` for FBI Salaries and Expenses) that they appear exactly once in the entire bill.

### Why not_found is critical

A `not_found` result means the extracted dollar string does not exist anywhere in the source bill text. This is the strongest signal of a potential extraction error — the LLM may have:

- Hallucinated a dollar amount
- Misread or transposed digits
- Formatted the amount differently than it appears in the source

**Across the included example data: not_found = 0 for every bill.** All 1,522 provisions with dollar amounts (797 verified + 725 ambiguous) were confirmed to exist in the source text.

### Internal consistency check

Beyond searching the source text, verification also checks that the parsed integer in `amount.value.dollars` is consistent with the `text_as_written` string. For example, if `text_as_written` is `"$2,285,513,000"` and `dollars` is `2285513000`, these are consistent. If `dollars` were `228551300` (a digit dropped), this would be flagged as a mismatch.

Across all example data: **0 internal consistency mismatches.**

## Raw Text Matching

Every provision includes a `raw_text` field — the first ~150 characters of the bill language that the provision was extracted from. The verifier checks whether this text is a verbatim substring of the source bill text. This is more than an amount check — it verifies that the provision's *context* (not just its dollar figure) comes from the actual bill.

### Four-tier matching

The verifier tries four progressively more lenient matching strategies:

#### Tier 1: Exact Match

The `raw_text` is searched as a **byte-identical** substring of the source text. No normalization, no transformation — the exact bytes must appear in the source.

**Example — exact match:**

- Source text: `For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.`
- Extracted `raw_text`: `For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.`
- Result: ✓ **Exact** — byte-identical substring

In the example data: **8,164 of 8,554 provisions (95.5%)** match at the exact tier. This is the strongest evidence that the provision was faithfully extracted from the correct location in the bill.

#### Tier 2: Normalized Match

If exact matching fails, the verifier normalizes both the `raw_text` and the source text before comparing:

- Collapse multiple whitespace characters to a single space
- Convert curly quotes (`"` `"`) to straight quotes (`"`)
- Convert em-dashes (`—`) and en-dashes (`–`) to hyphens (`-`)
- Trim leading and trailing whitespace

**Why this tier exists:** The XML-to-text conversion process can introduce minor formatting differences. The source XML may use Unicode curly quotes while the LLM output uses straight quotes. Whitespace around XML tags may be collapsed differently. These are formatting artifacts, not content errors.

In the example data: **71 provisions (2.8%)** match at the normalized tier.

#### Tier 3: Spaceless Match

If normalized matching also fails, the verifier removes **all spaces** from both strings and compares. This catches cases where word boundaries differ due to XML tag stripping — for example, `(1)not less than` vs. `(1) not less than`.

In the example data: **0 provisions** match at the spaceless tier.

#### Tier 4: No Match

If none of the three tiers find a match, the provision is marked as `no_match`. The raw text was not found in the source at any level of normalization.

**Common causes of no_match:**
- **Truncation:** The LLM truncated a very long provision, and the truncated text includes text from adjacent provisions that don't appear together in the source
- **Paraphrasing:** The LLM rephrased the statutory language instead of quoting it verbatim (most common for complex amendments like "Section X is amended by striking Y and inserting Z")
- **Concatenation:** The LLM combined text from multiple subsections into one `raw_text` field

In the example data: **38 provisions (1.5%)** are no_match. Examining them reveals an important pattern: **all 38 are non-dollar provisions** — riders and mandatory spending extensions that amend existing statutes. The LLM slightly reformatted section references in these provisions. No provision with a dollar amount has a no_match in the example data.

### What raw text matching proves (and doesn't)

**What it proves:**
- The provision text was taken from the actual bill, not fabricated
- At the exact tier: the provision is attributed to a specific, locatable passage in the source
- Combined with amount verification: the dollar figure and its context both trace to the source

**What it doesn't prove:**
- That the provision is classified correctly (is it really a "rider" vs. a "directive"?)
- That the dollar amount is attributed to the correct account (the amount exists in the source, but is it under the heading the LLM says it is?)
- That sub-allocation relationships are correct (is this really a sub-allocation of that parent account?)

The 95.6% exact match rate provides strong but not absolute attribution confidence. For the remaining 4.4%, the dollar amounts are still independently verified — you just can't be as certain about the exact source location from the raw text alone.

## Completeness Analysis

The third verification check measures **how much of the bill's content was captured** by the extraction.

### How it works

1. The `text_index` module scans the entire source text for every dollar-sign pattern (e.g., `$51,181,397,000`, `$500,000`, `$0`)
2. For each dollar pattern found, it checks whether any extracted provision has a matching `text_as_written` value
3. The completeness percentage is: `(matched dollar patterns) / (total dollar patterns in source) × 100`

### Interpreting coverage

| Bill | Coverage | Interpretation |
|------|----------|---------------|
| H.R. 9468 | **100.0%** | Every dollar amount in the source was captured. Perfect completeness — expected for a small, simple bill. |
| H.R. 4366 | **94.2%** | Most dollar amounts captured. The remaining 5.8% are dollar strings in the source text that no provision accounts for. |
| H.R. 5860 | **61.1%** | Many dollar strings in the source text are not captured. Expected for a CR — see explanation below. |

### Why coverage below 100% is often correct

Many dollar strings in bill text are **not independent provisions** and should not be extracted:

**Statutory cross-references:** "as authorized under section 1241(a) of the Food Security Act" — the referenced section contains dollar amounts, but those are amounts from a different law being cited for context.

**Loan guarantee ceilings:** "$3,500,000,000 for guaranteed farm ownership loans" — these are loan volume limits, not budget authority. They represent how much the government will guarantee in private lending, not how much it will spend.

**Struck amounts:** "striking '$50,000' and inserting '$75,000'" — when the bill amends another law by changing a dollar figure, the old amount being struck should not be extracted as a new provision.

**Prior-year references in CRs:** Continuing resolutions reference prior-year appropriations acts extensively. Those referenced acts contain many dollar amounts that appear in the CR's text but are citations, not new provisions. This is why H.R. 5860 has only 61.1% coverage — most dollar strings in the bill are references to prior-year levels, not new appropriations.

### When low coverage IS concerning

Low coverage on a regular appropriations bill (not a CR) may indicate missed provisions. Warning signs:

- **Coverage below 60%** on a regular bill or omnibus
- **Known major accounts** not appearing in `search --type appropriation`
- **Coverage dropping significantly** after re-extracting with a different model
- **Large sections** of the bill with no extracted provisions at all

If these signs appear, consider re-extracting with the default model and higher parallelism.

## Putting It All Together

The three checks provide layered confidence:

| Check | What It Verifies | Confidence Level |
|-------|-----------------|-----------------|
| Amount: verified | The dollar amount exists in the source at a unique position | **Highest** — amount is real and unambiguously located |
| Amount: ambiguous | The dollar amount exists in the source at multiple positions | **High** — amount is real, location is uncertain |
| Amount: not_found | The dollar amount doesn't exist in the source | **Alarm** — possible hallucination or formatting error |
| Raw text: exact | The bill text excerpt is byte-identical to the source | **Highest** — provision text is faithful and locatable |
| Raw text: normalized | The text matches after Unicode normalization | **High** — content is correct, formatting differs slightly |
| Raw text: no_match | The text isn't found in the source | **Review needed** — may be paraphrased or truncated |
| Coverage: 100% | All dollar strings in source are accounted for | **Complete** — nothing was missed |
| Coverage: >80% | Most dollar strings are accounted for | **Good** — some uncaptured strings are likely legitimate exclusions |
| Coverage: <60% (non-CR) | Many dollar strings are unaccounted for | **Investigate** — significant provisions may be missing |

For the included example data, the combined picture is strong:

- **0** dollar amounts not found in source (across 8,554 provisions)
- **95.6%** of raw text excerpts are byte-identical to the source
- **0** internal consistency mismatches between parsed dollars and text_as_written
- **13/13** CR substitution pairs fully verified (both new and old amounts)

## The verification.json File

All verification results are stored in `verification.json` alongside the extraction. This file contains:

- **`amount_checks`** — One entry per provision with a dollar amount: the text_as_written string, whether it was found, source positions, and status
- **`raw_text_checks`** — One entry per provision: the raw text preview, match tier (exact/normalized/spaceless/no_match), and found position
- **`completeness`** — Total dollar amounts in source, number accounted for, and a list of unaccounted dollar strings with their positions and surrounding context
- **`summary`** — Roll-up metrics: total provisions, amounts verified/not_found/ambiguous, raw text exact/normalized/spaceless/no_match, and completeness percentage

The `audit` command renders this data as the audit table. The `search` command uses it to populate the `$` column (✓/≈/✗), the `amount_status`, `match_tier`, and `quality` fields in JSON/CSV output.

See [verification.json Fields](../reference/verification-json.md) for the complete field reference.

## What Verification Cannot Check

Verification is powerful but has clear boundaries:

1. **Classification correctness.** Verification cannot tell you whether a provision classified as "rider" should actually be a "directive." That's LLM judgment, not a string-matching question.

2. **Attribution correctness.** Verification confirms that a dollar amount exists in the source text and that the raw text excerpt is faithful — but it cannot prove that the dollar amount was attributed to the *correct* account. If the bill says "$500 million for Program A" on line 100 and "$500 million for Program B" on line 200, and the LLM attributes $500M to Program B but pulls raw text from the Program A paragraph, the amount check says "ambiguous" (found multiple times) but doesn't catch the misattribution. The 95.6% exact raw text match rate provides strong evidence against this scenario — when the raw text matches exactly, attribution is very likely correct.

3. **Completeness of non-dollar provisions.** The completeness check counts dollar strings in the source. Riders, directives, and other provisions without dollar amounts are not part of the coverage metric. There is no automated way to measure whether all non-dollar provisions were captured.

4. **Correctness of sub-allocation relationships.** The tool checks that `detail_level: sub_allocation` provisions have `reference_amount` semantics (so they don't double-count), but it doesn't verify that the parent-child relationship between a sub-allocation and its parent account is correct.

5. **Fiscal year attribution.** The tool extracts `fiscal_year` from context, but verification doesn't independently confirm that the LLM assigned the right fiscal year to each provision.

For high-stakes analysis, use the `audit` command to establish baseline trust, then manually spot-check critical provisions using the procedure described in [Verify Extraction Accuracy](../how-to/verify-accuracy.md).

## Next Steps

- **[Verify Extraction Accuracy](../how-to/verify-accuracy.md)** — practical guide for running and interpreting the audit
- **[What Coverage Means (and Doesn't)](./coverage.md)** — deep dive into the completeness metric
- **[LLM Reliability and Guardrails](./llm-reliability.md)** — understanding the broader trust model
- **[verification.json Fields](../reference/verification-json.md)** — complete field reference