# How Semantic Search Works

Semantic search lets you find provisions by *meaning* rather than keywords. The query "school lunch programs for kids" finds "Child Nutrition Programs" even though the words don't overlap — because the *meaning* is similar. This chapter explains the technology behind this capability: what embeddings are, how cosine similarity works, how vectors are stored, and why certain queries work better than others.

## The Intuition

Imagine every provision is a point on a map of "meaning." Programs about similar things are close together on this map. "Child Nutrition Programs" and "school lunch programs for kids" are at nearby points even though they share zero words — because they mean similar things.

Your search query is also placed on this map, and the tool finds the nearest points. That's semantic search.

The "map" is actually a 3,072-dimensional vector space (far more dimensions than a physical map's two), and "nearby" is measured by the angle between vectors. But the intuition holds: similar meanings are close together, dissimilar meanings are far apart.

## What Actually Happens

### At Embed Time (One-Time Setup)

When you run `congress-approp embed`, each provision's text is sent to OpenAI's `text-embedding-3-large` model. The model returns a vector — a list of 3,072 floating-point numbers — that represents the provision's meaning in high-dimensional space.

The text sent to the model is built deterministically from the provision's key fields:

```text
Account: Child Nutrition Programs | Agency: Department of Agriculture | Text: For necessary expenses of the Food and Nutrition Service...
```

This combined text gives the embedding model enough context to understand what the provision is about. The exact fields included depend on the provision type:

- **Appropriations/Rescissions:** Account name, agency, program, raw text
- **CR Substitutions:** Account name, reference act, reference section, raw text
- **Directives/Riders:** Description, raw text
- **Other types:** Description or LLM classification, raw text

The resulting vectors are stored locally:

- `embeddings.json` — metadata (model, dimensions, count, hashes)
- `vectors.bin` — raw float32 array, `count × 3072 × 4` bytes

For the FY2024 omnibus with 2,364 provisions, `vectors.bin` is 29 MB and loads in under 2 milliseconds.

### At Query Time (`--semantic`)

When you run `search --semantic "school lunch programs for kids"`:

1. Your query text is sent to the same OpenAI embedding model (single API call, ~100ms, costs fractions of a cent)
2. The model returns a 3,072-dimensional query vector
3. The tool loads the pre-computed provision vectors from `vectors.bin`
4. It computes the cosine similarity between the query vector and every provision vector
5. Results are ranked by similarity descending, filtered by any hard constraints (`--type`, `--division`, `--min-dollars`, etc.), and truncated to `--top N`

### At Query Time (`--similar`)

When you run `search --similar 118-hr9468:0`:

1. The tool looks up provision 0's pre-computed vector from the `hr9468` directory's `vectors.bin`
2. It computes cosine similarity against every other provision's vector across all loaded bills
3. Results are ranked by similarity descending

**No API call is made** — the source provision's vector is already stored locally. This makes `--similar` instant and free.

## Cosine Similarity

Cosine similarity is the mathematical measure of how similar two vectors are. It computes the cosine of the angle between them in high-dimensional space.

### The Formula

For two vectors **a** and **b**:

```text
cosine_similarity(a, b) = (a · b) / (|a| × |b|)
```

Where `a · b` is the dot product (sum of element-wise products) and `|a|` is the L2 norm (square root of sum of squared elements).

Since OpenAI embedding vectors are **L2-normalized** (every vector has norm = 1.0), the formula simplifies to just the dot product:

```text
cosine_similarity(a, b) = a · b = Σ(aᵢ × bᵢ)
```

This is extremely fast to compute — just 3,072 multiplications and additions per pair. Over 2,500 provisions, the entire search takes less than 0.1 milliseconds.

### Score Ranges

Cosine similarity ranges from -1 to 1 in theory, but for text embeddings the practical range is much narrower. Here's what scores mean for appropriations provisions:

| Score Range | Interpretation | Real Example |
|-------------|---------------|--------------|
| **> 0.80** | Almost certainly the same program in a different bill | VA Supplemental "Comp & Pensions" ↔ Omnibus "Comp & Pensions" = **0.86** |
| **0.60 – 0.80** | Related topic, same policy area | "Comp & Pensions" ↔ "Readjustment Benefits" = **0.70** |
| **0.45 – 0.60** | Conceptually connected but not a direct match | "school lunch programs for kids" ↔ "Child Nutrition Programs" = **0.51** |
| **0.30 – 0.45** | Weak connection; may be coincidental | "cryptocurrency regulation" ↔ NRC "Regulation and Technology" = **0.30** |
| **< 0.30** | No meaningful relationship | Random topic ↔ unrelated provision |

These thresholds were calibrated through 30 experiments on the example data. They are specific to appropriations provisions and may not generalize to other domains.

### Why Cosine Instead of Euclidean Distance?

Cosine similarity measures the *direction* vectors point, ignoring their *magnitude*. Since all embedding vectors are normalized to unit length, magnitude is already removed — but the conceptual advantage remains: provisions about the same topic point in the same direction regardless of how long or detailed their text is.

In experiments on this project's data, cosine similarity, Euclidean distance, and dot product all produced identical rankings (Spearman ρ = 1.0). This is mathematically expected for L2-normalized vectors — all three metrics are monotone transformations of each other when norms are constant.

## What Embeddings Capture (and Don't)

### What works well

**Layperson → bureaucratic translation.** The embedding model understands that "school lunch programs for kids" and "Child Nutrition Programs" mean the same thing because it was trained on vast amounts of text that connects these concepts. This is particularly useful when the user does not know the official program name.

**Cross-bill matching.** The same program in different bills — even with different naming conventions — produces similar vectors:

| CR Account Name | Omnibus Account Name | Similarity |
|----------------|---------------------|------------|
| Rural Housing Service—Rural Community Facilities Program Account | Rural Community Facilities Program Account | ~0.78 |
| National Science Foundation—Research and Related Activities | Research and Related Activities | ~0.77 |

The embedding model ignores the hierarchical prefix ("Rural Housing Service—") and focuses on the semantic content.

**Topic discovery.** Searching for "clean energy research" finds Energy Efficiency and Renewable Energy, Nuclear Energy, and related accounts even though the specific program names don't match the query.

**Same-account matching across bills.** VA Supplemental "Compensation and Pensions" matches Omnibus "Compensation and Pensions" at 0.86 — the highest similarities in the dataset come from the same program appearing in different bills.

### What doesn't work well

**Provision type classification.** Embeddings don't strongly encode whether something is a rider vs. an appropriation vs. a limitation. A rider prohibiting funding for X and an appropriation funding X may have similar embeddings because they're *about* the same topic. If type matters, combine semantic search with `--type`.

**Vector arithmetic.** Analogies like "MilCon Army - Army + Navy = MilCon Navy" don't work. The embedding space doesn't support linear arithmetic the way word2vec sometimes does.

**Clustering.** Attempting DBSCAN or k-means clustering on the provision embeddings collapses almost everything into one cluster. Appropriations provisions are too semantically similar to each other (they're all about government spending) for global clustering to produce useful groups.

**Query stability.** Different phrasings of the same question can produce somewhat different top-5 results. In experiments, five different FEMA-related queries shared only 1 of 5 common results in their top-5 lists. This is a known property of embedding models — the ranking is sensitive to exact wording.

## The Embedding Model

The tool uses OpenAI's `text-embedding-3-large` model with the full 3,072 native output dimensions.

### Why this model?

- **Quality:** Best-in-class performance on semantic similarity benchmarks at the time of development
- **Dimensionality:** 3,072 dimensions provide lossless representation — experiments showed that truncating to 1,024 dimensions lost 1 of 20 top results, and truncating to 256 lost 4 of 20
- **Determinism:** Embedding the same text produces nearly identical vectors across calls (max deviation ~1e-6)
- **Normalization:** Outputs are L2-normalized, so cosine similarity reduces to a dot product

### Why full 3,072 dimensions?

Experiments compared truncated dimensions:

| Dimensions | Top-20 Overlap vs. 3072 | Storage (Omnibus) |
|------------|------------------------|-------------------|
| 256 | 16/20 (lossy) | ~2.4 MB |
| 512 | 18/20 (near-lossless) | ~4.8 MB |
| 1024 | 19/20 | ~9.7 MB |
| 3072 | 20/20 (ground truth) | ~29 MB |

Since binary vector files load in under 2ms regardless of size and storage is negligible for this use case, there was no reason to truncate. The full 3,072 dimensions are used.

### Consistency requirement

All embeddings in a dataset **must** use the same model and dimension count. Cosine similarity between vectors from different models or different dimension counts is undefined and will produce garbage results.

If you change models, you must regenerate embeddings for all bills in the dataset. The hash chain in `embeddings.json` helps detect this — the `model` and `dimensions` fields record what was used.

## Binary Vector Storage

Embeddings are stored in a split format optimized for the read-heavy access pattern:

### embeddings.json (metadata)

```json
{
  "schema_version": "1.0",
  "model": "text-embedding-3-large",
  "dimensions": 3072,
  "count": 2364,
  "extraction_sha256": "ae912e3427b8...",
  "vectors_file": "vectors.bin",
  "vectors_sha256": "7bd7821176bc..."
}
```

Human-readable, ~200 bytes. Contains everything you need to interpret the binary file: the model, dimensions, and count. Also contains SHA-256 hashes for the hash chain (linking embeddings to the extraction that produced them).

### vectors.bin (data)

Raw little-endian float32 array. No header, no delimiters, no structure — just `count × dimensions` floating-point numbers in sequence.

```text
[provision_0_dim_0] [provision_0_dim_1] ... [provision_0_dim_3071]
[provision_1_dim_0] [provision_1_dim_1] ... [provision_1_dim_3071]
...
[provision_N_dim_0] [provision_N_dim_1] ... [provision_N_dim_3071]
```

To read provision `i`'s vector, seek to byte offset `i × dimensions × 4` and read `dimensions × 4` bytes.

**Why binary instead of JSON?** Performance. The omnibus bill's vectors as a JSON array of float arrays would be ~57 MB and take ~175ms to parse. As binary, it's 29 MB and loads in <2ms. Since the tool loads vectors once per CLI invocation and queries many times, fast loading matters.

### Reading vectors in Python

```python
import json
import struct
import numpy as np

# Load metadata
with open("data/118-hr4366/embeddings.json") as f:
    meta = json.load(f)

dims = meta["dimensions"]  # 3072
count = meta["count"]       # 2364

# Option 1: Using struct (standard library)
with open("data/118-hr4366/vectors.bin", "rb") as f:
    raw = f.read()
for i in range(count):
    vec = struct.unpack(f"<{dims}f", raw[i*dims*4 : (i+1)*dims*4])

# Option 2: Using numpy (much faster for large files)
vectors = np.fromfile("data/118-hr4366/vectors.bin", dtype=np.float32).reshape(count, dims)

# Compute cosine similarity (vectors are already normalized)
similarity = vectors[0] @ vectors[1]  # dot product = cosine for unit vectors
```

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Load vectors from disk (14 bills) | ~8ms | Binary file I/O |
| Cosine similarity (one query vs. 8,500 provisions) | <0.5ms | 8,500 dot products of 3,072 dimensions |
| Embed query text (OpenAI API) | ~100ms | Network round-trip |
| **Total `--semantic` search** | **~110ms** | Dominated by the API call |
| **Total `--similar` search** | **~8ms** | No API call needed |

At 20 congresses (~60 bills, ~15,000 provisions), cosine computation would still be under 1ms. The bottleneck is always the network call for `--semantic`, which is inherently ~100ms regardless of dataset size.

## Staleness Detection

The hash chain links embeddings to the extraction they were built from:

```text
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

If you re-extract a bill (producing a new `extraction.json` with different provisions), the stored `extraction_sha256` in `embeddings.json` no longer matches. The tool detects this and warns:

```text
⚠ H.R. 4366: embeddings are stale (extraction.json has changed)
```

Stale embeddings still work — cosine similarity is still computed correctly — but the provision indices may have shifted, so the vectors may not correspond to the right provisions. Regenerate with `congress-approp embed` to fix.

## Comparison to Keyword Search

| Feature | Keyword Search (`--keyword`) | Semantic Search (`--semantic`) |
|---------|------------------------------|-------------------------------|
| Finds exact word matches | ✓ Always | Not guaranteed — may rank lower |
| Finds conceptual matches | ✗ Never | ✓ Core strength |
| Requires API key | No | Yes (OPENAI_API_KEY) |
| Requires pre-computed data | No | Yes (embeddings) |
| Deterministic | Yes — same query always returns same results | Nearly — scores vary by ~1e-6 across runs |
| Speed | ~1ms (string matching) | ~100ms (API call) |
| Cost per query | Free | ~$0.0001 |
| Best for | Known terms in bill text | Concepts, topics, layperson language |

**Recommendation:** Use keyword search when you know the exact term. Use semantic search when you don't know the official terminology, when you want to discover related provisions, or when you want to match across bills with different naming conventions. Use both for the most thorough coverage.

## Experimental Results

The embedding approach was validated through 30 experiments on the example data:

### Successful use cases

- **Layperson → bureaucratic:** "school lunch for kids" → "Child Nutrition Programs" (6/7 correct results)
- **Cross-bill matching:** VA Supplemental "Comp & Pensions" → Omnibus "Comp & Pensions" at 0.86
- **News clip → provisions:** Pasted news article excerpts found relevant provisions
- **Topic classification:** 15 policy topics correctly assigned via embedding nearest-neighbor
- **Orphan detection:** Provisions unique to one bill identified by low max-similarity to any other bill

### Failed use cases

- **Vector arithmetic/analogy:** "MilCon Army - Army + Navy" failed
- **Global clustering:** All provisions collapsed to one cluster
- **Provision type classification via embeddings:** Riders classified at 11% accuracy
- **Query stability:** 5 FEMA rephrasings shared only 1/5 common top-5 result

### Key calibration numbers

- **>0.80** = same account across bills (use for confident cross-bill matching)
- **0.60–0.80** = related topic, same policy area (use for discovery)
- **0.45–0.60** = loosely related (use as hints, not answers)
- **<0.45** = unlikely to be meaningfully related (treat as no match)

These thresholds are stable across the dataset but may need recalibration for very different bill types or future congresses.

## Tips for Better Results

1. **Be descriptive.** "Federal funding for scientific research at universities" works better than "science." More context gives the embedding model more signal.

2. **Use domain language when you know it.** "SNAP benefits supplemental nutrition" will outperform "food stamps for poor people."

3. **Combine with hard filters.** Semantic search provides ranking; `--type`, `--division`, `--min-dollars` provide constraints. Use both.

4. **Try multiple phrasings.** Query instability is real. If the topic matters, try 2–3 different phrasings and take the union of results.

5. **Follow up `--semantic` with `--similar`.** If semantic search finds one good provision, use its index with `--similar` to find related provisions across other bills without additional API calls.

6. **Trust low scores.** If the best match is below 0.40, the topic genuinely isn't in the dataset. That's the correct answer, not a failure.

## Next Steps

- **[Use Semantic Search](../tutorials/semantic-search.md)** — practical tutorial with real queries
- **[Track a Program Across Bills](../tutorials/track-program-across-bills.md)** — using `--similar` for cross-bill matching
- **[Generate Embeddings](../how-to/generate-embeddings.md)** — creating embeddings for your own data
- **[Data Integrity and the Hash Chain](./hash-chain.md)** — how staleness detection works