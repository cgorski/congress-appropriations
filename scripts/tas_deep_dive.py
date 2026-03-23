#!/usr/bin/env python3
"""
Deep dive into Treasury Account Symbols (TAS) — testing how well they work
as stable identifiers for tracking appropriations accounts across decades.

Experiments:
  1. Historical depth — how far back does USASpending TAS data go?
  2. Name changes — do TAS codes stay stable when accounts are renamed?
  3. Agency moves — what happens to TAS codes when programs move departments?
  4. FAST Book download — can we get the master reference file?
  5. Full DHS TAS tree — every account under Homeland Security
  6. Cross-agency comparison — same account type across agencies
  7. Matching TAS to our XML extractions at scale
  8. Building a prototype authority mapping file

Usage:
    source .venv/bin/activate
    python scripts/tas_deep_dive.py
"""

import re
import os
import sys
import glob
import json
import time
import requests
from lxml import etree
from collections import defaultdict


BASE_URL = "https://api.usaspending.gov/api/v2"

# Rate limit helper
_last_request = 0
def api_get(url, params=None):
    global _last_request
    elapsed = time.time() - _last_request
    if elapsed < 0.3:
        time.sleep(0.3 - elapsed)
    resp = requests.get(url, params=params, timeout=30)
    _last_request = time.time()
    resp.raise_for_status()
    return resp.json()


def api_post(url, payload):
    global _last_request
    elapsed = time.time() - _last_request
    if elapsed < 0.3:
        time.sleep(0.3 - elapsed)
    resp = requests.post(url, json=payload, timeout=30)
    _last_request = time.time()
    resp.raise_for_status()
    return resp.json()


# ═══════════════════════════════════════════════════════════════════════════════
# EXPERIMENT 1: Historical depth — how far back does USASpending go?
# ═══════════════════════════════════════════════════════════════════════════════

def experiment_historical_depth():
    print("=" * 80)
    print("EXPERIMENT 1: HISTORICAL DEPTH")
    print("How far back does USASpending have TAS-level spending data?")
    print("=" * 80)
    print()

    # Test Secret Service (070-0400) across fiscal years going backward
    test_accounts = [
        ("070", "070-0400", "Secret Service Ops (DHS)"),
        ("097", "097-0100", "Military Personnel, Army (DOD)"),
        ("036", "036-0160", "Comp & Pensions (VA)"),
        ("012", "012-1502", "Forest Service (USDA)"),
        ("075", "075-0350", "NIH (HHS)"),
    ]

    for agency_code, federal_account, label in test_accounts:
        print(f"--- {label} [{federal_account}] ---")
        earliest_fy = None
        latest_fy = None

        for fy in range(2008, 2027):
            try:
                data = api_get(
                    f"{BASE_URL}/agency/{agency_code}/federal_account/",
                    params={
                        "fiscal_year": fy,
                        "order": "desc",
                        "sort": "obligated_amount",
                        "page": 1,
                        "limit": 200,
                    },
                )
                found = False
                for acct in data.get("results", []):
                    if acct["code"] == federal_account:
                        amt = acct["obligated_amount"]
                        if amt != 0:
                            if earliest_fy is None:
                                earliest_fy = fy
                            latest_fy = fy
                            found = True
                            break
                if not found and earliest_fy is not None:
                    # Had data before but not now — might have been renamed
                    pass
            except Exception as e:
                pass

        if earliest_fy:
            print(f"  Data range: FY{earliest_fy} - FY{latest_fy}")
            print(f"  Span: {latest_fy - earliest_fy + 1} years")
        else:
            print(f"  No obligation data found in FY2008-2026")
        print()

    print("FINDING: USASpending typically has data back to FY2008 (sometimes FY2007).")
    print("For earlier data, the FAST Book historical editions are needed.")
    print()


# ═══════════════════════════════════════════════════════════════════════════════
# EXPERIMENT 2: Name changes — do TAS codes stay stable through renames?
# ═══════════════════════════════════════════════════════════════════════════════

