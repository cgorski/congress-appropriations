# extraction.json Fields

Complete reference for every field in `extraction.json` — the primary output of the `extract` command and the file all query commands read.

## Top-Level Structure

```json
{
  "schema_version": "1.0",
  "bill": { ... },
  "provisions": [ ... ],
  "summary": { ... },
  "chunk_map": [ ... ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string or null | Schema version identifier (e.g., `"1.0"`). Null in pre-versioned extractions. |
| `bill` | BillInfo | Bill-level metadata |
| `provisions` | array of Provision | Every extracted provision — the core data |
| `summary` | ExtractionSummary | LLM-generated summary statistics. **Diagnostic only — never used for budget authority computation.** |
| `chunk_map` | array | Maps chunk IDs to provision index ranges for traceability. Empty for single-chunk bills. |

---

## BillInfo (`bill`)

| Field | Type | Description |
|-------|------|-------------|
| `identifier` | string | Bill number as printed (e.g., `"H.R. 9468"`, `"H.R. 4366"`) |
| `classification` | string | Bill type: `regular`, `continuing_resolution`, `omnibus`, `minibus`, `supplemental`, `rescissions`, or a free-text string |
| `short_title` | string or null | The bill's short title if one is given (e.g., `"Veterans Benefits Continuity and Accountability Supplemental Appropriations Act, 2024"`) |
| `fiscal_years` | array of integers | Fiscal years covered (e.g., `[2024]` or `[2024, 2025]`) |
| `divisions` | array of strings | Division letters present in the bill (e.g., `["A", "B", "C", "D", "E", "F"]`). Empty array if the bill has no divisions. |
| `public_law` | string or null | Public law number if enacted (e.g., `"P.L. 118-158"`). Null if not identified in the text. |

**Example (H.R. 9468):**

```json
{
  "identifier": "H.R. 9468",
  "classification": "supplemental",
  "short_title": "Veterans Benefits Continuity and Accountability Supplemental Appropriations Act, 2024",
  "fiscal_years": [2024],
  "divisions": [],
  "public_law": null
}
```

---

## Provisions (`provisions`)

An array of provision objects. Each provision has a `provision_type` field that determines which type-specific fields are present, plus the common fields shared by all types.

See [Provision Types](./provision-types.md) for the complete type-by-type reference including type-specific fields and examples.

### Common Fields (All Provision Types)

| Field | Type | Description |
|-------|------|-------------|
| `provision_type` | string | Type discriminator: `appropriation`, `rescission`, `cr_substitution`, `transfer_authority`, `limitation`, `directed_spending`, `mandatory_spending_extension`, `directive`, `rider`, `continuing_resolution_baseline`, `other` |
| `section` | string | Section header (e.g., `"SEC. 101"`). Empty string if no section header applies. |
| `division` | string or null | Division letter (e.g., `"A"`). Null if the bill has no divisions. |
| `title` | string or null | Title numeral (e.g., `"IV"`, `"XIII"`). Null if not determinable. |
| `confidence` | float | LLM self-assessed confidence, 0.0–1.0. **Not calibrated.** Useful only for identifying outliers below 0.90. |
| `raw_text` | string | Verbatim excerpt from the bill text (~first 150 characters of the provision). Verified against source. |
| `notes` | array of strings | Explanatory annotations. Flags unusual patterns, drafting inconsistencies, or contextual information (e.g., `"advance appropriation"`, `"no-year funding"`, `"supplemental appropriation"`). |
| `cross_references` | array of CrossReference | References to other laws, sections, or bills. |

### CrossReference

| Field | Type | Description |
|-------|------|-------------|
| `ref_type` | string | Relationship type: `baseline_from`, `amends`, `notwithstanding`, `subject_to`, `see_also`, `transfer_to`, `rescinds_from`, `modifies`, `references`, `other` |
| `target` | string | The referenced law or section (e.g., `"31 U.S.C. 1105(a)"`, `"P.L. 118-47, Division A"`) |
| `description` | string or null | Optional clarifying note |

---

## Amount

Dollar amounts appear throughout the schema — on `appropriation`, `rescission`, `limitation`, `directed_spending`, `mandatory_spending_extension`, and `other` provision types. CR substitutions have `new_amount` and `old_amount` instead of a single `amount`.

Each amount has three sub-fields:

### AmountValue (`value`)

Tagged by the `kind` field:

| Kind | Fields | Description |
|------|--------|-------------|
| `specific` | `dollars` (integer) | An exact dollar amount. Always whole dollars, no cents. Can be negative for rescissions. Example: `{"kind": "specific", "dollars": 2285513000}` |
| `such_sums` | — | Open-ended: "such sums as may be necessary." No dollar figure. Example: `{"kind": "such_sums"}` |
| `none` | — | No dollar amount — the provision doesn't carry a dollar value. Example: `{"kind": "none"}` |

### Amount Semantics (`semantics`)

| Value | Meaning | Counted in Budget Authority? |
|-------|---------|------------------------------|
| `new_budget_authority` | New spending power granted to an agency | **Yes** (at top_level/line_item detail) |
| `rescission` | Cancellation of prior budget authority | Summed separately as rescissions |
| `reference_amount` | Dollar figure for context (sub-allocations, "of which" breakdowns) | **No** |
| `limitation` | Cap on how much may be spent for a purpose | **No** |
| `transfer_ceiling` | Maximum amount transferable between accounts | **No** |
| `mandatory_spending` | Mandatory spending referenced or extended | Tracked separately |
| Other string | Catch-all for unrecognized semantics | **No** |

### Text As Written (`text_as_written`)

The verbatim dollar string from the bill text (e.g., `"$2,285,513,000"`). Used by the verification pipeline — this exact string is searched for in the source XML.

### Complete Amount Example

```json
{
  "value": {
    "kind": "specific",
    "dollars": 2285513000
  },
  "semantics": "new_budget_authority",
  "text_as_written": "$2,285,513,000"
}
```

---

## Detail Level (Appropriation Type Only)

The `detail_level` field on appropriation provisions indicates structural position in the funding hierarchy:

| Level | Meaning | Counted in BA? | Example |
|-------|---------|---------------|---------|
| `top_level` | Main account appropriation | **Yes** | `"$10,643,713,000"` for FBI Salaries and Expenses |
| `line_item` | Numbered item within a section | **Yes** | `"(1) $3,500,000,000 for guaranteed farm ownership loans"` |
| `sub_allocation` | "Of which" breakdown | **No** | `"of which $216,900,000 shall remain available until expended"` |
| `proviso_amount` | Dollar amount in a "Provided, That" clause | **No** | `"Provided, That not to exceed $279,000 for reception expenses"` |
| `""` (empty) | Not applicable (non-appropriation provision types) | N/A | Directives, riders, etc. |

The `compute_totals()` function uses `detail_level` to prevent double-counting. Sub-allocations and proviso amounts are breakdowns of a parent appropriation, not additional money.

---

## Proviso

Conditions attached to appropriations via "Provided, That" clauses:

| Field | Type | Description |
|-------|------|-------------|
| `proviso_type` | string | `limitation`, `transfer`, `reporting`, `condition`, `prohibition`, `other` |
| `description` | string | Summary of the proviso |
| `amount` | Amount or null | Dollar amount if the proviso specifies one |
| `references` | array of strings | Referenced laws or sections |
| `raw_text` | string | Source text excerpt |

---

## Earmark

Community project funding or directed spending items:

| Field | Type | Description |
|-------|------|-------------|
| `recipient` | string | Who receives the funds |
| `location` | string or null | Geographic location |
| `requesting_member` | string or null | Member of Congress who requested it |

---

## CrAnomaly

Anomaly entries within a `continuing_resolution_baseline` provision:

| Field | Type | Description |
|-------|------|-------------|
| `account` | string | Account being modified |
| `modification` | string | What's changing |
| `delta` | integer or null | Dollar change if applicable |
| `raw_text` | string | Source text excerpt |

---

## ExtractionSummary (`summary`)

LLM-produced self-check totals. **These are diagnostic only — budget authority displayed by the `summary` command is always computed from individual provisions, never from these fields.**

| Field | Type | Description |
|-------|------|-------------|
| `total_provisions` | integer | Count of all provisions the LLM reported extracting |
| `by_division` | object | Provision count per division (e.g., `{"A": 130, "B": 10}`) |
| `by_type` | object | Provision count per type (e.g., `{"appropriation": 2, "rider": 2}`) |
| `total_budget_authority` | integer | LLM's self-reported sum of budget authority. **Not used for computation.** |
| `total_rescissions` | integer | LLM's self-reported sum of rescissions. **Not used for computation.** |
| `sections_with_no_provisions` | array of strings | Section headers where no provision was extracted — helps verify completeness |
| `flagged_issues` | array of strings | Anything unusual the LLM noticed: drafting inconsistencies, ambiguous language, potential errors |

---

## Chunk Map (`chunk_map`)

Links provisions to the extraction chunks they came from. For single-chunk bills (like H.R. 9468), this is an empty array. For multi-chunk bills, each entry maps a chunk ID (ULID) to a range of provision indices:

```json
[
  {
    "chunk_id": "01JRWN9T5RR0JTQ6C9FYYE96A8",
    "label": "A-I",
    "provision_start": 0,
    "provision_end": 42
  },
  {
    "chunk_id": "01JRWNA2B3C4D5E6F7G8H9J0K1",
    "label": "A-II",
    "provision_start": 42,
    "provision_end": 95
  }
]
```

This enables full audit trails — you can trace any provision back to the specific chunk and LLM call that produced it.

---

## Complete Minimal Example (H.R. 9468)

```json
{
  "schema_version": "1.0",
  "bill": {
    "identifier": "H.R. 9468",
    "classification": "supplemental",
    "short_title": "Veterans Benefits Continuity and Accountability Supplemental Appropriations Act, 2024",
    "fiscal_years": [2024],
    "divisions": [],
    "public_law": null
  },
  "provisions": [
    {
      "provision_type": "appropriation",
      "account_name": "Compensation and Pensions",
      "agency": "Department of Veterans Affairs",
      "program": null,
      "amount": {
        "value": { "kind": "specific", "dollars": 2285513000 },
        "semantics": "new_budget_authority",
        "text_as_written": "$2,285,513,000"
      },
      "fiscal_year": 2024,
      "availability": "to remain available until expended",
      "provisos": [],
      "earmarks": [],
      "detail_level": "top_level",
      "parent_account": null,
      "section": "",
      "division": null,
      "title": null,
      "confidence": 0.99,
      "raw_text": "For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.",
      "notes": [
        "Supplemental appropriation under Veterans Benefits Administration heading",
        "No-year funding"
      ],
      "cross_references": []
    },
    {
      "provision_type": "appropriation",
      "account_name": "Readjustment Benefits",
      "agency": "Department of Veterans Affairs",
      "program": null,
      "amount": {
        "value": { "kind": "specific", "dollars": 596969000 },
        "semantics": "new_budget_authority",
        "text_as_written": "$596,969,000"
      },
      "fiscal_year": 2024,
      "availability": "to remain available until expended",
      "provisos": [],
      "earmarks": [],
      "detail_level": "top_level",
      "parent_account": null,
      "section": "",
      "division": null,
      "title": null,
      "confidence": 0.99,
      "raw_text": "For an additional amount for ''Readjustment Benefits'', $596,969,000, to remain available until expended.",
      "notes": [
        "Supplemental appropriation under Veterans Benefits Administration heading",
        "No-year funding"
      ],
      "cross_references": []
    },
    {
      "provision_type": "rider",
      "description": "Establishes that each amount appropriated or made available by this Act is in addition to amounts otherwise appropriated for the fiscal year involved.",
      "policy_area": null,
      "section": "SEC. 101",
      "division": null,
      "title": null,
      "confidence": 0.98,
      "raw_text": "SEC. 101. Each amount appropriated or made available by this Act is in addition to amounts otherwise appropriated for the fiscal year involved.",
      "notes": [],
      "cross_references": []
    },
    {
      "provision_type": "directive",
      "description": "Requires the Secretary of Veterans Affairs to submit a report detailing corrections the Department will make to improve forecasting, data quality, and budget assumptions.",
      "deadlines": ["30 days after enactment"],
      "section": "SEC. 103",
      "division": null,
      "title": null,
      "confidence": 0.97,
      "raw_text": "SEC. 103. (a) Not later than 30 days after the date of enactment of this Act, the Secretary of Veterans Affairs shall submit to the Committees on App",
      "notes": [],
      "cross_references": []
    }
  ],
  "summary": {
    "total_provisions": 7,
    "by_division": {},
    "by_type": {
      "appropriation": 2,
      "rider": 2,
      "directive": 3
    },
    "total_budget_authority": 2882482000,
    "total_rescissions": 0,
    "sections_with_no_provisions": [],
    "flagged_issues": []
  },
  "chunk_map": []
}
```

> **Note:** The example above is abbreviated — the actual H.R. 9468 extraction has 7 provisions (2 appropriations, 2 riders, 3 directives). Only 4 are shown here for brevity.

---

## Accessing extraction.json

### From the CLI

All query commands (`search`, `summary`, `compare`, `audit`) read `extraction.json` automatically. You don't need to interact with the file directly for normal use.

### From Python

```python
import json

