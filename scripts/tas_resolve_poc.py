#!/usr/bin/env python3
"""
Proof-of-concept: LLM-based TAS (Treasury Account Symbol) resolution.

This script demonstrates a pipeline step that takes a bill's extracted provisions
and XML source, fetches relevant TAS codes from USASpending, and uses Claude
to match each appropriation provision to its TAS code.

The output is a `tas_mapping.json` per bill that maps provision indices to TAS codes.
Once every bill has a TAS mapping, cross-bill linking becomes trivial:
provisions with the same TAS code are the same account.

Pipeline position:
  XML → extract → enrich → resolve-tas → embed → link

Usage:
    source /Users/chris.gorski/anthropic_key.source
    source .venv/bin/activate
    python scripts/tas_resolve_poc.py [--bill BILL_DIR] [--all] [--dry-run]

Examples:
    python scripts/tas_resolve_poc.py --bill 116-hr1158
    python scripts/tas_resolve_poc.py --bill 118-hr2882
    python scripts/tas_resolve_poc.py --all --dry-run
"""

import argparse
import json
import os
import re
import sys
import time
from collections import defaultdict
from pathlib import Path

import anthropic
import requests
from lxml import etree

# ─── Configuration ────────────────────────────────────────────────────────────

USASPENDING_BASE = "https://api.usaspending.gov/api/v2"
TAS_CACHE_DIR = Path("tmp/tas_cache")
MODEL = "claude-opus-4-6"
MAX_PROVISIONS_PER_BATCH = 40  # How many provisions to send per LLM call

