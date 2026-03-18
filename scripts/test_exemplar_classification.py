#!/usr/bin/env python3
"""
Test exemplar-based advance vs current-year classification.

Uses pre-computed embedding vectors from the examples/ directory to test whether
a simple "mean exemplar vector" approach can distinguish advance appropriations
(money enacted now but available in a future FY) from current-year appropriations.

The approach:
1. From H.R. 4366 (FY2024 omnibus), find provisions whose text mentions
   "shall become available on October 1" (advance) vs those that don't (current-year).
2. Take 3 exemplars of each class, compute mean vectors.
3. For test provisions from H.R. 5371 (FY2026 minibus), classify by comparing
   cosine similarity to each class's mean vector.
4. Report accuracy.

This validates the v4.0 plan's claim that exemplar-based classification works
with zero API calls at classification time — just dot products.
"""

import json
import os
import sys
import numpy as np
from pathlib import Path


def load_vectors(bill_dir: str) -> np.ndarray:
    """Load embedding vectors for a bill."""
    meta_path = Path("examples") / bill_dir / "embeddings.json"
    vec_path = Path("examples") / bill_dir / "vectors.bin"

    with open(meta_path) as f:
        meta = json.load(f)

    dims = meta["dimensions"]
    count = meta["count"]

    raw = open(vec_path, "rb").read()
    vecs = np.frombuffer(raw, dtype=np.float32).reshape(count, dims)
    return vecs


def load_provisions(bill_dir: str) -> list:
    """Load provisions from extraction.json."""
    ext_path = Path("examples") / bill_dir / "extraction.json"
    with open(ext_path) as f:
        data = json.load(f)
    return data["provisions"]


def classify_provisions(provisions, indices_to_check, text_fn):
    """
    Classify provisions as advance or current-year based on text heuristics.

    Returns two lists: (advance_indices, current_year_indices)
    """
    advance = []
    current = []

    for i in indices_to_check:
        p = provisions[i]
        text = text_fn(p).lower()

        # Advance: "shall become available on October 1" or similar
        if "become available on october 1" in text:
            advance.append(i)
        elif "remain available" in text:
            # Current-year: has availability language but NOT advance
            current.append(i)

    return advance, current


def get_ba_appropriation_indices(provisions):
    """Get indices of appropriation provisions with new_budget_authority semantics."""
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


def provision_text(p):
    """Combine relevant text fields for classification."""
    parts = []
    parts.append(p.get("raw_text", "") or "")
    parts.append(p.get("availability", "") or "")
    # Also check notes which sometimes mention advance
    for note in (p.get("notes", None) or []):
        parts.append(note)
    return " ".join(parts)


def provision_label(p):
    """Short label for a provision."""
    name = (p.get("account_name", "") or "")[:50]
    dollars = 0
    amt = p.get("amount")
    if amt:
        val = amt.get("value", {})
        if val.get("kind") == "specific":
            dollars = val.get("dollars", 0)
    return f"{name} (${dollars:,})"


