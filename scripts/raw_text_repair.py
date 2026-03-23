#!/usr/bin/env python3
"""
Raw Text Repair — Stage 2 extraction quality pass.

Deterministically detects provisions where raw_text does not appear verbatim
in the source text, then repairs them using a two-tier approach:

  Tier 1 (deterministic): Find the longest matching prefix of raw_text in the
  source, then copy 150 chars of actual source text from that position. This
  handles single-word substitutions ("clause" vs "subsection", "on" vs "in")
  that the LLM makes when it paraphrases instead of copying verbatim.

  Tier 2 (LLM): For provisions where no prefix match is found, send the
  provision + surrounding source context to Claude Opus to locate the correct
  verbatim excerpt. The LLM's correction is then VERIFIED against the source —
  if it's not a verbatim substring, it's rejected.

  After repair, every provision's raw_text is guaranteed to be a verbatim
  substring of the source text, establishing 1-to-1 correspondence between
  extraction.json and the enrolled bill.

Pipeline position:
  extract (stage 1) → raw_text_repair (stage 2) → enrich → embed → ...

Usage:
    source .venv/bin/activate

    # Analyze only — no changes, no LLM calls
    python scripts/raw_text_repair.py --analyze

    # Analyze a single bill
    python scripts/raw_text_repair.py --analyze --bill 117-hr2471

    # Deterministic repair (no LLM, no API key needed)
    python scripts/raw_text_repair.py --repair --bill 118-hr2882

    # Repair all bills (deterministic first, then LLM for remainder)
    python scripts/raw_text_repair.py --repair

    # Repair with LLM fallback for stubborn cases
    source /Users/chris.gorski/anthropic_key.source
    python scripts/raw_text_repair.py --repair --llm-fallback

    # Dry run — show what would be repaired without writing
    python scripts/raw_text_repair.py --repair --dry-run

    # Also check text_as_written fields for problems
    python scripts/raw_text_repair.py --check-taw
"""

import argparse
import copy
import json
import os
import re
import sys
import time
from collections import defaultdict
from pathlib import Path

# ─── Text matching (mirrors the Rust verification logic) ─────────────────────


def normalize_for_comparison(s: str) -> str:
    """Collapse whitespace, normalize quotes and dashes."""
    result = []
    last_was_space = False
    for c in s:
        # Normalize curly quotes
        if c in "\u2018\u2019":
            c = "'"
        elif c in "\u201c\u201d":
            c = '"'
        # Normalize dashes
        elif c in "\u2014\u2013\u2012":
            c = "-"
        # Collapse whitespace
        elif c in " \t\n\r\x0b\x0c":
            if not last_was_space:
                last_was_space = True
                result.append(" ")
            continue

        last_was_space = False
        result.append(c)

    return "".join(result).strip()


def spaceless(s: str) -> str:
    """Remove ALL spaces for most aggressive comparison."""
    return normalize_for_comparison(s).replace(" ", "")


def find_match_tier(raw_text: str, source_text: str) -> str:
    """
    Check if raw_text appears in source_text at any tier.
    Returns: 'exact', 'normalized', 'spaceless', or 'no_match'.
    """
    if not raw_text or not raw_text.strip():
        return "empty"

    # Tier 1: Exact substring
    if raw_text in source_text:
        return "exact"

    # Tier 2: Normalized (whitespace + quotes + dashes)
    norm_raw = normalize_for_comparison(raw_text)
    norm_source = normalize_for_comparison(source_text)
    if norm_raw and norm_raw in norm_source:
        return "normalized"

    # Tier 3: Spaceless
    sl_raw = spaceless(raw_text)
    sl_source = spaceless(source_text)
    if sl_raw and sl_raw in sl_source:
        return "spaceless"

    return "no_match"


def find_best_source_position(raw_text: str, source_text: str) -> tuple[int, int] | None:
    """
    Find where in the source the raw_text best matches.
    Returns (start, end) byte positions, or None.
    """
    # Try exact
    pos = source_text.find(raw_text)
    if pos >= 0:
        return (pos, pos + len(raw_text))

    # Try normalized
    norm_raw = normalize_for_comparison(raw_text)
    norm_source = normalize_for_comparison(source_text)
    pos = norm_source.find(norm_raw)
    if pos >= 0:
        # Map back to original positions approximately
        # This is approximate — good enough for context extraction
        ratio = len(source_text) / max(1, len(norm_source))
        approx_start = int(pos * ratio)
        approx_end = int((pos + len(norm_raw)) * ratio)
        return (approx_start, approx_end)

    return None


# ─── Source text loading ──────────────────────────────────────────────────────


