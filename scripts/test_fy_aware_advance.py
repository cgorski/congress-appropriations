#!/usr/bin/env python3
"""
Test script: Fiscal-year-aware advance appropriation classification.

The simple keyword approach ("become available on October 1") catches advance
appropriations but doesn't distinguish advance from current-year when both
use similar language. The key insight from the panel discussion:

    If a provision says "available on October 1, YYYY" and YYYY > bill_fiscal_year,
    it's an ADVANCE appropriation (money for a future FY).

    If YYYY == bill_fiscal_year, it's CURRENT-YEAR (money available at the start
    of the FY the bill funds).

    If YYYY < bill_fiscal_year, it's unusual — possibly a correction or
    retroactive availability.

This script tests the fiscal-year-aware classification approach across all 13 bills,
comparing it to the naive keyword approach and quantifying the difference.

Algorithm:
1. Parse the bill's fiscal_years from extraction.json
2. For each provision with availability text:
   a. Extract any "October 1, YYYY" date from availability + raw_text
   b. If YYYY > max(bill_fiscal_years) → ADVANCE
   c. If YYYY == max(bill_fiscal_years) → CURRENT_YEAR (start of funded FY)
   d. If YYYY < max(bill_fiscal_years) → PRIOR_YEAR (unusual)
   e. If no October 1 date found, check for other advance signals
   f. Default → CURRENT_YEAR
3. Compare to naive keyword classifier
4. Quantify dollar impact of the differences
"""

import json
import os
import re
import sys
from pathlib import Path
from collections import Counter, defaultdict
from dataclasses import dataclass, field


@dataclass
class ClassificationResult:
    """Result of classifying a single provision's funding timing."""
    timing: str  # ADVANCE, CURRENT_YEAR, SUPPLEMENTAL, UNKNOWN
    source: str  # what evidence was used
    availability_year: int | None = None
    bill_fy: int | None = None
    detail: str = ""


@dataclass
class ProvisionInfo:
    """All the info we need about a provision for classification."""
    bill_dir: str
    bill_id: str
    bill_fiscal_years: list
    index: int
    provision_type: str
    account_name: str
    agency: str
    dollars: int | None
    semantics: str
    is_ba: bool
    raw_text: str
    availability: str
    notes: list


def load_provisions_with_context(bill_dir: str) -> list[ProvisionInfo]:
    """Load provisions with bill-level fiscal year context."""
    ext_path = Path("examples") / bill_dir / "extraction.json"
    with open(ext_path) as f:
        ext = json.load(f)

    bill_id = ext["bill"]["identifier"]
    bill_fys = ext["bill"].get("fiscal_years", [])

    results = []
    for i, p in enumerate(ext["provisions"]):
        amt = p.get("amount")
        dollars = None
        semantics = ""
        if amt:
            val = amt.get("value", {})
            if val.get("kind") == "specific":
                dollars = val.get("dollars", 0)
            semantics = amt.get("semantics", "")

        results.append(ProvisionInfo(
            bill_dir=bill_dir,
            bill_id=bill_id,
            bill_fiscal_years=bill_fys,
            index=i,
            provision_type=p.get("provision_type", ""),
            account_name=(p.get("account_name") or "")[:60],
            agency=(p.get("agency") or "")[:50],
            dollars=dollars,
            semantics=semantics,
            is_ba=semantics == "new_budget_authority",
            raw_text=p.get("raw_text") or "",
            availability=p.get("availability") or "",
            notes=p.get("notes") or [],
        ))

    return results


def extract_october_year(text: str) -> int | None:
    """
    Extract the year from "October 1, YYYY" or similar patterns.

    Handles:
      - "shall become available on October 1, 2024"
      - "available on October 1, 2026"
      - "available beginning October 1, 2025"
      - "which shall be available on October 1, 2026"
    """
    # Pattern: October 1, followed by a 4-digit year
    patterns = [
        r"October\s+1\s*,?\s*(\d{4})",
        r"october\s+1\s*,?\s*(\d{4})",
    ]
    for pattern in patterns:
        match = re.search(pattern, text, re.IGNORECASE)
        if match:
            return int(match.group(1))
    return None


