#!/usr/bin/env python3
"""
Test exemplar-based jurisdiction classification of division titles.

The v4.0 plan proposes using embedding exemplars to map division letters
(which are bill-internal) to canonical jurisdictions (Defense, THUD, CJS, etc.).

Unlike advance/current classification (where the signal is in availability text,
not in the embedding), jurisdiction classification should work well with embeddings
because the dominant semantic content of provisions IS their agency/topic — which
is exactly what jurisdiction captures.

Approach:
1. From the 13 bills, extract known division→jurisdiction mappings using
   heuristics on division titles from the XML (or extraction.json bill info).
2. For each jurisdiction, pick exemplar provisions from known divisions.
3. Test: given a provision from a different bill, can we classify its jurisdiction
   by comparing its embedding to jurisdiction centroids?

We also test a second approach: classifying raw division TITLE strings
(e.g., "Department of Defense" → Defense, "Transportation, Housing and Urban
Development" → THUD) by comparing them to exemplar title strings via embeddings.
Since we don't have title-level embeddings, we test provision-level classification.
"""

import json
import os
import sys
import numpy as np
from pathlib import Path
from collections import Counter, defaultdict


# Known division → jurisdiction mappings from NEXT_STEPS.md and bill structure
# These are manually curated ground truth for testing
KNOWN_MAPPINGS = {
    # H.R. 4366 (FY2024 omnibus: MilCon-VA, Ag, CJS, E&W, Interior, THUD)
    ("hr4366", "A"): "milcon-va",
    ("hr4366", "B"): "agriculture",
    ("hr4366", "C"): "cjs",
    ("hr4366", "D"): "energy-water",
    ("hr4366", "E"): "interior",
    ("hr4366", "F"): "thud",
    # H.R. 7148 (FY2026 omnibus: Defense, Labor-HHS, THUD, FinServ, State-ForeignOps)
    ("hr7148", "A"): "defense",
    ("hr7148", "B"): "labor-hhs",
    ("hr7148", "C"): "financial-services",
    ("hr7148", "D"): "thud",
    ("hr7148", "E"): "state-foreign-ops",
    # H.R. 6938 (FY2026 minibus: CJS, E&W, Interior)
    ("hr6938", "A"): "cjs",
    ("hr6938", "B"): "energy-water",
    ("hr6938", "C"): "interior",
    # H.R. 5371 (FY2026 minibus: CR + Ag + LegBranch + MilCon-VA)
    ("hr5371", "A"): "continuing-resolution",
    ("hr5371", "B"): "agriculture",
    ("hr5371", "C"): "legislative-branch",
    ("hr5371", "D"): "milcon-va",
    # H.R. 1968 (FY2025 full-year CR with appropriations)
    ("hr1968", "A"): "continuing-resolution",
    # H.R. 5860 (FY2024 CR)
    ("hr5860", "A"): "continuing-resolution",
}

# Which jurisdictions have enough data points across multiple bills to test?
# We need at least 2 bills per jurisdiction — one for exemplars, one for testing.
TESTABLE_JURISDICTIONS = [
    "milcon-va",
    "agriculture",
    "cjs",
    "energy-water",
    "interior",
    "thud",
    "defense",
    "continuing-resolution",
]


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


def get_provisions_for_division(provisions, division_letter):
    """Get indices of provisions in a specific division."""
    indices = []
    for i, p in enumerate(provisions):
        div = p.get("division") or ""
        if div.upper() == division_letter.upper():
            indices.append(i)
    return indices


def provision_label(p, max_len=60):
    name = (p.get("account_name", "") or "")[:max_len]
    if not name:
        name = (p.get("description", "") or "")[:max_len]
    return name