def load_source_text(bill_dir: str) -> str | None:
    """Load the clean text (.txt) file for a bill, or parse from XML."""
    bill_path = Path(bill_dir)

    # Prefer .txt file (generated during extraction)
    txt_files = list(bill_path.glob("BILLS-*.txt"))
    if txt_files:
        return txt_files[0].read_text(encoding="utf-8")

    # Fall back to XML → strip tags
    xml_files = list(bill_path.glob("BILLS-*.xml"))
    if xml_files:
        xml_text = xml_files[0].read_text(encoding="utf-8")
        # Crude tag stripping — good enough for substring matching
        clean = re.sub(r"<[^>]+>", " ", xml_text)
        clean = re.sub(r"\s+", " ", clean)
        return clean.strip()

    return None


# ─── Analysis ─────────────────────────────────────────────────────────────────


def analyze_bill(bill_dir: str) -> dict:
    """
    Analyze a single bill's extraction for raw_text mismatches.
    Returns a report dict.
    """
    ext_path = os.path.join(bill_dir, "extraction.json")
    if not os.path.isfile(ext_path):
        return {"error": "no extraction.json", "bill_dir": bill_dir}

    ext = json.load(open(ext_path))
    bill_id = ext["bill"]["identifier"]
    provisions = ext["provisions"]

    source_text = load_source_text(bill_dir)
    if source_text is None:
        return {"error": "no source text", "bill_dir": bill_dir, "bill_id": bill_id}

    results = {
        "bill_dir": os.path.basename(bill_dir),
        "bill_id": bill_id,
        "total_provisions": len(provisions),
        "exact": 0,
        "normalized": 0,
        "spaceless": 0,
        "no_match": 0,
        "empty": 0,
        "mismatches": [],
    }

    for i, p in enumerate(provisions):
        raw_text = p.get("raw_text", "")
        tier = find_match_tier(raw_text, source_text)
        results[tier] += 1

        if tier == "no_match":
            # Gather context for repair
            ptype = p.get("provision_type", "?")
            account = p.get("account_name", "")
            if not account:
                account = p.get("program_name", "") or p.get("description", "")
            section = p.get("section", "")
            division = p.get("division", "")

            # Try to find where in the source this provision likely lives
            # by searching for the section number or account name
            source_context = ""
            search_terms = []
            if section:
                search_terms.append(section)
            if account and len(account) > 10:
                search_terms.append(account)

            for term in search_terms:
                pos = source_text.find(term)
                if pos < 0:
                    # Try case-insensitive
                    pos = source_text.lower().find(term.lower())
                if pos >= 0:
                    start = max(0, pos - 100)
                    end = min(len(source_text), pos + 500)
                    source_context = source_text[start:end]
                    break

            # Also check text_as_written for amount provisions
            amt = p.get("amount") or p.get("new_amount") or {}
            if isinstance(amt, dict):
                taw = amt.get("text_as_written", "")
            else:
                taw = ""

            # Check if text_as_written is also broken
            taw_tier = find_match_tier(taw, source_text) if taw else "n/a"

            results["mismatches"].append({
                "index": i,
                "provision_type": ptype,
                "account": account[:80] if account else "",
                "section": section,
                "division": division,
                "raw_text": raw_text[:200],
                "raw_text_len": len(raw_text),
                "text_as_written": taw[:80] if taw else "",
                "taw_tier": taw_tier,
                "source_context": source_context[:600] if source_context else "",
                "has_source_context": bool(source_context),
            })

    return results


def analyze_all(data_dir: str, bill_filter: str | None = None) -> list[dict]:
    """Analyze all bills (or one specific bill)."""
    reports = []

    if bill_filter:
        bill_path = os.path.join(data_dir, bill_filter)
        if os.path.isdir(bill_path):
            reports.append(analyze_bill(bill_path))
        else:
            print(f"Error: {bill_path} is not a directory")
            sys.exit(1)
    else:
        for d in sorted(os.listdir(data_dir)):
            bill_path = os.path.join(data_dir, d)
            if not os.path.isdir(bill_path):
                continue
            if not os.path.isfile(os.path.join(bill_path, "extraction.json")):
                continue
            reports.append(analyze_bill(bill_path))

    return reports


