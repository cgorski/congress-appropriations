# Resolving Agency and Account Name Differences Across Bills

When comparing appropriations across fiscal years, the same program sometimes
appears under different agency names. The Army's research budget might be listed
under "Department of Defense—Army" in one bill and "Department of Defense—Department
of the Army" in another. These are the same program, but the tool can't tell
without your help.

The `dataset.json` file at the root of your data directory is where you record
these equivalences. Once recorded, every command — `compare`, `relate`,
`link suggest` — uses them automatically.

## The Problem

Run a Defense comparison and you'll likely see orphan pairs:

```bash
congress-approp compare --base-fy 2024 --current-fy 2026 \
    --subcommittee defense --dir data
```

```text
only in base    "RDT&E, Army"  agency="Department of Defense—Army"         $17.1B
only in current "RDT&E, Army"  agency="Department of Defense—Dept of Army" $16.7B
```

Same account name. Same program. Different agency string. The tool treats them
as different accounts.

## Two Ways to Discover Naming Variants

### `normalize suggest-text-match` — Local analysis

```bash
congress-approp normalize suggest-text-match --dir data
```

Scans your data for orphan pairs (same account name on both sides of a
cross-FY comparison, different agency name) and structural patterns
(preposition variants like "of" vs "for", prefix expansion like
"Defense—Army" vs "Defense—Department of the Army").

Runs entirely offline. No API calls. Instant.

```text
Found 94 suggested agency groups (252 orphan pairs resolvable):

  1. [064847a5] [orphan-pair] "Department of Health and Human Services"
     = "National Institutes of Health"
     Evidence: 27 shared accounts (e.g., national cancer institute, ...)

  2. [3dec4083] [orphan-pair] "Centers for Disease Control and Prevention"
     = "Department of Health and Human Services"
     Evidence: 13 shared accounts (e.g., environmental health, ...)
```

Each suggestion has an 8-character hash for use with `normalize accept`.

Use `--format hashes` to output just the hashes (one per line) for scripting:

```bash
congress-approp normalize suggest-text-match --dir data --format hashes
```

Use `--min-accounts N` to only show pairs sharing N or more account names
(higher = stronger evidence):

```bash
congress-approp normalize suggest-text-match --dir data --min-accounts 3
```

### `normalize suggest-llm` — LLM-assisted classification

```bash
congress-approp normalize suggest-llm --dir data
```

Sends unresolved ambiguous accounts to Claude along with the XML heading
context from each bill. The LLM sees the full organizational structure
surrounding each provision — the `[MAJOR]` and `[SUBHEADING]` headings from
the enrolled bill XML — and classifies agency pairs as SAME or DIFFERENT.

Requires `ANTHROPIC_API_KEY`. Uses Claude Opus.

The LLM uses three types of evidence:

- **XML heading hierarchy** — which department/agency heading the provision
  appears under in the bill structure
- **Dollar amounts** — similar amounts across years suggest the same program
- **Institutional knowledge** — understanding organizational relationships
  (e.g., Space Force is under Department of the Air Force)

Both suggest commands cache their results. Neither writes to `dataset.json`
directly — use `normalize accept` to review and persist.

## Accepting Suggestions

After running either suggest command, accept specific suggestions by hash:

```bash
congress-approp normalize accept 064847a5 3dec4083 --dir data
```

Or accept all cached suggestions at once:

```bash
congress-approp normalize accept --auto --dir data
```

The accept command reads from the suggestion cache
(`~/.congress-approp/cache/`), matches hashes, and writes the accepted
groups to `dataset.json`. If `dataset.json` already exists, new groups
are merged with existing ones.

## What dataset.json Looks Like

Open `data/dataset.json` in any text editor:

```json
{
  "schema_version": "1.0",
  "entities": {
    "agency_groups": [
      {
        "canonical": "Department of Health and Human Services",
        "members": [
          "National Institutes of Health",
          "Centers for Disease Control and Prevention"
        ]
      }
    ],
    "account_aliases": [
      {
        "canonical": "Office for Civil Rights",
        "aliases": ["Office of Civil Rights"]
      }
    ]
  }
}
```

Each **agency group** says: when matching, treat all these agency names as
equivalent. The `canonical` name is what appears in compare output. The
`members` are variants that get mapped to it.

Each **account alias** maps variant spellings of an account name to a
preferred form.

