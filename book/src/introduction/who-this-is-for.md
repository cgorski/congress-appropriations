# Who This Is For

`congress-approp` is built for anyone who needs to work with the details of federal appropriations bills — not just the headline numbers, but the individual provisions. This chapter describes five audiences and how each can get the most out of the tool.

---

## Journalists & Policy Researchers

**What you'd use this for:**

- **Fact-checking spending claims.** A press release says "Congress cut Program X by 15%." You can pull up every provision mentioning that program, compare the dollar amounts to the prior year's bill, and confirm or refute the claim against the enrolled bill text — not a summary or a committee report, but the law itself.
- **Comparing spending across fiscal years.** "How did THUD funding change from FY2024 to FY2026?" Use `compare --base-fy 2024 --current-fy 2026 --subcommittee thud` and get a per-account comparison: Tenant-Based Rental Assistance up $6.1B (+18.7%), Capital Investment Grants down $505M. No need to know which bills or divisions to look at — the tool resolves that automatically.
- **Finding provisions by topic.** You're writing a story about opioid treatment funding. Semantic search finds relevant provisions even when the bill text says "Substance Use Treatment and Prevention" instead of "opioid." Combine with `--fy 2026 --subcommittee labor-hhs` to scope results to a specific year and jurisdiction.
- **Separating advance from current-year spending.** 79.5% of MilCon-VA budget authority is advance appropriations for the next fiscal year. Without `--show-advance`, a reporter comparing year-over-year VA spending would be off by hundreds of billions of dollars. The tool flags this automatically.
- **Tracing a program across all bills.** Use `relate 118-hr9468:0 --fy-timeline` to see VA Compensation and Pensions across FY2024–FY2026, with current/advance/supplemental split per year and links to every matching provision.

**Start here:** [Getting Started](../getting-started/installation.md) → [Find Spending on a Topic](../tutorials/find-spending-on-topic.md) → [Compare Two Bills](../tutorials/compare-two-bills.md) → [Enrich Bills with Metadata](../how-to/enrich-data.md)

**API keys needed:** None for querying pre-extracted example data (including FY filtering, subcommittee scoping, advance splits, and relate). `OPENAI_API_KEY` if you want semantic (meaning-based) search. `CONGRESS_API_KEY` + `ANTHROPIC_API_KEY` if you want to download and extract additional bills yourself.

---

## Congressional Staffers & Analysts

**What you'd use this for:**

- **Tracking program funding across bills.** Use `relate` to trace a specific account — say, VA Compensation and Pensions — across all 14 bills with a fiscal year timeline showing the current-year, advance, and supplemental split. Save the matches as persistent links with `link accept` so you can reuse them in future comparisons.
- **Subcommittee-level analysis.** "What's the FY2026 Defense budget?" Use `summary --fy 2026 --subcommittee defense` and get $836B in budget authority from H.R. 7148 Division A. The tool maps division letters to canonical jurisdictions automatically — Division A means Defense in H.R. 7148 but CJS in H.R. 6938.
- **Identifying CR anomalies.** Continuing resolutions fund the government at prior-year rates *except* for specific anomalies. The tool extracts every `cr_substitution` as structured data so you can see exactly which programs got different treatment: `congress-approp search --dir data/118-hr5860 --type cr_substitution`.
- **Enriched bill classifications.** The tool distinguishes omnibus (5+ subcommittees), minibus (2–4), full-year CR with appropriations (like H.R. 1968 with $1.786T in appropriations alongside a CR mechanism), and supplementals — not just the raw LLM classification.
- **Exporting for briefings and spreadsheets.** Every query command supports `--format csv` output. Pipe it to a file and open it in Excel: `congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir data --format csv > thud_compare.csv`.

**Start here:** [Getting Started](../getting-started/installation.md) → [Compare Two Bills](../tutorials/compare-two-bills.md) → [Enrich Bills with Metadata](../how-to/enrich-data.md) → [Track a Program Across Bills](../tutorials/track-program-across-bills.md)