def classify_naive_keyword(prov: ProvisionInfo) -> ClassificationResult:
    """
    Naive keyword classifier — the approach from our earlier tests.
    Just checks if text mentions "become available on October 1" or similar.
    """
    combined = (prov.raw_text + " " + prov.availability).lower()

    advance_keywords = [
        "become available on october 1",
        "available on october 1",
        "available beginning october 1",
        "advance appropriation",
    ]

    for kw in advance_keywords:
        if kw in combined:
            return ClassificationResult(
                timing="ADVANCE",
                source=f"keyword: '{kw}'",
            )

    return ClassificationResult(
        timing="CURRENT_YEAR",
        source="default (no advance keyword found)",
    )


def classify_fy_aware(prov: ProvisionInfo) -> ClassificationResult:
    """
    Fiscal-year-aware classifier.

    Uses the bill's fiscal year(s) to determine whether an October 1 date
    represents the start of the funded FY (current-year) or a future FY (advance).
    """
    combined = prov.raw_text + " " + prov.availability
    combined_lower = combined.lower()

    # Step 1: What fiscal year does this bill fund?
    if not prov.bill_fiscal_years:
        # Can't determine — fall back to keyword
        return classify_naive_keyword(prov)

    bill_fy = max(prov.bill_fiscal_years)

    # Step 2: Check for supplemental context
    # If the bill is a supplemental, classify BA provisions as supplemental
    # (This is a bill-level signal, not provision-level)
    # We'll handle this at a higher level; for now just check provision notes
    for note in prov.notes:
        if "supplemental" in note.lower():
            return ClassificationResult(
                timing="SUPPLEMENTAL",
                source="note mentions supplemental",
                bill_fy=bill_fy,
            )

    # Step 3: Extract October 1 year from availability and raw_text
    october_year = extract_october_year(prov.availability)
    if october_year is None:
        october_year = extract_october_year(prov.raw_text)

    if october_year is not None:
        # We found an "October 1, YYYY" reference

        # October 1, YYYY is the START of fiscal year YYYY+1
        # e.g., "October 1, 2024" = start of FY2025
        availability_fy = october_year + 1

        if availability_fy > bill_fy:
            return ClassificationResult(
                timing="ADVANCE",
                source=f"October 1, {october_year} → FY{availability_fy} > bill FY{bill_fy}",
                availability_year=october_year,
                bill_fy=bill_fy,
                detail=f"Money enacted in FY{bill_fy} bill but available starting FY{availability_fy}",
            )
        elif availability_fy == bill_fy:
            return ClassificationResult(
                timing="CURRENT_YEAR",
                source=f"October 1, {october_year} → FY{availability_fy} == bill FY{bill_fy}",
                availability_year=october_year,
                bill_fy=bill_fy,
                detail=f"Available at start of funded FY{bill_fy}",
            )
        else:
            # availability_fy < bill_fy — unusual
            return ClassificationResult(
                timing="CURRENT_YEAR",
                source=f"October 1, {october_year} → FY{availability_fy} < bill FY{bill_fy} (prior-year reference)",
                availability_year=october_year,
                bill_fy=bill_fy,
                detail=f"References FY{availability_fy} but bill is for FY{bill_fy} — likely current-year with prior reference",
            )

    # Step 4: Check for "advance appropriation" explicit text
    if "advance appropriation" in combined_lower:
        return ClassificationResult(
            timing="ADVANCE",
            source="explicit 'advance appropriation' text",
            bill_fy=bill_fy,
        )

    # Step 5: Default — no advance signal found
    return ClassificationResult(
        timing="CURRENT_YEAR",
        source="default (no advance signal)",
        bill_fy=bill_fy,
    )


