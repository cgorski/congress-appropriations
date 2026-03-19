# LLM Reliability and Guardrails

Anyone evaluating whether to trust this tool's output will eventually ask: *"How do I know the LLM didn't make this up?"* This chapter answers that question comprehensively — explaining the trust model, documenting the accuracy metrics, cataloguing known failure modes, and describing what the tool can and cannot guarantee.

## The Trust Model

The architecture is designed around a single principle:

> **The LLM extracts once. Deterministic code verifies everything.**

The LLM (Claude) touches the data at exactly one point in the pipeline: during extraction (Stage 3). It reads bill text and produces structured JSON — classifying provisions, extracting dollar amounts, identifying account names, and assigning metadata like division, section, and detail level.

After that, the LLM is never consulted again. Every downstream operation — verification, budget authority computation, querying, searching, comparing, auditing — is deterministic code. If you don't trust the LLM's classification of a provision, the `raw_text` field lets you read the original bill language yourself.

This separation means:

- **Dollar amount verification** is a string search in the source XML. No LLM judgment involved.
- **Budget authority totals** are computed by summing individual provisions in Rust code. The LLM also produces its own totals, but these are diagnostic only — never used for computation.
- **Raw text matching** is byte-level substring comparison against the source. The LLM's output is checked, not trusted.
- **Semantic search ranking** uses pre-computed vectors and cosine similarity. The LLM plays no role at query time (except one small API call to embed your search text).

## Accuracy Metrics Across Example Data

The included example data — thirteen enacted bills across FY2024–FY2026 with 8,554 total provisions — provides a concrete benchmark for extraction quality:

### Dollar amount verification

| Metric | Result |
|--------|--------|
| Total provisions with dollar amounts | 1,522 |
| Dollar amounts found at unique position in source | 797 (52.4%) |
| Dollar amounts found at multiple positions in source | 725 (47.6%) |
| Dollar amounts **not found** in source | **0 (0.0%)** |

Every single dollar amount the LLM extracted actually exists in the source bill text. The 47.6% "ambiguous" rate is expected — round numbers like `$5,000,000` appear dozens of times in a large omnibus.

### Internal consistency

| Metric | Result |
|--------|--------|
| Mismatches between parsed `dollars` integer and `text_as_written` string | **0** |
| CR substitution pairs where both amounts verified | **13/13 (100%)** |

When the LLM extracts `"text_as_written": "$2,285,513,000"` and `"dollars": 2285513000`, these are independently checked for consistency. Zero mismatches across all example data.

### Raw text faithfulness

| Match Tier | Count | Percentage |
|------------|-------|-----------|
| Exact (byte-identical substring of source) | 2,392 | 95.6% |
| Normalized (matches after whitespace/quote normalization) | 71 | 2.8% |
| Spaceless (matches after removing all spaces) | 0 | 0.0% |
| No match (not found at any tier) | 38 | 1.5% |

95.6% of provisions have `raw_text` that is a byte-for-byte copy of the source bill text. The 1.5% that don't match are all non-dollar provisions — statutory amendments where the LLM slightly reformatted section references. **No provision with a dollar amount has a raw text mismatch.**

### Completeness

| Bill | Coverage |
|------|----------|
| H.R. 9468 (supplemental, 7 provisions) | 100.0% |
| H.R. 4366 (omnibus, 2,364 provisions) | 94.2% |
| H.R. 5860 (CR, 130 provisions) | 61.1% |

Coverage measures what percentage of dollar strings in the source text were captured by an extracted provision. Below 100% doesn't necessarily indicate errors — see [What Coverage Means](./coverage.md).

### Classification

| Metric | Result |
|--------|--------|
| Provisions classified into one of 10 specific types | 2,405 (96.2%) |
| Provisions classified as `other` (catch-all) | 96 (3.8%) |
| Unknown provision types caught by fallback parser | 0 |

The LLM classified 96.2% of provisions into specific types. The remaining 3.8% are genuinely unusual provisions (budget enforcement designations, fee authorities, fund recovery provisions) that the LLM correctly placed in the catch-all category rather than forcing into an inappropriate type.

## What the LLM Does Well

### Structured extraction from complex text

Appropriations bills are among the most structurally complex legislative documents — nested provisos, cross-references to other laws, hierarchical account structures, and domain-specific conventions. The LLM handles these well:

- **Account names** are correctly extracted from between `''` delimiters in the bill text
- **Dollar amounts** are parsed from formatted strings (`$10,643,713,000`) to integers (`10643713000`)
- **Sub-allocations** are correctly identified as breakdowns of parent accounts, not additional money
- **CR substitutions** are extracted with both the new and old amounts
- **Provisos** ("Provided, That" clauses) are recognized and categorized

### Handling edge cases

The system prompt includes specific instructions for legislative edge cases:

- **"Such sums as may be necessary"** — open-ended authorizations without a specific dollar figure, captured as `AmountValue::SuchSums`
- **Transfer authority ceilings** — marked as `transfer_ceiling` semantics so they don't inflate budget authority
- **Advance appropriations** — flagged in the `notes` field
- **Sub-allocation semantics** — marked as `reference_amount` to prevent double-counting