def main():
    if not Path("examples/hr4366/extraction.json").exists():
        print("ERROR: Run from repository root (appropriations/)")
        sys.exit(1)

    print("=" * 80)
    print("EXEMPLAR-BASED JURISDICTION CLASSIFICATION TEST")
    print("=" * 80)

    # ── Step 1: Load all bill data ──
    print("\n--- Step 1: Loading bill data ---")

    all_bills = {}
    bill_dirs = sorted([
        d for d in os.listdir("examples")
        if Path("examples") / d / "extraction.json" is not None
        and (Path("examples") / d / "extraction.json").exists()
    ])

    for bill_dir in bill_dirs:
        try:
            vecs = load_vectors(bill_dir)
            provs = load_provisions(bill_dir)
            all_bills[bill_dir] = {"vectors": vecs, "provisions": provs}
            print(f"  Loaded {bill_dir}: {len(provs)} provisions, {vecs.shape} vectors")
        except Exception as e:
            print(f"  SKIP {bill_dir}: {e}")

    # ── Step 2: Build jurisdiction → (bill, division, provision_indices) map ──
    print("\n--- Step 2: Building jurisdiction map ---")

    jurisdiction_data = defaultdict(list)  # jurisdiction -> [(bill_dir, division, [indices])]

    for (bill_dir, division), jurisdiction in KNOWN_MAPPINGS.items():
        if bill_dir not in all_bills:
            continue
        provs = all_bills[bill_dir]["provisions"]
        indices = get_provisions_for_division(provs, division)
        if indices:
            jurisdiction_data[jurisdiction].append({
                "bill": bill_dir,
                "division": division,
                "indices": indices,
            })
            print(f"  {jurisdiction:25s} ← {bill_dir} Div {division} ({len(indices)} provisions)")

    # ── Step 3: For each testable jurisdiction, build exemplar centroid from one bill,
    #            test on provisions from another bill ──
    print("\n--- Step 3: Cross-bill jurisdiction classification ---")

    # Strategy: for each jurisdiction with 2+ bills, use provisions from the first
    # bill as exemplars and test on provisions from the second bill.

    all_results = []

    for jurisdiction in TESTABLE_JURISDICTIONS:
        entries = jurisdiction_data.get(jurisdiction, [])
        if len(entries) < 2:
            print(f"\n  SKIP {jurisdiction}: only {len(entries)} bill(s), need 2+")
            continue

        # Use first entry as exemplar source, second as test
        exemplar_entry = entries[0]
        test_entry = entries[1]

        exemplar_bill = exemplar_entry["bill"]
        exemplar_indices = exemplar_entry["indices"]
        exemplar_vecs = all_bills[exemplar_bill]["vectors"]
        exemplar_provs = all_bills[exemplar_bill]["provisions"]

        test_bill = test_entry["bill"]
        test_indices = test_entry["indices"]
        test_vecs = all_bills[test_bill]["vectors"]
        test_provs = all_bills[test_bill]["provisions"]

        # Sample up to 10 exemplars (evenly spaced through the division)
        n_exemplars = min(10, len(exemplar_indices))
        step = max(1, len(exemplar_indices) // n_exemplars)
        sampled_exemplar_indices = exemplar_indices[::step][:n_exemplars]

        # Build centroid
        exemplar_matrix = exemplar_vecs[sampled_exemplar_indices]
        centroid = np.mean(exemplar_matrix, axis=0)
        centroid /= np.linalg.norm(centroid)

        all_results.append({
            "jurisdiction": jurisdiction,
            "exemplar_bill": exemplar_bill,
            "exemplar_div": exemplar_entry["division"],
            "n_exemplars": n_exemplars,
            "test_bill": test_bill,
            "test_div": test_entry["division"],
            "n_test": len(test_indices),
            "centroid": centroid,
            "test_indices": test_indices,
        })

    # Now classify: for each test provision, compute similarity to ALL jurisdiction
    # centroids and pick the highest.
    print("\n--- Step 4: Multi-class classification ---")
    print(f"  Jurisdictions with centroids: {len(all_results)}")

    # Build centroid matrix
    centroids = {}
    for r in all_results:
        centroids[r["jurisdiction"]] = r["centroid"]

    jurisdiction_names = sorted(centroids.keys())
    centroid_matrix = np.array([centroids[j] for j in jurisdiction_names])

    print(f"  Centroid matrix shape: {centroid_matrix.shape}")

    # Compute pairwise similarities between centroids
    print("\n  Centroid pairwise similarities:")
    print(f"  {'':25s}", end="")
    for j in jurisdiction_names:
        print(f" {j[:8]:>8s}", end="")
    print()

    for i, ji in enumerate(jurisdiction_names):
        print(f"  {ji:25s}", end="")
        for j_idx, jj in enumerate(jurisdiction_names):
            sim = float(np.dot(centroid_matrix[i], centroid_matrix[j_idx]))
            if i == j_idx:
                print(f"    {'—':>5s}", end="")
            else:
                print(f" {sim:>8.3f}", end="")
        print()

    # Classify every test provision
    print("\n  Per-jurisdiction results:")
    print(f"  {'Jurisdiction':25s} {'Exemplar':>10s} {'Test':>10s} {'Correct':>8s} {'Accuracy':>9s}")
    print(f"  {'─' * 25} {'─' * 10} {'─' * 10} {'─' * 8} {'─' * 9}")

    total_correct = 0
    total_tested = 0
    per_jurisdiction_errors = defaultdict(list)

    for r in all_results:
        test_bill = r["test_bill"]
        test_indices = r["test_indices"]
        test_vecs_bill = all_bills[test_bill]["vectors"]
        test_provs_bill = all_bills[test_bill]["provisions"]
        true_jurisdiction = r["jurisdiction"]

        correct = 0
        tested = 0

        for idx in test_indices:
            vec = test_vecs_bill[idx]
            # Compute similarity to all centroids
            sims = centroid_matrix @ vec
            predicted_idx = np.argmax(sims)
            predicted_jurisdiction = jurisdiction_names[predicted_idx]

            tested += 1
            if predicted_jurisdiction == true_jurisdiction:
                correct += 1
            else:
                per_jurisdiction_errors[true_jurisdiction].append({
                    "provision_index": idx,
                    "bill": test_bill,
                    "predicted": predicted_jurisdiction,
                    "true": true_jurisdiction,
                    "predicted_sim": float(sims[predicted_idx]),
                    "true_sim": float(sims[jurisdiction_names.index(true_jurisdiction)]),
                    "label": provision_label(test_provs_bill[idx]),
                })

        accuracy = correct / tested if tested > 0 else 0
        total_correct += correct
        total_tested += tested

        src = f"{r['exemplar_bill']}:{r['exemplar_div']}"
        tgt = f"{r['test_bill']}:{r['test_div']}"
        print(f"  {true_jurisdiction:25s} {src:>10s} {tgt:>10s} {correct:>5d}/{tested:<3d} {accuracy:>8.1%}")

    overall_accuracy = total_correct / total_tested if total_tested > 0 else 0
    print(f"  {'─' * 25} {'─' * 10} {'─' * 10} {'─' * 8} {'─' * 9}")
    print(f"  {'OVERALL':25s} {'':>10s} {'':>10s} {total_correct:>5d}/{total_tested:<3d} {overall_accuracy:>8.1%}")

    # ── Show misclassifications ──
    print("\n--- Step 5: Error analysis ---")

    total_errors = sum(len(v) for v in per_jurisdiction_errors.values())
    print(f"  Total misclassifications: {total_errors}")

    if total_errors > 0:
        # Show confusion matrix
        confusion = defaultdict(Counter)
        for jurisdiction, errors in per_jurisdiction_errors.items():
            for e in errors:
                confusion[jurisdiction][e["predicted"]] += 1

        print("\n  Confusion (true → predicted):")
        for true_j in sorted(confusion.keys()):
            for pred_j, count in confusion[true_j].most_common():
                print(f"    {true_j:25s} → {pred_j:25s}  ({count} provisions)")

        # Show some examples
        print("\n  Example misclassifications (up to 3 per jurisdiction):")
        for jurisdiction in sorted(per_jurisdiction_errors.keys()):
            errors = per_jurisdiction_errors[jurisdiction]
            print(f"\n    {jurisdiction}:")
            for e in errors[:3]:
                margin = e["true_sim"] - e["predicted_sim"]
                print(f"      [{e['bill']}:{e['provision_index']}] "
                      f"pred={e['predicted']} "
                      f"(sim={e['predicted_sim']:.3f} vs true={e['true_sim']:.3f}, "
                      f"margin={margin:+.3f})")
                print(f"        {e['label']}")

    # ── Step 6: Test with provisions from non-division bills ──
    # Some bills (like hr9468 supplemental, hr815 supplemental) don't have
    # clear jurisdiction mappings. Can we still classify their provisions?
    print("\n--- Step 6: Classify un-mapped bill provisions ---")

    unmapped_bills = ["hr815", "hr9468"]
    for bill_dir in unmapped_bills:
        if bill_dir not in all_bills:
            continue

        provs = all_bills[bill_dir]["provisions"]
        vecs = all_bills[bill_dir]["vectors"]

        print(f"\n  {bill_dir} ({len(provs)} provisions):")

        # Classify all provisions and show jurisdiction distribution
        classified = Counter()
        for i in range(len(provs)):
            vec = vecs[i]
            sims = centroid_matrix @ vec
            predicted_idx = np.argmax(sims)
            predicted = jurisdiction_names[predicted_idx]
            classified[predicted] += 1

        for j, count in classified.most_common():
            pct = count / len(provs) * 100
            print(f"    {j:25s}: {count:4d} ({pct:5.1f}%)")

    # ── Summary ──
    print("\n" + "=" * 80)
    print("SUMMARY")
    print("=" * 80)
    print(f"\n  Jurisdictions tested:     {len(all_results)}")
    print(f"  Total provisions tested:  {total_tested}")
    print(f"  Total correct:            {total_correct}")
    print(f"  Total errors:             {total_errors}")
    print(f"  Overall accuracy:         {overall_accuracy:.1%}")
    print(f"  Exemplars per class:      up to 10 (evenly sampled from one bill)")
    print(f"  API calls needed:         0 (just dot products on pre-computed vectors)")

    if overall_accuracy >= 0.90:
        print("\n  ✓ Exemplar-based jurisdiction classification looks STRONG for v4.0")
        print("    The dominant semantic (agency/topic) IS what jurisdiction captures.")
    elif overall_accuracy >= 0.75:
        print("\n  ~ Moderate accuracy — may need tuning but approach is viable")
    else:
        print("\n  ✗ Poor accuracy — exemplar approach may not work for jurisdiction")

    print("\n  Key insight: Unlike advance/current classification (where the signal")
    print("  is in availability text NOT encoded in embeddings), jurisdiction is")
    print("  inherently encoded in the embeddings because it's the dominant semantic.")


if __name__ == "__main__":
    main()
