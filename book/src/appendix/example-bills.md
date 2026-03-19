# Included Example Bills

The `examples/` directory ships with **thirteen pre-extracted enacted appropriations bills** across the 118th and 119th Congresses, covering FY2024 through FY2026. These are real enacted laws with real data — no API keys are needed to query them. All twelve appropriations subcommittees are represented for FY2026.

Each bill directory contains the source XML, extraction.json, verification.json, metadata.json, bill_meta.json (from `enrich`), embeddings.json, and vectors.bin (pre-computed embeddings for semantic search).

## Bill Summary

### 118th Congress (FY2024/FY2025)

| Directory | Bill | Classification | Subcommittees | Provisions | Budget Auth |
|-----------|------|---------------|---------------|-----------|------------|
| `examples/hr4366/` | H.R. 4366 | Omnibus | MilCon-VA, Ag, CJS, E&W, Interior, THUD | 2,364 | $846B |
| `examples/hr5860/` | H.R. 5860 | Continuing Resolution | (all, at prior-year rates) | 130 | $16B |
| `examples/hr9468/` | H.R. 9468 | Supplemental | VA | 7 | $2.9B |
| `examples/hr815/` | H.R. 815 | Supplemental | Defense, State (Ukraine/Israel/Taiwan) | 303 | $95B |
| `examples/hr2872/` | H.R. 2872 | Continuing Resolution | (further CR) | 31 | $0 |
| `examples/hr6363/` | H.R. 6363 | Continuing Resolution | (further CR + extensions) | 74 | ~$0 |
| `examples/hr7463/` | H.R. 7463 | Continuing Resolution | (CR extension) | 10 | $0 |
| `examples/hr9747/` | H.R. 9747 | Continuing Resolution | (CR + extensions, FY2025) | 114 | $383M |
| `examples/s870/` | S. 870 | Authorization | Fire administration | 49 | $0 |

### 119th Congress (FY2025/FY2026)

| Directory | Bill | Classification | Subcommittees | Provisions | Budget Auth |
|-----------|------|---------------|---------------|-----------|------------|
| `examples/hr1968/` | H.R. 1968 | Full-Year CR with Appropriations | Defense, Homeland, Labor-HHS, others | 526 | $1,786B |
| `examples/hr5371/` | H.R. 5371 | Minibus | CR + Ag + LegBranch + MilCon-VA | 1,048 | $681B |
| `examples/hr6938/` | H.R. 6938 | Minibus | CJS + Energy-Water + Interior | 1,061 | $196B |
| `examples/hr7148/` | H.R. 7148 | Omnibus | Defense + Labor-HHS + THUD + FinServ + State | 2,837 | $2,788B |

**Totals:** 8,554 provisions, $6.4 trillion in budget authority, 0 unverifiable dollar amounts, 95.5% raw text exact match.

**Missing:** H.R. 2882 (FY2024 second omnibus covering Defense, Labor-HHS, Homeland, State, FinServ, LegBranch). Extraction failed due to 15 persistent chunk failures. The enrolled XML is available on Congress.gov if someone wants to retry with a future extraction resume feature.

---

## H.R. 4366 — The FY2024 Omnibus

### What it is

The Consolidated Appropriations Act, 2024 is an **omnibus** — a single legislative vehicle packaging multiple annual appropriations bills together. It covers seven of the twelve appropriations subcommittee jurisdictions, organized into lettered divisions:

| Division | Subcommittee Jurisdiction |
|----------|--------------------------|
| A | Military Construction, Veterans Affairs |
| B | Agriculture, Rural Development, Food and Drug Administration |
| C | Commerce, Justice, Science |
| D | Energy and Water Development |
| E | Interior, Environment |
| F | Transportation, Housing and Urban Development |
| G–H | Other matters |

**Not included:** Defense, Labor-HHS-Education, Homeland Security, State-Foreign Operations, Financial Services, and Legislative Branch (these were addressed through other legislation for FY2024).

### Why it matters

