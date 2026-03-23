# The Authority System

The authority system solves a fundamental problem in federal budget analysis:
the same budget account can appear under different names, different agencies,
and different bill structures across fiscal years. Without a stable identity
for each account, tracking spending over time requires manual reconciliation
of thousands of name variants.

## The Problem

Consider the Secret Service's main operating account:

| Fiscal Year | Bill | Name Used |
|-------------|------|-----------|
| FY2020 | H.R. 1158 | United States Secret Service—Operations and Support |
| FY2021 | H.R. 133 | United States Secret Service—Operations and Support |
| FY2022 | H.R. 2471 | Operations and Support |
| FY2023 | H.R. 2617 | Operations and Support |
| FY2024 | H.R. 2882 | Operations and Support |

These are all the same account. But the LLM extraction faithfully reproduces
whatever name the bill text uses, which varies across congresses. A string-based
comparison would treat "United States Secret Service—Operations and Support"
and "Operations and Support" as different accounts.

The problem is worse for generic names. 151 different agencies have an account
called "Salaries and Expenses." Without knowing which agency a provision belongs
to, the name alone is meaningless.

## The Solution: Federal Account Symbols

The U.S. Treasury assigns every budget account a **Federal Account Symbol (FAS)**
— a code in the format `{agency_code}-{main_account}` that persists for the
life of the account regardless of what Congress calls it in bill text.

The Secret Service example resolves cleanly:

| FY | Name in Bill | FAS Code |
|----|-------------|----------|
| FY2020 | United States Secret Service—Operations and Support | 070-0400 |
| FY2022 | Operations and Support | 070-0400 |
| FY2024 | Operations and Support | 070-0400 |

Same code, every year. The code `070` identifies the Department of Homeland
Security and `0400` identifies the Secret Service Operations account within DHS.

## How the Authority System Works

The authority system has three layers:

### Layer 1: The FAST Book Reference

The tool ships with `fas_reference.json`, derived from the Federal Account
Symbols and Titles (FAST) Book published by the Bureau of the Fiscal Service.
This file contains 2,768 active FAS codes and 485 discontinued General Fund
accounts — the complete catalog of federal budget accounts as defined by
the Treasury.

### Layer 2: TAS Mapping (per bill)

The `resolve-tas` command maps each top-level budget authority provision to a
FAS code. It uses deterministic string matching for unambiguous names (~56%)
and Claude Opus for ambiguous cases (~44%), achieving 99.4% resolution across
the dataset. Each mapping is verified against the FAST Book reference.

The result is a `tas_mapping.json` per bill containing entries like:

```json
{
  "provision_index": 15,
  "account_name": "Operations and Support",
  "agency": "United States Secret Service",
  "fas_code": "070-0400",
  "confidence": "high",
  "method": "llm_resolved"
}
```

### Layer 3: The Authority Registry

The `authority build` command aggregates all per-bill TAS mappings into a
single `authorities.json` file. Each FAS code becomes one **authority** —
a record that collects every provision for that account across all bills
and fiscal years.

An authority record contains:

- **FAS code** — the stable identifier (e.g., `070-0400`)
- **Official title** — from the FAST Book
- **Provisions** — every instance across all bills, with bill identifier, fiscal year, dollar amount, and the account name the LLM extracted
- **Name variants** — all distinct names used for this account, classified by type
- **Events** — detected lifecycle changes (renames)

## Name Variant Classification

When the same FAS code has different account names across bills, the system
classifies each variant:

| Classification | Meaning | Example |
|---------------|---------|---------|
| `canonical` | The primary name (most frequently used) | "Salaries and Expenses" |
| `case_variant` | Differs only in capitalization | "salaries and expenses" |
| `prefix_variant` | Differs by em-dash agency prefix | "USSS—Operations and Support" vs "Operations and Support" |
| `name_change` | A genuine rename with a temporal boundary | "Allowances and Expenses" → "Members' Representational Allowances" |
| `inconsistent_extraction` | The LLM used different names without a clear pattern | Different formatting across bill editions |

