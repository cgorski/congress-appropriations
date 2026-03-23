# Upgrade Extraction Data

> **You will need:** `congress-approp` installed, existing extracted bill data (with `extraction.json`).
>
> **You will learn:** How to use the `upgrade` command to migrate extraction data to the latest schema version, re-verify against current code, and update files — all without making any LLM API calls.

The `upgrade` command is your tool for keeping extraction data current without re-extracting. When the tool's schema evolves — new fields, renamed fields, new verification checks, or updated deserialization logic — `upgrade` applies those changes to your existing data. It re-deserializes each bill's `extraction.json` through the current code's parsing logic, re-runs deterministic verification against the source XML, and writes updated files.

**No LLM API calls are made.** Upgrade is fast, free, and safe.

## When to Use Upgrade

Use `upgrade` when:

- **You've updated `congress-approp` to a new version** that includes schema changes, new provision type handling, or improved verification logic. The upgrade command applies those improvements to your existing extractions.
- **You want to re-verify without re-extracting.** Maybe you suspect the verification logic has been improved, or you want to check data integrity after moving files between systems.
- **You see schema version warnings.** If your data was extracted with an older schema version and the tool detects this, it may suggest running `upgrade`.
- **You want to normalize data.** Upgrade re-serializes through the current schema, which normalizes field names, fills in defaults for new fields, and standardizes enum values.

**Do NOT use `upgrade` when:**

- **You want a fresh extraction with a different model.** Use `extract` instead — that makes new LLM API calls.
- **Your source XML has changed.** If you re-downloaded the bill, you need to re-extract, not upgrade.

## Preview Before Upgrading

Always start with a dry run:

```bash
congress-approp upgrade --dir data --dry-run
```

This shows what would change for each bill without writing any files:

- Which bills would be upgraded
- Whether the schema version would change
- How many provisions would be re-parsed
- Whether verification results would differ

No files are modified during a dry run.

## Run the Upgrade

### Upgrade all bills in a directory

```bash
congress-approp upgrade --dir data
```

The tool walks recursively from the specified directory, finds every `extraction.json`, and upgrades each one. For each bill:

1. **Load** the existing `extraction.json`
2. **Re-deserialize** every provision through the current `from_value.rs` parsing logic, which handles missing fields, type coercions, and unknown provision types
3. **Re-compute** the `schema_version` field
4. **Re-run verification** against the source XML (if `BILLS-*.xml` is present in the same directory)
5. **Write** updated `extraction.json` and `verification.json`

### Upgrade a single bill

```bash
congress-approp upgrade --dir data/118/hr/9468
```

### Verbose output

Add `-v` for detailed logging:

```bash
congress-approp upgrade --dir data -v
```

This shows per-provision details: which fields were defaulted, which types were coerced, and any warnings from the deserialization process.

## What Upgrade Changes

### extraction.json

- **`schema_version`** is set to the current version
- **New fields** added in recent versions get their default values (e.g., a new `Option<String>` field defaults to `null`)
- **Renamed fields** are mapped from old names to new names
- **Type coercions** are applied — for example, if a dollar amount was stored as a string `"$10,000,000"` in an old extraction, upgrade converts it to the integer `10000000`
- **Unknown provision types** that have since been added to the schema are re-parsed into their proper variant instead of falling back to `Other`

The provision data itself is not re-generated — upgrade works with whatever the LLM originally produced. It only normalizes the *representation*, not the *content*.

### verification.json

Verification is fully re-run against the source XML:

- **Amount checks** — Every `text_as_written` dollar string is searched for in the source text
- **Raw text checks** — Every `raw_text` excerpt is checked as a substring of the source (exact → normalized → spaceless → no match)
- **Completeness** — The percentage of dollar strings in the source text matched to extracted provisions is recomputed

If the source XML (`BILLS-*.xml`) is not present in the bill directory, verification is skipped and the existing `verification.json` is left unchanged.

### metadata.json

The `source_xml_sha256` field is added or updated if the source XML is present. This is part of the hash chain that enables staleness detection for downstream artifacts (embeddings).

### What is NOT changed

