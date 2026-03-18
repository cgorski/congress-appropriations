# Enrich Bills with Metadata

The `enrich` command generates `bill_meta.json` for each bill directory, enabling fiscal year filtering, subcommittee scoping, and advance appropriation classification. Unlike extraction (which requires an Anthropic API key) or embedding (which requires an OpenAI API key), enrichment runs entirely offline.

## Quick Start

```bash
# Enrich all bills in the examples directory
congress-approp enrich --dir examples
```

This creates a `bill_meta.json` file in each bill directory. You only need to run it once per bill — the tool skips bills that already have metadata unless you pass `--force`.

## What It Enables

After enriching, you can use these filtering options on `summary`, `search`, and `compare`:

```bash
# See only FY2026 bills
congress-approp summary --dir examples --fy 2026

# Search within a specific subcommittee
congress-approp search --dir examples --type appropriation --fy 2026 --subcommittee thud

# Combine semantic search with FY and subcommittee filtering
congress-approp search --dir examples --semantic "housing assistance" --fy 2026 --subcommittee thud --top 5

# Compare THUD funding across fiscal years
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir examples
```

> **Note:** The `--fy` flag works without `enrich` — it uses the fiscal year data already in `extraction.json`. But `--subcommittee` requires the division-to-jurisdiction mapping that only `enrich` provides.

## What It Generates

The `enrich` command creates a `bill_meta.json` file in each bill directory containing five categories of metadata:

### Subcommittee Mappings

Each division in an omnibus or minibus bill gets mapped to a canonical jurisdiction. The tool parses division titles directly from the enrolled bill XML and classifies them using pattern matching:

| Division | Title (from XML) | Jurisdiction |
|----------|------------------|-------------|
| A | Department of Defense Appropriations Act, 2026 | `defense` |
| B | Departments of Labor, Health and Human Services... | `labor-hhs` |
| D | Transportation, Housing and Urban Development... | `thud` |
| G | Other Matters | `other` |

This solves the problem where Division A means Defense in one bill but CJS in another — the `--subcommittee` flag uses the canonical jurisdiction, not the letter.

Available subcommittee slugs for `--subcommittee`:

| Slug | Jurisdiction |
|------|-------------|
| `defense` | Department of Defense |
| `labor-hhs` | Labor, Health and Human Services, Education |
| `thud` | Transportation, Housing and Urban Development |
| `financial-services` | Financial Services and General Government |
| `cjs` | Commerce, Justice, Science |
| `energy-water` | Energy and Water Development |
| `interior` | Interior, Environment |
| `agriculture` | Agriculture, Rural Development |
| `legislative-branch` | Legislative Branch |
| `milcon-va` | Military Construction, Veterans Affairs |
| `state-foreign-ops` | State, Foreign Operations |
| `homeland-security` | Homeland Security |

### Advance Appropriation Classification

Each budget authority provision is classified as:

- **current_year** — money available in the fiscal year the bill funds
- **advance** — money enacted now but available in a future fiscal year
- **supplemental** — additional emergency or supplemental funding
- **unknown** — a future fiscal year is referenced but no known pattern was matched

The classification uses a fiscal-year-aware algorithm:

1. Extract "October 1, YYYY" from the provision's availability text — this means funds available starting fiscal year YYYY+1
2. Extract "first quarter of fiscal year YYYY" — this means funds for FY YYYY
3. Compare the availability year to the bill's fiscal year
4. If the availability year is later than the bill's fiscal year → **advance**
5. If the availability year equals the bill's fiscal year → **current_year** (start of the funded FY)
6. Check provision notes for "supplemental" → **supplemental**
7. Default to **current_year**

This correctly handles cases like:

- H.R. 4366 (FY2024): VA Compensation and Pensions "available October 1, 2024" → **advance** for FY2025 ($182 billion)
- H.R. 7148 (FY2026): Medicaid "for the first quarter of fiscal year 2027" → **advance** for FY2027 ($316 billion)
- H.R. 7148 (FY2026): Tenant-Based Rental Assistance "available October 1, 2026" → **advance** for FY2027 ($4 billion)

Across the 13-bill dataset, the algorithm identifies $1.49 trillion in advance appropriations — approximately 24% of total budget authority. Failing to separate advance from current-year can cause year-over-year comparisons to be off by hundreds of billions of dollars.

### Bill Nature

The enriched bill classification provides finer distinctions than the original LLM classification:

