# embeddings.json Fields

Complete reference for the embedding metadata file and its companion binary vector file. These are produced by the `congress-approp embed` command and consumed by `search --semantic` and `search --similar`.

## Overview

Embeddings use a split storage format:

- **`embeddings.json`** — Small JSON metadata file (~200 bytes, human-readable)
- **`vectors.bin`** — Binary float32 array (can be tens of megabytes for large bills)

The metadata file tells you everything you need to interpret the binary file: which model produced the vectors, how many dimensions each vector has, how many provisions are embedded, and SHA-256 hashes for the data integrity chain.

---

## embeddings.json Structure

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

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Embedding schema version. Currently `"1.0"`. |
| `model` | string | The OpenAI embedding model used (e.g., `"text-embedding-3-large"`). All embeddings in a dataset must use the same model — you cannot compare vectors from different models. |
| `dimensions` | integer | Number of dimensions per vector. Default is `3072` for `text-embedding-3-large`. All embeddings in a dataset must use the same dimension count. |
| `count` | integer | Number of provisions embedded. Should equal the length of the `provisions` array in the corresponding `extraction.json`. |
| `extraction_sha256` | string | SHA-256 hash of the `extraction.json` file these embeddings were built from. Used for staleness detection — if the extraction changes, this hash won't match and the tool warns that embeddings are stale. |
| `vectors_file` | string | Filename of the binary vectors file. Always `"vectors.bin"`. |
| `vectors_sha256` | string | SHA-256 hash of the `vectors.bin` file. Integrity check — detects corruption or truncation. |

### Example Files from Included Data

| Bill | Count | Dimensions | embeddings.json Size | vectors.bin Size |
|------|-------|------------|---------------------|-----------------|
| H.R. 4366 (omnibus) | 2,364 | 3,072 | ~230 bytes | 29,048,832 bytes (29 MB) |
| H.R. 5860 (CR) | 130 | 3,072 | ~230 bytes | 1,597,440 bytes (1.6 MB) |
| H.R. 9468 (supplemental) | 7 | 3,072 | ~230 bytes | 86,016 bytes (86 KB) |

---

## vectors.bin Format

A flat binary file containing raw **little-endian float32** values. There is no header, no delimiter, and no structure — just `count × dimensions` floating-point numbers in sequence.

### Layout

```text
[provision_0_dim_0] [provision_0_dim_1] ... [provision_0_dim_3071]
[provision_1_dim_0] [provision_1_dim_1] ... [provision_1_dim_3071]
...
[provision_N_dim_0] [provision_N_dim_1] ... [provision_N_dim_3071]
```

Each float32 is 4 bytes, stored in little-endian byte order. Provisions are stored in the same order as the `provisions` array in `extraction.json` — provision index 0 comes first, then index 1, and so on.

### File Size Formula

```text
file_size = count × dimensions × 4  (bytes)
```

For the omnibus: `2364 × 3072 × 4 = 29,048,832 bytes`

If the actual file size doesn't match this formula, the file is corrupted or truncated. The `vectors_sha256` hash in `embeddings.json` provides an independent integrity check.

### Reading a Specific Provision's Vector

To read the vector for provision at index `i`:

```text
byte_offset = i × dimensions × 4
byte_length = dimensions × 4
```

Seek to `byte_offset` and read `byte_length` bytes, then interpret as `dimensions` little-endian float32 values.

### Vector Properties

All vectors are **L2-normalized** — each vector has a Euclidean norm of approximately 1.0. This means:

- **Cosine similarity equals the dot product:** `cos(a, b) = a · b` (since `|a| = |b| = 1`)
- **Values range from approximately -0.1 to +0.1** per dimension (spread across 3,072 dimensions)
- **Similarity scores range from approximately 0.2 to 0.9** in practice for appropriations data

---

## Reading Vectors in Python

### Using struct (standard library)

