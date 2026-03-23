#!/usr/bin/env python3
"""
Congressional Appropriations — Demo Data & Visualizations

Produces:
  1. FY2026 budget authority by agency (top 25)
  2. Subcommittee scorecard across FY2020–FY2026
  3. Account timeline chart (top 10 accounts by total BA)
  4. Rename events timeline
  5. TAS resolution quality summary
  6. FY2026 treemap (HTML/Plotly)
  7. Defense vs non-Defense spending trend chart
  8. CR substitution impact analysis
  9. Biggest year-over-year changes by account
 10. Verification quality heatmap
 11. Semantic search showcase (requires OPENAI_API_KEY)
 12. Account trace showcase
 13. Spending trends (top 6 accounts line chart)
 14. Dataset summary card

 --- Python-from-JSON & CLI Export Demos ---
 15. Python: Load extraction.json and analyze provision types
 16. Python: Load authorities.json and build a pandas DataFrame
 17. Python: Source span verification (prove traceability mechanically)
 18. Python: Cross-bill account matching via TAS codes
 19. CLI Export: CSV → pandas round-trip
 20. CLI Export: JSON → jq recipes
 21. CLI Export: JSONL streaming pipeline
 22. Python: Build a custom "earmark finder"
 23. Python: Advance appropriation analysis from bill_meta.json
 24. CLI + Python: Compare pipeline with inflation adjustment

Usage:
    source .venv/bin/activate
    python tmp/demos.py
"""

import json
import os
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

DATA_DIR = Path("data")
OUT_DIR = Path("tmp/demo_output")
OUT_DIR.mkdir(parents=True, exist_ok=True)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def load_json(path):
    with open(path) as f:
        return json.load(f)


def load_all_extractions():
    """Load all extraction.json files from data/."""
    bills = {}
    for d in sorted(DATA_DIR.iterdir()):
        ext_path = d / "extraction.json"
        if ext_path.exists():
            bills[d.name] = load_json(ext_path)
    return bills


def load_all_bill_meta():
    """Load all bill_meta.json files."""
    metas = {}
    for d in sorted(DATA_DIR.iterdir()):
        meta_path = d / "bill_meta.json"
        if meta_path.exists():
            metas[d.name] = load_json(meta_path)
    return metas


def load_all_tas_mappings():
    """Load all tas_mapping.json files."""
    mappings = {}
    for d in sorted(DATA_DIR.iterdir()):
        tas_path = d / "tas_mapping.json"
        if tas_path.exists():
            mappings[d.name] = load_json(tas_path)
    return mappings


def load_all_verifications():
    """Load all verification.json files."""
    verifications = {}
    for d in sorted(DATA_DIR.iterdir()):
        ver_path = d / "verification.json"
        if ver_path.exists():
            verifications[d.name] = load_json(ver_path)
    return verifications


def ba_provisions(ext):
    """Yield top-level budget authority provisions from an extraction."""
    for p in ext.get("provisions", []):
        if p.get("provision_type") != "appropriation":
            continue
        amt = p.get("amount", {})
        if amt.get("semantics") != "new_budget_authority":
            continue
        dl = p.get("detail_level", "")
        if dl in ("sub_allocation", "proviso_amount"):
            continue
        dollars = (amt.get("value") or {}).get("dollars", 0) or 0
        yield p, dollars


def fiscal_years_for_bill(ext):
    return ext.get("bill", {}).get("fiscal_years", [])


def bill_identifier(ext):
    return ext.get("bill", {}).get("identifier", "?")


def format_dollars(n):
    """Format dollars in human-readable form."""
    if abs(n) >= 1_000_000_000_000:
        return f"${n / 1_000_000_000_000:.1f}T"
    elif abs(n) >= 1_000_000_000:
        return f"${n / 1_000_000_000:.1f}B"
    elif abs(n) >= 1_000_000:
        return f"${n / 1_000_000:.0f}M"
    else:
        return f"${n:,.0f}"


# ---------------------------------------------------------------------------
# Demo 1: FY2026 Budget Authority by Agency
# ---------------------------------------------------------------------------

def demo_fy2026_by_agency(bills):
    print("\n" + "=" * 95)
    print("DEMO 1: FY2026 Budget Authority by Agency (Top 25)")
    print("=" * 95)

    agency_totals = defaultdict(int)
    fy2026_bill_dirs = []
    for bill_dir, ext in bills.items():
        fys = fiscal_years_for_bill(ext)
        if 2026 in fys:
            fy2026_bill_dirs.append(bill_dir)
            for p, dollars in ba_provisions(ext):
                agency = p.get("agency") or "Unknown"
                agency_totals[agency] += dollars

    sorted_agencies = sorted(agency_totals.items(), key=lambda x: -x[1])
    total = sum(v for _, v in sorted_agencies)

    print(f"\nBills covering FY2026: {', '.join(fy2026_bill_dirs)}")
    print(f"\n{'Agency':<60s} {'BA ($)':>20s}  {'%':>6s}")
    print("-" * 90)
    for name, ba in sorted_agencies[:25]:
        pct = ba / total * 100 if total else 0
        print(f"{name[:59]:<60s} ${ba:>18,}  {pct:>5.1f}%")
    print(f"\n{'TOTAL':<60s} ${total:>18,}  100.0%")
    print(f"Agencies: {len(sorted_agencies)}")

    # Save CSV
    csv_path = OUT_DIR / "fy2026_by_agency.csv"
    with open(csv_path, "w") as f:
        f.write("agency,budget_authority,pct_of_total\n")
        for name, ba in sorted_agencies:
            pct = ba / total * 100 if total else 0
            f.write(f'"{name}",{ba},{pct:.2f}\n')
    print(f"\nSaved: {csv_path}")
    return sorted_agencies, total


# ---------------------------------------------------------------------------
# Demo 2: Subcommittee Scorecard FY2020–FY2026
# ---------------------------------------------------------------------------

def demo_subcommittee_scorecard(bills, metas):
    print("\n" + "=" * 95)
    print("DEMO 2: Subcommittee Scorecard — Budget Authority by FY")
    print("=" * 95)

    # Map (bill_dir, division) -> jurisdiction from bill_meta
    bill_div_to_jurisdiction = {}
    for bill_dir, meta in metas.items():
        for sc in meta.get("subcommittees", []):
            bill_div_to_jurisdiction[(bill_dir, sc["division"])] = sc["jurisdiction"]

    # Aggregate by jurisdiction × FY
    scorecard = defaultdict(lambda: defaultdict(int))  # jurisdiction -> fy -> dollars

    for bill_dir, ext in bills.items():
        fys = fiscal_years_for_bill(ext)
        if not fys:
            continue
        primary_fy = max(fys)  # Use the primary FY for the bill

        for p, dollars in ba_provisions(ext):
            div = p.get("division") or ""
            jurisdiction = bill_div_to_jurisdiction.get((bill_dir, div), "other")
            scorecard[jurisdiction][primary_fy] += dollars

    # Display
    target_fys = [2020, 2021, 2022, 2023, 2024, 2025, 2026]
    key_jurisdictions = [
        "defense", "labor_hhs", "thud", "milcon_va", "homeland_security",
        "agriculture", "cjs", "energy_water", "interior",
        "state_foreign_ops", "financial_services", "legislative_branch"
    ]

    print(f"\n{'Subcommittee':<22s}", end="")
    for fy in target_fys:
        print(f"  {'FY' + str(fy):>10s}", end="")
    print(f"  {'Change':>10s}")
    print("-" * (22 + 11 * len(target_fys) + 12))

    rows = []
    for jur in key_jurisdictions:
        vals = scorecard.get(jur, {})
        row_data = {"jurisdiction": jur}
        print(f"{jur:<22s}", end="")
        for fy in target_fys:
            v = vals.get(fy, 0)
            row_data[f"fy{fy}"] = v
            if v > 0:
                print(f"  {format_dollars(v):>10s}", end="")
            else:
                print(f"  {'—':>10s}", end="")
        # Change from first available to last available
        first_val = next((vals.get(fy, 0) for fy in target_fys if vals.get(fy, 0) > 0), 0)
        last_val = next((vals.get(fy, 0) for fy in reversed(target_fys) if vals.get(fy, 0) > 0), 0)
        if first_val > 0 and last_val > 0:
            change_pct = (last_val - first_val) / first_val * 100
            print(f"  {change_pct:>+9.1f}%")
        else:
            print(f"  {'—':>10s}")
        rows.append(row_data)

    # Save CSV
    csv_path = OUT_DIR / "subcommittee_scorecard.csv"
    with open(csv_path, "w") as f:
        f.write("jurisdiction," + ",".join(f"fy{fy}" for fy in target_fys) + "\n")
        for row in rows:
            f.write(row["jurisdiction"] + "," + ",".join(str(row.get(f"fy{fy}", 0)) for fy in target_fys) + "\n")
    print(f"\nSaved: {csv_path}")
    return scorecard


# ---------------------------------------------------------------------------
# Demo 3: Top 10 Account Timelines (from authorities.json)
# ---------------------------------------------------------------------------