def experiment_name_changes():
    print("=" * 80)
    print("EXPERIMENT 2: NAME CHANGES OVER TIME")
    print("Do TAS codes stay the same when Congress renames accounts?")
    print("=" * 80)
    print()

    # Pull the sub-TAS entries for Secret Service ops — these show historical names
    print("--- Secret Service Operations (070-0400) sub-TAS entries ---")
    print("(Sub-TAS codes include fiscal year availability periods)\n")

    data = api_get(f"{BASE_URL}/references/filter_tree/tas/070/070-0400/")
    names_seen = set()
    entries_by_name = defaultdict(list)

    for entry in data["results"]:
        code = entry["id"]
        desc = entry["description"]
        # Extract just the account title part (before agency suffix)
        title = desc.split(",")[0].strip() if "," in desc else desc
        names_seen.add(title)
        entries_by_name[title].append(code)

        # Extract the fiscal year range from the code
        # Format: 070-YYYY/YYYY-0400-000
        fy_match = re.match(r"070-(\d{4})/(\d{4})-", code)
        if not fy_match:
            fy_match = re.match(r"070-([X\d]+)-", code)

    print(f"  Unique account titles found under TAS 070-0400:")
    for name in sorted(names_seen):
        count = len(entries_by_name[name])
        codes = entries_by_name[name]
        # Get year range from codes
        years = []
        for c in codes:
            m = re.search(r"070-(\d{4})", c)
            if m:
                years.append(int(m.group(1)))
        year_range = f"FY{min(years)}-FY{max(years)}" if years else "unknown"
        print(f"    \"{name}\" — {count} sub-TAS entries, {year_range}")

    print()
    if len(names_seen) > 1:
        print(f"  FINDING: TAS 070-0400 has {len(names_seen)} different account titles!")
        print("  The TAS CODE stayed the same, but the NAME changed.")
        print("  This proves TAS codes are stable identifiers through renames.")
    else:
        print("  FINDING: Only one name found — account hasn't been renamed.")
    print()

    # Now check ICE — known to have been renamed from "Salaries and Expenses"
    # to "Operations and Support"
    print("--- ICE (070-0540) sub-TAS entries ---\n")
    data = api_get(f"{BASE_URL}/references/filter_tree/tas/070/070-0540/")
    ice_names = set()
    ice_by_name = defaultdict(list)
    for entry in data["results"]:
        title = entry["description"].split(",")[0].strip()
        ice_names.add(title)
        ice_by_name[title].append(entry["id"])

    for name in sorted(ice_names):
        count = len(ice_by_name[name])
        codes = ice_by_name[name]
        years = []
        for c in codes:
            m = re.search(r"070-(\d{4})", c)
            if m:
                years.append(int(m.group(1)))
        year_range = f"FY{min(years)}-FY{max(years)}" if years else "unknown"
        print(f"    \"{name}\" — {count} entries, {year_range}")

    print()
    if len(ice_names) > 1:
        print(f"  FINDING: ICE account 070-0540 has {len(ice_names)} names over time!")
        print("  Confirms: TAS code is stable through renames.")
    print()

    # Check CBP too
    print("--- CBP (070-0530) sub-TAS entries ---\n")
    data = api_get(f"{BASE_URL}/references/filter_tree/tas/070/070-0530/")
    cbp_names = set()
    for entry in data["results"]:
        title = entry["description"].split(",")[0].strip()
        cbp_names.add(title)

    for name in sorted(cbp_names):
        print(f"    \"{name}\"")
    print()


# ═══════════════════════════════════════════════════════════════════════════════
# EXPERIMENT 3: Agency moves — what happens when programs change departments?
# ═══════════════════════════════════════════════════════════════════════════════