This file contains **only user knowledge** — decisions that cannot be
derived from scanning bill files. There is no cached or derived data.

## How Matching Works

When you run `compare`, `relate`, or `link suggest`, the tool matches
provisions by **(agency, account name)**. Here's exactly what happens:

1. Both agency and account name are lowercased
2. Account name em-dash prefixes are stripped ("Dept—Account" → "account")
3. If `dataset.json` exists, agency names are mapped through the agency groups
4. If `dataset.json` exists, account names are mapped through account aliases
5. Provisions with the same (mapped agency, normalized account) are matched

**No other normalization happens.** The tool does not silently rename agencies
or merge accounts. If two provisions don't match, they appear as orphans —
and you can decide whether to add a group.

When normalization is applied, the compare output marks it:

```text
Account                          Base ($)        Current ($)    Status
RDT&E, Army                      $17,115,037,000 $16,705,760,000 changed (normalized)
Tenant-Based Rental Assistance   $32,386,831,000 $38,438,557,000 changed
```

The `(normalized)` marker tells you this match used an agency group from
`dataset.json`. Matches without the marker are exact. In CSV output,
`normalized` is a separate `true`/`false` column rather than a status suffix.

## Using --exact to Disable Normalization

```bash
congress-approp compare --exact --base-fy 2024 --current-fy 2026 --dir data
```

Ignores `dataset.json` entirely. Every match is exact lowercased strings
only. Use this to see the raw matching results without any entity resolution
applied.

## When dataset.json Doesn't Exist

The tool uses exact matching only. No implicit normalization. This is the
default behavior — explicit and predictable. To create a `dataset.json`:

```bash
congress-approp normalize suggest-text-match --dir data
congress-approp normalize accept --auto --dir data
```

## Viewing Current Rules

```bash
congress-approp normalize list --dir data
```

Displays all agency groups and account aliases currently in `dataset.json`.

## Editing by Hand

You can edit `dataset.json` directly in any text editor. The format is
simple JSON with two sections:

- **`agency_groups`** — each group has a `canonical` name and a list of
  `members` that should be treated as equivalent
- **`account_aliases`** — each alias has a `canonical` name and a list of
  alternative spellings

## Typical Workflow

1. **Run compare**, notice orphan pairs in the output
2. **Run `normalize suggest-text-match`** to discover obvious naming variants
3. **Review suggestions** — check the hashes, evidence, and shared accounts
4. **Accept the ones you trust**: `normalize accept HASH1 HASH2 --dir data`
5. **Re-run compare** — orphans are now matched, marked `(normalized)`
6. **For remaining ambiguous pairs**, run `normalize suggest-llm` for
   LLM-assisted classification with XML evidence
7. **Accept LLM suggestions** the same way: `normalize accept HASH --dir data`

## Tips

- **Start with `suggest-text-match`.** It finds the obvious pairs for free.
  Run `suggest-llm` only for the remaining ambiguous cases.
- **Use `--min-accounts 3`** to focus on the strongest suggestions first —
  pairs sharing 3+ account names are very likely the same agency.
- **Review every suggestion.** Especially from the LLM. Check the reasoning.
- **Verify merges.** After accepting groups, re-run compare and check that
  the merged numbers make sense. If a merged amount looks too high, you may
  have grouped agencies that should be separate.
- **One file per dataset.** The `dataset.json` file is specific to the data
  directory it lives in. Different data directories can have different
  normalization rules.
- **Version control it.** If your data directory is in git, commit
  `dataset.json` alongside your bill data. It records the decisions you
  made about entity identity.
- **Use `--exact` to verify.** At any time, run `compare --exact` to see
  the raw matching results without normalization. This is your ground truth.

## Cache Details

Both suggest commands store their results in `~/.congress-approp/cache/`.
The cache is:

- **Keyed by data directory** — different `--dir` values get separate caches
- **Auto-invalidated** — when any bill's `extraction.json` changes (added,
  removed, or re-extracted), the cache is invalidated and suggest recomputes
- **Read by `normalize accept`** — the accept command reads from cache
  rather than recomputing, making the suggest → accept workflow fast
- **Deletable** — if anything seems wrong, delete `~/.congress-approp/cache/`
  and re-run suggest

## See Also

- **[CLI Command Reference](../reference/cli.md)** — complete flag reference
  for all `normalize` subcommands
- **[Data Directory Layout](../reference/data-directory.md)** — where
  `dataset.json` lives relative to bill data