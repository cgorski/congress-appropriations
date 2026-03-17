# Field Reference

Complete reference for every field in `extraction.json` and `verification.json`. These files are produced by the `congress-approp extract` command.

---

## extraction.json

The top-level extraction output for a single bill.

### Bill Info (`bill`)

| Field | Type | Description |
|-------|------|-------------|
| `identifier` | string | Bill number as printed, e.g. `"H.R. 9468"` |
| `classification` | string | One of: `regular`, `continuing_resolution`, `omnibus`, `minibus`, `supplemental`, `rescissions`, or a free-text string |
| `short_title` | string or null | The bill's short title if one is given, e.g. `"Veterans Benefits Continuity and Accountability Supplemental Appropriations Act, 2024"` |
| `fiscal_years` | array of integers | Fiscal years covered, e.g. `[2024]` or `[2024, 2025]` |
| `divisions` | array of strings | Division letters present in the bill, e.g. `["A", "B", "C"]`. Empty array if the bill has no divisions |
| `public_law` | string or null | Public law number if enacted, e.g. `"P.L. 118-158"`. Null if not identified in the text |

### Provisions (`provisions`)

An array of provision objects. Each provision has a `provision_type` field that determines which type-specific fields are present, plus a set of common fields shared by all types.

#### Common Fields (all provision types)

| Field | Type | Description |
|-------|------|-------------|
| `provision_type` | string | Discriminator. One of the types listed below |
| `section` | string | Section header as written, e.g. `"SEC. 101"`. Empty string if no section header applies |
| `division` | string or null | Division letter, e.g. `"A"`. Null if the bill has no divisions |
| `title` | string or null | Title numeral, e.g. `"IV"`, `"XIII"`. Null if not determinable |
| `confidence` | float | LLM self-assessed confidence, 0.0–1.0. **Not calibrated.** Useful only for identifying outliers (< 0.90). Values above 0.90 are not meaningfully differentiated. |
| `raw_text` | string | Verbatim excerpt from the bill text (~first 150 characters of the provision). Verified against the source text |
| `notes` | array of strings | Explanatory annotations. Flags unusual patterns, drafting inconsistencies, or contextual information |
| `cross_references` | array of CrossReference | References to other laws, sections, or bills (see below) |

#### CrossReference

| Field | Type | Description |
|-------|------|-------------|
| `ref_type` | string | Relationship type: `baseline_from`, `amends`, `notwithstanding`, `subject_to`, `see_also`, `transfer_to`, `rescinds_from`, `modifies`, `references`, or `other` |
| `target` | string | The referenced law or section, e.g. `"31 U.S.C. 1105(a)"` or `"P.L. 118-47, Division A"` |
| `description` | string or null | Optional clarifying note |

---

## Provision Types

### `appropriation`

A grant of budget authority — the core spending provision.

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | Exact account name from between `''` delimiters in the bill text |
| `agency` | string or null | Department or agency that owns the account, e.g. `"Department of Veterans Affairs"` |
| `program` | string or null | Sub-account or program name if specified |
| `amount` | Amount | Dollar amount with semantics (see Amount section below) |
| `fiscal_year` | integer or null | Fiscal year the funds are available for |
| `availability` | string or null | Fund availability period, e.g. `"to remain available until expended"` or `"to remain available until September 30, 2026"` |
| `provisos` | array of Proviso | Conditions from "Provided, That" clauses (see below) |
| `earmarks` | array of Earmark | Community project funding items (see below) |
| `detail_level` | string | Granularity: `"top_level"`, `"line_item"`, `"sub_allocation"`, `"proviso_amount"`, or `""` |
| `parent_account` | string or null | For sub-allocations, the parent account name |

Note: Sub-allocations (`detail_level: "sub_allocation"`) should use `semantics: "reference_amount"` and are excluded from budget authority totals.

### `rescission`

Cancellation of previously appropriated funds.

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | Account being rescinded from |
| `agency` | string or null | Department or agency |
| `amount` | Amount | Dollar amount being rescinded (semantics will be `rescission`) |
| `reference_law` | string or null | The law whose funds are being rescinded, e.g. `"P.L. 117-328"` |
| `fiscal_years` | string or null | Which fiscal years' funds are affected |

### `transfer_authority`

Permission to move funds between accounts. The dollar amount is a **ceiling**, not new spending.

| Field | Type | Description |
|-------|------|-------------|
| `from_scope` | string | Source account(s) or scope |
| `to_scope` | string | Destination account(s) or scope |
| `limit` | string or null | Transfer limit description |
| `conditions` | array of strings | Conditions that must be met for the transfer |

### `limitation`

A cap or prohibition on spending.

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | What is being limited |
| `amount` | Amount or null | Dollar cap, if one is specified |
| `account_name` | string or null | Account the limitation applies to |
| `parent_account` | string or null | Parent account for proviso-based limitations |

### `directed_spending`