# Known agency code mappings (agency name fragments → USASpending agency codes)
AGENCY_CODE_MAP = {
    "department of homeland security": "070",
    "homeland security": "070",
    "department of defense": "097",
    "defense": "097",
    "department of veterans affairs": "036",
    "veterans affairs": "036",
    "department of transportation": "069",
    "transportation": "069",
    "department of housing and urban development": "086",
    "housing and urban development": "086",
    "hud": "086",
    "department of agriculture": "012",
    "agriculture": "012",
    "usda": "012",
    "department of health and human services": "075",
    "health and human services": "075",
    "hhs": "075",
    "department of justice": "015",
    "justice": "015",
    "department of state": "019",
    "state": "019",
    "department of the interior": "014",
    "interior": "014",
    "department of energy": "089",
    "energy": "089",
    "department of the treasury": "020",
    "treasury": "020",
    "department of labor": "1601",
    "labor": "1601",
    "department of education": "091",
    "education": "091",
    "department of commerce": "013",
    "commerce": "013",
    "environmental protection agency": "068",
    "epa": "068",
    "national aeronautics and space administration": "080",
    "nasa": "080",
    "national science foundation": "049",
    "nsf": "049",
    "small business administration": "073",
    "sba": "073",
    "general services administration": "047",
    "gsa": "047",
    "office of personnel management": "024",
    "opm": "024",
    "social security administration": "028",
    "ssa": "028",
    "corps of engineers": "096",
    "army corps": "096",
    "nuclear regulatory commission": "031",
    "nrc": "031",
    "federal communications commission": "027",
    "fcc": "027",
    "securities and exchange commission": "050",
    "sec": "050",
    "federal trade commission": "029",
    "ftc": "029",
    "equal employment opportunity commission": "045",
    "eeoc": "045",
    "national labor relations board": "420",
    "nlrb": "420",
    "peace corps": "1125",
    "u.s. agency for international development": "072",
    "usaid": "072",
    "agency for international development": "072",
    "executive office of the president": "1100",
    "railroad retirement board": "060",
    "consumer product safety commission": "061",
    "commodity futures trading commission": "339",
    "federal election commission": "360",
    "national archives": "088",
    "nara": "088",
    "government accountability office": "005",
    "gao": "005",
    "legislative branch": "009",
    # DHS sub-agencies that the LLM sometimes extracts as top-level
    "federal emergency management agency": "070",
    "fema": "070",
    "u.s. customs and border protection": "070",
    "customs and border protection": "070",
    "cbp": "070",
    "u.s. immigration and customs enforcement": "070",
    "immigration and customs enforcement": "070",
    "ice": "070",
    "transportation security administration": "070",
    "tsa": "070",
    "coast guard": "070",
    "united states secret service": "070",
    "secret service": "070",
    "cybersecurity and infrastructure security agency": "070",
    "cisa": "070",
    # DOD sub-agencies — all fetched from parent 097, but TAS codes use
    # service-specific prefixes (021=Army, 017=Navy/Marines, 057=Air Force/Space)
    "department of the army": "097",
    "department of the navy": "097",
    "department of the air force": "097",
    "defense logistics agency": "097",
    "army": "097",
    "navy": "097",
    "air force": "097",
    "marine corps": "097",
    "space force": "097",
    "army national guard": "097",
    "air national guard": "097",
    "army reserve": "097",
    "navy reserve": "097",
    "air force reserve": "097",
    # HHS sub-agencies
    "national institutes of health": "075",
    "nih": "075",
    "centers for disease control and prevention": "075",
    "cdc": "075",
    "food and drug administration": "075",
    "fda": "075",
    "centers for medicare and medicaid services": "075",
    "cms": "075",
    "health resources and services administration": "075",
    "hrsa": "075",
    "substance abuse and mental health services administration": "075",
    "samhsa": "075",
    "indian health service": "075",
    "administration for children and families": "075",
    # DOT sub-agencies
    "federal highway administration": "069",
    "fhwa": "069",
    "federal aviation administration": "069",
    "faa": "069",
    "federal transit administration": "069",
    "fta": "069",
    "federal railroad administration": "069",
    "fra": "069",
    "national highway traffic safety administration": "069",
    "nhtsa": "069",
    "pipeline and hazardous materials safety administration": "069",
    "phmsa": "069",
    "maritime administration": "069",
    # Interior sub-agencies
    "bureau of land management": "014",
    "national park service": "014",
    "u.s. fish and wildlife service": "014",
    "fish and wildlife service": "014",
    "bureau of reclamation": "014",
    "bureau of indian affairs": "014",
    "u.s. geological survey": "014",
    "geological survey": "014",
    "bureau of ocean energy management": "014",
    "office of surface mining": "014",
    # USDA sub-agencies
    "forest service": "012",
    "natural resources conservation service": "012",
    "farm service agency": "012",
    "rural development": "012",
    "food and nutrition service": "012",
    "animal and plant health inspection service": "012",
    "agricultural research service": "012",
    # DOJ sub-agencies
    "federal bureau of investigation": "015",
    "fbi": "015",
    "drug enforcement administration": "015",
    "dea": "015",
    "bureau of alcohol, tobacco, firearms and explosives": "015",
    "atf": "015",
    "federal bureau of prisons": "015",
    "u.s. marshals service": "015",
    "marshals service": "015",
    # Commerce sub-agencies
    "national oceanic and atmospheric administration": "013",
    "noaa": "013",
    "census bureau": "013",
    "bureau of the census": "013",
    "national institute of standards and technology": "013",
    "nist": "013",
    "patent and trademark office": "013",
    "international trade administration": "013",
}


# ─── USASpending API helpers ──────────────────────────────────────────────────

_last_api_call = 0.0


def api_get(url, params=None):
    """Rate-limited GET to USASpending API."""
    global _last_api_call
    elapsed = time.time() - _last_api_call
    if elapsed < 0.25:
        time.sleep(0.25 - elapsed)
    resp = requests.get(url, params=params, timeout=30)
    _last_api_call = time.time()
    resp.raise_for_status()
    return resp.json()


# DOD service branch TAS prefix mapping.
# USASpending nests all DOD under agency 097, but the actual TAS codes
# use service-specific prefixes. We fetch from 097 and let the LLM match
# to the correct service-specific TAS code (e.g., 021-2020 for Army O&M).
DOD_BRANCH_TAS_PREFIXES = {
    "army": "021",
    "navy": "017",
    "marine corps": "017",
    "air force": "057",
    "space force": "057",
}