### Graceful degradation

When the LLM encounters something it can't confidently classify, it falls back to `other` rather than guessing. The `llm_classification` field preserves the LLM's description of what it thinks the provision is, so information is never lost.

The `from_value.rs` resilient parser adds another layer: if the LLM produces unexpected JSON — missing fields, wrong types, extra fields, or unknown enum values — the parser absorbs the variance, counts it, and produces a `ConversionReport` documenting every compromise. Extraction rarely fails entirely.

## Known Failure Modes

### 1. LLM non-determinism

Re-extracting the same bill may produce slightly different results:

- **Provision counts may vary** by a small number (typically ±1-3% for large bills)
- **Classifications may shift** — a provision classified as `rider` in one extraction might become `limitation` in another
- **Detail levels may change** — a sub-allocation might be classified as a line item or vice versa
- **Notes and descriptions** are generated text and will differ between runs

**Mitigation:** Dollar amounts are verified against the source text regardless of classification. Budget authority totals are regression-tested against hardcoded expected values. If the numbers match, classification differences are cosmetic.

### 2. Paraphrased raw text on statutory amendments

The 38 `no_match` provisions in the example data are all statutory amendments — provisions that modify existing law by striking and inserting text. The LLM sometimes reformats the section numbering:

- Source: `Section 1886(d)(5)(G) of the Social Security Act (42 U.S.C. 1395ww(d)(5)(G)) is amended—`
- LLM: `Section 1886(d)(5)(G) of the Social Security Act (42 U.S.C. 1395ww(d)(5)(G)) is amended— (1) clause...`

The LLM includes text from the next line, creating a raw_text that doesn't appear as-is in the source. The statutory reference and substance are correct; the excerpt boundary is slightly off.

**Mitigation:** These provisions don't carry dollar amounts, so the amount verification is unaffected. The `match_tier: "no_match"` flag lets you identify and manually review them.

### 3. Missing provisions on large bills

The FY2024 omnibus has 94.2% coverage — meaning 5.8% of dollar strings in the source text weren't captured by any provision. For a 1,500-page bill, some provisions may be missed entirely.

Common causes:
- **Token limit truncation** — if a chunk is very long, the LLM may not process all of it
- **Ambiguous provision boundaries** — the LLM may merge two provisions or skip one
- **Unusual formatting** — provisions with atypical structure may not be recognized

**Mitigation:** The `audit` command shows completeness metrics. If coverage is low for a regular bill (not a CR), re-extracting with `--parallel 1` (which may handle tricky sections more carefully) or reviewing the chunk artifacts in `chunks/` can help identify what was missed.

### 4. Sub-allocation misclassification

The LLM occasionally marks a sub-allocation as `top_level` or a top-level provision as `sub_allocation`. This affects budget authority calculations because `top_level` provisions are counted and `sub_allocation` provisions are not.

**Mitigation:** Budget authority totals are regression-tested. For the example data, the exact totals ($846,137,099,554 / $16,000,000,000 / $2,882,482,000) are hardcoded in the test suite. Any misclassification that would change these totals would be caught. For newly extracted bills, manual spot-checking of large provisions is recommended.

### 5. Agency attribution errors

The `agency` field is inferred by the LLM from context — the heading hierarchy in the bill text. Occasionally the LLM assigns a provision to the wrong agency, especially near division or title boundaries where the context shifts.

**Mitigation:** The `account_name` is usually more reliable than `agency` because it's extracted from explicit `''` delimiters in the bill text. If agency attribution matters, cross-check using `--keyword` to find the provision by its text content, then verify the heading hierarchy in the source XML.

### 6. Confidence scores are uncalibrated

The LLM assigns a `confidence` score (0.0–1.0) to each provision, but these scores are not calibrated against actual accuracy:

- Scores above 0.90 are not meaningfully differentiated — 0.95 is not reliably more accurate than 0.91
- Scores below 0.80 may indicate genuine uncertainty and are worth reviewing
- The scores are useful only for identifying outliers, not for quantitative quality assessment

**Mitigation:** Don't use confidence scores for automated filtering. Use the verification metrics (amount_status, match_tier, quality) instead — these are computed from deterministic checks, not LLM self-assessment.

## The Resilient Parsing Layer

Between the LLM's raw JSON output and the structured Rust types, there's a translation layer (`from_value.rs`) that handles the messiness of LLM output:

| LLM Output Problem | How from_value.rs Handles It |
|---|---|
| Missing field (e.g., no `fiscal_year`) | Defaults to `None` or empty string; increments `null_to_default` counter |
| Wrong type (e.g., string `"$10,000,000"` instead of integer `10000000`) | Strips formatting and parses; increments `type_coercions` counter |
| Unknown provision type (e.g., `"earmark_extension"`) | Wraps as `Provision::Other` with original classification preserved; increments `unknown_provision_types` counter |
| Extra fields not in schema | Silently ignored for known types; preserved in `metadata` map for `Other` type |
| Completely unparseable provision | Logged as warning, skipped; increments `provisions_failed` counter |

