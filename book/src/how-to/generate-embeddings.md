# Generate Embeddings

> **You will need:** `congress-approp` installed, extracted bill data (with `extraction.json`), `OPENAI_API_KEY` environment variable set.
>
> **You will learn:** How to generate embedding vectors for semantic search and `--similar` matching, configure embedding options, detect and handle staleness, and manage embedding storage.

Embeddings are what power semantic search (`--semantic`) and cross-bill matching (`--similar`). Each provision's text is converted into a 3,072-dimensional vector that captures its meaning. Provisions about similar topics — even with completely different wording — will have vectors pointing in similar directions.

You only need to generate embeddings once per bill. After that, all semantic operations use the stored vectors locally, with the single exception of `--semantic` queries which make one small API call to embed your query text.

## Prerequisites

1. **Extracted bill data.** You need `extraction.json` in each bill directory. See [Extract Provisions from a Bill](./extract-provisions.md).
2. **OpenAI API key.** Embeddings use OpenAI's `text-embedding-3-large` model.

```bash
export OPENAI_API_KEY="your-key-here"
```

> **Note:** The included example data (`data/118-hr4366`, `data/118-hr5860`, `data/118-hr9468`) ships with pre-generated embeddings. You don't need to run `embed` for the examples unless you want to regenerate them.

## Generate Embeddings

### Single bill directory

```bash
congress-approp embed --dir data/118/hr/9468
```

For a small bill (7 provisions), this takes a few seconds. For the FY2024 omnibus (2,364 provisions), about 30 seconds.

### All bills under a directory

```bash
congress-approp embed --dir data
```

The tool walks recursively, finds every directory with an `extraction.json`, and generates embeddings for each one. Bills that already have up-to-date embeddings are skipped automatically.

### Preview without calling the API

```bash
congress-approp embed --dir data --dry-run
```

Shows how many provisions would be embedded and the estimated token count for each bill, without making any API calls.

## What Gets Created

The `embed` command writes two files to each bill directory:

### embeddings.json

A small JSON metadata file (~200 bytes, human-readable):

```json
{
  "schema_version": "1.0",
  "model": "text-embedding-3-large",
  "dimensions": 3072,
  "count": 7,
  "extraction_sha256": "a1b2c3d4e5f6...",
  "vectors_file": "vectors.bin",
  "vectors_sha256": "f6e5d4c3b2a1..."
}
```

| Field | Description |
|-------|-------------|
| `schema_version` | Embedding schema version |
| `model` | The OpenAI model used to generate embeddings |
| `dimensions` | Number of dimensions per vector |
| `count` | Number of provisions embedded (should match the provisions array length in `extraction.json`) |
| `extraction_sha256` | SHA-256 hash of the `extraction.json` this was built from — enables staleness detection |
| `vectors_file` | Filename of the binary vectors file |
| `vectors_sha256` | SHA-256 hash of the vectors file — integrity check |

### vectors.bin

A binary file containing raw little-endian float32 vectors. The file size is exactly `count × dimensions × 4` bytes:

| Bill | Provisions | Dimensions | File Size |
|------|-----------|------------|-----------|
| H.R. 9468 (supplemental) | 7 | 3,072 | 86 KB |
| H.R. 5860 (CR) | 130 | 3,072 | 1.6 MB |
| H.R. 4366 (omnibus) | 2,364 | 3,072 | 29 MB |

There is no header in the file — the count and dimensions come from `embeddings.json`. Vectors are stored in provision order (provision 0 first, then provision 1, etc.).

## Embedding Options

### Model

The default model is `text-embedding-3-large`, which provides the best quality embeddings available from OpenAI. You can override this:

```bash
congress-approp embed --dir data --model text-embedding-3-small
```

> **Warning:** All embeddings in a dataset must use the same model. You cannot compare vectors from different models. If you change models, regenerate embeddings for all bills.

### Dimensions

By default, the tool requests the full 3,072 dimensions from `text-embedding-3-large`. You can request fewer dimensions for smaller storage at the cost of some quality:

```bash
congress-approp embed --dir data --dimensions 1024
```

Experimental results from this project's testing:

| Dimensions | Storage (omnibus) | Top-20 Overlap vs. 3072 |
|------------|-------------------|------------------------|
| 256 | ~2.4 MB | 16/20 (lossy) |
| 512 | ~4.8 MB | 18/20 (near-lossless) |
| 1024 | ~9.7 MB | 19/20 |
| 3072 (default) | ~29 MB | 20/20 (ground truth) |

Since binary vector files load in under 2ms regardless of size, there is little practical reason to truncate dimensions.

> **Warning:** Like models, all embeddings in a dataset must use the same dimension count. Cosine similarity between vectors of different dimensions is undefined.

### Batch size

Provisions are sent to the API in batches. The default batch size is 100 provisions per API call:

```bash
congress-approp embed --dir data --batch-size 50
```

Smaller batch sizes make more API calls but reduce the impact of a single failed call. The default of 100 is efficient for most use cases.

## How Provision Text Is Built

Each provision is embedded using a deterministic text representation built by `build_embedding_text()`. The text concatenates the provision's meaningful fields:

```text
Account: Child Nutrition Programs | Agency: Department of Agriculture | Text: For necessary expenses of the Food and Nutrition Service...
```

The exact fields included depend on the provision type:

- **Appropriations/Rescissions:** Account name, agency, program, raw text
- **CR Substitutions:** Account name, reference act, reference section, raw text
- **Directives/Riders:** Description, raw text
- **Other types:** Description or LLM classification, raw text