def get_agency_code(agency_name: str, account_name: str = "") -> str | None:
    """Map an agency name to a USASpending agency code for TAS fetching.

    All DOD service branches map to 097 for fetching — the TAS codes under 097
    already include service-specific prefixes (021-xxx, 017-xxx, 057-xxx).
    The LLM handles matching provisions to the correct service-specific TAS code.
    """
    if not agency_name:
        return None
    lower = agency_name.lower().strip()

    # Try exact match first
    if lower in AGENCY_CODE_MAP:
        return AGENCY_CODE_MAP[lower]
    # Try substring match
    for key, code in AGENCY_CODE_MAP.items():
        if key in lower or lower in key:
            return code
    return None


def fetch_tas_for_agency(agency_code: str) -> list[dict]:
    """Fetch all TAS codes for an agency, with caching."""
    cache_path = TAS_CACHE_DIR / f"tas_{agency_code}.json"
    if cache_path.exists():
        with open(cache_path) as f:
            return json.load(f)

    try:
        data = api_get(f"{USASPENDING_BASE}/references/filter_tree/tas/{agency_code}/")
        results = data.get("results", [])

        # Save cache
        TAS_CACHE_DIR.mkdir(parents=True, exist_ok=True)
        with open(cache_path, "w") as f:
            json.dump(results, f, indent=2)

        return results
    except Exception as e:
        print(f"    Warning: Failed to fetch TAS for agency {agency_code}: {e}")
        return []


# ─── XML parsing ──────────────────────────────────────────────────────────────


def extract_xml_headings(xml_path: str) -> list[dict]:
    """Extract appropriations-major and appropriations-intermediate headings from XML."""
    tree = etree.parse(xml_path)
    headings = []

    for tag_name in ("appropriations-major", "appropriations-intermediate"):
        for elem in tree.iter(tag_name):
            header_el = elem.find("header")
            if header_el is not None and header_el.text:
                text = header_el.text.strip()
                if text:
                    # Get the division context
                    division = None
                    parent = elem.getparent()
                    while parent is not None:
                        local = etree.QName(parent.tag).localname if isinstance(parent.tag, str) else ""
                        if local == "division":
                            div_enum = parent.find("enum")
                            if div_enum is not None and div_enum.text:
                                division = div_enum.text.strip()
                            break
                        parent = parent.getparent()

                    headings.append({
                        "tag": tag_name,
                        "text": text,
                        "division": division,
                    })

    return headings


def get_xml_context_for_account(xml_path: str, account_name: str, window: int = 400) -> str:
    """Get XML context around an account name for LLM disambiguation."""
    with open(xml_path) as f:
        xml_text = f.read()

    # Search for the account name (try both unicode and ASCII quotes)
    search_variants = [
        f"\u2018\u2018{account_name}\u2019\u2019",  # Unicode curly quotes
        f"''{account_name}''",  # ASCII
        account_name,  # Plain
    ]

    pos = None
    for variant in search_variants:
        idx = xml_text.lower().find(variant.lower())
        if idx >= 0:
            pos = idx
            break

    if pos is None:
        return "(not found in XML)"

    start = max(0, pos - window)
    end = min(len(xml_text), pos + len(account_name) + window)
    raw = xml_text[start:end]

    # Clean XML tags but preserve heading structure
    cleaned = re.sub(r"<appropriations-major[^>]*>", "\n[MAJOR] ", raw)
    cleaned = re.sub(r"</appropriations-major>", " [/MAJOR]", cleaned)
    cleaned = re.sub(r"<appropriations-intermediate[^>]*>", "\n  [SUBHEADING] ", cleaned)
    cleaned = re.sub(r"</appropriations-intermediate>", " [/SUBHEADING]", cleaned)
    cleaned = re.sub(r"<header[^>]*>", "", cleaned)
    cleaned = re.sub(r"</header>", "", cleaned)
    cleaned = re.sub(r"<[^>]+>", "", cleaned)
    cleaned = re.sub(r"\s+", " ", cleaned)
    cleaned = cleaned.strip()

    return cleaned[:800]


