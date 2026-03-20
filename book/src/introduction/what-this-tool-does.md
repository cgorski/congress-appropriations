# What This Tool Does

## The Problem

Every year, Congress passes appropriations bills authorizing roughly $1.7 trillion in discretionary spending — the money that funds federal agencies, military operations, scientific research, infrastructure, veterans' benefits, and thousands of other programs. These bills run to approximately 1,500 pages annually, published as XML on Congress.gov.

The text is public, but it's practically unsearchable at the provision level. If you want to know how much Congress appropriated for a specific program, you have three options:

1. **Read the bill yourself.** The FY2024 omnibus alone is over 1,800 pages of dense legislative text with nested cross-references, "of which" sub-allocations, and provisions scattered across twelve divisions.
2. **Read CBO cost estimates or committee reports.** These are expert summaries, but they aggregate — you get totals by title or account, not individual provisions. They also don't cover every bill type the same way.
3. **Search Congress.gov full text.** You can find keywords, but you can't filter by provision type, sort by dollar amount, or compare the same program across bills.

None of these let you ask structured questions like "show me every rescission over $10 million" or "which programs got a different amount in the continuing resolution than in the omnibus" or "find all provisions related to opioid treatment, including ones that don't use the word 'opioid.'"

## What This Tool Does

`congress-approp` turns appropriations bill text into structured, queryable, verified data:

- **Downloads enrolled bill XML** from Congress.gov via its official API — the authoritative, machine-readable source
- **Extracts every spending provision** into structured JSON using Claude, capturing account names, dollar amounts, agencies, availability periods, provision types, section references, and more
- **Verifies every dollar amount** against the source text using deterministic string matching — no LLM in the verification loop
- **Generates semantic embeddings** for meaning-based search, so you can find "Child Nutrition Programs" by searching for "school lunch programs for kids" even with zero keyword overlap
- **Provides CLI query tools** to search, compare, summarize, and audit provisions across any number of extracted bills

## The Trust Model

LLM extraction is powerful but not infallible. This tool is designed around a simple principle: **the LLM extracts once; deterministic code verifies everything.**

The verification pipeline runs after extraction and checks every claim the LLM made against the source bill text. No language model is involved in verification — it's pure string matching with tiered fallback (exact → normalized → spaceless). The result across all included example data:

| Metric | Result |
|--------|--------|
| Total provisions extracted | 11,136 |
| Dollar amounts not found in source | **0** |
| Raw text byte-identical to source | **95.6%** (2,392 of 2,501) |
| CR substitution pairs verified | 13/13 (100%) |
| Sub-allocations correctly excluded from budget authority | ✓ |

Every extracted dollar amount can be traced back to a character position in the bill XML. The `audit` command shows this verification breakdown for any set of bills. If a number can't be verified, it's flagged — not silently accepted.

The remaining 4.4% of provisions where `raw_text` isn't a byte-identical substring are typically cases where the LLM truncated a very long provision or normalized whitespace. The dollar amounts in those provisions are still independently verified.

## What's Included

The tool ships with fourteen bills from the 118th and 119th Congresses (FY2024–FY2026), covering all major appropriations bill types:

| Bill | Title | Classification | Provisions | Budget Authority |
|------|-------|----------------|------------|------------------|
| H.R. 4366 | Consolidated Appropriations Act, 2024 | Omnibus | 2,364 | $846,137,099,554 |
| H.R. 5860 | Continuing Appropriations Act, 2024 and Other Extensions Act | Continuing Resolution | 130 | $16,000,000,000 |
| H.R. 9468 | Veterans Benefits Continuity and Accountability Supplemental Appropriations Act, 2024 | Supplemental | 7 | $2,882,482,000 |

Each bill directory includes the source XML, extracted provisions (`extraction.json`), verification report (`verification.json`), and pre-computed embeddings. No API keys are required to query this data.

## Five Things You Can Do Right Now

All of these work immediately with the included example data — no API keys needed.

**1. See budget totals for all included bills:**

```bash
congress-approp summary --dir data
```

Shows each bill's provision count, gross budget authority, rescissions, and net budget authority in a formatted table.

**2. Search all appropriations provisions:**

```bash
congress-approp search --dir data --type appropriation
```

Lists every appropriation-type provision across all three bills with account name, amount, division, and agency.

**3. Find FEMA funding:**

```bash
congress-approp search --dir data --keyword "Federal Emergency Management"
```

Searches provision text for any mention of FEMA across all bills.

**4. See what the continuing resolution changed:**

```bash
congress-approp search --dir data/118-hr5860 --type cr_substitution
```

Shows the 13 "anomalies" — programs where the CR set a different funding level instead of continuing at the prior-year rate.

**5. Audit verification status:**

```bash
congress-approp audit --dir data
```

Displays a detailed verification breakdown for each bill: how many dollar amounts were verified, how many raw text excerpts matched the source, and the completeness coverage metric.