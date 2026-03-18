#!/usr/bin/env python3
"""
Test script: Quantify the "attribution gap" in verification.

The verification system checks that every dollar string in extraction.json
EXISTS somewhere in the source XML. But existence != attribution. When a
dollar string like "$5,000,000" appears 46 times in the source text, the
verification passes ("found_multiple" / "ambiguous") but we have no proof
the LLM attributed it to the correct program.

This script quantifies:
1. How many provisions have ambiguous (multi-position) dollar amounts?
2. What's the total dollar exposure in ambiguous provisions?
3. What's the distribution of ambiguity (how many positions per amount)?
4. Which dollar strings are most ambiguous (appear most often)?
5. How does this interact with the "quality" scoring system?
6. What percentage of TOTAL budget authority is in ambiguous provisions?

This directly addresses the question: "Under what conditions does this
system produce a confident wrong answer that a reasonable user would act on?"

The answer: when a common dollar amount (e.g., $5,000,000) is attributed
to a specific program, verification says "found" (it IS in the source),
but we can't prove it's the RIGHT $5,000,000 out of 46 occurrences.
"""

import json
import os
import sys
from pathlib import Path
from collections import Counter, defaultdict


def load_extraction(bill_dir: str) -> dict:
    path = Path("examples") / bill_dir / "extraction.json"
    with open(path) as f:
        return json.load(f)


def load_verification(bill_dir: str) -> dict | None:
    path = Path("examples") / bill_dir / "verification.json"
    if not path.exists():
        return None
    with open(path) as f:
        return json.load(f)


def get_amount_dollars(provision: dict) -> int | None:
    """Extract the dollar value from a provision, if it has one."""
    amt = provision.get("amount")
    if not amt:
        return None
    val = amt.get("value", {})
    if val.get("kind") == "specific":
        return val.get("dollars", 0)
    return None


def get_text_as_written(provision: dict) -> str | None:
    """Extract the text_as_written dollar string from a provision."""
    amt = provision.get("amount")
    if not amt:
        return None
    return amt.get("text_as_written")


def compute_quality(amount_status: str | None, match_tier: str | None) -> str:
    """
    Replicate the quality scoring from query.rs.

    Amount uniqueness:
      found (1 position) = 3
      found_multiple = 2
      not_found = 0
      N/A = skip

    Text match:
      exact = 3
      normalized = 2
      spaceless = 1
      no_match = 0

    Combined 5-6 = strong, 3-4 = moderate, 1-2 = weak, 0 = unverifiable
    """
    if amount_status is None and match_tier is None:
        return "n/a"

    amount_score = 0
    if amount_status == "verified":
        amount_score = 3
    elif amount_status == "ambiguous":
        amount_score = 2
    elif amount_status == "not_found":
        amount_score = 0
    elif amount_status is None:
        # No dollar amount on this provision
        return "n/a"

    text_score = 0
    if match_tier == "exact":
        text_score = 3
    elif match_tier == "normalized":
        text_score = 2
    elif match_tier == "spaceless":
        text_score = 1
    elif match_tier == "no_match":
        text_score = 0

    combined = amount_score + text_score
    if combined >= 5:
        return "strong"
    elif combined >= 3:
        return "moderate"
    elif combined >= 1:
        return "weak"
    else:
        return "unverifiable"