# ─── Provision loading ────────────────────────────────────────────────────────


def load_provisions(bill_dir: str) -> list[dict]:
    """Load provisions from extraction.json."""
    ext_path = os.path.join(bill_dir, "extraction.json")
    if not os.path.exists(ext_path):
        return []

    with open(ext_path) as f:
        data = json.load(f)

    return data.get("provisions", [])


def get_top_level_appropriations(provisions: list[dict]) -> list[dict]:
    """Filter to top-level budget authority appropriations."""
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
        account = p.get("account_name", "")
        if not account:
            continue

        results.append({
            "index": i,
            "account_name": account,
            "agency": p.get("agency", ""),
            "dollars": amt.get("dollars", 0),
            "division": p.get("division"),
            "section": p.get("section", ""),
            "fiscal_year": p.get("fiscal_year"),
            "detail_level": dl,
        })

    return results


# ─── TAS resolution via LLM ──────────────────────────────────────────────────


TAS_SYSTEM_PROMPT = """You are an expert on U.S. federal budget accounts and Treasury Account Symbols (TAS).

You will receive:
1. A list of appropriation provisions extracted from a congressional bill, each with an account name, agency, and dollar amount.
2. A list of TAS codes for the relevant agency, each with a code (like "070-0400") and title (like "Operations and Support, United States Secret Service, Homeland Security").

Your task: Match each provision to the most likely TAS code.

Key rules:
- The account name in the bill often appears as "Agency Name—Account Title" with an em-dash. The TAS title is usually "Account Title, Agency Name, Department Name".
- "Salaries and Expenses" was renamed to "Operations and Support" for many DHS accounts around FY2017. Same TAS code, different name.
- If a provision clearly matches a TAS code, assign it with confidence "high".
- If the match is plausible but ambiguous (e.g., "Salaries and Expenses" could match multiple agencies), assign with confidence "medium".
- If you cannot identify a match, set tas_code to null and confidence to "none".
- Match to TAS codes in the 0000-3999 range. Codes 4000+ are revolving/trust/deposit funds and should not be matched.
- DOD service branches (Army=021, Navy/Marines=017, Air Force/Space=057) use account codes in the 1000-3000 range for their main appropriation accounts — these are valid matches.
- When multiple provisions map to the same TAS code, that's expected (a bill may have multiple line items under one account).

Return JSON (no markdown, no explanation):
{
  "mappings": [
    {
      "provision_index": 0,
      "account_name": "the account name from the provision",
      "tas_code": "070-0400" or null,
      "tas_title": "the TAS title" or null,
      "confidence": "high" | "medium" | "low" | "none",
      "reasoning": "brief explanation of why this match"
    }
  ]
}

CRITICAL: Return valid JSON only. No markdown code blocks. No text before or after the JSON."""


def build_tas_prompt(provisions: list[dict], tas_codes: list[dict], bill_id: str, xml_contexts: dict) -> str:
    """Build the user prompt for TAS resolution."""
    parts = [f"Bill: {bill_id}\n\n"]

    parts.append("== PROVISIONS TO MATCH ==\n\n")
    for i, p in enumerate(provisions):
        dollars = p.get("dollars", 0) or 0
        dollars_str = f"${dollars:,}" if dollars else "no amount"
        xml_ctx = xml_contexts.get(p["index"], "")
        ctx_snippet = f"\n    XML context: {xml_ctx[:200]}..." if xml_ctx and xml_ctx != "(not found in XML)" else ""

        parts.append(
            f"  [{i}] index={p['index']} account=\"{p['account_name']}\" "
            f"agency=\"{p.get('agency', '')}\" {dollars_str} "
            f"div={p.get('division', '?')} sec={p.get('section', '?')}"
            f"{ctx_snippet}\n"
        )

    parts.append("\n== TAS CODES FOR THIS AGENCY ==\n\n")
    for tas in tas_codes:
        # Skip revolving funds (4000+), deposit/receipt (5000+), trust (6000+)
        # but keep appropriation accounts (0000-3999) — DOD uses 1000-3000 range
        code = tas.get("id", "")
        main_code_match = re.search(r"-(\d{4})", code)
        if main_code_match:
            main_code = int(main_code_match.group(1))
            if main_code >= 4000:
                continue

        parts.append(f"  {tas['id']}: {tas['description']}\n")

    return "".join(parts)


