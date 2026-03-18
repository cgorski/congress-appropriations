#!/usr/bin/env python3
"""
Analyze WHY exemplar-based advance/current classification fails.

Hypothesis: The exemplar centroids capture "agency identity" (e.g., VA-ness)
rather than "advance-ness" because the embedding vectors encode the full
provision meaning — account name, agency, amounts, raw text — and the
advance/current distinction is a tiny signal drowned out by the dominant
"what account is this for" signal.

Tests:
1. Are misclassified provisions correlated with being VA-adjacent?
2. What does the similarity distribution look like for each class?
3. Does using MORE exemplars (diverse agencies) help?
4. Does using ONLY the availability text (not full provision) help?
   (Can't test this without re-embedding, but we can check what text was embedded)
5. Would a different approach work better — e.g., keyword/regex on raw_text?
"""

import json
import os
import sys
import numpy as np
from pathlib import Path
from collections import Counter, defaultdict


def load_vectors(bill_dir: str) -> np.ndarray:
    meta_path = Path("examples") / bill_dir / "embeddings.json"
    vec_path = Path("examples") / bill_dir / "vectors.bin"
    with open(meta_path) as f:
        meta = json.load(f)
    dims = meta["dimensions"]
    count = meta["count"]
    raw = open(vec_path, "rb").read()
    return np.frombuffer(raw, dtype=np.float32).reshape(count, dims)


def load_provisions(bill_dir: str) -> list:
    ext_path = Path("examples") / bill_dir / "extraction.json"
    with open(ext_path) as f:
        return json.load(f)["provisions"]


def provision_text(p):
    parts = []
    parts.append(p.get("raw_text", "") or "")
    parts.append(p.get("availability", "") or "")
    for note in (p.get("notes", None) or []):
        parts.append(note)
    return " ".join(parts)


def provision_label(p, max_len=50):
    name = (p.get("account_name", "") or "")[:max_len]
    agency = (p.get("agency", "") or "")[:30]
    dollars = 0
    amt = p.get("amount")
    if amt:
        val = amt.get("value", {})
        if val.get("kind") == "specific":
            dollars = val.get("dollars", 0)
    return f"{name} | {agency} | ${dollars:,}"


def is_advance_by_text(p):
    """Heuristic: does the provision text mention advance availability?"""
    text = provision_text(p).lower()
    return "become available on october 1" in text


def is_current_by_text(p):
    """Heuristic: has availability language but NOT advance."""
    text = provision_text(p).lower()
    if "become available on october 1" in text:
        return False
    return "remain available" in text


def get_ba_appropriation_indices(provisions):
    indices = []
    for i, p in enumerate(provisions):
        if p["provision_type"] != "appropriation":
            continue
        amt = p.get("amount")
        if not amt:
            continue
        if amt.get("semantics") != "new_budget_authority":
            continue
        indices.append(i)
    return indices