This is the largest and most complex bill in the example data. At 2,364 provisions across ~1,500 pages of legislative text, it's a comprehensive test of the tool's extraction, verification, and query capabilities. It includes every provision type except `cr_substitution` and `continuing_resolution_baseline` (which are specific to continuing resolutions).

### Provision type breakdown

| Type | Count | Percentage |
|------|-------|-----------|
| `appropriation` | 1,216 | 51.4% |
| `limitation` | 456 | 19.3% |
| `rider` | 285 | 12.1% |
| `directive` | 120 | 5.1% |
| `other` | 84 | 3.6% |
| `rescission` | 78 | 3.3% |
| `transfer_authority` | 77 | 3.3% |
| `mandatory_spending_extension` | 40 | 1.7% |
| `directed_spending` | 8 | 0.3% |
| **Total** | **2,364** | **100%** |

### Key accounts (top 10 by budget authority)

| Account | Agency | Budget Authority |
|---------|--------|-----------------|
| Compensation and Pensions | Department of Veterans Affairs | $197,382,903,000 |
| Supplemental Nutrition Assistance Program | Department of Agriculture | $122,382,521,000 |
| Medical Services | Department of Veterans Affairs | $71,000,000,000 |
| Child Nutrition Programs | Department of Agriculture | $33,266,226,000 |
| Tenant-Based Rental Assistance | Dept. of Housing and Urban Development | $32,386,831,000 |
| Medical Community Care | Department of Veterans Affairs | $20,382,000,000 |
| Weapons Activities | Department of Energy | $19,108,000,000 |
| Project-Based Rental Assistance | Dept. of Housing and Urban Development | $16,010,000,000 |
| Readjustment Benefits | Department of Veterans Affairs | $13,774,657,000 |
| Operations | Federal Aviation Administration | $12,729,627,000 |

> **Note:** The largest accounts (VA Comp & Pensions, SNAP, VA Medical Services) are mandatory spending programs that appear as appropriation lines in the bill text. See [Why the Numbers Might Not Match Headlines](../explanation/numbers-vs-headlines.md).

### Verification metrics

| Metric | Value |
|--------|-------|
| Dollar amounts verified (unique position) | 762 |
| Dollar amounts not found | **0** |
| Dollar amounts ambiguous (multiple positions) | 723 |
| Raw text exact match | 2,285 (96.7%) |
| Raw text normalized match | 59 (2.5%) |
| Raw text no match | 20 (0.8%) |
| Coverage | 94.2% |

The 20 "no match" provisions are all non-dollar statutory amendments where the LLM slightly reformatted section references. No provision with a dollar amount has a text mismatch.

### Key data files

| File | Size | Description |
|------|------|-------------|
| `BILLS-118hr4366enr.xml` | 1.8 MB | Enrolled bill XML from Congress.gov |
| `extraction.json` | ~12 MB | 2,364 structured provisions |
| `verification.json` | ~2 MB | Full verification report |
| `metadata.json` | ~300 bytes | Extraction provenance (model, hashes) |
| `embeddings.json` | ~230 bytes | Embedding metadata |
| `vectors.bin` | 29 MB | 2,364 × 3,072 float32 embedding vectors |

### Try it

```bash
# Summary
congress-approp summary --dir examples/hr4366

# All appropriations in Division A (MilCon-VA)
congress-approp search --dir examples/hr4366 --type appropriation --division A

# Rescissions over $1 billion
congress-approp search --dir examples/hr4366 --type rescission --min-dollars 1000000000

# Everything about the FBI
congress-approp search --dir examples/hr4366 --account "Federal Bureau of Investigation"

# Budget authority by department
congress-approp summary --dir examples/hr4366 --by-agency

# Full audit
congress-approp audit --dir examples/hr4366
```

---

## H.R. 5860 — The FY2024 Continuing Resolution

### What it is

The Continuing Appropriations Act, 2024 is a **continuing resolution (CR)** — temporary legislation that funded the federal government at FY2023 rates while Congress finished negotiating the full-year omnibus. It was enacted on November 16, 2023, about seven weeks into FY2024 (which started October 1).