def resolve_tas_with_llm(
    client: anthropic.Anthropic,
    provisions: list[dict],
    tas_codes: list[dict],
    bill_id: str,
    xml_path: str | None,
    dry_run: bool = False,
) -> list[dict]:
    """Send provisions + TAS codes to Claude for matching."""

    # Get XML context for each provision
    xml_contexts = {}
    if xml_path and os.path.exists(xml_path):
        for p in provisions:
            ctx = get_xml_context_for_account(xml_path, p["account_name"])
            xml_contexts[p["index"]] = ctx

    prompt = build_tas_prompt(provisions, tas_codes, bill_id, xml_contexts)

    if dry_run:
        approp_tas = [t for t in tas_codes if re.search(r'-(\d{4})', t['id']) and int(re.search(r'-(\d{4})', t['id']).group(1)) < 1000]
        print(f"\n--- DRY RUN: Would send {len(provisions)} provisions ---")
        print(f"    TAS codes available: {len(approp_tas)}")
        print(f"    Prompt length: {len(prompt)} chars")
        print(f"    First 500 chars of prompt:\n{prompt[:500]}")
        return []

    print(f"    Sending {len(provisions)} provisions to Claude ({MODEL})...")

    response = client.messages.create(
        model=MODEL,
        max_tokens=16000,
        temperature=1,  # required for extended thinking
        thinking={
            "type": "enabled",
            "budget_tokens": 10000,
        },
        system=TAS_SYSTEM_PROMPT,
        messages=[{"role": "user", "content": prompt}],
    )

    # With thinking enabled, response has thinking blocks + text blocks
    response_text = ""
    thinking_text = ""
    for block in response.content:
        if block.type == "thinking":
            thinking_text = block.thinking
        elif block.type == "text":
            response_text = block.text

    input_tokens = response.usage.input_tokens
    output_tokens = response.usage.output_tokens
    print(f"    Thinking: {len(thinking_text)} chars")
    print(f"    Tokens: {input_tokens} in, {output_tokens} out")

    # Parse the JSON response
    try:
        # Try to extract JSON from response (handle markdown wrapping)
        json_text = response_text.strip()
        if json_text.startswith("```"):
            json_text = re.sub(r"^```\w*\n?", "", json_text)
            json_text = re.sub(r"\n?```$", "", json_text)

        result = json.loads(json_text)
        mappings = result.get("mappings", [])
        print(f"    Got {len(mappings)} mappings")
        return mappings
    except json.JSONDecodeError as e:
        print(f"    ERROR: Failed to parse LLM response: {e}")
        print(f"    Response preview: {response_text[:300]}")
        return []


# ─── Main pipeline ────────────────────────────────────────────────────────────