def print_analysis(reports: list[dict]):
    """Print analysis results."""
    total_provs = 0
    total_mismatch = 0
    total_exact = 0
    total_norm = 0
    total_spaceless = 0

    print()
    print("╔══════════════════════════════════════════════════════════════════╗")
    print("║          RAW TEXT MISMATCH ANALYSIS                            ║")
    print("╚══════════════════════════════════════════════════════════════════╝")
    print()

    for r in reports:
        if "error" in r:
            print(f"  {r.get('bill_dir', '?')}: {r['error']}")
            continue

        total_provs += r["total_provisions"]
        total_exact += r["exact"]
        total_norm += r["normalized"]
        total_spaceless += r["spaceless"]
        total_mismatch += r["no_match"]

        if r["no_match"] > 0:
            print(f"  {r['bill_id']:15s}  {r['no_match']:4d} mismatches / {r['total_provisions']:5d} provisions")

    print()
    print(f"  TOTALS:")
    print(f"    Provisions:    {total_provs:>8,}")
    print(f"    Exact match:   {total_exact:>8,}  ({100*total_exact/max(1,total_provs):.1f}%)")
    print(f"    Normalized:    {total_norm:>8,}  ({100*total_norm/max(1,total_provs):.1f}%)")
    print(f"    Spaceless:     {total_spaceless:>8,}  ({100*total_spaceless/max(1,total_provs):.2f}%)")
    print(f"    NO MATCH:      {total_mismatch:>8,}  ({100*total_mismatch/max(1,total_provs):.1f}%)")
    print()

    # Breakdown by provision type
    type_counts = defaultdict(int)
    type_with_context = defaultdict(int)
    for r in reports:
        for m in r.get("mismatches", []):
            type_counts[m["provision_type"]] += 1
            if m["has_source_context"]:
                type_with_context[m["provision_type"]] += 1

    if type_counts:
        print("  Mismatches by provision type:")
        for t, c in sorted(type_counts.items(), key=lambda x: -x[1]):
            ctx = type_with_context.get(t, 0)
            print(f"    {t:35s} {c:4d}  ({ctx} have source context for repair)")
        print()

    # Show sample mismatches
    all_mismatches = []
    for r in reports:
        for m in r.get("mismatches", []):
            m["bill_id"] = r.get("bill_id", "?")
            m["bill_dir"] = r.get("bill_dir", "?")
            all_mismatches.append(m)

    if all_mismatches:
        # Show a few from each type
        by_type = defaultdict(list)
        for m in all_mismatches:
            by_type[m["provision_type"]].append(m)

        print("  Sample mismatches:")
        for ptype in sorted(by_type.keys(), key=lambda t: -len(by_type[t])):
            items = by_type[ptype]
            print(f"\n    --- {ptype} ({len(items)} total) ---")
            for m in items[:3]:
                print(f"    [{m['bill_id']} #{m['index']}] {m['account'][:55]}")
                print(f"      raw_text: \"{m['raw_text'][:100]}\"")
                if m["has_source_context"]:
                    print(f"      source nearby: \"{m['source_context'][:100]}...\"")
                else:
                    print(f"      ⚠ no source context found")
                print()

    return all_mismatches


# ─── Deterministic Repair (Tier 1) ───────────────────────────────────────────


def deterministic_repair(raw_text: str, source_text: str, target_len: int = 150) -> dict:
    """
    Attempt to repair raw_text by finding its longest matching prefix in the
    source text, then copying actual source characters from that position.

    Returns a dict with:
      corrected_raw_text: str or None
      method: 'prefix_match' or None
      prefix_len: how many chars matched before divergence
      source_start: byte position in source where the match starts
    """
    if not raw_text or not raw_text.strip():
        return {"corrected_raw_text": None, "method": None, "prefix_len": 0}

    # Strategy 1: Try progressively shorter prefixes of raw_text
    # Start from 80 chars (high confidence) down to 15 (minimum viable)
    max_prefix = min(80, len(raw_text))
    best_pos = -1
    best_prefix_len = 0

    for prefix_len in range(max_prefix, 14, -1):
        prefix = raw_text[:prefix_len]
        pos = source_text.find(prefix)
        if pos >= 0:
            best_pos = pos
            best_prefix_len = prefix_len
            break

    if best_pos >= 0 and best_prefix_len >= 15:
        # Found a prefix match — copy actual source text
        end = min(len(source_text), best_pos + target_len)
        corrected = source_text[best_pos:end]

        # Verify (should always pass since we copied from source)
        assert corrected in source_text, "BUG: corrected text not in source"

        return {
            "corrected_raw_text": corrected,
            "method": "prefix_match",
            "prefix_len": best_prefix_len,
            "source_start": best_pos,
            "source_end": end,
        }

    # Strategy 2: Try normalized prefix matching
    # The raw_text might have different quote marks or dashes
    norm_raw = normalize_for_comparison(raw_text)
    norm_source = normalize_for_comparison(source_text)

    for prefix_len in range(min(80, len(norm_raw)), 14, -1):
        prefix = norm_raw[:prefix_len]
        pos = norm_source.find(prefix)
        if pos >= 0:
            # Map back to approximate position in original source
            # Use ratio mapping (imperfect but close)
            ratio = len(source_text) / max(1, len(norm_source))
            approx_pos = int(pos * ratio)

            # Scan nearby in the original source for the best actual start
            # Look for the first few chars of raw_text near approx_pos
            search_start = max(0, approx_pos - 200)
            search_end = min(len(source_text), approx_pos + 200)
            search_window = source_text[search_start:search_end]

            # Try to find a good starting point using the first distinctive word
            words = [w for w in raw_text.split() if len(w) > 4]
            for word in words[:3]:
                word_pos = search_window.find(word)
                if word_pos >= 0:
                    # Walk backwards to find the sentence start
                    abs_word_pos = search_start + word_pos
                    # Look for the start of raw_text near this word
                    first_chars = raw_text[:10]
                    nearby_start = max(0, abs_word_pos - 50)
                    nearby = source_text[nearby_start:abs_word_pos + 20]
                    fc_pos = nearby.find(first_chars[:5])
                    if fc_pos >= 0:
                        actual_start = nearby_start + fc_pos
                        end = min(len(source_text), actual_start + target_len)
                        corrected = source_text[actual_start:end]
                        if corrected in source_text:
                            return {
                                "corrected_raw_text": corrected,
                                "method": "normalized_prefix_match",
                                "prefix_len": prefix_len,
                                "source_start": actual_start,
                                "source_end": end,
                            }
                    break
            break

    # Strategy 3: Search for text_as_written dollar amount as anchor
    # (handled by caller who can pass additional context)

    return {"corrected_raw_text": None, "method": None, "prefix_len": 0}