- **The provisions themselves** — the LLM's original extraction is preserved. Upgrade doesn't re-classify provisions, change account names, or modify dollar amounts.
- **tokens.json** — Token usage records from the original extraction are untouched.
- **chunks/** — Per-chunk LLM artifacts are not modified.
- **embeddings.json / vectors.bin** — Embeddings are not regenerated. If the upgrade changes `extraction.json`, the embeddings become stale. The tool will warn you about this, and you can run `embed` to regenerate.

## Handling the SuchSums Fix

One specific issue that `upgrade` addresses: in early versions, `SuchSums` amount variants (for "such sums as may be necessary" provisions) could serialize incorrectly. The upgrade command detects and fixes this, converting them to the proper tagged enum format. This is transparent — you don't need to do anything special.

## After Upgrading

### Check the audit

Run `audit` to see whether verification metrics improved:

```bash
congress-approp audit --dir data
```

If the upgrade applied new verification logic, you may see changes in the Exact/NormText/TextMiss columns. The NotFound column should remain at 0 (it would only increase if the upgrade somehow corrupted dollar amount strings, which it doesn't).

### Check for stale embeddings

If upgrade modified `extraction.json`, the hash chain detects that embeddings are stale:

```text
⚠ H.R. 4366: embeddings are stale (extraction.json has changed)
```

Regenerate embeddings if you use semantic search:

```bash
congress-approp embed --dir data
```

### Verify budget authority totals

As a sanity check, confirm that budget authority totals haven't changed:

```bash
congress-approp summary --dir data --format json
```

Upgrade should never change the dollar amounts in provisions, so budget authority totals should be identical before and after. If they differ, something unexpected happened — file a bug report.

For the included example data, the expected totals are:

| Bill | Budget Authority | Rescissions |
|------|-----------------|-------------|
| H.R. 4366 | $846,137,099,554 | $24,659,349,709 |
| H.R. 5860 | $16,000,000,000 | $0 |
| H.R. 9468 | $2,882,482,000 | $0 |

## Upgrade vs. Re-Extract: Decision Guide

| Situation | Use `upgrade` | Use `extract` |
|-----------|:---:|:---:|
| Updated to a new version of congress-approp | ✓ | |
| Want to try a different LLM model | | ✓ |
| Schema version is outdated | ✓ | |
| Low coverage — want more provisions extracted | | ✓ |
| Verification logic improved | ✓ | |
| Source XML was re-downloaded | | ✓ |
| Want to normalize field names and types | ✓ | |
| NotFound > 0 and you suspect extraction errors | | ✓ |

**Key principle:** `upgrade` preserves the LLM's work and improves how it's stored and verified. `extract` discards the LLM's work and starts over.

## Troubleshooting

### "No extraction.json found"

The `upgrade` command only processes directories that already contain `extraction.json`. If you haven't extracted a bill yet, use `extract` first.

### "No source XML found — skipping verification"

Upgrade re-runs verification against the source XML. If the `BILLS-*.xml` file isn't in the bill directory (maybe you moved files around), verification is skipped. The extraction data is still upgraded, but `verification.json` won't be updated.

To fix, make sure the source XML is in the same directory as `extraction.json`:

```bash
ls data/118/hr/9468/
# Should show both BILLS-118hr9468enr.xml and extraction.json
```

### Budget authority totals changed after upgrade

This should not happen. If it does:

1. Compare the pre-upgrade and post-upgrade `extraction.json` using `diff` or a JSON diff tool
2. Look for provisions whose `detail_level` or `semantics` changed — these fields affect the budget authority calculation
3. File a bug report with the before/after data

## Quick Reference

```bash
# Preview what would change (no files modified)
congress-approp upgrade --dir data --dry-run

# Upgrade all bills under a directory
congress-approp upgrade --dir data

# Upgrade a single bill
congress-approp upgrade --dir data/118/hr/9468

# Upgrade with verbose logging
congress-approp upgrade --dir data -v

# Verify after upgrading
congress-approp audit --dir data

# Regenerate stale embeddings after upgrade
congress-approp embed --dir data
```

## Full Command Reference

```text
congress-approp upgrade [OPTIONS]

Options:
    --dir <DIR>  Data directory to upgrade [default: ./data]
    --dry-run    Show what would change without writing files
```

## Next Steps

- **[Verify Extraction Accuracy](./verify-accuracy.md)** — run a full audit after upgrading
- **[Extract Provisions from a Bill](./extract-provisions.md)** — when upgrade isn't enough and you need a fresh extraction
- **[Data Integrity and the Hash Chain](../explanation/hash-chain.md)** — understand how the hash chain detects stale artifacts