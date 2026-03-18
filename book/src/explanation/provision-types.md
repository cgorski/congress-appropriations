# The Provision Type System

Every provision extracted from an appropriations bill is classified into one of 11 types. This classification determines what fields are available, how dollar amounts are interpreted, and how the provision contributes to budget authority calculations. This chapter documents each type in detail with real examples from the included data.

## Overview

The `Provision` enum in the Rust source code uses tagged serialization — each JSON object self-identifies with a `provision_type` field:

```json
{"provision_type": "appropriation", "account_name": "...", "amount": {...}, ...}
{"provision_type": "rescission", "account_name": "...", "amount": {...}, ...}
{"provision_type": "rider", "description": "...", ...}
```

This means you can always determine a provision's type by reading the `provision_type` field. Different types carry different fields, but all share a set of common fields.

## Common Fields (All Provision Types)

Every provision, regardless of type, has these fields:

| Field | Type | Description |
|-------|------|-------------|
| `provision_type` | string | The type discriminator (e.g., `"appropriation"`, `"rescission"`) |
| `section` | string | Section header from the bill (e.g., `"SEC. 101"`). Empty string if no section applies. |
| `division` | string or null | Division letter for omnibus bills (e.g., `"A"`). Null for bills without divisions. |
| `title` | string or null | Title numeral (e.g., `"IV"`, `"XIII"`). Null if not determinable. |
| `confidence` | float | LLM self-assessed confidence, 0.0–1.0. **Not calibrated** — useful only for identifying outliers below 0.90. Values above 0.90 are not meaningfully differentiated. |
| `raw_text` | string | Verbatim excerpt from the bill text (~first 150 characters of the provision). Verified against the source text. |
| `notes` | array of strings | Explanatory annotations — flags unusual patterns, drafting inconsistencies, or contextual information like "advance appropriation" or "no-year funding." |
| `cross_references` | array of objects | References to other laws, sections, or bills. Each has `ref_type`, `target`, and optional `description`. |

## Distribution in the Example Data

Not every bill contains every type. The distribution reflects the nature of each bill:

| Type | H.R. 4366 (Omnibus) | H.R. 5860 (CR) | H.R. 9468 (Supp) | Total |
|------|---------------------|-----------------|-------------------|-------|
| `appropriation` | 1,216 | 5 | 2 | 1,223 |
| `limitation` | 456 | 4 | — | 460 |
| `rider` | 285 | 49 | 2 | 336 |
| `directive` | 120 | 2 | 3 | 125 |
| `other` | 84 | 12 | — | 96 |
| `rescission` | 78 | — | — | 78 |
| `transfer_authority` | 77 | — | — | 77 |
| `mandatory_spending_extension` | 40 | 44 | — | 84 |
| `directed_spending` | 8 | — | — | 8 |
| `cr_substitution` | — | 13 | — | 13 |
| `continuing_resolution_baseline` | — | 1 | — | 1 |
| **Total** | **2,364** | **130** | **7** | **2,501** |

Key patterns:
- **The omnibus** is dominated by appropriations (51%), limitations (19%), and riders (12%)
- **The CR** is dominated by riders (38%) and mandatory spending extensions (34%), with only 13 CR substitutions and 5 standalone appropriations
- **The supplemental** has just 2 appropriations and 5 non-spending provisions (riders and directives)

## The 11 Provision Types

### `appropriation`

**What it is:** A grant of budget authority — the core spending provision. This is what most people think of when they think of an appropriations bill: Congress authorizing an agency to spend a specific amount of money.

**In bill text:** Typically appears as: *"For necessary expenses of [account], $X,XXX,XXX,XXX..."*

**Real example from H.R. 9468:**

