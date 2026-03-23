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

---

## bill_meta.json

Bill-level metadata generated by the `enrich` command. This file is optional — all commands from v3.x work without it. It is required for `--subcommittee` filtering, `--show-advance` display, and enriched bill classification display.

### Top-Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Always `"1.0"` for this format |
| `congress` | integer or null | Congress number parsed from the XML filename (e.g., `119` from `BILLS-119hr7148enr.xml`). Null if the filename doesn't match the expected pattern |
| `fiscal_years` | array of integers | Fiscal years this bill covers, copied from `extraction.json` `bill.fiscal_years` |
| `bill_nature` | string | Enriched bill classification. One of: `regular`, `omnibus`, `minibus`, `continuing_resolution`, `full_year_cr_with_appropriations`, `supplemental`, `authorization`, or a free-text string |
| `subcommittees` | array of SubcommitteeMapping | Division letter → jurisdiction mappings (see below) |
| `provision_timing` | array of ProvisionTiming | Advance/current/supplemental classification per BA provision (see below) |
| `canonical_accounts` | array of CanonicalAccount | Normalized account names per provision (see below) |
| `extraction_sha256` | string | SHA-256 hash of `extraction.json` at the time of enrichment. Part of the hash chain for staleness detection |

### SubcommitteeMapping

| Field | Type | Description |
|-------|------|-------------|
| `division` | string | Division letter (e.g., `"A"`, `"B"`) |
| `jurisdiction` | string | Canonical jurisdiction slug. One of: `defense`, `labor_hhs`, `thud`, `financial_services`, `cjs`, `energy_water`, `interior`, `agriculture`, `legislative_branch`, `milcon_va`, `state_foreign_ops`, `homeland_security`, `continuing_resolution`, `extenders`, `policy`, `budget_process`, `other` |
| `title` | string | Raw division title from the XML (e.g., `"DEPARTMENT OF DEFENSE APPROPRIATIONS ACT, 2026"`) |
| `source` | ClassificationSource | How this jurisdiction was determined (see below) |

### ProvisionTiming

| Field | Type | Description |
|-------|------|-------------|
| `provision_index` | integer | Index into the `provisions` array in `extraction.json` (0-based) |
| `timing` | string | One of: `current_year`, `advance`, `supplemental`, `unknown` |
| `available_fy` | integer or null | The fiscal year the money becomes available, if determined. For advance appropriations, this is the future FY |
| `source` | ClassificationSource | How this timing was determined (see below) |

### CanonicalAccount

| Field | Type | Description |
|-------|------|-------------|
| `provision_index` | integer | Index into the `provisions` array (0-based) |
| `canonical_name` | string | Normalized account name: lowercased, em-dash/en-dash prefix stripped, whitespace trimmed. E.g., `"Department of VA—Compensation and Pensions"` becomes `"compensation and pensions"` |

### ClassificationSource

A tagged object describing how a classification was determined. Provides provenance for every automated decision.

| Variant (`type` field) | Additional Fields | Description |
|------------------------|-------------------|-------------|
| `xml_structure` | — | Parsed directly from XML element structure |
| `pattern_match` | `pattern` (string) | Matched against a known text pattern (e.g., `"department of defense"`) |
| `fiscal_year_comparison` | `availability_fy` (integer), `bill_fy` (integer) | Determined by comparing the provision's availability FY to the bill's FY |
| `note_text` | — | Classified based on the provision's `notes` array content |
| `default_rule` | — | No specific signal found; applied the default classification |
| `llm_classification` | `model` (string), `confidence` (float) | Classified by an LLM (future feature) |
| `manual` | — | Manually overridden by a user |

---

## links/links.json

Persistent cross-bill provision links stored at the data root directory (not inside any bill directory). Created by `link accept` and consumed by `compare --use-links` and `link list`.

### Top-Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Always `"1.0"` |
| `embedding_model` | string | The embedding model used when links were created (e.g., `"text-embedding-3-large"`). Links are model-specific — re-embedding with a different model invalidates link hashes |
| `accepted` | array of AcceptedLink | All accepted links (see below) |

### AcceptedLink