def main():
    if not Path("examples/hr4366/extraction.json").exists():
        print("ERROR: Run from repository root (appropriations/)")
        sys.exit(1)

    print("=" * 80)
    print("ATTRIBUTION GAP ANALYSIS")
    print("=" * 80)
    print()
    print("Question: Under what conditions does this system produce a confident")
    print("wrong answer that a reasonable user would act on?")
    print()

    # Discover all bills
    bill_dirs = sorted([
        d for d in os.listdir("examples")
        if (Path("examples") / d / "extraction.json").exists()
    ])

    # ── Analysis 1: Per-bill verification status breakdown ──
    print("--- Analysis 1: Verification status breakdown per bill ---")
    print()
    print(f"  {'Bill':<12s} {'Provs':>6s} {'Verified':>9s} {'Ambig':>6s} "
          f"{'NotFound':>9s} {'NoDollar':>9s} {'Ambig$':>16s} {'AmbigBA':>16s}")
    print(f"  {'─' * 12} {'─' * 6} {'─' * 9} {'─' * 6} {'─' * 9} {'─' * 9} {'─' * 16} {'─' * 16}")

    total_provisions = 0
    total_verified = 0
    total_ambiguous = 0
    total_not_found = 0
    total_no_dollar = 0
    total_ambiguous_dollars = 0
    total_ambiguous_ba = 0
    total_budget_authority = 0

    all_ambiguous_provisions = []  # (bill, index, dollars, text_as_written, positions, account_name, semantics)

    for bill_dir in bill_dirs:
        ext = load_extraction(bill_dir)
        ver = load_verification(bill_dir)
        provisions = ext["provisions"]
        bill_id = ext["bill"]["identifier"]

        # Build verification lookup
        amount_checks = {}
        raw_text_checks = {}
        if ver:
            for check in ver.get("amount_checks", []):
                idx = check["provision_index"]
                amount_checks[idx] = check
            for check in ver.get("raw_text_checks", []):
                idx = check["provision_index"]
                raw_text_checks[idx] = check

        bill_verified = 0
        bill_ambiguous = 0
        bill_not_found = 0
        bill_no_dollar = 0
        bill_ambiguous_dollars = 0
        bill_ambiguous_ba = 0
        bill_ba = 0

        for i, p in enumerate(provisions):
            dollars = get_amount_dollars(p)
            semantics = (p.get("amount") or {}).get("semantics", "")
            is_ba = semantics == "new_budget_authority"
            text_written = get_text_as_written(p)

            if is_ba and dollars is not None:
                bill_ba += abs(dollars)

            if dollars is None:
                bill_no_dollar += 1
                continue

            check = amount_checks.get(i)
            if check is None:
                # No verification data for this provision
                bill_no_dollar += 1
                continue

            status = check.get("status", "")
            positions = check.get("source_positions", [])

            if status == "verified":
                bill_verified += 1
            elif status in ("ambiguous", "found_multiple"):
                bill_ambiguous += 1
                bill_ambiguous_dollars += abs(dollars)
                if is_ba:
                    bill_ambiguous_ba += abs(dollars)
                all_ambiguous_provisions.append({
                    "bill": bill_id,
                    "bill_dir": bill_dir,
                    "index": i,
                    "dollars": dollars,
                    "text_as_written": text_written,
                    "positions": len(positions),
                    "account_name": (p.get("account_name") or "")[:50],
                    "semantics": semantics,
                    "provision_type": p.get("provision_type", ""),
                    "raw_text_tier": raw_text_checks.get(i, {}).get("match_tier", "unknown"),
                })
            elif status == "not_found":
                bill_not_found += 1
            else:
                bill_no_dollar += 1

        total_provisions += len(provisions)
        total_verified += bill_verified
        total_ambiguous += bill_ambiguous
        total_not_found += bill_not_found
        total_no_dollar += bill_no_dollar
        total_ambiguous_dollars += bill_ambiguous_dollars
        total_ambiguous_ba += bill_ambiguous_ba
        total_budget_authority += bill_ba

        print(
            f"  {bill_id:<12s} {len(provisions):>6d} {bill_verified:>9d} {bill_ambiguous:>6d} "
            f"{bill_not_found:>9d} {bill_no_dollar:>9d} "
            f"${bill_ambiguous_dollars:>14,} ${bill_ambiguous_ba:>14,}"
        )

    print(f"  {'─' * 12} {'─' * 6} {'─' * 9} {'─' * 6} {'─' * 9} {'─' * 9} {'─' * 16} {'─' * 16}")
    print(
        f"  {'TOTAL':<12s} {total_provisions:>6d} {total_verified:>9d} {total_ambiguous:>6d} "
        f"{total_not_found:>9d} {total_no_dollar:>9d} "
        f"${total_ambiguous_dollars:>14,} ${total_ambiguous_ba:>14,}"
    )

    print()
    print("  Legend:")
    print("    Verified  = dollar string found at exactly 1 position (unique attribution)")
    print("    Ambig     = dollar string found at multiple positions (attribution uncertain)")
    print("    NotFound  = dollar string not found in source text")
    print("    NoDollar  = provision has no dollar amount (riders, directives, etc.)")
    print("    Ambig$    = total dollars in ambiguous provisions (all semantics)")
    print("    AmbigBA   = total dollars in ambiguous provisions with new_budget_authority")

    # ── Analysis 2: Ambiguous provisions as % of budget authority ──
    print()
    print("--- Analysis 2: Attribution risk as % of budget authority ---")
    print()

    if total_budget_authority > 0:
        pct = total_ambiguous_ba / total_budget_authority * 100
        print(f"  Total budget authority across all bills: ${total_budget_authority:,}")
        print(f"  Budget authority in ambiguous provisions: ${total_ambiguous_ba:,}")
        print(f"  Attribution risk: {pct:.1f}% of total BA has ambiguous source attribution")
    else:
        print("  No budget authority data available")

    if total_verified + total_ambiguous > 0:
        pct_provs = total_ambiguous / (total_verified + total_ambiguous) * 100
        print(f"  Provisions with dollar amounts: {total_verified + total_ambiguous}")
        print(f"  Of those, ambiguous: {total_ambiguous} ({pct_provs:.1f}%)")

    # ── Analysis 3: Distribution of ambiguity (positions per amount) ──
    print()
    print("--- Analysis 3: How ambiguous? (positions per dollar string) ---")
    print()

    position_counts = Counter()
    for ap in all_ambiguous_provisions:
        position_counts[ap["positions"]] += 1

    if position_counts:
        print(f"  {'Positions':>10s} {'Provisions':>11s} {'Example'}")
        print(f"  {'─' * 10} {'─' * 11} {'─' * 50}")
        for n_pos in sorted(position_counts.keys()):
            count = position_counts[n_pos]
            # Find an example
            example = next(
                (ap for ap in all_ambiguous_provisions if ap["positions"] == n_pos),
                None,
            )
            example_str = ""
            if example:
                example_str = (
                    f"{example['text_as_written'] or '?':>15s} "
                    f"({example['bill']} — {example['account_name'][:30]})"
                )
            print(f"  {n_pos:>10d} {count:>11d} {example_str}")
    else:
        print("  No ambiguous provisions found!")

    # ── Analysis 4: Most commonly ambiguous dollar strings ──
    print()
    print("--- Analysis 4: Most ambiguous dollar strings ---")
    print()

    dollar_string_counts = defaultdict(list)
    for ap in all_ambiguous_provisions:
        taw = ap["text_as_written"] or "?"
        dollar_string_counts[taw].append(ap)

    # Sort by number of occurrences as ambiguous
    sorted_strings = sorted(
        dollar_string_counts.items(),
        key=lambda x: -len(x[1]),
    )

    print(f"  {'Dollar String':>18s} {'Times Ambig':>12s} {'Max Positions':>14s} {'Total $':>16s}")
    print(f"  {'─' * 18} {'─' * 12} {'─' * 14} {'─' * 16}")
    for taw, provisions in sorted_strings[:20]:
        times = len(provisions)
        max_pos = max(p["positions"] for p in provisions)
        total_dollars = sum(abs(p["dollars"]) for p in provisions)
        print(f"  {taw:>18s} {times:>12d} {max_pos:>14d} ${total_dollars:>14,}")

    if len(sorted_strings) > 20:
        print(f"  ... and {len(sorted_strings) - 20} more unique dollar strings")

    # ── Analysis 5: Quality score distribution for ambiguous provisions ──
    print()
    print("--- Analysis 5: Quality scores for ambiguous provisions ---")
    print()

    quality_counts = Counter()
    quality_dollars = defaultdict(int)

    for ap in all_ambiguous_provisions:
        q = compute_quality("ambiguous", ap["raw_text_tier"])
        quality_counts[q] += 1
        quality_dollars[q] += abs(ap["dollars"])

    print(f"  {'Quality':>12s} {'Count':>7s} {'Dollars':>16s} {'Interpretation'}")
    print(f"  {'─' * 12} {'─' * 7} {'─' * 16} {'─' * 50}")
    for q in ["strong", "moderate", "weak", "unverifiable", "n/a"]:
        if quality_counts[q] > 0:
            interp = {
                "strong": "Should not exist — ambiguous amount can't be 'strong'",
                "moderate": "Ambiguous amount + exact/normalized text match",
                "weak": "Ambiguous amount + poor text match",
                "unverifiable": "Amount not found (shouldn't be here)",
                "n/a": "No dollar amount (shouldn't be here)",
            }.get(q, "")
            print(f"  {q:>12s} {quality_counts[q]:>7d} ${quality_dollars[q]:>14,} {interp}")

    # ── Analysis 6: Largest individual ambiguous provisions ──
    print()
    print("--- Analysis 6: Largest ambiguous provisions (highest $ at risk) ---")
    print()

    sorted_by_dollars = sorted(
        all_ambiguous_provisions,
        key=lambda x: -abs(x["dollars"]),
    )

    print(f"  {'Bill':<12s} {'$':>16s} {'Positions':>10s} {'Semantics':>20s} {'Account'}")
    print(f"  {'─' * 12} {'─' * 16} {'─' * 10} {'─' * 20} {'─' * 40}")
    for ap in sorted_by_dollars[:25]:
        print(
            f"  {ap['bill']:<12s} ${abs(ap['dollars']):>14,} {ap['positions']:>10d} "
            f"{ap['semantics']:>20s} {ap['account_name']}"
        )

    if len(sorted_by_dollars) > 25:
        print(f"  ... and {len(sorted_by_dollars) - 25} more")

    # ── Analysis 7: The "confident wrong answer" scenario ──
    print()
    print("--- Analysis 7: The 'confident wrong answer' scenario ---")
    print()

    # Find provisions where:
    # - The dollar amount is ambiguous (found multiple times in source)
    # - The raw text matches exactly (so the quality score is "moderate")
    # - The dollar amount is large (>$100M)
    # - The semantics are new_budget_authority (so it affects the total)
    # This is the scenario where a user sees a ✓ checkmark, a moderate/strong
    # quality score, and a large dollar amount — but the attribution could be wrong.

    confident_wrong_candidates = [
        ap for ap in all_ambiguous_provisions
        if ap["raw_text_tier"] == "exact"
        and abs(ap["dollars"]) >= 100_000_000
        and ap["semantics"] == "new_budget_authority"
    ]

    if confident_wrong_candidates:
        print(f"  Provisions that could produce a 'confident wrong answer':")
        print(f"  (Ambiguous amount + exact text match + >$100M + budget authority)")
        print()
        print(f"  Found: {len(confident_wrong_candidates)} provisions")
        print(f"  Total exposure: ${sum(abs(c['dollars']) for c in confident_wrong_candidates):,}")
        print()

        for c in sorted(confident_wrong_candidates, key=lambda x: -abs(x["dollars"]))[:15]:
            print(
                f"    {c['bill']:<12s} ${abs(c['dollars']):>14,} "
                f"({c['positions']} positions in source) "
                f"{c['account_name']}"
            )
        if len(confident_wrong_candidates) > 15:
            print(f"    ... and {len(confident_wrong_candidates) - 15} more")
    else:
        print("  No provisions match the 'confident wrong answer' criteria.")
        print("  (Ambiguous amount + exact text match + >$100M + budget authority)")

    # ── Analysis 8: What does the ✓ checkmark actually mean? ──
    print()
    print("--- Analysis 8: What the ✓ checkmark communicates vs. guarantees ---")
    print()
    print("  The search output shows a ✓ for provisions where the dollar amount")
    print("  was found in the source text. Users interpret this as 'verified correct.'")
    print()
    print("  What ✓ GUARANTEES:")
    print("    - The dollar string exists somewhere in the enrolled bill XML")
    print("    - The LLM did not hallucinate a dollar amount from nothing")
    print()
    print("  What ✓ does NOT guarantee:")
    print("    - That the amount is attributed to the correct program/account")
    print("    - That the amount represents new budget authority (vs. a reference)")
    print("    - That the amount hasn't been double-counted across provisions")
    print()

    if total_verified + total_ambiguous > 0:
        verified_pct = total_verified / (total_verified + total_ambiguous) * 100
        ambiguous_pct = total_ambiguous / (total_verified + total_ambiguous) * 100
        print(f"  In the current dataset:")
        print(f"    {total_verified} provisions ({verified_pct:.1f}%) have UNIQUE attribution")
        print(f"      (dollar string found at exactly 1 position — strong evidence of correct attribution)")
        print(f"    {total_ambiguous} provisions ({ambiguous_pct:.1f}%) have AMBIGUOUS attribution")
        print(f"      (dollar string found at multiple positions — could be attributed to wrong program)")

    if total_budget_authority > 0:
        # Compute uniquely-attributed BA
        unique_ba = total_budget_authority - total_ambiguous_ba
        unique_pct = unique_ba / total_budget_authority * 100
        ambig_pct = total_ambiguous_ba / total_budget_authority * 100
        print()
        print(f"  Budget authority attribution:")
        print(f"    ${unique_ba:,} ({unique_pct:.1f}%) has unique attribution")
        print(f"    ${total_ambiguous_ba:,} ({ambig_pct:.1f}%) has ambiguous attribution")

    # ── Summary ──
    print()
    print("=" * 80)
    print("CONCLUSIONS")
    print("=" * 80)
    print()
    print("  The verification system checks EXISTENCE, not ATTRIBUTION.")
    print()
    print("  For the majority of provisions, existence implies attribution because")
    print("  the dollar string is unique in the source text. But for provisions with")
    print("  common dollar amounts ($5M, $10M, $25M, etc.), the same string appears")
    print("  many times and the ✓ checkmark overstates our confidence.")
    print()
    print("  The v4.0 'attribution confidence' scoring system addresses this by")
    print("  combining amount uniqueness with raw text match quality. But the")
    print("  current UI (✓ checkmark) does not distinguish unique from ambiguous.")
    print()
    print("  RECOMMENDATION:")
    print("    1. Change ✓ to show ✓ (unique) vs ~ (ambiguous) vs ✗ (not found)")
    print("    2. Implement attribution_confidence scoring in v4.0")
    print("    3. Document that 'verified' means 'exists in source' not 'correctly attributed'")
    print("    4. For high-stakes use (journalism, official reports), recommend")
    print("       filtering to HIGH confidence provisions only")


if __name__ == "__main__":
    main()
