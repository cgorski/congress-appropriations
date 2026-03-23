#!/usr/bin/env python3
"""
Quick demo: TAS-based cross-bill linking for DHS accounts only.

Instead of resolving entire omnibus bills (343+ provisions, 20+ batches),
this script:
1. Filters to DHS-related provisions only from two bills
2. Fetches DHS TAS codes once
3. Sends each bill's DHS provisions to Opus for TAS resolution
4. Shows which TAS codes appear in both bills = automatic cross-bill links

Usage:
    source /Users/chris.gorski/anthropic_key.source
    source .venv/bin/activate
    python scripts/tas_cross_link_demo.py
"""

import json
import os
import re
import sys
import time
from collections import defaultdict
from pathlib import Path

import anthropic
import requests

# ─── Config ───────────────────────────────────────────────────────────────────

MODEL = "claude-opus-4-6"
USASPENDING_BASE = "https://api.usaspending.gov/api/v2"
TAS_CACHE_DIR = Path("tmp/tas_cache")

# Bills to compare — one FY2020, one FY2024, both have DHS
BILL_A = "116-hr1158"   # FY2020 minibus (Defense, CJS, FinServ, Homeland)
BILL_B = "118-hr2882"   # FY2024 second omnibus (Defense, FinServ, Homeland, Labor-HHS, LegBranch, State)
# Can also try 116-hr133 (FY2021 omnibus)

# DHS agency keywords for filtering provisions
DHS_KEYWORDS = [
    "homeland security",
    "secret service",
    "customs and border",
    "immigration and customs",
    "coast guard",
    "transportation security",
    "federal emergency management",
    "fema",
    "cybersecurity and infrastructure",
    "cisa",
    "federal law enforcement training",
    "countering weapons",
    "science and technology directorate",
]

# ─── Helpers ──────────────────────────────────────────────────────────────────

def fetch_dhs_tas():
    """Fetch DHS TAS codes from USASpending, with caching."""
    cache_path = TAS_CACHE_DIR / "tas_070.json"
    if cache_path.exists():
        with open(cache_path) as f:
            return json.load(f)

    print("  Fetching DHS TAS codes from USASpending...")
    resp = requests.get(
        f"{USASPENDING_BASE}/references/filter_tree/tas/070/",
        timeout=30,
    )
    resp.raise_for_status()
    results = resp.json().get("results", [])

    TAS_CACHE_DIR.mkdir(parents=True, exist_ok=True)
    with open(cache_path, "w") as f:
        json.dump(results, f, indent=2)

    return results


def is_dhs_provision(provision):
    """Check if a provision is DHS-related based on agency name."""
    agency = (provision.get("agency") or "").lower()
    account = (provision.get("account_name") or "").lower()
    combined = f"{agency} {account}"
    return any(kw in combined for kw in DHS_KEYWORDS)


def get_dhs_top_level(bill_dir):
    """Load top-level BA appropriations that are DHS-related."""
    ext_path = os.path.join("data", bill_dir, "extraction.json")
    if not os.path.exists(ext_path):
        return [], ""

    with open(ext_path) as f:
        data = json.load(f)

    bill_id = data.get("bill", {}).get("identifier", bill_dir)
    provisions = data.get("provisions", [])

    results = []
    for i, p in enumerate(provisions):
        if p.get("provision_type") != "appropriation":
            continue
        amt = p.get("amount", {})
        if amt.get("semantics") != "new_budget_authority":
            continue
        dl = p.get("detail_level", "")
        if dl in ("sub_allocation", "proviso_amount"):
            continue
        if not p.get("account_name"):
            continue
        if not is_dhs_provision(p):
            continue

        results.append({
            "index": i,
            "account_name": p.get("account_name", ""),
            "agency": p.get("agency", ""),
            "dollars": amt.get("dollars", 0),
            "division": p.get("division"),
            "section": p.get("section", ""),
            "fiscal_year": p.get("fiscal_year"),
        })

    return results, bill_id


def get_xml_context(bill_dir, account_name, window=400):
    """Get XML context around an account name."""
    xml_files = list(Path("data", bill_dir).glob("BILLS-*.xml"))
    if not xml_files:
        return ""

    with open(xml_files[0]) as f:
        xml_text = f.read()

    # Search for account name
    search_variants = [
        f"\u2018\u2018{account_name}\u2019\u2019",
        f"''{account_name}''",
        account_name,
    ]

    pos = None
    for variant in search_variants:
        idx = xml_text.lower().find(variant.lower())
        if idx >= 0:
            pos = idx
            break

    if pos is None:
        return ""

    start = max(0, pos - window)
    end = min(len(xml_text), pos + len(account_name) + window)
    raw = xml_text[start:end]

    # Clean XML tags, preserve heading structure
    cleaned = re.sub(r"<appropriations-major[^>]*>", "\n[MAJOR] ", raw)
    cleaned = re.sub(r"</appropriations-major>", " [/MAJOR]", cleaned)
    cleaned = re.sub(r"<appropriations-intermediate[^>]*>", "\n  [SUBHEADING] ", cleaned)
    cleaned = re.sub(r"</appropriations-intermediate>", " [/SUBHEADING]", cleaned)
    cleaned = re.sub(r"<header[^>]*>", "", cleaned)
    cleaned = re.sub(r"</header>", "", cleaned)
    cleaned = re.sub(r"<[^>]+>", "", cleaned)
    cleaned = re.sub(r"\s+", " ", cleaned)
    return cleaned.strip()[:600]


