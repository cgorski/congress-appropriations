# Recipes & Demos

Worked examples using the included 32-bill dataset (`data/`). All commands run locally against the pre-extracted data with no API keys unless noted. Semantic search requires `OPENAI_API_KEY`.

The `book/cookbook/cookbook.py` script reproduces all CSVs, charts, and JSON shown on this page. See [Run All Demos Yourself](#run-all-demos-yourself) at the bottom.

---

## Dataset Overview

| | |
|---|---|
| **116th Congress** (2019–2021) | 11 bills — FY2019, FY2020, FY2021 |
| **117th Congress** (2021–2023) | 7 bills — FY2021, FY2022, FY2023 |
| **118th Congress** (2023–2025) | 10 bills — FY2024, FY2025 |
| **119th Congress** (2025–2027) | 4 bills — FY2025, FY2026 |
| **Total** | **32 bills, 34,568 provisions, $21.5 trillion in budget authority** |
| **Accounts tracked** | 1,051 unique Federal Account Symbols across 937 cross-bill links |
| **Source traceability** | 100% — every provision has exact byte positions in the enrolled bill |
| **Dollar verification** | 99.995% — 18,583 of 18,584 dollar amounts confirmed in source text |

### Subcommittee coverage by fiscal year

The `--subcommittee` filter requires bills with separate divisions per jurisdiction. FY2025 was funded through H.R. 1968, a full-year [continuing resolution](../reference/glossary.md) that wraps all 12 subcommittees into a single division — so `--subcommittee` cannot break it apart. Use `trace` or `search --fy 2025` to access FY2025 data by account.

| Fiscal Year | Subcommittee filter | Notes |
|---|---|---|
| FY2019 | Partial | Only supplemental and disaster relief bills |
| FY2020–FY2024 | ✅ Full | Traditional omnibus/minibus bills with per-subcommittee divisions |
| FY2025 | ❌ Not available | Funded via full-year CR (H.R. 1968) — all jurisdictions in one division |
| FY2026 | ✅ Full | Three bills cover all 12 subcommittees |

---

## Quick Reference

```bash
# Track any federal account across all fiscal years (by FAS code or name search)
congress-approp trace "child nutrition" --dir data

# Budget totals for FY2026
congress-approp summary --dir data --fy 2026

# Find FEMA provisions across all bills covering FY2026
congress-approp search --dir data --keyword "Federal Emergency Management" --fy 2026

# Compare THUD funding FY2024 → FY2026 with inflation adjustment
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir data --use-authorities --real

# Verification quality across all 32 bills
congress-approp audit --dir data
```

---

## Searching and Tracking Accounts

### Keyword search

The `--keyword` flag searches the `raw_text` field — the verbatim bill language stored with each provision. It is case-insensitive. Combine with `--type` to filter by provision type, `--fy` by fiscal year, `--agency` by department, or `--min-dollars` / `--max-dollars` for dollar ranges. All filters are ANDed.

```bash
congress-approp search --dir data --keyword "veterans" --type appropriation
```

```text
┌───┬───────────┬───────────────┬───────────────────────────────────────────────┬─────────────────┬─────────┬─────┐
│ $ ┆ Bill      ┆ Type          ┆ Description / Account                         ┆      Amount ($) ┆ Section ┆ Div │
╞═══╪═══════════╪═══════════════╪═══════════════════════════════════════════════╪═════════════════╪═════════╪═════╡
│ ✓ ┆ H.R. 133  ┆ appropriation ┆ Compensation and Pensions                     ┆   6,110,251,552 ┆         ┆ J   │
│ ✓ ┆ H.R. 133  ┆ appropriation ┆ Readjustment Benefits                         ┆  14,946,618,000 ┆         ┆ J   │
│ ✓ ┆ H.R. 133  ┆ appropriation ┆ General Operating Expenses, Veterans Benefit… ┆   3,180,000,000 ┆         ┆ J   │
│ ...                                                                                                              │
```

**Column reference:**

| Column | Meaning |
|---|---|
| **$** | Dollar amount verification status. **✓** = dollar string found at one unique position in the [enrolled](../reference/glossary.md) bill text. **≈** = found at multiple positions (common for round numbers) — correct but location ambiguous. **✗** = not found in source — needs review. Blank = provision has no dollar amount. |
| **Bill** | The enacted legislation this provision comes from |
| **Type** | Provision classification: `appropriation` (grant of [budget authority](../reference/glossary.md)), `rescission` (cancellation of prior funds), `transfer_authority` (permission to move funds), `rider` (policy provision, no spending), `directive` (reporting requirement), `limitation` (spending cap), `cr_substitution` ([CR](../reference/glossary.md) anomaly replacing one dollar amount with another), and others |
| **Description / Account** | Account name (for appropriations, rescissions) or description text (for riders, directives). This is the name as written in the bill text, between `''` delimiters. |
| **Amount ($)** | Budget authority in dollars. **—** = provision carries no dollar value. |
| **Section** | Section reference in the bill (e.g., `SEC. 1701`). Empty if no numbered section. |
| **Div** | Division letter for omnibus/minibus bills. Division letters are bill-internal — Division A means different things in different bills. |

---

### Tracking an account across fiscal years

The `trace` command follows a single federal account across every bill in the dataset using its [Federal Account Symbol](../reference/glossary.md) (FAS code) — a government-assigned identifier that persists through name changes and reorganizations.

**Finding the FAS code by name:**

```bash
congress-approp trace "child nutrition" --dir data
```

If the name matches multiple accounts, the tool lists them with their FAS codes. Use the code for the specific account:

```bash
congress-approp trace 012-3539 --dir data
```

```text
TAS 012-3539: Child Nutrition Programs, Food and Nutrition Service, Agriculture
  Agency: Department of Agriculture

┌──────┬──────────────────────┬───────────┬──────────────────────────┐
│ FY   ┆ Budget Authority ($) ┆ Bill(s)   ┆ Account Name(s)          │
╞══════╪══════════════════════╪═══════════╪══════════════════════════╡
│ 2020 ┆       23,615,098,000 ┆ H.R. 1865 ┆ Child Nutrition Programs │
│ 2021 ┆       25,118,440,000 ┆ H.R. 133  ┆ Child Nutrition Programs │
│ 2022 ┆       26,883,922,000 ┆ H.R. 2471 ┆ Child Nutrition Programs │
│ 2023 ┆       28,545,432,000 ┆ H.R. 2617 ┆ Child Nutrition Programs │
│ 2024 ┆       33,266,226,000 ┆ H.R. 4366 ┆ Child Nutrition Programs │
│ 2026 ┆       37,841,674,000 ┆ H.R. 5371 ┆ Child Nutrition Programs │
└──────┴──────────────────────┴───────────┴──────────────────────────┘

  6 fiscal years, 6 bills, 175,270,792,000 total
```

| Column | Meaning |
|---|---|
| **FY** | Federal fiscal year (Oct 1 – Sep 30). FY2024 = Oct 2023 – Sep 2024. |
| **Budget Authority ($)** | What Congress authorized the agency to obligate. This is [budget authority](../reference/glossary.md), not outlays. |
| **Bill(s)** | Enacted legislation providing the funding. `(CR)` = continuing resolution; `(supplemental)` = emergency funding. |
| **Account Name(s)** | Account name as written in each bill. May vary across congresses — the FAS code is the stable identifier. |

FY2025 is absent here because H.R. 1968 (the full-year CR) continued FY2024 rates without a separate line item for this account.

**Accounts with name changes** demonstrate why FAS codes are necessary for cross-bill tracking:

```bash
congress-approp trace 070-0400 --dir data
```

```text
TAS 070-0400: Operations and Support, United States Secret Service, Homeland Security
  Agency: Department of Homeland Security

┌──────┬──────────────────────┬────────────────┬─────────────────────────────────────────────┐
│ FY   ┆ Budget Authority ($) ┆ Bill(s)        ┆ Account Name(s)                             │
╞══════╪══════════════════════╪════════════════╪═════════════════════════════════════════════╡
│ 2020 ┆        2,336,401,000 ┆ H.R. 1158      ┆ United States Secret Service—Operations an… │
│ 2021 ┆        2,373,109,000 ┆ H.R. 133       ┆ United States Secret Service—Operations an… │
│ 2022 ┆        2,554,729,000 ┆ H.R. 2471      ┆ Operations and Support                      │
│ 2023 ┆        2,734,267,000 ┆ H.R. 2617      ┆ Operations and Support                      │
│ 2024 ┆        3,007,982,000 ┆ H.R. 2882      ┆ Operations and Support                      │
│ 2025 ┆          231,000,000 ┆ H.R. 9747 (CR) ┆ United States Secret Service—Operations an… │
└──────┴──────────────────────┴────────────────┴─────────────────────────────────────────────┘

  Name variants across bills:
    "Operations and Support" (117-hr2471, 117-hr2617, 118-hr2882) [prefix]
    "United States Secret Service—Operations and Sup…" (116-hr1158, 116-hr133, 118-hr9747) [canonical]

  6 fiscal years, 6 bills, 13,237,488,000 total
```

The account was renamed between the 116th and 117th Congress — the "United States Secret Service—" prefix was dropped. FAS code `070-0400` unifies both names. The FY2025 row shows $231M from H.R. 9747 (a CR supplement), not the full-year level.

---

### Semantic search

When the official program name is unknown, semantic search matches provisions by meaning rather than keywords. Requires `OPENAI_API_KEY` (one API call per query, ~100ms).

```bash
export OPENAI_API_KEY="your-key"
congress-approp search --dir data --semantic "school lunch programs for kids" --top 3
```

```text
┌──────┬───────────────────┬───────────────┬──────────────────────────┬────────────────┐
│ Sim  ┆ Bill              ┆ Type          ┆ Description / Account    ┆     Amount ($) │
╞══════╪═══════════════════╪═══════════════╪══════════════════════════╪════════════════╡
│ 0.52 ┆ H.R. 1865 (116th) ┆ appropriation ┆ Child Nutrition Programs ┆ 23,615,098,000 │
│ 0.51 ┆ H.R. 4366 (118th) ┆ appropriation ┆ Child Nutrition Programs ┆ 33,266,226,000 │
│ 0.51 ┆ H.R. 2471 (117th) ┆ appropriation ┆ Child Nutrition Programs ┆ 26,883,922,000 │
└──────┴───────────────────┴───────────────┴──────────────────────────┴────────────────┘
```

"school lunch programs for kids" shares no keywords with "Child Nutrition Programs", but semantic search matches them by meaning. The **Sim** column is cosine similarity between the query and provision embeddings:

| Sim Score | Interpretation |
|---|---|
| > 0.80 | Almost certainly the same program (when comparing provisions across bills) |
| 0.60–0.80 | Related topic, same policy area |
| 0.45–0.60 | Loosely related |
| < 0.45 | Unlikely to be meaningfully related |

Scores reflect the full provision text (account name + agency + raw bill language), not just the account name, which is why good matches are often in the 0.45–0.55 range rather than near 1.0.

**Additional examples** (tested against the dataset):

| Query | Top Result | Sim |
|---|---|---|
| opioid crisis drug treatment | Substance Abuse Treatment | 0.48 |
| space exploration | Exploration (NASA) | 0.57 |
| military pay raises for soldiers | Military Personnel, Army | 0.53 |
| fighting wildfires | Wildland Fire Management | 0.53 |
| veterans mental health | VA mental health counseling directives | 0.53 |

---

## Comparing Across Fiscal Years

### Year-over-year comparison with inflation adjustment

```bash
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud \
    --dir data --use-authorities --real
```

| Flag | Purpose |
|---|---|
| `--base-fy 2024` | Use all bills covering FY2024 as the baseline |
| `--current-fy 2026` | Use all bills covering FY2026 as the comparison |
| `--subcommittee thud` | Scope to Transportation, Housing and Urban Development. The tool resolves which division in each bill corresponds to THUD. |
| `--use-authorities` | Match accounts using Treasury Account Symbols instead of name strings. Handles renames and agency reorganizations. |
| `--real` | Add inflation-adjusted columns using bundled CPI-U data. |

```text
20 orphan(s) rescued via TAS authority matching
Comparing: H.R. 4366 (118th)  →  H.R. 7148 (119th)

┌─────────────────────────────────────┬──────────────────────┬────────────────┬────────────────┬─────────────────┬─────────┬───────────┬───┬──────────┐
│ Account                             ┆ Agency               ┆       Base ($) ┆    Current ($) ┆       Delta ($) ┆     Δ % ┆ Real Δ %* ┆   ┆ Status   │
╞═════════════════════════════════════╪══════════════════════╪════════════════╪════════════════╪═════════════════╪═════════╪═══════════╪═══╪══════════╡
│ Tenant-Based Rental Assistance      ┆ Department of Housi… ┆ 32,386,831,000 ┆ 38,438,557,000 ┆  +6,051,726,000 ┆  +18.7% ┆    +13.8% ┆ ▲ ┆ changed  │
│ Federal-Aid Highways                ┆ Federal Highway Adm… ┆ 60,834,782,888 ┆ 63,396,105,821 ┆  +2,561,322,933 ┆   +4.2% ┆     -0.1% ┆ ▼ ┆ changed  │
│ Operations                          ┆ Federal Aviation Ad… ┆ 12,729,627,000 ┆ 13,710,000,000 ┆    +980,373,000 ┆   +7.7% ┆     +3.2% ┆ ▲ ┆ changed  │
│ Facilities and Equipment            ┆ Federal Aviation Ad… ┆  3,191,250,000 ┆  4,000,000,000 ┆    +808,750,000 ┆  +25.3% ┆    +20.1% ┆ ▲ ┆ changed  │
│ Capital Investment Grants           ┆ Federal Transit Adm… ┆  2,205,000,000 ┆  1,700,000,000 ┆    -505,000,000 ┆  -22.9% ┆    -26.1% ┆ ▼ ┆ changed  │
│ Public Housing Fund                 ┆ Department of Housi… ┆  8,810,784,000 ┆  8,319,393,000 ┆    -491,391,000 ┆   -5.6% ┆     -9.5% ┆ ▼ ┆ changed  │
│ ...                                 ┆                      ┆                ┆                ┆                 ┆         ┆           ┆   ┆          │
```

**Column reference:**

| Column | Meaning |
|---|---|
| **Account** | Appropriations account name, matched between the two fiscal years |
| **Agency** | Parent department or agency |
| **Base ($)** | Total budget authority for this account in FY2024 |
| **Current ($)** | Total budget authority in FY2026 |
| **Delta ($)** | Current minus Base |
| **Δ %** | Nominal percentage change (not inflation-adjusted) |
| **Real Δ %\*** | Inflation-adjusted percentage change using CPI-U data. Asterisk indicates this is computed from a price index, not a number verified against bill text. |
| **▲ / ▼ / —** | ▲ = real increase (beat inflation), ▼ = real cut or inflation erosion, — = unchanged |
| **Status** | `changed` = in both FYs, different amounts. `unchanged` = same amount. `only in base` = not in FY2026. `only in current` = new in FY2026. `matched (TAS …) (normalized)` = matched via Treasury Account Symbol because the name differed. |

The Federal-Aid Highways row illustrates why inflation adjustment matters: nominal +4.2%, but real -0.1%. The nominal increase does not keep pace with inflation.

The `--real` flag works on any `compare` command — any subcommittee, any fiscal year pair. No API key needed.

The "20 orphan(s) rescued via TAS authority matching" message indicates 20 accounts that would have appeared unmatched (different names between FY2024 and FY2026) were paired using their FAS codes.

---

### Subcommittee budget authority across fiscal years

Individual subcommittee totals can be retrieved per fiscal year using `summary --fy Y --subcommittee S`. The `book/cookbook/cookbook.py` script runs all combinations; the resulting table:

| Subcommittee | FY2020 | FY2021 | FY2022 | FY2023 | FY2024 | FY2026 | Change |
|---|---|---|---|---|---|---|---|
| Defense | $693B | $695B | $723B | $791B | $819B | $836B | +21% |
| Labor-HHS | $1,089B | $1,167B | $1,305B | $1,408B | $1,435B | $1,729B | +59% |
| THUD | $97B | $87B | $112B | $162B | $184B | $183B | +88% |
| MilCon-VA | $256B | $272B | $316B | $332B | $360B | $495B | +94% |
| Homeland Security | $73B | $75B | $81B | $85B | $88B | — | +20% |
| Agriculture | $120B | $205B | $197B | $212B | $187B | $177B | +48% |
| CJS | $84B | $81B | $84B | $89B | $88B | $88B | +5% |
| Energy & Water | $50B | $53B | $57B | $61B | $63B | $69B | +38% |
| Interior | $37B | $37B | $39B | $45B | $40B | $40B | +7% |
| State-Foreign Ops | $56B | $62B | $59B | $61B | $62B | $53B | -6% |
| Financial Services | $37B | $38B | $39B | $41B | $40B | $41B | +11% |
| Legislative Branch | $5B | $5B | $6B | $7B | $7B | $7B | +43% |

FY2025 is omitted for individual subcommittees because it was funded through a full-year CR with all jurisdictions under one division — see the [coverage note](#subcommittee-coverage-by-fiscal-year) above.

All values are [budget authority](../reference/glossary.md). These include mandatory spending programs that appear as appropriation lines (e.g., SNAP under Agriculture, Medicaid under Labor-HHS). The MilCon-VA figure ($495B for FY2026) includes $394B in advance appropriations — see the next section.

---

### Advance vs. current-year appropriations

```bash
congress-approp summary --dir data --fy 2026 --subcommittee milcon-va --show-advance
```

```text
┌───────────────────┬──────┬────────────────┬────────────┬─────────────────┬─────────────────┬─────────────────┬─────────────────┬─────────────────┐
│ Bill              ┆ FYs  ┆ Classification ┆ Provisions ┆     Current ($) ┆     Advance ($) ┆    Total BA ($) ┆ Rescissions ($) ┆      Net BA ($) │
╞═══════════════════╪══════╪════════════════╪════════════╪═════════════════╪═════════════════╪═════════════════╪═════════════════╪═════════════════╡
│ H.R. 5371 (119th) ┆ 2026 ┆ Minibus        ┆        263 ┆ 101,839,976,450 ┆ 393,592,053,000 ┆ 495,432,029,450 ┆  16,499,000,000 ┆ 478,933,029,450 │
│ TOTAL             ┆      ┆                ┆        263 ┆ 101,839,976,450 ┆ 393,592,053,000 ┆ 495,432,029,450 ┆  16,499,000,000 ┆ 478,933,029,450 │
└───────────────────┴──────┴────────────────┴────────────┴─────────────────┴─────────────────┴─────────────────┴─────────────────┴─────────────────┘
```

| Column | Meaning |
|---|---|
| **Current ($)** | Budget authority available in the current fiscal year (FY2026) |
| **Advance ($)** | Budget authority enacted in this bill but available starting in a future fiscal year (FY2027+). Common for VA medical accounts. |
| **Total BA ($)** | Current + Advance. This is the number shown without `--show-advance`. |
| **Rescissions ($)** | Cancellations of previously enacted budget authority (absolute value) |
| **Net BA ($)** | Total BA minus Rescissions |

79.4% of FY2026 MilCon-VA budget authority ($394B of $495B) is advance appropriations for FY2027. Only $102B is current-year spending. Without `--show-advance`, the total combines both, which can distort year-over-year comparisons by hundreds of billions of dollars.

The classification uses `bill_meta.json` generated by `enrich` (run once, no API key). The algorithm compares each provision's availability dates against the bill's fiscal year.

---

### CR substitutions — what the continuing resolution changed

[Continuing resolutions](../reference/glossary.md) fund the government at prior-year rates, except for specific anomalies (CR substitutions) where Congress sets a different level.

```bash
congress-approp search --dir data/118-hr5860 --type cr_substitution
```

```text
┌───┬───────────┬──────────────────────────────────────────┬───────────────┬───────────────┬──────────────┬──────────┬─────┐
│ $ ┆ Bill      ┆ Account                                  ┆       New ($) ┆       Old ($) ┆    Delta ($) ┆ Section  ┆ Div │
╞═══╪═══════════╪══════════════════════════════════════════╪═══════════════╪═══════════════╪══════════════╪══════════╪═════╡
│ ✓ ┆ H.R. 5860 ┆ Rural Housing Service—Rural Community…   ┆    25,300,000 ┆    75,300,000 ┆  -50,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Rural Utilities Service—Rural Water a…   ┆    60,000,000 ┆   325,000,000 ┆ -265,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ National Science Foundation—STEM Educ…   ┆    92,000,000 ┆   217,000,000 ┆ -125,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ National Science Foundation—Research …   ┆   608,162,000 ┆   818,162,000 ┆ -210,000,000 ┆ SEC. 101 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Office of Personnel Management—Salari…   ┆   219,076,000 ┆   190,784,000 ┆  +28,292,000 ┆ SEC. 126 ┆ A   │
│ ✓ ┆ H.R. 5860 ┆ Department of Transportation—Federal …   ┆   617,000,000 ┆   570,000,000 ┆  +47,000,000 ┆ SEC. 137 ┆ A   │
│ ...                                                                                                                      │
└───┴───────────┴──────────────────────────────────────────┴───────────────┴───────────────┴──────────────┴──────────┴─────┘
13 provisions found
```

The `cr_substitution` table shows **New** (the CR level), **Old** (the prior-year rate being replaced), and **Delta** (the difference). Negative delta = funding cut below the prior-year rate. The full dataset contains 123 CR substitutions across all bills.

To see all CR substitutions: `congress-approp search --dir data --type cr_substitution`

---

## Working with the Data Programmatically

### Loading extraction.json in Python

Each bill's provisions are in `data/{bill_dir}/extraction.json`:

```python
import json
from collections import Counter

ext = json.load(open('data/119-hr7148/extraction.json'))
provisions = ext['provisions']

# Count by type
type_counts = Counter(p['provision_type'] for p in provisions)
for ptype, count in type_counts.most_common():
    print(f"  {ptype}: {count}")
```

```text
  appropriation: 1201
  limitation: 553
  rider: 325
  directive: 285
  transfer_authority: 107
  rescission: 98
  mandatory_spending_extension: 82
  other: 63
  directed_spending: 59
  continuing_resolution_baseline: 1
```

**Field access patterns:**

```python
p = provisions[0]
p['provision_type']       # → 'appropriation'
p['account_name']         # → 'Military Personnel, Army'
p['agency']               # → 'Department of Defense'

# Dollar amount (defensive — some fields can be null)
amt = p.get('amount') or {}
value = (amt.get('value') or {}).get('dollars', 0) or 0
# → 54538366000

amt['semantics']          # → 'new_budget_authority'
#   'new_budget_authority' — counts toward budget totals
#   'rescission'           — cancellation of prior funds
#   'transfer_ceiling'     — max transfer amount (not new spending)
#   'limitation'           — spending cap
#   'reference_amount'     — sub-allocation or contextual (not counted)
#   'mandatory_spending'   — mandatory program in the appropriation text

p['detail_level']         # → 'top_level'
#   'top_level'       — main account appropriation (counts toward totals)
#   'line_item'       — numbered item within a section (counts)
#   'sub_allocation'  — "of which" breakdown (does NOT count)
#   'proviso_amount'  — amount in a "Provided, That" clause (does NOT count)

p['raw_text'][:80]        # → verbatim bill language
p['confidence']           # → 0.97 (LLM self-assessed; not calibrated above 0.90)
p['section']              # → '' (empty if no section number)
p['division']             # → 'A'

# Source span — exact byte position in the enrolled bill
span = p.get('source_span') or {}
span['start']             # → UTF-8 byte offset in the source text file
span['end']               # → exclusive end byte
span['file']              # → 'BILLS-119hr7148enr.txt'
span['verified']          # → True (source_bytes[start:end] == raw_text)
```

**Filtering to top-level budget authority provisions** (the ones counted in totals):

```python
for p in provisions:
    if p.get('provision_type') != 'appropriation':
        continue
    amt = p.get('amount') or {}
    if amt.get('semantics') != 'new_budget_authority':
        continue
    dl = p.get('detail_level', '')
    if dl in ('sub_allocation', 'proviso_amount'):
        continue
    dollars = (amt.get('value') or {}).get('dollars', 0) or 0
    print(f"{p['account_name'][:50]:50s}  ${dollars:>15,}")
```

---

### Building a pandas DataFrame from authorities.json

`data/authorities.json` contains the cross-bill account registry — 1,051 accounts with provisions, name variants, and rename events. To flatten it into a DataFrame:

```python
import json
import pandas as pd

auth = json.load(open('data/authorities.json'))

rows = []
for a in auth['authorities']:
    for prov in a.get('provisions', []):
        for fy in prov.get('fiscal_years', []):
            rows.append({
                'fas_code': a['fas_code'],
                'agency_code': a['agency_code'],
                'agency': a['agency_name'],
                'title': a['fas_title'],
                'fiscal_year': fy,
                'dollars': prov.get('dollars', 0) or 0,
                'bill': prov['bill_identifier'],
                'bill_dir': prov['bill_dir'],
                'confidence': prov['confidence'],
                'method': prov['method'],
            })

df = pd.DataFrame(rows)
```

**Key fields:**

| Column | Meaning |
|---|---|
| `fas_code` | Federal Account Symbol — primary key. Format: `{agency_code}-{main_account}` (e.g., `070-0400`). Assigned by Treasury, stable across renames. |
| `agency_code` | CGAC agency code. `021` = Army, `017` = Navy, `057` = Air Force, `097` = DOD-wide, `070` = DHS, `075` = HHS, `036` = VA. |
| `confidence` | TAS resolution confidence. `verified` = deterministic match. `high` = LLM-resolved, confirmed in FAST Book. `inferred` = LLM-resolved, not directly confirmed. |
| `method` | Resolution method. `direct_match`, `suffix_match`, `agency_disambiguated` = deterministic. `llm_resolved` = Claude Opus. |

**Common operations:**

```python
# Budget authority by fiscal year
df.groupby('fiscal_year')['dollars'].sum().sort_index()

# Top 10 agencies
df.groupby('agency')['dollars'].sum().sort_values(ascending=False).head(10)

# Pivot: one row per account, one column per FY
df.pivot_table(values='dollars', index=['fas_code', 'title'],
               columns='fiscal_year', aggfunc='sum', fill_value=0)

# Export
df.to_csv('budget_timeline.csv', index=False)
```

---

### CLI CSV export and analysis

Export provisions from the CLI, then load in Python or a spreadsheet:

```bash
congress-approp search --dir data --type appropriation --fy 2026 --format csv > fy2026_approps.csv
```

```python
import pandas as pd

df = pd.read_csv('fy2026_approps.csv')
```

**CSV field reference:**

| Field | Meaning |
|---|---|
| `bill` | Bill identifier with congress (e.g., `H.R. 7148 (119th)`) |
| `congress` | Congress number (116–119) |
| `provision_type` | One of the 11 provision types |
| `account_name` | Account name from the bill text |
| `agency` | Department or agency |
| `dollars` | Dollar amount as plain integer |
| `old_dollars` | For `cr_substitution` only: the replaced amount |
| `semantics` | What the amount means (see field guide above) |
| `detail_level` | `top_level`, `line_item`, `sub_allocation`, or `proviso_amount` |
| `amount_status` | `found` (unique), `found_multiple`, `not_found`, or empty |
| `quality` | `strong`, `moderate`, or `weak` |
| `match_tier` | `exact`, `normalized`, or `no_match` |
| `raw_text` | Verbatim bill language (~150 chars) |
| `provision_index` | Zero-based position in the bill's provisions array |

> **Do not sum the `dollars` column directly.** Filter to `semantics == 'new_budget_authority'` and exclude `detail_level` in `('sub_allocation', 'proviso_amount')` to avoid double-counting. Or use `congress-approp summary` which handles this automatically.

```python
ba = df[(df['semantics'] == 'new_budget_authority') &
        (~df['detail_level'].isin(['sub_allocation', 'proviso_amount']))]
print(f"FY2026 BA provisions: {len(ba)}")
print(f"Total: ${ba['dollars'].sum():,.0f}")
```

Other export formats: `--format json` (array), `--format jsonl` (one object per line for streaming), `--format csv`.

**jq one-liners:**

```bash
# Top 5 rescissions by dollar amount
congress-approp search --dir data --type rescission --format json | \
  jq 'sort_by(-.dollars) | .[0:5] | .[] | {bill, account_name, dollars}'

# Count provisions by type for FY2026
congress-approp search --dir data --fy 2026 --format json | \
  jq 'group_by(.provision_type) | map({type: .[0].provision_type, count: length}) | sort_by(-.count)'
```

---

### Source span verification

Every provision carries a `source_span` with exact byte offsets into the enrolled bill text. To independently verify a provision:

```python
import json

ext = json.load(open('data/118-hr9468/extraction.json'))
p = ext['provisions'][0]
span = p['source_span']

source_bytes = open(f"data/118-hr9468/{span['file']}", 'rb').read()
actual = source_bytes[span['start']:span['end']].decode('utf-8')

assert actual == p['raw_text']  # True
```

```text
Account:  Compensation and Pensions
Dollars:  $2,285,513,000
Span:     bytes 371..482 in BILLS-118hr9468enr.txt
Match:    True
```

`start` and `end` are **UTF-8 byte offsets**. In Python, use `open(path, 'rb').read()[start:end].decode('utf-8')` — not character-based indexing.

| Field | Meaning |
|---|---|
| `start` | Start byte offset (inclusive) |
| `end` | End byte offset (exclusive) — standard Python slice semantics |
| `file` | Source filename (e.g., `BILLS-118hr9468enr.txt`) |
| `verified` | `true` if `source_bytes[start:end]` is byte-identical to `raw_text` |
| `match_tier` | `exact`, `repaired_prefix`, `repaired_substring`, or `repaired_normalized` |

To verify all provisions across multiple bills:

```python
import json, os

for bill_dir in ['118-hr9468', '119-hr7148', '119-hr5371']:
    ext = json.load(open(f'data/{bill_dir}/extraction.json'))
    for i, p in enumerate(ext['provisions']):
        span = p.get('source_span') or {}
        if not span.get('file'):
            continue
        source = open(f'data/{bill_dir}/{span["file"]}', 'rb').read()
        actual = source[span['start']:span['end']].decode('utf-8')
        assert actual == p['raw_text'], f'{bill_dir} provision {i}: MISMATCH'
    print(f'{bill_dir}: {len(ext["provisions"])} provisions verified')
```

---

## Visualizations

Generated by `book/cookbook/cookbook.py`. The images below are included in the repository; run the script to regenerate from the current data.

### FY2026 Interactive Treemap

FY2026 budget authority ($5.6 trillion across 1,076 accounts) organized by jurisdiction → agency → account. The file is a self-contained HTML page — [open it in your browser](cookbook-assets/fy2026_treemap.html).

Hierarchy: jurisdiction (subcommittee) → agency (department) → account. Click to zoom. Color intensity encodes dollar amount.

### Defense vs. Non-Defense Spending Trend

![Defense vs. Non-Defense Spending FY2019–FY2026](cookbook-assets/defense_vs_nondefense.png)

Dark blue = Defense. Light blue = all other subcommittees. Defense grew from $693B to $836B (+21%) over this period. Non-defense growth is primarily driven by mandatory spending programs (Medicaid, SNAP, VA Compensation) that appear as appropriation lines in the bill text. See [Why the Numbers Might Not Match Headlines](../explanation/numbers-vs-headlines.md).

### Top 6 Federal Accounts by Budget Authority

![Top 6 Account Spending Trends](cookbook-assets/spending_trends_top6.png)

Each line is one Treasury Account Symbol (FAS code). The top accounts are dominated by mandatory programs that appear as appropriation line items: Medicaid, Health Care Trust Funds, and VA Compensation & Pensions.

> **Note on FY2025→FY2026 jumps:** Some accounts show sharp increases between FY2025 and FY2026 (e.g., Medicaid $261B → $1,086B). This is because FY2025 was covered by a single full-year CR while FY2026 has multiple omnibus/minibus bills — the amounts are correct per bill, but the visual jump reflects different legislative coverage.

### Verification Quality Heatmap

![Verification Quality Heatmap](cookbook-assets/verification_heatmap.png)

Each row is a bill; each column is a verification metric. Color intensity shows the percentage of provisions meeting that criterion.

| Column | What it measures | Dataset result |
|---|---|---|
| **$ Verified** | Dollar string at unique position in source | 10,468 (56.3% of provisions with amounts) |
| **$ Ambiguous** | Dollar string at multiple positions — correct but location uncertain | 8,115 |
| **$ Not Found** | Dollar string not in source | 1 (0.005%) |
| **Text Exact** | `raw_text` byte-identical to source | 32,691 (94.6%) |
| **Text Normalized** | Matches after whitespace/quote normalization | 1,287 (3.7%) |
| **Text No Match** | Not found at any tier | 585 (1.7%) |

Bills with low **$ Verified** percentages (e.g., CRs) are expected — most CR provisions do not carry dollar amounts.

---

## Run All Demos Yourself

`book/cookbook/cookbook.py` runs 24 demos including everything above plus TAS resolution quality per bill, account rename events, directed spending analysis, advance appropriation breakdown, and more.

### Setup

```bash
source .venv/bin/activate
pip install -r book/cookbook/requirements.txt
```

### Run

```bash
python book/cookbook/cookbook.py
```

For semantic search demos (optional):

```bash
export OPENAI_API_KEY="your-key"
python book/cookbook/cookbook.py
```

### Output

All files go to `tmp/demo_output/`:

| File | Description |
|---|---|
| `fy2026_treemap.html` | Interactive budget treemap |
| `defense_vs_nondefense.png` | Stacked bar chart |
| `spending_trends_top6.png` | Line chart — top 6 accounts |
| `verification_heatmap.png` | Verification quality heatmap |
| `authorities_flat.csv` | Full dataset as flat CSV — every provision-FY pair |
| `biggest_changes_2024_2026.csv` | Account-level changes FY2024 → FY2026 |
| `cr_substitutions.csv` | Every CR substitution across all bills |
| `rename_events.csv` | Account rename events with fiscal year boundaries |
| `subcommittee_scorecard.csv` | 12 subcommittees × 7 fiscal years |
| `fy2026_by_agency.csv` | FY2026 budget authority by agency |
| `semantic_search_demos.json` | Semantic query results |
| `dataset_summary.json` | Summary statistics |