The CR's core mechanism (SEC. 101) says: fund everything at last year's level. But 13 specific programs got different treatment through **CR substitutions** (anomalies) — provisions that substitute one dollar amount for another, setting a different level than the default prior-year rate.

### Why it matters

CRs are politically significant because the anomalies reveal congressional priorities — which programs Congress chose to fund above or below the default rate. The tool extracts these as structured data with both the new and old amounts, making analysis straightforward.

CRs also have a very different provision profile than omnibus bills: dominated by riders and mandatory spending extensions rather than new appropriations. This tests the tool's ability to handle diverse provision types.

### Provision type breakdown

| Type | Count | Percentage |
|------|-------|-----------|
| `rider` | 49 | 37.7% |
| `mandatory_spending_extension` | 44 | 33.8% |
| `cr_substitution` | 13 | 10.0% |
| `other` | 12 | 9.2% |
| `appropriation` | 5 | 3.8% |
| `limitation` | 4 | 3.1% |
| `directive` | 2 | 1.5% |
| `continuing_resolution_baseline` | 1 | 0.8% |
| **Total** | **130** | **100%** |

### The 13 CR substitutions

These are the programs where Congress set a specific funding level instead of continuing at the prior-year rate:

| Account | New Amount | Old Amount | Delta | Change |
|---------|-----------|-----------|-------|--------|
| Bilateral Econ. Assistance—Migration and Refugee Assistance | $915,048,000 | $1,535,048,000 | -$620,000,000 | -40.4% |
| *(section 521(d)(1) reference)* | $122,572,000 | $705,768,000 | -$583,196,000 | -82.6% |
| Bilateral Econ. Assistance—International Disaster Assistance | $637,902,000 | $937,902,000 | -$300,000,000 | -32.0% |
| Int'l Security Assistance—Narcotics Control | $74,996,000 | $374,996,000 | -$300,000,000 | -80.0% |
| Rural Utilities Service—Rural Water | $60,000,000 | $325,000,000 | -$265,000,000 | -81.5% |
| NSF—Research and Related Activities | $608,162,000 | $818,162,000 | -$210,000,000 | -25.7% |
| NSF—STEM Education | $92,000,000 | $217,000,000 | -$125,000,000 | -57.6% |
| State Dept—Diplomatic Programs | $87,054,000 | $147,054,000 | -$60,000,000 | -40.8% |
| Rural Housing Service—Community Facilities | $25,300,000 | $75,300,000 | -$50,000,000 | -66.4% |
| DOT—FAA Facilities and Equipment | $2,174,200,000 | $2,221,200,000 | -$47,000,000 | -2.1% |
| NOAA—Operations, Research, and Facilities | $42,000,000 | $62,000,000 | -$20,000,000 | -32.3% |
| DOT—FAA Facilities and Equipment | $617,000,000 | $570,000,000 | +$47,000,000 | +8.2% |
| OPM—Salaries and Expenses | $219,076,000 | $190,784,000 | +$28,292,000 | +14.8% |

Eleven of thirteen substitutions are cuts. Only OPM Salaries and one FAA account received increases. All 13 pairs are fully verified — both the new and old dollar amounts were found in the source bill text.

### The $16 billion FEMA appropriation

The CR's $16 billion budget authority comes primarily from SEC. 129, which appropriated $16 billion for the Federal Emergency Management Agency Disaster Relief Fund — a standalone emergency appropriation outside the CR's baseline mechanism. This is the largest single appropriation in the CR.

### Verification metrics

| Metric | Value |
|--------|-------|
| Dollar amounts verified (unique position) | 33 |
| Dollar amounts not found | **0** |
| Dollar amounts ambiguous (multiple positions) | 2 |
| Raw text exact match | 102 (78.5%) |
| Raw text normalized match | 12 (9.2%) |
| Raw text no match | 16 (12.3%) |
| Coverage | 61.1% |

The lower coverage (61.1%) is expected for a CR — most dollar strings in the text are references to prior-year appropriations acts, not new provisions. The 16 "no match" raw text provisions are riders and mandatory spending extensions that amend existing statutes, where the LLM slightly reformatted section references.

### Try it

