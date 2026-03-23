#!/usr/bin/env python3
"""
Compare Secret Service account structure from bill XML against
Treasury Account Symbols (TAS) from USASpending.gov.

Usage:
    source .venv/bin/activate
    python scripts/ss_tas_check.py
"""

import re
import os
import glob
import json
import requests
from lxml import etree


def fetch_tas_data():
    """Fetch Secret Service TAS codes from USASpending API."""
    print("=== FETCHING SECRET SERVICE TAS CODES FROM USASPENDING ===\n")

    # Get all DHS federal accounts
    resp = requests.get(
        "https://api.usaspending.gov/api/v2/agency/070/federal_account/",
        params={
            "fiscal_year": 2024,
            "order": "desc",
            "sort": "obligated_amount",
            "page": 1,
            "limit": 100,
        },
    )
    data = resp.json()

    ss_accounts = []
    for acct in data["results"]:
        if "secret service" in acct["name"].lower():
            ss_accounts.append(acct)
            print(f"  TAS {acct['code']}: {acct['name']}")
            print(f"    Obligated FY2024: ${acct['obligated_amount']:,.2f}")
            print()

    # Also get the sub-TAS detail for the main ops account
    print("  --- Sub-TAS detail for 070-0400 (main ops account) ---")
    resp2 = requests.get(
        "https://api.usaspending.gov/api/v2/references/filter_tree/tas/070/070-0400/"
    )
    sub_tas = resp2.json()
    for entry in sub_tas["results"][:8]:
        print(f"    {entry['id']}: {entry['description']}")
    print()

    return ss_accounts


def extract_ss_from_xml(xml_path):
    """Extract Secret Service account structure from a bill XML file."""
    tree = etree.parse(xml_path)
    results = []

    # Strategy 1: Find appropriations-intermediate headers for Secret Service
    for inter in tree.iter("appropriations-intermediate"):
        header_el = inter.find("header")
        if header_el is None:
            continue
        header_text = (header_el.text or "").strip()

        if "secret service" not in header_text.lower():
            continue

        results.append(
            {
                "type": "section_heading",
                "element": "appropriations-intermediate",
                "text": header_text,
            }
        )

        # Look for appropriations-small children (sub-accounts)
        for small in inter.iter("appropriations-small"):
            sh = small.find("header")
            if sh is not None:
                sh_text = (sh.text or "").strip()
                if sh_text:
                    full_text = etree.tostring(
                        small, method="text", encoding="unicode"
                    )
                    dollars = re.findall(r"\$[\d,]+", full_text)
                    results.append(
                        {
                            "type": "sub_account",
                            "element": "appropriations-small",
                            "text": sh_text,
                            "dollars": dollars[:5],
                        }
                    )

        # Look for text content with account name patterns (double single-quotes)
        for text_el in inter.iter("text"):
            raw = etree.tostring(text_el, method="text", encoding="unicode")
            # Find account name patterns: two single-quotes around a name
            for m in re.finditer(r"\u2018\u2018([^\u2019]+)\u2019\u2019", raw):
                name = m.group(1).strip()
                if len(name) > 5:
                    results.append(
                        {
                            "type": "account_ref_unicode",
                            "element": "text",
                            "text": name,
                        }
                    )
            # Also try ASCII double-single-quote pattern
            for m in re.finditer(r"''([^']+)''", raw):
                name = m.group(1).strip()
                if len(name) > 5:
                    results.append(
                        {
                            "type": "account_ref_ascii",
                            "element": "text",
                            "text": name,
                        }
                    )

    # Strategy 2: Search full text for Secret Service account name patterns
    full_xml_text = etree.tostring(tree, method="text", encoding="unicode")

    # Find "United States Secret Service" em-dash account patterns
    ss_account_pattern = re.compile(
        r"United States Secret Service[\u2014\u2013—–-]+([\w\s,]+?)(?=\s*[,.]|\s+is\b|\s+for\b|\s*\$)",
        re.IGNORECASE,
    )
    seen_accounts = set()
    for m in ss_account_pattern.finditer(full_xml_text):
        acct_name = m.group(1).strip()
        acct_name = re.sub(r"\s+", " ", acct_name)
        if len(acct_name) > 3 and acct_name.lower() not in seen_accounts:
            seen_accounts.add(acct_name.lower())
            results.append(
                {
                    "type": "emdash_account",
                    "element": "full_text_search",
                    "text": f"United States Secret Service\u2014{acct_name}",
                }
            )

    # Strategy 3: Look for dollar amounts near Secret Service mentions
    dollar_pattern = re.compile(
        r"(United States Secret Service[^\n]{0,100}\$[\d,]+)"
        r"|"
        r"(\$[\d,]+[^\n]{0,100}Secret Service)",
        re.IGNORECASE,
    )
    for m in dollar_pattern.finditer(full_xml_text):
        snippet = (m.group(1) or m.group(2) or "").strip()
        snippet = re.sub(r"\s+", " ", snippet)
        if len(snippet) > 20:
            dollar_vals = re.findall(r"\$([\d,]+)", snippet)
            results.append(
                {
                    "type": "dollar_context",
                    "element": "full_text_search",
                    "text": snippet[:200],
                    "dollars": [f"${d}" for d in dollar_vals],
                }
            )

    # Deduplicate
    seen = set()
    deduped = []
    for r in results:
        key = (r["type"], r["text"][:80])
        if key not in seen:
            seen.add(key)
            deduped.append(r)

    return deduped