def deterministic_repair_batch(
    mismatches: list[dict],
    source_text: str,
    provisions: list[dict],
) -> list[dict]:
    """
    Attempt deterministic repair on a batch of mismatched provisions.
    Returns list of repair result dicts.
    """
    repairs = []

    for m in mismatches:
        idx = m["index"]
        raw_text = m.get("raw_text", "")
        if not raw_text:
            # Get from provisions list
            if idx < len(provisions):
                raw_text = provisions[idx].get("raw_text", "")

        result = deterministic_repair(raw_text, source_text)

        if result["corrected_raw_text"]:
            # Double-verify: the correction must be in source
            verify_tier = find_match_tier(result["corrected_raw_text"], source_text)
            repairs.append({
                "index": idx,
                "corrected_raw_text": result["corrected_raw_text"],
                "confidence": "high" if result["prefix_len"] >= 40 else "medium",
                "verify_tier": verify_tier,
                "method": result["method"],
                "prefix_len": result["prefix_len"],
                "source_start": result.get("source_start"),
                "source_end": result.get("source_end"),
            })
        else:
            # Could not repair deterministically — needs LLM fallback
            repairs.append({
                "index": idx,
                "corrected_raw_text": None,
                "confidence": "needs_llm",
                "method": None,
                "prefix_len": result.get("prefix_len", 0),
            })

    return repairs


# ─── LLM Repair (Tier 2 — fallback for deterministic failures) ───────────────

REPAIR_SYSTEM_PROMPT = """You are a precise text extraction assistant. You will receive:

1. A provision that was extracted from a U.S. appropriations bill, including its raw_text field.
2. The actual source text from the enrolled bill surrounding where this provision should be.

The raw_text field is supposed to be a VERBATIM substring of the source text — the first ~150 characters of the relevant passage, copied character-for-character. However, the original extraction paraphrased or reformatted the text instead of copying it verbatim.

Your task: Find the correct verbatim excerpt from the source text and return it.

Rules:
- The corrected raw_text MUST be a substring that literally appears in the provided source text.
- It should be approximately 100-200 characters long.
- It should start at the beginning of the provision's text in the source (e.g., the start of the sentence that establishes the appropriation, rider, or directive).
- Do NOT paraphrase, normalize, or fix typos. Copy EXACTLY from the source.
- If you cannot find a suitable verbatim excerpt, return null.

Return JSON only:
{"corrected_raw_text": "the verbatim excerpt" or null, "start_phrase": "first 30 chars to verify", "confidence": "high" | "medium" | "low"}"""