**API keys needed:** None for querying pre-extracted data (including FY filtering, subcommittee scoping, advance splits, relate, and link management). Most staffers won't need to run extractions themselves — the included example data covers 13 enacted bills across FY2024–FY2026.

---

## Data Scientists & Developers

**What you'd use this for:**

- **Building dashboards and visualizations.** The `--format json` and `--format jsonl` output modes give you machine-readable provision data ready for ingestion into dashboards, notebooks, or databases. Every provision includes structured fields for amount, agency, account, division, section, provision type, and more.
- **Integrating into data pipelines.** `congress-approp` is both a CLI tool and a Rust library (`congress_appropriations`). You can call it from scripts via the CLI or embed it directly in Rust projects via the library API. The JSON schema is stable within major versions.
- **Extending with new provision types or analysis.** The extraction schema supports 11 provision types today. If you need to capture something new — say, a specific category of earmark or a new kind of spending limitation — the [Adding a New Provision Type](../contributing/new-provision-type.md) guide walks you through it.

**Start here:** [Getting Started](../getting-started/installation.md) → [Export Data for Spreadsheets and Scripts](../tutorials/export-data.md) → [Use the Library API from Rust](../how-to/library-api.md) → [Architecture Overview](../contributing/architecture.md)

**API keys needed:** Depends on your workflow. None for querying existing extractions. `OPENAI_API_KEY` for generating embeddings (semantic search). `CONGRESS_API_KEY` + `ANTHROPIC_API_KEY` for downloading and extracting new bills.

---

## Auditors & Oversight Staff

**What you'd use this for:**

- **Validating extracted numbers.** The `audit` command gives you a per-bill breakdown of verification status: how many dollar amounts were found in the source text, how many raw text excerpts matched byte-for-byte, and a completeness metric showing what percentage of dollar strings in the source were accounted for. Across the included dataset, 99.995% of dollar amounts are verified against the source text. See [Accuracy Metrics](../appendix/accuracy-metrics.md) for the full breakdown.
- **Assessing extraction completeness.** The verification report flags any dollar amount that appears in the source XML but isn't captured by an extracted provision. A completeness percentage below 100% doesn't necessarily indicate a missed provision — many dollar strings in bill text are statutory cross-references, loan guarantee ceilings, or old amounts being struck by amendments — but it gives you a starting point for investigation.
- **Tracing numbers to source.** Every verified dollar amount includes a character position in the source text. Every provision includes `raw_text` that can be matched against the bill XML. You can independently confirm any number the tool reports by opening the source file and checking the indicated position.

**Start here:** [Getting Started](../getting-started/installation.md) → [Verify Extraction Accuracy](../how-to/verify-accuracy.md) → [LLM Reliability and Guardrails](../explanation/llm-reliability.md)

**API keys needed:** None. All verification and audit operations work entirely offline against already-extracted data.

---

## Contributors

**What you'd use this for:**

- **Adding features.** The tool is open source under MIT/Apache-2.0. Whether you want to add a new CLI subcommand, support a new bill format, or improve the extraction prompt, the contributor guides walk you through the codebase and conventions.
- **Fixing bugs.** The [Testing Strategy](../contributing/testing.md) chapter explains how the test suite is structured — including golden-file tests against the example bills — so you can reproduce issues and verify fixes.
- **Understanding the architecture.** The [Architecture Overview](../contributing/architecture.md) and [Code Map](../contributing/code-map.md) chapters explain how the pipeline stages connect, where each module lives, and how data flows from XML download through LLM extraction and verification to query output.

**Start here:** [Architecture Overview](../contributing/architecture.md) → [Code Map](../contributing/code-map.md) → [Testing Strategy](../contributing/testing.md) → [Style Guide and Conventions](../contributing/style-guide.md)

**API keys needed:** `CONGRESS_API_KEY` + `ANTHROPIC_API_KEY` if you're working on download or extraction features. `OPENAI_API_KEY` if you're working on embedding or semantic search features. None if you're working on query, verification, or CLI features — the example data is sufficient.