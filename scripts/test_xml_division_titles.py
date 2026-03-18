#!/usr/bin/env python3
"""
Test script: Extract division titles from bill XML files.

Verifies that division titles can be parsed directly from the XML structure,
which would enable jurisdiction classification via pattern matching on the
title text rather than via embedding exemplars.

Congressional bill XML uses <toc-entry level="division"> elements in the
table of contents, and <header> elements within <division> tags in the body.
This script tries both approaches to extract human-readable division titles
like "Department of Defense" or "Transportation, Housing and Urban Development".
"""

import os
import re
import sys
from pathlib import Path

# We use the stdlib xml.etree since we just need basic parsing
# roxmltree is Rust-only; Python side uses ElementTree
import xml.etree.ElementTree as ET


# Known jurisdiction patterns for validation
JURISDICTION_PATTERNS = {
    "defense": [
        r"department of defense",
        r"defense$",
        r"defense appropriations",
    ],
    "labor-hhs": [
        r"departments? of labor.*health",
        r"labor.*hhs",
        r"labor, health",
    ],
    "thud": [
        r"transportation.*housing.*urban",
        r"thud",
    ],
    "financial-services": [
        r"financial services",
        r"general government",
    ],
    "cjs": [
        r"commerce.*justice.*science",
        r"department of justice",
        r"science, and related agencies",
    ],
    "energy-water": [
        r"energy.*water",
        r"corps of engineers",
    ],
    "interior": [
        r"interior.*environment",
        r"department of the interior",
    ],
    "agriculture": [
        r"agriculture.*rural",
        r"department of agriculture",
    ],
    "legislative-branch": [
        r"legislative branch",
    ],
    "milcon-va": [
        r"military construction.*veterans",
        r"milcon",
    ],
    "state-foreign-ops": [
        r"state.*foreign",
        r"department of state",
    ],
    "homeland-security": [
        r"homeland security",
    ],
    "continuing-resolution": [
        r"continuing appropriations",
        r"continuing resolution",
        r"further additional continuing",
        r"additional continuing",
        r"further continuing",
    ],
}


def classify_title(title: str) -> tuple:
    """
    Try to classify a division title into a jurisdiction using regex patterns.
    Returns (jurisdiction, confidence) where confidence is 'pattern_match' or 'unknown'.
    """
    title_lower = title.lower().strip()

    for jurisdiction, patterns in JURISDICTION_PATTERNS.items():
        for pattern in patterns:
            if re.search(pattern, title_lower):
                return jurisdiction, "pattern_match"

    return "unknown", "unknown"


def extract_divisions_from_toc(root, ns: dict) -> list:
    """
    Extract division titles from <toc-entry level="division"> elements.
    These appear in the table of contents section of the XML.
    """
    divisions = []

    # Try with and without namespace
    for toc_entry in root.iter():
        tag = toc_entry.tag
        # Strip namespace if present
        if "}" in tag:
            tag = tag.split("}", 1)[1]

        if tag == "toc-entry":
            level = toc_entry.get("level", "")
            if level == "division":
                # Get the text content
                text = "".join(toc_entry.itertext()).strip()
                # Clean up whitespace
                text = re.sub(r"\s+", " ", text)
                divisions.append(text)

    return divisions


def extract_divisions_from_headers(root, ns: dict) -> list:
    """
    Extract division titles from <header> elements inside <division> tags.
    These appear in the body of the bill.
    """
    divisions = []

    for elem in root.iter():
        tag = elem.tag
        if "}" in tag:
            tag = tag.split("}", 1)[1]

        if tag == "division":
            # Look for a header child
            for child in elem:
                child_tag = child.tag
                if "}" in child_tag:
                    child_tag = child_tag.split("}", 1)[1]

                if child_tag == "header":
                    text = "".join(child.itertext()).strip()
                    text = re.sub(r"\s+", " ", text)
                    divisions.append(text)
                    break  # Only take the first header per division

    return divisions


def extract_divisions_from_enum_header(root) -> list:
    """
    Extract division info from <enum> + <header> pairs within division elements.
    Common pattern: <division><enum>A</enum><header>Title Text</header>...
    """
    divisions = []

    for elem in root.iter():
        tag = elem.tag
        if "}" in tag:
            tag = tag.split("}", 1)[1]

        if tag == "division":
            enum_text = None
            header_text = None

            for child in elem:
                child_tag = child.tag
                if "}" in child_tag:
                    child_tag = child_tag.split("}", 1)[1]

                if child_tag == "enum" and enum_text is None:
                    enum_text = "".join(child.itertext()).strip()
                elif child_tag == "header" and header_text is None:
                    header_text = "".join(child.itertext()).strip()
                    header_text = re.sub(r"\s+", " ", header_text)

                if enum_text is not None and header_text is not None:
                    break

            if header_text:
                divisions.append({
                    "letter": enum_text or "?",
                    "title": header_text,
                })

    return divisions