def demo_account_timelines():
    print("\n" + "=" * 95)
    print("DEMO 3: Top 10 Federal Accounts by Total Budget Authority — Fiscal Year Timeline")
    print("=" * 95)

    auth = load_json(DATA_DIR / "authorities.json")
    authorities = auth["authorities"]

    # Sort by total dollars
    sorted_auth = sorted(authorities, key=lambda a: -(a.get("total_dollars") or 0))

    top10 = sorted_auth[:10]
    all_fys = sorted(auth["summary"]["fiscal_years_covered"])

    print(f"\n{'FAS Code':<12s} {'Title':<50s} {'Total BA':>16s}")
    print("-" * 80)
    for a in top10:
        print(f"{a['fas_code']:<12s} {a['fas_title'][:49]:<50s} {format_dollars(a.get('total_dollars', 0)):>16s}")

    print(f"\nYear-by-year breakdown:")
    print(f"\n{'FAS Code':<12s}", end="")
    for fy in all_fys:
        print(f"  {'FY' + str(fy):>12s}", end="")
    print()
    print("-" * (12 + 14 * len(all_fys)))

    timeline_data = []
    for a in top10:
        # Build FY -> dollars from provisions
        fy_dollars = defaultdict(int)
        for prov in a.get("provisions", []):
            d = prov.get("dollars") or 0
            for fy in prov.get("fiscal_years", []):
                fy_dollars[fy] += d
        print(f"{a['fas_code']:<12s}", end="")
        row = {"fas_code": a["fas_code"], "title": a["fas_title"]}
        for fy in all_fys:
            v = fy_dollars.get(fy, 0)
            row[f"fy{fy}"] = v
            if v > 0:
                print(f"  {format_dollars(v):>12s}", end="")
            else:
                print(f"  {'—':>12s}", end="")
        print()
        timeline_data.append(row)

    # Save CSV
    csv_path = OUT_DIR / "top10_account_timelines.csv"
    with open(csv_path, "w") as f:
        f.write("fas_code,title," + ",".join(f"fy{fy}" for fy in all_fys) + "\n")
        for row in timeline_data:
            f.write(f'"{row["fas_code"]}","{row["title"]}",' + ",".join(str(row.get(f"fy{fy}", 0)) for fy in all_fys) + "\n")
    print(f"\nSaved: {csv_path}")
    return timeline_data, all_fys


# ---------------------------------------------------------------------------
# Demo 4: Rename Events
# ---------------------------------------------------------------------------

def demo_rename_events():
    print("\n" + "=" * 95)
    print("DEMO 4: Account Rename Events Detected Across Fiscal Years")
    print("=" * 95)

    auth = load_json(DATA_DIR / "authorities.json")
    events = []
    for a in auth["authorities"]:
        for ev in a.get("events", []):
            if ev["event_type"]["type"] == "rename":
                events.append({
                    "fy": ev["fiscal_year"],
                    "fas_code": a["fas_code"],
                    "agency": a["agency_name"],
                    "from": ev["event_type"]["from"],
                    "to": ev["event_type"]["to"],
                })

    events.sort(key=lambda e: (e["fy"], e["fas_code"]))
    print(f"\n{len(events)} rename events detected:\n")
    for ev in events:
        print(f"  FY{ev['fy']}: {ev['fas_code']}  ({ev['agency'][:40]})")
        print(f"    FROM: \"{ev['from'][:70]}\"")
        print(f"      TO: \"{ev['to'][:70]}\"")
        print()

    csv_path = OUT_DIR / "rename_events.csv"
    with open(csv_path, "w") as f:
        f.write("fiscal_year,fas_code,agency,from_name,to_name\n")
        for ev in events:
            f.write(f'{ev["fy"]},"{ev["fas_code"]}","{ev["agency"]}","{ev["from"]}","{ev["to"]}"\n')
    print(f"Saved: {csv_path}")
    return events


# ---------------------------------------------------------------------------
# Demo 5: TAS Resolution Quality
# ---------------------------------------------------------------------------

def demo_tas_quality():
    print("\n" + "=" * 95)
    print("DEMO 5: TAS Resolution Quality by Bill")
    print("=" * 95)

    mappings = load_all_tas_mappings()
    print(f"\n{'Bill':<20s} {'Dir':<16s} {'Total':>7s} {'Determ':>7s} {'LLM':>7s} {'Unmatched':>9s} {'Rate':>7s}")
    print("-" * 80)

    totals = {"total": 0, "det": 0, "llm": 0, "un": 0}
    rows = []
    for bill_dir, m in sorted(mappings.items()):
        s = m["summary"]
        row = {
            "bill": m["bill_identifier"],
            "dir": bill_dir,
            "total": s["total_provisions"],
            "deterministic": s["deterministic_matched"],
            "llm": s["llm_matched"],
            "unmatched": s["unmatched"],
            "rate": s["match_rate_pct"],
        }
        rows.append(row)
        totals["total"] += s["total_provisions"]
        totals["det"] += s["deterministic_matched"]
        totals["llm"] += s["llm_matched"]
        totals["un"] += s["unmatched"]
        print(f"{m['bill_identifier']:<20s} {bill_dir:<16s} {s['total_provisions']:>7d} {s['deterministic_matched']:>7d} {s['llm_matched']:>7d} {s['unmatched']:>9d} {s['match_rate_pct']:>6.1f}%")

    overall_rate = (totals["det"] + totals["llm"]) / totals["total"] * 100 if totals["total"] else 0
    print(f"\n{'TOTAL':<37s} {totals['total']:>7d} {totals['det']:>7d} {totals['llm']:>7d} {totals['un']:>9d} {overall_rate:>6.1f}%")

    csv_path = OUT_DIR / "tas_quality.csv"
    with open(csv_path, "w") as f:
        f.write("bill,bill_dir,total,deterministic,llm,unmatched,match_rate_pct\n")
        for row in rows:
            f.write(f'"{row["bill"]}","{row["dir"]}",{row["total"]},{row["deterministic"]},{row["llm"]},{row["unmatched"]},{row["rate"]:.1f}\n')
    print(f"\nSaved: {csv_path}")
    return rows


# ---------------------------------------------------------------------------
# Demo 6: FY2026 Treemap (Plotly HTML)
# ---------------------------------------------------------------------------

def demo_treemap(bills, metas):
    print("\n" + "=" * 95)
    print("DEMO 6: FY2026 Budget Treemap (Interactive HTML)")
    print("=" * 95)

    try:
        import plotly.express as px
        import pandas as pd
    except ImportError:
        print("  Skipping — plotly/pandas not installed")
        return

    # Build treemap data: jurisdiction -> agency -> account -> dollars
    bill_div_to_jurisdiction = {}
    for bill_dir, meta in metas.items():
        for sc in meta.get("subcommittees", []):
            bill_div_to_jurisdiction[(bill_dir, sc["division"])] = sc.get("title", sc["jurisdiction"])

    records = []
    for bill_dir, ext in bills.items():
        fys = fiscal_years_for_bill(ext)
        if 2026 not in fys:
            continue
        for p, dollars in ba_provisions(ext):
            if dollars <= 0:
                continue
            div = p.get("division") or ""
            jurisdiction = bill_div_to_jurisdiction.get((bill_dir, div), "Other")
            agency = p.get("agency") or "Unknown"
            account = p.get("account_name") or "Unknown"
            records.append({
                "jurisdiction": jurisdiction[:50],
                "agency": agency[:50],
                "account": account[:60],
                "dollars": dollars,
                "dollars_fmt": format_dollars(dollars),
                "bill": bill_identifier(ext),
            })

    if not records:
        print("  No FY2026 data found")
        return

    df = pd.DataFrame(records)

    # Aggregate duplicates
    df_agg = df.groupby(["jurisdiction", "agency", "account"], as_index=False)["dollars"].sum()
    df_agg["dollars_billions"] = df_agg["dollars"] / 1e9
    df_agg["dollars_fmt"] = df_agg["dollars"].apply(format_dollars)

    fig = px.treemap(
        df_agg,
        path=["jurisdiction", "agency", "account"],
        values="dollars",
        title="FY2026 Federal Discretionary Budget Authority by Jurisdiction / Agency / Account",
        hover_data={"dollars_fmt": True, "dollars": False},
        color="dollars_billions",
        color_continuous_scale="Blues",
    )
    fig.update_layout(
        width=1400,
        height=900,
        font_size=12,
        coloraxis_colorbar_title="$B",
    )

    html_path = OUT_DIR / "fy2026_treemap.html"
    fig.write_html(str(html_path), include_plotlyjs="cdn")
    print(f"  Generated interactive treemap: {html_path}")
    print(f"  Records: {len(df_agg)}, Total: {format_dollars(df_agg['dollars'].sum())}")
    return df_agg


# ---------------------------------------------------------------------------
# Demo 7: Defense vs Non-Defense Spending Trend
# ---------------------------------------------------------------------------