def resolve_bill(bill_dir: str, dry_run: bool = False) -> dict | None:
    """Resolve TAS codes for all top-level appropriations in a bill."""
    bill_name = os.path.basename(bill_dir)
    print(f"\n{'=' * 70}")
    print(f"  RESOLVING TAS: {bill_name}")
    print(f"{'=' * 70}")

    # Load provisions
    provisions = load_provisions(bill_dir)
    if not provisions:
        print(f"  No extraction.json found in {bill_dir}")
        return None

    top_level = get_top_level_appropriations(provisions)
    print(f"  Total provisions: {len(provisions)}")
    print(f"  Top-level BA appropriations: {len(top_level)}")

    if not top_level:
        print("  No top-level appropriations to resolve.")
        return None

    # Find XML file
    xml_files = list(Path(bill_dir).glob("BILLS-*.xml"))
    xml_path = str(xml_files[0]) if xml_files else None
    if xml_path:
        print(f"  XML source: {os.path.basename(xml_path)}")
    else:
        print("  WARNING: No XML file found — no structural context for LLM")

    # Extract XML headings for context
    if xml_path:
        headings = extract_xml_headings(xml_path)
        print(f"  XML headings: {len(headings)} (major + intermediate)")

    # Determine which agencies are in this bill
    agencies = set()
    for p in top_level:
        agency = p.get("agency", "")
        if agency:
            code = get_agency_code(agency, p.get("account_name", ""))
            if code:
                agencies.add(code)

    if not agencies:
        # Fallback: try to determine from bill directory name or XML
        print("  WARNING: Could not determine agency codes from provision data")
        # Try common ones
        agencies = {"070", "097", "036", "069", "086"}

    print(f"  Agency codes to fetch TAS for: {sorted(agencies)}")

    # Fetch TAS codes for all relevant agencies
    all_tas = []
    for agency_code in sorted(agencies):
        tas = fetch_tas_for_agency(agency_code)
        print(f"  Fetched {len(tas)} TAS codes for agency {agency_code}")
        all_tas.extend(tas)

    print(f"  Total TAS codes: {len(all_tas)}")

    # Initialize Anthropic client
    client = anthropic.Anthropic()

    # Batch provisions by agency to keep prompts focused
    provisions_by_agency = defaultdict(list)
    for p in top_level:
        agency_code = get_agency_code(p.get("agency", ""), p.get("account_name", "")) or "unknown"
        provisions_by_agency[agency_code].append(p)

    all_mappings = []

    for agency_code, agency_provisions in provisions_by_agency.items():
        # Get TAS codes for this specific agency
        if agency_code != "unknown":
            agency_tas = fetch_tas_for_agency(agency_code)
        else:
            agency_tas = all_tas

        # Batch if needed
        for batch_start in range(0, len(agency_provisions), MAX_PROVISIONS_PER_BATCH):
            batch = agency_provisions[batch_start:batch_start + MAX_PROVISIONS_PER_BATCH]

            print(f"\n  Batch: agency={agency_code}, provisions {batch_start+1}-{batch_start+len(batch)} of {len(agency_provisions)}")

            # Get bill identifier from extraction
            ext_path = os.path.join(bill_dir, "extraction.json")
            with open(ext_path) as f:
                bill_id = json.load(f).get("bill", {}).get("identifier", bill_name)

            mappings = resolve_tas_with_llm(
                client=client,
                provisions=batch,
                tas_codes=agency_tas,
                bill_id=bill_id,
                xml_path=xml_path,
                dry_run=dry_run,
            )
            all_mappings.extend(mappings)

    if dry_run:
        print(f"\n  DRY RUN complete. Would resolve {len(top_level)} provisions.")
        return None

    # Build the output file
    output = {
        "schema_version": "1.0",
        "bill_dir": bill_name,
        "bill_identifier": bill_id,
        "model": MODEL,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "total_provisions": len(provisions),
        "resolved_provisions": len(top_level),
        "mappings": all_mappings,
        "summary": {
            "high_confidence": sum(1 for m in all_mappings if m.get("confidence") == "high"),
            "medium_confidence": sum(1 for m in all_mappings if m.get("confidence") == "medium"),
            "low_confidence": sum(1 for m in all_mappings if m.get("confidence") == "low"),
            "unmatched": sum(1 for m in all_mappings if m.get("confidence") == "none" or m.get("tas_code") is None),
        },
    }

    # Count unique TAS codes
    tas_codes_found = set()
    for m in all_mappings:
        if m.get("tas_code"):
            tas_codes_found.add(m["tas_code"])
    output["summary"]["unique_tas_codes"] = len(tas_codes_found)

    # Save
    output_path = os.path.join(bill_dir, "tas_mapping.json")
    with open(output_path, "w") as f:
        json.dump(output, f, indent=2)

    print(f"\n  ✅ Saved {output_path}")
    print(f"  Summary: {output['summary']}")

    return output