This deterministic construction means the same provision always produces the same embedding text, regardless of when or where you run the command.

## Staleness Detection

The hash chain connects embeddings to their source extraction:

```text
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

If you re-extract a bill (producing a new `extraction.json`), the embeddings become stale. Commands that use embeddings will warn you:

```text
⚠ H.R. 4366: embeddings are stale (extraction.json has changed)
```

This warning is advisory — the tool still works, but similarity results may not match the current provisions. To fix it, regenerate embeddings:

```bash
congress-approp embed --dir data/118/hr/4366
```

The `embed` command automatically detects stale embeddings and regenerates them. Up-to-date embeddings are skipped.

## Skipping Up-to-Date Bills

When you run `embed` on a directory with multiple bills, the tool checks each one:

1. Does `embeddings.json` exist?
2. Does `extraction_sha256` in `embeddings.json` match the current SHA-256 of `extraction.json`?
3. Does `vectors_sha256` in `embeddings.json` match the current SHA-256 of `vectors.bin`?

If all three checks pass, the bill is skipped with a message like:

```text
Skipping H.R. 9468: embeddings up to date
```

This makes it safe to run `embed --dir data` repeatedly — it only does work where needed.

## Cost Estimates

Embedding generation is inexpensive compared to extraction:

| Bill | Provisions | Estimated Cost |
|------|-----------|---------------|
| H.R. 9468 (7 provisions) | 7 | < $0.001 |
| H.R. 5860 (130 provisions) | 130 | < $0.01 |
| H.R. 4366 (2,364 provisions) | 2,364 | < $0.01 |

The `text-embedding-3-large` model charges per token. Even the largest omnibus bill with 2,364 provisions uses only a few tens of thousands of tokens total, which costs pennies.

Use `--dry-run` to preview the exact token count before committing.

## Reading Vectors in Python

If you want to work with the embeddings outside of `congress-approp`:

```python
import json
import struct

# Load metadata
with open("data/118-hr9468/embeddings.json") as f:
    meta = json.load(f)

dims = meta["dimensions"]  # 3072
count = meta["count"]       # 7

# Load vectors
with open("data/118-hr9468/vectors.bin", "rb") as f:
    raw = f.read()

# Parse into list of vectors
vectors = []
for i in range(count):
    start = i * dims * 4
    end = start + dims * 4
    vec = struct.unpack(f"<{dims}f", raw[start:end])
    vectors.append(vec)

# Vectors are L2-normalized (norm ≈ 1.0), so cosine similarity = dot product
def cosine(a, b):
    return sum(x * y for x, y in zip(a, b))

# Compare provision 0 to provision 1
print(f"Similarity: {cosine(vectors[0], vectors[1]):.4f}")
```

You can also load the vectors into numpy for faster computation:

```python
import numpy as np

vectors = np.frombuffer(raw, dtype=np.float32).reshape(count, dims)

# Cosine similarity matrix
similarity_matrix = vectors @ vectors.T
```

## After Generating Embeddings

Once embeddings are generated, you can use:

- **Semantic search:** `congress-approp search --dir data --semantic "your query" --top 10`
- **Similar provisions:** `congress-approp search --dir data --similar 118-hr9468:0 --top 5`

The `--similar` flag does not make any API calls — it uses the stored vectors directly. The `--semantic` flag makes one API call to embed your query text (~100ms).

## Troubleshooting

### "OPENAI_API_KEY environment variable not set"

Set your API key:

```bash
export OPENAI_API_KEY="your-key-here"
```

### "No extraction.json found"

You need to extract the bill before generating embeddings. Run `congress-approp extract` first.

### Embeddings stale warning after re-extraction

This is expected. Run `congress-approp embed --dir <path>` to regenerate.

### Very large vectors.bin file

The omnibus bill produces a ~29 MB vectors.bin file. This is expected for 2,364 provisions × 3,072 dimensions × 4 bytes per float. The file loads in under 2ms despite its size.

These files are excluded from the crates.io package (via `Cargo.toml` `exclude` field) because they exceed the 10 MB upload limit. They are included in the git repository for users who clone.

## Quick Reference

```bash
# Set API key
export OPENAI_API_KEY="your-key"

# Generate embeddings for one bill
congress-approp embed --dir data/118/hr/9468

# Generate embeddings for all bills
congress-approp embed --dir data

# Preview without API calls
congress-approp embed --dir data --dry-run

# Use a different model
congress-approp embed --dir data --model text-embedding-3-small

# Use fewer dimensions
congress-approp embed --dir data --dimensions 1024

# Smaller batch size
congress-approp embed --dir data --batch-size 50
```

## Full Command Reference

```text
congress-approp embed [OPTIONS]

Options:
    --dir <DIR>                Data directory [default: ./data]
    --model <MODEL>            Embedding model [default: text-embedding-3-large]
    --dimensions <DIMENSIONS>  Request this many dimensions from the API [default: 3072]
    --batch-size <BATCH_SIZE>  Provisions per API batch [default: 100]
    --dry-run                  Preview without calling API
```

## Next Steps

- **[Use Semantic Search](../tutorials/semantic-search.md)** — put your new embeddings to work
- **[Track a Program Across Bills](../tutorials/track-program-across-bills.md)** — cross-bill matching with `--similar`
- **[Data Integrity and the Hash Chain](../explanation/hash-chain.md)** — how staleness detection works