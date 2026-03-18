# Data Integrity and the Hash Chain

Every stage of the extraction pipeline produces files that depend on the output of the previous stage. The XML produces the extraction, the extraction produces the embeddings, and the embeddings enable semantic search. But what happens if you re-download the XML, or re-extract with a different model? The downstream files become stale — they were built from data that no longer matches.

The hash chain is a simple mechanism that detects this staleness automatically. Each downstream artifact records the SHA-256 hash of the input it was built from. When you run a command that uses those artifacts, the tool recomputes the hash and compares. If they don't match, you get a warning.

## The Chain

```text
BILLS-*.xml ──sha256──▶ metadata.json (source_xml_sha256)
                              │
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
                              │
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
```

Three links, each connecting an input to the artifact that records its hash:

### Link 1: Source XML → Metadata

When extraction runs, it computes the SHA-256 hash of the source XML file (`BILLS-*.xml`) and stores it in `metadata.json`:

```json
{
  "model": "claude-opus-4-6",
  "source_xml_sha256": "a3f7b2c4e8d1..."
}
```

If someone re-downloads the XML (perhaps a corrected version was published), the hash in `metadata.json` no longer matches the file on disk. This tells you the extraction was built from a different version of the source.

### Link 2: Extraction → Embeddings

When embeddings are generated, the SHA-256 hash of `extraction.json` is stored in `embeddings.json`:

```json
{
  "schema_version": "1.0",
  "model": "text-embedding-3-large",
  "dimensions": 3072,
  "count": 2364,
  "extraction_sha256": "b5d9e1f3a7c2...",
  "vectors_file": "vectors.bin",
  "vectors_sha256": "c8f2a4b6d0e3..."
}
```

If you re-extract the bill (with a different model, or after a prompt improvement), the new `extraction.json` has a different hash than what `embeddings.json` recorded. The provisions may have changed — different provision count, different classifications, different text — but the embedding vectors still correspond to the old provisions.

### Link 3: Vectors → Embeddings

The SHA-256 hash of `vectors.bin` is also stored in `embeddings.json`. This is an integrity check: if the binary file is corrupted, truncated, or replaced, the hash mismatch is detected.

## How Staleness Detection Works

The `staleness.rs` module implements the checking logic. It's called by commands that depend on embeddings — primarily `search --semantic` and `search --similar`.

### What happens on every query

1. The tool loads `extraction.json` for each bill
2. If the command uses embeddings, it loads `embeddings.json` for each bill
3. It computes the SHA-256 hash of the current `extraction.json` on disk
4. It compares that hash to the `extraction_sha256` stored in `embeddings.json`
5. If they differ, it prints a warning to stderr

### The warning

```text
⚠ H.R. 4366: embeddings are stale (extraction.json has changed)
```

This warning is **advisory only** — it never blocks execution. The tool still runs your query, still computes cosine similarity, and still returns results. But the results may be unreliable because the provision indices in the embedding vectors may not correspond to the current provisions.

### Why warnings don't block

Strict enforcement (refusing to run with stale data) would be frustrating in practice. You might have re-extracted one bill out of twenty and want to run a query across all of them while you regenerate embeddings in the background. The warning tells you what's stale; you decide whether it matters for your current task.

## When Staleness Occurs

| Action | What Becomes Stale | Fix |
|--------|--------------------|-----|
| Re-download XML | extraction.json (built from old XML) | Re-extract: `congress-approp extract --dir <path>` |
| Re-extract bill | embeddings.json + vectors.bin (built from old extraction) | Re-embed: `congress-approp embed --dir <path>` |
| Upgrade extraction data | embeddings.json + vectors.bin (extraction.json changed) | Re-embed: `congress-approp embed --dir <path>` |
| Manually edit extraction.json | embeddings.json + vectors.bin | Re-embed |
| Move files to a new machine | Nothing — hashes are content-based, not path-based | No fix needed |
| Copy bill directory | Nothing — all files move together | No fix needed |

## Automatic Skip for Up-to-Date Bills

The `embed` command uses the hash chain to avoid unnecessary work. When you run:

```bash
congress-approp embed --dir data
```

For each bill, it checks:

1. Does `embeddings.json` exist?
2. Does the stored `extraction_sha256` match the current SHA-256 of `extraction.json`?
3. Does the stored `vectors_sha256` match the current SHA-256 of `vectors.bin`?

If all three pass, the bill is skipped:

```text
Skipping H.R. 9468: embeddings up to date
```

This makes it safe to run `embed --dir data` repeatedly — only bills with new or changed extractions are processed. The same logic applies when running `embed` after upgrading some bills but not others.

## Performance

Hash computation is fast:

| Operation | Time |
|-----------|------|
| SHA-256 of H.R. 9468 extraction.json (~15 KB) | <1ms |
| SHA-256 of H.R. 4366 extraction.json (~12 MB) | ~5ms |
| SHA-256 of H.R. 4366 vectors.bin (~29 MB) | ~8ms |
| **Total for 3 example bills** | **~15ms** |

At scale (20 congresses, ~60 bills), total hashing time would be ~50ms — still negligible compared to the ~10ms JSON parsing time. There is no performance reason to skip or cache hash checks.

