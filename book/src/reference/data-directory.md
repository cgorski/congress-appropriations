# Data Directory Layout

Complete reference for the file and directory structure used by `congress-approp`. Every bill lives in its own directory. Files are discovered by recursively walking from whatever `--dir` path you provide, looking for `extraction.json` as the anchor file.

## Directory Structure

```text
data/                              ← any --dir path works
├── hr4366/                        ← bill directory (FY2024 omnibus)
│   ├── BILLS-118hr4366enr.xml     ← source XML from Congress.gov
│   ├── extraction.json            ← structured provisions (REQUIRED — anchor file)
│   ├── verification.json          ← deterministic verification report
│   ├── metadata.json              ← extraction provenance (model, hashes, timestamps)
│   ├── tokens.json                ← LLM token usage from extraction
│   ├── bill_meta.json             ← bill metadata: FY, jurisdictions, advance classification (enrich)
│   ├── embeddings.json            ← embedding metadata (model, dimensions, hashes)
│   ├── vectors.bin                ← raw float32 embedding vectors
│   └── chunks/                    ← per-chunk LLM artifacts (gitignored)
│       ├── 01JRWN9T5RR0JTQ6C9FYYE96A8.json
│       ├── 01JRWNA2B3C4D5E6F7G8H9J0K1.json
│       └── ...
├── hr5860/                        ← bill directory (FY2024 CR)
│   ├── BILLS-118hr5860enr.xml
│   ├── extraction.json
│   ├── verification.json
│   ├── metadata.json
│   ├── tokens.json
│   ├── embeddings.json
│   ├── vectors.bin
│   └── chunks/
└── hr9468/                        ← bill directory (VA supplemental)
    ├── BILLS-118hr9468enr.xml
    ├── extraction.json
    ├── verification.json
    ├── metadata.json
    ├── embeddings.json
    ├── vectors.bin
    └── chunks/
```

## File Reference

| File | Required? | Written By | Read By | Mutable? | Size (Omnibus) |
|------|-----------|-----------|---------|----------|----------------|
| `BILLS-*.xml` | For extraction | `download` | `extract`, `upgrade`, `enrich` | Never | ~1.8 MB |
| `extraction.json` | **Yes** (anchor) | `extract`, `upgrade` | All query commands | Only by re-extract or upgrade |~12 MB |
| `verification.json` | No | `extract`, `upgrade` | `audit`, `search` (for quality fields) | Only by re-extract or upgrade | ~2 MB |
| `metadata.json` | No | `extract` | Staleness detection | Only by re-extract | ~300 bytes |
| `tokens.json` | No | `extract` | Informational only | Never | ~200 bytes |
| `bill_meta.json` | No | `enrich` | `--subcommittee` filtering, staleness detection | Only by re-enrich | ~5 KB |
| `embeddings.json` | No | `embed` | Semantic search, staleness detection | Only by re-embed | ~230 bytes |
| `vectors.bin` | No | `embed` | `search --semantic`, `search --similar` | Only by re-embed | ~29 MB |
| `chunks/*.json` | No | `extract` | Debugging and analysis only | Never | Varies |

### Which files are required?

**Only `extraction.json` is required.** The loader (`loading.rs`) walks recursively from the `--dir` path, finds every file named `extraction.json`, and treats each one as a bill directory. Everything else is optional:

- Without `verification.json`: The `audit` command won't work, and search results won't include `amount_status`, `match_tier`, or `quality` fields.
- Without `metadata.json`: Staleness detection for the source XML link is unavailable.
- Without `BILLS-*.xml`: Extraction, upgrade, and enrich can't run (they need the source XML). Query commands work fine.
- Without `bill_meta.json`: The `--subcommittee` flag is unavailable. The `--fy` flag still works (it uses fiscal year data from `extraction.json`). Run `congress-approp enrich` to generate this file — no API keys required.
- Without `embeddings.json` + `vectors.bin`: `--semantic` and `--similar` searches are unavailable. If you cloned the git repository, these files are included for the example data. If you installed via `cargo install`, run `congress-approp embed --dir data` to generate them (~30 seconds per bill, requires `OPENAI_API_KEY`).
- Without `tokens.json`: No impact on any operation.
- Without `chunks/`: No impact on any operation (these are local provenance records).