def demo_defense_trend(bills, metas):
    print("\n" + "=" * 95)
    print("DEMO 7: Defense vs. Non-Defense Discretionary Spending Trend")
    print("=" * 95)

    bill_div_to_jurisdiction = {}
    for bill_dir, meta in metas.items():
        for sc in meta.get("subcommittees", []):
            bill_div_to_jurisdiction[(bill_dir, sc["division"])] = sc["jurisdiction"]

    # Aggregate by FY and defense/non-defense
    fy_defense = defaultdict(int)
    fy_nondefense = defaultdict(int)

    for bill_dir, ext in bills.items():
        fys = fiscal_years_for_bill(ext)
        if not fys:
            continue
        primary_fy = max(fys)

        for p, dollars in ba_provisions(ext):
            div = p.get("division") or ""
            jur = bill_div_to_jurisdiction.get((bill_dir, div), "other")
            if jur == "defense":
                fy_defense[primary_fy] += dollars
            else:
                fy_nondefense[primary_fy] += dollars

    target_fys = sorted(set(list(fy_defense.keys()) + list(fy_nondefense.keys())))
    target_fys = [fy for fy in target_fys if 2019 <= fy <= 2026]

    print(f"\n{'FY':<6s} {'Defense':>16s} {'Non-Defense':>16s} {'Total':>16s} {'Def %':>8s}")
    print("-" * 66)
    for fy in target_fys:
        d = fy_defense.get(fy, 0)
        nd = fy_nondefense.get(fy, 0)
        t = d + nd
        pct = d / t * 100 if t else 0
        print(f"FY{fy:<4d} {format_dollars(d):>16s} {format_dollars(nd):>16s} {format_dollars(t):>16s} {pct:>7.1f}%")

    # Generate chart
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        import matplotlib.ticker as mticker

        fig, ax = plt.subplots(figsize=(12, 6))

        def_vals = [fy_defense.get(fy, 0) / 1e12 for fy in target_fys]
        nondef_vals = [fy_nondefense.get(fy, 0) / 1e12 for fy in target_fys]
        fy_labels = [f"FY{fy}" for fy in target_fys]

        ax.bar(fy_labels, def_vals, label="Defense", color="#2c5f8a", width=0.6)
        ax.bar(fy_labels, nondef_vals, bottom=def_vals, label="Non-Defense", color="#7fb3d8", width=0.6)

        ax.set_ylabel("Budget Authority ($ Trillions)")
        ax.set_title("Federal Discretionary Budget Authority: Defense vs. Non-Defense (FY2019–FY2026)")
        ax.legend(loc="upper left")
        ax.yaxis.set_major_formatter(mticker.FormatStrFormatter("$%.1fT"))
        ax.set_ylim(0, max(d + nd for d, nd in zip(def_vals, nondef_vals)) * 1.15)

        for i, fy in enumerate(target_fys):
            total = def_vals[i] + nondef_vals[i]
            ax.text(i, total + 0.05, f"${total:.1f}T", ha="center", va="bottom", fontsize=9)

        plt.tight_layout()
        chart_path = OUT_DIR / "defense_vs_nondefense.png"
        plt.savefig(chart_path, dpi=150)
        plt.close()
        print(f"\n  Saved chart: {chart_path}")
    except Exception as e:
        print(f"\n  Chart generation failed: {e}")


# ---------------------------------------------------------------------------
# Demo 8: CR Substitution Impact Analysis
# ---------------------------------------------------------------------------

def demo_cr_substitutions(bills):
    print("\n" + "=" * 95)
    print("DEMO 8: CR Substitution Impact — Programs Congress Changed from Prior-Year Rates")
    print("=" * 95)

    subs = []
    for bill_dir, ext in bills.items():
        bill_id = bill_identifier(ext)
        fys = fiscal_years_for_bill(ext)
        for p in ext.get("provisions", []):
            if p.get("provision_type") != "cr_substitution":
                continue
            new_amt = p.get("new_amount", {})
            old_amt = p.get("old_amount", {})
            new_d = (new_amt.get("value") or {}).get("dollars", 0) or 0
            old_d = (old_amt.get("value") or {}).get("dollars", 0) or 0
            delta = new_d - old_d
            account = p.get("account_name") or p.get("reference_section") or "(unnamed)"
            subs.append({
                "bill": bill_id,
                "bill_dir": bill_dir,
                "fys": fys,
                "account": account,
                "new": new_d,
                "old": old_d,
                "delta": delta,
                "section": p.get("section", ""),
            })

    subs.sort(key=lambda s: -abs(s["delta"]))

    total_cuts = sum(s["delta"] for s in subs if s["delta"] < 0)
    total_increases = sum(s["delta"] for s in subs if s["delta"] > 0)

    print(f"\nTotal CR substitutions across all bills: {len(subs)}")
    print(f"Total cuts:      {format_dollars(total_cuts)}")
    print(f"Total increases: {format_dollars(total_increases)}")
    print(f"Net impact:      {format_dollars(total_cuts + total_increases)}")

    print(f"\nTop 15 biggest changes (by absolute delta):")
    print(f"{'Bill':<18s} {'Account':<45s} {'Delta':>14s}")
    print("-" * 80)
    for s in subs[:15]:
        sign = "+" if s["delta"] >= 0 else ""
        print(f"{s['bill']:<18s} {s['account'][:44]:<45s} {sign}{format_dollars(s['delta']):>13s}")

    # By-bill summary
    print(f"\nCR substitutions by bill:")
    bill_totals = defaultdict(lambda: {"count": 0, "cuts": 0, "increases": 0})
    for s in subs:
        bt = bill_totals[s["bill"]]
        bt["count"] += 1
        if s["delta"] < 0:
            bt["cuts"] += s["delta"]
        else:
            bt["increases"] += s["delta"]

    print(f"{'Bill':<20s} {'Count':>6s} {'Cuts':>14s} {'Increases':>14s} {'Net':>14s}")
    print("-" * 72)
    for bill, bt in sorted(bill_totals.items()):
        net = bt["cuts"] + bt["increases"]
        print(f"{bill:<20s} {bt['count']:>6d} {format_dollars(bt['cuts']):>14s} {format_dollars(bt['increases']):>14s} {format_dollars(net):>14s}")

    csv_path = OUT_DIR / "cr_substitutions.csv"
    with open(csv_path, "w") as f:
        f.write("bill,account,section,new_dollars,old_dollars,delta\n")
        for s in subs:
            f.write(f'"{s["bill"]}","{s["account"]}","{s["section"]}",{s["new"]},{s["old"]},{s["delta"]}\n')
    print(f"\nSaved: {csv_path}")
    return subs


# ---------------------------------------------------------------------------
# Demo 9: Biggest Year-over-Year Account Changes (via authorities)
# ---------------------------------------------------------------------------

def demo_biggest_changes():
    print("\n" + "=" * 95)
    print("DEMO 9: Biggest Year-over-Year Account Changes (FY2024 → FY2026)")
    print("=" * 95)

    auth = load_json(DATA_DIR / "authorities.json")

    changes = []
    for a in auth["authorities"]:
        fy_dollars = defaultdict(int)
        for prov in a.get("provisions", []):
            d = prov.get("dollars") or 0
            for fy in prov.get("fiscal_years", []):
                fy_dollars[fy] += d

        fy2024 = fy_dollars.get(2024, 0)
        fy2026 = fy_dollars.get(2026, 0)
        if fy2024 > 0 and fy2026 > 0:
            delta = fy2026 - fy2024
            pct = (fy2026 - fy2024) / fy2024 * 100
            changes.append({
                "fas_code": a["fas_code"],
                "title": a["fas_title"],
                "agency": a["agency_name"],
                "fy2024": fy2024,
                "fy2026": fy2026,
                "delta": delta,
                "pct": pct,
            })

    # Top 15 increases
    changes.sort(key=lambda c: -c["delta"])
    print(f"\nTop 15 INCREASES (FY2024 → FY2026):")
    print(f"{'Account':<50s} {'FY2024':>14s} {'FY2026':>14s} {'Delta':>14s} {'%':>8s}")
    print("-" * 104)
    for c in changes[:15]:
        print(f"{c['title'][:49]:<50s} {format_dollars(c['fy2024']):>14s} {format_dollars(c['fy2026']):>14s} {format_dollars(c['delta']):>14s} {c['pct']:>+7.1f}%")

    # Top 15 decreases
    changes.sort(key=lambda c: c["delta"])
    print(f"\nTop 15 DECREASES (FY2024 → FY2026):")
    print(f"{'Account':<50s} {'FY2024':>14s} {'FY2026':>14s} {'Delta':>14s} {'%':>8s}")
    print("-" * 104)
    for c in changes[:15]:
        print(f"{c['title'][:49]:<50s} {format_dollars(c['fy2024']):>14s} {format_dollars(c['fy2026']):>14s} {format_dollars(c['delta']):>14s} {c['pct']:>+7.1f}%")

    csv_path = OUT_DIR / "biggest_changes_2024_2026.csv"
    changes.sort(key=lambda c: -abs(c["delta"]))
    with open(csv_path, "w") as f:
        f.write("fas_code,title,agency,fy2024,fy2026,delta,pct_change\n")
        for c in changes:
            f.write(f'"{c["fas_code"]}","{c["title"]}","{c["agency"]}",{c["fy2024"]},{c["fy2026"]},{c["delta"]},{c["pct"]:.1f}\n')
    print(f"\nSaved: {csv_path}")
    return changes


# ---------------------------------------------------------------------------
# Demo 10: Verification Quality Heatmap
# ---------------------------------------------------------------------------

