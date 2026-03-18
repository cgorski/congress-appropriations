# Who This Is For

`congress-approp` is built for anyone who needs to work with the details of federal appropriations bills — not just the headline numbers, but the individual provisions. This chapter describes five audiences and how each can get the most out of the tool.

---

## Journalists & Policy Researchers

**What you'd use this for:**

- **Fact-checking spending claims.** A press release says "Congress cut Program X by 15%." You can pull up every provision mentioning that program, compare the dollar amounts to the prior year's bill, and confirm or refute the claim against the enrolled bill text — not a summary or a committee report, but the law itself.
- **Finding provisions by topic.** You're writing a story about opioid treatment funding. Semantic search finds relevant provisions even when the bill text says "Substance Use Treatment and Prevention" instead of "opioid." Run `congress-approp search --dir examples --semantic "opioid crisis drug treatment"` and get ranked results by meaning similarity.
- **Comparing bills side by side.** How did the final omnibus differ from the continuing resolution? The `compare` command shows you which programs gained or lost funding, which were added, and which were dropped entirely.

**Start here:** [Getting Started](../getting-started/installation.md) → [Find Spending on a Topic](../tutorials/find-spending-on-topic.md) → [Compare Two Bills](../tutorials/compare-two-bills.md)

**API keys needed:** None for querying pre-extracted example data. `OPENAI_API_KEY` if you want semantic (meaning-based) search. `CONGRESS_API_KEY` + `ANTHROPIC_API_KEY` if you want to download and extract additional bills yourself.

---

## Congressional Staffers & Analysts

**What you'd use this for:**

- **Tracking program funding across bills.** Follow a specific account — say, "Veterans Health Administration, Medical Services" — across the omnibus, the CR, and a supplemental to see the complete FY2024 funding picture. The `search` command with `--keyword` filters across all extracted bills at once.
- **Identifying CR anomalies.** Continuing resolutions fund the government at prior-year rates *except* for specific anomalies. The tool extracts every `cr_substitution` as structured data so you can see exactly which programs got different treatment: `congress-approp search --dir examples/hr5860 --type cr_substitution`.
- **Exporting for briefings and spreadsheets.** Every query command supports `--format csv` output. Pipe it to a file and open it in Excel for briefing materials, charts, or further analysis: `congress-approp search --dir examples --type appropriation --format csv > provisions.csv`.

**Start here:** [Getting Started](../getting-started/installation.md) → [Compare Two Bills](../tutorials/compare-two-bills.md) → [Work with CR Substitutions](../how-to/cr-substitutions.md)

**API keys needed:** None for querying pre-extracted data. Most staffers won't need to run extractions themselves — the goal is to eventually include all enacted appropriations bills as pre-extracted example data.

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

- **Validating extracted numbers.** The `audit` command gives you a per-bill breakdown of verification status: how many dollar amounts were found in the source text, how many raw text excerpts matched byte-for-byte, and a completeness metric showing what percentage of dollar strings in the source were accounted for. Across the included example data, 0 of 2,501 provisions had unverifiable dollar amounts.
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