Earmark or community project funding directed to a specific recipient.

| Field | Type | Description |
|-------|------|-------------|
| `account_name` | string | Account providing the funds |
| `amount` | Amount | Dollar amount directed |
| `earmark` | Earmark or null | Recipient details (see below) |
| `detail_level` | string | Typically `"sub_allocation"` or `"line_item"` |
| `parent_account` | string or null | Parent account name |

### `cr_substitution`

A continuing resolution anomaly that substitutes one dollar amount for another.

| Field | Type | Description |
|-------|------|-------------|
| `reference_act` | string | The act being modified |
| `reference_section` | string | Section being modified |
| `new_amount` | Amount | The new dollar amount (X in "substituting X for Y") |
| `old_amount` | Amount | The old dollar amount being replaced (Y) |
| `account_name` | string or null | Account affected |

### `mandatory_spending_extension`

Amendment to authorizing statute — common in Division B of omnibus bills.

| Field | Type | Description |
|-------|------|-------------|
| `program_name` | string | Program being extended |
| `statutory_reference` | string | The statute being amended, e.g. `"Section 330B(b)(2) of the Public Health Service Act"` |
| `amount` | Amount or null | Dollar amount if specified |
| `period` | string or null | Duration of the extension |
| `extends_through` | string or null | End date or fiscal year |

### `directive`

A reporting requirement or instruction to an agency.

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | What is being directed |
| `deadlines` | array of strings | Any deadlines mentioned (e.g. `"30 days after enactment"`) |

### `rider`

A policy provision that doesn't directly appropriate, rescind, or limit funds.

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | What the rider does |
| `policy_area` | string or null | Policy domain if identifiable |

### `continuing_resolution_baseline`

The core CR mechanism — usually SEC. 101 or equivalent.

| Field | Type | Description |
|-------|------|-------------|
| `reference_year` | integer or null | The fiscal year being used as the baseline rate |
| `reference_laws` | array of strings | Laws providing the baseline funding levels |
| `rate` | string or null | The rate description (e.g. "the rate for operations") |
| `duration` | string or null | How long the CR lasts |
| `anomalies` | array of CrAnomaly | Explicit anomalies modifying specific accounts |

#### CrAnomaly

| Field | Type | Description |
|-------|------|-------------|
| `account` | string | Account being modified |
| `modification` | string | What's changing |
| `delta` | integer or null | Dollar change if applicable |
| `raw_text` | string | Source text excerpt |

### `other`

Catch-all for provisions that don't fit other types.

| Field | Type | Description |
|-------|------|-------------|
| `llm_classification` | string | The model's description of what this provision is |
| `description` | string | Summary of the provision |
| `amounts` | array of Amount | Any dollar amounts mentioned |
| `references` | array of strings | Any references mentioned |
| `metadata` | object | Arbitrary key-value pairs |

---

## Amount

Dollar amounts appear throughout the schema. Each has three sub-fields.

| Field | Type | Description |
|-------|------|-------------|
| `value` | AmountValue | The actual dollar figure (see below) |
| `semantics` | string | What the amount represents in budget terms |
| `text_as_written` | string | Verbatim dollar string from the bill, e.g. `"$2,285,513,000"`. Used for verification |

### AmountValue (`value`)

Tagged by the `kind` field:

| Kind | Fields | Description |
|------|--------|-------------|
| `specific` | `dollars` (integer) | An exact dollar amount. Always whole dollars, no cents. Can be negative for rescissions. Example: `{"kind": "specific", "dollars": 2285513000}` |
| `such_sums` | — | Open-ended: "such sums as may be necessary." No dollar figure |
| `none` | — | No dollar amount — the provision doesn't carry a dollar value (directives, riders) |

### Amount Semantics (`semantics`)

| Value | Meaning |
|-------|---------|
| `new_budget_authority` | New spending power granted to an agency. **This is what counts toward total budget authority.** |
| `transfer_ceiling` | Maximum amount that may be transferred between accounts. Not new spending |
| `rescission` | Cancellation of prior budget authority |
| `limitation` | A cap on how much of an appropriation may be spent for a purpose |
| `reference_amount` | A dollar figure mentioned for context but not itself an appropriation (e.g. "of which not less than $X") |
| `mandatory_spending` | Mandatory spending referenced or extended in the bill |

---

## Proviso

Conditions attached to appropriations via "Provided, That" clauses.

| Field | Type | Description |
|-------|------|-------------|
| `proviso_type` | string | One of: `limitation`, `transfer`, `reporting`, `condition`, `prohibition`, `other` |
| `description` | string | Summary of the proviso |
| `amount` | Amount or null | Dollar amount if the proviso specifies one |
| `references` | array of strings | Referenced laws or sections |
| `raw_text` | string | Source text excerpt |

## Earmark

Community project funding or directed spending items.