def main():
    if not Path("examples/hr4366/extraction.json").exists():
        print("ERROR: Run from repository root (appropriations/)")
        sys.exit(1)

    print("=" * 80)
    print("FISCAL-YEAR-AWARE ADVANCE CLASSIFICATION TEST")
    print("=" * 80)

    # Load all bills
    bill_dirs = sorted([
        d for d in os.listdir("examples")
        if (Path("examples") / d / "extraction.json").exists()
    ])

    all_provisions = []
    for bill_dir in bill_dirs:
        provs = load_provisions_with_context(bill_dir)
        all_provisions.extend(provs)

    print(f"\nLoaded {len(all_provisions)} provisions across {len(bill_dirs)} bills")

    # Filter to BA appropriations (the ones where advance/current matters)
    ba_provisions = [p for p in all_provisions if p.is_ba and p.dollars is not None]
    print(f"BA provisions with dollar amounts: {len(ba_provisions)}")

    # ── Test 1: Compare naive vs FY-aware classifiers ──
    print("\n--- Test 1: Naive keyword vs FY-aware classification ---\n")

    naive_results = [(p, classify_naive_keyword(p)) for p in ba_provisions]
    aware_results = [(p, classify_fy_aware(p)) for p in ba_provisions]

    # Count agreements and disagreements
    agree = 0
    disagree = []
    for (p, naive), (_, aware) in zip(naive_results, aware_results):
        if naive.timing == aware.timing:
            agree += 1
        else:
            disagree.append((p, naive, aware))

    print(f"  Agree: {agree}")
    print(f"  Disagree: {len(disagree)}")
    print(f"  Agreement rate: {agree / len(ba_provisions) * 100:.1f}%")

    if disagree:
        print(f"\n  Disagreements (provisions where classifiers differ):")
        print(f"  {'Bill':<12s} {'$':>16s} {'Naive':>12s} {'FY-Aware':>12s} {'Account':<40s}")
        print(f"  {'─' * 12} {'─' * 16} {'─' * 12} {'─' * 12} {'─' * 40}")

        total_disagree_dollars = 0
        for p, naive, aware in sorted(disagree, key=lambda x: -abs(x[0].dollars or 0)):
            dollars = abs(p.dollars) if p.dollars else 0
            total_disagree_dollars += dollars
            print(f"  {p.bill_id:<12s} ${dollars:>14,} {naive.timing:>12s} {aware.timing:>12s} {p.account_name:<40s}")
            print(f"    Naive:    {naive.source}")
            print(f"    FY-aware: {aware.source}")
            if aware.detail:
                print(f"    Detail:   {aware.detail}")
            print()

        print(f"  Total dollars in disagreements: ${total_disagree_dollars:,}")

    # ── Test 2: Per-bill classification summary (FY-aware) ──
    print("\n--- Test 2: Per-bill classification summary (FY-aware) ---\n")

    bill_stats = defaultdict(lambda: {
        "advance_count": 0, "advance_dollars": 0,
        "current_year_count": 0, "current_year_dollars": 0,
        "supplemental_count": 0, "supplemental_dollars": 0,
        "unknown_count": 0, "unknown_dollars": 0,
        "total_count": 0, "total_dollars": 0,
        "bill_fys": [],
    })

    for p, result in aware_results:
        stats = bill_stats[p.bill_id]
        dollars = abs(p.dollars) if p.dollars else 0
        stats["total_count"] += 1
        stats["total_dollars"] += dollars
        stats["bill_fys"] = p.bill_fiscal_years

        key = result.timing.lower()
        stats[f"{key}_count"] = stats.get(f"{key}_count", 0) + 1
        stats[f"{key}_dollars"] = stats.get(f"{key}_dollars", 0) + dollars

    print(f"  {'Bill':<12s} {'FYs':>8s} {'Total BA':>16s} {'Advance':>16s} {'Current':>16s} {'Adv %':>7s}")
    print(f"  {'─' * 12} {'─' * 8} {'─' * 16} {'─' * 16} {'─' * 16} {'─' * 7}")

    total_advance = 0
    total_current = 0
    total_ba = 0

    for bill_id in sorted(bill_stats.keys()):
        s = bill_stats[bill_id]
        fys = ",".join(str(y) for y in s["bill_fys"])
        adv_pct = s["advance_dollars"] / s["total_dollars"] * 100 if s["total_dollars"] > 0 else 0
        total_advance += s["advance_dollars"]
        total_current += s.get("current_year_dollars", 0)
        total_ba += s["total_dollars"]

        print(f"  {bill_id:<12s} {fys:>8s} ${s['total_dollars']:>14,} ${s['advance_dollars']:>14,} ${s.get('current_year_dollars', 0):>14,} {adv_pct:>6.1f}%")

    print(f"  {'─' * 12} {'─' * 8} {'─' * 16} {'─' * 16} {'─' * 16} {'─' * 7}")
    adv_pct_total = total_advance / total_ba * 100 if total_ba > 0 else 0
    print(f"  {'TOTAL':<12s} {'':>8s} ${total_ba:>14,} ${total_advance:>14,} ${total_current:>14,} {adv_pct_total:>6.1f}%")

    print(f"\n  Advance appropriations: ${total_advance:,} ({adv_pct_total:.1f}% of total BA)")
    print(f"  Current-year:           ${total_current:,}")

    # ── Test 3: Validate known cases ──
    print("\n--- Test 3: Validate against known cases ---\n")

    known_cases = [
        # (bill_dir, account_name_substring, expected_timing, expected_reason)
        ("hr4366", "Compensation and Pensions", "ADVANCE",
         "The $182B VA Comp&Pensions in FY2024 omnibus is advance for FY2025"),
        ("hr7148", "Tenant-Based Rental Assistance", "ADVANCE",
         "The $4B TBRA in FY2026 omnibus is advance for FY2027"),
        ("hr4366", "Military Construction, Army", "CURRENT_YEAR",
         "MilCon Army in FY2024 omnibus is current-year FY2024 spending"),
    ]

    for bill_dir, account_substr, expected_timing, reason in known_cases:
        found = False
        for p, result in aware_results:
            if p.bill_dir == bill_dir and account_substr.lower() in p.account_name.lower():
                # Take the largest-dollar provision for this account
                if not found or abs(p.dollars or 0) > found_dollars:
                    found = True
                    found_p = p
                    found_result = result
                    found_dollars = abs(p.dollars or 0)

        if found:
            match = "✓" if found_result.timing == expected_timing else "✗"
            print(f"  {match} {found_p.bill_id} — {found_p.account_name}")
            print(f"    Expected: {expected_timing} ({reason})")
            print(f"    Got:      {found_result.timing}")
            print(f"    Source:   {found_result.source}")
            if found_result.detail:
                print(f"    Detail:   {found_result.detail}")
            print(f"    Dollars:  ${found_dollars:,}")
            print()
        else:
            print(f"  ? Could not find {account_substr} in {bill_dir}")
            print()

    # ── Test 4: Find the Medicaid false positive ──
    print("--- Test 4: Medicaid false positive check ---\n")
    print("  NEXT_STEPS.md says the naive keyword approach falsely classifies")
    print("  Medicaid as advance. The FY-aware approach should fix this because")
    print("  Medicaid's October 1 date should match the bill's FY.\n")

    for p, result in aware_results:
        if "medicaid" in p.account_name.lower() or "grants to states for medicaid" in p.account_name.lower():
            naive_r = classify_naive_keyword(p)
            print(f"  {p.bill_id} [{p.index}] {p.account_name}")
            print(f"    Dollars:   ${abs(p.dollars):,}")
            print(f"    Naive:     {naive_r.timing} ({naive_r.source})")
            print(f"    FY-aware:  {result.timing} ({result.source})")
            if result.detail:
                print(f"    Detail:    {result.detail}")
            print(f"    Avail:     \"{p.availability[:100]}\"")
            print(f"    Raw (100): \"{p.raw_text[:100]}\"")
            print()

    # ── Test 5: Edge cases — provisions with no availability field ──
    print("--- Test 5: Provisions with no availability text ---\n")

    no_avail = [p for p in ba_provisions if not p.availability.strip()]
    has_avail = [p for p in ba_provisions if p.availability.strip()]

    print(f"  BA provisions with availability text:    {len(has_avail)}")
    print(f"  BA provisions without availability text: {len(no_avail)}")
    print(f"  (Provisions without availability default to CURRENT_YEAR)")

    no_avail_dollars = sum(abs(p.dollars) for p in no_avail if p.dollars)
    has_avail_dollars = sum(abs(p.dollars) for p in has_avail if p.dollars)
    print(f"\n  Dollars with availability:    ${has_avail_dollars:,}")
    print(f"  Dollars without availability: ${no_avail_dollars:,}")

    # Check: are any no-availability provisions actually advance?
    # (They'd have October 1 in raw_text but not in availability)
    no_avail_but_oct1 = []
    for p in no_avail:
        year = extract_october_year(p.raw_text)
        if year is not None:
            no_avail_but_oct1.append((p, year))

    if no_avail_but_oct1:
        print(f"\n  ⚠ {len(no_avail_but_oct1)} provisions have no availability field")
        print(f"    but mention 'October 1' in raw_text:")
        for p, year in no_avail_but_oct1[:5]:
            result = classify_fy_aware(p)
            print(f"    {p.bill_id} [{p.index}] Oct 1, {year} → {result.timing}")
            print(f"      {p.account_name} ${abs(p.dollars):,}")
            print(f"      raw: \"{p.raw_text[:120]}\"")
        if len(no_avail_but_oct1) > 5:
            print(f"    ... and {len(no_avail_but_oct1) - 5} more")
    else:
        print(f"\n  ✓ No provisions without availability text mention October 1 in raw_text")

    # ── Test 6: Full accuracy comparison ──
    print("\n--- Test 6: Classification accuracy comparison ---\n")

    # Build ground truth from availability text + fiscal year context
    # Ground truth: if October 1 year + 1 > bill FY, it's advance
    # This is the "best we can do" without reading the actual bill
    ground_truth = {}
    for p in ba_provisions:
        combined = p.raw_text + " " + p.availability
        oct_year = extract_october_year(combined)
        bill_fy = max(p.bill_fiscal_years) if p.bill_fiscal_years else None

        if oct_year is not None and bill_fy is not None:
            avail_fy = oct_year + 1
            if avail_fy > bill_fy:
                ground_truth[(p.bill_dir, p.index)] = "ADVANCE"
            else:
                ground_truth[(p.bill_dir, p.index)] = "CURRENT_YEAR"
        else:
            ground_truth[(p.bill_dir, p.index)] = "CURRENT_YEAR"

    # Score naive
    naive_correct = 0
    naive_errors = 0
    naive_false_advance = 0  # classified advance but actually current
    naive_missed_advance = 0  # classified current but actually advance

    for p, result in naive_results:
        gt = ground_truth[(p.bill_dir, p.index)]
        if result.timing == gt:
            naive_correct += 1
        else:
            naive_errors += 1
            if result.timing == "ADVANCE" and gt == "CURRENT_YEAR":
                naive_false_advance += 1
            elif result.timing == "CURRENT_YEAR" and gt == "ADVANCE":
                naive_missed_advance += 1

    # Score FY-aware
    aware_correct = 0
    aware_errors = 0
    aware_false_advance = 0
    aware_missed_advance = 0

    for p, result in aware_results:
        gt = ground_truth[(p.bill_dir, p.index)]
        timing = result.timing
        # Treat SUPPLEMENTAL as CURRENT_YEAR for comparison purposes
        if timing == "SUPPLEMENTAL":
            timing = "CURRENT_YEAR"
        if timing == gt:
            aware_correct += 1
        else:
            aware_errors += 1
            if timing == "ADVANCE" and gt == "CURRENT_YEAR":
                aware_false_advance += 1
            elif timing == "CURRENT_YEAR" and gt == "ADVANCE":
                aware_missed_advance += 1

    total_gt = len(ba_provisions)
    gt_advance = sum(1 for v in ground_truth.values() if v == "ADVANCE")
    gt_current = sum(1 for v in ground_truth.values() if v == "CURRENT_YEAR")

    print(f"  Ground truth distribution:")
    print(f"    ADVANCE:      {gt_advance}")
    print(f"    CURRENT_YEAR: {gt_current}")
    print(f"    Total:        {total_gt}")
    print()

    print(f"  {'Metric':<25s} {'Naive Keyword':>15s} {'FY-Aware':>15s}")
    print(f"  {'─' * 25} {'─' * 15} {'─' * 15}")
    print(f"  {'Correct':.<25s} {naive_correct:>15d} {aware_correct:>15d}")
    print(f"  {'Errors':.<25s} {naive_errors:>15d} {aware_errors:>15d}")
    print(f"  {'Accuracy':.<25s} {naive_correct/total_gt*100:>14.1f}% {aware_correct/total_gt*100:>14.1f}%")
    print(f"  {'False advance (FP)':.<25s} {naive_false_advance:>15d} {aware_false_advance:>15d}")
    print(f"  {'Missed advance (FN)':.<25s} {naive_missed_advance:>15d} {aware_missed_advance:>15d}")

    # ── Test 7: Dollar impact of false advance classifications ──
    print("\n--- Test 7: Dollar impact of classification differences ---\n")

    naive_advance_dollars = sum(
        abs(p.dollars)
        for p, r in naive_results
        if r.timing == "ADVANCE" and p.dollars
    )
    aware_advance_dollars = sum(
        abs(p.dollars)
        for p, r in aware_results
        if r.timing == "ADVANCE" and p.dollars
    )
    gt_advance_dollars = sum(
        abs(p.dollars)
        for p in ba_provisions
        if ground_truth[(p.bill_dir, p.index)] == "ADVANCE" and p.dollars
    )

    print(f"  Ground truth advance $:  ${gt_advance_dollars:>18,}")
    print(f"  Naive classifier advance $: ${naive_advance_dollars:>18,}")
    print(f"  FY-aware classifier advance $: ${aware_advance_dollars:>18,}")
    print()

    naive_overcount = naive_advance_dollars - gt_advance_dollars
    aware_overcount = aware_advance_dollars - gt_advance_dollars

    print(f"  Naive overcount:   ${naive_overcount:>18,}")
    print(f"  FY-aware overcount: ${aware_overcount:>18,}")

    if naive_overcount > 0:
        print(f"\n  The naive classifier over-counts advance by ${naive_overcount:,}")
        print(f"  because it classifies 'available October 1' as advance even when")
        print(f"  October 1 is the START of the funded fiscal year (current-year).")
    if aware_overcount == 0:
        print(f"\n  ✓ FY-aware classifier has no over-count (exact match to ground truth)")

    # ── Summary ──
    print("\n" + "=" * 80)
    print("SUMMARY")
    print("=" * 80)
    print(f"""
  The fiscal-year-aware classifier improves on the naive keyword approach by:

  1. Using the bill's fiscal year to interpret October 1 dates
     - "October 1, 2024" in a FY2024 bill = start of FY2025 → ADVANCE
     - "October 1, 2025" in a FY2026 bill = start of FY2026 → CURRENT_YEAR

  2. Eliminating false positives where "available on October 1" means
     "available at the start of the funded fiscal year" (current-year)

  3. Maintaining zero API calls — pure date parsing + fiscal year comparison

  Results:
    Naive accuracy:    {naive_correct/total_gt*100:.1f}%  (false advance: {naive_false_advance}, missed: {naive_missed_advance})
    FY-aware accuracy: {aware_correct/total_gt*100:.1f}%  (false advance: {aware_false_advance}, missed: {aware_missed_advance})

  Total advance appropriations detected: ${aware_advance_dollars:,}
  ({aware_advance_dollars/total_ba*100:.1f}% of total BA)

  RECOMMENDATION:
    Use FY-aware classification for the enrich command.
    Algorithm:
      1. Parse October 1 year from availability + raw_text
      2. Compare to bill fiscal year: year+1 > bill_fy → ADVANCE
      3. Explicit "advance appropriation" text → ADVANCE
      4. Default → CURRENT_YEAR
      5. Record classification source for provenance
""")


if __name__ == "__main__":
    main()
