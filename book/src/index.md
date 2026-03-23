# Congressional Appropriations Analyzer

`congress-approp` is a Rust CLI tool and library that downloads U.S. federal appropriations bills from Congress.gov, extracts every spending provision into structured JSON using Claude, and verifies each dollar amount against the source text. The included dataset covers 32 enacted bills across FY2019–FY2026 with 34,568 provisions and $21.5 trillion in budget authority.

Dollar amounts are verified by deterministic string matching against the enrolled bill text — no LLM in the verification loop. 99.995% of extracted dollar amounts are confirmed present in the source (18,583 of 18,584). Every provision carries a `source_span` with exact byte offsets into the enrolled bill for independent verification.

> **Jump straight to working examples:** [Recipes & Demos](./tutorials/cookbook.md) — track any federal account across fiscal years, compare subcommittees with inflation adjustment, load the data in Python, and more. No API keys needed.

## What's Included

This book ships with **32 enacted appropriations bills** across 4 congresses (116th–119th), covering FY2019 through FY2026. All twelve appropriations subcommittees are represented for FY2020–FY2024 and FY2026. You don't need any API keys to explore them — just install the tool and start querying.

### 116th Congress (FY2019–FY2021) — 11 bills

| Bill | Classification | Provisions | Budget Auth |
|------|---------------|-----------|------------|
| H.R. 1865 | Omnibus (FY2020, 8 subcommittees) | 3,338 | $1,710B |
| H.R. 1158 | Minibus (FY2020, Defense + CJS + FinServ + Homeland) | 1,519 | $887B |
| H.R. 133 | Omnibus (FY2021, all 12 subcommittees) | 6,739 | $3,378B |
| H.R. 2157 | Supplemental (FY2019, disaster relief) | 116 | $19B |
| H.R. 3401 | Supplemental (FY2019, humanitarian) | 55 | $5B |
| H.R. 6074 | Supplemental (FY2020, COVID preparedness) | 55 | $8B |
| + 5 CRs | Continuing resolutions | 351 | $31B |

### 117th Congress (FY2021–FY2023) — 7 bills

| Bill | Classification | Provisions | Budget Auth |
|------|---------------|-----------|------------|
| H.R. 2471 | Omnibus (FY2022) | 5,063 | $3,031B |
| H.R. 2617 | Omnibus (FY2023) | 5,910 | $3,379B |
| H.R. 3237 | Supplemental (FY2021, Capitol security) | 47 | $2B |
| H.R. 7691 | Supplemental (FY2022, Ukraine) | 67 | $40B |
| H.R. 6833 | CR + Ukraine supplemental | 240 | $46B |
| + 2 CRs | Continuing resolutions | 37 | $0 |

### 118th Congress (FY2024/FY2025) — 10 bills

| Bill | Classification | Provisions | Budget Auth |
|------|---------------|-----------|------------|
| H.R. 4366 | Omnibus (MilCon-VA, Ag, CJS, E&W, Interior, THUD) | 2,323 | $921B |
| H.R. 2882 | Omnibus (Defense, FinServ, Homeland, Labor-HHS, LegBranch, State) | 2,608 | $2,451B |
| H.R. 815 | Supplemental (Ukraine/Israel/Taiwan) | 306 | $95B |
| H.R. 9468 | Supplemental (VA) | 7 | $3B |
| H.R. 5860 | Continuing Resolution + 13 anomalies | 136 | $16B |
| S. 870 | Authorization (Fire Admin) | 51 | $0 |
| + 4 CRs | Continuing resolutions | 233 | $0 |

### 119th Congress (FY2025/FY2026) — 4 bills

| Bill | Classification | Provisions | Budget Auth |
|------|---------------|-----------|------------|
| H.R. 7148 | Omnibus (Defense + Labor-HHS + THUD + FinServ + State) | 2,774 | $2,841B |
| H.R. 5371 | Minibus (CR + Ag + LegBranch + MilCon-VA) | 1,051 | $681B |
| H.R. 6938 | Minibus (CJS + Energy-Water + Interior) | 1,028 | $196B |
| H.R. 1968 | Full-Year CR with Appropriations (FY2025) | 514 | $1,786B |

**Totals:** 32 bills, 34,568 provisions, $21.5 trillion in budget authority, 1,051 accounts tracked by Treasury Account Symbol across FY2019–FY2026.

## What Can You Do?

**"How did THUD funding change from FY2024 to FY2026?"**