The tool always checks — it never caches hash results. Since the check takes milliseconds and the files are immutable in normal operation, this is the right tradeoff: simplicity and correctness over micro-optimization.

## What's NOT in the Hash Chain

### chunks/ directory

The `chunks/` directory contains per-chunk LLM artifacts — thinking traces, raw responses, conversion reports. These are local provenance records for debugging and analysis. They are:

- **Not part of the hash chain** — no downstream artifact records their hashes
- **Not required** for any operation — all query commands work without them
- **Gitignored** by default — they contain model thinking content and aren't meant for distribution

If the chunks are deleted, nothing breaks. They're useful for understanding *why* the LLM classified a provision a certain way, but they're not part of the data integrity chain.

### verification.json

The verification report is regenerated by the `upgrade` command and could be regenerated at any time from `extraction.json` + `BILLS-*.xml`. It's not part of the hash chain because it's a derived artifact — you can always reproduce it from its inputs.

### tokens.json

Token usage records from the extraction are informational only. They don't affect any downstream operation and aren't part of the hash chain.

## The Immutability Model

The hash chain works because of the write-once principle: every file is immutable after creation. This means:

- **No concurrent modification.** Two processes reading the same bill data will never see partially written files.
- **No invalidation logic.** There's nothing to invalidate — files are either current (hashes match) or stale (hashes don't match).
- **No locking.** Read operations don't need to coordinate. Write operations (extract, embed, upgrade) overwrite files atomically.

The one planned exception is `links.json` (not yet implemented), which will be append-only — new links are added, existing links can be removed, but the file grows monotonically. Even this follows a simple consistency model: links reference provision indices in specific bill directories, and if those bills are re-extracted, the links become invalid (detectable via hash chain).

## Verifying Integrity Manually

You can verify the hash chain yourself using standard tools:

### Check extraction against metadata

```bash
# Compute the current SHA-256 of the source XML
shasum -a 256 examples/hr9468/BILLS-118hr9468enr.xml

# Compare to what metadata.json recorded
python3 -c "
import json
meta = json.load(open('examples/hr9468/metadata.json'))
print(f'Recorded: {meta.get(\"source_xml_sha256\", \"NOT SET\")}')
"
```

### Check embeddings against extraction

```bash
# Compute the current SHA-256 of extraction.json
shasum -a 256 examples/hr9468/extraction.json

# Compare to what embeddings.json recorded
python3 -c "
import json
emb = json.load(open('examples/hr9468/embeddings.json'))
print(f'Recorded: {emb[\"extraction_sha256\"]}')
"
```

### Check vectors.bin integrity

```bash
# Compute the current SHA-256 of vectors.bin
shasum -a 256 examples/hr9468/vectors.bin

# Compare to what embeddings.json recorded
python3 -c "
import json
emb = json.load(open('examples/hr9468/embeddings.json'))
print(f'Recorded: {emb[\"vectors_sha256\"]}')
"
```

If all three pairs match, the data is consistent across the entire chain.

## Design Decisions

### Why SHA-256?

SHA-256 is:
- **Collision-resistant** — the probability of two different files producing the same hash is astronomically small
- **Fast** — computing a hash takes milliseconds even for the largest files in the pipeline
- **Standard** — available in every language and platform via the `sha2` crate in Rust, `hashlib` in Python, `shasum` on the command line
- **Deterministic** — the same file always produces the same hash, regardless of when or where it's computed

### Why content-based hashing instead of timestamps?

Timestamps tell you *when* a file was modified, not *whether its content changed*. If you copy a bill directory to a new machine, the timestamps change but the content doesn't. Content-based hashing correctly reports "no staleness" in this case.

Conversely, if you re-extract a bill and the LLM happens to produce identical output, the timestamps change but the content doesn't. Content-based hashing correctly reports "no staleness" here too — the embeddings are still valid because the extraction didn't actually change.

### Why warn instead of error?

Stale embeddings still produce *some* results — they may just not correspond perfectly to the current provisions. In practice, re-extraction often produces very similar provisions (same accounts, same amounts, slightly different wording), so stale embeddings are "mostly correct" even when technically outdated. Blocking execution would be overly strict for this use case.

The warning goes to stderr so it doesn't interfere with stdout output (which may be piped to `jq` or a file).

## Summary

| Component | Records Hash Of | Stored In | Checked When |
|-----------|----------------|-----------|-------------|
| Source XML hash | `BILLS-*.xml` | `metadata.json` | `extract`, `upgrade` |
| Extraction hash | `extraction.json` | `embeddings.json` | `embed`, `search --semantic`, `search --similar` |
| Vectors hash | `vectors.bin` | `embeddings.json` | `embed`, `search --semantic`, `search --similar` |

The hash chain is simple by design — three links, SHA-256, advisory warnings, millisecond overhead. It provides confidence that the artifacts you're querying were built from the data you think they were built from, without imposing any operational burden.

## Next Steps

- **[The Extraction Pipeline](./pipeline.md)** — the five stages that produce the artifacts in the hash chain
- **[Generate Embeddings](../how-to/generate-embeddings.md)** — how the embed command uses the hash chain to skip up-to-date bills
- **[Data Directory Layout](../reference/data-directory.md)** — where each file lives and what it contains