def demo_verification_heatmap():
    print("\n" + "=" * 95)
    print("DEMO 10: Verification Quality Heatmap Across All Bills")
    print("=" * 95)

    verifications = load_all_verifications()

    rows = []
    for bill_dir, ver in sorted(verifications.items()):
        s = ver.get("summary", {})
        total = s.get("total_provisions", 0)
        if total == 0:
            continue
        rows.append({
            "bill": bill_dir,
            "provisions": total,
            "verified": s.get("amounts_verified", 0),
            "not_found": s.get("amounts_not_found", 0),
            "ambiguous": s.get("amounts_ambiguous", 0),
            "exact": s.get("raw_text_exact", 0),
            "normalized": s.get("raw_text_normalized", 0),
            "spaceless": s.get("raw_text_spaceless", 0),
            "no_match": s.get("raw_text_no_match", 0),
            "coverage": s.get("completeness_pct", 0),
        })

    print(f"\n{'Bill':<18s} {'Provs':>6s} {'Verified':>8s} {'NotFound':>8s} {'Exact':>6s} {'Norm':>6s} {'NoMatch':>7s} {'Cov%':>6s}")
    print("-" * 72)
    for r in rows:
        print(f"{r['bill']:<18s} {r['provisions']:>6d} {r['verified']:>8d} {r['not_found']:>8d} {r['exact']:>6d} {r['normalized']:>6d} {r['no_match']:>7d} {r['coverage']:>5.1f}%")

    # Totals
    t_prov = sum(r["provisions"] for r in rows)
    t_ver = sum(r["verified"] for r in rows)
    t_nf = sum(r["not_found"] for r in rows)
    t_exact = sum(r["exact"] for r in rows)
    t_norm = sum(r["normalized"] for r in rows)
    t_nomatch = sum(r["no_match"] for r in rows)
    print(f"\n{'TOTAL':<18s} {t_prov:>6d} {t_ver:>8d} {t_nf:>8d} {t_exact:>6d} {t_norm:>6d} {t_nomatch:>7d}")
    print(f"\nNot Found rate: {t_nf}/{t_prov} = {t_nf/t_prov*100:.3f}%")
    print(f"Exact match rate: {t_exact}/{t_prov} = {t_exact/t_prov*100:.1f}%")

    # Generate heatmap
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        import numpy as np

        bills_labels = [r["bill"] for r in rows]
        metrics = ["verified", "ambiguous", "not_found", "exact", "normalized", "no_match"]
        metric_labels = ["$ Verified", "$ Ambiguous", "$ Not Found", "Text Exact", "Text Normalized", "Text No Match"]

        # Normalize each row by provision count
        data = []
        for r in rows:
            total = r["provisions"]
            if total == 0:
                data.append([0] * len(metrics))
            else:
                data.append([r[m] / total * 100 for m in metrics])

        data_np = np.array(data)

        fig, ax = plt.subplots(figsize=(10, max(8, len(rows) * 0.35)))
        im = ax.imshow(data_np, cmap="YlOrRd_r", aspect="auto", vmin=0, vmax=100)

        ax.set_xticks(range(len(metric_labels)))
        ax.set_xticklabels(metric_labels, rotation=45, ha="right", fontsize=9)
        ax.set_yticks(range(len(bills_labels)))
        ax.set_yticklabels(bills_labels, fontsize=8)

        # Add text annotations
        for i in range(len(rows)):
            for j in range(len(metrics)):
                val = data_np[i, j]
                color = "white" if val < 30 else "black"
                if val > 0:
                    ax.text(j, i, f"{val:.0f}%", ha="center", va="center", fontsize=7, color=color)

        ax.set_title("Verification Quality by Bill (% of provisions)")
        plt.colorbar(im, label="% of provisions", shrink=0.6)
        plt.tight_layout()

        chart_path = OUT_DIR / "verification_heatmap.png"
        plt.savefig(chart_path, dpi=150)
        plt.close()
        print(f"\nSaved chart: {chart_path}")
    except Exception as e:
        print(f"\nChart generation failed: {e}")

    return rows


# ---------------------------------------------------------------------------
# Demo 11: Semantic Search Showcase (requires OPENAI_API_KEY)
# ---------------------------------------------------------------------------

def demo_semantic_search():
    print("\n" + "=" * 95)
    print("DEMO 11: Semantic Search — Finding Provisions by Meaning")
    print("=" * 95)

    queries = [
        ("school lunch programs for kids", "Zero keyword overlap with 'Child Nutrition Programs'"),
        ("opioid crisis drug treatment", "Finds substance abuse funding under various names"),
        ("space exploration", "Finds NASA accounts even without the word 'NASA'"),
        ("clean energy research", "Finds DOE renewable energy programs"),
        ("housing assistance for poor families", "Finds HUD rental assistance programs"),
        ("military pay raises for soldiers", "Finds Military Personnel accounts"),
        ("protecting endangered species", "Finds Fish and Wildlife, EPA programs"),
        ("fighting wildfires", "Finds Forest Service, Interior fire programs"),
        ("veterans mental health", "Finds VA medical and counseling programs"),
        ("border security and immigration", "Finds CBP, ICE, and USCIS programs"),
    ]

    # Check if we can run semantic search
    openai_key = os.environ.get("OPENAI_API_KEY", "")
    if not openai_key:
        print("\n  OPENAI_API_KEY not set — skipping semantic search demos")
        print("  Set it with: source /Users/chris.gorski/openai-cantina-gorski.source")
        return

    results_all = []
    for query, note in queries:
        print(f"\n  Query: \"{query}\"")
        print(f"  Why:   {note}")
        try:
            result = subprocess.run(
                ["cargo", "run", "--release", "--", "search", "--dir", "data",
                 "--semantic", query, "--top", "3", "--format", "json"],
                capture_output=True, text=True, timeout=30,
            )
            if result.returncode == 0 and result.stdout.strip():
                matches = json.loads(result.stdout)
                for m in matches:
                    sim = m.get("similarity", 0)
                    desc = m.get("description", m.get("account_name", "?"))[:55]
                    dollars = m.get("dollars")
                    bill = m.get("bill", "?")
                    d_str = format_dollars(dollars) if dollars else "—"
                    print(f"    {sim:.2f}  {bill:<18s} {desc:<55s} {d_str}")
                results_all.append({"query": query, "note": note, "matches": matches})
            else:
                print(f"    (no results or error)")
        except subprocess.TimeoutExpired:
            print(f"    (timed out)")
        except Exception as e:
            print(f"    (error: {e})")

    if results_all:
        json_path = OUT_DIR / "semantic_search_demos.json"
        with open(json_path, "w") as f:
            json.dump(results_all, f, indent=2)
        print(f"\nSaved: {json_path}")
    return results_all


# ---------------------------------------------------------------------------
# Demo 12: Account Trace Showcase
# ---------------------------------------------------------------------------

def demo_trace_showcase():
    print("\n" + "=" * 95)
    print("DEMO 12: Account Traces — Following Programs Across Fiscal Years")
    print("=" * 95)

    traces = [
        ("070-0400", "Secret Service Operations (name changed across congresses)"),
        ("012-3539", "Child Nutrition Programs (steady growth)"),
        ("069-1750", "Federal Transit Administration Formula Grants"),
        ("097-0100", "Operation and Maintenance, Defense-Wide"),
        ("086-0302", "Tenant-Based Rental Assistance (largest HUD program)"),
        ("036-0140", "Compensation and Pensions, VA (includes advance approps)"),
    ]

    all_traces = []
    for fas_code, note in traces:
        print(f"\n  {fas_code}: {note}")
        try:
            # Use table output (not JSON) — the JSON format doesn't include
            # dollar amounts in timeline entries, but the table output does.
            result = subprocess.run(
                ["cargo", "run", "--release", "--", "trace", fas_code, "--dir", "data"],
                capture_output=True, text=True, timeout=15,
            )
            output = result.stdout + result.stderr
            for line in output.split("\n"):
                line_stripped = line.strip()
                # Skip cargo build noise
                if not line_stripped:
                    continue
                if line_stripped.startswith("Finished") or line_stripped.startswith("Running"):
                    continue
                print(f"    {line_stripped}")
            all_traces.append({"fas_code": fas_code, "note": note, "raw": output})
        except subprocess.TimeoutExpired:
            print(f"    (timed out)")
        except Exception as e:
            print(f"    (error: {e})")

    return all_traces


# ---------------------------------------------------------------------------
# Demo 13: Spending Trend Line Chart (top 6 accounts)
# ---------------------------------------------------------------------------

def demo_spending_trends():
    print("\n" + "=" * 95)
    print("DEMO 13: Spending Trends — Top 6 Accounts by Total BA")
    print("=" * 95)

    auth = load_json(DATA_DIR / "authorities.json")
    all_fys = sorted(auth["summary"]["fiscal_years_covered"])
    all_fys = [fy for fy in all_fys if 2019 <= fy <= 2026]

    # Build timelines for top accounts
    top_accounts = sorted(auth["authorities"], key=lambda a: -(a.get("total_dollars") or 0))[:6]

    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        import matplotlib.ticker as mticker

        fig, ax = plt.subplots(figsize=(14, 7))
        colors = ["#2c5f8a", "#d4533b", "#5ba55b", "#9467bd", "#e8963e", "#17becf"]

        for idx, a in enumerate(top_accounts):
            fy_dollars = defaultdict(int)
            for prov in a.get("provisions", []):
                d = prov.get("dollars") or 0
                for fy in prov.get("fiscal_years", []):
                    fy_dollars[fy] += d

            vals = [fy_dollars.get(fy, None) for fy in all_fys]
            # Filter out None for plotting
            plot_fys = [fy for fy, v in zip(all_fys, vals) if v is not None and v > 0]
            plot_vals = [v / 1e9 for v in vals if v is not None and v > 0]

            label = a["fas_title"][:45]
            ax.plot(plot_fys, plot_vals, marker="o", linewidth=2, label=label, color=colors[idx % len(colors)])
            print(f"  {a['fas_code']}: {label}")
            for fy, v in zip(plot_fys, plot_vals):
                print(f"    FY{fy}: ${v:.1f}B")

        ax.set_xlabel("Fiscal Year")
        ax.set_ylabel("Budget Authority ($ Billions)")
        ax.set_title("Top 6 Federal Accounts by Budget Authority (FY2019–FY2026)")
        ax.legend(loc="upper left", fontsize=8)
        ax.yaxis.set_major_formatter(mticker.FormatStrFormatter("$%.0fB"))
        ax.set_xticks(all_fys)
        ax.set_xticklabels([f"FY{fy}" for fy in all_fys])
        ax.grid(axis="y", alpha=0.3)

        plt.tight_layout()
        chart_path = OUT_DIR / "spending_trends_top6.png"
        plt.savefig(chart_path, dpi=150)
        plt.close()
        print(f"\nSaved chart: {chart_path}")
    except Exception as e:
        print(f"\nChart generation failed: {e}")


