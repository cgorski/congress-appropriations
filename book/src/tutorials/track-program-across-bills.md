# Track a Program Across Bills

> **You will need:** `congress-approp` installed, access to the `examples/` directory. Optionally: `OPENAI_API_KEY` for semantic search.
>
> **You will learn:** How to follow a specific program's funding across multiple bills using `--similar`, and how to interpret cross-bill matching results.

A single program — say, VA Compensation and Pensions — can appear in multiple bills within the same fiscal year: the full-year omnibus, a continuing resolution, and an emergency supplemental. Tracking it across all three tells you the complete funding story. But account names aren't always consistent between bills, and keyword search only works when you know the exact terminology each bill uses.

The `--similar` flag solves this by using pre-computed embedding vectors to find provisions that *mean* the same thing, even when the words differ.

## The Scenario

H.R. 9468 (the VA Supplemental) appropriated $2,285,513,000 for "Compensation and Pensions." You want to find every related provision in the omnibus (H.R. 4366) and the continuing resolution (H.R. 5860).

## Step 1: Identify the Source Provision

First, find the provision you want to track. You can use any search command to locate it:

```bash
congress-approp search --dir examples/hr9468 --type appropriation
```

```text
┌───┬───────────┬───────────────┬─────────────────────────────┬───────────────┬─────────┬─────┐
│ $ ┆ Bill      ┆ Type          ┆ Description / Account       ┆    Amount ($) ┆ Section ┆ Div │
╞═══╪═══════════╪═══════════════╪═════════════════════════════╪═══════════════╪═════════╪═════╡
│ ✓ ┆ H.R. 9468 ┆ appropriation ┆ Compensation and Pensions   ┆ 2,285,513,000 ┆         ┆     │
│ ✓ ┆ H.R. 9468 ┆ appropriation ┆ Readjustment Benefits       ┆   596,969,000 ┆         ┆     │
└───┴───────────┴───────────────┴─────────────────────────────┴───────────────┴─────────┴─────┘
2 provisions found
```

Compensation and Pensions is the first provision listed. To use `--similar`, you need the **bill directory name** and **provision index**. The directory is `hr9468` (the directory name inside `examples/`), and the index is `0` (first provision, zero-indexed).

You can also see the index in JSON output:

```bash
congress-approp search --dir examples/hr9468 --type appropriation --format json
```

Look for the `"provision_index": 0` field in the first result.

## Step 2: Find Similar Provisions Across All Bills

Now use `--similar` to find the closest matches across every loaded bill:

```bash
congress-approp search --dir examples --similar hr9468:0 --top 10
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
│ ...                                                                                         │
└──────┴───────────┴───────────────┴────────────────────────────────┴─────────────────┴─────┘
```

This is the complete picture of Comp & Pensions across the dataset:

1. **0.86 similarity** — The omnibus's main Comp & Pensions appropriation: $182.3 billion. This is the regular-year funding for the same account that the supplemental topped up by $2.3 billion.
2. **0.78 similarity** — The omnibus's *advance appropriation* for Comp & Pensions: $15.1 billion. This is money enacted in FY2024 but available for FY2025.
3. **0.73 similarity** — A $22 million limitation on the Comp & Pensions account.
4. **0.70 similarity** — Readjustment Benefits from the *same supplemental*. This is a different VA account, but conceptually close because it's also VA mandatory benefits.
5. **0.68 similarity** — A rescission of Medical Support and Compliance funds. Related VA account, lower similarity because it's a different type of action (rescission vs. appropriation).

### Why no CR matches?

The continuing resolution (H.R. 5860) doesn't have a specific Comp & Pensions provision because CRs fund at the prior-year rate by default. Only the 13 programs with anomalies (CR substitutions) appear as explicit provisions. VA Comp & Pensions wasn't one of them — it was simply continued at its prior-year level.

## Step 3: How --similar Works Under the Hood

The `--similar` flag does **not** make any API calls. Here's what happens:

1. It looks up the embedding vector for `hr9468:0` from the pre-computed `vectors.bin` file
2. It loads the embedding vectors for every provision in every bill under `--dir`
3. It computes the cosine similarity between the source vector and every other vector
4. It ranks by similarity descending and returns the top N results

Because everything is pre-computed and stored locally, this operation takes less than a millisecond for 2,500 provisions. The only prerequisite is that embeddings have been generated (via `congress-approp embed`) for all the bills you want to search.

## Step 4: Interpret Similarity Scores

The similarity score tells you how closely related two provisions are in "meaning space":

| Score | Interpretation | Example |
|-------|---------------|---------|
| **> 0.80** | Almost certainly the same program | VA Supp "Comp & Pensions" ↔ Omnibus "Comp & Pensions" (0.86) |
| **0.60 – 0.80** | Related topic, same policy area | "Comp & Pensions" ↔ "Medical Support and Compliance" (0.68) |
| **0.45 – 0.60** | Loosely related | VA provisions ↔ non-VA provisions with similar structure |
| **< 0.45** | Probably not meaningfully related | VA provisions ↔ transportation or energy provisions |

For cross-bill tracking, focus on matches **above 0.75** — these are very likely the same account in a different bill.