```bash
congress-approp enrich --dir data                    # Generate metadata (once, no API key)
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir data
```

82 accounts matched across fiscal years — Tenant-Based Rental Assistance up $6.1B (+18.7%), Transit Formula Grants reclassified at $14.6B, Capital Investment Grants down $505M.

**"What's the FY2026 MilCon-VA budget, and how much is advance?"**

```bash
congress-approp summary --dir data --fy 2026 --subcommittee milcon-va --show-advance
```

```text
┌───────────┬────────────────┬────────────┬─────────────────┬─────────────────┬─────────────────┬─────────────────┬─────────────────┐
│ Bill      ┆ Classification ┆ Provisions ┆     Current ($) ┆     Advance ($) ┆    Total BA ($) ┆ Rescissions ($) ┆      Net BA ($) │
╞═══════════╪════════════════╪════════════╪═════════════════╪═════════════════╪═════════════════╪═════════════════╪═════════════════╡
│ H.R. 5371 ┆ Minibus        ┆        257 ┆ 101,742,083,450 ┆ 393,689,946,000 ┆ 495,432,029,450 ┆  16,499,000,000 ┆ 478,933,029,450 │
└───────────┴────────────────┴────────────┴─────────────────┴─────────────────┴─────────────────┴─────────────────┴─────────────────┘
```

79.5% of MilCon-VA is advance appropriations for the next fiscal year — without `--show-advance`, you'd overstate current-year VA spending by $394 billion.

**"Trace VA Compensation and Pensions across all fiscal years"**

```bash
congress-approp relate 118-hr9468:0 --dir data --fy-timeline
```

Shows every matching provision across FY2024–FY2026 with current/advance/supplemental split, plus deterministic hashes you can save as persistent links for future comparisons.

**"Find everything about FEMA disaster relief"**

```bash
congress-approp search --dir data --semantic "FEMA disaster relief funding" --top 5
```

Finds FEMA provisions across 5 different bills by *meaning*, not just keywords — even when the bill text says "Federal Emergency Management Agency—Disaster Relief Fund" instead of "FEMA."

## Key Concepts

- **`enrich`** generates bill metadata offline (no API keys) — enabling fiscal year filtering, subcommittee scoping, and advance appropriation detection.
- **`--fy 2026`** filters any command to bills covering that fiscal year.
- **`--subcommittee thud`** scopes to a specific appropriations jurisdiction, resolving division letters automatically (Division D in one bill, Division F in another — both map to THUD).
- **`--show-advance`** separates current-year spending from advance appropriations (money enacted now but available in a future fiscal year). Critical for year-over-year comparisons.
- **`relate`** traces one provision across all bills with a fiscal year timeline.
- **`link suggest` / `link accept`** persist cross-bill relationships so `compare --use-links` can handle renames automatically.

## Navigating This Book

This book is organized so you can jump to whatever fits your needs:

- **[Recipes & Demos](./tutorials/cookbook.md)** — Copy-paste recipes for journalists, staffers, and data scientists. Track any program across fiscal years, compare with inflation adjustment, load JSON in Python, and more. Interactive visualizations included. **Start here if you want results fast.**
- **[Getting Started](./getting-started/installation.md)** — Install the tool and run your first query in under five minutes. Start here if you want hands-on immediately.
- **Getting to Know the Tool** — Background reading on [what this tool does](./introduction/what-this-tool-does.md), [who it's for](./introduction/who-this-is-for.md), and a [primer on how federal appropriations work](./introduction/appropriations-primer.md) if you're new to the domain.
- **[Tutorials](./tutorials/find-spending-on-topic.md)** — Step-by-step walkthroughs for common tasks: finding spending on a topic, comparing bills, tracking programs, exporting data, and more.
- **[How-To Guides](./how-to/download-bills.md)** — Task-oriented recipes for specific operations like downloading bills, extracting provisions, and generating embeddings.
- **[Explanation](./explanation/pipeline.md)** — Deep dives into how the extraction pipeline, verification, semantic search, provision types, and budget authority calculation work under the hood.
- **[Reference](./reference/cli.md)** — Lookup material: CLI commands, JSON field definitions, provision types, environment variables, data directory layout, and the glossary.
- **[For Contributors](./contributing/architecture.md)** — Architecture overview, code map, and guides for adding new provision types, commands, and tests.

## Version

This documentation covers **congress-approp v6.0.0**.

- **GitHub:** <https://github.com/cgorski/congress-appropriations>
- **crates.io:** <https://crates.io/crates/congress-appropriations>