```json
{
  "provision_type": "appropriation",
  "account_name": "Compensation and Pensions",
  "agency": "Department of Veterans Affairs",
  "amount": {
    "value": { "kind": "specific", "dollars": 2285513000 },
    "semantics": "new_budget_authority",
    "text_as_written": "$2,285,513,000"
  },
  "detail_level": "top_level",
  "availability": "to remain available until expended",
  "fiscal_year": 2024,
  "parent_account": null,
  "provisos": [],
  "earmarks": [],
  "raw_text": "For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.",
  "confidence": 0.99
}
```

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | The appropriations account name, extracted from `''` delimiters in the bill text |
| `agency` | string or null | Parent department or agency |
| `program` | string or null | Sub-account or program name if specified |
| `amount` | Amount | Dollar amount with semantics (see [Amount Fields](#amount-fields) below) |
| `fiscal_year` | integer or null | Fiscal year the funds are available for |
| `availability` | string or null | Fund availability period (e.g., "to remain available until expended") |
| `provisos` | array | "Provided, That" conditions attached to the appropriation |
| `earmarks` | array | Community project funding items |
| `detail_level` | string | `"top_level"`, `"line_item"`, `"sub_allocation"`, or `"proviso_amount"` |
| `parent_account` | string or null | For sub-allocations, the parent account name |

**Budget authority impact:** Appropriations with `semantics: "new_budget_authority"` at `detail_level: "top_level"` or `"line_item"` are counted in the budget authority total. Sub-allocations and proviso amounts are excluded to prevent double-counting.

**Count:** 1,223 across example data (49% of all provisions)

---

### `rescission`

**What it is:** Cancellation of previously appropriated funds. Congress is taking back money it already gave — reducing net budget authority.

**In bill text:** Typically contains phrases like *"is hereby rescinded"* or *"is rescinded."*

**Real example from H.R. 4366:**

```json
{
  "provision_type": "rescission",
  "account_name": "Nonrecurring Expenses Fund",
  "agency": "Department of Health and Human Services",
  "amount": {
    "value": { "kind": "specific", "dollars": 12440000000 },
    "semantics": "rescission",
    "text_as_written": "$12,440,000,000"
  },
  "reference_law": "Fiscal Responsibility Act of 2023",
  "fiscal_years": null
}
```

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | Account being rescinded from |
| `agency` | string or null | Department or agency |
| `amount` | Amount | Dollar amount (semantics will be `"rescission"`) |
| `reference_law` | string or null | The law whose funds are being rescinded |
| `fiscal_years` | string or null | Which fiscal years' funds are affected |

**Budget authority impact:** Rescissions are summed separately and subtracted to produce Net BA in the summary table. The $12.44B Nonrecurring Expenses Fund rescission in the example above is the largest single rescission in the FY2024 omnibus.

**Count:** 78 across example data (3.1%)

---

### `cr_substitution`

**What it is:** A continuing resolution anomaly that substitutes one dollar amount for another. The bill says "apply by substituting '$X' for '$Y'" — meaning fund the program at $X instead of the prior-year level of $Y.

**In bill text:** *"...shall be applied by substituting '$25,300,000' for '$75,300,000'..."*

**Real example from H.R. 5860:**

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
  "reference_act": "Further Consolidated Appropriations Act, 2024",
  "reference_section": "title I",
  "section": "SEC. 101",
  "division": "A"
}
```

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string or null | Account affected (may be null if the bill references a statute section instead) |
| `new_amount` | Amount | The new dollar amount ($X in "substituting $X for $Y") |
| `old_amount` | Amount | The old dollar amount being replaced ($Y) |
| `reference_act` | string | The act being modified |
| `reference_section` | string | Section being modified |

**Both** `new_amount` and `old_amount` are independently verified against the source text. In the example data, all 13 CR substitution pairs are fully verified.

**Display:** When you search for `--type cr_substitution`, the table automatically shows **New**, **Old**, and **Delta** columns instead of a single Amount column.

**Count:** 13 across example data (all in H.R. 5860)

---

### `transfer_authority`

**What it is:** Permission to move funds between accounts. The dollar amount is a ceiling (maximum that may be transferred), not new spending.

**In bill text:** *"...may transfer not to exceed $X from [source] to [destination]..."*

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `from_scope` | string | Source account(s) or scope |
| `to_scope` | string | Destination account(s) or scope |
| `limit` | TransferLimit | Transfer ceiling (percentage, fixed amount, or description) |
| `conditions` | array of strings | Conditions that must be met |

**Budget authority impact:** Transfer authority provisions have `semantics: "transfer_ceiling"`. These are **not** counted in budget authority totals because they don't represent new spending — they're permission to reallocate existing funds.

**Count:** 77 across example data (all in H.R. 4366)

---

### `limitation`

**What it is:** A cap or prohibition on spending. "Not more than $X", "none of the funds", "shall not exceed $X."

**In bill text:** *"Provided, That not to exceed $279,000 shall be available for official reception and representation expenses."*

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | What is being limited |
| `amount` | Amount or null | Dollar cap, if one is specified |
| `account_name` | string or null | Account the limitation applies to |
| `parent_account` | string or null | Parent account for proviso-based limitations |

**Budget authority impact:** Limitations have `semantics: "limitation"` and are **not** counted in budget authority totals. They constrain how appropriated funds may be used, but they don't provide new spending authority.

**Count:** 460 across example data (18.4%)

---

### `directed_spending`

**What it is:** Earmark or community project funding directed to a specific recipient.

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | Account providing the funds |
| `amount` | Amount | Dollar amount directed |
| `earmark` | Earmark or null | Recipient details: `recipient`, `location`, `requesting_member` |
| `detail_level` | string | Typically `"sub_allocation"` or `"line_item"` |
| `parent_account` | string or null | Parent account name |

**Note:** Most earmarks in appropriations bills are listed in the joint explanatory statement — a separate document not included in the enrolled bill XML. The provisions extracted here are earmarks that appear in the bill text itself, which is relatively rare. Only 8 appear in the example data.

**Count:** 8 across example data (all in H.R. 4366)

---

### `mandatory_spending_extension`

**What it is:** An amendment to an authorizing statute — common in continuing resolutions and Division B/C of omnibus bills. These provisions extend, modify, or reauthorize mandatory spending programs that would otherwise expire.

**In bill text:** *"Section 330B(b)(2) of the Public Health Service Act is amended by striking '2023' and inserting '2024'."*

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `program_name` | string | Program being extended |
| `statutory_reference` | string | The statute being amended |
| `amount` | Amount or null | Dollar amount if specified |
| `period` | string or null | Duration of the extension |
| `extends_through` | string or null | End date or fiscal year |

**Budget authority impact:** If an amount is present and has `semantics: "mandatory_spending"`, it is tracked separately from discretionary budget authority.

**Count:** 84 across example data (40 in omnibus, 44 in CR)

---

### `directive`

**What it is:** A reporting requirement or instruction to an agency. No direct spending impact.

**In bill text:** *"The Secretary shall submit a report to Congress within 30 days..."*

**Real example from H.R. 9468:**

```json
{
  "provision_type": "directive",
  "description": "Requires the Inspector General of the Department of Veterans Affairs to conduct a review of the circumstances surrounding and underlying causes of the announced VBA funding shortfall for FY2024...",
  "deadlines": ["180 days after enactment"],
  "section": "SEC. 104"
}
```

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | What is being directed |
| `deadlines` | array of strings | Any deadlines mentioned |

**Budget authority impact:** None — directives don't carry dollar amounts.

**Count:** 125 across example data

---

### `rider`

**What it is:** A policy provision that doesn't directly appropriate, rescind, or limit funds. Riders establish rules, extend authorities, or set policy conditions.

**In bill text:** *"Each amount appropriated or made available by this Act is in addition to amounts otherwise appropriated for the fiscal year involved."*

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | What the rider does |
| `policy_area` | string or null | Policy domain if identifiable |

**Budget authority impact:** None — riders don't carry dollar amounts.

**Count:** 336 across example data (the second most common type)

---

### `continuing_resolution_baseline`

**What it is:** The core CR mechanism — usually SEC. 101 or equivalent — that establishes the default rule: "fund everything at the prior fiscal year's rate."

**In bill text:** *"Such amounts as may be necessary...under the authority and conditions provided in the applicable appropriations Act for fiscal year 2023..."*

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `reference_year` | integer or null | The fiscal year used as the baseline rate |
| `reference_laws` | array of strings | Laws providing the baseline funding levels |
| `rate` | string or null | Rate description |
| `duration` | string or null | How long the CR lasts |
| `anomalies` | array | Explicit anomalies (usually captured as separate `cr_substitution` provisions) |

**Budget authority impact:** The CR baseline itself doesn't have a specific dollar amount — it says "fund at last year's rate" without stating what that rate is. The CR substitutions are the exceptions to this baseline.

**Count:** 1 across example data (in H.R. 5860)

---

### `other`

**What it is:** A catch-all for provisions that don't fit neatly into any of the 10 specific types. The LLM uses this when it can't confidently classify a provision, or when the provision represents an unusual legislative pattern.

**Real examples include:** Authority for corporations to make expenditures, emergency designations under budget enforcement rules, recoveries of unobligated balances, and fee collection authorities.

**Type-specific fields:**

| Field | Type | Description |
|-------|------|-------------|
| `llm_classification` | string | The LLM's original description of what this provision is |
| `description` | string | Summary of the provision |
| `amounts` | array of Amount | Any dollar amounts mentioned |
| `references` | array of strings | Any references mentioned |
| `metadata` | object | Arbitrary key-value pairs for fields that didn't fit the standard schema |

**Important:** When the LLM produces a `provision_type` that doesn't match any of the 10 known types, the resilient parser in `from_value.rs` wraps it as `Other` with the original classification preserved in `llm_classification`. This means the data is never lost — it's just put in the catch-all bucket with full transparency about why.

In the example data, all 96 `other` provisions were deliberately classified as "other" by the LLM itself (not caught by the fallback). They represent genuinely unusual provisions like budget enforcement designations, fee authorities, and fund recovery provisions.

**Count:** 96 across example data (3.8%)

## Amount Fields

Many provision types include an `amount` field (or `new_amount`/`old_amount` for CR substitutions). The amount structure has three components:

### AmountValue (`value`)

The actual dollar figure:

| Kind | Fields | Description |
|------|--------|-------------|
| `specific` | `dollars` (integer) | An exact dollar amount. Always whole dollars. Can be negative for rescissions. |
| `such_sums` | — | Open-ended: "such sums as may be necessary." No dollar figure. |
| `none` | — | No dollar amount — the provision doesn't carry a dollar value. |

### Amount Semantics (`semantics`)

What the dollar amount represents in budget terms:

| Value | Meaning | Counted in BA? |
|-------|---------|---------------|
| `new_budget_authority` | New spending power granted to an agency | **Yes** (at top_level/line_item detail) |
| `rescission` | Cancellation of prior budget authority | Separately as rescissions |
| `reference_amount` | A dollar figure for context (sub-allocations, "of which" breakdowns) | **No** |
| `limitation` | A cap on spending | **No** |
| `transfer_ceiling` | Maximum transfer amount | **No** |
| `mandatory_spending` | Mandatory program referenced in the bill | Tracked separately |

**Distribution in example data:**

| Semantics | Count | Notes |
|-----------|-------|-------|
| `reference_amount` | 649 | Most common — sub-allocations, proviso amounts, contextual references |
| `new_budget_authority` | 511 | The core spending provisions |
| `limitation` | 167 | Caps and restrictions |
| `rescission` | 78 | Cancellations |
| `other` | 43 | Miscellaneous |
| `mandatory_spending` | 13 | Mandatory program amounts |
| `transfer_ceiling` | 2 | Transfer limits |

The fact that `reference_amount` is the most common semantics value (not `new_budget_authority`) reflects the hierarchical structure of appropriations: many provisions are breakdowns of a parent account ("of which $X shall be for..."), not independent spending authority.

### Text As Written (`text_as_written`)

The verbatim dollar string from the bill text (e.g., `"$2,285,513,000"`). This is what the verification pipeline searches for in the source text to confirm the amount is real.

## Detail Levels

The `detail_level` field on appropriation provisions indicates where the provision sits in the funding hierarchy:

| Level | Meaning | Counted in BA? |
|-------|---------|---------------|
| `top_level` | The main account appropriation (e.g., "$57B for Medical Services") | **Yes** |
| `line_item` | A numbered item within a section (e.g., "(1) $3.5B for guaranteed farm ownership loans") | **Yes** |
| `sub_allocation` | An "of which" breakdown ("of which $300M shall be for fusion energy research") | **No** |
| `proviso_amount` | A dollar amount in a "Provided, That" clause | **No** |
| `""` (empty) | Provisions where detail level doesn't apply (directives, riders) | N/A |

**Why this matters:** The `compute_totals()` function uses detail_level to avoid double-counting. If an account appropriates $8.2B and has an "of which $300M for fusion research" sub-allocation, only the $8.2B is counted — the $300M is a breakdown, not additional money. The sub-allocation has `semantics: "reference_amount"` AND `detail_level: "sub_allocation"` to make this unambiguous.

**Distribution for appropriation-type provisions in H.R. 4366:**

| Detail Level | Count |
|-------------|-------|
| `top_level` | 483 |
| `sub_allocation` | 396 |
| `line_item` | 272 |
| `proviso_amount` | 65 |

Nearly a third of appropriation provisions are sub-allocations — breakdowns that should not be double-counted.

## How Types Affect the CLI

The `search` command adapts its table display based on the provision types in the results:

- **Standard display:** Shows Bill, Type, Description/Account, Amount, Section, Div
- **CR substitutions:** Automatically shows New, Old, and Delta columns instead of a single Amount
- **Semantic search:** Adds a Sim (similarity) column at the left

The `summary` command uses provision types to compute budget authority (only `appropriation` type with `new_budget_authority` semantics) and rescissions (only `rescission` type).

The `compare` command only matches `appropriation` provisions between the base and current bill sets — other types are excluded from the comparison.

## Adding Custom Provision Types

If you need to capture a legislative pattern not covered by the existing 11 types, see [Adding a New Provision Type](../contributing/new-provision-type.md) for the implementation guide. The key files involved are:

1. `ontology.rs` — Add the enum variant
2. `from_value.rs` — Add the parsing logic
3. `prompts.rs` — Update the LLM system prompt
4. `main.rs` — Update display logic

The `Other` type serves as a bridge — provisions that could be a new type today are captured as `Other` with full metadata, so historical data doesn't need to be re-extracted when a new type is added.

## Next Steps

- **[Budget Authority Calculation](./budget-authority.md)** — exactly how provision types and detail levels combine to produce budget totals
- **[Provision Types Reference](../reference/provision-types.md)** — compact lookup table for all types and fields
- **[extraction.json Fields](../reference/extraction-json.md)** — complete field reference for all provision data