## Step 5: Track the Second Account

Repeat for Readjustment Benefits (provision index 1 in the supplemental):

```bash
congress-approp search --dir examples --similar hr9468:1 --top 5
```

```text
┌──────┬───────────┬───────────────┬────────────────────────────────┬─────────────────┬─────┐
│ Sim  ┆ Bill      ┆ Type          ┆ Description / Account          ┆      Amount ($) ┆ Div │
╞══════╪═══════════╪═══════════════╪════════════════════════════════╪═════════════════╪═════╡
│ 0.88 ┆ H.R. 4366 ┆ appropriation ┆ Readjustment Benefits          ┆  13,399,805,000 ┆ A   │
│ 0.76 ┆ H.R. 9468 ┆ appropriation ┆ Compensation and Pensions      ┆   2,285,513,000 ┆     │
│ ...                                                                                         │
└──────┴───────────┴───────────────┴────────────────────────────────┴─────────────────┴─────┘
```

Top match at 0.88: the omnibus Readjustment Benefits account at $13.4 billion. The supplemental added $597 million on top of that.

## When Account Names Differ Between Bills

The example data happens to use the same account names across bills, but this isn't always the case. Continuing resolutions often use hierarchical names like:

- CR: `"Rural Housing Service—Rural Community Facilities Program Account"`
- Omnibus: `"Rural Community Facilities Program Account"`

Keyword matching would miss this, but `--similar` handles it because the embeddings capture the *meaning* of the provision, not just the words.

To demonstrate, let's find the omnibus counterparts of the CR substitutions that have different naming conventions:

```bash
# First, find a CR substitution provision index
congress-approp search --dir examples/hr5860 --type cr_substitution --format json
# Note: the first CR substitution (Rural Housing) is at some index — check provision_index

# Then find similar provisions in the omnibus
congress-approp search --dir examples --similar hr5860:<INDEX> --top 3
```

Even though "Rural Housing Service—Rural Community Facilities Program Account" and "Rural Community Facilities Program Account" are different strings, the embedding similarity will be in the 0.75–0.80 range — high enough to confidently identify them as the same program.

## Building a Funding Timeline

Once you can match accounts across bills, you can assemble a complete funding picture. For VA Comp & Pensions in FY2024:

| Source | Amount | Type |
|--------|--------|------|
| H.R. 4366 (Omnibus) | $182,310,515,000 | Regular appropriation |
| H.R. 4366 (Omnibus) | $15,072,388,000 | Advance appropriation (FY2025) |
| H.R. 9468 (Supplemental) | $2,285,513,000 | Emergency supplemental |
| H.R. 5860 (CR) | *(prior-year rate)* | No explicit provision — funded by CR baseline |

With multiple fiscal years extracted, you could extend this to a multi-year timeline. The `--similar` command makes cross-year matching possible even when account names evolve.

## What's Coming: Persistent Links

Currently, `--similar` results are ephemeral — you see them, but they aren't saved. A future `link suggest` / `link accept` workflow will let you persist these relationships:

```bash
# Future workflow (not yet implemented):
congress-approp link suggest --dir data --threshold 0.80
congress-approp link accept --dir data a1b2c3 d4e5f6
congress-approp compare --base data/fy2023 --current data/fy2024 --use-links
```

This will enable automatic cross-year matching even when account names change, with human review for ambiguous cases.

## Tips for Cross-Bill Tracking

1. **Start from the smaller bill.** If you're tracking between a supplemental (7 provisions) and an omnibus (2,364 provisions), start from the supplemental and search into the omnibus. It's easier to review 5–10 matches than 2,364.

2. **Use `--top 3` to reduce noise.** You rarely need more than the top 3 matches. The best match is almost always the right one.

3. **Combine with `--type` for precision.** If you're matching appropriations, add `--type appropriation` to exclude riders, directives, and other provision types from the results:

   ```bash
   congress-approp search --dir examples --similar hr9468:0 --type appropriation --top 5
   ```

4. **Check both directions.** If provision A in bill X matches provision B in bill Y at 0.85, provision B in bill Y should also match provision A in bill X at a similar score. If it doesn't, something is off.

5. **Low max similarity means the program is unique.** If your source provision's best match in another bill is below 0.55, the program may genuinely not exist in that bill. This is useful for identifying new programs or eliminated ones.

## Summary

| Task | Command |
|------|---------|
| Find the omnibus version of a supplemental provision | `search --dir examples --similar hr9468:0 --top 3` |
| Find related provisions across all bills | `search --dir examples --similar hr4366:42 --top 10` |
| Restrict matches to appropriations only | `search --dir examples --similar hr9468:0 --type appropriation --top 5` |
| Find provisions in a specific bill | `search --dir examples/hr4366 --similar hr9468:0 --top 5` |

## Next Steps

- **[Use Semantic Search](./semantic-search.md)** — search by meaning using text queries instead of provision references
- **[Compare Two Bills](./compare-two-bills.md)** — account-level comparison using name matching
- **[How Semantic Search Works](../explanation/semantic-search.md)** — understand the embedding and cosine similarity mechanics