| Original Classification | Enriched Bill Nature | Reason |
|------------------------|---------------------|--------|
| `continuing_resolution` | `full_year_cr_with_appropriations` | H.R. 1968 has 260 appropriations + a CR baseline — it's a hybrid containing $1.786 trillion in full-year appropriations |
| `omnibus` | `minibus` | H.R. 5371 covers only 3 subcommittees (Agriculture, Legislative Branch, MilCon-VA) |
| `supplemental_appropriations` | `supplemental` | H.R. 815 is normalized to the canonical enum value |

The classification uses provision type distribution and subcommittee count: 5+ real subcommittees = omnibus, 2-4 = minibus, CR baseline + many appropriations without multiple subcommittees = full-year CR with appropriations.

### Canonical Account Names

Every account name is normalized for cross-bill matching:

| Original | Canonical |
|----------|-----------|
| `Grants-In-Aid for Airports` | `grants-in-aid for airports` |
| `Grants-in-Aid for Airports` | `grants-in-aid for airports` |
| `Grants-in-aid for Airports` | `grants-in-aid for airports` |
| `Department of VA—Compensation and Pensions` | `compensation and pensions` |

Normalization lowercases, strips em-dash and en-dash prefixes, and trims whitespace. This eliminates false orphans in `compare` caused by capitalization differences and hierarchical naming conventions.

### Classification Provenance

Every classification in `bill_meta.json` records how it was determined:

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

This means: "classified as advance because the money becomes available in FY2027 but the bill covers FY2026." Provenance types include `xml_structure`, `pattern_match`, `fiscal_year_comparison`, `note_text`, and `default_rule`.

## When to Re-Enrich

The tool automatically detects when `bill_meta.json` is stale — when `extraction.json` has changed since enrichment. You will see a warning:

```text
⚠ H.R. 7148: bill metadata is stale (extraction.json has changed). Run `enrich --force`.
```

Run `enrich --force` to regenerate metadata for all bills.

## Flags

| Flag | Description |
|------|-------------|
| `--dir <DIR>` | Data directory [default: `./data`] |
| `--dry-run` | Show what would be generated without writing files |
| `--force` | Re-enrich even if `bill_meta.json` already exists |

## Previewing Before Writing

Use `--dry-run` to see what the enrich command would produce without writing any files:

```bash
congress-approp enrich --dir examples --dry-run
```

```text
  would enrich H.R. 1968: nature=FullYearCrWithAppropriations, 3 divisions, 192 BA provisions (8 advance, 3 supplemental)
  would enrich H.R. 4366: nature=Omnibus, 7 divisions, 511 BA provisions (11 advance, 4 supplemental)
  would enrich H.R. 7148: nature=Omnibus, 9 divisions, 505 BA provisions (11 advance, 4 supplemental)
  ...
```

## Using with Compare

The `compare` command benefits most from enrichment. Without `enrich`, comparing two omnibus bills that cover different subcommittees produces hundreds of false orphans. With enrichment and `--subcommittee` scoping:

```bash
# Before: 759 orphans (mixing Defense with Agriculture)
congress-approp compare --base examples/hr4366 --current examples/hr7148

# After: 43 meaningful changes, 12 unchanged
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir examples
```

The `--base-fy` and `--current-fy` flags automatically select the right bills for each fiscal year and the `--subcommittee` flag scopes to the correct division in each bill.

## Known Limitations

- **Sub-agency mismatches** — the LLM sometimes uses "Maritime Administration" in one bill and "Department of Transportation" in another for the same accounts. This creates approximately 20 false orphans per subcommittee comparison. A sub-agency normalization lookup is planned for a future release.
- **17 supplemental policy division titles** (e.g., "FEND Off Fentanyl Act", "Protecting Americans from Foreign Adversary Controlled Applications Act") are classified as `other` jurisdiction by default. These are from just two bills (H.R. 815 and S. 870) and don't affect regular appropriations bill analysis.
- **Advance detection patterns** cover "October 1, YYYY" and "first quarter of fiscal year YYYY." If Congress uses novel phrasing in future bills, those provisions would default to `current_year`. The tool logs a warning when it detects a provision referencing a future fiscal year but not matching any known advance pattern.

## Related

- [The Extraction Pipeline](../explanation/pipeline.md) — where `enrich` fits in the overall pipeline
- [Data Integrity and the Hash Chain](../explanation/hash-chain.md) — how staleness detection works for `bill_meta.json`
- [CLI Command Reference](../reference/cli.md) — complete flag reference for `enrich` and other commands
- [Data Directory Layout](../reference/data-directory.md) — where `bill_meta.json` lives in the directory structure