# ---------------------------------------------------------------------------
# Demo 14: Dataset Summary Card
# ---------------------------------------------------------------------------

def demo_summary_card(bills):
    print("\n" + "=" * 95)
    print("DEMO 14: Dataset Summary Card")
    print("=" * 95)

    auth = load_json(DATA_DIR / "authorities.json")
    total_provisions = sum(len(ext.get("provisions", [])) for ext in bills.values())
    total_ba = 0
    total_rescissions = 0
    for ext in bills.values():
        for p in ext.get("provisions", []):
            amt = p.get("amount") or {}
            val_obj = amt.get("value") or {}
            val = val_obj.get("dollars", 0) or 0
            sem = amt.get("semantics", "")
            dl = p.get("detail_level", "")
            pt = p.get("provision_type", "")
            if pt == "appropriation" and sem == "new_budget_authority" and dl not in ("sub_allocation", "proviso_amount"):
                total_ba += val
            if pt == "rescission" and sem == "rescission":
                total_rescissions += abs(val)

    all_fys = set()
    for ext in bills.values():
        all_fys.update(fiscal_years_for_bill(ext))

    card = {
        "bills": len(bills),
        "provisions": total_provisions,
        "fiscal_years": sorted(all_fys),
        "budget_authority": total_ba,
        "rescissions": total_rescissions,
        "net_ba": total_ba - total_rescissions,
        "authorities": auth["summary"]["total_authorities"],
        "cross_bill_linked": auth["summary"]["authorities_in_multiple_bills"],
        "rename_events": auth["summary"]["total_events"],
    }

    print(f"""
  ┌─────────────────────────────────────────────────────────┐
  │  Congressional Appropriations Dataset                   │
  ├─────────────────────────────────────────────────────────┤
  │  Bills:              {card['bills']:>10,}                        │
  │  Provisions:         {card['provisions']:>10,}                        │
  │  Fiscal Years:       {card['fiscal_years'][0]}–{card['fiscal_years'][-1]:>24}  │
  │  Budget Authority:   {format_dollars(card['budget_authority']):>14s}                    │
  │  Rescissions:        {format_dollars(card['rescissions']):>14s}                    │
  │  Net BA:             {format_dollars(card['net_ba']):>14s}                    │
  │  TAS Authorities:    {card['authorities']:>10,}                        │
  │  Cross-Bill Links:   {card['cross_bill_linked']:>10,}                        │
  │  Rename Events:      {card['rename_events']:>10,}                        │
  └─────────────────────────────────────────────────────────┘
""")

    json_path = OUT_DIR / "dataset_summary.json"
    with open(json_path, "w") as f:
        json.dump(card, f, indent=2)
    print(f"Saved: {json_path}")
    return card


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    print("=" * 95)
    print("  CONGRESSIONAL APPROPRIATIONS — DEMO SUITE")
    print("  Loading 32 bills from data/...")
    print("=" * 95)

    bills = load_all_extractions()
    metas = load_all_bill_meta()
    print(f"  Loaded {len(bills)} bills, {len(metas)} with metadata")

    # Run all demos
    demo_summary_card(bills)
    demo_fy2026_by_agency(bills)
    demo_subcommittee_scorecard(bills, metas)
    demo_account_timelines()
    demo_rename_events()
    demo_tas_quality()
    demo_treemap(bills, metas)
    demo_defense_trend(bills, metas)
    demo_cr_substitutions(bills)
    demo_biggest_changes()
    demo_verification_heatmap()
    demo_trace_showcase()
    demo_spending_trends()
    demo_semantic_search()

    # --- Part 2: Python-from-JSON & CLI Export Demos ---
    demo_python_load_extraction()
    demo_python_pandas_authorities()
    demo_python_source_span_proof()
    demo_python_cross_bill_tas()
    demo_cli_csv_pandas_roundtrip()
    demo_cli_json_jq_recipes()
    demo_cli_jsonl_streaming()
    demo_python_earmark_finder(bills)
    demo_python_advance_analysis(bills, metas)
    demo_cli_python_compare_pipeline()

    print("\n" + "=" * 95)
    print("  ALL DEMOS COMPLETE")
    print(f"  Output files in: {OUT_DIR}/")
    print("=" * 95)
    print()
    for f in sorted(OUT_DIR.iterdir()):
        size = f.stat().st_size
        if size > 1_000_000:
            size_str = f"{size / 1_000_000:.1f} MB"
        elif size > 1_000:
            size_str = f"{size / 1_000:.1f} KB"
        else:
            size_str = f"{size} B"
        print(f"  {f.name:<45s} {size_str:>10s}")


# ---------------------------------------------------------------------------
# Demo 15: Python — Load extraction.json and analyze provision types
# ---------------------------------------------------------------------------

def demo_python_load_extraction():
    print("\n" + "=" * 95)
    print("DEMO 15: Python — Load extraction.json and Analyze Provision Types")
    print("  Shows: How to load and work with extraction data from Python")
    print("=" * 95)

    # Pick a mid-size bill for demonstration
    ext = load_json(DATA_DIR / "119-hr7148" / "extraction.json")
    bill = ext["bill"]
    provisions = ext["provisions"]

    print(f"""
  # ----- Python code (copy-paste ready) -----
  import json

  ext = json.load(open('data/119-hr7148/extraction.json'))
  provisions = ext['provisions']

  # Count by type
  from collections import Counter
  type_counts = Counter(p['provision_type'] for p in provisions)
  for ptype, count in type_counts.most_common():
      print(f'  {{ptype}}: {{count}}')
  # -------------------------------------------
""")

    from collections import Counter
    type_counts = Counter(p["provision_type"] for p in provisions)
    print(f"  Bill: {bill['identifier']} ({bill.get('classification', '?')})")
    print(f"  Total provisions: {len(provisions)}")
    print(f"\n  Provision type breakdown:")
    for ptype, count in type_counts.most_common():
        print(f"    {ptype:<40s} {count:>6d}")

    # Show top 5 appropriations by dollar amount
    approp_provs = []
    for p in provisions:
        if p.get("provision_type") == "appropriation":
            amt = p.get("amount", {})
            dollars = (amt.get("value") or {}).get("dollars", 0) or 0
            if dollars > 0 and amt.get("semantics") == "new_budget_authority":
                dl = p.get("detail_level", "")
                if dl not in ("sub_allocation", "proviso_amount"):
                    approp_provs.append((p, dollars))

    approp_provs.sort(key=lambda x: -x[1])
    print(f"\n  Top 10 appropriations by dollar amount:")
    print(f"  {'Account':<50s} {'Agency':<30s} {'Amount':>16s}")
    print(f"  {'-'*98}")
    for p, dollars in approp_provs[:10]:
        acct = (p.get("account_name") or "?")[:49]
        agency = (p.get("agency") or "?")[:29]
        print(f"  {acct:<50s} {agency:<30s} {format_dollars(dollars):>16s}")

    print(f"""
  # ----- Access any field directly -----
  p = provisions[0]
  p['provision_type']     # → '{provisions[0].get("provision_type")}'
  p['account_name']       # → '{provisions[0].get("account_name", "")[:50]}'
  p['amount']['value']    # → {provisions[0].get("amount", {}).get("value", {})}
  p['amount']['semantics']# → '{provisions[0].get("amount", {}).get("semantics", "")}'
  p['raw_text'][:80]      # → '{provisions[0].get("raw_text", "")[:80]}'
  p['confidence']         # → {provisions[0].get("confidence")}
  p['section']            # → '{provisions[0].get("section", "")}'
  p['division']           # → '{provisions[0].get("division", "")}'
  # -------------------------------------------
""")


# ---------------------------------------------------------------------------
# Demo 16: Python — Load authorities.json into pandas
# ---------------------------------------------------------------------------

