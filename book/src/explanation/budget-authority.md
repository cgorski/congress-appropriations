# Budget Authority Calculation

The budget authority number is the most important output of this tool — it's what journalists cite, what staffers track, and what analysts compare year over year. This chapter explains exactly how it's computed, what's included, what's excluded, and why.

## The Formula

Budget authority is computed by the `compute_totals()` function in `ontology.rs`. The logic is simple and deterministic:

```text
Budget Authority = sum of amount.value.dollars
    WHERE provision_type = "appropriation"
    AND   amount.semantics = "new_budget_authority"
    AND   detail_level NOT IN ("sub_allocation", "proviso_amount")
```

Rescissions are computed separately:

```text
Rescissions = sum of |amount.value.dollars|
    WHERE provision_type = "rescission"
    AND   amount.semantics = "rescission"
```

Net Budget Authority = Budget Authority − Rescissions.

**This computation uses the actual provisions — never the LLM's self-reported summary totals.** The LLM also produces an `ExtractionSummary` with its own `total_budget_authority` field, but this is used only for diagnostics. If the LLM's arithmetic is wrong, it doesn't matter — the provision-level sum is authoritative.

## What's Included in Budget Authority

### Top-level appropriations

The main account appropriation — the headline dollar figure for each account. For example:

```json
{
  "provision_type": "appropriation",
  "account_name": "Compensation and Pensions",
  "amount": {
    "value": { "kind": "specific", "dollars": 2285513000 },
    "semantics": "new_budget_authority"
  },
  "detail_level": "top_level"
}
```

This $2.285 billion counts toward budget authority because:
- ✓ `provision_type` is `"appropriation"`
- ✓ `semantics` is `"new_budget_authority"`
- ✓ `detail_level` is `"top_level"` (not excluded)

### Line items

Numbered items within a section — for example, when a section lists multiple accounts:

```text
(1) $3,500,000,000 for guaranteed farm ownership loans
(2) $3,100,000,000 for farm ownership direct loans
(3) $2,118,491,000 for unsubsidized guaranteed operating loans
```

Each is extracted as a separate provision with `detail_level: "line_item"`. Line items count toward budget authority because they represent distinct funding decisions, not breakdowns of a parent amount.

### Mandatory spending lines

Programs like SNAP ($122 billion) and VA Compensation and Pensions ($182 billion) appear as appropriation lines in the bill text, even though they're technically mandatory spending. The tool extracts what the bill says — it doesn't distinguish mandatory from discretionary. These amounts are included in the budget authority total because they have `provision_type: "appropriation"` and `semantics: "new_budget_authority"`.

This is why the omnibus total ($846 billion) is much larger than what you might expect for discretionary spending alone. See [Why the Numbers Might Not Match Headlines](./numbers-vs-headlines.md) for more on this distinction.

### Advance appropriations

Some provisions enact budget authority in the current bill but make it available starting in a future fiscal year. For example, VA Medical Services often includes an advance appropriation for the next fiscal year. These are included in the budget authority total because the bill does enact them — the `notes` field typically flags them with "advance appropriation" or similar language.

## What's Excluded from Budget Authority

### Sub-allocations (`detail_level: "sub_allocation"`)

When a provision says "of which $300,000,000 shall be for fusion energy research," the $300 million is a **breakdown** of the parent account's funding, not money on top of it. Including both the parent and the sub-allocation would double-count.

Sub-allocations are captured as separate provisions with:
- `detail_level: "sub_allocation"`
- `semantics: "reference_amount"`
- `parent_account` pointing to the parent account name

Both the detail level and the semantics independently exclude them from the budget authority sum.

**Example:** The FBI Salaries and Expenses account has:

| Provision | Amount | Detail Level | Semantics | Counted? |
|-----------|--------|-------------|-----------|----------|
| FBI S&E (main) | $10,643,713,000 | `top_level` | `new_budget_authority` | ✓ Yes |
| "of which" sub-allocation | $216,900,000 | `sub_allocation` | `reference_amount` | ✗ No |
| Reception expense limitation | $279,000 | (limitation type) | `limitation` | ✗ No |

