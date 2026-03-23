# Resolving Treasury Account Symbols

Every federal budget account has a Federal Account Symbol (FAS) — a stable
identifier assigned by the Treasury Department that persists through account
renames and reorganizations. The `resolve-tas` command maps each extracted
appropriation provision to its FAS code, enabling cross-bill account tracking
regardless of how Congress names the account in different years.

## Why TAS Resolution Matters

The same budget account can appear under different names across bills:

| Fiscal Year | Bill | Account Name |
|-------------|------|-------------|
| FY2020 | H.R. 1158 | United States Secret Service—Operations and Support |
| FY2022 | H.R. 2471 | Operations and Support |
| FY2024 | H.R. 2882 | Operations and Support |

Without TAS resolution, these look like different accounts. With it, all three
map to FAS code `070-0400` — the same Treasury account.

## The FAST Book Reference

The tool ships with `fas_reference.json`, derived from the Federal Account
Symbols and Titles (FAST) Book published by the Bureau of the Fiscal Service
at [tfx.treasury.gov](https://tfx.treasury.gov/reference-books/fast-book).
This reference contains:

- **2,768 active FAS codes** across 156 agencies
- **485 discontinued General Fund accounts** from the Changes sheet
- Official titles, agency names, fund types, and legislation references

The FAS code format is `{agency_code}-{main_account}`:
- `070-0400` → agency 070 (DHS), main account 0400 (Secret Service Ops)
- `021-2020` → agency 021 (Army), main account 2020 (Operation and Maintenance)
- `075-0350` → agency 075 (HHS), main account 0350 (NIH)

## Running TAS Resolution

### Preview what will happen (no API calls)

```bash
congress-approp resolve-tas --dir data --dry-run
```

This shows how many provisions need resolution per bill and estimates the LLM cost:

```text
  H.R. 2882             448/491  deterministic, 43 need LLM (~$1.29)
  H.R. 4366             467/498  deterministic, 31 need LLM (~$0.93)
```

### Deterministic only (free, no API key)

```bash
congress-approp resolve-tas --dir data --no-llm
```

Matches provisions against the FAST Book using string comparison. Handles
~56% of provisions — those with unique account names or where the agency
code disambiguates among multiple candidates. Zero false positives.

### Full resolution (deterministic + LLM)

```bash
congress-approp resolve-tas --dir data
```

Provisions that cannot be matched deterministically are sent to Claude Opus
in batches, grouped by agency. The LLM receives the provision's account name,
agency, and dollar amount along with all FAS codes for that agency. Each
returned FAS code is verified against the FAST Book — if the code is not in
the reference, the match is flagged as `inferred` rather than `high`.

Achieves ~99.4% resolution across the full dataset.

### Resolve a single bill

```bash
congress-approp resolve-tas --dir data --bill 118-hr2882
```

### Re-resolve after changes

```bash
congress-approp resolve-tas --dir data --bill 118-hr2882 --force
```

## How the Two-Tier Matching Works

### Tier 1: Deterministic (free, instant)

For each top-level budget authority appropriation:

1. **Direct match**: Lowercase the account name, look up in the FAS short-title
   index. If exactly one FAS code has this title, match it.

2. **Short-title match**: Extract the first comma-delimited segment of the
   account name (e.g., "Operation and Maintenance" from "Operation and
   Maintenance, Army"). Look up in the index. If unique, match.

3. **Suffix match**: Strip any em-dash agency prefix (e.g., "United States
   Secret Service—Operations and Support" → "Operations and Support"). Look up
   the suffix. If unique, match.

4. **Agency disambiguation**: If multiple FAS codes share the same title (151
   agencies have "Salaries and Expenses"), use the provision's agency to narrow
   the candidates. If exactly one candidate matches the agency, match it.

5. **DOD service branch detection**: When the agency is "Department of Defense"
   but the account name contains ", Army", ", Navy", ", Air Force", etc., the
   resolver uses the service-specific CGAC code (021, 017, 057) instead of the
   DOD umbrella code (097).

If none of these strategies produce a single unambiguous match, the provision
is left `unmatched` for the LLM tier.

### Tier 2: LLM (requires ANTHROPIC_API_KEY)

Unmatched provisions are batched by agency and sent to Claude Opus with:
- The provision's account name, agency, and dollar amount
- All FAS codes for that agency from the FAST Book

The LLM returns a FAS code and reasoning for each provision. Each returned
code is verified against the FAST Book. Codes confirmed in the reference are
marked `high` confidence; codes the LLM knows from training but that are not
in the reference are marked `inferred`.

## Understanding the Output

The command produces `tas_mapping.json` per bill:

```json
{
  "schema_version": "1.0",
  "bill_identifier": "H.R. 2882",
  "fas_reference_hash": "a1b2c3...",
  "mappings": [
    {
      "provision_index": 0,
      "account_name": "Operations and Support",
      "agency": "United States Secret Service",
      "dollars": 3007982000,
      "fas_code": "070-0400",
      "fas_title": "Operations and Support, United States Secret Service, Homeland Security",
      "confidence": "verified",
      "method": "direct_match"
    }
  ],
  "summary": {
    "total_provisions": 491,
    "deterministic_matched": 448,
    "llm_matched": 39,
    "unmatched": 4,
    "match_rate_pct": 99.2
  }
}
```

### Confidence levels

| Level | Meaning |
|-------|---------|
| `verified` | Deterministic match confirmed against the FAST Book. Mechanically provable. |
| `high` | LLM matched, and the FAS code exists in the FAST Book. |
| `inferred` | LLM matched, but the FAS code is not in the FAST Book (known from training data). |
| `unmatched` | Could not resolve. Typically edge cases: Postal Service, intelligence community, newly created accounts. |

### Match methods

| Method | How the match was made |
|--------|----------------------|
| `direct_match` | Account name uniquely matched one FAS short title. |
| `suffix_match` | After stripping the em-dash agency prefix, the suffix uniquely matched. |
| `agency_disambiguated` | Multiple FAS codes shared the title, but the agency code narrowed to one. |
| `llm_resolved` | Claude Opus provided the mapping. |

## The 40 Unmatched Provisions

Across the full 32-bill dataset, 40 provisions (0.6%) could not be resolved
even with the LLM. These are genuine edge cases:

- **Postal Service accounts** — USPS has its own funding structure
- **Intelligence community accounts** — classified budget lines
- **FDIC Inspector General** — FDIC is self-funded
- **Newly created programs** — not yet in the FAST Book

These 40 provisions represent less than 0.05% of total budget authority.

## Updating the FAST Book Reference

The FAST Book is updated periodically by the Bureau of the Fiscal Service.
To refresh the bundled reference data:

1. Download the updated Excel from
   [tfx.treasury.gov/reference-books/fast-book](https://tfx.treasury.gov/reference-books/fast-book)
2. Save as `tmp/fast_book_part_ii_iii.xlsx`
3. Run: `python scripts/convert_fast_book.py`
4. The updated `data/fas_reference.json` will be generated
5. Re-run `resolve-tas --force` to apply the new reference

## Cost Summary

| Scenario | Cost | What you get |
|----------|------|-------------|
| `--no-llm` (free) | $0 | ~56% of provisions resolved deterministically |
| Full resolution (one bill) | $1–4 | ~99% resolution for that bill |
| Full resolution (32 bills) | ~$85 | 99.4% resolution across the dataset |

This is a one-time cost per bill. Once `tas_mapping.json` is produced, the
FAS codes are permanent — they do not change unless the bill is re-extracted.