# ─── TAS Resolution via LLM ──────────────────────────────────────────────────

TAS_SYSTEM_PROMPT = """You are an expert on U.S. federal budget accounts and Treasury Account Symbols (TAS).

You will receive a list of DHS appropriation provisions from a congressional bill, plus the full list of DHS TAS codes from USASpending.

Match each provision to the most likely TAS code.

Key rules:
- Account names in bills often use em-dash format: "Agency—Account Title". The TAS title inverts this: "Account Title, Agency, Department".
- "Salaries and Expenses" was renamed to "Operations and Support" for many DHS accounts around FY2017. Same TAS code.
- DHS sub-agencies (CBP, ICE, TSA, USCG, USSS, FEMA, CISA) each have their own TAS codes under agency prefix 070.
- Match to TAS codes in the 0000-3999 range. Codes 4000+ are revolving/trust funds.
- Confidence: "high" = clear match, "medium" = plausible but ambiguous, "none" = no match found.
- If you know the correct TAS code from your training but it's not in the provided list, still return it with a note.

Return JSON only (no markdown, no explanation):
{
  "mappings": [
    {
      "provision_index": 0,
      "account_name": "the account name",
      "tas_code": "070-0400" or null,
      "tas_title": "the TAS title" or null,
      "confidence": "high" | "medium" | "low" | "none",
      "reasoning": "brief explanation"
    }
  ]
}"""


def resolve_dhs_provisions(client, provisions, tas_codes, bill_id, bill_dir):
    """Send DHS provisions to Opus for TAS resolution."""
    # Build prompt
    parts = [f"Bill: {bill_id}\n\n"]
    parts.append("== DHS PROVISIONS TO MATCH ==\n\n")

    for i, p in enumerate(provisions):
        dollars = p.get("dollars", 0) or 0
        d_str = f"${dollars:,}" if dollars else "no amount"
        ctx = get_xml_context(bill_dir, p["account_name"])
        ctx_snippet = f"\n    XML: {ctx[:200]}..." if ctx else ""

        parts.append(
            f"  [{i}] idx={p['index']} account=\"{p['account_name']}\" "
            f"agency=\"{p.get('agency', '')}\" {d_str} "
            f"div={p.get('division', '?')} fy={p.get('fiscal_year', '?')}"
            f"{ctx_snippet}\n"
        )

    parts.append("\n== DHS TAS CODES (agency 070) ==\n\n")
    for tas in tas_codes:
        code = tas.get("id", "")
        # Skip revolving/trust funds
        m = re.search(r"-(\d{4})", code)
        if m and int(m.group(1)) >= 4000:
            continue
        parts.append(f"  {tas['id']}: {tas['description']}\n")

    prompt = "".join(parts)
    print(f"    Prompt: {len(prompt)} chars, {len(provisions)} provisions")

    # Send to Opus with thinking
    response = client.messages.create(
        model=MODEL,
        max_tokens=16000,
        temperature=1,
        thinking={
            "type": "enabled",
            "budget_tokens": 10000,
        },
        system=TAS_SYSTEM_PROMPT,
        messages=[{"role": "user", "content": prompt}],
    )

    # Parse response
    response_text = ""
    thinking_text = ""
    for block in response.content:
        if block.type == "thinking":
            thinking_text = block.thinking
        elif block.type == "text":
            response_text = block.text

    print(f"    Thinking: {len(thinking_text)} chars")
    print(f"    Tokens: {response.usage.input_tokens} in, {response.usage.output_tokens} out")

    # Parse JSON
    try:
        json_text = response_text.strip()
        if json_text.startswith("```"):
            json_text = re.sub(r"^```\w*\n?", "", json_text)
            json_text = re.sub(r"\n?```$", "", json_text)
        result = json.loads(json_text)
        mappings = result.get("mappings", [])
        print(f"    Got {len(mappings)} mappings")
        return mappings
    except json.JSONDecodeError as e:
        print(f"    ERROR parsing response: {e}")
        print(f"    Response: {response_text[:300]}")
        return []