def extract_division_letters_from_text(text: str) -> list:
    """
    Fallback: extract division references from the plain text using regex.
    Looks for patterns like "DIVISION A—Department of Defense"
    """
    pattern = r"DIVISION\s+([A-Z](?:-[A-Z])?)\s*[-—–]\s*(.+?)(?:\n|$)"
    matches = re.findall(pattern, text, re.IGNORECASE)
    return [{"letter": m[0], "title": m[1].strip()} for m in matches]


def try_parse_xml(xml_path: Path) -> ET.Element:
    """
    Try to parse XML, handling the DTD issues that congressional XML sometimes has.
    """
    content = xml_path.read_text(encoding="utf-8", errors="replace")

    # Remove DTD declaration which can cause issues
    content = re.sub(r"<!DOCTYPE[^>]*>", "", content)

    # Remove any processing instructions
    content = re.sub(r"<\?xml[^>]*\?>", "", content)

    # Add a dummy XML declaration
    content = '<?xml version="1.0" encoding="UTF-8"?>\n' + content.strip()

    try:
        return ET.fromstring(content)
    except ET.ParseError:
        # Try stripping more aggressively
        content = re.sub(r"<!ENTITY[^>]*>", "", content)
        content = re.sub(r"<!ELEMENT[^>]*>", "", content)
        content = re.sub(r"<!ATTLIST[^>]*>", "", content)
        content = re.sub(r"\[.*?\]>", ">", content, flags=re.DOTALL)
        return ET.fromstring(content)


def process_bill(bill_dir: str, xml_path: Path) -> dict:
    """Process a single bill XML and extract division information."""
    result = {
        "bill_dir": bill_dir,
        "xml_file": xml_path.name,
        "toc_divisions": [],
        "body_divisions": [],
        "enum_header_divisions": [],
        "text_fallback_divisions": [],
        "parse_error": None,
    }

    try:
        root = try_parse_xml(xml_path)
    except Exception as e:
        result["parse_error"] = str(e)[:200]
        # Try text fallback
        try:
            text = xml_path.read_text(encoding="utf-8", errors="replace")
            result["text_fallback_divisions"] = extract_division_letters_from_text(text)
        except Exception:
            pass
        return result

    ns = {}  # We handle namespaces manually by stripping them

    result["toc_divisions"] = extract_divisions_from_toc(root, ns)
    result["body_divisions"] = extract_divisions_from_headers(root, ns)
    result["enum_header_divisions"] = extract_divisions_from_enum_header(root)

    # Also try text fallback on the raw content
    try:
        text = xml_path.read_text(encoding="utf-8", errors="replace")
        result["text_fallback_divisions"] = extract_division_letters_from_text(text)
    except Exception:
        pass

    return result