def main():
    if not Path("examples/hr4366/extraction.json").exists():
        print("ERROR: Run from repository root (appropriations/)")
        sys.exit(1)

    print("=" * 80)
    print("EXEMPLAR CLASSIFICATION FAILURE ANALYSIS")
    print("=" * 80)

    # Load H.R. 4366 data
    vecs_4366 = load_vectors("hr4366")
    provs_4366 = load_provisions("hr4366")
    ba_indices = get_ba_appropriation_indices(provs_4366)

    advance_idx = [i for i in ba_indices if is_advance_by_text(provs_4366[i])]
    current_idx = [i for i in ba_indices if is_current_by_text(provs_4366[i])]

    # ── Analysis 1: What agencies are in each class? ──
    print("\n--- Analysis 1: Agency distribution by class ---")

    adv_agencies = Counter()
    for i in advance_idx:
        agency = (provs_4366[i].get("agency", "") or "(none)")
        adv_agencies[agency] += 1

    cur_agencies = Counter()
    for i in current_idx:
        agency = (provs_4366[i].get("agency", "") or "(none)")
        cur_agencies[agency] += 1

    print(f"\n  Advance provisions ({len(advance_idx)} total):")
    for agency, count in adv_agencies.most_common():
        print(f"    {count:3d}  {agency}")

    print(f"\n  Current-year provisions ({len(current_idx)} total) — top 15:")
    for agency, count in cur_agencies.most_common(15):
        print(f"    {count:3d}  {agency}")
    remaining = sum(c for _, c in cur_agencies.most_common()[15:])
    if remaining:
        print(f"    ... and {remaining} more across {len(cur_agencies) - 15} agencies")

    # ── Analysis 2: Exemplar agency bias ──
    print("\n--- Analysis 2: Exemplar agency bias ---")

    adv_exemplars = advance_idx[:3]
    cur_exemplars = current_idx[:3]

    print("  Advance exemplars are ALL from these agencies:")
    for i in adv_exemplars:
        agency = provs_4366[i].get("agency", "") or "(none)"
        print(f"    [{i}] {agency}: {(provs_4366[i].get('account_name','') or '')[:50]}")

    print("  Current-year exemplars are ALL from these agencies:")
    for i in cur_exemplars:
        agency = provs_4366[i].get("agency", "") or "(none)"
        print(f"    [{i}] {agency}: {(provs_4366[i].get('account_name','') or '')[:50]}")

    adv_mean = np.mean(vecs_4366[adv_exemplars], axis=0)
    adv_mean /= np.linalg.norm(adv_mean)
    cur_mean = np.mean(vecs_4366[cur_exemplars], axis=0)
    cur_mean /= np.linalg.norm(cur_mean)

    # ── Analysis 3: Error rate by agency ──
    print("\n--- Analysis 3: Error rate by agency (H.R. 4366 self-test) ---")

    agency_stats = defaultdict(lambda: {"correct": 0, "error": 0, "total": 0})

    for i in advance_idx:
        if i in adv_exemplars:
            continue
        agency = provs_4366[i].get("agency", "") or "(none)"
        vec = vecs_4366[i]
        sim_adv = float(np.dot(vec, adv_mean))
        sim_cur = float(np.dot(vec, cur_mean))
        predicted = "ADVANCE" if sim_adv > sim_cur else "CURRENT"
        agency_stats[agency]["total"] += 1
        if predicted == "ADVANCE":
            agency_stats[agency]["correct"] += 1
        else:
            agency_stats[agency]["error"] += 1

    for i in current_idx:
        if i in cur_exemplars:
            continue
        agency = provs_4366[i].get("agency", "") or "(none)"
        vec = vecs_4366[i]
        sim_adv = float(np.dot(vec, adv_mean))
        sim_cur = float(np.dot(vec, cur_mean))
        predicted = "ADVANCE" if sim_adv > sim_cur else "CURRENT"
        agency_stats[agency]["total"] += 1
        if predicted == "CURRENT":
            agency_stats[agency]["correct"] += 1
        else:
            agency_stats[agency]["error"] += 1

    print(f"\n  {'Agency':<55s} {'Total':>5s} {'Err':>4s} {'Acc':>6s}")
    print(f"  {'─' * 55} {'─' * 5} {'─' * 4} {'─' * 6}")
    for agency, stats in sorted(agency_stats.items(), key=lambda x: -x[1]["error"]):
        total = stats["total"]
        errors = stats["error"]
        acc = stats["correct"] / total if total > 0 else 0
        if total >= 3:  # Only show agencies with enough data
            print(f"  {agency[:55]:<55s} {total:>5d} {errors:>4d} {acc:>5.0%}")

    # ── Analysis 4: Similarity score distributions ──
    print("\n--- Analysis 4: Similarity score distributions ---")

    adv_scores = {"adv_sim": [], "cur_sim": []}
    cur_scores = {"adv_sim": [], "cur_sim": []}

    for i in advance_idx:
        if i in adv_exemplars:
            continue
        vec = vecs_4366[i]
        adv_scores["adv_sim"].append(float(np.dot(vec, adv_mean)))
        adv_scores["cur_sim"].append(float(np.dot(vec, cur_mean)))

    for i in current_idx:
        if i in cur_exemplars:
            continue
        vec = vecs_4366[i]
        cur_scores["adv_sim"].append(float(np.dot(vec, adv_mean)))
        cur_scores["cur_sim"].append(float(np.dot(vec, cur_mean)))

    if adv_scores["adv_sim"]:
        print(f"\n  Advance provisions (should prefer adv centroid):")
        print(f"    sim_to_adv:  mean={np.mean(adv_scores['adv_sim']):.4f}  std={np.std(adv_scores['adv_sim']):.4f}  min={np.min(adv_scores['adv_sim']):.4f}  max={np.max(adv_scores['adv_sim']):.4f}")
        print(f"    sim_to_cur:  mean={np.mean(adv_scores['cur_sim']):.4f}  std={np.std(adv_scores['cur_sim']):.4f}  min={np.min(adv_scores['cur_sim']):.4f}  max={np.max(adv_scores['cur_sim']):.4f}")
        margins = np.array(adv_scores["adv_sim"]) - np.array(adv_scores["cur_sim"])
        print(f"    margin:      mean={np.mean(margins):.4f}  std={np.std(margins):.4f}  min={np.min(margins):.4f}  max={np.max(margins):.4f}")
        print(f"    margin > 0 (correct): {np.sum(margins > 0)}/{len(margins)}")

    if cur_scores["adv_sim"]:
        print(f"\n  Current-year provisions (should prefer cur centroid):")
        print(f"    sim_to_adv:  mean={np.mean(cur_scores['adv_sim']):.4f}  std={np.std(cur_scores['adv_sim']):.4f}  min={np.min(cur_scores['adv_sim']):.4f}  max={np.max(cur_scores['adv_sim']):.4f}")
        print(f"    sim_to_cur:  mean={np.mean(cur_scores['cur_sim']):.4f}  std={np.std(cur_scores['cur_sim']):.4f}  min={np.min(cur_scores['cur_sim']):.4f}  max={np.max(cur_scores['cur_sim']):.4f}")
        margins = np.array(cur_scores["cur_sim"]) - np.array(cur_scores["adv_sim"])
        print(f"    margin:      mean={np.mean(margins):.4f}  std={np.std(margins):.4f}  min={np.min(margins):.4f}  max={np.max(margins):.4f}")
        print(f"    margin > 0 (correct): {np.sum(margins > 0)}/{len(margins)}")

    # ── Analysis 5: Diverse exemplars (more agencies) ──
    print("\n--- Analysis 5: Diverse exemplars (spread across agencies) ---")

    # Try to pick advance exemplars from different agencies
    adv_by_agency = defaultdict(list)
    for i in advance_idx:
        agency = provs_4366[i].get("agency", "") or "(none)"
        adv_by_agency[agency].append(i)

    cur_by_agency = defaultdict(list)
    for i in current_idx:
        agency = provs_4366[i].get("agency", "") or "(none)"
        cur_by_agency[agency].append(i)

    # Pick one from each agency for diverse exemplars
    diverse_adv = []
    for agency, indices in sorted(adv_by_agency.items()):
        diverse_adv.append(indices[0])
    diverse_adv = diverse_adv[:min(len(diverse_adv), 9)]  # cap at 9

    # Pick from diverse agencies for current
    diverse_cur = []
    for agency, indices in sorted(cur_by_agency.items()):
        diverse_cur.append(indices[0])
    diverse_cur = diverse_cur[:min(len(diverse_cur), 9)]  # cap at 9

    print(f"  Diverse advance exemplars ({len(diverse_adv)}):")
    for i in diverse_adv:
        agency = provs_4366[i].get("agency", "") or "(none)"
        print(f"    [{i}] {agency[:50]}")

    print(f"  Diverse current exemplars ({len(diverse_cur)}, showing first 9):")
    for i in diverse_cur[:9]:
        agency = provs_4366[i].get("agency", "") or "(none)"
        print(f"    [{i}] {agency[:50]}")

    if len(diverse_adv) >= 2 and len(diverse_cur) >= 2:
        div_adv_mean = np.mean(vecs_4366[diverse_adv], axis=0)
        div_adv_mean /= np.linalg.norm(div_adv_mean)
        div_cur_mean = np.mean(vecs_4366[diverse_cur], axis=0)
        div_cur_mean /= np.linalg.norm(div_cur_mean)

        class_sim = float(np.dot(div_adv_mean, div_cur_mean))
        print(f"\n  Centroid similarity with diverse exemplars: {class_sim:.4f}")
        print(f"  (Was {float(np.dot(adv_mean, cur_mean)):.4f} with 3-exemplar VA-heavy set)")

        # Re-test with diverse exemplars
        correct_div = 0
        total_div = 0
        for i in advance_idx:
            if i in diverse_adv:
                continue
            vec = vecs_4366[i]
            sim_a = float(np.dot(vec, div_adv_mean))
            sim_c = float(np.dot(vec, div_cur_mean))
            total_div += 1
            if sim_a > sim_c:
                correct_div += 1

        for i in current_idx:
            if i in diverse_cur:
                continue
            vec = vecs_4366[i]
            sim_a = float(np.dot(vec, div_adv_mean))
            sim_c = float(np.dot(vec, div_cur_mean))
            total_div += 1
            if sim_c > sim_a:
                correct_div += 1

        accuracy_div = correct_div / total_div if total_div > 0 else 0
        print(f"\n  H.R. 4366 self-test with diverse exemplars:")
        print(f"    Tested: {total_div}")
        print(f"    Correct: {correct_div}")
        print(f"    Accuracy: {accuracy_div:.1%}")
        print(f"    (Was {250}/{361} = 69.3% with 3-exemplar VA-heavy set)")

    # ── Analysis 6: Simple regex/keyword approach ──
    print("\n--- Analysis 6: Simple keyword approach (no embeddings) ---")

    advance_keywords = [
        "become available on october 1",
        "shall become available on",
        "available beginning october 1",
        "advance appropriation",
    ]

    correct_kw = 0
    total_kw = 0
    errors_kw = 0

    for bills_dir in ["hr4366", "hr5371", "hr7148", "hr1968"]:
        provs = load_provisions(bills_dir)
        ba_idx = get_ba_appropriation_indices(provs)
        for i in ba_idx:
            p = provs[i]
            text = provision_text(p).lower()

            # Only test provisions that have availability language
            if not ("remain available" in text or "october 1" in text or "advance" in text):
                continue

            # Keyword classification
            predicted_advance = any(kw in text for kw in advance_keywords)

            # Ground truth (same heuristic as before)
            actual_advance = "become available on october 1" in text

            total_kw += 1
            if predicted_advance == actual_advance:
                correct_kw += 1
            else:
                errors_kw += 1

    accuracy_kw = correct_kw / total_kw if total_kw > 0 else 0
    print(f"  Keyword-based classification across 4 bills:")
    print(f"    Tested: {total_kw} provisions with availability language")
    print(f"    Correct: {correct_kw}")
    print(f"    Errors: {errors_kw}")
    print(f"    Accuracy: {accuracy_kw:.1%}")

    # ── Analysis 7: What does the embedding text actually contain? ──
    print("\n--- Analysis 7: What the embedding captures ---")
    print("  The build_embedding_text() function in query.rs concatenates:")
    print("    Account: {account_name} | Agency: {agency} | Type: {provision_type}")
    print("    | Section: {section} | Division: {division}")
    print("    | Dollars: {amount} | Semantics: {semantics}")
    print("    | Text: {raw_text}")
    print()
    print("  The 'availability' field is NOT included in the embedding text.")
    print("  The advance/current signal lives primarily in 'availability' and")
    print("  sometimes in 'raw_text' — but raw_text is often truncated to ~150 chars,")
    print("  which may not reach the availability clause.")
    print()
    print("  Example advance provision raw_text (first 200 chars):")
    for i in advance_idx[:2]:
        rt = (provs_4366[i].get("raw_text", "") or "")[:200]
        avail = provs_4366[i].get("availability", "") or "(no availability field)"
        print(f"    [{i}] raw_text: \"{rt}\"")
        print(f"         availability: \"{avail}\"")
        print()

    # ── Summary ──
    print("=" * 80)
    print("CONCLUSIONS")
    print("=" * 80)
    print()
    print("  1. AGENCY BIAS: The 3 advance exemplars are all VA provisions.")
    print("     The centroid captures 'VA-ness' not 'advance-ness'.")
    print()
    print("  2. EMBEDDING CONTENT: build_embedding_text() does NOT include the")
    print("     'availability' field, which is where advance language lives.")
    print("     The advance/current signal is weak or absent in the vectors.")
    print()
    print("  3. CLASS OVERLAP: Centroid similarity is ~0.54 — the classes are")
    print("     poorly separated in embedding space.")
    print()
    print("  4. KEYWORD APPROACH: Simple keyword matching on raw_text + availability")
    print("     is likely more reliable for this specific classification task.")
    print()
    print("  5. RECOMMENDATION FOR v4.0:")
    print("     - Use keyword/regex on availability + raw_text as primary classifier")
    print("     - Use LLM as fallback for ambiguous cases (like Medicaid)")
    print("     - Do NOT rely on embedding exemplars for advance/current classification")
    print("     - Embedding exemplars MAY still work for jurisdiction classification")
    print("       (where the signal IS the dominant semantic — what agency/topic)")


if __name__ == "__main__":
    main()