| Field | Type | Description |
|-------|------|-------------|
| `recipient` | string | Who receives the funds |
| `location` | string or null | Geographic location |
| `requesting_member` | string or null | Member of Congress who requested the earmark |

---

## Summary (`summary`)

LLM-produced self-check totals. Useful for quick overview but should be verified against the provisions array.

| Field | Type | Description |
|-------|------|-------------|
| `total_provisions` | integer | Count of all provisions extracted |
| `by_division` | object | Provision count per division, e.g. `{"A": 130, "B": 10}` |
| `by_type` | object | Provision count per type, e.g. `{"appropriation": 2, "rider": 2, "directive": 2}` |
| `total_budget_authority` | integer | Sum of all amounts with `new_budget_authority` semantics. Does NOT include transfer ceilings or reference amounts |
| `total_rescissions` | integer | Sum of all amounts with `rescission` semantics |
| `sections_with_no_provisions` | array of strings | Section headers where no provision was extracted — helps verify completeness |
| `flagged_issues` | array of strings | Anything unusual the model noticed: drafting inconsistencies, ambiguous language, potential errors |
| `chunk_map` | array | Maps chunk IDs to provision indices for traceability (empty for single-chunk bills) |

---

## verification.json

Deterministic verification of extracted provisions against the source bill text. No LLM involved — this is pure string matching and arithmetic.

### Amount Checks (`amount_checks`)

One entry per provision that has a dollar amount.

| Field | Type | Description |
|-------|------|-------------|
| `provision_index` | integer | Index into the `provisions` array (0-based) |
| `text_as_written` | string | The dollar string being checked, e.g. `"$2,285,513,000"` |
| `found_in_source` | boolean | Whether the string was found in the source text |
| `source_positions` | array of integers | Character offset(s) where found |
| `status` | string | `verified` (found exactly once or in source), `not_found`, `ambiguous` (found multiple times), or `mismatch` |

### Raw Text Checks (`raw_text_checks`)

One entry per provision, checking that `raw_text` is a substring of the source.

| Field | Type | Description |
|-------|------|-------------|
| `provision_index` | integer | Index into the `provisions` array |
| `raw_text_preview` | string | First ~80 characters of the raw text being checked |
| `is_verbatim_substring` | boolean | True only for `exact` tier matches |
| `match_tier` | string | How closely it matched (see tiers below) |
| `found_at_position` | integer or null | Character offset if exact match; null otherwise |

#### Match Tiers

| Tier | Description |
|------|-------------|
| `exact` | Byte-for-byte substring match in the source text |
| `normalized` | Matches after collapsing whitespace and normalizing curly quotes (`"` → `"`) and dashes (`—` → `-`) |
| `spaceless` | Matches after removing all spaces. Catches text artifacts where words are joined |
| `no_match` | Not found at any tier. The raw text may be paraphrased rather than verbatim |

### Arithmetic Checks (`arithmetic_checks`)

Group-level sum verification (e.g., do line items sum to the stated total for a title).

| Field | Type | Description |
|-------|------|-------------|
| `scope` | string | What's being summed (e.g. a title or division) |
| `extracted_sum` | integer | Sum of extracted provisions in this scope |
| `stated_total` | integer or null | Total stated in the bill, if any |
| `status` | string | `verified`, `not_found`, `mismatch`, or `no_reference` |

### Completeness (`completeness`)

Checks whether every dollar amount in the source text is accounted for by an extracted provision.

| Field | Type | Description |
|-------|------|-------------|
| `total_dollar_amounts_in_text` | integer | How many dollar amounts the text index found in the bill |
| `accounted_for` | integer | How many are matched to an extracted provision |
| `unaccounted` | array of objects | Dollar amounts in the bill that no provision captured |

Each unaccounted entry:

| Field | Type | Description |
|-------|------|-------------|
| `text` | string | The dollar string, e.g. `"$500,000"` |
| `value` | integer | Parsed dollar value |
| `position` | integer | Character offset in the source text |
| `context` | string | Surrounding text for identification |

### Verification Summary (`summary`)

Roll-up metrics for the entire bill.

| Field | Type | Description |
|-------|------|-------------|
| `total_provisions` | integer | Total provisions checked |
| `amounts_verified` | integer | Provisions whose dollar amount was found in source (found at exactly one position) |
| `amounts_not_found` | integer | Provisions whose dollar amount was NOT found (not present in source text) |
| `amounts_ambiguous` | integer | Provisions whose dollar amount appeared multiple times (found at multiple positions) |
| `raw_text_exact` | integer | Provisions with exact raw text match |
| `raw_text_normalized` | integer | Provisions with normalized match |
| `raw_text_spaceless` | integer | Provisions with spaceless match |
| `raw_text_no_match` | integer | Provisions with no raw text match |
| `completeness_pct` | float | Percentage of source dollar amounts accounted for (100.0 = all captured) |
| `provisions_by_detail_level` | object | Count of provisions at each detail level |