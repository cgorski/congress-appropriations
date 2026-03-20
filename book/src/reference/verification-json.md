# verification.json Fields

Complete reference for every field in `verification.json` — the deterministic verification report produced by the `extract` and `upgrade` commands. No LLM is involved in generating this file; it is pure string matching and arithmetic against the source bill text.

## Top-Level Structure

```json
{
  "amount_checks": [ ... ],
  "raw_text_checks": [ ... ],
  "arithmetic_checks": [ ... ],
  "completeness": { ... },
  "summary": { ... }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `amount_checks` | array of AmountCheck | One entry per provision with a dollar amount |
| `raw_text_checks` | array of RawTextCheck | One entry per provision |
| `arithmetic_checks` | array of ArithmeticCheck | Group-level sum verification (deprecated in newer files) |
| `completeness` | Completeness | Dollar amount coverage analysis |
| `summary` | VerificationSummary | Roll-up metrics for the entire bill |

---

## Amount Checks (`amount_checks`)

One entry for each provision that has a `text_as_written` dollar string. Checks whether that exact string exists in the source bill text.

| Field | Type | Description |
|-------|------|-------------|
| `provision_index` | integer | Index into the `provisions` array in `extraction.json` (0-based) |
| `text_as_written` | string | The dollar string being checked (e.g., `"$2,285,513,000"`) |
| `found_in_source` | boolean | Whether the string was found anywhere in the source text |
| `source_positions` | array of integers | Character offset(s) where the string was found. Empty if not found. |
| `status` | string | Verification result (see below) |

### Status Values

| Status | Meaning | Action |
|--------|---------|--------|
| `verified` | Dollar string found at exactly **one** position in the source text. Highest confidence — amount is real and location is unambiguous. | None needed |
| `ambiguous` | Dollar string found at **multiple** positions. Amount is correct but location is uncertain (common for round numbers like `$5,000,000`). | Acceptable — not an error |
| `not_found` | Dollar string **not found anywhere** in the source text. The LLM may have hallucinated or misformatted the amount. | **Review manually** — check the source XML |
| `mismatch` | Internal consistency check failed — the parsed `dollars` integer doesn't match the `text_as_written` string. | **Review manually** — likely a parsing issue |

### Example

```json
{
  "provision_index": 0,
  "text_as_written": "$2,285,513,000",
  "found_in_source": true,
  "source_positions": [431],
  "status": "verified"
}
```

### Counts in Example Data

| Bill | Verified | Ambiguous | Not Found |
|------|----------|-----------|-----------|
| H.R. 4366 | 762 | 723 | 0 |
| H.R. 5860 | 33 | 2 | 0 |
| H.R. 9468 | 2 | 0 | 0 |
| **Total** | **797** | **725** | **0** |

---

## Raw Text Checks (`raw_text_checks`)

One entry per provision. Checks whether the provision's `raw_text` excerpt is a substring of the source bill text, using tiered matching.

| Field | Type | Description |
|-------|------|-------------|
| `provision_index` | integer | Index into the `provisions` array (0-based) |
| `raw_text_preview` | string | First ~80 characters of the raw text being checked |
| `is_verbatim_substring` | boolean | True only for `exact` tier matches |
| `match_tier` | string | How closely the raw text matched (see below) |
| `found_at_position` | integer or null | Character offset if exact match; null otherwise |

### Match Tiers

| Tier | Method | What It Handles | Count in Example Data |
|------|--------|-----------------|----------------------|
| `exact` | Byte-identical substring match | Clean, faithful extractions | 2,392 (95.6%) |
| `normalized` | Matches after collapsing whitespace and normalizing curly quotes (`"` → `"`) and dashes (`—` → `-`) | Unicode formatting differences from XML-to-text conversion | 71 (2.8%) |
| `spaceless` | Matches after removing all spaces | Word-joining artifacts from XML tag stripping | 0 (0.0%) |
| `no_match` | Not found at any tier | Paraphrased, truncated, or concatenated text from adjacent sections | 38 (1.5%) |

### Example

```json
{
  "provision_index": 0,
  "raw_text_preview": "For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to r",
  "is_verbatim_substring": true,
  "match_tier": "exact",
  "found_at_position": 371
}
```

---

## Arithmetic Checks (`arithmetic_checks`)

Group-level sum verification — checks whether line items within a section or title sum to a stated total.

> **Note:** This field is deprecated in newer extraction files. It may be absent or empty. When present, it uses this structure:

| Field | Type | Description |
|-------|------|-------------|
| `scope` | string | What's being summed (e.g., a title or division) |
| `extracted_sum` | integer | Sum of extracted provisions in this scope |
| `stated_total` | integer or null | Total stated in the bill, if any |
| `status` | string | `verified`, `not_found`, `mismatch`, or `no_reference` |

Old files that include this field still load correctly. New extractions and upgrades omit it.

---

## Completeness (`completeness`)

Checks whether every dollar-sign pattern in the source bill text is accounted for by at least one extracted provision.

| Field | Type | Description |
|-------|------|-------------|
| `total_dollar_amounts_in_text` | integer | How many dollar patterns the text index found in the source bill text |
| `accounted_for` | integer | How many of those patterns were matched to an extracted provision's `text_as_written` |
| `unaccounted` | array of UnaccountedAmount | Dollar amounts in the bill that no provision captured |

### UnaccountedAmount

Each entry represents a dollar string found in the source text that wasn't matched to any extracted provision:

| Field | Type | Description |
|-------|------|-------------|
| `text` | string | The dollar string (e.g., `"$500,000"`) |
| `value` | integer | Parsed dollar value |
| `position` | integer | Character offset in the source text |
| `context` | string | Surrounding text (~100 characters) for identification |

### Example

```json
{
  "total_dollar_amounts_in_text": 2,
  "accounted_for": 2,
  "unaccounted": []
}
```

For a bill with unaccounted amounts:

```json
{
  "total_dollar_amounts_in_text": 1734,
  "accounted_for": 1634,
  "unaccounted": [
    {
      "text": "$500,000",
      "value": 500000,
      "position": 45023,
      "context": "pursuant to section 502(b) of the Agricultural Credit Act, $500,000 for each State"
    }
  ]
}
```

The unaccounted amounts are typically statutory cross-references, loan guarantee ceilings, struck amounts in amendments, or prior-year references in CRs. See [What Coverage Means (and Doesn't)](../explanation/coverage.md) for detailed interpretation.

### Coverage Calculation

```text
Coverage = (accounted_for / total_dollar_amounts_in_text) × 100%
```

| Bill | Total | Accounted | Coverage |
|------|-------|-----------|----------|
| H.R. 4366 | ~1,734 | ~1,634 | 94.2% |
| H.R. 5860 | ~36 | ~22 | 61.1% |
| H.R. 9468 | 2 | 2 | 100.0% |

---

## Verification Summary (`summary`)

Roll-up metrics for the entire bill — these are the numbers displayed by the `audit` command.

| Field | Type | Description |
|-------|------|-------------|
| `total_provisions` | integer | Total provisions checked |
| `amounts_verified` | integer | Provisions whose dollar amount was found at exactly one position |
| `amounts_not_found` | integer | Provisions whose dollar amount was NOT found in source text |
| `amounts_ambiguous` | integer | Provisions whose dollar amount appeared at multiple positions |
| `raw_text_exact` | integer | Provisions with exact (byte-identical) raw text match |
| `raw_text_normalized` | integer | Provisions with normalized match |
| `raw_text_spaceless` | integer | Provisions with spaceless match |
| `raw_text_no_match` | integer | Provisions with no raw text match at any tier |
| `completeness_pct` | float | Percentage of source dollar amounts accounted for (100.0 = all captured) |
| `provisions_by_detail_level` | object | Count of provisions at each detail level (e.g., `{"top_level": 483, "sub_allocation": 396}`) |

### Example (H.R. 9468)

```json
{
  "total_provisions": 7,
  "amounts_verified": 2,
  "amounts_not_found": 0,
  "amounts_ambiguous": 0,
  "raw_text_exact": 5,
  "raw_text_normalized": 0,
  "raw_text_spaceless": 0,
  "raw_text_no_match": 2,
  "completeness_pct": 100.0,
  "provisions_by_detail_level": {
    "top_level": 2
  }
}
```

### Mapping to Audit Table Columns

| Audit Column | Summary Field |
|-------------|---------------|
| Provisions | `total_provisions` |
| Verified | `amounts_verified` |
| NotFound | `amounts_not_found` |
| Ambig | `amounts_ambiguous` |
| Exact | `raw_text_exact` |
| NormText | `raw_text_normalized` |
| Spaceless | `raw_text_spaceless` |
| TextMiss | `raw_text_no_match` |
| Coverage | `completeness_pct` |

---

## How verification.json Is Used

### By the `audit` command

The `audit` command reads `verification.json` for each bill and renders the summary metrics as the audit table.

### By the `search` command

Search uses verification data to populate these output fields:

| Search Output Field | Source in verification.json |
|---|---|
| `amount_status` | `amount_checks[i].status` — mapped to `"found"`, `"found_multiple"`, or `"not_found"` |
| `match_tier` | `raw_text_checks[i].match_tier` — `"exact"`, `"normalized"`, `"spaceless"`, or `"no_match"` |
| `quality` | Derived from both: `"strong"` if amount verified + text exact; `"moderate"` if either is imperfect; `"weak"` if amount not found; `"n/a"` for provisions without dollar amounts |

### By the `summary` command

The summary footer ("0 dollar amounts unverified across all bills") counts the total `amounts_not_found` across all loaded bills.

---

## When verification.json Is Generated

- **By `extract`:** Automatically after LLM extraction completes. Verification runs against the source XML with no LLM involvement.
- **By `upgrade`:** Re-generated when upgrading extraction data to a new schema version. The source XML must be present in the bill directory for verification to run.

If the source XML (`BILLS-*.xml`) is not present, verification is skipped and `verification.json` is not created or updated.

---

## Accessing verification.json

### From the CLI

You don't need to read this file directly — the `audit` and `search` commands surface its data in user-friendly formats.

### From Python

```python
import json

with open("data/118-hr9468/verification.json") as f:
    v = json.load(f)

# Summary metrics
print(f"Not found: {v['summary']['amounts_not_found']}")
print(f"Coverage: {v['summary']['completeness_pct']:.1f}%")
print(f"Exact text matches: {v['summary']['raw_text_exact']}")

# Check individual provisions
for check in v["amount_checks"]:
    if check["status"] == "not_found":
        print(f"WARNING: Provision {check['provision_index']}: {check['text_as_written']} not found in source")

# See unaccounted dollar amounts
for ua in v["completeness"]["unaccounted"]:
    print(f"Unaccounted: {ua['text']} at position {ua['position']}")
    print(f"  Context: {ua['context']}")
```

---

## Related References

- **[How Verification Works](../explanation/verification.md)** — detailed explanation of the three verification checks
- **[What Coverage Means (and Doesn't)](../explanation/coverage.md)** — interpreting the completeness metric
- **[Verify Extraction Accuracy](../how-to/verify-accuracy.md)** — practical guide for running and interpreting the audit
- **[extraction.json Fields](./extraction-json.md)** — the extraction data that verification checks against