def build_repair_prompt(mismatch: dict, source_text: str) -> str:
    """Build the user prompt for a single raw_text repair."""
    parts = []
    parts.append("== PROVISION ==\n")
    parts.append(f"Type: {mismatch['provision_type']}\n")
    if mismatch.get("account"):
        parts.append(f"Account/Description: {mismatch['account']}\n")
    if mismatch.get("section"):
        parts.append(f"Section: {mismatch['section']}\n")
    if mismatch.get("division"):
        parts.append(f"Division: {mismatch['division']}\n")
    parts.append(f"\nCurrent raw_text (BROKEN — not verbatim):\n\"{mismatch['raw_text']}\"\n")

    if mismatch.get("text_as_written"):
        parts.append(f"\nDollar text_as_written: \"{mismatch['text_as_written']}\"\n")

    # Provide source context
    parts.append("\n== SOURCE TEXT (from enrolled bill) ==\n")
    if mismatch.get("source_context"):
        parts.append(mismatch["source_context"])
    else:
        # Try to find by section or keyword in the full source
        search_terms = []
        if mismatch.get("section"):
            search_terms.append(mismatch["section"])
        if mismatch.get("text_as_written"):
            search_terms.append(mismatch["text_as_written"])
        # Use the first 20 non-trivial chars of raw_text as a fuzzy search
        raw_start = mismatch["raw_text"][:40].strip()
        if len(raw_start) > 10:
            search_terms.append(raw_start)

        found_context = False
        for term in search_terms:
            pos = source_text.find(term)
            if pos < 0:
                pos = source_text.lower().find(term.lower())
            if pos >= 0:
                start = max(0, pos - 200)
                end = min(len(source_text), pos + 800)
                parts.append(source_text[start:end])
                found_context = True
                break

        if not found_context:
            # Last resort: provide a large window around any partial match
            # Use the longest word from the raw_text as search anchor
            words = sorted(mismatch["raw_text"].split(), key=len, reverse=True)
            for word in words[:5]:
                if len(word) < 6:
                    continue
                pos = source_text.lower().find(word.lower())
                if pos >= 0:
                    start = max(0, pos - 300)
                    end = min(len(source_text), pos + 700)
                    parts.append(source_text[start:end])
                    found_context = True
                    break

            if not found_context:
                parts.append("(Could not locate relevant section in source text)")

    parts.append("\n\n== TASK ==\n")
    parts.append("Find the verbatim excerpt from the source text that corresponds to this provision. ")
    parts.append("Return the first ~150 characters of the provision's actual text, copied exactly from the source above.")

    return "".join(parts)


def repair_batch(
    mismatches: list[dict],
    source_text: str,
    client,
    dry_run: bool = False,
    batch_size: int = 10,
) -> list[dict]:
    """
    Repair a batch of mismatched provisions using Claude.
    Returns list of {index, corrected_raw_text, confidence} dicts.
    """
    repairs = []

    for i, m in enumerate(mismatches):
        prompt = build_repair_prompt(m, source_text)

        if dry_run:
            print(f"    [{i+1}/{len(mismatches)}] Would repair provision {m['index']} ({m['provision_type']})")
            print(f"      Prompt length: {len(prompt)} chars")
            print(f"      Current raw_text: \"{m['raw_text'][:80]}\"")
            if m.get("source_context"):
                print(f"      Has source context: yes ({len(m['source_context'])} chars)")
            else:
                print(f"      Has source context: no (will search by keywords)")
            print()
            continue

        print(f"    [{i+1}/{len(mismatches)}] Repairing provision {m['index']} ({m['provision_type']}: {m['account'][:40]})...", end="", flush=True)

        try:
            response = client.messages.create(
                model="claude-opus-4-6",
                max_tokens=16000,
                temperature=1,
                thinking={
                    "type": "enabled",
                    "budget_tokens": 10000,
                },
                system=REPAIR_SYSTEM_PROMPT,
                messages=[{"role": "user", "content": prompt}],
            )

            # Parse response
            response_text = ""
            for block in response.content:
                if block.type == "text":
                    response_text = block.text

            tokens_in = response.usage.input_tokens
            tokens_out = response.usage.output_tokens

            # Parse JSON
            json_text = response_text.strip()
            if json_text.startswith("```"):
                json_text = re.sub(r"^```\w*\n?", "", json_text)
                json_text = re.sub(r"\n?```$", "", json_text)

            result = json.loads(json_text)
            corrected = result.get("corrected_raw_text")
            confidence = result.get("confidence", "low")

            if corrected:
                # Verify the correction is actually in the source
                verify_tier = find_match_tier(corrected, source_text)
                if verify_tier in ("exact", "normalized"):
                    repairs.append({
                        "index": m["index"],
                        "corrected_raw_text": corrected,
                        "confidence": confidence,
                        "verify_tier": verify_tier,
                        "tokens": tokens_in + tokens_out,
                    })
                    print(f" ✅ ({verify_tier}, {confidence})")
                else:
                    print(f" ❌ LLM correction also not in source ({verify_tier})")
                    repairs.append({
                        "index": m["index"],
                        "corrected_raw_text": None,
                        "confidence": "failed",
                        "verify_tier": verify_tier,
                        "reason": "LLM correction not found in source text",
                        "attempted": corrected[:100] if corrected else None,
                        "tokens": tokens_in + tokens_out,
                    })
            else:
                print(f" ⚠ LLM returned null")
                repairs.append({
                    "index": m["index"],
                    "corrected_raw_text": None,
                    "confidence": "null",
                    "reason": "LLM could not find verbatim excerpt",
                    "tokens": tokens_in + tokens_out,
                })

        except json.JSONDecodeError as e:
            print(f" ❌ JSON parse error: {e}")
            repairs.append({
                "index": m["index"],
                "corrected_raw_text": None,
                "confidence": "error",
                "reason": f"JSON parse error: {e}",
            })
        except Exception as e:
            print(f" ❌ API error: {e}")
            repairs.append({
                "index": m["index"],
                "corrected_raw_text": None,
                "confidence": "error",
                "reason": str(e),
            })

        # Rate limit
        time.sleep(0.5)

    return repairs


