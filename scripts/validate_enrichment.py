#!/usr/bin/env python3
"""
Validate enrichment output (bill_meta.json) against known expected values.

This script checks every bill_meta.json in the examples/ directory against
manually verified ground truth: bill nature, jurisdiction mappings, advance
appropriation classifications, and canonical account normalization.

Run from the repository root:
    .venv/bin/python3 scripts/validate_enrichment.py
"""

import json
import os
import sys
from pathlib import Path
from collections import Counter, defaultdict


def load_meta(bill_dir: str) -> dict | None:
    path = Path("examples") / bill_dir / "bill_meta.json"
    if not path.exists():
        return None
    with open(path) as f:
        return json.load(f)


def load_extraction(bill_dir: str) -> dict:
    path = Path("examples") / bill_dir / "extraction.json"
    with open(path) as f:
        return json.load(f)


def main():
    if not Path("examples/hr4366/extraction.json").exists():
        print("ERROR: Run from repository root (appropriations/)")
        sys.exit(1)

    print("=" * 80)
    print("ENRICHMENT VALIDATION")
    print("=" * 80)

    errors = []
    warnings = []

    # ── Expected bill natures ──
    expected_natures = {
        "hr1968": "full_year_cr_with_appropriations",
        "hr2872": "continuing_resolution",
        "hr4366": "omnibus",
        "hr5371": "minibus",
        "hr5860": "continuing_resolution",
        "hr6363": "continuing_resolution",
        "hr6938": "minibus",
        "hr7148": "omnibus",
        "hr7463": "continuing_resolution",
        "hr815": "supplemental",
        "hr9468": "supplemental",
        "hr9747": "continuing_resolution",
        "s870": "authorization",
    }

    # ── Expected congress numbers ──
    expected_congress = {
        "hr1968": 119,
        "hr2872": 118,
        "hr4366": 118,
        "hr5371": 119,
        "hr5860": 118,
        "hr6363": 118,
        "hr6938": 119,
        "hr7148": 119,
        "hr7463": 118,
        "hr815": 118,
        "hr9468": 118,
        "hr9747": 118,
        "s870": 118,
    }

    # ── Expected jurisdiction mappings (spot checks) ──
    expected_jurisdictions = {
        "hr4366": {
            "A": "milcon_va",
            "B": "agriculture",
            "C": "cjs",
            "D": "energy_water",
            "E": "interior",
            "F": "thud",
        },
        "hr7148": {
            "A": "defense",
            "B": "labor_hhs",
            "D": "thud",
            "E": "financial_services",
            "F": "state_foreign_ops",
            "H": "continuing_resolution",
        },
        "hr6938": {
            "A": "cjs",
            "B": "energy_water",
            "C": "interior",
        },
        "hr5371": {
            "A": "continuing_resolution",
            "B": "agriculture",
            "C": "legislative_branch",
            "D": "milcon_va",
        },
    }

    # ── Expected known advance provisions (spot checks) ──
    # (bill_dir, provision_index, expected_timing, description)
    expected_advance = [
        ("hr4366", 1201, "advance", "VA Comp & Pensions $182B"),
        ("hr4366", 1215, "advance", "VA Medical Services $71B"),
        ("hr4366", 1969, "advance", "TBRA $4B advance"),
        ("hr7148", 607, "advance", "Medicaid first-quarter $316B"),
        ("hr7148", 868, "advance", "SSI first-quarter $23.5B"),
        ("hr7148", 1370, "advance", "TBRA $4B advance"),
        ("hr7148", 719, "advance", "Education for Disadvantaged $19B"),
    ]

    # ── Expected canonical account normalization (spot checks) ──
    expected_canonical = [
        ("hr4366", "Grants-In-Aid for Airports", "grants-in-aid for airports"),
        ("hr4366", "Grants-in-Aid for Airports", "grants-in-aid for airports"),
        ("hr7148", "Grants-In-Aid for Airports", "grants-in-aid for airports"),
    ]

    # ── Validate each bill ──
    bill_dirs = sorted([
        d for d in os.listdir("examples")
        if (Path("examples") / d / "extraction.json").exists()
    ])

    print(f"\nFound {len(bill_dirs)} bill directories")

    for bill_dir in bill_dirs:
        meta = load_meta(bill_dir)
        ext = load_extraction(bill_dir)
        bill_id = ext["bill"]["identifier"]

        if meta is None:
            errors.append(f"{bill_dir}: bill_meta.json not found")
            continue

        # Schema version
        if meta.get("schema_version") != "1.0":
            errors.append(f"{bill_dir}: schema_version is '{meta.get('schema_version')}', expected '1.0'")

        # Congress number
        if bill_dir in expected_congress:
            if meta.get("congress") != expected_congress[bill_dir]:
                errors.append(
                    f"{bill_dir}: congress is {meta.get('congress')}, "
                    f"expected {expected_congress[bill_dir]}"
                )

        # Bill nature
        if bill_dir in expected_natures:
            if meta.get("bill_nature") != expected_natures[bill_dir]:
                errors.append(
                    f"{bill_dir}: bill_nature is '{meta.get('bill_nature')}', "
                    f"expected '{expected_natures[bill_dir]}'"
                )

        # Fiscal years match extraction
        ext_fys = ext["bill"].get("fiscal_years", [])
        meta_fys = meta.get("fiscal_years", [])
        if ext_fys != meta_fys:
            errors.append(
                f"{bill_dir}: fiscal_years mismatch: extraction={ext_fys}, meta={meta_fys}"
            )

        # Extraction SHA256 is not empty
        if not meta.get("extraction_sha256"):
            errors.append(f"{bill_dir}: extraction_sha256 is empty")

        # Subcommittees are present for bills with divisions
        ext_divisions = ext["bill"].get("divisions", [])
        meta_divisions = [s["division"] for s in meta.get("subcommittees", [])]
        if ext_divisions and not meta_divisions:
            errors.append(f"{bill_dir}: has divisions {ext_divisions} but no subcommittee mappings")

        # Check jurisdiction mappings (spot checks)
        if bill_dir in expected_jurisdictions:
            sub_map = {s["division"]: s["jurisdiction"] for s in meta.get("subcommittees", [])}
            for div, expected_j in expected_jurisdictions[bill_dir].items():
                actual_j = sub_map.get(div)
                if actual_j != expected_j:
                    errors.append(
                        f"{bill_dir} Div {div}: jurisdiction is '{actual_j}', "
                        f"expected '{expected_j}'"
                    )

        # Check that every subcommittee mapping has a source
        for s in meta.get("subcommittees", []):
            if not s.get("source"):
                errors.append(
                    f"{bill_dir} Div {s['division']}: subcommittee mapping has no source"
                )

        # Check provision_timing entries
        timing_by_index = {t["provision_index"]: t for t in meta.get("provision_timing", [])}

        # Every timing entry should reference a valid provision index
        n_provisions = len(ext.get("provisions", []))
        for t in meta.get("provision_timing", []):
            if t["provision_index"] >= n_provisions:
                errors.append(
                    f"{bill_dir}: provision_timing index {t['provision_index']} "
                    f"out of range (bill has {n_provisions} provisions)"
                )
            if not t.get("source"):
                errors.append(
                    f"{bill_dir}: provision_timing[{t['provision_index']}] has no source"
                )
            if t["timing"] not in ("current_year", "advance", "supplemental", "unknown"):
                errors.append(
                    f"{bill_dir}: provision_timing[{t['provision_index']}] has invalid timing '{t['timing']}'"
                )

        # Check canonical_accounts entries
        for ca in meta.get("canonical_accounts", []):
            if ca["provision_index"] >= n_provisions:
                errors.append(
                    f"{bill_dir}: canonical_accounts index {ca['provision_index']} out of range"
                )
            # Canonical name should be lowercase
            if ca["canonical_name"] != ca["canonical_name"].lower():
                errors.append(
                    f"{bill_dir}: canonical_accounts[{ca['provision_index']}] "
                    f"'{ca['canonical_name']}' is not lowercase"
                )
            # Canonical name should not be empty
            if not ca["canonical_name"].strip():
                errors.append(
                    f"{bill_dir}: canonical_accounts[{ca['provision_index']}] has empty name"
                )

    # ── Validate known advance provisions ──
    print("\n--- Advance Provision Spot Checks ---\n")
    for bill_dir, prov_idx, expected_timing, description in expected_advance:
        meta = load_meta(bill_dir)
        if meta is None:
            errors.append(f"{bill_dir}: bill_meta.json not found for advance check")
            continue

        timing_map = {t["provision_index"]: t for t in meta.get("provision_timing", [])}
        entry = timing_map.get(prov_idx)

        if entry is None:
            errors.append(
                f"{bill_dir}[{prov_idx}]: not in provision_timing — {description}"
            )
            print(f"  ✗ {bill_dir}[{prov_idx}] NOT FOUND — {description}")
        elif entry["timing"] != expected_timing:
            errors.append(
                f"{bill_dir}[{prov_idx}]: timing is '{entry['timing']}', "
                f"expected '{expected_timing}' — {description}"
            )
            print(f"  ✗ {bill_dir}[{prov_idx}] {entry['timing']} != {expected_timing} — {description}")
        else:
            print(f"  ✓ {bill_dir}[{prov_idx}] {entry['timing']} — {description}")
            if entry.get("available_fy"):
                print(f"    available_fy={entry['available_fy']}, source={entry['source']['type']}")

    # ── Validate canonical account normalization ──
    print("\n--- Canonical Account Normalization Spot Checks ---\n")
    for bill_dir, original_name, expected_canonical_name in expected_canonical:
        meta = load_meta(bill_dir)
        ext = load_extraction(bill_dir)
        if meta is None:
            continue

        # Find the provision with this original account name
        found = False
        for ca in meta.get("canonical_accounts", []):
            idx = ca["provision_index"]
            if idx < len(ext["provisions"]):
                prov = ext["provisions"][idx]
                orig = prov.get("account_name", "")
                if orig == original_name:
                    if ca["canonical_name"] == expected_canonical_name:
                        print(f"  ✓ {bill_dir}: '{original_name}' → '{ca['canonical_name']}'")
                    else:
                        errors.append(
                            f"{bill_dir}: '{original_name}' normalized to '{ca['canonical_name']}', "
                            f"expected '{expected_canonical_name}'"
                        )
                        print(f"  ✗ {bill_dir}: '{original_name}' → '{ca['canonical_name']}' (expected '{expected_canonical_name}')")
                    found = True
                    break
        if not found:
            warnings.append(
                f"{bill_dir}: could not find provision with account_name='{original_name}' for canonical check"
            )

    # ── Cross-bill case-insensitive matching validation ──
    print("\n--- Cross-Bill Case-Insensitive Matching ---\n")

    # Collect all canonical names across bills
    all_canonical = defaultdict(list)  # canonical_name -> [(bill, original_name)]
    for bill_dir in bill_dirs:
        meta = load_meta(bill_dir)
        ext = load_extraction(bill_dir)
        if meta is None:
            continue

        for ca in meta.get("canonical_accounts", []):
            idx = ca["provision_index"]
            if idx < len(ext["provisions"]):
                orig = ext["provisions"][idx].get("account_name", "")
                all_canonical[ca["canonical_name"]].append((bill_dir, orig))

    # Find accounts that now match across bills via canonical names
    # but had different original casing
    cross_bill_matches = 0
    for canonical, entries in all_canonical.items():
        bills = set(bd for bd, _ in entries)
        if len(bills) >= 2:
            originals = set(orig for _, orig in entries)
            if len(originals) > 1:
                cross_bill_matches += 1

    print(f"  Accounts matching cross-bill via canonical normalization: {len([k for k, v in all_canonical.items() if len(set(bd for bd, _ in v)) >= 2])}")
    print(f"  Of those, with differing original casing: {cross_bill_matches}")

    # ── Aggregate statistics ──
    print("\n--- Aggregate Statistics ---\n")

    total_advance = 0
    total_current = 0
    total_supplemental = 0
    total_unknown = 0
    total_advance_dollars = 0
    total_current_dollars = 0

    for bill_dir in bill_dirs:
        meta = load_meta(bill_dir)
        ext = load_extraction(bill_dir)
        if meta is None:
            continue

        for t in meta.get("provision_timing", []):
            idx = t["provision_index"]
            dollars = 0
            if idx < len(ext["provisions"]):
                amt = ext["provisions"][idx].get("amount", {})
                if amt:
                    val = amt.get("value", {})
                    if val.get("kind") == "specific":
                        dollars = abs(val.get("dollars", 0))

            if t["timing"] == "advance":
                total_advance += 1
                total_advance_dollars += dollars
            elif t["timing"] == "current_year":
                total_current += 1
                total_current_dollars += dollars
            elif t["timing"] == "supplemental":
                total_supplemental += 1
            elif t["timing"] == "unknown":
                total_unknown += 1

    total_ba_provisions = total_advance + total_current + total_supplemental + total_unknown
    print(f"  Total BA provisions classified: {total_ba_provisions}")
    print(f"  Current-year: {total_current} (${total_current_dollars:,})")
    print(f"  Advance: {total_advance} (${total_advance_dollars:,})")
    print(f"  Supplemental: {total_supplemental}")
    print(f"  Unknown: {total_unknown}")
    if total_ba_provisions > 0:
        adv_pct = total_advance_dollars / (total_advance_dollars + total_current_dollars) * 100 if (total_advance_dollars + total_current_dollars) > 0 else 0
        print(f"  Advance as % of (advance + current) dollars: {adv_pct:.1f}%")

    # ── Nature distribution ──
    print("\n  Bill nature distribution:")
    nature_counts = Counter()
    for bill_dir in bill_dirs:
        meta = load_meta(bill_dir)
        if meta:
            nature_counts[meta.get("bill_nature", "?")] += 1
    for nature, count in nature_counts.most_common():
        print(f"    {nature}: {count}")

    # ── Jurisdiction coverage ──
    print("\n  Jurisdiction coverage across all bills:")
    jurisdiction_bills = defaultdict(set)
    for bill_dir in bill_dirs:
        meta = load_meta(bill_dir)
        if meta:
            for s in meta.get("subcommittees", []):
                jurisdiction_bills[s["jurisdiction"]].add(bill_dir)
    for j in sorted(jurisdiction_bills.keys()):
        bills = jurisdiction_bills[j]
        print(f"    {j:25s}: {len(bills)} bills")

    # ── Summary ──
    print("\n" + "=" * 80)
    print("RESULTS")
    print("=" * 80)

    if errors:
        print(f"\n  ✗ {len(errors)} ERRORS:")
        for e in errors:
            print(f"    - {e}")
    else:
        print(f"\n  ✓ No errors found")

    if warnings:
        print(f"\n  ⚠ {len(warnings)} WARNINGS:")
        for w in warnings:
            print(f"    - {w}")

    if not errors:
        print(f"\n  All {len(bill_dirs)} bills validated successfully.")
        print(f"  {total_ba_provisions} provision timing classifications verified.")
        print(f"  {sum(len(load_meta(d).get('canonical_accounts', [])) for d in bill_dirs if load_meta(d))} canonical account names verified.")

    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
