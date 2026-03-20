# Congressional Appropriations Analyzer

**Turn federal spending bills into searchable, structured data.**

`congress-approp` is a Rust command-line tool that downloads U.S. federal appropriations bills from Congress.gov, extracts every spending provision into structured JSON using Claude, verifies each dollar amount against the source bill text, and gives you tools to search, compare, summarize, and audit the results. No more hunting through 1,500 pages of legislative text to find out how much Congress appropriated for a program.

> **Trust callout:** Across 11,136 provisions extracted from fourteen bills, every single dollar amount was found verbatim in the source bill text. Zero unverifiable amounts. The LLM extracts; deterministic code verifies.

## What's Included

This book ships with **fourteen bills, continuing resolutions, supplementals, and authorizations. All twelve appropriations subcommittees are represented for FY2026. You don't need any API keys to explore them — just install the tool and start querying.

### 118th Congress (FY2024/FY2025)

| Bill | Classification | Subcommittees | Provisions | Budget Auth |
|------|---------------|---------------|-----------|------------|
| H.R. 4366 | Omnibus | MilCon-VA, Ag, CJS, E&W, Interior, THUD | 2,364 | $846B |
| H.R. 5860 | Continuing Resolution | (all, at prior-year rates) | 130 | $16B |
| H.R. 9468 | Supplemental | VA | 7 | $2.9B |
| H.R. 815 | Supplemental | Defense, State (Ukraine/Israel/Taiwan) | 303 | $95B |
| H.R. 2872 | Continuing Resolution | (further CR) | 31 | $0 |
| H.R. 6363 | Continuing Resolution | (further CR + extensions) | 74 | ~$0 |
| H.R. 7463 | Continuing Resolution | (CR extension) | 10 | $0 |
| H.R. 9747 | Continuing Resolution | (CR + extensions, FY2025) | 114 | $383M |
| S. 870 | Authorization | Fire administration | 49 | $0 |

### 119th Congress (FY2025/FY2026)

| Bill | Classification | Subcommittees | Provisions | Budget Auth |
|------|---------------|---------------|-----------|------------|
| H.R. 1968 | Full-Year CR with Appropriations | Defense, Homeland, Labor-HHS, others | 526 | $1,786B |
| H.R. 5371 | Minibus | CR + Ag + LegBranch + MilCon-VA | 1,048 | $681B |
| H.R. 6938 | Minibus | CJS + Energy-Water + Interior | 1,061 | $196B |
| H.R. 7148 | Omnibus | Defense + Labor-HHS + THUD + FinServ + State | 2,837 | $2,788B |

**Totals:** 11,136 provisions, $6.4 trillion in budget authority, 0 unverifiable dollar amounts.

## What Can You Do?

**"How did THUD funding change from FY2024 to FY2026?"**

```bash
congress-approp enrich --dir examples                    # Generate metadata (once, no API key)
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir examples
```

82 accounts matched across fiscal years — Tenant-Based Rental Assistance up $6.1B (+18.7%), Transit Formula Grants reclassified at $14.6B, Capital Investment Grants down $505M.

**"What's the FY2026 MilCon-VA budget, and how much is advance?"**

```bash
congress-approp summary --dir examples --fy 2026 --subcommittee milcon-va --show-advance
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
congress-approp relate hr9468:0 --dir examples --fy-timeline
```

Shows every matching provision across FY2024–FY2026 with current/advance/supplemental split, plus deterministic hashes you can save as persistent links for future comparisons.

**"Find everything about FEMA disaster relief"**

```bash
congress-approp search --dir examples --semantic "FEMA disaster relief funding" --top 5
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

- **[Getting Started](./getting-started/installation.md)** — Install the tool and run your first query in under five minutes. Start here if you want hands-on immediately.
- **Getting to Know the Tool** — Background reading on [what this tool does](./introduction/what-this-tool-does.md), [who it's for](./introduction/who-this-is-for.md), and a [primer on how federal appropriations work](./introduction/appropriations-primer.md) if you're new to the domain.
- **[Tutorials](./tutorials/find-spending-on-topic.md)** — Step-by-step walkthroughs for common tasks: finding spending on a topic, comparing bills, tracking programs, exporting data, and more.
- **[How-To Guides](./how-to/download-bills.md)** — Task-oriented recipes for specific operations like downloading bills, extracting provisions, and generating embeddings.
- **[Explanation](./explanation/pipeline.md)** — Deep dives into how the extraction pipeline, verification, semantic search, provision types, and budget authority calculation work under the hood.
- **[Reference](./reference/cli.md)** — Lookup material: CLI commands, JSON field definitions, provision types, environment variables, data directory layout, and the glossary.
- **[For Contributors](./contributing/architecture.md)** — Architecture overview, code map, and guides for adding new provision types, commands, and tests.

## Version

This documentation covers **congress-approp v4.0.x**.

- **GitHub:** <https://github.com/cgorski/congress-appropriations>
- **crates.io:** <https://crates.io/crates/congress-appropriations>