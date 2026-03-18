# Use Semantic Search

> **You will need:** `congress-approp` installed, access to the `examples/` directory, `OPENAI_API_KEY` environment variable set.
>
> **You will learn:** How to find provisions by meaning instead of keywords, how to interpret similarity scores, how to use `--similar` for cross-bill matching, and when semantic search is (and isn't) the right tool.

Keyword search finds provisions that contain the exact words you type. Semantic search finds provisions that *mean* what you're looking for — even when the words are completely different. This is the difference between searching for "school lunch" (zero results in appropriations language) and finding "$33 billion for Child Nutrition Programs" (the actual provision that funds school lunches).

This tutorial walks through setup, real queries against the example data, and practical techniques for getting the best results.

## Prerequisites

Semantic search requires two things:

1. **Pre-computed embeddings** for the bills you want to search. The included example data already has these — you don't need to generate them.
2. **`OPENAI_API_KEY`** set in your environment. This is needed at query time to embed your search text (a single API call, ~100ms, costs fractions of a cent).

```bash
export OPENAI_API_KEY="your-key-here"
```

If you're working with your own extracted bills that don't have embeddings yet, generate them first:

```bash
congress-approp embed --dir your-data-directory
```

See [Generate Embeddings](../how-to/generate-embeddings.md) for details.

## Your First Semantic Search

Let's start with the headline example — searching for a concept using everyday language that has zero keyword overlap with the actual provision:

```bash
congress-approp search --dir examples --semantic "school lunch programs for kids" --top 5
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

Not a single word in "school lunch programs for kids" appears in "Child Nutrition Programs" — and yet it's the top result at 0.51 similarity. The embedding model understands that school lunches and child nutrition are the same concept.

Compare this to a keyword search for the same phrase:

```bash
congress-approp search --dir examples --keyword "school lunch"
```

```text
0 provisions found
```

Zero results. Keyword search can only find provisions containing the literal words "school lunch," which no provision in any of these bills does.

## Understanding the Sim Column

When you use `--semantic` or `--similar`, the table gains a **Sim** column showing the cosine similarity between your query and each provision's embedding vector. Scores range from 0 to 1:

| Score Range | What It Means | Example |
|-------------|---------------|---------|
| **> 0.80** | Nearly identical meaning — almost certainly the same program in a different bill | VA Supp "Comp & Pensions" ↔ Omnibus "Comp & Pensions" |
| **0.60 – 0.80** | Related topic, same policy area | "Clean energy" ↔ "Energy Efficiency and Renewable Energy" |
| **0.45 – 0.60** | Conceptually connected but not a direct match | "School lunch" ↔ "Child Nutrition Programs" (0.51) |
| **0.30 – 0.45** | Weak connection; may be coincidental | "Cryptocurrency regulation" ↔ "Regulation and Technology" |
| **< 0.30** | No meaningful relationship | Random topic ↔ unrelated provision |

**Key insight:** A score of 0.51 for "school lunch" → "Child Nutrition Programs" is strong for a *conceptual translation* query. Scores above 0.80 typically occur only when comparing the same program in different bills.

## More Queries to Try

These examples demonstrate different types of semantic matching. Try each one against the example data:

### Layperson → Bureaucratic Translation

The most common use case — you know what you want in plain English, but the bill uses formal government terminology:

```bash
# Plain language → official program names
congress-approp search --dir examples --semantic "money for fixing roads and bridges" --top 5
# → Highway Infrastructure Programs, Federal-Aid Highways, National Infrastructure Investments

congress-approp search --dir examples --semantic "space exploration and rockets" --top 5
# → Exploration (NASA), Space Operations, Space Technology

congress-approp search --dir examples --semantic "fighting wildfires" --top 5
# → Wildland Fire Management, Wildfire Suppression Operations Reserve Fund

congress-approp search --dir examples --semantic "help for homeless veterans" --top 5
# → Homeless Assistance Grants, various VA provisions
```

### Topic Discovery

When you're exploring a policy area without knowing specific program names:

```bash
# What's in the bill about clean energy?
congress-approp search --dir examples --semantic "clean energy research" --top 10

# What about drug enforcement?
congress-approp search --dir examples --semantic "drug enforcement and narcotics control" --top 10

# Nuclear weapons and defense?
congress-approp search --dir examples --semantic "nuclear weapons maintenance and modernization" --top 10
```

### News Story → Provisions

Paste a phrase from a news article to find the relevant provisions:

```bash
# From a headline about the opioid crisis
congress-approp search --dir examples --semantic "opioid crisis drug treatment" --top 5

# From a story about border security
congress-approp search --dir examples --semantic "border wall construction and immigration enforcement" --top 5

# From a story about scientific research funding
congress-approp search --dir examples --semantic "federal funding for scientific research grants" --top 10
```

## Combining Semantic Search with Filters

Semantic search provides the *ranking* (which provisions are most relevant to your query). Hard filters provide *constraints* (which provisions are even eligible to appear). When combined, the filters apply first, then semantic ranking orders the remaining results.

### Filter by provision type

If you only want appropriation-type provisions (not riders, directives, or limitations):

```bash
congress-approp search --dir examples --semantic "clean energy" --type appropriation --top 5
```

This is useful because semantic search doesn't distinguish provision types — a rider about clean energy policy scores as high as an appropriation for clean energy funding. Adding `--type appropriation` ensures you only see provisions with dollar amounts.

### Filter by dollar range

Find large provisions about a topic:

```bash
congress-approp search --dir examples --semantic "scientific research" --type appropriation --min-dollars 1000000000 --top 5
```

This returns only appropriations of $1 billion or more that are semantically related to scientific research.

### Filter by division

Focus on a specific part of the omnibus:

```bash
# Only Division A (MilCon-VA)
congress-approp search --dir examples --semantic "veterans health care" --division A --top 5

# Only Division B (Agriculture)
congress-approp search --dir examples --semantic "farm subsidies" --division B --top 5
```

### Combine multiple filters

```bash
congress-approp search --dir examples \
  --semantic "renewable energy and climate" \
  --type appropriation \
  --min-dollars 100000000 \
  --division D \
  --top 10
```

This finds the top 10 appropriations of $100M+ in Division D (Energy and Water) related to renewable energy and climate.

## Finding Similar Provisions with --similar

While `--semantic` embeds a *text query* and searches for matching provisions, `--similar` takes an *existing provision* and finds the most similar provisions across all loaded bills. This is the cross-bill matching tool.

### Basic usage

The syntax is `--similar <bill_directory>:<provision_index>`:

```bash
congress-approp search --dir examples --similar hr9468:0 --top 5
```

```text
┌──────┬───────────┬───────────────┬────────────────────────────────┬─────────────────┬─────┐
│ Sim  ┆ Bill      ┆ Type          ┆ Description / Account          ┆      Amount ($) ┆ Div │
╞══════╪═══════════╪═══════════════╪════════════════════════════════╪═════════════════╪═════╡
│ 0.86 ┆ H.R. 4366 ┆ appropriation ┆ Compensation and Pensions      ┆ 182,310,515,000 ┆ A   │
│ 0.78 ┆ H.R. 4366 ┆ appropriation ┆ Compensation and Pensions      ┆  15,072,388,000 ┆ A   │
│ 0.73 ┆ H.R. 4366 ┆ limitation    ┆ Compensation and Pensions      ┆      22,109,000 ┆ A   │
│ 0.70 ┆ H.R. 9468 ┆ appropriation ┆ Readjustment Benefits          ┆     596,969,000 ┆     │
│ 0.68 ┆ H.R. 4366 ┆ rescission    ┆ Medical Support and Compliance ┆   1,550,000,000 ┆ A   │
└──────┴───────────┴───────────────┴────────────────────────────────┴─────────────────┴─────┘
5 provisions found
```

Here `hr9468:0` means "provision index 0 in the `hr9468` directory" — that's the VA Supplemental's Compensation and Pensions appropriation. The top match in the omnibus is the same account at 0.86 similarity.

### Key difference from --semantic

| Feature | `--semantic` | `--similar` |
|---------|-------------|-------------|
| Input | A text query you type | An existing provision by directory:index |
| API call? | Yes — embeds your query text via OpenAI (~100ms) | **No** — uses pre-computed vectors from `vectors.bin` |
| Use case | Find provisions matching a concept | Match the same program across bills |
| Requires OPENAI_API_KEY? | Yes | No |

Because `--similar` doesn't make any API calls, it's instant and free. It looks up the source provision's pre-computed vector and computes cosine similarity against every other provision's vector locally.

### Finding the provision index

To use `--similar`, you need the provision index. There are several ways to find it:

**Method 1:** Use `--format json` and look for the `provision_index` field:

```bash
congress-approp search --dir examples/hr9468 --type appropriation --format json | \
  jq '.[] | "\(.provision_index): \(.account_name) $\(.dollars)"'
```

```text
"0: Compensation and Pensions $2285513000"
"1: Readjustment Benefits $596969000"
```

**Method 2:** In the table output, count rows from the top (zero-indexed). The first row is index 0, the second is index 1, and so on within each bill.

**Method 3:** For a specific account, search for it and note the `provision_index` in the JSON output.

### Cross-bill matching with different naming conventions

CRs and omnibus bills often use different naming conventions for the same account. Embeddings handle this because they capture meaning, not just words:

- CR: `"Rural Housing Service—Rural Community Facilities Program Account"` 
- Omnibus: `"Rural Community Facilities Program Account"`

Despite the different names, `--similar` will match these at approximately 0.78 similarity — well above the threshold for confident matching.

## When Semantic Search Doesn't Work

Semantic search is powerful but not universal. Here are situations where other approaches work better:

### Exact account name lookups

If you know the precise account name, `--account` is faster, deterministic, and doesn't require an API key:

```bash
# Better than semantic search for exact lookups
congress-approp search --dir examples --account "Child Nutrition Programs"
```

### No conceptual match in the dataset

If you search for a topic that genuinely isn't in the bills, similarity scores will be low — and that's the correct answer:

```bash
congress-approp search --dir examples --semantic "cryptocurrency regulation bitcoin blockchain" --top 3
```

```text
┌──────┬───────────┬───────────────┬───────────────────────────────┬─────────────┬─────┐
│ Sim  ┆ Bill      ┆ Type          ┆ Description / Account         ┆  Amount ($) ┆ Div │
╞══════╪═══════════╪═══════════════╪═══════════════════════════════╪═════════════╪═════╡
│ 0.30 ┆ H.R. 4366 ┆ appropriation ┆ Regulation and Technology     ┆  62,400,000 ┆ E   │
│ 0.29 ┆ H.R. 4366 ┆ appropriation ┆ Regulation and Technology     ┆      40,000 ┆ E   │
│ 0.29 ┆ H.R. 4366 ┆ appropriation ┆ Regulation and Technology     ┆ 116,186,000 ┆ E   │
└──────┴───────────┴───────────────┴───────────────────────────────┴─────────────┴─────┘
3 provisions found
```

Scores of 0.29–0.30 are well below any meaningful threshold. The tool correctly surfaces the *closest* things it has (NRC "Regulation and Technology" — the word "regulation" provides a weak signal) but the low scores tell you: nothing in this dataset is actually about cryptocurrency.

**Treat scores below 0.40 as "no meaningful match."**

### Distinguishing provision types by embedding

Embeddings capture *what the provision is about*, not *what type of action it is*. A rider that prohibits funding for abortions and an appropriation for reproductive health services may score highly similar because they're about the same *topic* — even though they represent opposite policy actions.

If provision type matters, always combine semantic search with `--type`:

```bash
# Find appropriations about reproductive health, not policy riders
congress-approp search --dir examples --semantic "reproductive health" --type appropriation --top 5
```

### Query instability

Different phrasings of the same question can produce somewhat different results. In experiments, five different phrasings of a FEMA-related query shared only one common provision in their top-5 results. This is a known property of embedding models.

**Mitigation:** If the topic matters, try 2–3 different phrasings and take the union of results. A future `--multi-query` feature will automate this.

## Cost and Performance

Semantic search is fast and inexpensive:

| Operation | Time | Cost |
|-----------|------|------|
| Embed your query text (one API call) | ~100ms | ~$0.0001 |
| Cosine similarity over 2,500 provisions | <0.1ms | Free (local) |
| Load embedding vectors from disk | ~2ms | Free (local) |
| **Total per search** | **~100ms** | **~$0.0001** |

Embedding generation (one-time per bill):

| Bill | Provisions | Time | Approximate Cost |
|------|-----------|------|-----------------|
| H.R. 9468 (supplemental) | 7 | ~2 seconds | < $0.01 |
| H.R. 5860 (CR) | 130 | ~5 seconds | < $0.01 |
| H.R. 4366 (omnibus) | 2,364 | ~30 seconds | < $0.01 |

The embedding model is `text-embedding-3-large` with 3,072 dimensions. Vectors are stored as binary float32 files that load in milliseconds.

## How It Works Under the Hood

For a detailed technical explanation, see [How Semantic Search Works](../explanation/semantic-search.md). In brief:

1. **At embed time:** Each provision's meaningful text (account name + agency + bill text) is sent to OpenAI's embedding model, which returns a 3,072-dimensional vector. These vectors are stored in `vectors.bin`.

2. **At query time (--semantic):** Your search text is sent to the same model (one API call). The returned vector is compared to every stored provision vector using cosine similarity (the dot product of normalized vectors). Results are ranked by similarity.

3. **At query time (--similar):** The source provision's vector is looked up from the stored `vectors.bin`. No API call needed — everything is local.

4. **The math:** Cosine similarity measures the angle between two vectors in 3,072-dimensional space. Vectors pointing in the same direction (similar meaning) have high cosine similarity; vectors pointing in different directions (different meanings) have low similarity.

## Tips for Effective Semantic Search

1. **Be descriptive, not terse.** "Federal funding for scientific research at universities" works better than just "science." Longer queries give the embedding model more context.

2. **Use domain language when you know it.** "SNAP benefits supplemental nutrition" will rank higher than "food stamps for poor people" because the embedding model has seen more formal language in its training data.

3. **Combine with hard filters.** Semantic search ranks; filters constrain. Use them together:
   ```bash
   congress-approp search --dir examples --semantic "your query" --type appropriation --min-dollars 1000000 --top 10
   ```

4. **Try both `--semantic` and `--similar`.** If you find one good provision via semantic search, switch to `--similar` with that provision's index to find related provisions across other bills without additional API calls.

5. **Trust low scores.** If the best match is below 0.40, the topic likely isn't in the dataset. Don't force an interpretation.

6. **Check results with keyword search.** After semantic search finds a promising account, verify with `--account` or `--keyword` to make sure you're seeing the complete picture:
   ```bash
   # Semantic search found "Child Nutrition Programs" — now get everything for that account
   congress-approp search --dir examples --account "Child Nutrition"
   ```

## Quick Reference

| Task | Command |
|------|---------|
| Search by meaning | `search --semantic "your query" --top 10` |
| Search by meaning, only appropriations | `search --semantic "your query" --type appropriation --top 10` |
| Search by meaning, large provisions only | `search --semantic "your query" --min-dollars 1000000000 --top 10` |
| Find similar provisions across bills | `search --similar hr9468:0 --top 5` |
| Find similar appropriations only | `search --similar hr9468:0 --type appropriation --top 5` |

## Next Steps

- **[How Semantic Search Works](../explanation/semantic-search.md)** — the full technical explanation of embeddings, cosine similarity, and vector storage
- **[Track a Program Across Bills](./track-program-across-bills.md)** — using `--similar` for cross-bill matching
- **[Generate Embeddings](../how-to/generate-embeddings.md)** — creating embeddings for your own extracted bills