# ─── Main ─────────────────────────────────────────────────────────────────────

def main():
    os.chdir(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

    print()
    print("╔══════════════════════════════════════════════════════════════════╗")
    print("║  TAS CROSS-BILL LINKING DEMO — DHS Accounts Only               ║")
    print("╚══════════════════════════════════════════════════════════════════╝")
    print()

    # Step 1: Load DHS provisions from both bills
    print(f"=== Loading DHS provisions ===\n")

    provs_a, bill_id_a = get_dhs_top_level(BILL_A)
    print(f"  {BILL_A} ({bill_id_a}): {len(provs_a)} DHS top-level BA provisions")

    provs_b, bill_id_b = get_dhs_top_level(BILL_B)
    print(f"  {BILL_B} ({bill_id_b}): {len(provs_b)} DHS top-level BA provisions")
    print()

    if not provs_a or not provs_b:
        print("ERROR: Need extractions for both bills. Check data directory.")
        sys.exit(1)

    # Step 2: Fetch DHS TAS codes
    print(f"=== Fetching DHS TAS codes ===\n")
    dhs_tas = fetch_dhs_tas()
    approp_tas = [t for t in dhs_tas if re.search(r"-(\d{4})", t["id"]) and int(re.search(r"-(\d{4})", t["id"]).group(1)) < 4000]
    print(f"  Total DHS TAS codes: {len(dhs_tas)}")
    print(f"  Appropriation accounts (0-3999): {len(approp_tas)}")
    print()

    # Step 3: Resolve TAS for each bill
    client = anthropic.Anthropic()

    print(f"=== Resolving TAS for {BILL_A} ({bill_id_a}) ===\n")
    # Batch if needed (40 per batch)
    mappings_a = []
    for batch_start in range(0, len(provs_a), 40):
        batch = provs_a[batch_start:batch_start + 40]
        print(f"  Batch {batch_start // 40 + 1}: provisions {batch_start + 1}-{batch_start + len(batch)}")
        result = resolve_dhs_provisions(client, batch, dhs_tas, bill_id_a, BILL_A)
        mappings_a.extend(result)
    print()

    print(f"=== Resolving TAS for {BILL_B} ({bill_id_b}) ===\n")
    mappings_b = []
    for batch_start in range(0, len(provs_b), 40):
        batch = provs_b[batch_start:batch_start + 40]
        print(f"  Batch {batch_start // 40 + 1}: provisions {batch_start + 1}-{batch_start + len(batch)}")
        result = resolve_dhs_provisions(client, batch, dhs_tas, bill_id_b, BILL_B)
        mappings_b.extend(result)
    print()

    # Step 4: Analyze matches
    print("=" * 75)
    print("  INDIVIDUAL BILL RESULTS")
    print("=" * 75)
    print()

    for label, mappings in [(f"{BILL_A} ({bill_id_a})", mappings_a), (f"{BILL_B} ({bill_id_b})", mappings_b)]:
        high = sum(1 for m in mappings if m.get("confidence") == "high")
        med = sum(1 for m in mappings if m.get("confidence") == "medium")
        low = sum(1 for m in mappings if m.get("confidence") == "low")
        none = sum(1 for m in mappings if m.get("confidence") == "none" or not m.get("tas_code"))
        total = len(mappings)
        unique_tas = len(set(m["tas_code"] for m in mappings if m.get("tas_code")))

        print(f"  {label}:")
        print(f"    Total: {total}, High: {high}, Medium: {med}, Low: {low}, Unmatched: {none}")
        print(f"    Unique TAS codes: {unique_tas}")
        print(f"    Match rate: {(total - none) / max(1, total) * 100:.1f}%")
        print()

    # Step 5: Cross-bill linking!
    print("=" * 75)
    print("  CROSS-BILL LINKING — TAS codes that appear in BOTH bills")
    print("=" * 75)
    print()

    # Build TAS -> provisions mapping for each bill
    tas_a = defaultdict(list)
    for m in mappings_a:
        if m.get("tas_code"):
            tas_a[m["tas_code"]].append(m)

    tas_b = defaultdict(list)
    for m in mappings_b:
        if m.get("tas_code"):
            tas_b[m["tas_code"]].append(m)

    # Find shared TAS codes
    shared_tas = set(tas_a.keys()) & set(tas_b.keys())
    only_a = set(tas_a.keys()) - set(tas_b.keys())
    only_b = set(tas_b.keys()) - set(tas_a.keys())

    print(f"  TAS codes in {BILL_A}: {len(tas_a)}")
    print(f"  TAS codes in {BILL_B}: {len(tas_b)}")
    print(f"  Shared (= automatic links): {len(shared_tas)}")
    print(f"  Only in {BILL_A}: {len(only_a)}")
    print(f"  Only in {BILL_B}: {len(only_b)}")
    print()

    if shared_tas:
        print("  ┌─────────────┬────────────────────────────────────────────────────┐")
        print("  │ TAS Code    │ Accounts Linked                                    │")
        print("  ├─────────────┼────────────────────────────────────────────────────┤")

        for tas_code in sorted(shared_tas):
            entries_a = tas_a[tas_code]
            entries_b = tas_b[tas_code]
            acct_a = entries_a[0].get("account_name", "")[:45]
            acct_b = entries_b[0].get("account_name", "")[:45]
            title = entries_a[0].get("tas_title", "")[:45]

            print(f"  │ {tas_code:11s} │ {title:50s} │")
            print(f"  │             │   FY2020: \"{acct_a}\"")
            print(f"  │             │   FY2024: \"{acct_b}\"")
            print(f"  │             │")

        print("  └─────────────┴────────────────────────────────────────────────────┘")
        print()

    # Show name differences (evidence of renaming)
    rename_candidates = []
    for tas_code in sorted(shared_tas):
        names_a = set(m["account_name"] for m in tas_a[tas_code])
        names_b = set(m["account_name"] for m in tas_b[tas_code])
        if names_a != names_b:
            rename_candidates.append((tas_code, names_a, names_b))

    if rename_candidates:
        print("  ⚠ NAME DIFFERENCES detected across bills (same TAS, different name):")
        print()
        for tas_code, names_a, names_b in rename_candidates:
            print(f"    TAS {tas_code}:")
            for n in names_a:
                print(f"      {BILL_A}: \"{n}\"")
            for n in names_b:
                print(f"      {BILL_B}: \"{n}\"")
            print()

    # Show only-in-one-bill accounts (potential new/eliminated accounts)
    if only_a:
        print(f"  Accounts only in {BILL_A} (FY2020):")
        for tas_code in sorted(only_a):
            entries = tas_a[tas_code]
            acct = entries[0].get("account_name", "")[:55]
            conf = entries[0].get("confidence", "?")
            print(f"    {tas_code}: \"{acct}\" [{conf}]")
        print()

    if only_b:
        print(f"  Accounts only in {BILL_B} (FY2024):")
        for tas_code in sorted(only_b):
            entries = tas_b[tas_code]
            acct = entries[0].get("account_name", "")[:55]
            conf = entries[0].get("confidence", "?")
            print(f"    {tas_code}: \"{acct}\" [{conf}]")
        print()

    # Save results
    output = {
        "schema_version": "1.0",
        "description": "DHS cross-bill TAS linking demo",
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "model": MODEL,
        "bill_a": {
            "dir": BILL_A,
            "identifier": bill_id_a,
            "dhs_provisions": len(provs_a),
            "mappings": mappings_a,
        },
        "bill_b": {
            "dir": BILL_B,
            "identifier": bill_id_b,
            "dhs_provisions": len(provs_b),
            "mappings": mappings_b,
        },
        "cross_links": {
            "shared_tas_codes": sorted(shared_tas),
            "only_in_a": sorted(only_a),
            "only_in_b": sorted(only_b),
            "name_differences": [
                {
                    "tas_code": tc,
                    "names_a": sorted(na),
                    "names_b": sorted(nb),
                }
                for tc, na, nb in rename_candidates
            ],
        },
        "summary": {
            "total_shared": len(shared_tas),
            "total_only_a": len(only_a),
            "total_only_b": len(only_b),
            "total_renames": len(rename_candidates),
        },
    }

    output_path = "tmp/dhs_cross_link_demo.json"
    os.makedirs("tmp", exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(output, f, indent=2)
    print(f"  Full results saved to: {output_path}")

    # Final summary
    print()
    print("=" * 75)
    print("  FINDINGS")
    print("=" * 75)
    print()
    print(f"  1. TAS resolution identified {len(shared_tas)} DHS accounts that appear")
    print(f"     in BOTH {BILL_A} (FY2020) and {BILL_B} (FY2024).")
    print()
    print("  2. These links were found by TAS code alone — no embeddings,")
    print("     no similarity thresholds, no manual review needed.")
    print()
    if rename_candidates:
        print(f"  3. {len(rename_candidates)} account(s) have different names across bills")
        print("     but the SAME TAS code — confirmed renames that embedding")
        print("     similarity might miss.")
        print()
    print(f"  4. {len(only_a)} account(s) only in FY2020, {len(only_b)} only in FY2024.")
    print("     These may be new accounts, eliminated accounts, or TAS matching gaps.")
    print()
    print("  5. This demonstrates the authority-based approach: TAS codes provide")
    print("     stable identity that persists through account renames, making")
    print("     cross-bill linking a simple GROUP BY operation.")
    print()


if __name__ == "__main__":
    main()