def print_mapping_report(output: dict):
    """Print a human-readable report of TAS mappings."""
    if not output:
        return

    print(f"\n{'─' * 70}")
    print(f"  TAS MAPPING REPORT: {output['bill_identifier']}")
    print(f"{'─' * 70}")

    by_tas = defaultdict(list)
    unmatched = []

    for m in output.get("mappings", []):
        tas = m.get("tas_code")
        if tas:
            by_tas[tas].append(m)
        else:
            unmatched.append(m)

    print(f"\n  Matched to {len(by_tas)} unique TAS codes:")
    for tas_code in sorted(by_tas.keys()):
        entries = by_tas[tas_code]
        first = entries[0]
        total_dollars = sum(e.get("dollars", 0) or 0 for e in entries if isinstance(e.get("dollars"), (int, float)))
        conf = first.get("confidence", "?")
        title = first.get("tas_title", "")
        print(f"\n    TAS {tas_code}: {title}")
        for e in entries:
            d = e.get("dollars", 0) or 0
            d_str = f"${d:>15,}" if d else "         no amount"
            acct = e.get("account_name", "")[:50]
            print(f"      [{e.get('confidence', '?'):6s}] {acct:50s}  {d_str}")

    if unmatched:
        print(f"\n  Unmatched ({len(unmatched)}):")
        for m in unmatched:
            acct = m.get("account_name", "")[:60]
            print(f"    ❌ {acct}")

    s = output.get("summary", {})
    total = s.get("high_confidence", 0) + s.get("medium_confidence", 0) + s.get("low_confidence", 0) + s.get("unmatched", 0)
    if total > 0:
        match_rate = (total - s.get("unmatched", 0)) / total * 100
        print(f"\n  Match rate: {match_rate:.1f}% ({total - s.get('unmatched', 0)}/{total})")
        print(f"  High: {s.get('high_confidence', 0)}, Medium: {s.get('medium_confidence', 0)}, "
              f"Low: {s.get('low_confidence', 0)}, None: {s.get('unmatched', 0)}")


def cross_bill_linking_demo(bill_dirs: list[str]):
    """Demonstrate how TAS mappings enable trivial cross-bill linking."""
    print(f"\n{'=' * 70}")
    print("  CROSS-BILL LINKING DEMO")
    print(f"  Using TAS mappings from {len(bill_dirs)} bills")
    print(f"{'=' * 70}")

    # Load all TAS mappings
    all_provisions = []  # (bill, provision_index, tas_code, account_name, dollars, confidence)

    for bill_dir in bill_dirs:
        tas_path = os.path.join(bill_dir, "tas_mapping.json")
        if not os.path.exists(tas_path):
            continue

        with open(tas_path) as f:
            data = json.load(f)

        bill_id = data.get("bill_identifier", os.path.basename(bill_dir))
        bill_name = os.path.basename(bill_dir)

        for m in data.get("mappings", []):
            if m.get("tas_code"):
                all_provisions.append({
                    "bill": bill_id,
                    "bill_dir": bill_name,
                    "index": m.get("provision_index"),
                    "tas_code": m["tas_code"],
                    "tas_title": m.get("tas_title", ""),
                    "account_name": m.get("account_name", ""),
                    "confidence": m.get("confidence", ""),
                })

    if not all_provisions:
        print("  No TAS mappings found. Run resolve first.")
        return

    # Group by TAS code
    by_tas = defaultdict(list)
    for p in all_provisions:
        by_tas[p["tas_code"]].append(p)

    # Find TAS codes that appear in multiple bills (= cross-bill links)
    multi_bill = {
        tas: entries
        for tas, entries in by_tas.items()
        if len(set(e["bill_dir"] for e in entries)) > 1
    }

    print(f"\n  Total provisions with TAS: {len(all_provisions)}")
    print(f"  Unique TAS codes: {len(by_tas)}")
    print(f"  TAS codes in multiple bills: {len(multi_bill)} (these are automatic links!)")

    # Show the cross-bill links
    if multi_bill:
        print(f"\n  --- Cross-Bill Links (same TAS code) ---\n")
        for tas_code in sorted(multi_bill.keys()):
            entries = multi_bill[tas_code]
            title = entries[0].get("tas_title", "")
            bills_involved = sorted(set(e["bill_dir"] for e in entries))
            print(f"  TAS {tas_code}: {title}")
            for e in entries:
                conf = e.get("confidence", "?")
                acct = e.get("account_name", "")[:45]
                print(f"    {e['bill']:20s}  [{conf:6s}] {acct}")
            print()

    print("  FINDING: With TAS resolution, cross-bill linking is a simple GROUP BY.")
    print("  No embeddings needed. No similarity thresholds. No human review for")
    print("  high-confidence matches. The TAS code IS the link.")