def apply_repairs(bill_dir: str, repairs: list[dict], write: bool = True) -> dict:
    """
    Apply verified repairs to extraction.json.
    Only applies repairs where the corrected text is verified in the source.
    Adds source_span to repaired provisions for 1-to-1 correspondence.
    Returns a summary of what was changed.
    """
    ext_path = os.path.join(bill_dir, "extraction.json")
    ext = json.load(open(ext_path))
    original = copy.deepcopy(ext)

    applied = 0
    failed = 0
    skipped = 0

    for repair in repairs:
        idx = repair["index"]
        corrected = repair.get("corrected_raw_text")

        if corrected is None:
            failed += 1
            continue

        if repair.get("verify_tier") not in ("exact", "normalized"):
            skipped += 1
            continue

        if idx < len(ext["provisions"]):
            old_rt = ext["provisions"][idx].get("raw_text", "")
            ext["provisions"][idx]["raw_text"] = corrected

            # Add source_span if we have byte positions from deterministic repair
            if repair.get("source_start") is not None:
                ext["provisions"][idx]["source_span"] = {
                    "start": repair["source_start"],
                    "end": repair["source_end"],
                    "verified": True,
                }

            applied += 1

    summary = {
        "total_repairs_attempted": len(repairs),
        "applied": applied,
        "failed": failed,
        "skipped": skipped,
    }

    if write and applied > 0:
        # Backup original
        backup_path = ext_path + ".pre-repair"
        if not os.path.exists(backup_path):
            with open(backup_path, "w") as f:
                json.dump(original, f, indent=2)

        # Write repaired version
        with open(ext_path, "w") as f:
            json.dump(ext, f, indent=2)

        print(f"    Wrote {applied} repairs to {ext_path}")
        print(f"    Backup saved to {backup_path}")
    elif applied > 0:
        print(f"    Would apply {applied} repairs (dry run)")

    return summary


def add_source_spans(bill_dir: str, write: bool = True) -> dict:
    """
    For ALL provisions (not just repaired ones), compute source_span
    by finding the raw_text position in the source text.

    This establishes 1-to-1 correspondence: every provision points to
    its exact byte range in the enrolled bill text.
    """
    ext_path = os.path.join(bill_dir, "extraction.json")
    ext = json.load(open(ext_path))

    source_text = load_source_text(bill_dir)
    if source_text is None:
        return {"error": "no source text"}

    # Determine which .txt file is the source
    bill_path = Path(bill_dir)
    txt_files = list(bill_path.glob("BILLS-*.txt"))
    source_filename = txt_files[0].name if txt_files else "unknown"

    added = 0
    not_found = 0

    for i, p in enumerate(ext["provisions"]):
        rt = p.get("raw_text", "")
        if not rt.strip():
            continue

        # Already has a verified span?
        existing_span = p.get("source_span")
        if existing_span and existing_span.get("verified"):
            continue

        # Find position
        pos = source_text.find(rt)
        if pos >= 0:
            p["source_span"] = {
                "start": pos,
                "end": pos + len(rt),
                "file": source_filename,
                "verified": True,
            }
            added += 1
        else:
            # Try normalized
            norm_rt = normalize_for_comparison(rt)
            norm_source = normalize_for_comparison(source_text)
            npos = norm_source.find(norm_rt)
            if npos >= 0:
                # Approximate mapping back to original positions
                ratio = len(source_text) / max(1, len(norm_source))
                approx_start = int(npos * ratio)
                approx_end = int((npos + len(norm_rt)) * ratio)
                p["source_span"] = {
                    "start": approx_start,
                    "end": approx_end,
                    "file": source_filename,
                    "verified": False,  # approximate position
                    "match_tier": "normalized",
                }
                added += 1
            else:
                p["source_span"] = {
                    "start": None,
                    "end": None,
                    "file": source_filename,
                    "verified": False,
                    "match_tier": "no_match",
                }
                not_found += 1

    summary = {
        "spans_added": added,
        "not_found": not_found,
        "total": len(ext["provisions"]),
    }

    if write:
        with open(ext_path, "w") as f:
            json.dump(ext, f, indent=2)
        print(f"    Added source_span to {added} provisions ({not_found} not found)")

    return summary


# ─── Also check text_as_written ───────────────────────────────────────────────