---

## File Descriptions

### BILLS-*.xml

The enrolled bill XML downloaded from Congress.gov. The filename follows the GPO convention:

```text
BILLS-{congress}{type}{number}enr.xml
```

Examples:
- `BILLS-118hr4366enr.xml` — H.R. 4366, 118th Congress, enrolled version
- `BILLS-118hr5860enr.xml` — H.R. 5860, 118th Congress, enrolled version
- `BILLS-118hr9468enr.xml` — H.R. 9468, 118th Congress, enrolled version

The XML uses semantic markup from the GPO bill DTD: `<division>`, `<title>`, `<section>`, `<appropriations-small>`, `<quote>`, `<proviso>`, and many more. This semantic structure is what enables reliable parsing and chunk boundary detection.

**Immutable after download.** The source text is never modified by any operation.

### extraction.json

The primary output of the `extract` command. Contains:

- **`bill`** — Bill-level metadata: identifier, classification, short title, fiscal years, divisions
- **`provisions`** — Array of every extracted provision with full structured fields
- **`summary`** — LLM-generated summary statistics (diagnostic only — never used for computation)
- **`chunk_map`** — Links each provision to the extraction chunk that produced it
- **`schema_version`** — Version of the extraction schema

This is the **anchor file** — the loader discovers bill directories by finding this file. All query commands (`search`, `summary`, `compare`, `audit`) read it.

See [extraction.json Fields](./extraction-json.md) for the complete field reference.

### verification.json

Deterministic verification of every provision against the source bill text. No LLM involved — pure string matching.

Contains:
- **`amount_checks`** — Was each dollar string found in the source?
- **`raw_text_checks`** — Is each raw text excerpt a substring of the source?
- **`completeness`** — How many dollar strings in the source were matched to provisions?
- **`summary`** — Roll-up metrics (verified, not_found, ambiguous, match tiers, coverage)

See [verification.json Fields](./verification-json.md) for the complete field reference.

### metadata.json

Extraction provenance — records which model produced the extraction and when:

```json
{
  "model": "claude-opus-4-6",
  "prompt_version": "a1b2c3d4...",
  "extraction_timestamp": "2024-03-17T14:30:00Z",
  "source_xml_sha256": "e5f6a7b8c9d0..."
}
```

The `source_xml_sha256` field is part of the [hash chain](../explanation/hash-chain.md) — it records the SHA-256 of the source XML so the tool can detect if the XML has been re-downloaded.

### bill_meta.json

Bill-level metadata generated by the `enrich` command. Contains fiscal year scoping, subcommittee jurisdiction mappings (division letter → canonical jurisdiction), advance appropriation classification for each budget authority provision, enriched bill nature (omnibus, minibus, full-year CR with appropriations, etc.), and canonical (case-normalized) account names for cross-bill matching.

```json
{
  "schema_version": "1.0",
  "congress": 119,
  "fiscal_years": [2026],
  "bill_nature": "omnibus",
  "subcommittees": [
    { "division": "A", "jurisdiction": "defense", "title": "...", "source": { "type": "pattern_match", "pattern": "department of defense" } }
  ],
  "provision_timing": [
    { "provision_index": 1370, "timing": "advance", "available_fy": 2027, "source": { "type": "fiscal_year_comparison", "availability_fy": 2027, "bill_fy": 2026 } }
  ],
  "canonical_accounts": [
    { "provision_index": 0, "canonical_name": "military personnel, army" }
  ],
  "extraction_sha256": "b461a687..."
}
```

