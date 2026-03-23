# Verifying Extraction Data

The `verify-text` command checks that every provision's `raw_text` field is
a verbatim substring of the enrolled bill source text, and optionally repairs
any discrepancies. After verification, every provision carries a `source_span`
with exact byte positions linking it back to the enrolled bill.

## Quick Start

```bash
# Analyze without modifying anything
congress-approp verify-text --dir data

# Repair mismatches and add source spans
congress-approp verify-text --dir data --repair

# Verify a single bill
congress-approp verify-text --dir data --bill 118-hr2882 --repair
```

## What It Checks

During LLM extraction, the model is instructed to copy the first ~150
characters of each provision's source text verbatim into the `raw_text` field.
In practice, the model occasionally makes small substitutions:

- **Word substitutions**: "clause" instead of "subsection", "on" instead of "in"
- **Quote character differences**: straight quotes (`''`) instead of Unicode curly quotes (`''`)
- **Whitespace normalization**: newlines collapsed into spaces

The `verify-text` command detects these mismatches by searching for each
provision's `raw_text` in the bill's source text file (`BILLS-*.txt`).

## The 3-Tier Repair Algorithm

When `--repair` is specified, mismatched provisions are repaired using a
deterministic algorithm that requires no LLM calls:

### Tier 1: Prefix Match

Find the longest prefix of `raw_text` (15–80 characters) that appears in the
source text. When found, copy the actual source bytes from that position.

This handles single-word substitutions that occur after a long correct prefix.
For example, if the first 80 characters match but then the model wrote "clause"
where the source says "subsection", the prefix matcher finds the correct
position and copies the real text.

### Tier 2: Substring Match

If the prefix is too short (e.g., the provision starts with "(a) " which
appears thousands of times), search for the longest *internal* substring
(starting from various offsets within `raw_text`). Walk backward from the
match position to recover the provision's start.

This handles cases where the first few characters are generic but a distinctive
phrase later in the text is unique in the source.

### Tier 3: Normalized Position Mapping

Build a character-level map between a normalized version of the source
(whitespace and quote characters collapsed) and the original source. Search
in normalized space, then map the hit position back to original byte offsets.

This handles curly-quote vs. straight-quote differences and newline-vs-space
mismatches that the first two tiers cannot resolve.

### Properties

- All three tiers are deterministic: same input produces same output.
- Every repair is guaranteed to be a verbatim substring of the source, because
  the algorithm copies directly from the source text.
- No LLM calls are made. The entire process runs in under 10 seconds for
  34,568 provisions.

## The Source Span Invariant

After `verify-text --repair`, every provision has a `source_span` field:

```json
{
  "source_span": {
    "start": 45892,
    "end": 46042,
    "file": "BILLS-118hr2882enr.txt",
    "verified": true,
    "match_tier": "exact"
  }
}
```

The invariant:

```
source_file_bytes[start .. end] == provision.raw_text
```

where `start` and `end` are **UTF-8 byte offsets** into the source file.

### Byte Offsets vs. Character Offsets

The `start` and `end` values match Rust's native `str` indexing, which operates
on byte positions. In files containing multi-byte UTF-8 characters (such as
curly quotes, which are 3 bytes each), byte offsets differ from character offsets.

To verify the invariant in Python, use byte-level slicing:

```python
import json

extraction = json.load(open("data/118-hr2882/extraction.json"))
source_bytes = open("data/118-hr2882/BILLS-118hr2882enr.txt", "rb").read()

for provision in extraction["provisions"]:
    span = provision.get("source_span")
    if span and span.get("verified"):
        actual = source_bytes[span["start"]:span["end"]].decode("utf-8")
        assert actual == provision["raw_text"], f"Invariant violated at {span}"
```

Do **not** use Python's character-based string slicing (`source_str[start:end]`)
— it will produce incorrect results when the file contains multi-byte characters.

## Match Tiers

The `match_tier` field on each source span records how the span was established:

| Tier | Meaning |
|------|---------|
| `exact` | `raw_text` was already a verbatim substring of the source. No repair needed. |
| `repaired_prefix` | Fixed via Tier 1 — longest prefix match + source byte copy. |
| `repaired_substring` | Fixed via Tier 2 — internal substring match + walk-back. |
| `repaired_normalized` | Fixed via Tier 3 — normalized position mapping. |

## Output

### Analysis mode (no `--repair`)

```text
34568 provisions: 34568 exact, 0 repaired (0 prefix, 0 substring, 0 normalized), 0 unverified
Traceable: 34568/34568 (100.000%)

✅ Every provision is traceable to the enrolled bill source text.
```

### After repair

The command modifies `extraction.json` to:
1. Replace any incorrect `raw_text` with the verbatim source excerpt.
2. Add `source_span` to each provision.

A backup is created at `extraction.json.pre-repair` before any modifications.

### JSON output

```bash
congress-approp verify-text --dir data --format json
```

```json
{
  "total": 34568,
  "exact": 34568,
  "repaired_prefix": 0,
  "repaired_substring": 0,
  "repaired_normalized": 0,
  "unverified": 0,
  "spans_added": 0,
  "traceable_pct": 100.0
}
```

## When to Run

Run `verify-text --repair` once after extraction. The command is idempotent —
running it again on already-repaired data produces no changes (all provisions
are already `exact`).

If you re-extract a bill (`extract --force`), run `verify-text --repair` again
on that bill to update the source spans.

## Technical Details

The `verify-text` command works at the `serde_json::Value` level rather than
through the typed `Provision` enum. This allows it to write the `source_span`
field on each provision object in the JSON without modifying the Rust type
definitions for all 11 provision variants. The field is ignored by the Rust
deserializer (Serde skips unknown fields) but is available to any consumer
reading the JSON directly.