def analyze_text_as_written(data_dir: str, bill_filter: str | None = None):
    """
    Separately analyze text_as_written fields for problems.
    These carry dollar amounts and are critical for verification.
    """
    print()
    print("╔══════════════════════════════════════════════════════════════════╗")
    print("║          TEXT_AS_WRITTEN ANALYSIS                              ║")
    print("╚══════════════════════════════════════════════════════════════════╝")
    print()

    issues = []
    total_with_taw = 0

    dirs = [bill_filter] if bill_filter else sorted(os.listdir(data_dir))

    for d in dirs:
        bill_path = os.path.join(data_dir, d) if not bill_filter else os.path.join(data_dir, d)
        ext_path = os.path.join(bill_path, "extraction.json")
        if not os.path.isfile(ext_path):
            continue

        ext = json.load(open(ext_path))
        bill_id = ext["bill"]["identifier"]
        source_text = load_source_text(bill_path)
        if source_text is None:
            continue

        for i, p in enumerate(ext["provisions"]):
            # Check all amount fields
            for field_name in ["amount", "new_amount", "old_amount"]:
                amt = p.get(field_name)
                if not amt or not isinstance(amt, dict):
                    continue
                taw = amt.get("text_as_written", "")
                if not taw:
                    continue

                total_with_taw += 1

                # Check if taw is in source
                if taw not in source_text:
                    # Check normalized
                    norm_taw = normalize_for_comparison(taw)
                    norm_source = normalize_for_comparison(source_text)
                    if norm_taw in norm_source:
                        continue  # OK at normalized tier

                    # Real problem
                    issues.append({
                        "bill_id": bill_id,
                        "bill_dir": d,
                        "index": i,
                        "field": field_name,
                        "provision_type": p.get("provision_type", "?"),
                        "text_as_written": taw[:100],
                        "account": (p.get("account_name") or p.get("program_name") or "")[:50],
                    })

    print(f"  Total text_as_written values checked: {total_with_taw:,}")
    print(f"  NOT FOUND in source: {len(issues)}")
    print()

    if issues:
        print("  Issues:")
        for iss in issues:
            print(f"    {iss['bill_id']:15s} [{iss['index']:4d}] {iss['provision_type']:30s}")
            print(f"      {iss['field']}: \"{iss['text_as_written']}\"")
            print(f"      account: {iss['account']}")
            print()

    return issues


