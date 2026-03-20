# Find How Much Congress Spent on a Topic

> **You will need:** `congress-approp` installed, access to the `data/` directory. For semantic search: `OPENAI_API_KEY`.
>
> **You will learn:** Three ways to find spending provisions — by account name, by keyword, and by semantic meaning — and when to use each one.

Imagine your editor asks: *"How much did Congress give the VA in the FY2024 omnibus?"* Or a constituent writes: *"How much federal money goes to school lunch programs?"* This tutorial walks through how to answer questions like these, starting with the simplest approach and building to the most powerful.

## Start with the Agency Rollup

If your question is about an entire department, the fastest answer is the by-agency summary:

```bash
congress-approp summary --dir data --by-agency
```

This prints the standard bill summary table, followed by a second table breaking down budget authority by parent department. Here's the top of that second table:

```text
┌─────────────────────────────────────────────────────┬─────────────────┬─────────────────┬────────────┐
│ Department                                           ┆ Budget Auth ($) ┆ Rescissions ($) ┆ Provisions │
╞═════════════════════════════════════════════════════╪═════════════════╪═════════════════╪════════════╡
│ Department of Veterans Affairs                       ┆ 343,238,707,982 ┆   9,799,155,560 ┆         51 │
│ Department of Agriculture                            ┆ 187,748,124,000 ┆     351,891,000 ┆        266 │
│ Department of Housing and Urban Development          ┆  75,743,762,466 ┆      85,000,000 ┆        116 │
│ Department of Energy                                 ┆  50,776,281,000 ┆               0 ┆         62 │
│ Department of Justice                                ┆  37,960,158,000 ┆   1,158,272,000 ┆        186 │
│ ...                                                                                                    │
└─────────────────────────────────────────────────────┴─────────────────┴─────────────────┴────────────┘
```

So the answer to "how much did the VA get?" is approximately $343 billion in budget authority across all three bills, with $9.8 billion in rescissions.

**Important caveat:** This total includes mandatory spending programs that appear as appropriation lines in the bill text. VA's Compensation and Pensions account alone is $197 billion — that's a mandatory entitlement, not discretionary spending, even though it appears in the appropriations bill. See [Why the Numbers Might Not Match Headlines](../explanation/numbers-vs-headlines.md) for more on this distinction.

## Search by Account Name

When you know the program's official name (or part of it), `--account` is the most precise filter. It matches against the structured `account_name` field:

```bash
congress-approp search --dir data --account "Child Nutrition"
```

```text
┌───┬───────────┬───────────────┬─────────────────────────────────────────────┬────────────────┬─────────┬─────┐
│ $ ┆ Bill      ┆ Type          ┆ Description / Account                       ┆     Amount ($) ┆ Section ┆ Div │
╞═══╪═══════════╪═══════════════╪═════════════════════════════════════════════╪════════════════╪═════════╪═════╡
│ ✓ ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆ 33,266,226,000 ┆         ┆ B   │
│ ✓ ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆     18,004,000 ┆         ┆ B   │
│ ✓ ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆     21,005,000 ┆         ┆ B   │
│ ✓ ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆      5,000,000 ┆         ┆ B   │
│ ≈ ┆ H.R. 4366 ┆ limitation    ┆ Child Nutrition Programs                    ┆        500,000 ┆         ┆ B   │
│ ≈ ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆     10,000,000 ┆         ┆ B   │
│ ≈ ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆      1,000,000 ┆         ┆ B   │
│ ✓ ┆ H.R. 4366 ┆ appropriation ┆ McGovern-Dole International Food for Educ…  ┆    240,000,000 ┆         ┆ B   │
│ ≈ ┆ H.R. 4366 ┆ limitation    ┆ McGovern-Dole International Food for Educ…  ┆     24,000,000 ┆         ┆ B   │
└───┴───────────┴───────────────┴─────────────────────────────────────────────┴────────────────┴─────────┴─────┘
```

The top result — $33,266,226,000 — is the top-level appropriation for Child Nutrition Programs. The smaller amounts below it are sub-allocations ("of which $18,004,000 shall be for...") and proviso amounts that break down how the top-level figure is to be spent. These sub-allocations have `reference_amount` semantics and are **not** counted again in the budget authority total — no double-counting.

The McGovern-Dole account also matches because it has "Child Nutrition" in its full name.

### When to use `--account` vs. `--keyword`

- **`--account`** matches against the structured `account_name` field extracted by the LLM — the official name of the appropriations account.
- **`--keyword`** searches the full `raw_text` field — the actual bill language.

Sometimes the account name doesn't contain the term you're looking for, but the bill text does. Other times, the bill text doesn't mention a term that is in the account name. Use both when you want to be thorough.

## Search by Keyword in Bill Text