| Field | Type | Description |
|-------|------|-------------|
| `hash` | string | Deterministic 8-character hex hash computed from source provision, target provision, and embedding model. Same inputs always produce the same hash |
| `source` | ProvisionRef | The source provision (see below) |
| `target` | ProvisionRef | The target provision (see below) |
| `similarity` | float | Cosine similarity between the two provisions' embedding vectors at the time the link was created |
| `relationship` | string | One of: `same_account` (verified name match), `renamed` (different name, same program), `reclassified` (different semantics), `related` (similar but unverified) |
| `evidence` | object | How the link was established. Tagged by `type` field: `name_match`, `high_similarity`, or `manual` |
| `accepted_at` | string | ISO 8601 timestamp of when the link was accepted |
| `note` | string or null | Optional user annotation (e.g., `"Account renamed from X to Y"`) |

### ProvisionRef

| Field | Type | Description |
|-------|------|-------------|
| `bill_dir` | string | Bill directory name (e.g., `"118-hr4366"`, `"119-hr7148"`) |
| `provision_index` | integer | Index into the bill's `provisions` array in `extraction.json` (0-based) |
| `label` | string | Human-readable label (typically the account name) |

---

## dataset.json

User-managed entity resolution rules stored at the data root directory (alongside bill directories). Created by `normalize accept` and consumed by `compare`, `relate`, and `link suggest`. Contains only knowledge that cannot be derived from scanning per-bill files.

### Top-Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Always `"1.0"` |
| `entities` | object | Entity resolution rules (see below) |

### entities.agency_groups

An array of agency equivalence groups. During matching, if a provision's agency matches the canonical OR any member of a group, it is normalized to the canonical name.

| Field | Type | Description |
|-------|------|-------------|
| `canonical` | string | The preferred agency name shown in output |
| `members` | array of strings | Variant names treated as equivalent to the canonical name |

### entities.account_aliases

An array of account name equivalences.

| Field | Type | Description |
|-------|------|-------------|
| `canonical` | string | The preferred account name |
| `aliases` | array of strings | Variant spellings treated as equivalent to the canonical name |

### File Lifecycle

- **Created by:** `normalize accept` (from cached `suggest-text-match` or `suggest-llm` results)
- **Also editable by hand** — simple JSON format
- **Read by:** `compare`, `relate`, `link suggest`
- **Ignored by:** `compare --exact`
- **Never auto-generated or overwritten** by `enrich`, `extract`, `embed`, or any other command. Contains only user decisions.
- **Partially superseded by:** `resolve-tas` + `authority build` for cross-bill account matching. The `dataset.json` entity resolution still applies to `compare` output formatting; TAS-based matching via `--use-authorities` is more accurate for account identity.

---

## source_span (inline field on provisions in extraction.json)

Added by the `verify-text --repair` command. Present on each provision object in `extraction.json` after the verify-text pipeline stage has run. Records the exact byte position of the provision's `raw_text` in the enrolled bill source text.

| Field | Type | Description |
|-------|------|-------------|
| `start` | integer | Start byte offset in the source `.txt` file (inclusive). **UTF-8 byte offset**, not character offset. Matches Rust's native `str` indexing. |
| `end` | integer | End byte offset in the source `.txt` file (exclusive). UTF-8 byte offset. |
| `file` | string | Source filename, e.g., `"BILLS-118hr2882enr.txt"` |
| `verified` | boolean | `true` if `source_bytes[start..end]` is byte-identical to `raw_text` |
| `match_tier` | string | How the span was established: `"exact"`, `"repaired_prefix"`, `"repaired_substring"`, or `"repaired_normalized"` |

### Invariant

When `verified` is `true`:

```
source_file_bytes[start .. end] == provision.raw_text
```

**Important:** `start` and `end` are UTF-8 byte offsets. Languages that use character-based indexing (Python `str`, JavaScript) must use byte-level slicing:

```python
raw_bytes = open("BILLS-118hr2882enr.txt", "rb").read()
actual = raw_bytes[span["start"]:span["end"]].decode("utf-8")
assert actual == provision["raw_text"]
```

### File Lifecycle

