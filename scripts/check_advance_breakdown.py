#!/usr/bin/env python3
"""Check advance appropriation budget authority breakdown for H.R. 7148."""

import json


def main():
    meta = json.load(open("examples/hr7148/bill_meta.json"))
    ext = json.load(open("examples/hr7148/extraction.json"))

    # Total BA from provisions
    ba_total = 0
    for p in ext["provisions"]:
        amt = p.get("amount")
        if amt is None:
            continue
        if amt.get("semantics") != "new_budget_authority":
            continue
        if p["provision_type"] != "appropriation":
            continue
        dl = p.get("detail_level", "")
        if dl in ("sub_allocation", "proviso_amount"):
            continue
        val = amt.get("value", {})
        if val.get("kind") == "specific":
            ba_total += val.get("dollars", 0)

    # Split by timing
    timing_map = {t["provision_index"]: t for t in meta["provision_timing"]}
    advance_ba = 0
    current_ba = 0
    supplemental_ba = 0
    other_ba = 0

    for i, p in enumerate(ext["provisions"]):
        amt = p.get("amount")
        if amt is None:
            continue
        if amt.get("semantics") != "new_budget_authority":
            continue
        if p["provision_type"] != "appropriation":
            continue
        dl = p.get("detail_level", "")
        if dl in ("sub_allocation", "proviso_amount"):
            continue
        val = amt.get("value", {})
        if val.get("kind") != "specific":
            continue
        dollars = val.get("dollars", 0)

        timing_entry = timing_map.get(i, {})
        timing = timing_entry.get("timing", "unknown") if timing_entry else "unknown"

        if timing == "advance":
            advance_ba += dollars
        elif timing == "supplemental":
            supplemental_ba += dollars
        elif timing == "current_year":
            current_ba += dollars
        else:
            other_ba += dollars

    print("H.R. 7148 Budget Authority Breakdown:")
    print(f"  Total BA:      ${ba_total:>18,}")
    if ba_total > 0:
        print(
            f"  Current-year:  ${current_ba:>18,}  ({current_ba / ba_total * 100:.1f}%)"
        )
        print(
            f"  Advance:       ${advance_ba:>18,}  ({advance_ba / ba_total * 100:.1f}%)"
        )
        print(
            f"  Supplemental:  ${supplemental_ba:>18,}  ({supplemental_ba / ba_total * 100:.1f}%)"
        )
        if other_ba:
            print(
                f"  Other/Unknown: ${other_ba:>18,}  ({other_ba / ba_total * 100:.1f}%)"
            )


if __name__ == "__main__":
    main()