def main():
    if not Path("examples/hr4366/extraction.json").exists():
        print("ERROR: Run from repository root (appropriations/)")
        sys.exit(1)

    print("=" * 80)
    print("XML DIVISION TITLE EXTRACTION TEST")
    print("=" * 80)

    # Find all XML files
    xml_files = []
    for bill_dir in sorted(os.listdir("examples")):
        bill_path = Path("examples") / bill_dir
        if not bill_path.is_dir():
            continue
        for f in sorted(bill_path.iterdir()):
            if f.suffix == ".xml" and f.name.startswith("BILLS-"):
                xml_files.append((bill_dir, f))

    print(f"\nFound {len(xml_files)} XML files across {len(set(d for d, _ in xml_files))} bills")

    # Process each bill
    all_results = []
    for bill_dir, xml_path in xml_files:
        result = process_bill(bill_dir, xml_path)
        all_results.append(result)

    # ── Report: What extraction methods work? ──
    print("\n--- Method comparison ---")
    print(f"  {'Bill':<12s} {'XML File':<35s} {'TOC':>4s} {'Body':>5s} {'Enum+Hdr':>9s} {'TextRE':>7s} {'Error'}")
    print(f"  {'─' * 12} {'─' * 35} {'─' * 4} {'─' * 5} {'─' * 9} {'─' * 7} {'─' * 20}")

    for r in all_results:
        error = "YES" if r["parse_error"] else ""
        print(
            f"  {r['bill_dir']:<12s} "
            f"{r['xml_file']:<35s} "
            f"{len(r['toc_divisions']):>4d} "
            f"{len(r['body_divisions']):>5d} "
            f"{len(r['enum_header_divisions']):>9d} "
            f"{len(r['text_fallback_divisions']):>7d} "
            f"{error}"
        )

    # ── Report: Extracted division titles ──
    print("\n--- Extracted division titles (best method per bill) ---")

    all_divisions = []  # (bill_dir, letter, title, method)

    for r in all_results:
        bill_dir = r["bill_dir"]

        # Prefer enum+header (has letter + title), then toc, then text fallback
        if r["enum_header_divisions"]:
            for d in r["enum_header_divisions"]:
                all_divisions.append((bill_dir, d["letter"], d["title"], "enum+header"))
        elif r["toc_divisions"]:
            for i, title in enumerate(r["toc_divisions"]):
                # Try to extract letter from title
                m = re.match(r"Division\s+([A-Z](?:-[A-Z])?)\s*[-—–:]\s*(.*)", title, re.IGNORECASE)
                if m:
                    all_divisions.append((bill_dir, m.group(1), m.group(2), "toc"))
                else:
                    all_divisions.append((bill_dir, f"#{i + 1}", title, "toc"))
        elif r["text_fallback_divisions"]:
            for d in r["text_fallback_divisions"]:
                all_divisions.append((bill_dir, d["letter"], d["title"], "text_regex"))

    # Print all divisions grouped by bill
    current_bill = None
    for bill_dir, letter, title, method in all_divisions:
        if bill_dir != current_bill:
            current_bill = bill_dir
            print(f"\n  {bill_dir}:")
        jurisdiction, confidence = classify_title(title)
        if confidence == "pattern_match":
            j_str = f"[{jurisdiction}]"
        else:
            j_str = "[???]"
        print(f"    Div {letter:5s} → {title[:60]:<60s} {j_str}")

    # ── Report: Classification results ──
    print("\n--- Jurisdiction classification via pattern matching ---")

    total = len(all_divisions)
    classified = sum(1 for _, _, title, _ in all_divisions if classify_title(title)[1] == "pattern_match")
    unclassified = total - classified
    unique_titles = set(title for _, _, title, _ in all_divisions)

    print(f"  Total divisions found: {total}")
    print(f"  Unique titles: {len(unique_titles)}")
    print(f"  Classified by pattern: {classified} ({classified / total * 100:.1f}%)" if total > 0 else "  No divisions found")
    print(f"  Unclassified (need LLM): {unclassified}")

    if unclassified > 0:
        print("\n  Unclassified division titles:")
        seen = set()
        for _, letter, title, _ in all_divisions:
            jurisdiction, confidence = classify_title(title)
            if confidence != "pattern_match" and title not in seen:
                seen.add(title)
                print(f"    \"{title}\"")

    # ── Report: Cross-reference with extraction.json divisions ──
    print("\n--- Cross-reference: XML divisions vs extraction.json divisions ---")

    for r in all_results:
        bill_dir = r["bill_dir"]
        ext_path = Path("examples") / bill_dir / "extraction.json"
        if not ext_path.exists():
            continue

        with open(ext_path) as f:
            ext = json.load(f)

        ext_divisions = ext.get("bill", {}).get("divisions", [])
        xml_div_count = len(r["enum_header_divisions"]) or len(r["toc_divisions"]) or len(r["text_fallback_divisions"])

        if ext_divisions or xml_div_count > 0:
            print(f"  {bill_dir}: extraction.json divisions={ext_divisions}  XML divisions found={xml_div_count}")

    # ── Summary ──
    print("\n" + "=" * 80)
    print("CONCLUSIONS")
    print("=" * 80)

    methods_that_work = []
    for r in all_results:
        if r["enum_header_divisions"]:
            methods_that_work.append("enum+header")
        elif r["toc_divisions"]:
            methods_that_work.append("toc")
        elif r["text_fallback_divisions"]:
            methods_that_work.append("text_regex")
        elif r["parse_error"]:
            methods_that_work.append("parse_error")
        else:
            methods_that_work.append("no_divisions")

    from collections import Counter
    method_counts = Counter(methods_that_work)

    print(f"\n  Extraction method success rates:")
    for method, count in method_counts.most_common():
        print(f"    {method:20s}: {count} bills")

    print(f"\n  Pattern classification rate: {classified}/{total} = {classified / total * 100:.1f}%" if total > 0 else "")

    if unclassified > 0 and total > 0:
        print(f"  Unclassified titles: {unclassified}/{total} = {unclassified / total * 100:.1f}%")
        print(f"  These {unclassified} titles would need LLM classification or manual mapping.")
    else:
        print("  All division titles classified by pattern matching alone.")

    print(f"\n  RECOMMENDATION:")
    print(f"    1. Parse division <enum> + <header> from XML (works for most bills)")
    print(f"    2. Pattern-match title text to jurisdiction (handles {classified}/{total} cases)")
    print(f"    3. LLM fallback for unrecognized titles ({unclassified} cases in current dataset)")
    print(f"    4. Do NOT use embedding exemplars for jurisdiction — use structured XML parsing")


# Need json for the cross-reference section
import json

if __name__ == "__main__":
    main()