- **Created by:** `verify-text --repair`
- **Read by:** any consumer of `extraction.json` (the field is on each provision object)
- **Ignored by:** Rust's typed `Provision` enum (Serde skips unknown fields). The Rust `verify-text` command works at the `serde_json::Value` level.
- **Invalidated when:** the bill is re-extracted (`extract --force`). Re-run `verify-text --repair` after re-extraction.

---

## tas_mapping.json

Per-bill Treasury Account Symbol mapping. Created by `resolve-tas` and consumed by `authority build` and `compare --use-authorities`. Maps each top-level budget authority appropriation to a Federal Account Symbol (FAS) code.

### Top-Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Always `"1.0"` |
| `bill_dir` | string | Bill directory name (e.g., `"118-hr2882"`) |
| `bill_identifier` | string | Bill identifier (e.g., `"H.R. 2882"`) |
| `model` | string or null | LLM model used for Tier 2 matching (e.g., `"claude-opus-4-6"`), or `null` if all deterministic |
| `fas_reference_hash` | string | SHA-256 of `fas_reference.json` used during this resolution. For staleness detection. |
| `timestamp` | string | ISO 8601 timestamp of when the resolution was performed |
| `mappings` | array of TasMapping | One entry per top-level BA appropriation provision |
| `summary` | TasSummary | Aggregate statistics |

### TasMapping

| Field | Type | Description |
|-------|------|-------------|
| `provision_index` | integer | Index into the bill's `provisions` array in `extraction.json` (0-based) |
| `account_name` | string | Account name as extracted by the LLM |
| `agency` | string | Agency name as extracted by the LLM |
| `dollars` | integer or null | Dollar amount (if available) |
| `fas_code` | string or null | Matched Federal Account Symbol (e.g., `"070-0400"`), or `null` if unmatched |
| `fas_title` | string or null | Official FAST Book title for the matched FAS code |
| `confidence` | string | `"verified"` (deterministic match), `"high"` (LLM match confirmed in FAST Book), `"inferred"` (LLM match not in FAST Book), or `"unmatched"` |
| `method` | string | `"direct_match"`, `"suffix_match"`, `"agency_disambiguated"`, `"llm_resolved"`, or `"none"` |
| `reasoning` | string or null | LLM reasoning (only populated for `llm_resolved` matches) |

### TasSummary

| Field | Type | Description |
|-------|------|-------------|
| `total_provisions` | integer | Total top-level BA provisions considered |
| `deterministic_matched` | integer | Matched by string comparison against FAST Book (Tier 1) |
| `llm_matched` | integer | Matched by Claude Opus (Tier 2) |
| `unmatched` | integer | Could not be resolved |
| `unique_fas_codes` | integer | Number of distinct FAS codes found in this bill |
| `match_rate_pct` | float | `(deterministic_matched + llm_matched) / total_provisions * 100` |

### File Lifecycle

- **Created by:** `resolve-tas`
- **Read by:** `authority build`, `compare --use-authorities`
- **Invalidated when:** the bill is re-extracted (provision indices may change). Re-run `resolve-tas --force`.
- **Skipped for:** bills with zero top-level BA provisions (CRs without anomalies, authorization bills)

---

## authorities.json

Cross-bill account authority registry. Stored at the data root directory (alongside bill directories). Created by `authority build` and consumed by `trace` and `authority list`.

### Top-Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Always `"1.0"` |
| `generated_at` | string | ISO 8601 timestamp |
| `fas_reference_hash` | string | SHA-256 of `fas_reference.json` used during build |
| `authorities` | array of AccountAuthority | One entry per unique FAS code |
| `summary` | RegistrySummary | Aggregate statistics |

### AccountAuthority

| Field | Type | Description |
|-------|------|-------------|
| `fas_code` | string | Primary identifier — the Federal Account Symbol (e.g., `"070-0400"`) |
| `agency_code` | string | CGAC agency code (e.g., `"070"`) |
| `fas_title` | string | Official title from the FAST Book |
| `agency_name` | string | Agency name from the FAST Book |
| `name_variants` | array of NameVariant | All distinct account names observed across bills |
| `provisions` | array of AuthorityProvisionRef | Every provision instance across all bills |
| `bill_count` | integer | Number of distinct bills this account appears in |
| `fiscal_years` | array of integers | Fiscal years this account has been seen in |
| `total_dollars` | integer | Total budget authority across all provisions |
| `events` | array of AuthorityEvent | Detected lifecycle events (renames) |