# ─── CLI ──────────────────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser(description="POC: LLM-based TAS resolution")
    parser.add_argument("--bill", type=str, help="Single bill directory to resolve (e.g., 116-hr1158)")
    parser.add_argument("--all", action="store_true", help="Resolve all bills with extractions")
    parser.add_argument("--dry-run", action="store_true", help="Show what would be sent without calling LLM")
    parser.add_argument("--demo-links", action="store_true", help="Demo cross-bill linking from existing TAS mappings")
    parser.add_argument("--data-dir", type=str, default="data", help="Data directory")
    args = parser.parse_args()

    os.chdir(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

    data_dir = args.data_dir

    if args.demo_links:
        bill_dirs = sorted(glob for glob in Path(data_dir).iterdir() if glob.is_dir() and (glob / "tas_mapping.json").exists())
        cross_bill_linking_demo([str(d) for d in bill_dirs])
        return

    if args.bill:
        bill_path = os.path.join(data_dir, args.bill)
        if not os.path.isdir(bill_path):
            print(f"Error: {bill_path} is not a directory")
            sys.exit(1)
        output = resolve_bill(bill_path, dry_run=args.dry_run)
        if output:
            print_mapping_report(output)

    elif args.all:
        bill_dirs = sorted(
            d for d in Path(data_dir).iterdir()
            if d.is_dir() and (d / "extraction.json").exists()
        )
        print(f"Found {len(bill_dirs)} bills with extractions")

        results = []
        for bill_dir in bill_dirs:
            output = resolve_bill(str(bill_dir), dry_run=args.dry_run)
            if output:
                results.append(output)
                print_mapping_report(output)

        if results and not args.dry_run:
            # Run cross-bill demo
            cross_bill_linking_demo([str(d) for d in bill_dirs])

    else:
        # Default: resolve a few interesting bills for testing
        test_bills = [
            "116-hr1158",  # FY2020 minibus (DHS, CJS, FinServ, Homeland)
            "116-hr133",   # FY2021 omnibus (all 12 subcommittees)
            "118-hr2882",  # FY2024 second omnibus
        ]
        results = []
        for bill_name in test_bills:
            bill_path = os.path.join(data_dir, bill_name)
            if os.path.isdir(bill_path) and os.path.exists(os.path.join(bill_path, "extraction.json")):
                output = resolve_bill(bill_path, dry_run=args.dry_run)
                if output:
                    results.append(output)
                    print_mapping_report(output)
            else:
                print(f"  Skipping {bill_name} — no extraction")

        if results and not args.dry_run:
            cross_bill_linking_demo([os.path.join(data_dir, b) for b in test_bills])


if __name__ == "__main__":
    main()