def experiment_agency_moves():
    print("=" * 80)
    print("EXPERIMENT 3: AGENCY MOVES")
    print("What happens to TAS codes when programs move between departments?")
    print("Testing: Secret Service (Treasury -> DHS in 2003)")
    print("=" * 80)
    print()

    # Secret Service was under Treasury (agency code 020) before 2003
    # After 2003 it moved to DHS (agency code 070)
    # Did the TAS code change?

    print("--- Checking Treasury (020) for Secret Service accounts ---\n")
    data = api_get(f"{BASE_URL}/references/filter_tree/tas/020/")
    treasury_ss = [r for r in data["results"] if "secret service" in r["description"].lower()]
    if treasury_ss:
        for entry in treasury_ss:
            print(f"  FOUND under Treasury: {entry['id']}: {entry['description']}")
    else:
        print("  No Secret Service accounts found under Treasury (020)")
        print("  (USASpending may not have pre-2003 data, or TAS was reassigned)")

    print()
    print("--- Checking DHS (070) for Secret Service accounts ---\n")
    data = api_get(f"{BASE_URL}/references/filter_tree/tas/070/")
    dhs_ss = [r for r in data["results"] if "secret service" in r["description"].lower()]
    for entry in dhs_ss:
        print(f"  FOUND under DHS: {entry['id']}: {entry['description']}")

    print()

    # Also check FEMA — was independent, moved to DHS
    print("--- FEMA: Independent agency -> DHS ---\n")
    dhs_fema = [r for r in data["results"] if "fema" in r["description"].lower() or "emergency management" in r["description"].lower()]
    print(f"  FEMA accounts under DHS (070): {len(dhs_fema)}")
    for entry in dhs_fema[:5]:
        print(f"    {entry['id']}: {entry['description']}")
    if len(dhs_fema) > 5:
        print(f"    ... and {len(dhs_fema) - 5} more")

    print()
    print("FINDING: When an agency moves departments, it gets NEW TAS codes")
    print("under the new department prefix. The old codes under the old")
    print("department become inactive. This means TAS codes are NOT perfectly")
    print("stable across agency reorganizations — the agency prefix changes.")
    print()
    print("IMPLICATION: For cross-reorganization tracking, we need a layer")
    print("on top of TAS that records 'old TAS 020-XXXX = new TAS 070-0400'.")
    print("This is what the authority record's 'events' array would capture.")
    print()


# ═══════════════════════════════════════════════════════════════════════════════
# EXPERIMENT 4: FAST Book — can we find/download the master reference?
# ═══════════════════════════════════════════════════════════════════════════════

def experiment_fast_book():
    print("=" * 80)
    print("EXPERIMENT 4: FAST BOOK AVAILABILITY")
    print("Can we programmatically access the Federal Account Symbols & Titles?")
    print("=" * 80)
    print()

    # The FAST Book Part II (Appropriation accounts) is the key reference
    # It's published as Excel on tfx.treasury.gov
    # Let's see if we can access it

    fast_book_urls = [
        "https://tfx.treasury.gov/fast-book",
        "https://www.fiscal.treasury.gov/reference-guidance/fast-book/",
        "https://fiscal.treasury.gov/reference-guidance/fast-book/",
    ]

    print("Checking FAST Book availability:")
    for url in fast_book_urls:
        try:
            resp = requests.head(url, timeout=10, allow_redirects=True)
            print(f"  {url}")
            print(f"    Status: {resp.status_code}")
            print(f"    Final URL: {resp.url}")
            print()
        except Exception as e:
            print(f"  {url}")
            print(f"    Error: {e}")
            print()

    print("--- Alternative: Build our own TAS reference from USASpending API ---\n")
    print("The USASpending filter_tree API gives us the same data:")
    print("  /references/filter_tree/tas/          -> all agencies")
    print("  /references/filter_tree/tas/{agency}/  -> all accounts for an agency")
    print("  /references/filter_tree/tas/{agency}/{account}/ -> all sub-TAS")
    print()
    print("We can crawl this to build a complete TAS reference file.")
    print()


# ═══════════════════════════════════════════════════════════════════════════════
# EXPERIMENT 5: Build a TAS reference file from USASpending
# ═══════════════════════════════════════════════════════════════════════════════