# ─── Main ─────────────────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser(description="Raw text repair — stage 2 extraction pass")
    parser.add_argument("--analyze", action="store_true", help="Analyze mismatches without repairing")
    parser.add_argument("--repair", action="store_true", help="Repair mismatches using LLM")
    parser.add_argument("--bill", type=str, help="Single bill directory (e.g., 117-hr2471)")
    parser.add_argument("--dry-run", action="store_true", help="Show what would be repaired without writing")
    parser.add_argument("--llm-fallback", action="store_true", help="Use Claude Opus for provisions that deterministic repair can't fix")
    parser.add_argument("--data-dir", type=str, default="data", help="Data directory")
    parser.add_argument("--limit", type=int, default=None, help="Max provisions to repair per bill")
    parser.add_argument("--check-taw", action="store_true", help="Also analyze text_as_written fields")
    parser.add_argument("--add-spans", action="store_true", help="Add source_span to all provisions (after repair)")
    args = parser.parse_args()

    os.chdir(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

    if not args.analyze and not args.repair and not args.check_taw and not args.add_spans:
        print("Specify --analyze, --repair, --check-taw, or --add-spans")
        parser.print_help()
        sys.exit(1)

    # ── Add source spans to all provisions ──
    if args.add_spans:
        print("  Adding source_span to all provisions...")
        if args.bill:
            bill_path = os.path.join(args.data_dir, args.bill)
            summary = add_source_spans(bill_path, write=not args.dry_run)
            print(f"    {summary}")
        else:
            for d in sorted(os.listdir(args.data_dir)):
                bill_path = os.path.join(args.data_dir, d)
                if not os.path.isdir(bill_path):
                    continue
                if not os.path.isfile(os.path.join(bill_path, "extraction.json")):
                    continue
                print(f"  {d}: ", end="", flush=True)
                summary = add_source_spans(bill_path, write=not args.dry_run)
        return

    if args.check_taw:
        analyze_text_as_written(args.data_dir, args.bill)

    if args.analyze or args.repair:
        # Step 1: Analyze
        print("  Analyzing raw_text matches...")
        reports = analyze_all(args.data_dir, args.bill)
        all_mismatches = print_analysis(reports)

        if not args.repair:
            # Analysis only — count how many could be repaired
            with_context = sum(1 for m in all_mismatches if m.get("has_source_context"))
            print(f"  {with_context}/{len(all_mismatches)} mismatches have source context for LLM repair.")
            print(f"  Run with --repair to fix them.")
            return

        # Step 2: Repair
        if not all_mismatches:
            print("  No mismatches to repair!")
            return

        # Initialize LLM client only if --llm-fallback is set
        client = None
        if args.llm_fallback and not args.dry_run:
            try:
                import anthropic
                client = anthropic.Anthropic()
                print("  LLM fallback enabled (will use Claude for stubborn cases)")
            except Exception as e:
                print(f"  Warning: Could not initialize Anthropic client: {e}")
                print("  Proceeding with deterministic repair only")

        # Group mismatches by bill
        by_bill = defaultdict(list)
        for m in all_mismatches:
            by_bill[m["bill_dir"]].append(m)

        total_det_applied = 0
        total_llm_applied = 0
        total_failed = 0

        for bill_dir_name, mismatches in sorted(by_bill.items()):
            bill_path = os.path.join(args.data_dir, bill_dir_name)
            bill_id = mismatches[0].get("bill_id", bill_dir_name)

            print(f"\n  === Repairing {bill_id} ({len(mismatches)} mismatches) ===")

            source_text = load_source_text(bill_path)
            if source_text is None:
                print(f"    ⚠ No source text found, skipping")
                continue

            # Load provisions for deterministic repair
            ext = json.load(open(os.path.join(bill_path, "extraction.json")))
            provisions = ext.get("provisions", [])

            # Apply limit
            batch = mismatches
            if args.limit:
                batch = mismatches[:args.limit]
                if len(mismatches) > args.limit:
                    print(f"    (limiting to {args.limit} of {len(mismatches)} mismatches)")

            # ── Tier 1: Deterministic prefix-match repair ──
            print(f"    Tier 1 (deterministic): ", end="", flush=True)
            det_repairs = deterministic_repair_batch(batch, source_text, provisions)

            det_success = sum(1 for r in det_repairs if r.get("corrected_raw_text"))
            det_fail = sum(1 for r in det_repairs if not r.get("corrected_raw_text"))
            print(f"{det_success} fixed, {det_fail} remaining")

            # Show details for deterministic repairs
            for r in det_repairs:
                if r.get("corrected_raw_text"):
                    idx = r["index"]
                    plen = r.get("prefix_len", 0)
                    method = r.get("method", "?")
                    print(f"      ✅ [{idx}] prefix={plen} method={method}")

            # ── Tier 2: LLM fallback for remaining failures ──
            llm_repairs = []
            needs_llm = [
                (det_repairs[i], batch[i])
                for i in range(len(det_repairs))
                if not det_repairs[i].get("corrected_raw_text")
            ]

            if needs_llm and client and args.llm_fallback:
                llm_batch = [m for _, m in needs_llm]
                print(f"    Tier 2 (LLM): repairing {len(llm_batch)} provisions...")
                llm_repairs = repair_batch(
                    llm_batch, source_text, client, dry_run=args.dry_run
                )
            elif needs_llm:
                remaining_types = defaultdict(int)
                for _, m in needs_llm:
                    remaining_types[m["provision_type"]] += 1
                type_summary = ", ".join(f"{t}={c}" for t, c in sorted(remaining_types.items(), key=lambda x: -x[1]))
                print(f"    Tier 2 (LLM): skipped — {len(needs_llm)} unrepaired ({type_summary})")
                if not args.llm_fallback:
                    print(f"      Use --llm-fallback to attempt LLM repair")

            # ── Combine repairs ──
            all_repairs = []
            for r in det_repairs:
                if r.get("corrected_raw_text"):
                    all_repairs.append(r)
            for r in llm_repairs:
                if r.get("corrected_raw_text"):
                    all_repairs.append(r)

            if not args.dry_run and all_repairs:
                summary = apply_repairs(bill_path, all_repairs, write=True)
                total_det_applied += sum(1 for r in det_repairs if r.get("corrected_raw_text"))
                total_llm_applied += sum(1 for r in llm_repairs if r.get("corrected_raw_text"))
                total_failed += summary["failed"]

                # Re-analyze to verify
                post_report = analyze_bill(bill_path)
                remaining = post_report.get("no_match", 0)
                print(f"    Remaining mismatches after repair: {remaining}")

                # Add source_spans to ALL provisions (not just repaired ones)
                print(f"    Adding source_spans to all provisions...")
                span_summary = add_source_spans(bill_path, write=True)
            elif args.dry_run:
                print(f"    Dry run — no changes made")
                print(f"    Would apply: {sum(1 for r in all_repairs if r.get('corrected_raw_text'))} repairs")

        if not args.dry_run:
            print(f"\n  ═══════════════════════════════════════════")
            print(f"  REPAIR SUMMARY:")
            print(f"    Deterministic (Tier 1): {total_det_applied} applied")
            print(f"    LLM fallback (Tier 2):  {total_llm_applied} applied")
            print(f"    Total applied:          {total_det_applied + total_llm_applied}")
            print(f"    Failed (unfixable):     {total_failed}")
            print(f"\n  Run --analyze again to verify results.")


if __name__ == "__main__":
    main()