def demo_python_pandas_authorities():
    print("\n" + "=" * 95)
    print("DEMO 16: Python — Load authorities.json into a Pandas DataFrame")
    print("  Shows: Full dataset in a DataFrame for analysis, groupby, export")
    print("=" * 95)

    try:
        import pandas as pd
    except ImportError:
        print("  Skipping — pandas not installed")
        return

    auth = load_json(DATA_DIR / "authorities.json")

    print(f"""
  # ----- Python code (copy-paste ready) -----
  import json, pandas as pd

  auth = json.load(open('data/authorities.json'))

  # Flatten provisions into one row per provision-FY pair
  rows = []
  for a in auth['authorities']:
      for prov in a['provisions']:
          for fy in prov['fiscal_years']:
              rows.append({{
                  'fas_code': a['fas_code'],
                  'agency': a['agency_name'],
                  'title': a['fas_title'],
                  'fiscal_year': fy,
                  'dollars': prov.get('dollars', 0) or 0,
                  'bill': prov['bill_identifier'],
                  'confidence': prov['confidence'],
              }})

  df = pd.DataFrame(rows)
  print(df.shape)
  print(df.groupby('fiscal_year')['dollars'].sum())
  # -------------------------------------------
""")

    rows = []
    for a in auth["authorities"]:
        for prov in a.get("provisions", []):
            for fy in prov.get("fiscal_years", []):
                rows.append({
                    "fas_code": a["fas_code"],
                    "agency": a["agency_name"],
                    "title": a["fas_title"],
                    "fiscal_year": fy,
                    "dollars": prov.get("dollars", 0) or 0,
                    "bill": prov["bill_identifier"],
                    "confidence": prov["confidence"],
                })

    df = pd.DataFrame(rows)
    print(f"  DataFrame shape: {df.shape} (rows × columns)")
    print(f"  Columns: {list(df.columns)}")

    print(f"\n  Budget Authority by Fiscal Year:")
    fy_totals = df.groupby("fiscal_year")["dollars"].sum().sort_index()
    for fy, total in fy_totals.items():
        print(f"    FY{fy}: {format_dollars(total)}")

    print(f"\n  Top 10 agencies by total BA:")
    agency_totals = df.groupby("agency")["dollars"].sum().sort_values(ascending=False)
    for agency, total in agency_totals.head(10).items():
        print(f"    {agency[:55]:<56s} {format_dollars(total):>14s}")

    print(f"\n  Confidence distribution:")
    conf_counts = df["confidence"].value_counts()
    for conf, count in conf_counts.items():
        print(f"    {conf:<20s} {count:>6d} ({count/len(df)*100:.1f}%)")

    # Save the DataFrame as CSV
    csv_path = OUT_DIR / "authorities_flat.csv"
    df.to_csv(csv_path, index=False)
    print(f"\n  Saved flattened DataFrame: {csv_path} ({len(df)} rows)")

    # Show a pivot table example
    print(f"\n  Pivot table — Defense agencies across FYs:")
    defense_df = df[df["agency"].str.contains("Defense|Army|Navy|Air Force", case=False, na=False)]
    pivot = defense_df.pivot_table(values="dollars", index="agency", columns="fiscal_year", aggfunc="sum", fill_value=0)
    if not pivot.empty:
        for agency in pivot.index[:5]:
            vals = pivot.loc[agency]
            nonzero = [(fy, v) for fy, v in vals.items() if v > 0]
            if nonzero:
                first_fy, first_v = nonzero[0]
                last_fy, last_v = nonzero[-1]
                print(f"    {agency[:45]:<46s} FY{first_fy}: {format_dollars(first_v):>12s} → FY{last_fy}: {format_dollars(last_v):>12s}")


# ---------------------------------------------------------------------------
# Demo 17: Python — Source span verification (mechanically prove traceability)
# ---------------------------------------------------------------------------

def demo_python_source_span_proof():
    print("\n" + "=" * 95)
    print("DEMO 17: Python — Mechanical Source Span Verification")
    print("  Shows: How to independently verify any provision against the enrolled bill")
    print("=" * 95)

    print(f"""
  # ----- Python code (copy-paste ready) -----
  import json

  ext = json.load(open('data/118-hr9468/extraction.json'))
  for i, p in enumerate(ext['provisions']):
      span = p.get('source_span')
      if not span or not span.get('verified'):
          continue
      source_bytes = open(f'data/118-hr9468/{{span["file"]}}', 'rb').read()
      actual = source_bytes[span['start']:span['end']].decode('utf-8')
      assert actual == p['raw_text'], f'MISMATCH at provision {{i}}'
  print('All provisions verified.')
  # -------------------------------------------
""")

    # Run it for real across multiple bills
    test_bills = ["118-hr9468", "118-hr4366", "119-hr7148", "119-hr5371"]
    total_checked = 0
    total_matched = 0

    for bill_dir in test_bills:
        ext_path = DATA_DIR / bill_dir / "extraction.json"
        if not ext_path.exists():
            continue
        ext = load_json(ext_path)
        for i, p in enumerate(ext.get("provisions", [])):
            span = p.get("source_span")
            if not span or not span.get("file"):
                continue
            total_checked += 1
            source_file = DATA_DIR / bill_dir / span["file"]
            if not source_file.exists():
                continue
            source_bytes = source_file.read_bytes()
            actual = source_bytes[span["start"]:span["end"]].decode("utf-8")
            if actual == p.get("raw_text", ""):
                total_matched += 1

    print(f"  Verified {total_checked} provisions across {len(test_bills)} bills:")
    print(f"    Byte-exact match: {total_matched}/{total_checked} ({total_matched/total_checked*100:.1f}%)")
    if total_matched == total_checked:
        print(f"    ✅ Every source_span[start:end] == raw_text")
    else:
        print(f"    ⚠ {total_checked - total_matched} mismatches found")

    # Show the detailed mechanics for one provision
    ext = load_json(DATA_DIR / "118-hr9468" / "extraction.json")
    p = ext["provisions"][0]
    span = p.get("source_span", {})
    print(f"""
  Detailed proof for H.R. 9468, provision 0:
    Type:       {p['provision_type']}
    Account:    {p.get('account_name', '')}
    Dollars:    ${p.get('amount', {}).get('value', {}).get('dollars', 0):,}
    Span:       bytes {span.get('start')}..{span.get('end')} in {span.get('file')}
    Match tier: {span.get('match_tier')}
    Verified:   {span.get('verified')}

    raw_text[:80]:
      "{p.get('raw_text', '')[:80]}"

    source_bytes[{span.get('start')}:{span.get('end')}][:80]:
      "{open(DATA_DIR / '118-hr9468' / span['file'], 'rb').read()[span['start']:span['end']].decode('utf-8')[:80]}"
""")


# ---------------------------------------------------------------------------
# Demo 18: Python — Cross-bill account matching via TAS codes
# ---------------------------------------------------------------------------

def demo_python_cross_bill_tas():
    print("\n" + "=" * 95)
    print("DEMO 18: Python — Cross-Bill Account Matching via TAS Codes")
    print("  Shows: How to use tas_mapping.json to track accounts across bills")
    print("=" * 95)

    print(f"""
  # ----- Python code (copy-paste ready) -----
  import json
  from collections import defaultdict

  # Load TAS mappings from two bills
  fy24 = json.load(open('data/118-hr4366/tas_mapping.json'))
  fy26 = json.load(open('data/119-hr7148/tas_mapping.json'))

  # Index by FAS code
  fy24_by_fas = {{m['fas_code']: m for m in fy24['mappings'] if m.get('fas_code')}}
  fy26_by_fas = {{m['fas_code']: m for m in fy26['mappings'] if m.get('fas_code')}}

  # Find accounts that appear in both
  shared = set(fy24_by_fas) & set(fy26_by_fas)
  print(f'Accounts in both bills: {{len(shared)}}')

  # Show changes
  for fas in sorted(shared)[:10]:
      m24, m26 = fy24_by_fas[fas], fy26_by_fas[fas]
      d24 = m24.get('dollars') or 0
      d26 = m26.get('dollars') or 0
      delta = d26 - d24
      print(f'  {{fas}} {{m24["account_name"][:40]}}:  ${{d24:,}} → ${{d26:,}}  ({{delta:+,}})')
  # -------------------------------------------
""")

    # Run it for real
    tas24_path = DATA_DIR / "118-hr4366" / "tas_mapping.json"
    tas26_path = DATA_DIR / "119-hr7148" / "tas_mapping.json"

    if not tas24_path.exists() or not tas26_path.exists():
        print("  TAS mapping files not found — skipping")
        return

    fy24 = load_json(tas24_path)
    fy26 = load_json(tas26_path)

    fy24_by_fas = {}
    for m in fy24["mappings"]:
        if m.get("fas_code"):
            fy24_by_fas.setdefault(m["fas_code"], []).append(m)
    fy26_by_fas = {}
    for m in fy26["mappings"]:
        if m.get("fas_code"):
            fy26_by_fas.setdefault(m["fas_code"], []).append(m)

    shared = set(fy24_by_fas) & set(fy26_by_fas)
    only_24 = set(fy24_by_fas) - set(fy26_by_fas)
    only_26 = set(fy26_by_fas) - set(fy24_by_fas)

    print(f"  H.R. 4366 (FY2024): {len(fy24_by_fas)} unique FAS codes")
    print(f"  H.R. 7148 (FY2026): {len(fy26_by_fas)} unique FAS codes")
    print(f"  Shared:              {len(shared)}")
    print(f"  Only in FY2024:      {len(only_24)}")
    print(f"  Only in FY2026:      {len(only_26)}")

    # Show top 10 biggest changes among shared accounts
    changes = []
    for fas in shared:
        d24 = sum(m.get("dollars") or 0 for m in fy24_by_fas[fas])
        d26 = sum(m.get("dollars") or 0 for m in fy26_by_fas[fas])
        if d24 > 0 and d26 > 0:
            name24 = fy24_by_fas[fas][0]["account_name"]
            name26 = fy26_by_fas[fas][0]["account_name"]
            changes.append((fas, name24, name26, d24, d26, d26 - d24))

    changes.sort(key=lambda c: -abs(c[5]))
    print(f"\n  Top 10 biggest changes (shared accounts):")
    print(f"  {'FAS Code':<12s} {'Account':<40s} {'FY2024':>14s} {'FY2026':>14s} {'Delta':>14s}")
    print(f"  {'-'*96}")
    for fas, n24, n26, d24, d26, delta in changes[:10]:
        name = n26[:39]
        if n24.lower() != n26.lower():
            name = f"{n26[:18]}←{n24[:18]}"
        print(f"  {fas:<12s} {name:<40s} {format_dollars(d24):>14s} {format_dollars(d26):>14s} {format_dollars(delta):>14s}")

    # Show a renamed account
    renamed = [(fas, n24, n26) for fas, n24, n26, *_ in changes if n24.lower().strip() != n26.lower().strip()]
    if renamed:
        print(f"\n  Accounts with name changes ({len(renamed)} found):")
        for fas, n24, n26 in renamed[:5]:
            print(f"    {fas}: \"{n24[:45]}\" → \"{n26[:45]}\"")