```python
import json
import struct

with open("examples/hr9468/embeddings.json") as f:
    meta = json.load(f)

dims = meta["dimensions"]  # 3072
count = meta["count"]       # 7

with open("examples/hr9468/vectors.bin", "rb") as f:
    raw = f.read()

# Verify file size
assert len(raw) == count * dims * 4, "File size mismatch — possible corruption"

# Parse into list of tuples
vectors = []
for i in range(count):
    start = i * dims * 4
    end = start + dims * 4
    vec = struct.unpack(f"<{dims}f", raw[start:end])
    vectors.append(vec)

# Check normalization
norm = sum(x * x for x in vectors[0]) ** 0.5
print(f"Vector 0 L2 norm: {norm:.6f}")  # Should be ~1.000000
```

### Using numpy (faster for large files)

```python
import numpy as np
import json

with open("examples/hr4366/embeddings.json") as f:
    meta = json.load(f)

vectors = np.fromfile(
    "examples/hr4366/vectors.bin",
    dtype=np.float32
).reshape(meta["count"], meta["dimensions"])

print(f"Shape: {vectors.shape}")  # (2364, 3072)
print(f"Vector 0 norm: {np.linalg.norm(vectors[0]):.6f}")  # ~1.000000

# Cosine similarity matrix (fast — vectors are normalized)
similarity = vectors @ vectors.T
print(f"Provision 0 vs 1 similarity: {similarity[0, 1]:.4f}")
```

### Computing Cosine Similarity

Since vectors are L2-normalized, cosine similarity is just the dot product:

```python
def cosine_similarity(a, b):
    return sum(x * y for x, y in zip(a, b))

# Or with numpy:
sim = np.dot(vectors[0], vectors[1])
```

---

## Reading Vectors in Rust

The `congress-approp` library provides the `embeddings` module:

```rust
use congress_appropriations::approp::embeddings;
use std::path::Path;

if let Some(loaded) = embeddings::load(Path::new("examples/hr9468"))? {
    println!("Model: {}", loaded.metadata.model);
    println!("Dimensions: {}", loaded.dimensions());
    println!("Count: {}", loaded.count());

    // Get vector for provision 0
    let vec0: &[f32] = loaded.vector(0);

    // Cosine similarity between provisions 0 and 1
    let sim = embeddings::cosine_similarity(loaded.vector(0), loaded.vector(1));
    println!("Similarity: {:.4}", sim);
}
```

### Key Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `embeddings::load(dir)` | `fn load(dir: &Path) -> Result<Option<LoadedEmbeddings>>` | Load embeddings from a bill directory. Returns `None` if no `embeddings.json` exists. |
| `embeddings::save(dir, meta, vecs)` | `fn save(dir: &Path, metadata: &EmbeddingsMetadata, vectors: &[f32]) -> Result<()>` | Save embeddings to a bill directory. Writes both `embeddings.json` and `vectors.bin`. |
| `embeddings::cosine_similarity(a, b)` | `fn cosine_similarity(a: &[f32], b: &[f32]) -> f32` | Compute cosine similarity (dot product for normalized vectors). |
| `embeddings::normalize(vec)` | `fn normalize(vec: &mut [f32])` | L2-normalize a vector in place. |
| `loaded.vector(i)` | `fn vector(&self, i: usize) -> &[f32]` | Get the embedding vector for provision at index `i`. |
| `loaded.count()` | `fn count(&self) -> usize` | Number of embedded provisions. |
| `loaded.dimensions()` | `fn dimensions(&self) -> usize` | Number of dimensions per vector. |

---

## The Hash Chain

Embeddings participate in the data integrity hash chain:

```text
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

### Staleness Detection

When you run a command that uses embeddings (`search --semantic` or `search --similar`), the tool:

1. Computes the SHA-256 of the current `extraction.json` on disk
2. Compares it to `extraction_sha256` in `embeddings.json`
3. If they differ, prints a warning to stderr:

```text
⚠ H.R. 4366: embeddings are stale (extraction.json has changed)
```

This means the extraction was modified (re-extracted or upgraded) after the embeddings were generated. The provision indices in the vectors may no longer correspond to the current provisions. The warning is advisory — execution continues, but results may be unreliable.

**Fix:** Regenerate embeddings with `congress-approp embed --dir <path>`.

### Integrity Check

The `vectors_sha256` field verifies that `vectors.bin` hasn't been corrupted. If the hash doesn't match, the binary file was modified, truncated, or replaced since embeddings were generated.

### Automatic Skip

The `embed` command checks the hash chain before processing each bill. If `extraction_sha256` matches the current extraction and `vectors_sha256` matches the current vectors file, the bill is skipped:

```text
Skipping H.R. 9468: embeddings up to date
```

This makes it safe to run `embed --dir data` repeatedly — only bills with new or changed extractions are processed.

---

## Consistency Requirements

### Same model across all bills

All embeddings in a dataset must use the same model. Cosine similarity between vectors from different models is undefined. The `model` field in `embeddings.json` records which model was used.

If you change models, regenerate embeddings for **all** bills:

```bash
# Delete existing embeddings (optional — embed will overwrite)
congress-approp embed --dir data --model text-embedding-3-large
```

### Same dimensions across all bills

All embeddings must use the same dimension count. The default is 3,072 (the native output of `text-embedding-3-large`). If you truncate dimensions with `--dimensions 1024`, all bills must use 1,024.

The `dimensions` field in `embeddings.json` records the dimension count. The tool does not currently check for dimension mismatches across bills — comparing vectors of different dimensions will silently produce garbage results.

### Provision count alignment

The `count` field should equal the number of provisions in `extraction.json`. If the extraction is re-run (producing a different number of provisions), the stored vectors no longer align with the provisions — the hash chain detects this as staleness.

---

## Storage on crates.io

The `vectors.bin` files are excluded from the crates.io package via the `exclude` field in `Cargo.toml`:

```toml
exclude = ["examples/*/vectors.bin"]
```

This is because the omnibus bill's `vectors.bin` (29 MB) exceeds crates.io's 10 MB upload limit. Users who install from crates.io can generate embeddings themselves:

```bash
export OPENAI_API_KEY="your-key"
congress-approp embed --dir examples
```

Users who clone the GitHub repository get the pre-generated `vectors.bin` files.

---

## Embedding Model Details

The default model is OpenAI's `text-embedding-3-large`:

| Property | Value |
|----------|-------|
| Model name | `text-embedding-3-large` |
| Native dimensions | 3,072 |
| Normalization | L2-normalized (unit vectors) |
| Determinism | Near-perfect — max deviation ~1e-6 across repeated embeddings of the same text |
| Supported dimension truncation | 256, 512, 1024, 3072 (via `--dimensions` flag) |

### Dimension Truncation Trade-offs

Experimental results from this project:

| Dimensions | Top-20 Overlap vs. 3072 | vectors.bin Size (Omnibus) | Load Time |
|------------|------------------------|---------------------------|-----------|
| 256 | 16/20 (lossy) | ~2.4 MB | <1ms |
| 512 | 18/20 (near-lossless) | ~4.8 MB | <1ms |
| 1024 | 19/20 | ~9.7 MB | ~1ms |
| 3072 (default) | 20/20 (ground truth) | ~29 MB | ~2ms |

Since binary files load in milliseconds regardless of size, the full 3,072 dimensions are recommended. There is no practical performance benefit to truncation.

---

## Related References

- **[How Semantic Search Works](../explanation/semantic-search.md)** — how embeddings enable meaning-based search
- **[Generate Embeddings](../how-to/generate-embeddings.md)** — creating and managing embeddings
- **[Data Integrity and the Hash Chain](../explanation/hash-chain.md)** — staleness detection across the pipeline
- **[Data Directory Layout](./data-directory.md)** — where embedding files fit in the directory structure