The first three categories (canonical, case, prefix) account for the vast
majority of variants and are harmless — they reflect different formatting
conventions in different bills, not actual program changes.

## Authority Events

When the system detects a clear temporal boundary — one name used exclusively
before a fiscal year, another used exclusively after — it records a **rename
event**:

```
TAS 000-0438: Contingent Expenses, House of Representatives
  ⟹  FY2025: renamed from "Allowances and Expenses"
                         to "Members' Representational Allowances"
```

Across the 32-bill dataset spanning FY2019–FY2026, the system detects 40
rename events. These are cases where Congress formally changed an account's
title in the enacted bill text.

Events currently cover renames only. Future versions may detect agency moves
(e.g., Secret Service moving from Treasury to DHS in 2003), account splits,
and account merges.

## Using the Authority System

### Track an account across fiscal years

```bash
# By FAS code
congress-approp trace 070-0400 --dir data

# By name (searches across title, agency, and all name variants)
congress-approp trace "coast guard operations" --dir data
```

The timeline output shows budget authority per fiscal year, which bills
contributed, and the account names used. Continuing resolution and
supplemental bills are labeled.

### Browse the registry

```bash
# All authorities
congress-approp authority list --dir data

# Filter to one agency
congress-approp authority list --dir data --agency 070

# JSON output for programmatic use
congress-approp authority list --dir data --format json
```

### Use in comparisons

```bash
congress-approp compare --base-fy 2024 --current-fy 2026 \
    --subcommittee thud --dir data --use-authorities
```

The `--use-authorities` flag matches accounts by FAS code instead of by
name, resolving orphan pairs where the same account has different names
or agency attributions across fiscal years.

## What the FAS Code Represents

The FAS code is a two-part identifier:

```
070-0400
 │    │
 │    └── Main account code (4 digits) — the specific account
 └─────── CGAC agency code (3 digits) — the department or agency
```

Key properties:

- **Stable through renames.** When "Salaries and Expenses" became "Operations
  and Support" for DHS accounts around FY2017, the FAS code did not change.

- **Changes on reorganization.** When the Secret Service moved from Treasury
  (agency 020) to DHS (agency 070) in 2003, it received new FAS codes under
  the 070 prefix. For tracking across reorganizations, the authority system
  would need historical cross-references (not yet implemented).

- **Assigned by Treasury.** These are not invented identifiers — they are
  the government's own account numbering system, published in the FAST Book
  and used across USASpending.gov, the OMB budget database, and Treasury
  financial reports.

## Scope and Limitations

The authority system covers **discretionary appropriations** — the spending
that Congress votes on annually through the twelve appropriations bills,
plus supplementals and continuing resolutions. This is roughly 26% of total
federal spending.

It does **not** cover:
- Mandatory spending (Social Security, Medicare, Medicaid — ~63% of spending)
- Net interest on the national debt (~11% of spending)
- Trust funds, revolving funds, or other non-appropriated accounts

The dollar amounts represent **budget authority** (what Congress authorizes
agencies to obligate), not **outlays** (what the Treasury actually disburses).
Budget authority and outlays can differ significantly, especially for
multi-year accounts.

40 provisions (0.6%) across the dataset could not be resolved to a FAS code.
These are genuine edge cases: Postal Service accounts, intelligence community
programs, FDIC self-funded accounts, and newly created programs not yet in
the FAST Book. They represent less than 0.05% of total budget authority.

## Data Files

| File | Location | Content |
|------|----------|---------|
| `fas_reference.json` | `data/` | Bundled FAST Book reference (2,768 FAS codes) |
| `tas_mapping.json` | Per bill directory | FAS code per top-level appropriation provision |
| `authorities.json` | `data/` | Aggregated account registry with timelines and events |

The `authorities.json` file is rebuilt from scratch by `authority build`.
It is a derived artifact — delete it and rebuild at any time from the
per-bill `tas_mapping.json` files.