### NameVariant

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The account name as extracted by the LLM |
| `bills` | array of strings | Bill directories where this name was used |
| `classification` | string or null | `"canonical"`, `"case_variant"`, `"prefix_variant"`, `"name_change"`, or `"inconsistent_extraction"` |
| `fiscal_years` | array of integers | Fiscal years where this name was observed |

### AuthorityEvent

| Field | Type | Description |
|-------|------|-------------|
| `fiscal_year` | integer | The fiscal year when this event was first observed |
| `event_type` | object | Tagged by `type` field. Currently only `"rename"` with `from` and `to` string fields. |

### AuthorityProvisionRef

| Field | Type | Description |
|-------|------|-------------|
| `bill_dir` | string | Bill directory name |
| `bill_identifier` | string | Bill identifier (e.g., `"H.R. 2882"`) |
| `provision_index` | integer | Index into the bill's `provisions` array |
| `fiscal_years` | array of integers | Fiscal years the bill covers |
| `dollars` | integer or null | Dollar amount |
| `account_name` | string | The account name as extracted |
| `confidence` | string | TAS mapping confidence (`"verified"`, `"high"`, etc.) |
| `method` | string | TAS mapping method (`"direct_match"`, `"llm_resolved"`, etc.) |

### RegistrySummary

| Field | Type | Description |
|-------|------|-------------|
| `total_authorities` | integer | Total unique FAS codes |
| `total_provisions` | integer | Total provision references across all authorities |
| `bills_included` | integer | Number of bills with TAS mappings |
| `fiscal_years_covered` | array of integers | All fiscal years represented |
| `authorities_with_name_variants` | integer | Authorities with more than one distinct account name |
| `authorities_in_multiple_bills` | integer | Authorities appearing in 2+ bills |
| `total_events` | integer | Total detected lifecycle events |

### File Lifecycle

- **Created by:** `authority build`
- **Read by:** `trace`, `authority list`
- **Rebuilt from scratch** every time `authority build` runs. It is a derived artifact — delete it and rebuild at any time from the per-bill `tas_mapping.json` files.
- **Use `--force`** to rebuild when new bills have been TAS-resolved.

---

## fas_reference.json

Bundled reference data from the FAST Book (Federal Account Symbols and Titles), published by the Bureau of the Fiscal Service. Stored at the data root. Used by `resolve-tas` for deterministic matching and FAS code verification.

### Top-Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Always `"1.0"` |
| `source` | string | Source description |
| `source_url` | string | URL where the FAST Book can be downloaded |
| `publisher` | string | Bureau of the Fiscal Service, U.S. Department of the Treasury |
| `generated_at` | string | ISO 8601 timestamp of conversion |
| `statistics` | object | Summary counts |
| `agencies` | array | Agency code + name pairs |
| `accounts` | array of FasAccount | All active FAS codes |
| `discontinued` | array of FasAccount | Discontinued General Fund accounts from the Changes sheet |

### FasAccount

| Field | Type | Description |
|-------|------|-------------|
| `fas_code` | string | Federal Account Symbol (e.g., `"070-0400"`) |
| `agency_code` | string | CGAC agency code (e.g., `"070"`) |
| `main_account` | string | Main account code (e.g., `"0400"`) |
| `agency_name` | string | Agency name from the FAST Book |
| `title` | string | Full account title (e.g., `"Operations and Support, United States Secret Service, Homeland Security"`) |
| `fund_type` | string | `"general"`, `"revolving"`, `"special"`, `"trust"`, `"deposit"`, `"management"`, or `"consolidated_working"` |
| `has_no_year_variant` | boolean | Whether a no-year (X) TAS variant exists |
| `has_annual_variant` | boolean | Whether an annual TAS variant exists |
| `last_updated` | string or null | Date of last FAST Book update for this account |

### File Lifecycle

- **Generated by:** `python scripts/convert_fast_book.py` from the FAST Book Excel file
- **Read by:** `resolve-tas`, `authority build`
- **Updated:** when a new edition of the FAST Book is published (typically annually). Download the updated Excel, run the conversion script, then `resolve-tas --force` to re-resolve.