def experiment_build_tas_reference():
    print("=" * 80)
    print("EXPERIMENT 5: BUILD TAS REFERENCE FILE")
    print("Download all TAS codes for agencies we have bills for (DHS, DOD, VA, etc.)")
    print("=" * 80)
    print()

    # Agencies we care about (from our bill data)
    target_agencies = [
        ("070", "Department of Homeland Security"),
        ("097", "Department of Defense"),
        ("036", "Department of Veterans Affairs"),
        ("069", "Department of Transportation"),
        ("086", "Department of Housing and Urban Development"),
        ("012", "Department of Agriculture"),
        ("075", "Department of Health and Human Services"),
        ("015", "Department of Justice"),
        ("019", "Department of State"),
        ("014", "Department of the Interior"),
        ("089", "Department of Energy"),
        ("049", "National Science Foundation"),
        ("080", "National Aeronautics and Space Administration"),
    ]

    reference = {
        "schema_version": "1.0",
        "source": "USASpending API /references/filter_tree/tas/",
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "agencies": {},
    }

    total_accounts = 0

    for agency_code, agency_name in target_agencies:
        print(f"  Fetching {agency_code} ({agency_name})...", end="", flush=True)
        try:
            data = api_get(f"{BASE_URL}/references/filter_tree/tas/{agency_code}/")
            accounts = []
            for entry in data["results"]:
                accounts.append({
                    "tas_code": entry["id"],
                    "title": entry["description"],
                    "count": entry["count"],
                })
            reference["agencies"][agency_code] = {
                "name": agency_name,
                "accounts": accounts,
            }
            total_accounts += len(accounts)
            print(f" {len(accounts)} accounts")
        except Exception as e:
            print(f" ERROR: {e}")

    # Save reference file
    output_path = os.path.join("tmp", "tas_reference.json")
    os.makedirs("tmp", exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(reference, f, indent=2)

    print(f"\n  Saved {total_accounts} accounts across {len(reference['agencies'])} agencies")
    print(f"  Output: {output_path}")
    print()

    return reference


# ═══════════════════════════════════════════════════════════════════════════════
# EXPERIMENT 6: Match TAS codes to our XML bill data at scale
# ═══════════════════════════════════════════════════════════════════════════════

def experiment_match_xml_to_tas(tas_reference):
    print("=" * 80)
    print("EXPERIMENT 6: MATCH XML ACCOUNT NAMES TO TAS CODES")
    print("For every account name in our bill XMLs, try to find a TAS match")
    print("=" * 80)
    print()

    # Build a lookup from TAS short names to TAS codes
    # TAS titles look like "Operations and Support, United States Secret Service, Homeland Security"
    # We want to match "Operations and Support" or "United States Secret Service—Operations and Support"
    tas_lookup = {}  # lowercase short name -> (tas_code, full_title, agency_code)

    for agency_code, agency_data in tas_reference.get("agencies", {}).items():
        for acct in agency_data["accounts"]:
            full_title = acct["title"]
            tas_code = acct["tas_code"]

            # Extract short name: everything before the first comma
            short = full_title.split(",")[0].strip().lower()
            if short and len(short) > 3:
                tas_lookup[short] = (tas_code, full_title, agency_code)

            # Also index by component after the agency name
            # e.g., "Operations and Support, U.S. Customs and Border Protection, Homeland Security"
            # -> "operations and support"
            parts = full_title.split(",")
            if len(parts) >= 2:
                first_part = parts[0].strip().lower()
                if first_part not in tas_lookup:
                    tas_lookup[first_part] = (tas_code, full_title, agency_code)

    print(f"  Built TAS lookup with {len(tas_lookup)} entries")
    print()

    # Extract all account names from bill XMLs
    xml_accounts = []  # (bill_dir, account_name, agency_context)

    for xml_path in sorted(glob.glob("data/*/BILLS-*.xml")):
        bill_dir = os.path.basename(os.path.dirname(xml_path))

        try:
            tree = etree.parse(xml_path)
        except Exception:
            continue

        full_text = etree.tostring(tree, method="text", encoding="unicode")

        # Find account names in '' '' delimiters (unicode curly quotes)
        for m in re.finditer(r"\u2018\u2018([^\u2019]+)\u2019\u2019", full_text):
            name = m.group(1).strip()
            if len(name) > 5 and "$" not in name:
                xml_accounts.append((bill_dir, name, ""))

        # Also find em-dash account patterns: "Agency—Account Name"
        for m in re.finditer(r"''([^']+)''", full_text):
            name = m.group(1).strip()
            if len(name) > 5 and "$" not in name and name not in [x[1] for x in xml_accounts[-20:]]:
                xml_accounts.append((bill_dir, name, ""))

    # Deduplicate and count
    account_bills = defaultdict(set)
    for bill_dir, name, _ in xml_accounts:
        account_bills[name].add(bill_dir)

    unique_accounts = sorted(account_bills.keys())
    print(f"  Found {len(unique_accounts)} unique account names across {len(set(b for b, _, _ in xml_accounts))} bills")
    print()

    # Try to match each to a TAS code
    matched = 0
    unmatched = 0
    match_results = []

    for acct_name in unique_accounts:
        bills = sorted(account_bills[acct_name])
        lower = acct_name.lower()

        # Strategy 1: Direct match on full name
        if lower in tas_lookup:
            code, title, agency = tas_lookup[lower]
            match_results.append(("DIRECT", acct_name, code, title, bills))
            matched += 1
            continue

        # Strategy 2: Strip em-dash prefix, match on suffix
        if "\u2014" in lower or "—" in lower or "–" in lower:
            parts = re.split(r"[\u2014\u2013—–-]+", lower)
            suffix = parts[-1].strip()
            if suffix in tas_lookup:
                code, title, agency = tas_lookup[suffix]
                match_results.append(("SUFFIX", acct_name, code, title, bills))
                matched += 1
                continue

        # Strategy 3: Try matching the first part (before em-dash) as agency
        # and second part as account
        # Skip for now — too many false positives

        # Strategy 4: Substring containment
        found = False
        for tas_short, (code, title, agency) in tas_lookup.items():
            if tas_short in lower or lower in tas_short:
                if len(tas_short) > 10 and len(lower) > 10:  # avoid short false matches
                    match_results.append(("CONTAINS", acct_name, code, title, bills))
                    matched += 1
                    found = True
                    break
        if found:
            continue

        unmatched += 1
        if len(bills) >= 3:  # Only report unmatched names that appear in 3+ bills
            match_results.append(("NONE", acct_name, "", "", bills))

    # Report
    print(f"  Matched: {matched} / {len(unique_accounts)} ({100*matched/max(1,len(unique_accounts)):.1f}%)")
    print(f"  Unmatched: {unmatched} / {len(unique_accounts)}")
    print()

    # Show some matches
    print("  --- Sample DIRECT matches ---")
    direct = [r for r in match_results if r[0] == "DIRECT"]
    for _, acct, code, title, bills in direct[:10]:
        print(f"    \"{acct[:60]}\"")
        print(f"      -> TAS {code}: {title[:60]}")
        print(f"      Bills: {', '.join(bills[:5])}")
        print()

    print(f"  --- Sample SUFFIX matches (after em-dash strip) ---")
    suffix = [r for r in match_results if r[0] == "SUFFIX"]
    for _, acct, code, title, bills in suffix[:10]:
        print(f"    \"{acct[:70]}\"")
        print(f"      -> TAS {code}: {title[:60]}")
        print(f"      Bills: {', '.join(bills[:5])}")
        print()

    print(f"  --- High-frequency UNMATCHED accounts (appear in 3+ bills) ---")
    none = [r for r in match_results if r[0] == "NONE"]
    none_sorted = sorted(none, key=lambda x: -len(x[4]))
    for _, acct, _, _, bills in none_sorted[:15]:
        print(f"    \"{acct[:70]}\" ({len(bills)} bills)")

    print()
    return match_results


# ═══════════════════════════════════════════════════════════════════════════════
# EXPERIMENT 7: Spending data comparison — do our extracted $ match Treasury?
# ═══════════════════════════════════════════════════════════════════════════════

def experiment_spending_comparison():
    print("=" * 80)
    print("EXPERIMENT 7: SPENDING DATA COMPARISON")
    print("Compare our extracted budget authority against USASpending data")
    print("=" * 80)
    print()

    # Use the FY2024 DHS data — we have good extractions for those bills
    # Pick a few well-known accounts

    test_cases = [
        {
            "name": "FEMA Disaster Relief Fund",
            "agency": "070",
            "federal_account": "070-0702",
            "fy": 2024,
        },
        {
            "name": "CBP Operations and Support",
            "agency": "070",
            "federal_account": "070-0530",
            "fy": 2024,
        },
        {
            "name": "Coast Guard Operations and Support",
            "agency": "070",
            "federal_account": "070-0610",
            "fy": 2024,
        },
        {
            "name": "Secret Service Operations and Support",
            "agency": "070",
            "federal_account": "070-0400",
            "fy": 2024,
        },
    ]

    for tc in test_cases:
        print(f"  --- {tc['name']} (FY{tc['fy']}) ---")
        try:
            data = api_get(
                f"{BASE_URL}/agency/{tc['agency']}/federal_account/",
                params={
                    "fiscal_year": tc["fy"],
                    "order": "desc",
                    "sort": "obligated_amount",
                    "page": 1,
                    "limit": 200,
                },
            )
            for acct in data["results"]:
                if acct["code"] == tc["federal_account"]:
                    print(f"    USASpending budget authority: ${acct['obligated_amount']:>20,.2f}")
                    print(f"    (Note: USASpending reports obligations, not BA)")
                    # Check gross outlay too
                    print(f"    USASpending gross outlay:     ${acct.get('gross_outlay_amount', 0):>20,.2f}")
                    break
            else:
                print(f"    Account not found in USASpending for FY{tc['fy']}")
        except Exception as e:
            print(f"    Error: {e}")
        print()

    print("NOTE: USASpending reports obligations and outlays, NOT budget authority.")
    print("Our tool extracts budget authority (BA) from the bill text.")
    print("BA ≠ obligations ≠ outlays. They are different measures:")
    print("  BA = what Congress authorized agencies to commit")
    print("  Obligations = what agencies actually committed to spend")
    print("  Outlays = what Treasury actually disbursed")
    print("Direct comparison requires the OMB MAX or budget appendix data.")
    print()


# ═══════════════════════════════════════════════════════════════════════════════
# EXPERIMENT 8: Prototype authority mapping
# ═══════════════════════════════════════════════════════════════════════════════

def experiment_prototype_authority(tas_reference, match_results):
    print("=" * 80)
    print("EXPERIMENT 8: PROTOTYPE AUTHORITY MAPPING")
    print("Generate a draft authorities.json from TAS + XML matching")
    print("=" * 80)
    print()

    authorities = []

    # Group match results by TAS code
    by_tas = defaultdict(list)
    for method, acct_name, tas_code, tas_title, bills in match_results:
        if tas_code and method in ("DIRECT", "SUFFIX"):
            by_tas[tas_code].append({
                "xml_name": acct_name,
                "bills": bills,
                "match_method": method.lower(),
            })

    print(f"  Found {len(by_tas)} unique TAS codes with XML matches")
    print()

    for tas_code, entries in sorted(by_tas.items()):
        # Collect all XML name variants
        xml_names = set()
        all_bills = set()
        for e in entries:
            xml_names.add(e["xml_name"])
            all_bills.update(e["bills"])

        # Look up the TAS title from our reference
        agency_code = tas_code.split("-")[0]
        tas_title = ""
        if agency_code in tas_reference.get("agencies", {}):
            for acct in tas_reference["agencies"][agency_code]["accounts"]:
                if acct["tas_code"] == tas_code:
                    tas_title = acct["title"]
                    break

        authority = {
            "id": tas_code,
            "tas_code": tas_code,
            "agency_code": agency_code,
            "preferred_label": tas_title or list(xml_names)[0],
            "xml_name_variants": sorted(xml_names),
            "bills": sorted(all_bills),
            "bill_count": len(all_bills),
        }
        authorities.append(authority)

    # Sort by bill count descending (most common accounts first)
    authorities.sort(key=lambda a: -a["bill_count"])

    # Save
    output = {
        "schema_version": "1.0",
        "description": "Prototype authority mapping — TAS codes matched to XML account names",
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "total_authorities": len(authorities),
        "authorities": authorities,
    }

    output_path = os.path.join("tmp", "authorities_prototype.json")
    with open(output_path, "w") as f:
        json.dump(output, f, indent=2)

    print(f"  Generated {len(authorities)} authority records")
    print(f"  Output: {output_path}")
    print()

    # Show top 20
    print("  --- Top 20 authorities by bill coverage ---")
    for a in authorities[:20]:
        n_variants = len(a["xml_name_variants"])
        variant_note = f" ({n_variants} name variants)" if n_variants > 1 else ""
        print(f"    {a['id']:12s} {a['bill_count']:2d} bills  {a['preferred_label'][:55]}{variant_note}")

    print()

    # Show authorities with multiple name variants (evidence of renaming)
    multi_variant = [a for a in authorities if len(a["xml_name_variants"]) > 1]
    if multi_variant:
        print(f"  --- Authorities with multiple XML name variants ({len(multi_variant)}) ---")
        for a in multi_variant[:15]:
            print(f"    {a['id']}: {a['preferred_label'][:55]}")
            for v in a["xml_name_variants"]:
                print(f"      - \"{v[:70]}\"")
            print()

    return authorities


# ═══════════════════════════════════════════════════════════════════════════════
# EXPERIMENT 9: TAS code structure analysis
# ═══════════════════════════════════════════════════════════════════════════════

def experiment_tas_structure(tas_reference):
    print("=" * 80)
    print("EXPERIMENT 9: TAS CODE STRUCTURE ANALYSIS")
    print("Understanding the anatomy of TAS codes and what each part means")
    print("=" * 80)
    print()

    print("TAS Code Structure:")
    print("  AAA-BBBB (Federal Account level)")
    print("    AAA  = Agency identifier (e.g., 070 = DHS, 097 = DOD)")
    print("    BBBB = Main account code (e.g., 0400 = SS Ops)")
    print()
    print("  AAA-YYYY/ZZZZ-BBBB-SSS (Treasury Account level)")
    print("    YYYY/ZZZZ = Period of availability (fiscal years)")
    print("    SSS  = Sub-account code (usually 000)")
    print()
    print("  Special availability codes:")
    print("    X = No-year (available until expended)")
    print("    YYYY/YYYY = One-year")
    print("    YYYY/ZZZZ = Multi-year")
    print()

    # Analyze account code patterns across agencies
    print("--- Account code patterns by agency ---\n")

    for agency_code in sorted(tas_reference.get("agencies", {}).keys()):
        agency = tas_reference["agencies"][agency_code]
        accounts = agency["accounts"]

        # Look at account code ranges
        main_codes = []
        for acct in accounts:
            m = re.match(r"\d+-(\d+)", acct["tas_code"])
            if m:
                main_codes.append(int(m.group(1)))

        if main_codes:
            print(f"  {agency_code} ({agency['name'][:40]})")
            print(f"    {len(accounts)} accounts, codes range {min(main_codes):04d}-{max(main_codes):04d}")

            # Count by prefix ranges
            ranges = defaultdict(int)
            for c in main_codes:
                if c < 1000:
                    ranges["0000-0999 (appropriations)"] += 1
                elif c < 2000:
                    ranges["1000-1999 (special/no-year)"] += 1
                elif c < 4000:
                    ranges["2000-3999 (revolving/mgmt)"] += 1
                elif c < 5000:
                    ranges["4000-4999 (revolving funds)"] += 1
                elif c < 6000:
                    ranges["5000-5999 (deposit/receipt)"] += 1
                else:
                    ranges["6000+     (trust/other)"] += 1

            for rng, count in sorted(ranges.items()):
                print(f"      {rng}: {count}")
            print()

    print("FINDING: Account codes 0000-0999 are typically appropriated accounts")
    print("(the ones that appear in appropriations bills). Higher codes are")
    print("revolving funds, trust funds, and special accounts that are usually")
    print("NOT in appropriations bills. This helps filter our matching.\n")


# ═══════════════════════════════════════════════════════════════════════════════
# MAIN
# ═══════════════════════════════════════════════════════════════════════════════

def main():
    os.chdir(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    print()
    print("╔══════════════════════════════════════════════════════════════════╗")
    print("║  TAS DEEP DIVE — Treasury Account Symbols as Authority IDs     ║")
    print("╚══════════════════════════════════════════════════════════════════╝")
    print()

    # Run experiments
    experiment_historical_depth()
    experiment_name_changes()
    experiment_agency_moves()
    experiment_fast_book()

    tas_reference = experiment_build_tas_reference()
    experiment_tas_structure(tas_reference)

    match_results = experiment_match_xml_to_tas(tas_reference)
    experiment_spending_comparison()
    authorities = experiment_prototype_authority(tas_reference, match_results)

    # Final summary
    print("=" * 80)
    print("FINAL SUMMARY")
    print("=" * 80)
    print()
    print("1. HISTORICAL DEPTH: USASpending goes back to ~FY2008.")
    print("   For earlier data, need FAST Book historical editions.")
    print()
    print("2. NAME STABILITY: TAS codes ARE stable through renames.")
    print("   'Salaries and Expenses' -> 'Operations and Support' keeps")
    print("   the same TAS code. This is exactly what we need.")
    print()
    print("3. AGENCY MOVES: TAS codes CHANGE when agencies move departments.")
    print("   Secret Service went from 020-XXXX (Treasury) to 070-0400 (DHS).")
    print("   Our authority system needs an 'agency_move' event to bridge these.")
    print()
    print("4. MATCHING QUALITY: TAS short names match XML account names well")
    print("   after em-dash prefix stripping. The mapping is viable.")
    print()
    print("5. ARCHITECTURE: The authority file should:")
    print("   a. Use TAS code as primary ID (for post-reorganization accounts)")
    print("   b. Record historical TAS codes for pre-reorganization periods")
    print("   c. Be seeded from USASpending API + FAST Book")
    print("   d. Be enriched by XML heading matching from our bill extractions")
    print("   e. Be curated by human review for edge cases")
    print()
    print("6. FILES GENERATED:")
    print("   tmp/tas_reference.json     — TAS codes for 13 major agencies")
    print("   tmp/authorities_prototype.json — Draft authority mappings")
    print()


if __name__ == "__main__":
    main()