def compare_tas_to_xml(tas_accounts, xml_results_by_bill):
    """Compare TAS account names to XML-extracted account names."""
    print("\n" + "=" * 80)
    print("COMPARISON: TAS CODES vs XML ACCOUNT NAMES")
    print("=" * 80 + "\n")

    tas_names = {}
    for acct in tas_accounts:
        code = acct["code"]
        name = acct["name"]
        # Extract just the account type (before the comma + agency)
        short_name = name.split(",")[0].strip()
        tas_names[code] = {"full": name, "short": short_name}

    print("TAS reference accounts:")
    for code, names in tas_names.items():
        print(f"  {code}: {names['short']}")
    print()

    # Collect all unique account names from XML across all bills
    xml_account_names = {}
    for bill_dir, results in sorted(xml_results_by_bill.items()):
        for r in results:
            if r["type"] in ("emdash_account", "account_ref_ascii", "account_ref_unicode"):
                name = r["text"]
                clean = name.lower().strip()
                if clean not in xml_account_names:
                    xml_account_names[clean] = {"original": name, "bills": []}
                xml_account_names[clean]["bills"].append(bill_dir)

    print("Unique account names found in XML across all bills:")
    for clean, info in sorted(xml_account_names.items()):
        bills = ", ".join(sorted(set(info["bills"])))
        print(f"  \"{info['original']}\"")
        print(f"    Found in: {bills}")
    print()

    # Try to match each XML account name to a TAS code
    print("--- MATCHING ---\n")
    for clean, info in sorted(xml_account_names.items()):
        original = info["original"]
        matched = False
        for code, tas in tas_names.items():
            tas_lower = tas["short"].lower()
            # Check if the XML name contains the TAS short name or vice versa
            if tas_lower in clean or clean.endswith(tas_lower):
                print(f"  MATCH: \"{original}\"")
                print(f"    -> TAS {code}: {tas['full']}")
                print(f"    Bills: {', '.join(sorted(set(info['bills'])))}")
                matched = True
                break
            # Also check after stripping the agency prefix
            xml_after_dash = clean.split("\u2014")[-1].split("—")[-1].split("–")[-1].strip()
            if xml_after_dash == tas_lower:
                print(f"  MATCH (after dash strip): \"{original}\"")
                print(f"    -> TAS {code}: {tas['full']}")
                print(f"    Bills: {', '.join(sorted(set(info['bills'])))}")
                matched = True
                break
        if not matched:
            print(f"  NO MATCH: \"{original}\"")
            print(f"    Bills: {', '.join(sorted(set(info['bills'])))}")
        print()


def main():
    os.chdir(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

    # Step 1: Fetch TAS data
    tas_accounts = fetch_tas_data()

    # Step 2: Extract Secret Service info from all bill XMLs
    print("=" * 80)
    print("SECRET SERVICE ACCOUNT STRUCTURE FROM BILL XML")
    print("=" * 80 + "\n")

    xml_results_by_bill = {}
    for xml_path in sorted(glob.glob("data/*/BILLS-*.xml")):
        bill_dir = os.path.basename(os.path.dirname(xml_path))
        results = extract_ss_from_xml(xml_path)
        if results:
            xml_results_by_bill[bill_dir] = results
            print(f"--- {bill_dir} ---")
            for r in results:
                rtype = r["type"]
                text = r["text"]
                dollars = r.get("dollars", [])
                d_str = f"  {', '.join(dollars)}" if dollars else ""
                print(f"  [{rtype:25s}] {text[:120]}{d_str}")
            print()

    if not xml_results_by_bill:
        print("No Secret Service data found in any XML files.")
        return

    # Step 3: Compare
    compare_tas_to_xml(tas_accounts, xml_results_by_bill)

    # Step 4: Summary
    print("=" * 80)
    print("SUMMARY")
    print("=" * 80 + "\n")

    all_bills_with_ss = sorted(xml_results_by_bill.keys())
    print(f"Bills with Secret Service provisions: {len(all_bills_with_ss)}")
    for b in all_bills_with_ss:
        n = len(xml_results_by_bill[b])
        print(f"  {b}: {n} items found")
    print()

    print("Key findings:")
    print("  1. The XML <appropriations-intermediate> heading is the SECTION")
    print("     level (\"United States Secret Service\") — NOT the account level.")
    print("  2. Individual accounts appear as em-dash suffixed names in the text:")
    print("     \"United States Secret Service—Operations and Support\"")
    print("  3. The TAS short name (e.g., \"Operations and Support\") matches")
    print("     the part AFTER the em-dash in the XML account name.")
    print("  4. The TAS code (e.g., 070-0400) is the stable identifier that")
    print("     persists even when the account name changes.")
    print()
    print("Mapping strategy:")
    print("  XML: \"United States Secret Service—Operations and Support\"")
    print("  -> strip agency prefix -> \"Operations and Support\"")
    print("  -> match to TAS short name -> 070-0400")
    print("  -> verify agency code 070 = DHS")


if __name__ == "__main__":
    main()