```bash
# Summary
congress-approp summary --dir examples/hr5860

# All CR substitutions (table auto-adapts to show New/Old/Delta)
congress-approp search --dir examples/hr5860 --type cr_substitution

# The core CR mechanism
congress-approp search --dir examples/hr5860 --type continuing_resolution_baseline

# Mandatory programs extended
congress-approp search --dir examples/hr5860 --type mandatory_spending_extension

# Standalone appropriations (FEMA, etc.)
congress-approp search --dir examples/hr5860 --type appropriation

# Full audit
congress-approp audit --dir examples/hr5860
```

---

## H.R. 9468 — The VA Supplemental

### What it is

The Veterans Benefits Continuity and Accountability Supplemental Appropriations Act, 2024 is a **supplemental** — emergency funding enacted outside the regular annual cycle. It was passed after the VA disclosed an unexpected shortfall in its Compensation and Pensions and Readjustment Benefits accounts.

At only 7 provisions, it's the smallest bill in the example data and serves as an excellent introduction to the tool — small enough to read every provision, yet representative of real appropriations legislation.

### Why it matters

This bill tells a complete story in 7 provisions:

1. **$2,285,513,000** for Compensation and Pensions — additional funding to cover the shortfall
2. **$596,969,000** for Readjustment Benefits — additional funding for veteran readjustment
3. **SEC. 101** (rider) — establishes that these amounts are "in addition to" regular appropriations
4. **SEC. 102** (rider) — makes the funds available under normal authorities and conditions
5. **SEC. 103(a)** (directive) — requires the VA Secretary to report on corrective actions within 30 days
6. **SEC. 103(b)** (directive) — requires quarterly status reports on fund usage through September 2026
7. **SEC. 104** (directive) — requires the VA Inspector General to review the causes of the shortfall within 180 days

The two appropriations provide the money; the two riders establish the legal framework; the three directives impose accountability. This is a typical supplemental pattern — emergency funding paired with oversight requirements.

### Provision type breakdown

| Type | Count |
|------|-------|
| `directive` | 3 |
| `appropriation` | 2 |
| `rider` | 2 |
| **Total** | **7** |

### Verification metrics

| Metric | Value |
|--------|-------|
| Dollar amounts verified (unique position) | 2 |
| Dollar amounts not found | **0** |
| Dollar amounts ambiguous | 0 |
| Raw text exact match | 5 (71.4%) |
| Raw text normalized match | 0 |
| Raw text no match | 2 (28.6%) |
| Coverage | 100.0% |

**Perfect coverage** — every dollar amount in the source text is captured. The only two dollar strings in the bill ($2,285,513,000 and $596,969,000) are both verified. The 2 "no match" raw text provisions are the longer SEC. 103 directives, where the LLM truncated the excerpt.

### A teaching example

The VA Supplemental is used throughout this documentation as the primary teaching example because:

- **It's small enough to show completely.** All 7 provisions fit in a single JSON output.
- **It covers three provision types.** Appropriations, riders, and directives.
- **Both dollar amounts are unique.** No ambiguity — each amount maps to exactly one position in the source.
- **It has real-world significance.** The VA funding shortfall was a major news story in 2024.
- **It cross-references the omnibus.** The same accounts (Comp & Pensions, Readjustment Benefits) appear in H.R. 4366, enabling cross-bill matching demonstrations.

### Try it

```bash
# See all 7 provisions
congress-approp search --dir examples/hr9468

# Just the two appropriations
congress-approp search --dir examples/hr9468 --type appropriation

# The three directives (reporting requirements)
congress-approp search --dir examples/hr9468 --type directive

# Full JSON for the complete picture
congress-approp search --dir examples/hr9468 --format json

# Compare to the omnibus — see the same accounts in both
congress-approp compare --base examples/hr4366 --current examples/hr9468 --agency "Veterans"

# Find the omnibus counterpart of the Comp & Pensions provision
congress-approp search --dir examples --similar hr9468:0 --top 5

# Audit
congress-approp audit --dir examples/hr9468
```

---

## What Each Bill Directory Contains

