# Congressional Appropriations Analyzer

**Turn federal spending bills into searchable, structured data.**

`congress-approp` is a Rust command-line tool that downloads U.S. federal appropriations bills from Congress.gov, extracts every spending provision into structured JSON using Claude, verifies each dollar amount against the source bill text, and gives you tools to search, compare, summarize, and audit the results. No more hunting through 1,500 pages of legislative text to find out how much Congress appropriated for a program.

> **Trust callout:** Across 2,501 provisions extracted from three FY2024 bills, every single dollar amount was found verbatim in the source bill text. Zero unverifiable amounts. The LLM extracts; deterministic code verifies.

## What's Included

This book ships with **three pre-extracted example bills** covering the major appropriations bill types. You don't need any API keys to explore them — just install the tool and start querying:

| Bill | Description | Provisions | Budget Authority |
|------|-------------|------------|------------------|
| H.R. 4366 | Consolidated Appropriations Act, 2024 (omnibus) | 2,364 | $846B |
| H.R. 5860 | Continuing Appropriations Act, 2024 (CR) | 130 | $16B |
| H.R. 9468 | Veterans Benefits Supplemental, 2024 | 7 | $2.9B |

## Quick Example

```bash
congress-approp summary --dir examples
```

```text
┌───────────┬───────────────────────┬────────────┬─────────────────┬─────────────────┬─────────────────┐
│ Bill      ┆ Classification        ┆ Provisions ┆ Budget Auth ($) ┆ Rescissions ($) ┆      Net BA ($) │
╞═══════════╪═══════════════════════╪════════════╪═════════════════╪═════════════════╪═════════════════╡
│ H.R. 4366 ┆ Omnibus               ┆       2364 ┆ 846,137,099,554 ┆  24,659,349,709 ┆ 821,477,749,845 │
│ H.R. 5860 ┆ Continuing Resolution ┆        130 ┆  16,000,000,000 ┆               0 ┆  16,000,000,000 │
│ H.R. 9468 ┆ Supplemental          ┆          7 ┆   2,882,482,000 ┆               0 ┆   2,882,482,000 │
│ TOTAL     ┆                       ┆       2501 ┆ 865,019,581,554 ┆  24,659,349,709 ┆ 840,360,231,845 │
└───────────┴───────────────────────┴────────────┴─────────────────┴─────────────────┴─────────────────┘

0 dollar amounts unverified across all bills. Run `congress-approp audit` for detailed verification.
```

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

This documentation covers **congress-approp v3.2.x**.

- **GitHub:** <https://github.com/cgorski/congress-appropriations>
- **crates.io:** <https://crates.io/crates/congress-appropriations>