# Cookbook Scripts

This directory contains the Python scripts referenced in the [Recipes & Demos](../src/tutorials/cookbook.md) chapter of the book.

## Setup

```bash
# From the repository root:
cd book/cookbook
pip install -r requirements.txt
```

Or if you use the project's virtual environment:

```bash
source .venv/bin/activate
pip install -r book/cookbook/requirements.txt
```

## Scripts

| Script | What it does | API keys needed |
|--------|-------------|-----------------|
| `cookbook.py` | Runs all 24 demos from the Recipes & Demos page — generates CSVs, charts, and JSON | `OPENAI_API_KEY` for semantic search demos (optional; all other demos run without it) |

## Running

```bash
# From the repository root:
source .venv/bin/activate
python book/cookbook/cookbook.py
```

Output goes to `tmp/demo_output/`:

| File | Description |
|------|-------------|
| `fy2026_treemap.html` | Interactive Plotly treemap — FY2026 spending by jurisdiction/agency/account |
| `defense_vs_nondefense.png` | Stacked bar chart — Defense vs. non-defense FY2019–FY2026 |
| `spending_trends_top6.png` | Line chart — top 6 accounts over 8 fiscal years |
| `verification_heatmap.png` | Heatmap — verification quality across all 32 bills |
| `authorities_flat.csv` | Every provision-FY pair as a flat CSV — ready for pandas, R, or Excel |
| `biggest_changes_2024_2026.csv` | All account-level changes FY2024 → FY2026 |
| `cr_substitutions.csv` | Every CR substitution across all bills |
| `rename_events.csv` | 40 account rename events with fiscal year boundaries |
| `subcommittee_scorecard.csv` | 12 subcommittees × 7 fiscal years |
| `semantic_search_demos.json` | 10 semantic queries with top-3 results each |
| `dataset_summary.json` | Dataset summary card (bills, provisions, BA, authorities) |

## Requirements

- **Python 3.10+**
- **`data/` directory** with extracted bills (the 32-bill dataset)
- **`congress-approp` binary** on your PATH (for CLI export demos)
- **`OPENAI_API_KEY`** (optional — only for Demo 11: Semantic Search)

## Regenerating

The cookbook page in the book includes embedded output from these scripts. If you re-extract bills or add new ones, regenerate the output:

```bash
python book/cookbook/cookbook.py
```

Then compare the new output against what's in the book. If numbers have changed, update the cookbook page (`book/src/tutorials/cookbook.md`).