This file is entirely optional. All commands that existed before v4.0 work without it. It is required only for `--subcommittee` filtering. The `--fy` flag works without it (falling back to `extraction.json` fiscal year data). The `extraction_sha256` field is part of the hash chain — it records the SHA-256 of `extraction.json` at enrichment time, enabling staleness detection.

**Requires no API keys to generate.** Run `congress-approp enrich --dir data` to create this file for all bills. See [Enrich Bills with Metadata](../how-to/enrich-data.md) for a detailed guide.

### tokens.json

LLM token usage from extraction:

```json
{
  "total_input": 1200,
  "total_output": 1500,
  "total_cache_read": 800,
  "total_cache_create": 400,
  "calls": 1
}
```

Informational only — not used by any downstream operation. Useful for cost estimation and monitoring.

### embeddings.json

Embedding metadata — a small JSON file (~230 bytes) that describes the companion `vectors.bin` file:

```json
{
  "schema_version": "1.0",
  "model": "text-embedding-3-large",
  "dimensions": 3072,
  "count": 2364,
  "extraction_sha256": "a1b2c3d4...",
  "vectors_file": "vectors.bin",
  "vectors_sha256": "e5f6a7b8..."
}
```

The `extraction_sha256` and `vectors_sha256` fields are part of the hash chain for staleness detection.

See [embeddings.json Fields](./embeddings-json.md) for the complete field reference.

### vectors.bin

Raw little-endian float32 embedding vectors. No header — just `count × dimensions × 4` bytes of floating-point data. The count and dimensions come from `embeddings.json`.

File sizes for the example data:

| Bill | Provisions | Dimensions | File Size |
|------|-----------|------------|-----------|
| H.R. 4366 | 2,364 | 3,072 | 29,048,832 bytes (29 MB) |
| H.R. 5860 | 130 | 3,072 | 1,597,440 bytes (1.6 MB) |
| H.R. 9468 | 7 | 3,072 | 86,016 bytes (86 KB) |

These files are excluded from the crates.io package (`Cargo.toml` `exclude` field) because they exceed the 10 MB upload limit. They are included in the git repository for users who clone.

See [embeddings.json Fields](./embeddings-json.md) for reading instructions.

### chunks/ directory

Per-chunk LLM artifacts stored with ULID filenames (e.g., `01JRWN9T5RR0JTQ6C9FYYE96A8.json`). Each file contains:

- **Thinking content** — The model's internal reasoning for this chunk
- **Raw response** — The raw JSON the LLM produced before parsing
- **Parsed provisions** — The provisions extracted from this chunk after resilient parsing
- **Conversion report** — Type coercions, null-to-default conversions, and warnings

These are permanent provenance records — useful for understanding why the LLM classified a particular provision a certain way, or for debugging extraction issues. They are:

- **Gitignored** by default (`.gitignore` includes `chunks/`)
- **Not part of the hash chain** — no downstream artifact references them
- **Not required** for any query operation
- **Not included** in the crates.io package

Deleting the `chunks/` directory has no effect on any operation.

---

## Nesting Flexibility

The `--dir` flag accepts any directory path. The loader walks recursively from that path, finding every `extraction.json`. This means any nesting structure works:

```bash
# Flat structure (like the examples)
congress-approp summary --dir data
# Finds: data/118-hr4366/extraction.json, data/118-hr5860/extraction.json, data/118-hr9468/extraction.json

# Nested by congress/type/number
congress-approp summary --dir data
# Finds: data/118/hr/4366/extraction.json, data/118/hr/5860/extraction.json, etc.

# Single bill directory
congress-approp summary --dir data/118/hr/9468
# Finds: data/118/hr/9468/extraction.json

# Any arbitrary nesting
congress-approp summary --dir ~/my-appropriations-project/fy2024
# Finds all extraction.json files anywhere under that path
```

The directory name is used as the bill identifier for `--similar` references. For example, if the path is `data/hr9468/extraction.json`, the bill directory name is `hr9468`, and you'd reference it as `--similar 118-hr9468:0`.