# ---------------------------------------------------------------------------
# Demo 19: CLI Export — CSV → pandas round-trip
# ---------------------------------------------------------------------------

def demo_cli_csv_pandas_roundtrip():
    print("\n" + "=" * 95)
    print("DEMO 19: CLI Export — CSV to Pandas Round-Trip")
    print("  Shows: Export from CLI, load in pandas, analyze, save")
    print("=" * 95)

    try:
        import pandas as pd
    except ImportError:
        print("  Skipping — pandas not installed")
        return

    # Step 1: Export from CLI
    csv_path = OUT_DIR / "cli_export_appropriations.csv"
    print(f"""
  # ----- Shell: Export all appropriations to CSV -----
  congress-approp search --dir data --type appropriation --fy 2026 --format csv > {csv_path}
  # -------------------------------------------
""")
    result = subprocess.run(
        ["cargo", "run", "--release", "--", "search", "--dir", "data",
         "--type", "appropriation", "--fy", "2026", "--format", "csv"],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        print(f"  CLI export failed: {result.stderr[:200]}")
        return

    with open(csv_path, "w") as f:
        f.write(result.stdout)

    # Step 2: Load in pandas
    df = pd.read_csv(csv_path)
    print(f"  Exported {len(df)} rows × {len(df.columns)} columns")
    print(f"  Columns: {list(df.columns)}")

    print(f"""
  # ----- Python: Load and analyze -----
  import pandas as pd

  df = pd.read_csv('{csv_path}')

  # Only top-level budget authority (exclude sub-allocations)
  ba = df[(df['semantics'] == 'new_budget_authority') &
          (~df['detail_level'].isin(['sub_allocation', 'proviso_amount']))]

  print(f'Top-level BA provisions: {{len(ba)}}')
  print(f'Total: ${{ba["dollars"].sum():,.0f}}')

  # Top 10 by dollar amount
  top10 = ba.nlargest(10, 'dollars')[['account_name', 'agency', 'dollars', 'bill']]
  print(top10.to_string())
  # -------------------------------------------
""")

    ba = df[(df["semantics"] == "new_budget_authority") &
            (~df["detail_level"].isin(["sub_allocation", "proviso_amount"]))]

    print(f"  Top-level BA provisions: {len(ba)}")
    print(f"  Total: {format_dollars(ba['dollars'].sum())}")

    print(f"\n  Top 10 by dollar amount:")
    top10 = ba.nlargest(10, "dollars")[["account_name", "agency", "dollars", "bill"]]
    for _, row in top10.iterrows():
        print(f"    {row['account_name'][:40]:<41s} {row['agency'][:25]:<26s} {format_dollars(row['dollars']):>14s}  ({row['bill']})")

    # Step 3: Groupby agency
    print(f"\n  Top 10 agencies by FY2026 BA:")
    agency_ba = ba.groupby("agency")["dollars"].sum().sort_values(ascending=False)
    for agency, total in agency_ba.head(10).items():
        print(f"    {agency[:55]:<56s} {format_dollars(total):>14s}")


# ---------------------------------------------------------------------------
# Demo 20: CLI Export — JSON + jq recipes
# ---------------------------------------------------------------------------

def demo_cli_json_jq_recipes():
    print("\n" + "=" * 95)
    print("DEMO 20: CLI Export — JSON + jq Recipes")
    print("  Shows: Piping CLI JSON output to jq for one-liners")
    print("=" * 95)

    recipes = [
        (
            "Find the 5 biggest rescissions",
            ["search", "--dir", "data", "--type", "rescission", "--format", "json"],
            "jq 'sort_by(-.dollars) | .[0:5] | .[] | {bill, account_name, dollars}'",
            lambda data: sorted(
                [r for r in data if r.get("dollars")],
                key=lambda r: -(r.get("dollars") or 0)
            )[:5],
            lambda results: [
                f"  {r.get('bill',''):<18s} {r.get('account_name','')[:40]:<41s} {format_dollars(abs(r.get('dollars', 0))):>14s}"
                for r in results
            ],
        ),
        (
            "Count provisions by type across all FY2026 bills",
            ["search", "--dir", "data", "--fy", "2026", "--format", "json"],
            "jq 'group_by(.provision_type) | map({type: .[0].provision_type, count: length}) | sort_by(-.count)'",
            lambda data: sorted(
                [{"type": k, "count": len(list(g))}
                 for k, g in __import__("itertools").groupby(
                     sorted(data, key=lambda r: r.get("provision_type", "")),
                     key=lambda r: r.get("provision_type", ""))],
                key=lambda x: -x["count"]
            ),
            lambda results: [f"  {r['type']:<40s} {r['count']:>6d}" for r in results],
        ),
        (
            "Extract all account names and dollar amounts as a flat list",
            ["search", "--dir", "data", "--type", "appropriation", "--fy", "2026", "--format", "json"],
            "jq '.[] | select(.semantics==\"new_budget_authority\") | {account_name, dollars, bill}'",
            lambda data: [
                r for r in data
                if r.get("semantics") == "new_budget_authority" and r.get("dollars")
            ][:8],
            lambda results: [
                f"  {r.get('account_name','')[:45]:<46s} {format_dollars(r.get('dollars',0)):>14s}  ({r.get('bill','')})"
                for r in results
            ],
        ),
    ]

    for title, cli_args, jq_cmd, py_filter, py_format in recipes:
        print(f"\n  Recipe: {title}")
        print(f"  $ congress-approp {' '.join(cli_args)} | {jq_cmd}")
        try:
            result = subprocess.run(
                ["cargo", "run", "--release", "--"] + cli_args,
                capture_output=True, text=True, timeout=30,
            )
            if result.returncode == 0 and result.stdout.strip():
                data = json.loads(result.stdout)
                filtered = py_filter(data)
                lines = py_format(filtered)
                print(f"  Result ({len(filtered)} items):")
                for line in lines:
                    print(line)
            else:
                print(f"  (no results)")
        except Exception as e:
            print(f"  (error: {e})")


# ---------------------------------------------------------------------------
# Demo 21: CLI Export — JSONL streaming pipeline
# ---------------------------------------------------------------------------

def demo_cli_jsonl_streaming():
    print("\n" + "=" * 95)
    print("DEMO 21: CLI Export — JSONL Streaming Pipeline")
    print("  Shows: Process provisions one at a time without loading all into memory")
    print("=" * 95)

    print(f"""
  # ----- Shell: Stream provisions and filter inline -----
  # Find all provisions over $10B, one per line:
  congress-approp search --dir data --type appropriation --min-dollars 10000000000 \\
      --format jsonl | while IFS= read -r line; do
    acct=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('account_name',''))")
    dollars=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('dollars',0))")
    echo "$acct: \\$$dollars"
  done

  # Or in Python:
  import subprocess, json
  proc = subprocess.Popen(
      ['congress-approp', 'search', '--dir', 'data', '--type', 'appropriation',
       '--min-dollars', '10000000000', '--format', 'jsonl'],
      stdout=subprocess.PIPE, text=True)
  for line in proc.stdout:
      p = json.loads(line)
      print(f"{{p['account_name']}}: ${{p['dollars']:,}}")
  # -------------------------------------------
""")

    # Run it
    result = subprocess.run(
        ["cargo", "run", "--release", "--", "search", "--dir", "data",
         "--type", "appropriation", "--min-dollars", "50000000000",
         "--format", "jsonl"],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode == 0 and result.stdout.strip():
        lines = result.stdout.strip().split("\n")
        print(f"  Streaming {len(lines)} provisions over $50B:")
        for line in lines[:15]:
            p = json.loads(line)
            acct = p.get("account_name", "?")[:50]
            dollars = p.get("dollars", 0)
            bill = p.get("bill", "?")
            print(f"    {bill:<18s} {acct:<51s} {format_dollars(dollars):>14s}")
        if len(lines) > 15:
            print(f"    ... and {len(lines) - 15} more")


# ---------------------------------------------------------------------------
# Demo 22: Python — Build a custom "earmark finder"
# ---------------------------------------------------------------------------

def demo_python_earmark_finder(bills):
    print("\n" + "=" * 95)
    print("DEMO 22: Python — Build a Custom Earmark/Directed Spending Finder")
    print("  Shows: Search for specific provision types and extract structured data")
    print("=" * 95)

    print(f"""
  # ----- Python code (copy-paste ready) -----
  import json, os

  earmarks = []
  for bill_dir in os.listdir('data'):
      ext_path = f'data/{{bill_dir}}/extraction.json'
      if not os.path.isfile(ext_path): continue
      ext = json.load(open(ext_path))
      bill_id = ext['bill']['identifier']
      for p in ext['provisions']:
          if p['provision_type'] == 'directed_spending':
              earmarks.append({{
                  'bill': bill_id,
                  'account': p.get('account_name', ''),
                  'dollars': (p.get('amount',{{}}).get('value',{{}}).get('dollars',0)),
                  'recipient': (p.get('earmark',{{}}) or {{}}).get('recipient',''),
                  'location': (p.get('earmark',{{}}) or {{}}).get('location',''),
              }})
  print(f'Found {{len(earmarks)}} directed spending provisions')
  # -------------------------------------------
""")

    earmarks = []
    for bill_dir, ext in bills.items():
        bill_id = bill_identifier(ext)
        for p in ext.get("provisions", []):
            if p.get("provision_type") == "directed_spending":
                earmark = p.get("earmark") or {}
                amt = p.get("amount", {})
                dollars = (amt.get("value") or {}).get("dollars", 0) or 0
                earmarks.append({
                    "bill": bill_id,
                    "bill_dir": bill_dir,
                    "account": p.get("account_name") or "",
                    "dollars": dollars,
                    "recipient": earmark.get("recipient", ""),
                    "location": earmark.get("location", ""),
                    "member": earmark.get("requesting_member", ""),
                    "section": p.get("section", ""),
                    "division": p.get("division", ""),
                })

    earmarks.sort(key=lambda e: -(e["dollars"] or 0))
    print(f"  Found {len(earmarks)} directed spending provisions across {len(bills)} bills")

    if earmarks:
        total = sum(e["dollars"] for e in earmarks)
        print(f"  Total directed spending: {format_dollars(total)}")
        print(f"\n  Top 15 by dollar amount:")
        print(f"  {'Bill':<18s} {'Account':<30s} {'Amount':>14s} {'Recipient':<30s}")
        print(f"  {'-'*95}")
        for e in earmarks[:15]:
            print(f"  {e['bill']:<18s} {e['account'][:29]:<30s} {format_dollars(e['dollars']):>14s} {e['recipient'][:29]}")

    # Also find provisions with earmark sub-objects on appropriations
    earmark_subs = []
    for bill_dir, ext in bills.items():
        bill_id = bill_identifier(ext)
        for p in ext.get("provisions", []):
            for em in p.get("earmarks", []) or []:
                if em and em.get("recipient"):
                    earmark_subs.append({
                        "bill": bill_id,
                        "account": p.get("account_name", ""),
                        "recipient": em.get("recipient", ""),
                        "location": em.get("location", ""),
                        "member": em.get("requesting_member", ""),
                    })

    if earmark_subs:
        print(f"\n  Additionally, {len(earmark_subs)} earmark sub-objects found on appropriation provisions")
        for e in earmark_subs[:5]:
            print(f"    {e['bill']}: {e['account'][:30]} → {e['recipient'][:50]}")

    csv_path = OUT_DIR / "directed_spending.csv"
    with open(csv_path, "w") as f:
        f.write("bill,account,dollars,recipient,location,member,section,division\n")
        for e in earmarks:
            f.write(f'"{e["bill"]}","{e["account"]}",{e["dollars"]},"{e["recipient"]}","{e["location"]}","{e["member"]}","{e["section"]}","{e["division"]}"\n')
    print(f"\n  Saved: {csv_path}")


# ---------------------------------------------------------------------------
# Demo 23: Python — Advance Appropriation Analysis from bill_meta.json
# ---------------------------------------------------------------------------

def demo_python_advance_analysis(bills, metas):
    print("\n" + "=" * 95)
    print("DEMO 23: Python — Advance vs. Current-Year Appropriation Analysis")
    print("  Shows: Use bill_meta.json to separate advance from current-year spending")
    print("=" * 95)

    print(f"""
  # ----- Python code (copy-paste ready) -----
  import json

  meta = json.load(open('data/119-hr5371/bill_meta.json'))
  ext  = json.load(open('data/119-hr5371/extraction.json'))

  current_total = 0
  advance_total = 0
  for pt in meta.get('provision_timing', []):
      idx = pt['provision_index']
      timing = pt['timing']
      p = ext['provisions'][idx]
      amt = p.get('amount', {{}})
      dollars = (amt.get('value', {{}}).get('dollars', 0)) or 0
      if timing == 'advance':
          advance_total += dollars
      elif timing == 'current_year':
          current_total += dollars

  print(f'Current-year: ${{current_total:,}}')
  print(f'Advance:      ${{advance_total:,}}')
  print(f'Advance %:    {{advance_total/(current_total+advance_total)*100:.1f}}%')
  # -------------------------------------------
""")

    # Run across all FY2026 bills
    print(f"  Advance vs. Current-Year split by bill (FY2026):")
    print(f"  {'Bill':<22s} {'Current ($)':>18s} {'Advance ($)':>18s} {'Adv %':>8s}")
    print(f"  {'-'*70}")

    grand_current = 0
    grand_advance = 0

    for bill_dir, meta in sorted(metas.items()):
        fys = meta.get("fiscal_years", [])
        if 2026 not in fys:
            continue
        ext_path = DATA_DIR / bill_dir / "extraction.json"
        if not ext_path.exists():
            continue
        ext = load_json(ext_path)
        provisions = ext.get("provisions", [])

        current = 0
        advance = 0
        for pt in meta.get("provision_timing", []):
            idx = pt["provision_index"]
            timing = pt["timing"]
            if idx >= len(provisions):
                continue
            p = provisions[idx]
            amt = p.get("amount", {})
            dollars = (amt.get("value") or {}).get("dollars", 0) or 0
            if timing == "advance":
                advance += dollars
            elif timing == "current_year":
                current += dollars

        if current + advance > 0:
            adv_pct = advance / (current + advance) * 100
            bill_id = ext.get("bill", {}).get("identifier", bill_dir)
            print(f"  {bill_id:<22s} {format_dollars(current):>18s} {format_dollars(advance):>18s} {adv_pct:>7.1f}%")
            grand_current += current
            grand_advance += advance

    if grand_current + grand_advance > 0:
        grand_pct = grand_advance / (grand_current + grand_advance) * 100
        print(f"  {'-'*70}")
        print(f"  {'TOTAL':<22s} {format_dollars(grand_current):>18s} {format_dollars(grand_advance):>18s} {grand_pct:>7.1f}%")
        print(f"""
  ⚠  {grand_pct:.0f}% of FY2026 BA is advance appropriations — money enacted now but
     available in FY2027+. Without separating this, year-over-year comparisons
     would be off by {format_dollars(grand_advance)}.
""")


# ---------------------------------------------------------------------------
# Demo 24: CLI + Python — Compare pipeline with inflation
# ---------------------------------------------------------------------------

def demo_cli_python_compare_pipeline():
    print("\n" + "=" * 95)
    print("DEMO 24: CLI + Python — Full Compare Pipeline with Export")
    print("  Shows: Use CLI to compare, export CSV, analyze in Python")
    print("=" * 95)

    print(f"""
  # ----- Shell: Run comparison and export -----
  congress-approp compare --base-fy 2024 --current-fy 2026 \\
      --subcommittee defense --dir data --use-authorities \\
      --format csv > tmp/demo_output/defense_compare.csv

  # ----- Python: Load and analyze -----
  import pandas as pd

  df = pd.read_csv('tmp/demo_output/defense_compare.csv')
  increased = df[df['delta'] > 0]
  decreased = df[df['delta'] < 0]
  print(f'Programs increased: {{len(increased)}}')
  print(f'Programs decreased: {{len(decreased)}}')
  print(f'Total increase: ${{increased["delta"].sum():,.0f}}')
  print(f'Total decrease: ${{decreased["delta"].sum():,.0f}}')
  # -------------------------------------------
""")

    csv_path = OUT_DIR / "defense_compare.csv"
    result = subprocess.run(
        ["cargo", "run", "--release", "--", "compare",
         "--base-fy", "2024", "--current-fy", "2026",
         "--subcommittee", "defense", "--dir", "data",
         "--use-authorities", "--format", "csv"],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        print(f"  Compare command failed: {result.stderr[:300]}")
        return

    with open(csv_path, "w") as f:
        f.write(result.stdout)

    try:
        import pandas as pd
        df = pd.read_csv(csv_path)
        print(f"  Exported {len(df)} rows to {csv_path}")
        print(f"  Columns: {list(df.columns)}")

        if "delta" in df.columns:
            increased = df[df["delta"] > 0]
            decreased = df[df["delta"] < 0]
            unchanged = df[df["delta"] == 0]

            print(f"\n  Defense FY2024 → FY2026 Summary:")
            print(f"    Accounts increased: {len(increased)}")
            print(f"    Accounts decreased: {len(decreased)}")
            print(f"    Accounts unchanged: {len(unchanged)}")
            if len(increased) > 0:
                print(f"    Total increase:     {format_dollars(increased['delta'].sum())}")
            if len(decreased) > 0:
                print(f"    Total decrease:     {format_dollars(decreased['delta'].sum())}")
            if "base_dollars" in df.columns and "current_dollars" in df.columns:
                total_base = df["base_dollars"].sum()
                total_current = df["current_dollars"].sum()
                if total_base > 0:
                    print(f"    Net change:         {format_dollars(total_current - total_base)} ({(total_current - total_base)/total_base*100:+.1f}%)")

            # Top 5 increases
            if len(increased) > 0:
                print(f"\n  Top 5 Defense increases:")
                top_inc = increased.nlargest(5, "delta")
                for _, row in top_inc.iterrows():
                    acct = str(row.get("account", row.get("account_name", "?")))[:45]
                    delta = row["delta"]
                    print(f"    {acct:<46s} {format_dollars(delta):>14s}")

            # Top 5 decreases
            if len(decreased) > 0:
                print(f"\n  Top 5 Defense decreases:")
                top_dec = decreased.nsmallest(5, "delta")
                for _, row in top_dec.iterrows():
                    acct = str(row.get("account", row.get("account_name", "?")))[:45]
                    delta = row["delta"]
                    print(f"    {acct:<46s} {format_dollars(delta):>14s}")
        else:
            print(f"  Note: 'delta' column not found. Available: {list(df.columns)}")

    except ImportError:
        print("  Skipping pandas analysis — not installed")


if __name__ == "__main__":
    main()