with open("examples/hr9468/extraction.json") as f:
    data = json.load(f)

# Bill info
print(data["bill"]["identifier"])  # "H.R. 9468"

# Provisions
for p in data["provisions"]:
    ptype = p["provision_type"]
    if ptype == "appropriation":
        dollars = p["amount"]["value"]["dollars"]
        account = p["account_name"]
        print(f"{account}: ${dollars:,}")
```

### From Rust (Library API)

```rust
use congress_appropriations::load_bills;
use std::path::Path;

let bills = load_bills(Path::new("examples"))?;
for bill in &bills {
    println!("{}: {} provisions",
        bill.extraction.bill.identifier,
        bill.extraction.provisions.len());
}
```

See [Use the Library API from Rust](../how-to/library-api.md) for the full guide.

---

## Schema Versioning

The `schema_version` field tracks the extraction data format. When the schema evolves (new fields, renamed fields), the `upgrade` command migrates existing data to the latest version without re-extraction.

| Version | Description |
|---------|-------------|
| `null` | Pre-versioned data (before v1.1.0) |
| `"1.0"` | Current schema with all documented fields |

The `upgrade` command adds `schema_version` to pre-versioned files and applies any necessary field migrations. See [Upgrade Extraction Data](../how-to/upgrade-data.md).

---

## Related References

- **[Provision Types](./provision-types.md)** — type-by-type field reference with examples
- **[verification.json Fields](./verification-json.md)** — the verification report that accompanies each extraction
- **[embeddings.json Fields](./embeddings-json.md)** — embedding metadata
- **[Data Directory Layout](./data-directory.md)** — where extraction.json fits in the file hierarchy