---

## The Hash Chain

Each downstream artifact records the SHA-256 hash of its input, enabling staleness detection:

```text
BILLS-*.xml ──sha256──▶ metadata.json (source_xml_sha256)
                              │
extraction.json ──sha256──▶ bill_meta.json (extraction_sha256)     ← NEW in v4.0
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
                              │
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

If any link in the chain breaks (input file changed but downstream wasn't regenerated), the tool warns but doesn't block. See [Data Integrity and the Hash Chain](../explanation/hash-chain.md) for details.

---

## Immutability Model

Every file except `links/links.json` is **write-once**. The links file is append-only (`link accept` adds entries, `link remove` deletes them):

| File | Written When | Modified When |
|------|-------------|---------------|
| `BILLS-*.xml` | `download` | Never |
| `extraction.json` | `extract`, `upgrade` | Only by deliberate re-extraction or upgrade |
| `verification.json` | `extract`, `upgrade` | Only by deliberate re-extraction or upgrade |
| `metadata.json` | `extract` | Only by re-extraction |
| `tokens.json` | `extract` | Never |
| `bill_meta.json` | `enrich` | Only by re-enrichment (`enrich --force`) |
| `embeddings.json` | `embed` | Only by re-embedding |
| `vectors.bin` | `embed` | Only by re-embedding |
| `chunks/*.json` | `extract` | Never |

This write-once design means:

- **No file locking needed** — multiple read processes can run simultaneously
- **No database needed** — JSON files on disk are the right abstraction for a read-dominated workload
- **No caching needed** — the files ARE the cache
- **Trivially relocatable** — copy a bill directory anywhere and it works

The write:read ratio is approximately 1:500. Bills are extracted ~15 times per year (when Congress enacts new legislation), but queried hundreds to thousands of times.

---

## Git Configuration

The project includes two git-related configurations for the data files:

### .gitignore

```text
chunks/          # Per-chunk LLM artifacts (local provenance, not for distribution)
NEXT_STEPS.md    # Internal context handoff document
.venv/           # Python virtual environment
```

The `chunks/` directory is gitignored because it contains model thinking traces that are useful for local debugging but not needed for downstream operations or distribution.

### .gitattributes

```text
*.bin binary
```

The `vectors.bin` files are marked as binary in git to prevent line-ending conversion and diff attempts on float32 data.

---

## Size Estimates

| Component | H.R. 9468 (Supp) | H.R. 5860 (CR) | H.R. 4366 (Omnibus) |
|-----------|:-----------------:|:---------------:|:-------------------:|
| Source XML | 9 KB | 131 KB | 1.8 MB |
| extraction.json | 15 KB | 200 KB | 12 MB |
| verification.json | 5 KB | 40 KB | 2 MB |
| metadata.json | ~300 B | ~300 B | ~300 B |
| tokens.json | ~200 B | ~200 B | ~200 B |
| bill_meta.json | ~1 KB | ~2 KB | ~5 KB |
| embeddings.json | ~230 B | ~230 B | ~230 B |
| vectors.bin | 86 KB | 1.6 MB | 29 MB |
| chunks/ | ~10 KB | ~100 KB | ~15 MB |
| **Total** | **~120 KB** | **~2 MB** | **~60 MB** |

For 20 congresses (~60 bills), total storage would be approximately 200–400 MB, dominated by `vectors.bin` files for large omnibus bills.

---

## Related References

- **[The Extraction Pipeline](../explanation/pipeline.md)** — how each file is produced
- **[Data Integrity and the Hash Chain](../explanation/hash-chain.md)** — how staleness detection works across files
- **[extraction.json Fields](./extraction-json.md)** — complete field reference for the primary data file
- **[verification.json Fields](./verification-json.md)** — complete field reference for the verification report
- **[embeddings.json Fields](./embeddings-json.md)** — complete field reference for embedding metadata