def main():
    # Verify we're in the right directory
    if not Path("examples/hr4366/extraction.json").exists():
        print("ERROR: Run this script from the repository root (appropriations/)")
        sys.exit(1)

    print("=" * 80)
    print("EXEMPLAR-BASED ADVANCE vs CURRENT-YEAR CLASSIFICATION TEST")
    print("=" * 80)

    # ── Step 1: Build exemplars from H.R. 4366 (FY2024 omnibus) ──
    print("\n--- Step 1: Building exemplars from H.R. 4366 ---")

    vecs_4366 = load_vectors("hr4366")
    provs_4366 = load_provisions("hr4366")

    ba_indices = get_ba_appropriation_indices(provs_4366)
    print(f"  Total BA appropriation provisions: {len(ba_indices)}")

    advance_idx, current_idx = classify_provisions(
        provs_4366, ba_indices, provision_text
    )
    print(f"  Advance provisions (heuristic): {len(advance_idx)}")
    print(f"  Current-year provisions (heuristic): {len(current_idx)}")

    if len(advance_idx) < 3 or len(current_idx) < 3:
        print("ERROR: Not enough exemplars found")
        sys.exit(1)

    # Take first 3 of each as exemplars
    adv_exemplars = advance_idx[:3]
    cur_exemplars = current_idx[:3]

    print("\n  Advance exemplars:")
    for i in adv_exemplars:
        print(f"    [{i}] {provision_label(provs_4366[i])}")

    print("\n  Current-year exemplars:")
    for i in cur_exemplars:
        print(f"    [{i}] {provision_label(provs_4366[i])}")

    # Compute mean exemplar vectors (normalized)
    adv_mean = np.mean(vecs_4366[adv_exemplars], axis=0)
    adv_mean /= np.linalg.norm(adv_mean)

    cur_mean = np.mean(vecs_4366[cur_exemplars], axis=0)
    cur_mean /= np.linalg.norm(cur_mean)

    # How separable are the classes?
    class_sim = float(np.dot(adv_mean, cur_mean))
    print(f"\n  Cosine similarity between class centroids: {class_sim:.4f}")
    print(f"  (Lower = more separable. 1.0 = identical, 0.0 = orthogonal)")

    # ── Step 2: Self-test on H.R. 4366 (excluding exemplars) ──
    print("\n--- Step 2: Self-test on H.R. 4366 (excluding exemplars) ---")

    correct = 0
    total = 0
    errors = []

    # Test all advance provisions not used as exemplars
    for i in advance_idx:
        if i in adv_exemplars:
            continue
        vec = vecs_4366[i]
        sim_adv = float(np.dot(vec, adv_mean))
        sim_cur = float(np.dot(vec, cur_mean))
        predicted = "ADVANCE" if sim_adv > sim_cur else "CURRENT"
        actual = "ADVANCE"
        total += 1
        if predicted == actual:
            correct += 1
        else:
            errors.append((i, predicted, actual, sim_adv, sim_cur))

    # Test current-year provisions not used as exemplars
    for i in current_idx:
        if i in cur_exemplars:
            continue
        vec = vecs_4366[i]
        sim_adv = float(np.dot(vec, adv_mean))
        sim_cur = float(np.dot(vec, cur_mean))
        predicted = "ADVANCE" if sim_adv > sim_cur else "CURRENT"
        actual = "CURRENT"
        total += 1
        if predicted == actual:
            correct += 1
        else:
            errors.append((i, predicted, actual, sim_adv, sim_cur))

    accuracy = correct / total if total > 0 else 0
    print(f"  Tested: {total} provisions (excluding {len(adv_exemplars) + len(cur_exemplars)} exemplars)")
    print(f"  Correct: {correct}")
    print(f"  Errors: {len(errors)}")
    print(f"  Accuracy: {accuracy:.1%}")

    if errors:
        print("\n  Misclassifications:")
        for i, pred, actual, sim_a, sim_c in errors[:10]:
            label = provision_label(provs_4366[i])
            margin = abs(sim_a - sim_c)
            print(f"    [{i}] pred={pred} actual={actual} margin={margin:.4f} | {label}")

    # ── Step 3: Cross-bill test on H.R. 5371 (FY2026 minibus) ──
    print("\n--- Step 3: Cross-bill test on H.R. 5371 (FY2026 minibus) ---")

    vecs_5371 = load_vectors("hr5371")
    provs_5371 = load_provisions("hr5371")

    ba_indices_5371 = get_ba_appropriation_indices(provs_5371)
    advance_5371, current_5371 = classify_provisions(
        provs_5371, ba_indices_5371, provision_text
    )

    print(f"  BA appropriation provisions: {len(ba_indices_5371)}")
    print(f"  Advance (heuristic ground truth): {len(advance_5371)}")
    print(f"  Current-year (heuristic ground truth): {len(current_5371)}")

    correct_cross = 0
    total_cross = 0
    errors_cross = []

    for i in advance_5371:
        vec = vecs_5371[i]
        sim_adv = float(np.dot(vec, adv_mean))
        sim_cur = float(np.dot(vec, cur_mean))
        predicted = "ADVANCE" if sim_adv > sim_cur else "CURRENT"
        total_cross += 1
        if predicted == "ADVANCE":
            correct_cross += 1
        else:
            errors_cross.append((i, predicted, "ADVANCE", sim_adv, sim_cur))

    for i in current_5371:
        vec = vecs_5371[i]
        sim_adv = float(np.dot(vec, adv_mean))
        sim_cur = float(np.dot(vec, cur_mean))
        predicted = "ADVANCE" if sim_adv > sim_cur else "CURRENT"
        total_cross += 1
        if predicted == "CURRENT":
            correct_cross += 1
        else:
            errors_cross.append((i, predicted, "CURRENT", sim_adv, sim_cur))

    accuracy_cross = correct_cross / total_cross if total_cross > 0 else 0
    print(f"\n  Cross-bill results:")
    print(f"  Tested: {total_cross}")
    print(f"  Correct: {correct_cross}")
    print(f"  Errors: {len(errors_cross)}")
    print(f"  Accuracy: {accuracy_cross:.1%}")

    if errors_cross:
        print("\n  Misclassifications:")
        for i, pred, actual, sim_a, sim_c in errors_cross[:10]:
            label = provision_label(provs_5371[i])
            margin = abs(sim_a - sim_c)
            print(f"    [{i}] pred={pred} actual={actual} margin={margin:.4f} | {label}")

    # ── Step 4: Cross-bill test on H.R. 7148 (FY2026 omnibus) ──
    print("\n--- Step 4: Cross-bill test on H.R. 7148 (FY2026 omnibus) ---")

    vecs_7148 = load_vectors("hr7148")
    provs_7148 = load_provisions("hr7148")

    ba_indices_7148 = get_ba_appropriation_indices(provs_7148)
    advance_7148, current_7148 = classify_provisions(
        provs_7148, ba_indices_7148, provision_text
    )

    print(f"  BA appropriation provisions: {len(ba_indices_7148)}")
    print(f"  Advance (heuristic ground truth): {len(advance_7148)}")
    print(f"  Current-year (heuristic ground truth): {len(current_7148)}")

    correct_7148 = 0
    total_7148 = 0
    errors_7148 = []

    for i in advance_7148:
        vec = vecs_7148[i]
        sim_adv = float(np.dot(vec, adv_mean))
        sim_cur = float(np.dot(vec, cur_mean))
        predicted = "ADVANCE" if sim_adv > sim_cur else "CURRENT"
        total_7148 += 1
        if predicted == "ADVANCE":
            correct_7148 += 1
        else:
            errors_7148.append((i, predicted, "ADVANCE", sim_adv, sim_cur))

    for i in current_7148:
        vec = vecs_7148[i]
        sim_adv = float(np.dot(vec, adv_mean))
        sim_cur = float(np.dot(vec, cur_mean))
        predicted = "ADVANCE" if sim_adv > sim_cur else "CURRENT"
        total_7148 += 1
        if predicted == "CURRENT":
            correct_7148 += 1
        else:
            errors_7148.append((i, predicted, "CURRENT", sim_adv, sim_cur))

    accuracy_7148 = correct_7148 / total_7148 if total_7148 > 0 else 0
    print(f"\n  Cross-bill results:")
    print(f"  Tested: {total_7148}")
    print(f"  Correct: {correct_7148}")
    print(f"  Errors: {len(errors_7148)}")
    print(f"  Accuracy: {accuracy_7148:.1%}")

    if errors_7148:
        print("\n  Misclassifications:")
        for i, pred, actual, sim_a, sim_c in errors_7148[:10]:
            label = provision_label(provs_7148[i])
            margin = abs(sim_a - sim_c)
            print(f"    [{i}] pred={pred} actual={actual} margin={margin:.4f} | {label}")
        if len(errors_7148) > 10:
            print(f"    ... and {len(errors_7148) - 10} more")

    # ── Summary ──
    print("\n" + "=" * 80)
    print("SUMMARY")
    print("=" * 80)

    total_all = total + total_cross + total_7148
    correct_all = correct + correct_cross + correct_7148
    errors_all = len(errors) + len(errors_cross) + len(errors_7148)
    accuracy_all = correct_all / total_all if total_all > 0 else 0

    print(f"  H.R. 4366 self-test:    {correct}/{total} = {accuracy:.1%}")
    print(f"  H.R. 5371 cross-bill:   {correct_cross}/{total_cross} = {accuracy_cross:.1%}")
    print(f"  H.R. 7148 cross-bill:   {correct_7148}/{total_7148} = {accuracy_7148:.1%}")
    print(f"  ──────────────────────────────────────")
    print(f"  OVERALL:                {correct_all}/{total_all} = {accuracy_all:.1%}")
    print(f"  Total errors:           {errors_all}")
    print(f"  Exemplars used:         3 advance + 3 current (from H.R. 4366 only)")
    print(f"  API calls needed:       0 (just dot products)")

    if accuracy_all >= 0.90:
        print("\n  ✓ Exemplar-based classification looks viable for v4.0")
    elif accuracy_all >= 0.75:
        print("\n  ~ Moderate accuracy — may need more exemplars or LLM fallback")
    else:
        print("\n  ✗ Poor accuracy — exemplar approach may not work as designed")


if __name__ == "__main__":
    main()
