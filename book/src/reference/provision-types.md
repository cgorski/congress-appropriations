# Provision Types

Quick reference for all 11 provision types in the extraction schema. For detailed explanations with real examples and distribution data, see [The Provision Type System](../explanation/provision-types.md).

## At a Glance

| Type | What It Is | Has Dollar Amount? | Counted in BA? |
|------|-----------|-------------------|---------------|
| `appropriation` | Grant of budget authority | Yes | Yes (at top_level/line_item) |
| `rescission` | Cancellation of prior funds | Yes | Separately (subtracted for Net BA) |
| `cr_substitution` | CR anomaly — substituting $X for $Y | Yes (new + old) | No (CR baseline amounts) |
| `transfer_authority` | Permission to move funds between accounts | Sometimes (ceiling) | No |
| `limitation` | Cap or prohibition on spending | Sometimes | No |
| `directed_spending` | Earmark / community project funding | Yes | Depends on detail_level |
| `mandatory_spending_extension` | Amendment to authorizing statute | Sometimes | No (tracked separately) |
| `directive` | Reporting requirement or instruction | No | No |
| `rider` | Policy provision (no direct spending) | No | No |
| `continuing_resolution_baseline` | Core CR mechanism (SEC. 101) | No | No |
| `other` | Catch-all for unclassifiable provisions | Sometimes | No |

## Common Fields (All Types)

Every provision carries these fields regardless of type:

| Field | Type | Description |
|-------|------|-------------|
| `provision_type` | string | The type discriminator |
| `section` | string | Section header (e.g., `"SEC. 101"`). Empty string if none. |
| `division` | string or null | Division letter (e.g., `"A"`). Null for bills without divisions. |
| `title` | string or null | Title numeral (e.g., `"IV"`). Null if not determinable. |
| `confidence` | float | LLM self-assessed confidence, 0.0–1.0. Not calibrated — useful only for identifying outliers below 0.90. |
| `raw_text` | string | Verbatim excerpt from the bill text (~first 150 characters). Verified against source. |
| `notes` | array of strings | Explanatory annotations (e.g., "advance appropriation", "no-year funding"). |
| `cross_references` | array of CrossReference | References to other laws, sections, or bills. |

### CrossReference Fields

| Field | Type | Description |
|-------|------|-------------|
| `ref_type` | string | Relationship: `baseline_from`, `amends`, `notwithstanding`, `subject_to`, `see_also`, `transfer_to`, `rescinds_from`, `modifies`, `references`, `other` |
| `target` | string | The referenced law or section (e.g., `"31 U.S.C. 1105(a)"`) |
| `description` | string or null | Optional clarifying note |

---

## appropriation

Grant of budget authority — the core spending provision.

**Bill text pattern:** *"For necessary expenses of [account], $X,XXX,XXX,XXX..."*

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | Appropriations account name from `''` delimiters in bill text |
| `agency` | string or null | Parent department or agency |
| `program` | string or null | Sub-account or program name |
| `amount` | Amount | Dollar amount with semantics |
| `fiscal_year` | integer or null | Fiscal year the funds are available for |
| `availability` | string or null | Fund availability (e.g., `"to remain available until expended"`) |
| `provisos` | array of Proviso | "Provided, That" conditions |
| `earmarks` | array of Earmark | Community project funding items |
| `detail_level` | string | `"top_level"`, `"line_item"`, `"sub_allocation"`, or `"proviso_amount"` |
| `parent_account` | string or null | Parent account for sub-allocations |

**Budget authority:** Counted when `semantics == "new_budget_authority"` AND `detail_level` is `"top_level"` or `"line_item"`. Sub-allocations and proviso amounts are excluded to prevent double-counting.

**Example (from H.R. 9468):**

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
  "confidence": 0.99,
  "raw_text": "For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended."
}
```

**Count in example data:** 1,223 (49% of all provisions)

---

## rescission

Cancellation of previously appropriated funds.

**Bill text pattern:** *"...is hereby rescinded"* or *"Of the unobligated balances... $X is rescinded"*

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | Account being rescinded from |
| `agency` | string or null | Department or agency |
| `amount` | Amount | Dollar amount (semantics: `"rescission"`) |
| `reference_law` | string or null | The law whose funds are being rescinded |
| `fiscal_years` | string or null | Which fiscal years' funds are affected |

**Budget authority:** Summed separately and subtracted to produce Net BA.

**Example (from H.R. 4366):**

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
  "reference_law": "Fiscal Responsibility Act of 2023"
}
```