Every compromise is counted in the `ConversionReport`, which is saved with each chunk's artifacts. You can see exactly how many null-to-default conversions, type coercions, and unknown types occurred during extraction.

This design philosophy — **absorb variance, count it, never crash** — means extraction almost never fails entirely, even when the LLM produces imperfect JSON.

## What This Tool Cannot Guarantee

### Classification correctness

The tool cannot guarantee that a provision classified as `rider` is actually a rider and not a `limitation` or `directive`. Classification is LLM judgment, and there is currently no gold-standard evaluation set to measure classification accuracy.

The 11 provision types are well-defined in the system prompt, and the LLM is generally consistent, but edge cases exist. A provision that limits spending ("none of the funds shall be used for...") could be classified as either a `limitation` or a `rider` depending on context.

### Complete extraction on large bills

The tool cannot guarantee 100% completeness on large omnibus bills. The 94.2% coverage on H.R. 4366 is good but not perfect. Some provisions may be missed, especially those with unusual formatting or those that fall at chunk boundaries.

### Correct attribution

The tool verifies that dollar amounts exist in the source text (not fabricated) and that raw text excerpts are faithful (not paraphrased). But it cannot prove that the dollar amount is attributed to the *correct* account. If `$500,000,000` appears 20 times in the bill, the verification says "amount is real" but not "this $500M belongs to Program A and not Program B."

The 95.6% exact raw text match rate provides strong indirect evidence of correct attribution — when the exact bill text matches, the provision is almost certainly from the right location. But "almost certainly" is not "guaranteed."

### Consistency across re-extractions

Different extraction runs of the same bill may produce slightly different results due to LLM non-determinism. The verification pipeline ensures dollar amounts are always correct, but provision counts, classifications, and descriptions may vary.

### Fiscal year correctness

The `fiscal_year` field is inferred from context. The tool does not independently verify that the LLM assigned the correct fiscal year to each provision.

## How to Build Confidence in the Data

### For individual provisions

1. **Check `amount_status`** — should be `"found"` or `"found_multiple"`, never `"not_found"`
2. **Check `match_tier`** — `"exact"` is best, `"normalized"` is fine, `"no_match"` warrants review
3. **Check `quality`** — `"strong"` means both amount and text verified; `"moderate"` or `"weak"` means something didn't check out fully
4. **Read `raw_text`** — the bill language is right there; does it match what the provision claims?
5. **Verify against source** — `grep` the dollar string in the XML for independent confirmation

### For aggregate results

1. **Run `audit`** — check that NotFound = 0 for every bill
2. **Check budget totals** — compare to CBO scores or committee reports for sanity
3. **Spot-check** — pick 5-10 provisions at random, verify each against the source XML
4. **Cross-reference** — compare the by-agency rollup to known department-level totals

### For publication

If you're publishing numbers from this tool:

1. Always cite the specific bill and provision
2. Note that amounts are budget authority, not outlays
3. Note whether the number includes mandatory spending
4. Verify the specific provision against the source XML (takes 30 seconds with `grep`)
5. Link to the source bill on Congress.gov for reader verification

## Comparison to Alternatives

| Approach | Accuracy | Coverage | Structured? | Cost |
|----------|----------|----------|-------------|------|
| **This tool** | High (0 unverifiable amounts) | Good (94% omnibus, 100% small bills) | Yes — 11 typed provisions with full fields | LLM API costs for extraction |
| Manual reading | Perfect (human judgment) | Low (nobody reads 1,500 pages) | No — notes and spreadsheets | Staff time |
| CBO cost estimates | High (expert analysis) | Partial (aggregated by title/function) | No — PDF reports | Free (published) |
| Committee reports | High (staff analysis) | Good (account-level tables) | No — PDF/HTML reports | Free (published) |
| Keyword search on Congress.gov | Perfect (exact text) | Low (can't filter by type/amount/agency) | No — raw text search | Free |

The tool's advantage is the combination of **structured data** (searchable, filterable, comparable) with **verification against source** (every dollar amount traced to the bill text). No other approach provides both.

## Summary

| Question | Answer |
|----------|--------|
| Can the LLM hallucinate dollar amounts? | In theory, yes. In practice, 0 of 8,554 dollar amounts were unverifiable across the thirteen example bills. |
| Can the LLM misclassify provisions? | Yes — classification is LLM judgment. Dollar amounts and raw text are verified; classification is not. |
| Can the LLM miss provisions? | Yes — 94.2% coverage on the omnibus means some provisions may be missed. |
| Is the budget authority total reliable? | Yes — computed from provisions (not LLM summaries), regression-tested, and independently reproducible. |
| Should I verify before publishing? | Yes — spot-check specific provisions against the source XML. The `audit` command is your first-pass quality check. |
| Is the tool better than reading the bill myself? | For finding specific provisions across 1,500 pages, absolutely. For understanding a single provision in depth, read the bill. |

## Next Steps

- **[Verify Extraction Accuracy](../how-to/verify-accuracy.md)** — practical guide for auditing results
- **[How Verification Works](./verification.md)** — technical details of the three verification checks
- **[What Coverage Means (and Doesn't)](./coverage.md)** — understanding the completeness metric