Every bill directory in the example data has the same file structure:

```text
examples/hr9468/
├── BILLS-118hr9468enr.xml     ← Source XML from Congress.gov (enrolled version)
├── extraction.json            ← All provisions with structured fields
├── verification.json          ← Deterministic verification against source text
├── metadata.json              ← Extraction provenance (model, hashes, timestamps)
├── embeddings.json            ← Embedding metadata (model, dimensions, hashes)
└── vectors.bin                ← Binary float32 embedding vectors (3,072 dimensions)
```

> **Note:** `tokens.json` (LLM token usage) is not included in the example data because the extractions were produced during development. The `chunks/` directory is also not included — it's gitignored as local provenance.

See [Data Directory Layout](../reference/data-directory.md) for the complete file reference.

---

## Aggregate Metrics Across All Thirteen Bills

| Metric | Value |
|--------|-------|
| **Total provisions** | 8,554 |
| **Total budget authority** | $6,412,476,574,673 |
| **Total rescissions** | $84,074,524,379 |
| **Amounts NOT found in source** | **0** |
| **Raw text exact match rate** | 95.5% |
| **Advance appropriations detected** | $1.49 trillion (18% of total BA) |
| **FY2026 subcommittee coverage** | All 12 subcommittees |

The headline number: **0 dollar amounts unverifiable across 8,554 provisions from thirteen bills.** Every extracted dollar amount was found in the source bill text.

---

## Using Example Data for Development

The example data serves multiple purposes:

### As test fixtures

The integration test suite (`tests/cli_tests.rs`) runs against `examples/` and hardcodes exact budget authority totals. Any change to the example data or to the budget authority calculation logic that would alter these numbers is caught immediately.

### As documentation source

Every command example, output table, and JSON snippet in this documentation was generated from the example data. The data is the documentation's source of truth.

### As training data for understanding

If you're new to appropriations, reading through `examples/hr9468/extraction.json` (just 7 provisions) is the fastest way to understand what the tool produces. Then explore `examples/hr5860` for CR-specific patterns, and `examples/hr4366` for the full complexity of an omnibus.

### As baseline for comparison

When you extract your own bills, you can compare them to the examples:

```bash
# Compare your FY2025 omnibus to the FY2024 omnibus
congress-approp compare --base examples/hr4366 --current data/119/hr/YOUR_BILL

# Find similar provisions across fiscal years
congress-approp search --dir examples --dir data --similar hr9468:0 --top 5
```

---

## Updating Example Data

The example data is checked into the git repository and should only be updated deliberately. The update process:

1. Run extraction against the source XML: `congress-approp extract --dir examples/hrNNNN`
2. Run the audit to verify quality: `congress-approp audit --dir examples/hrNNNN`
3. Regenerate embeddings: `congress-approp embed --dir examples/hrNNNN`
4. Run the full test suite: `cargo test`
5. Verify budget authority totals match expected values
6. Update the hardcoded test values in `tests/cli_tests.rs` if totals changed (with justification)
7. Update documentation if provision counts or metrics changed

> **Caution:** LLM non-determinism means re-extraction may produce slightly different provision counts or classifications. The verification pipeline ensures dollar amounts are always correct, but provision-level details may vary. Only re-extract example data when there's a specific reason (schema change, prompt improvement, new model).

---

## Future Example Data

The goal is to eventually include all enacted appropriations bills so users can query without running the LLM extraction themselves. Planned additions:

- **FY2023 appropriations** (117th and 118th Congress bills)
- **FY2025 appropriations** (119th Congress bills, as they are enacted)
- **Defense appropriations** (the largest single bill, not covered by the current omnibus example)
- **Labor-HHS-Education** (the largest domestic bill, also not in the current examples)

Contributors who extract additional bills and verify their quality are welcome to submit them as additions to the example data.

## Next Steps

- **[Your First Query](../getting-started/first-query.md)** — start exploring the example data
- **[Accuracy Metrics](./accuracy-metrics.md)** — detailed verification breakdown
- **[Data Directory Layout](../reference/data-directory.md)** — what each file contains