**Count in example data:** 78 (3.1%)

---

## cr_substitution

Continuing resolution anomaly — substitutes one dollar amount for another.

**Bill text pattern:** *"...shall be applied by substituting '$X' for '$Y'..."*

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string or null | Account affected (null if bill references a statute section) |
| `new_amount` | Amount | The new dollar amount ($X — the replacement level) |
| `old_amount` | Amount | The old dollar amount ($Y — the level being replaced) |
| `reference_act` | string | The act being modified |
| `reference_section` | string | Section being modified |

**Both** amounts are independently verified. The search table automatically shows **New**, **Old**, and **Delta** columns.

**Example (from H.R. 5860):**

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
  "section": "SEC. 101",
  "division": "A"
}
```

**Count in example data:** 13 (all in H.R. 5860)

---

## transfer_authority

Permission to move funds between accounts. The dollar amount is a **ceiling**, not new spending.

| Field | Type | Description |
|-------|------|-------------|
| `from_scope` | string | Source account(s) or scope |
| `to_scope` | string | Destination account(s) or scope |
| `limit` | TransferLimit | Transfer ceiling (percentage, fixed amount, or description) |
| `conditions` | array of strings | Conditions that must be met |

**Budget authority:** Not counted — `semantics: "transfer_ceiling"`.

**Count in example data:** 77 (all in H.R. 4366)

---

## limitation

Cap or prohibition on spending.

**Bill text pattern:** *"not more than $X"*, *"none of the funds"*, *"shall not exceed"*

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | What is being limited |
| `amount` | Amount or null | Dollar cap, if specified |
| `account_name` | string or null | Account the limitation applies to |
| `parent_account` | string or null | Parent account for proviso-based limitations |

**Budget authority:** Not counted — `semantics: "limitation"`.

**Count in example data:** 460 (18.4%)

---

## directed_spending

Earmark or community project funding directed to a specific recipient.

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | Account providing the funds |
| `amount` | Amount | Dollar amount directed |
| `earmark` | Earmark or null | `recipient`, `location`, `requesting_member` |
| `detail_level` | string | Typically `"sub_allocation"` or `"line_item"` |
| `parent_account` | string or null | Parent account name |

**Note:** Most earmarks are in the joint explanatory statement (a separate document), not the enrolled bill XML. Only earmarks in the bill text itself appear here.

**Count in example data:** 8 (all in H.R. 4366)

---

## mandatory_spending_extension

Amendment to an authorizing statute — extends, modifies, or reauthorizes mandatory programs.

| Field | Type | Description |
|-------|------|-------------|
| `program_name` | string | Program being extended |
| `statutory_reference` | string | The statute being amended (e.g., `"Section 330B(b)(2) of the Public Health Service Act"`) |
| `amount` | Amount or null | Dollar amount if specified |
| `period` | string or null | Duration of the extension |
| `extends_through` | string or null | End date or fiscal year |

**Count in example data:** 84 (40 in omnibus, 44 in CR)

---

## directive

Reporting requirement or instruction to an agency.

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | What is being directed |
| `deadlines` | array of strings | Any deadlines mentioned (e.g., `"30 days after enactment"`) |

**Budget authority:** None — directives don't carry dollar amounts.

**Example (from H.R. 9468):**

```json
{
  "provision_type": "directive",
  "description": "Requires the Inspector General of the Department of Veterans Affairs to conduct a review of the circumstances surrounding and underlying causes of the announced VBA funding shortfall for FY2024...",
  "deadlines": ["180 days after enactment"],
  "section": "SEC. 104"
}
```

**Count in example data:** 125

---

## rider

Policy provision that doesn't directly appropriate, rescind, or limit funds.

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | What the rider does |
| `policy_area` | string or null | Policy domain if identifiable |

**Budget authority:** None.

**Count in example data:** 336

---

## continuing_resolution_baseline

The core CR mechanism — usually SEC. 101 — establishing the default funding rule.

| Field | Type | Description |
|-------|------|-------------|
| `reference_year` | integer or null | Fiscal year used as the baseline rate |
| `reference_laws` | array of strings | Laws providing baseline funding levels |
| `rate` | string or null | Rate description (e.g., "the rate for operations") |
| `duration` | string or null | How long the CR lasts |
| `anomalies` | array of CrAnomaly | Explicit anomalies (usually captured as separate `cr_substitution` provisions) |

**Count in example data:** 1 (in H.R. 5860)

---

## other

Catch-all for provisions that don't fit any of the 10 specific types.

| Field | Type | Description |
|-------|------|-------------|
| `llm_classification` | string | The LLM's original description of what this provision is |
| `description` | string | Summary of the provision |
| `amounts` | array of Amount | Any dollar amounts mentioned |
| `references` | array of strings | Any references mentioned |
| `metadata` | object | Arbitrary key-value pairs for non-standard fields |

When the LLM produces an unknown `provision_type` string, the resilient parser wraps it as `Other` with the original classification preserved in `llm_classification`. In the example data, all 96 `other` provisions were deliberately classified as "other" by the LLM — none triggered the fallback parser.

**Count in example data:** 96 (3.8%)

---

## Amount Fields

Dollar amounts appear on many provision types. Each amount has three components:

### AmountValue (`value`)

| Kind | Fields | Description |
|------|--------|-------------|
| `specific` | `dollars` (integer) | Exact whole-dollar amount. Can be negative for rescissions. |
| `such_sums` | — | Open-ended: "such sums as may be necessary" |
| `none` | — | No dollar amount |

### Amount Semantics (`semantics`)

| Value | Meaning | Counted in Budget Authority? |
|-------|---------|------------------------------|
| `new_budget_authority` | New spending power | **Yes** (at top_level/line_item) |
| `rescission` | Cancellation of prior BA | Separately (subtracted for Net BA) |
| `reference_amount` | Contextual amount (sub-allocations, "of which" breakdowns) | **No** |
| `limitation` | Cap on spending | **No** |
| `transfer_ceiling` | Maximum transfer amount | **No** |
| `mandatory_spending` | Mandatory program amount | Tracked separately |

### Text As Written (`text_as_written`)

The verbatim dollar string from the bill text (e.g., `"$2,285,513,000"`). Used for verification — the string is searched for in the source XML to confirm the amount is real.

## Detail Levels (Appropriation Type Only)

| Level | Meaning | Counted in BA? |
|-------|---------|---------------|
| `top_level` | Main account appropriation | **Yes** |
| `line_item` | Numbered item within a section | **Yes** |
| `sub_allocation` | "Of which" breakdown | **No** |
| `proviso_amount` | Dollar amount in a "Provided, That" clause | **No** |
| `""` (empty) | Not applicable (non-appropriation types) | N/A |

## Proviso Fields

Conditions attached to appropriations via "Provided, That" clauses:

| Field | Type | Description |
|-------|------|-------------|
| `proviso_type` | string | `limitation`, `transfer`, `reporting`, `condition`, `prohibition`, `other` |
| `description` | string | Summary of the proviso |
| `amount` | Amount or null | Dollar amount if specified |
| `references` | array of strings | Referenced laws or sections |
| `raw_text` | string | Source text excerpt |

## Earmark Fields

Community project funding items:

| Field | Type | Description |
|-------|------|-------------|
| `recipient` | string | Who receives the funds |
| `location` | string or null | Geographic location |
| `requesting_member` | string or null | Member of Congress who requested it |

## Distribution in Example Data

| Type | H.R. 4366 (Omnibus) | H.R. 5860 (CR) | H.R. 9468 (Supp) | Total |
|------|:---:|:---:|:---:|:---:|
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

## Next Steps

- **[The Provision Type System](../explanation/provision-types.md)** — detailed explanations with real examples and analysis
- **[extraction.json Fields](./extraction-json.md)** — complete field reference for the full JSON structure
- **[Budget Authority Calculation](../explanation/budget-authority.md)** — how types and detail levels affect budget totals