Only the $10.6 billion top-level amount counts. The $216.9 million is a directive about how to spend part of the $10.6 billion, not additional funding.

### Proviso amounts (`detail_level: "proviso_amount"`)

Dollar amounts in "Provided, That" clauses are also excluded. These clauses attach conditions to an appropriation — they may specify sub-uses or transfer authorities, but they don't add new money.

### Transfer ceilings (`semantics: "transfer_ceiling"`)

Transfer authority provisions specify the maximum amount that may be moved between accounts. This isn't new spending — it's permission to reallocate existing funds. Transfer ceilings have `semantics: "transfer_ceiling"` and are excluded from budget authority.

### Limitations (`semantics: "limitation"`)

Spending caps ("not more than $X") constrain how appropriated funds may be used but don't provide new authority. They have `semantics: "limitation"` and are excluded.

### Reference amounts (`semantics: "reference_amount"`)

Dollar figures mentioned for context — statutory cross-references, prior-year comparisons, loan guarantee ceilings — that don't represent new spending authority. These have `semantics: "reference_amount"` and are excluded.

### Non-appropriation provision types

Only provisions with `provision_type: "appropriation"` contribute to the budget authority total. Other types are excluded entirely:

- **Rescissions** are summed separately (and subtracted for Net BA)
- **CR substitutions** set funding levels but are not directly counted as new BA in the summary (CRs fund at prior-year rates plus adjustments — the tool captures the substituted amounts but doesn't model the baseline)
- **Transfer authority**, **limitations**, **directives**, **riders**, **mandatory spending extensions**, **directed spending**, **continuing resolution baselines**, and **other** provisions are all excluded from the BA calculation

## Verifying the Calculation

You can independently verify the budget authority calculation against the example data.

### Using the CLI

```bash
congress-approp summary --dir examples --format json
```

This produces:

```json
[
  {
    "identifier": "H.R. 4366",
    "budget_authority": 846137099554,
    "rescissions": 24659349709,
    "net_ba": 821477749845
  },
  {
    "identifier": "H.R. 5860",
    "budget_authority": 16000000000,
    "rescissions": 0,
    "net_ba": 16000000000
  },
  {
    "identifier": "H.R. 9468",
    "budget_authority": 2882482000,
    "rescissions": 0,
    "net_ba": 2882482000
  }
]
```

### Using Python directly

You can replicate the calculation by reading `extraction.json` and applying the same filters:

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

The Python calculation produces exactly the same number as the CLI. If these ever diverge, something is wrong — file a bug report.

### The $22 million difference

If you sum all appropriation provisions with `new_budget_authority` semantics *without* excluding sub-allocations and proviso amounts, you get $846,159,099,554 — about $22 million more than the official total. That $22 million represents sub-allocations and proviso amounts that are correctly excluded from the budget authority sum.

This is by design: the detail_level filter prevents double-counting between parent accounts and their "of which" breakdowns.

## How Rescissions Work

Rescissions are cancellations of previously appropriated funds. They reduce the net budget authority:

```text
Net BA = Budget Authority − Rescissions
       = $846,137,099,554 − $24,659,349,709
       = $821,477,749,845  (for H.R. 4366)
```

Rescissions are always displayed as positive numbers in the summary table (absolute value), even though they represent a reduction. The subtraction happens in the Net BA column.

### The largest rescissions in the example data

| Account | Amount | Division |
|---------|--------|----------|
| Nonrecurring Expenses Fund (HHS) | $12,440,000,000 | C |
| Medical Services (VA) | $3,034,205,000 | A |
| Medical Community Care (VA) | $2,657,977,000 | A |
| Veterans Health Administration | $1,951,750,000 | A |
| Medical Support and Compliance (VA) | $1,550,000,000 | A |

The $12.44 billion HHS rescission is from the Fiscal Responsibility Act of 2023 — Congress clawing back unspent pandemic-era funds. The VA rescissions are from prior-year unobligated balances being recovered.

## CR Budget Authority

Continuing resolutions present a special case. The H.R. 5860 summary shows $16 billion in budget authority. This comes from the standalone appropriations in the CR (principally the $16 billion for FEMA Disaster Relief Fund), not from the CR baseline mechanism.

The CR baseline — "fund at prior-year rates" — doesn't have an explicit dollar amount in the bill. The tool captures the 13 CR substitutions (anomalies) that set specific levels for specific programs, but it doesn't model the total funding implied by the "continue at prior-year rate" provision. To know the full funding picture during a CR, you need both the CR data and the prior-year regular appropriations bill data.

## Why Budget Authority ≠ What You Read in Headlines

Three common sources of confusion:

### 1. This tool reports budget authority, not outlays

Budget authority is what Congress authorizes; outlays are what Treasury spends. The two differ because agencies often obligate funds in one year but disburse them over several years. Headline federal spending figures ($6.7 trillion) are in outlays. This tool reports budget authority.

### 2. Mandatory spending appears in the totals

Programs like SNAP ($122 billion) and VA Compensation and Pensions ($182 billion) appear as appropriation lines in the bill text. They're technically mandatory spending (determined by eligibility rules, not annual votes), but they show up in appropriations bills. The tool extracts what the bill says.

### 3. Not all 12 appropriations bills are in one omnibus

The FY2024 omnibus (H.R. 4366) covers MilCon-VA, Agriculture, CJS, Energy-Water, Interior, THUD, and other matters — but it does NOT cover Defense, Labor-HHS, Homeland Security, State-Foreign Ops, Financial Services, or Legislative Branch. Those were in separate legislation. So the $846 billion total represents 7 of 12 bills, not the entire discretionary budget.

See [Why the Numbers Might Not Match Headlines](./numbers-vs-headlines.md) for a comprehensive explanation of these differences.

## The Trust Model for Budget Authority

The budget authority number has several layers of protection against errors:

1. **Computed from provisions, not LLM summaries.** The `compute_totals()` function sums individual provisions. The LLM's self-reported totals are diagnostic only.

2. **Dollar amounts are verified against source text.** Every `text_as_written` dollar string is searched for in the bill XML. Across 2,501 provisions in the example data: 0 amounts not found.

3. **Sub-allocation exclusion prevents double-counting.** The `detail_level` filter is deterministic and applied in Rust code, not by the LLM.

4. **Regression-tested.** The project's integration test suite hardcodes the exact budget authority for each example bill ($846,137,099,554 / $16,000,000,000 / $2,882,482,000). Any change in extraction data or computation logic that would alter these numbers is caught by tests.

5. **Independently reproducible.** The Python calculation above reproduces the same number from the same JSON data. Anyone can verify the computation.

The weakest link is the LLM's classification of `semantics` and `detail_level` — if the LLM incorrectly labels a sub-allocation as `top_level`, it would be included in the total when it shouldn't be. The 95.6% exact raw text match rate provides indirect evidence that provisions are attributed correctly, and the hardcoded regression totals catch systematic errors, but there's no automated per-provision check of detail_level correctness.

For high-stakes analysis, spot-check a sample of provisions with `search --format json` and verify that the detail_level and semantics assignments match what the bill text actually says.

## Quick Reference

| Component | Computation | Example Data Total |
|-----------|------------|-------------------|
| **Budget Authority** | Sum of appropriation provisions with `new_budget_authority` semantics at `top_level` or `line_item` detail | $865,019,581,554 (across all 3 bills) |
| **Rescissions** | Sum of rescission provisions (absolute value) | $24,659,349,709 |
| **Net BA** | Budget Authority − Rescissions | $840,360,231,845 |

Per bill:

| Bill | Budget Authority | Rescissions | Net BA |
|------|-----------------|-------------|--------|
| H.R. 4366 (Omnibus) | $846,137,099,554 | $24,659,349,709 | $821,477,749,845 |
| H.R. 5860 (CR) | $16,000,000,000 | $0 | $16,000,000,000 |
| H.R. 9468 (Supplemental) | $2,882,482,000 | $0 | $2,882,482,000 |

## Next Steps

- **[Why the Numbers Might Not Match Headlines](./numbers-vs-headlines.md)** — understanding the gap between this tool's totals and public budget figures
- **[The Provision Type System](./provision-types.md)** — how types and semantics interact
- **[Verify Extraction Accuracy](../how-to/verify-accuracy.md)** — auditing the underlying data