The `--keyword` flag searches the `raw_text` field — the excerpt of actual bill language stored with each provision. This finds provisions where the term appears anywhere in the source text, regardless of account name:

```bash
congress-approp search --dir data --keyword "Federal Emergency Management"
```

```text
┌───┬───────────┬───────────────┬───────────────────────────────────────────────┬────────────────┬──────────┬─────┐
│ $ ┆ Bill      ┆ Type          ┆ Description / Account                         ┆     Amount ($) ┆ Section  ┆ Div │
╞═══╪═══════════╪═══════════════╪═══════════════════════════════════════════════╪════════════════╪══════════╪═════╡
│   ┆ H.R. 5860 ┆ other         ┆ Allows FEMA Disaster Relief Fund to be appor… ┆              — ┆ SEC. 128 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ appropriation ┆ Federal Emergency Management Agency—Disast…   ┆ 16,000,000,000 ┆ SEC. 129 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ appropriation ┆ Office of the Inspector General—Operations…   ┆      2,000,000 ┆ SEC. 129 ┆ A   │
└───┴───────────┴───────────────┴───────────────────────────────────────────────┴────────────────┴──────────┴─────┘
3 provisions found
```

This found three provisions: the $16B FEMA Disaster Relief Fund appropriation, a $2M Inspector General appropriation, and a non-dollar provision about how the fund can be apportioned. All three are in the continuing resolution (H.R. 5860), not the omnibus — because FEMA's regular funding falls under the Homeland Security appropriations bill, which isn't one of the divisions included in this particular omnibus.

### Useful keywords for exploring

Here are some keywords that surface interesting provisions in the example data:

| Keyword | What It Finds |
|---------|---------------|
| `"notwithstanding"` | Provisions that override other legal requirements — often important policy exceptions |
| `"is hereby rescinded"` | Rescission provisions (also findable with `--type rescission`) |
| `"shall submit a report"` | Reporting requirements and directives |
| `"not to exceed"` | Caps and limitations on spending |
| `"transfer"` | Fund transfer authorities |
| `"Veterans Affairs"` | All VA-related provisions across all bills |

### Combining filters

All search filters are combined with AND logic. Every provision in the result must match every filter you specify:

```bash
# Appropriations over $1 billion in Division A (MilCon-VA)
congress-approp search --dir data --type appropriation --division A --min-dollars 1000000000

# Rescissions from the Department of Justice
congress-approp search --dir data --type rescission --agency "Justice"

# Directives in the VA supplemental
congress-approp search --dir data/hr9468 --type directive
```

## Search by Meaning (Semantic Search)

Keyword search has a fundamental limitation: it only finds provisions that use the exact words you search for. If you search for "school lunch" but the bill says "Child Nutrition Programs," keyword search returns nothing.

Semantic search solves this. It uses embedding vectors to understand the *meaning* of your query and rank provisions by conceptual similarity — even when the words don't overlap at all.

**Prerequisites:** Semantic search requires `OPENAI_API_KEY` (to embed your query text at search time) and pre-computed embeddings for the bills you're searching. The included example data has pre-computed embeddings, so you just need the API key.

```bash
export OPENAI_API_KEY="your-key-here"
congress-approp search --dir data --semantic "school lunch programs for kids" --top 5
```

```text
┌──────┬───────────┬───────────────┬─────────────────────────────────────────────┬────────────────┬─────┐
│ Sim  ┆ Bill      ┆ Type          ┆ Description / Account                       ┆     Amount ($) ┆ Div │
╞══════╪═══════════╪═══════════════╪═════════════════════════════════════════════╪════════════════╪═════╡
│ 0.51 ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆ 33,266,226,000 ┆ B   │
│ 0.46 ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆     10,000,000 ┆ B   │
│ 0.45 ┆ H.R. 4366 ┆ rider         ┆ Pilot project grant recipients shall be r…  ┆              — ┆ B   │
│ 0.45 ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆     18,004,000 ┆ B   │
│ 0.44 ┆ H.R. 4366 ┆ appropriation ┆ Child Nutrition Programs                    ┆      5,000,000 ┆ B   │
└──────┴───────────┴───────────────┴─────────────────────────────────────────────┴────────────────┴─────┘
5 provisions found
```

The query "school lunch programs for kids" has **zero keyword overlap** with "Child Nutrition Programs" — yet it's the top result at 0.51 similarity. The embeddings understand that these concepts are about the same thing.

### More semantic search examples

Try these queries against the example data to get a feel for how semantic search finds provisions that keyword search would miss:

```bash
# "Fixing roads and bridges" → finds Highway Infrastructure Programs, Federal-Aid Highways
congress-approp search --dir data --semantic "money for fixing roads and bridges" --top 5

# "Space exploration" → finds NASA Exploration, Space Operations, Space Technology
congress-approp search --dir data --semantic "space exploration" --top 5

# "Clean energy" → finds Energy Efficiency and Renewable Energy, Nuclear Energy
congress-approp search --dir data --semantic "clean energy research" --top 5
```

### Combining semantic search with filters

You can narrow semantic results with hard filters. For example, find only appropriation-type provisions about clean energy with at least $100 million:

```bash
congress-approp search --dir data --semantic "clean energy" --type appropriation --min-dollars 100000000 --top 10
```

The filters are applied first (hard constraints that must match), then the remaining provisions are ranked by semantic similarity.

### When semantic search doesn't help

Semantic search is not always the right tool:

- **Exact account name lookup:** If you know the account name, use `--account`. It's faster, deterministic, and doesn't require an API key.
- **No conceptual match:** If nothing in the dataset relates to your query, similarity scores will be low (below 0.40). Low scores are an honest answer — the tool isn't hallucinating relevance.
- **Provision type distinction:** Embeddings don't strongly encode whether something is a rider vs. an appropriation. If you need only appropriations, add `--type appropriation` as a hard filter.

## Get the Full Details in JSON

Once you've found interesting provisions in the table view, switch to JSON to see every field:

```bash
congress-approp search --dir data --account "Child Nutrition" --type appropriation --format json
```

This returns the full structured data for each matching provision, including fields the table truncates: `raw_text` (the full excerpt), `semantics`, `detail_level`, `agency`, `division`, `notes`, `cross_references`, and more.

For example, the top-level Child Nutrition Programs appropriation includes:

```json
{
  "account_name": "Child Nutrition Programs",
  "agency": "Department of Agriculture",
  "bill": "H.R. 4366",
  "dollars": 33266226000,
  "semantics": "new_budget_authority",
  "detail_level": "top_level",
  "division": "B",
  "provision_type": "appropriation",
  "quality": "strong",
  "amount_status": "found",
  "match_tier": "exact",
  "raw_text": "For necessary expenses of the Food and Nutrition Service..."
}
```

Key fields to check:

- **`semantics`**: `new_budget_authority` means this counts toward the budget authority total. `reference_amount` means it's a sub-allocation or contextual amount.
- **`detail_level`**: `top_level` is the main account appropriation. `sub_allocation` is an "of which" breakdown. `line_item` is a numbered item within a section.
- **`quality`**: `strong` means the dollar amount was verified and the raw text matched the source. `moderate` or `weak` means something didn't check out as well.

## Cross-Check Against the Source

For any provision you plan to cite, you can verify it directly against the bill XML. The `raw_text` field contains the excerpt, and the `text_as_written` dollar string can be searched in the source file:

```bash
# Find the dollar string in the source XML
grep '33,266,226,000' data/118-hr4366/BILLS-118hr4366enr.xml
```

If the string is found (which it will be — the audit confirms this), you know the extraction is accurate. For a full verification procedure, see [Verify Extraction Accuracy](../how-to/verify-accuracy.md).

## Export for Further Analysis

Once you've identified the provisions you care about, export them for further work:

```bash
# CSV for Excel or Google Sheets
congress-approp search --dir data --account "Child Nutrition" --format csv > child_nutrition.csv

# JSON for Python, R, or jq
congress-approp search --dir data --agency "Veterans" --type appropriation --format json > va_appropriations.json
```

See [Export Data for Spreadsheets and Scripts](./export-data.md) for detailed recipes.

## Summary: Which Search Method to Use

| Method | Flag | Best For | Limitations |
|--------|------|----------|-------------|
| **Account name** | `--account` | Known program names | Only matches the `account_name` field |
| **Keyword** | `--keyword` | Terms that appear in bill text | Only finds exact word matches |
| **Agency** | `--agency` | Department-level filtering | Case-insensitive substring match |
| **Semantic** | `--semantic` | Finding provisions by meaning | Requires embeddings + `OPENAI_API_KEY` |
| **Provision type** | `--type` | Filtering by category | Relies on LLM classification accuracy |
| **Division** | `--division` | Scoping to a part of an omnibus bill | Only applicable to multi-division bills |
| **Dollar range** | `--min-dollars` / `--max-dollars` | Finding large or small provisions | Only filters on absolute value |

For the most thorough search, try multiple approaches. Start with `--account` or `--keyword` for precision, then use `--semantic` to catch provisions you might have missed with different terminology.

## Next Steps

- **[Compare Two Bills](./compare-two-bills.md)** — see what changed between a CR and an omnibus
- **[Track a Program Across Bills](./track-program-across-bills.md)** — follow a specific account across bills using `--similar`
- **[Use Semantic Search](./semantic-search.md)** — deeper dive into embedding-based search