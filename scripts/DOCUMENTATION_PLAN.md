# Documentation Update Plan — Phase 1 (v4.0)

This document contains the complete documentation plan for the Phase 1 implementation.
Every section includes the actual prose to be added or changed. After this plan is
finalized, code implementation begins.

---

## Table of Contents

1. [CHANGELOG.md](#1-changelogmd)
2. [README.md](#2-readmemd)
3. [docs/ARCHITECTURE.md](#3-docsarchitecturemd)
4. [docs/FIELD_REFERENCE.md](#4-docsfield_referencemd)
5. [book/src/SUMMARY.md](#5-booksrcsummarymd)
6. [book/src/reference/cli.md](#6-booksrcreferenceclimd)
7. [book/src/reference/data-directory.md](#7-booksrcreferencedata-directorymd)
8. [book/src/reference/glossary.md](#8-booksrcreferenceglosarymd)
9. [book/src/explanation/pipeline.md](#9-booksrcexplanationpipelinemd)
10. [book/src/explanation/hash-chain.md](#10-booksrcexplanationhash-chainmd)
11. [book/src/how-to/enrich-data.md (NEW)](#11-booksrchow-toenrich-datamd-new)
12. [book/src/explanation/provision-types.md](#12-booksrcexplanationprovision-typesmd)
13. [book/src/appendix/changelog.md](#13-booksrcappendixchangelogmd)
14. [book/src/contributing/code-map.md](#14-booksrccontributingcode-mapmd)

---

## 1. CHANGELOG.md

Add new entry at the top of the file, before the `[3.2.0]` entry.

### Prose to add:

```markdown
## [4.0.0] — 2026-XX-XX

### Added
- **`enrich` command** — generates `bill_meta.json` per bill directory with fiscal year
  metadata, subcommittee/jurisdiction mappings, advance appropriation classification,
  bill nature classification, and canonical account names. Requires no API keys — uses
  XML parsing and deterministic keyword matching. Run `enrich --verify` (future) for
  LLM-assisted classification of novel division titles.
- **`--fy <YEAR>` flag** on `summary`, `search`, and `compare` — filter to bills covering
  a specific fiscal year. Uses `bill.fiscal_years` from extraction data (no `enrich`
  required for basic FY filtering).
- **`--subcommittee <SLUG>` flag** on `summary`, `search`, and `compare` — filter by
  appropriations subcommittee jurisdiction (e.g., `defense`, `thud`, `cjs`,
  `milcon-va`). Requires `bill_meta.json` (run `enrich` first). Maps division letters to
  canonical jurisdictions per-bill, solving the problem where Division A means Defense in
  one bill but CJS in another.
- **`--base-fy` and `--current-fy` flags** on `compare` — compare all bills for one
  fiscal year against all bills for another, with optional `--subcommittee` scoping. Use
  with `--dir` to point at the data directory.
- **`bill_meta.json`** — new per-bill metadata file containing:
  - `congress` number (parsed from XML filename)
  - `fiscal_years` (from extraction data)
  - `bill_nature` (enriched classification: `omnibus`, `minibus`,
    `full_year_cr_with_appropriations`, `supplemental`, etc.)
  - `subcommittees` (division letter → jurisdiction mapping with classification source)
  - `provision_timing` (advance/current/supplemental for each BA provision)
  - `canonical_accounts` (case-normalized, prefix-stripped account names)
  - `extraction_sha256` (hash chain link)
- **Advance appropriation detection** — the `enrich` command classifies each budget
  authority provision as current-year, advance, or supplemental using a fiscal-year-aware
  algorithm. Detects "shall become available on October 1, YYYY" and "for the first
  quarter of fiscal year YYYY" patterns, comparing the availability date to the bill's
  fiscal year. Logs warnings for provisions that reference future fiscal years but don't
  match known advance patterns.
- **`~` indicator in search $ column** — ambiguous provisions (dollar string found at
  multiple positions in source) now show `~` instead of blank, distinguishing them from
  provisions with no dollar amount. `✓` continues to mean unique attribution; `✗` means
  not found.
- **Hash chain extended** to cover `bill_meta.json`: the file records
  `extraction_sha256`, and staleness detection warns when the extraction has changed
  since enrichment.
- **Cross-semantics orphan rescue in compare** — when a provision exists in both bills
  with the same account name but different semantics (e.g., Transit Formula Grants
  classified as `limitation` in one bill and `new_budget_authority` in another), compare
  now rescues it from orphan status and shows it as "reclassified" instead of "only in
  base"/"only in current".

### Changed
- **Compare uses case-insensitive account matching.** Account names are now lowercased
  and em-dash/en-dash prefix-stripped before comparison. This resolves 52 false orphans
  across the 13-bill dataset caused by capitalization differences like "Grants-In-Aid"
  vs "Grants-in-Aid" vs "Grants-in-aid".
- **Compare handler consolidated.** The `handle_compare` function in `main.rs` now calls
  `query::compare()` instead of reimplementing the comparison logic. Duplicate
  `build_account_map`, `normalize_account_name`, and `describe_bills` functions removed
  from `main.rs`.
- **`CompareRow` field names** changed to match CLI JSON output: `account` → `account_name`,
  `base_amount` → `base_dollars`, `current_amount` → `current_dollars`.
- **`LoadedBill` struct** now includes `bill_meta: Option<BillMeta>` — loaded
  automatically from `bill_meta.json` if present, `None` otherwise.
- Version bumped to 4.0.0.

### Known Limitations
- **Sub-agency vs parent department mismatches** create approximately 20 false orphans
  per subcommittee comparison (e.g., "Maritime Administration" in one bill vs "Department
  of Transportation" in another). A sub-agency normalization lookup table is planned for
  a future release.
- **`--subcommittee` requires `enrich`** — the flag produces a clear error message if
  `bill_meta.json` is not found. `--fy` works without `enrich`.
- **17 supplemental policy division titles** (e.g., "FEND Off Fentanyl Act",
  "Protecting Americans from Foreign Adversary Controlled Applications Act") are
  classified as `other` jurisdiction by default. `enrich --verify` (future) will use
  LLM classification for these.
```

---

## 2. README.md

### 2a. Update Quick Start section (after "Explore the Example Data")

Add new subsection after the existing semantic search example, before "### Included Bills":

```markdown
### Enrich Bills for Fiscal Year and Subcommittee Filtering

The `enrich` command generates metadata that enables fiscal year and subcommittee
filtering. It requires no API keys — it parses the bill XML and uses deterministic
classification rules.

```bash
# Generate bill metadata (no API keys needed)
congress-approp enrich --dir examples

# Now you can filter by fiscal year
congress-approp summary --dir examples --fy 2026

# Filter by subcommittee jurisdiction
congress-approp search --dir examples --semantic "housing assistance" --fy 2026 --subcommittee thud --top 5

# Compare across fiscal years for a specific subcommittee
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir examples
```

Without `enrich`, the `--fy` flag still works for basic filtering (using fiscal year
data from the extraction). The `--subcommittee` flag requires `enrich` because it
needs the division-to-jurisdiction mapping that `enrich` generates.
```

### 2b. Update CLI Reference table

Add `enrich` row to the subcommand table:

```markdown
| `enrich` | Generate bill metadata for FY/subcommittee filtering (no API key needed) |
```

### 2c. Update Common Flags section

Add to the common flags list:

```markdown
- `--fy <YEAR>` on `summary`, `search`, `compare` filters to bills covering that fiscal year
- `--subcommittee <SLUG>` on `summary`, `search`, `compare` filters by jurisdiction (requires `enrich`)
```

### 2d. Update the search output `$` column description

Change the existing text from:

> The **$** column shows verification status: ✓ means the dollar amount string was found
> at exactly one position in the source text.

To:

> The **$** column shows verification status:
> - **✓** — dollar amount found at exactly one position in the source text (unique attribution)
> - **~** — dollar amount found at multiple positions (exists in source but attribution is ambiguous)
> - **✗** — dollar amount not found in source text (needs manual verification)
> - *(blank)* — provision has no dollar amount (riders, directives, etc.)

---

## 3. docs/ARCHITECTURE.md

### 3a. Update Pipeline section

Add Stage 2.5 between Extract (Stage 3) and Embed (Stage 4) in the pipeline diagram:

```markdown
### Stage 2.5: Enrich

The `enrich` command generates bill-level metadata by parsing the source XML and
analyzing the extraction output. It runs entirely offline — no API calls.

**What it produces:**
- **Subcommittee mappings** — parses `<division><enum>A</enum><header>Department of
  Defense Appropriations Act, 2026</header>` from the XML and maps each division letter
  to a canonical jurisdiction (Defense, THUD, CJS, etc.) using pattern matching on the
  title text.
- **Advance appropriation classification** — for each budget authority provision,
  extracts "October 1, YYYY" or "first quarter of fiscal year YYYY" from the
  availability text, compares to the bill's fiscal year, and classifies as advance
  (money for a future FY), current-year, or supplemental. This is a fiscal-year-aware
  algorithm — "October 1, 2025" in a FY2026 bill means start-of-FY2026 (current-year),
  not advance.
- **Bill nature** — enriches the LLM's bill classification with finer distinctions.
  For example, H.R. 1968 is classified as `continuing_resolution` by the LLM but is
  actually a "full-year CR with appropriations" containing $1.786 trillion in
  appropriations. The enrich command detects this from the provision type distribution.
- **Canonical account names** — lowercase, em-dash-prefix-stripped versions of every
  account name, used for case-insensitive cross-bill matching.

**Input:** `extraction.json` + `BILLS-*.xml`
**Output:** `bill_meta.json`
**Requires:** No API keys. Runs offline.

The `bill_meta.json` file is optional for all existing commands. It's required only
when using `--subcommittee` filtering or `--show-advance` display options.
```

### 3b. Update Module Map section

Add to the "Core data types" table:

```markdown
| `bill_meta.rs` | ~350 | Bill-level metadata types and classification functions. `BillMeta`, `BillNature`, `Jurisdiction`, `SubcommitteeMapping`, `ProvisionTiming`, `FundingTiming`, `CanonicalAccount`, `ClassificationSource`. Functions for XML division parsing, jurisdiction classification, fiscal-year-aware advance detection, account normalization. |
```

### 3c. Update Hash Chain section

Add new link to the hash chain diagram:

```text
BILLS-*.xml ──sha256──▶ metadata.json (source_xml_sha256)
                              │
extraction.json ──sha256──▶ bill_meta.json (extraction_sha256)     ← NEW
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
                              │
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

And add description:

```markdown
### Link 1.5: Extraction → Bill Metadata

When `enrich` runs, it computes the SHA-256 hash of `extraction.json` and stores it in
`bill_meta.json`:

```json
{
  "schema_version": "1.0",
  "extraction_sha256": "b461a687..."
}
```

If the extraction is re-run (producing a different `extraction.json`), the hash in
`bill_meta.json` no longer matches. This tells you the bill metadata may be stale —
provision indices in `provision_timing` and `canonical_accounts` could have shifted.
Run `enrich --force` to regenerate.
```

---

## 4. docs/FIELD_REFERENCE.md

### Add new section at the end: `bill_meta.json`

```markdown
---

## bill_meta.json

Bill-level metadata generated by the `enrich` command. This file is optional — all
existing commands work without it. It's required for `--subcommittee` filtering and
advance appropriation classification.

### Top-Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Always `"1.0"` for this format |
| `congress` | integer or null | Congress number parsed from the XML filename (e.g., `119` for `BILLS-119hr7148enr.xml`). Null if the filename doesn't match the expected pattern |
| `fiscal_years` | array of integers | Fiscal years this bill covers, copied from `extraction.json` `bill.fiscal_years` |
| `bill_nature` | string | Enriched bill classification. One of: `regular`, `omnibus`, `minibus`, `continuing_resolution`, `full_year_cr_with_appropriations`, `supplemental`, `authorization`, or a free-text string |
| `subcommittees` | array of SubcommitteeMapping | Division letter → jurisdiction mappings |
| `provision_timing` | array of ProvisionTiming | Advance/current/supplemental classification per BA provision |
| `canonical_accounts` | array of CanonicalAccount | Normalized account names per provision |
| `extraction_sha256` | string | SHA-256 hash of `extraction.json` at the time of enrichment. Part of the hash chain for staleness detection |

### SubcommitteeMapping

| Field | Type | Description |
|-------|------|-------------|
| `division` | string | Division letter (e.g., `"A"`, `"B"`) |
| `jurisdiction` | string | Canonical jurisdiction slug. One of: `defense`, `labor_hhs`, `thud`, `financial_services`, `cjs`, `energy_water`, `interior`, `agriculture`, `legislative_branch`, `milcon_va`, `state_foreign_ops`, `homeland_security`, `continuing_resolution`, `extenders`, `policy`, `budget_process`, `other` |
| `title` | string | Raw division title from the XML (e.g., `"DEPARTMENT OF DEFENSE APPROPRIATIONS ACT, 2026"`) |
| `source` | ClassificationSource | How this jurisdiction was determined |

### ProvisionTiming

| Field | Type | Description |
|-------|------|-------------|
| `provision_index` | integer | Index into the `provisions` array in `extraction.json` (0-based) |
| `timing` | string | One of: `current_year`, `advance`, `supplemental`, `unknown` |
| `available_fy` | integer or null | The fiscal year the money becomes available, if determined. For advance appropriations, this is the future FY. For current-year, this equals the bill's fiscal year or is null |
| `source` | ClassificationSource | How this timing was determined |

### CanonicalAccount

| Field | Type | Description |
|-------|------|-------------|
| `provision_index` | integer | Index into the `provisions` array (0-based) |
| `canonical_name` | string | Normalized account name: lowercased, em-dash/en-dash prefix stripped, whitespace trimmed. For example, `"Department of Veterans Affairs—Veterans Benefits Administration—Compensation and Pensions"` becomes `"compensation and pensions"` |

### ClassificationSource

A tagged object describing how a classification was determined. Provides provenance
for every automated decision.

| Variant | Fields | Description |
|---------|--------|-------------|
| `xml_structure` | — | Parsed directly from XML element structure |
| `pattern_match` | `pattern` (string) | Matched against a known text pattern (e.g., `"department of defense"`) |
| `fiscal_year_comparison` | `availability_fy` (int), `bill_fy` (int) | Determined by comparing the provision's availability FY to the bill's FY |
| `note_text` | — | Classified based on the provision's `notes` array content |
| `default_rule` | — | No specific signal found; applied the default classification |
| `llm_classification` | `model` (string), `confidence` (float) | Classified by an LLM (used by `enrich --verify`, not yet implemented) |
| `manual` | — | Manually overridden by a user |

### Example bill_meta.json

```json
{
  "schema_version": "1.0",
  "congress": 119,
  "fiscal_years": [2026],
  "bill_nature": "omnibus",
  "subcommittees": [
    {
      "division": "A",
      "jurisdiction": "defense",
      "title": "DEPARTMENT OF DEFENSE APPROPRIATIONS ACT, 2026",
      "source": { "type": "pattern_match", "pattern": "department of defense" }
    },
    {
      "division": "B",
      "jurisdiction": "labor_hhs",
      "title": "DEPARTMENTS OF LABOR, HEALTH AND HUMAN SERVICES, AND EDUCATION, AND RELATED AGENCIES APPROPRIATIONS ACT, 2026",
      "source": { "type": "pattern_match", "pattern": "departments? of labor.*health" }
    },
    {
      "division": "D",
      "jurisdiction": "thud",
      "title": "TRANSPORTATION, HOUSING AND URBAN DEVELOPMENT, AND RELATED AGENCIES APPROPRIATIONS ACT, 2026",
      "source": { "type": "pattern_match", "pattern": "transportation.*housing.*urban" }
    },
    {
      "division": "G",
      "jurisdiction": "other",
      "title": "Other Matters",
      "source": { "type": "default_rule" }
    }
  ],
  "provision_timing": [
    {
      "provision_index": 1369,
      "timing": "current_year",
      "available_fy": null,
      "source": { "type": "default_rule" }
    },
    {
      "provision_index": 1370,
      "timing": "advance",
      "available_fy": 2027,
      "source": {
        "type": "fiscal_year_comparison",
        "availability_fy": 2027,
        "bill_fy": 2026
      }
    }
  ],
  "canonical_accounts": [
    { "provision_index": 0, "canonical_name": "military personnel, army" },
    { "provision_index": 1369, "canonical_name": "tenant-based rental assistance" }
  ],
  "extraction_sha256": "b461a6878d8d9fae67d1752642823dd3aa72b74996ed17717d652d5dd909719f"
}
```
```

---

## 5. book/src/SUMMARY.md

### Add new entry in "How-To Guides" section

After the "Upgrade Extraction Data" entry, add:

```markdown
- [Enrich Bills with Metadata](./how-to/enrich-data.md)
```

---

## 6. book/src/reference/cli.md

### 6a. Add `enrich` section (after `embed`, before `upgrade`)

```markdown
## enrich

Generate bill metadata for fiscal year filtering, subcommittee scoping, and advance
appropriation classification. This command parses the source XML and analyzes the
extraction output — **no API keys are required**.

```text
congress-approp enrich [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--dir <DIR>` | Data directory [default: `./data`] |
| `--dry-run` | Preview what would be generated without writing files |
| `--force` | Re-enrich even if `bill_meta.json` already exists |

### What It Generates

For each bill directory, `enrich` creates a `bill_meta.json` file containing:

- **Congress number** — parsed from the XML filename (e.g., `BILLS-119hr7148enr.xml` → 119)
- **Subcommittee mappings** — division letter → jurisdiction (e.g., Division A → Defense)
- **Bill nature** — enriched classification (omnibus, minibus, full-year CR with appropriations, etc.)
- **Advance appropriation classification** — each budget authority provision is classified as current-year, advance, or supplemental
- **Canonical account names** — case-normalized, prefix-stripped names for cross-bill matching

### Examples

```bash
# Enrich all bills in the examples directory
congress-approp enrich --dir examples

# Preview without writing files
congress-approp enrich --dir examples --dry-run

# Force re-enrichment of already-enriched bills
congress-approp enrich --dir examples --force
```

### When to Run

Run `enrich` once after extracting bills, before using `--subcommittee` filters.
You don't need to re-run it unless you re-extract a bill (the tool warns about
staleness via the hash chain).

The `--fy` flag on other commands works without `enrich` — it uses the fiscal year
data already in `extraction.json`. But `--subcommittee` requires the
division-to-jurisdiction mapping that only `enrich` provides.
```

### 6b. Update `summary` section — add new flags

Add to the flag table for `summary`:

```markdown
| `--fy <YEAR>` | Filter to bills covering this fiscal year |
| `--subcommittee <SLUG>` | Filter by subcommittee jurisdiction (requires `enrich`). Slugs: `defense`, `labor-hhs`, `thud`, `financial-services`, `cjs`, `energy-water`, `interior`, `agriculture`, `legislative-branch`, `milcon-va`, `state-foreign-ops`, `homeland-security` |
```

Add examples:

```markdown
# FY2026 budget summary
congress-approp summary --dir examples --fy 2026

# FY2026 THUD subcommittee only
congress-approp summary --dir examples --fy 2026 --subcommittee thud
```

### 6c. Update `search` section — add new flags

Add to the "Filter Flags" table for `search`:

```markdown
| `--fy <YEAR>` | Filter to bills covering this fiscal year |
| `--subcommittee <SLUG>` | Filter by subcommittee jurisdiction (requires `enrich`) |
```

Add examples:

```markdown
# Find FY2026 defense appropriations
congress-approp search --dir examples --type appropriation --fy 2026 --subcommittee defense

# Semantic search scoped to FY2026 THUD
congress-approp search --dir examples --semantic "housing vouchers" --fy 2026 --subcommittee thud --top 5
```

### 6d. Update `compare` section — add new flags

Add to the flag table for `compare`:

```markdown
| `--base-fy <YEAR>` | Use all bills for this FY as the base set (alternative to `--base`) |
| `--current-fy <YEAR>` | Use all bills for this FY as the current set (alternative to `--current`) |
| `--dir <DIR>` | Data directory (required with `--base-fy`/`--current-fy`) |
| `--subcommittee <SLUG>` | Scope comparison to one jurisdiction (requires `enrich`) |
```

Add examples:

```markdown
# Compare THUD funding: FY2024 → FY2026
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir examples

# Compare all FY2024 vs all FY2026 (without subcommittee scope)
congress-approp compare --base-fy 2024 --current-fy 2026 --dir examples
```

Add a note about the cross-semantics behavior:

```markdown
### Reclassification Detection

When the same account exists in both bills but with different budget semantics (e.g.,
classified as `limitation` in one and `new_budget_authority` in the other), compare now
shows the account as **reclassified** instead of listing it as an orphan in both sides.
This catches cases like Transit Formula Grants, which Congress funds through contract
authority that the LLM may classify differently between bills.
```

### 6e. Update `$` column description in the search section

In the "Table Output Columns" subsection, change the `$` column description:

```markdown
| `$` | Verification indicator. **✓** = dollar amount found at exactly one position in source (strong attribution). **~** = found at multiple positions (amount exists but attribution is ambiguous). **✗** = not found in source (review manually). Blank = provision has no dollar amount. |
```

---

## 7. book/src/reference/data-directory.md

### 7a. Add `bill_meta.json` to the directory tree

In the directory structure diagram, add after `metadata.json`:

```text
│   ├── bill_meta.json              ← bill metadata: FY, jurisdictions, advance classification
```

### 7b. Add to the file reference table

```markdown
| `bill_meta.json` | No | `enrich` | `--subcommittee`, `--fy` filtering, compare | Only by re-enrich | ~5 KB |
```

### 7c. Add file description section

```markdown
### bill_meta.json

Bill-level metadata generated by the `enrich` command. Contains fiscal year scoping,
subcommittee jurisdiction mappings, advance appropriation classification, and canonical
account names.

```json
{
  "schema_version": "1.0",
  "congress": 119,
  "fiscal_years": [2026],
  "bill_nature": "omnibus",
  "subcommittees": [...],
  "provision_timing": [...],
  "canonical_accounts": [...],
  "extraction_sha256": "b461a687..."
}
```

This file is entirely optional. All commands that existed before v4.0 work without it.
It is required only for `--subcommittee` filtering. The `--fy` flag works without it
(falling back to `extraction.json` fiscal year data).

The `extraction_sha256` field is part of the hash chain — it records the SHA-256 of
`extraction.json` at enrichment time, enabling staleness detection.

See [bill_meta.json Fields](../../docs/FIELD_REFERENCE.md) for the complete field
reference.
```

### 7d. Update hash chain section

Update the hash chain diagram to include the new link:

```text
BILLS-*.xml ──sha256──▶ metadata.json (source_xml_sha256)
                              │
extraction.json ──sha256──▶ bill_meta.json (extraction_sha256)     ← NEW
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
                              │
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

---

## 8. book/src/reference/glossary.md

### Add new terms (alphabetical insertion):

```markdown
**Bill Nature** — An enriched classification of an appropriations bill that provides
finer distinctions than the LLM's `classification` field. Where the extraction might
classify H.R. 1968 as `continuing_resolution`, the bill nature recognizes it as
`full_year_cr_with_appropriations` — a hybrid vehicle containing $1.786 trillion in
full-year appropriations alongside a CR mechanism. Generated by the `enrich` command
and stored in `bill_meta.json`. Values: `regular`, `omnibus`, `minibus`,
`continuing_resolution`, `full_year_cr_with_appropriations`, `supplemental`,
`authorization`, or a free-text string. See [Enrich Bills with Metadata](../how-to/enrich-data.md).

**Canonical Account Name** — A normalized version of an account name used for
cross-bill matching: lowercased, em-dash and en-dash prefixes stripped, whitespace
trimmed. For example, `"Department of Veterans Affairs—Veterans Benefits
Administration—Compensation and Pensions"` becomes `"compensation and pensions"`.
This ensures that the same account matches across bills even when the LLM uses
different naming conventions. Generated by `enrich` and stored in `bill_meta.json`.

**Classification Source** — A provenance record in `bill_meta.json` that documents how
each automated classification was determined. Every jurisdiction mapping, advance/current
timing classification, and bill nature determination records whether it came from XML
structure parsing, pattern matching, fiscal year comparison, note text analysis, a
default rule, or LLM classification. This enables auditing: you can see exactly why the
tool classified a provision as "advance" or a division as "defense."

**Enrich** — The process of generating bill-level metadata (`bill_meta.json`) from the
source XML and extraction output. Unlike extraction (which requires an LLM API key),
enrichment runs entirely offline using XML parsing and deterministic classification
rules. Run `congress-approp enrich --dir examples` to enrich all bills. See [Enrich
Bills with Metadata](../how-to/enrich-data.md).

**Funding Timing** — Whether a budget authority provision's money is available in the
current fiscal year (`current_year`), a future fiscal year (`advance`), or was provided
as emergency/supplemental funding (`supplemental`). Determined by the `enrich` command
using a fiscal-year-aware algorithm that compares "October 1, YYYY" dates in the
availability text to the bill's fiscal year. Critical for year-over-year comparisons —
without separating advance from current, a reporter might overstate FY2024 VA spending
by $182 billion (the advance appropriation for FY2025). See [Enrich Bills with
Metadata](../how-to/enrich-data.md).

**Jurisdiction** — The appropriations subcommittee responsible for a division of an
omnibus or minibus bill. The twelve traditional jurisdictions are: Defense, Labor-HHS,
THUD (Transportation-Housing-Urban Development), Financial Services, CJS
(Commerce-Justice-Science), Energy-Water, Interior, Agriculture, Legislative Branch,
MilCon-VA (Military Construction-Veterans Affairs), State-Foreign Operations, and
Homeland Security. Division letters are bill-internal (Division A means Defense in one
bill but CJS in another), so the `enrich` command maps each division to its canonical
jurisdiction. Used with the `--subcommittee` flag. See [Enrich Bills with
Metadata](../how-to/enrich-data.md).

**Subcommittee** — See **Jurisdiction**. In the context of this tool, `--subcommittee`
refers to the twelve appropriations subcommittee jurisdictions, each of which produces
one of the twelve annual appropriations bills. When bills are combined into an omnibus,
each subcommittee's bill typically becomes one division.
```

### Update existing "Division" term:

Change to:

```markdown
**Division** — A lettered section of an omnibus or minibus bill (Division A, Division B,
etc.), each typically corresponding to one of the twelve appropriations subcommittee
jurisdictions. **Division letters are bill-internal** — Division A means Defense in
H.R. 7148 but CJS in H.R. 6938 and MilCon-VA in H.R. 4366. For cross-bill filtering,
use `--subcommittee` (which resolves division letters to canonical jurisdictions via
`bill_meta.json`) instead of `--division`. The `--division` flag is still available for
within-bill filtering when you know the specific letter.
```

### Update existing "Advance Appropriation" term:

Change to:

```markdown
**Advance Appropriation** — Budget authority enacted in the current year's
appropriations bill but not available for obligation until a future fiscal year. Common
for VA medical accounts, where FY2024 legislation may include advance appropriations
available starting in FY2025. The `enrich` command classifies each budget authority
provision as `current_year`, `advance`, or `supplemental` using a fiscal-year-aware
algorithm. This classification is stored in `bill_meta.json` in the `provision_timing`
array. Advance appropriations represent approximately 18% ($1.14 trillion) of total
budget authority across the 13-bill dataset. Failing to separate advance from current-
year can cause year-over-year comparisons to be off by hundreds of billions of dollars.
See [Enrich Bills with Metadata](../how-to/enrich-data.md).
```

---

## 9. book/src/explanation/pipeline.md

### Add Stage 2.5 between Extract and Embed

After the Stage 3 (Extract) section and before Stage 4 (Embed), add:

```markdown
## Stage 2.5: Enrich (Optional)

The `enrich` command generates bill-level metadata by parsing the source XML structure
and analyzing the already-extracted provisions. It bridges the gap between raw extraction
and informed querying.

**Why this stage exists:** The LLM extracts provisions faithfully — every dollar amount,
every account name, every section reference. But it doesn't know that Division A in
H.R. 7148 covers Defense while Division A in H.R. 6938 covers CJS. It doesn't know that
"shall become available on October 1, 2024" in a FY2024 bill means the money is for
FY2025 (an advance appropriation). It doesn't know that "Grants-In-Aid for Airports"
and "Grants-in-Aid for Airports" are the same account. The `enrich` command adds this
structural and normalization knowledge.

**How it works:**

1. **Parse division titles from XML.** The enrolled bill XML contains
   `<division><enum>A</enum><header>Department of Defense Appropriations Act, 2026</header>`
   elements. The enrich command extracts each division's letter and title, then classifies
   the title to a jurisdiction using case-insensitive pattern matching against known
   subcommittee names.

2. **Classify advance vs current-year.** For each budget authority provision, the command
   checks the `availability` field and `raw_text` for "October 1, YYYY" or "first quarter
   of fiscal year YYYY" patterns. It compares the referenced year to the bill's fiscal year:
   if the money becomes available after the bill's FY ends, it's advance.

3. **Normalize account names.** Each account name is lowercased and stripped of
   hierarchical em-dash prefixes (e.g., "Department of VA—Compensation and Pensions" →
   "compensation and pensions") for cross-bill matching.

4. **Classify bill nature.** The provision type distribution determines whether the bill
   is an omnibus (5+ subcommittees), minibus (2-4), full-year CR with appropriations
   (CR baseline + hundreds of regular appropriations), or other type.

**Input:** `extraction.json` + `BILLS-*.xml`
**Output:** `bill_meta.json`
**Requires:** Nothing — no API keys, no network access.

This stage is optional. All commands from v3.x continue to work without it. It's required
only for `--subcommittee` filtering and advance appropriation separation.
```

Also update the pipeline overview diagram to include the new stage:

```text
                    ┌──────────┐
  Congress.gov ───▶ │ Download │ ───▶ BILLS-*.xml
                    └──────────┘
                         │
                    ┌──────────┐
                    │  Parse   │ ───▶ clean text + chunk boundaries
                    │  + XML   │
                    └──────────┘
                         │
                    ┌──────────┐
  Anthropic API ◀── │ Extract  │ ───▶ extraction.json + verification.json
                    │  (LLM)   │      metadata.json + tokens.json + chunks/
                    └──────────┘
                         │
                    ┌──────────┐
                    │ Enrich   │ ───▶ bill_meta.json          ← NEW (offline)
                    │(optional)│
                    └──────────┘
                         │
                    ┌──────────┐
  OpenAI API ◀───── │  Embed   │ ───▶ embeddings.json + vectors.bin
                    └──────────┘
                         │
                    ┌──────────┐
                    │  Query   │ ───▶ search, compare, summary, audit
                    └──────────┘
```

---

## 10. book/src/explanation/hash-chain.md

### Add new link description

After the "Link 1" section and before the existing "Link 2" section, add:

```markdown
### Link 1.5: Extraction → Bill Metadata

When the `enrich` command runs, it hashes `extraction.json` and records it:

```json
{
  "extraction_sha256": "b461a6878d8d9fae..."
}
```

If the extraction changes (e.g., re-extracted with a new model), the provision indices
in `bill_meta.json` may no longer be valid — provision 1369 in the new extraction might
be a different provision than provision 1369 in the old extraction. The staleness check
detects this and warns:

```text
⚠ H.R. 7148: bill metadata is stale (extraction.json has changed). Run `enrich --force`.
```
```

Also update the chain diagram to include the new link.

---

## 11. book/src/how-to/enrich-data.md (NEW)

This is a brand new chapter.

```markdown
# Enrich Bills with Metadata

The `enrich` command generates `bill_meta.json` for each bill directory, enabling fiscal
year filtering, subcommittee scoping, and advance appropriation classification. Unlike
extraction (which requires an Anthropic API key) or embedding (which requires an OpenAI
API key), enrichment runs entirely offline.

## Quick Start

```bash
# Enrich all bills in the examples directory
congress-approp enrich --dir examples
```

This creates a `bill_meta.json` file in each bill directory. You only need to run it once
per bill — the tool skips bills that already have metadata unless you pass `--force`.

## What It Enables

After enriching, you can use these new filtering options:

```bash
# See only FY2026 bills
congress-approp summary --dir examples --fy 2026

# Search within a specific subcommittee
congress-approp search --dir examples --type appropriation --fy 2026 --subcommittee thud

# Compare THUD funding across fiscal years
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir examples
```

## What It Generates

### Subcommittee Mappings

Each division in an omnibus or minibus bill gets mapped to a canonical jurisdiction:

| Division | Title (from XML) | Jurisdiction |
|----------|------------------|-------------|
| A | Department of Defense Appropriations Act, 2026 | `defense` |
| B | Departments of Labor, Health and Human Services... | `labor-hhs` |
| D | Transportation, Housing and Urban Development... | `thud` |
| G | Other Matters | `other` |

The tool parses division titles directly from the enrolled bill XML and classifies them
using pattern matching. About 40% of division titles match the twelve traditional
subcommittee names. Generic titles like "Other Matters" are classified as `other`.
Supplemental policy titles (e.g., "FEND Off Fentanyl Act") are also classified as `other`
by default.

### Advance Appropriation Classification

Each budget authority provision is classified as:

- **current_year** — money available in the fiscal year the bill funds
- **advance** — money enacted now but available in a future fiscal year
- **supplemental** — additional emergency or supplemental funding
- **unknown** — a future fiscal year is referenced but no known pattern was matched

The classification uses a fiscal-year-aware algorithm:

1. Extract "October 1, YYYY" from the provision's availability text
2. "October 1, YYYY" means funds available starting fiscal year YYYY+1
3. If YYYY+1 > the bill's fiscal year → **advance**
4. If YYYY+1 = the bill's fiscal year → **current_year** (start of the funded FY)
5. Also check for "first quarter of fiscal year YYYY" — if YYYY > bill FY → **advance**
6. Default to **current_year**

This algorithm correctly handles cases like:
- H.R. 4366 (FY2024): VA Compensation and Pensions "available October 1, 2024" → **advance** for FY2025
- H.R. 7148 (FY2026): Tenant-Based Rental Assistance "available October 1, 2026" → **advance** for FY2027
- H.R. 7148 (FY2026): Medicaid "for the first quarter of fiscal year 2027" → **advance** for FY2027

### Bill Nature

The enriched bill classification provides finer distinctions than the LLM's original
classification:

| LLM Classification | Enriched Bill Nature | Reason |
|--------------------|--------------------|--------|
| `continuing_resolution` | `full_year_cr_with_appropriations` | H.R. 1968 has 260 appropriations + a CR baseline — it's a hybrid |
| `omnibus` | `minibus` | H.R. 5371 covers only 3 subcommittees (Ag, LegBranch, MilCon-VA) |
| `supplemental_appropriations` | `supplemental` | H.R. 815 is normalized to the canonical enum value |

### Canonical Account Names

Every account name is normalized for cross-bill matching:

| Original | Canonical |
|----------|-----------|
| `Grants-In-Aid for Airports` | `grants-in-aid for airports` |
| `Grants-in-Aid for Airports` | `grants-in-aid for airports` |
| `Department of VA—Compensation and Pensions` | `compensation and pensions` |

This eliminates false orphans in `compare` caused by capitalization differences and
hierarchical naming conventions.

## Provenance

Every classification in `bill_meta.json` records how it was determined. When you inspect
the file, you'll see entries like:

```json
{
  "timing": "advance",
  "available_fy": 2027,
  "source": {
    "type": "fiscal_year_comparison",
    "availability_fy": 2027,
    "bill_fy": 2026
  }
}
```

This means: "classified as advance because the money becomes available in FY2027 but the
bill covers FY2026." This provenance makes it possible to audit every automated decision
and understand exactly why a particular provision was classified the way it was.

## When to Re-Enrich

The tool automatically detects when `bill_meta.json` is stale — when `extraction.json`
has changed since enrichment. You'll see a warning:

```text
⚠ H.R. 7148: bill metadata is stale (extraction.json has changed)
```

Run `enrich --force` to regenerate metadata for all bills, or re-enrich only the
affected bill directory.

## Flags

| Flag | Description |
|------|-------------|
| `--dir <DIR>` | Data directory [default: `./data`] |
| `--dry-run` | Show what would be generated without writing files |
| `--force` | Re-enrich even if `bill_meta.json` already exists |
```

---

## 12. book/src/explanation/provision-types.md

### Add note about cross-semantics in compare

In the section discussing how provision types interact with budget authority calculation,
add:

```markdown
### Cross-Semantics in Compare

The `compare` command matches accounts between two sets of bills using budget authority
provisions. Occasionally, the LLM classifies the same program with different semantics
across bills — for example, Transit Formula Grants might be classified with `semantics:
"limitation"` in one bill and `semantics: "new_budget_authority"` in another.

Starting in v4.0, the compare command detects these cross-semantics cases. When an
account name appears in one bill's budget authority map but not the other's, the tool
searches the other bill for provisions with the same canonical account name regardless
of semantics. If found, the account is shown as **reclassified** rather than appearing
as an orphan in both columns.

This recovers matches for programs like Transit Formula Grants ($14.6 billion), which
Congress funds through contract authority that the LLM may legitimately classify as
either a limitation or new budget authority depending on the specific language used.
```

---

## 13. book/src/appendix/changelog.md

Mirror the CHANGELOG.md content from section 1 above into the book's changelog appendix.

---

## 14. book/src/contributing/code-map.md

### Add `bill_meta.rs` to the module listing

```markdown
| `bill_meta.rs` | ~350 | Bill metadata types (`BillMeta`, `BillNature`, `Jurisdiction`, `SubcommitteeMapping`, `ProvisionTiming`, `FundingTiming`, `CanonicalAccount`, `ClassificationSource`) and classification functions. Parses division titles from XML, classifies jurisdictions via pattern matching, detects advance appropriations via fiscal-year-aware date comparison, normalizes account names. No external dependencies — runs entirely offline. |
```

---

## Verification Checklist

After all documentation is updated, verify:

- [ ] All code examples in docs compile or are plausible CLI invocations
- [ ] All file paths in cross-references point to real files
- [ ] The SUMMARY.md new entry points to a file that exists
- [ ] The glossary terms are in alphabetical order
- [ ] The pipeline diagram in all locations matches (README, ARCHITECTURE.md, pipeline.md)
- [ ] The hash chain diagram in all locations matches
- [ ] The CLI reference documents every new flag
- [ ] The field reference documents every field in bill_meta.json
- [ ] The changelog covers every user-visible change
- [ ] No orphaned references to features that aren't implemented

---

## Implementation Notes

- The documentation changes should be committed alongside the code changes, not after
- All prose examples should be verified against actual command output after implementation
- The `enrich-data.md` chapter should be reviewed by both the journalist persona (Sarah)
  and the staffer persona (Marcus) to ensure workflows are clear
- If any behavior differs from what's documented here during implementation, update the
  documentation to match reality — the code is the source of truth