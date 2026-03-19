# Changelog

All notable changes to `congress-approp` are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

For the full changelog with technical details, see [CHANGELOG.md](https://github.com/cgorski/congress-appropriations/blob/main/CHANGELOG.md) in the repository.

---

## [4.2.0] — 2026-03-19

### Added
- **`fiscal_year`, `detail_level`, `confidence`, `provision_index`, and `match_tier` columns** in `search --format csv` output. The CSV now matches the documented column set.
- **`fiscal_year()` and `detail_level()` accessor methods** on the `Provision` enum in the library API.
- **`fiscal_years` field** in `BillSummary` and a new "FYs" column in the `summary` table showing which fiscal years each bill covers.
- **Smart export warning** — when exporting to CSV/JSON/JSONL, stderr shows a breakdown by semantics type and warns about sub-allocation summing when mixed semantics are present.
- **Export Data section in README** with quick export patterns and a sub-allocation warning.
- **3 new integration tests** plus 5 new assertions on existing tests. Total: 191 tests (146 unit + 45 integration).

### Fixed
- **Documentation:** Export tutorial listed CSV columns that didn't exist. Code now matches docs. Added bold warning about sub-allocation summing trap and "Computing Totals Correctly" subsection.

## [4.1.0] — 2026-03-19

### Added
- **`--real` flag on `compare`** — inflation-adjusted "Real Δ %*" column using CPI-U data from the Bureau of Labor Statistics.
- **`--cpi-file <PATH>` flag on `compare`** — override bundled CPI-U data with a custom price index file.
- **`inflation.rs` module** — CPI data loading, fiscal-year-weighted averages, inflation rate calculation, real delta computation. 16 unit tests.
- **Bundled CPI data** (`cpi.json`) — monthly CPI-U values from Jan 2013 through Feb 2026. No network access required at runtime.
- **Inflation flags** — ▲ (real increase), ▼ (real cut or inflation erosion), — (unchanged) in compare output.
- **Inflation-aware CSV and JSON output** with `real_delta_pct` and `inflation_flag` columns/fields.
- **Staleness warning** when bundled CPI data is more than 60 days old.
- **Inflation adjustment how-to chapter** in the documentation book.

## [4.0.0] — 2026-03-19

### Added
- **`enrich` command** — generates `bill_meta.json` per bill with fiscal year metadata, subcommittee/jurisdiction mappings, advance appropriation classification, bill nature enrichment, and canonical account names. No API keys required.
- **`relate` command** — deep-dive on one provision across all bills with embedding similarity, confidence tiers, fiscal year timeline (`--fy-timeline`), and deterministic link hashes (`--format hashes`).
- **`link suggest` / `link accept` / `link remove` / `link list`** — persistent cross-bill provision links. Discover candidates via embedding similarity, accept by hash, manage saved relationships.
- **`--fy <YEAR>`** on `summary`, `search`, `compare` — filter to bills covering a specific fiscal year.
- **`--subcommittee <SLUG>`** on `summary`, `search`, `compare` — filter by appropriations subcommittee jurisdiction (requires `enrich`).
- **`--show-advance`** on `summary` — separates current-year from advance appropriations in the output.
- **`--base-fy` / `--current-fy`** on `compare` — compare all bills for one fiscal year against another.
- **`compare --use-links`** — uses accepted links for matching across renames.
- **Advance appropriation detection** — fiscal-year-aware classification identifying $1.49 trillion in advance appropriations across the 13-bill dataset.
- **Cross-semantics orphan rescue** in compare — recovers provisions like Transit Formula Grants ($14.6B) that have different semantics across bills.
- **Sub-agency normalization** — 35-entry lookup table resolving agency granularity mismatches in compare (e.g., "Maritime Administration" ↔ "Department of Transportation").
- Pre-enriched `bill_meta.json` for all 13 example bills.

### Changed
- **Compare uses case-insensitive account matching** — resolves 52 false orphans from capitalization differences.
- **Summary displays enriched bill classification** when `bill_meta.json` is available (e.g., "Full-Year CR with Appropriations" instead of "Continuing Resolution").
- **Summary handler consolidated** to call `query::summarize()` instead of reimplementing inline.
- **Hash chain extended** to cover `bill_meta.json`.
- Version bumped to 4.0.0.

## [3.2.0] — 2026-03-18

### Added
- **`--continue-on-error` flag on `extract`** — opt-in to saving partial results when some chunks fail.

### Changed
- **Extract aborts on chunk failure by default.** Prevents garbage partial extractions.
- **Per-bill error handling** in multi-bill extraction runs.

## [3.1.0] — 2026-03-18

### Added

- **`--all-versions` flag on `download`** — explicitly download all text versions (introduced, engrossed, enrolled, etc.) when needed for conference tracking or bill comparison workflows.
- **`--force` flag on `extract`** — re-extract bills even if `extraction.json` already exists. Without this flag, already-extracted bills are automatically skipped, making it safe to re-run after partial failures.

### Changed

- **Download defaults to enrolled only.** The `download` command now fetches only the enrolled (signed into law) XML by default, instead of every available text version. This prevents downloading 4–6 unnecessary files per bill and avoids wasted API calls during extraction. Use `--version` to request a specific version or `--all-versions` for all versions.
- **Extract prefers enrolled XML.** When a bill directory contains multiple `BILLS-*.xml` files, the `extract` command automatically uses only the enrolled version (`*enr.xml`) and ignores other versions.
- **Extract skips already-extracted bills.** If `extraction.json` already exists in a bill directory, `extract` skips it with an informational message. Use `--force` to override. The `ANTHROPIC_API_KEY` is not required when all bills are already extracted.
- **Extract is resilient to parse failures.** If an XML file fails to parse (e.g., a non-enrolled version with an unexpected structure), the tool logs a warning and continues to the next bill instead of aborting the entire run.
- **Better error messages on XML parse failure.** Parse errors now include the filename that failed.
- Version bumped to 3.1.0.

---

## [3.0.0] — 2026-03-17

### Added

- **Semantic search** — `--semantic "query"` on the `search` command ranks provisions by meaning similarity using OpenAI embeddings. Finds "Child Nutrition Programs" from "school lunch programs for kids" with zero keyword overlap. See [Use Semantic Search](../tutorials/semantic-search.md).
- **Find similar** — `--similar bill_dir:index` finds provisions most similar to a specific one across all loaded bills. Useful for cross-bill matching and year-over-year tracking. No API call needed — uses pre-computed vectors. See [Track a Program Across Bills](../tutorials/track-program-across-bills.md).
- **`embed` command** — generates embeddings for extracted bills using OpenAI `text-embedding-3-large`. Writes `embeddings.json` (metadata) + `vectors.bin` (binary float32 vectors) per bill directory. Skips up-to-date bills automatically. See [Generate Embeddings](../how-to/generate-embeddings.md).
- **Pre-generated embeddings** for all three example bills (3,072 dimensions). Semantic search works on example data without running `embed`.
- **OpenAI API client** (`src/api/openai/`) for the embeddings endpoint.
- **Hash chain** — `source_xml_sha256` in `metadata.json`, `extraction_sha256` in `embeddings.json`. Enables staleness detection across the full pipeline. See [Data Integrity and the Hash Chain](../explanation/hash-chain.md).
- **Staleness detection** (`src/approp/staleness.rs`) — checks whether downstream artifacts are consistent with their inputs. Warns but never blocks.
- **`--top N`** flag on `search` for controlling semantic/similar result count (default 20).
- Cosine similarity utilities in `embeddings.rs` with unit tests.
- `build_embedding_text()` in `query.rs` — deterministic text builder for provision embeddings.

### Changed

- `handle_search` is now async to support OpenAI embedding API calls.
- README: removed coverage percentages from intro and bill table (was confusing). Updated summary table example to match current output.
- `chunks/` directory renamed from `.chunks/` — LLM artifacts kept as local provenance (gitignored, not part of hash chain).
- Example `metadata.json` files updated with `source_xml_sha256` field.

---

## [2.1.0] — 2026-03-17

### Added

- `--division` filter on `search` command — scope results to a single division letter (e.g., `--division A` for MilCon-VA).
- `--min-dollars` and `--max-dollars` filters on `search` command — find provisions within a dollar range.
- `--format jsonl` output on `search` and `summary` — one JSON object per line, pipeable with `jq`. See [Output Formats](../reference/output-formats.md).
- Enhanced `--dry-run` on `extract` — now shows chunk count and estimated input tokens.
- Footer on `summary` table showing count of unverified dollar amounts across all bills.
- This changelog.

### Changed

- `summary` table no longer shows the `Coverage` column — it was routinely misinterpreted as an accuracy metric when it actually measures what percentage of dollar strings in the source text were matched to a provision. Many unmatched dollar strings (statutory references, loan ceilings, old amounts being struck) are *correctly* excluded. The coverage metric remains available in `audit` and in `--format json` output as `completeness_pct`. See [What Coverage Means (and Doesn't)](../explanation/coverage.md).

### Fixed

- `cargo fmt` and `cargo clippy` clean.

---

## [2.0.0] — 2026-03-17

### Added

- `--model` flag and `APPROP_MODEL` environment variable on `extract` command — override the default LLM model. See [Extract Provisions from a Bill](../how-to/extract-provisions.md).
- `upgrade` command — migrate extraction data to the latest schema version and re-verify without LLM. See [Upgrade Extraction Data](../how-to/upgrade-data.md).
- `audit` command (replaces `report`) — detailed verification breakdown per bill. See [Verify Extraction Accuracy](../how-to/verify-accuracy.md).
- `compare` command warns when comparing different bill classifications (e.g., supplemental vs. omnibus).
- `amount_status` field in search output — `found`, `found_multiple`, or `not_found`.
- `quality` field in search output — `strong`, `moderate`, or `weak` derived from verification data.
- `match_tier` field in search output — `exact`, `normalized`, `spaceless`, or `no_match`.
- `schema_version` field in `extraction.json` and `verification.json`.
- 18 integration tests covering all CLI commands with pinned budget authority totals.

### Changed

- `report` command renamed to `audit` (`report` kept as alias).
- Search output field `verified` renamed to `amount_status` with richer values.
- `compare` output status labels changed: `eliminated` → `only in base`, `new` → `only in current`.
- `arithmetic_checks` field in `verification.json` deprecated — omitted from new files, old files still load.

### Removed

- `hallucinated` terminology removed from all output and documentation.

---

## [1.2.0] — 2026-03-16

### Added

- `audit` command with column guide explaining every metric.
- `compare` command guard rails for cross-classification comparisons.

### Changed

- Terminology overhaul: `report` → `audit` throughout documentation.

---

## [1.1.0] — 2026-03-16

### Added

- Schema versioning (`schema_version: "1.0"`) in extraction and verification files.
- `upgrade` command for migrating pre-versioned data.
- Verification clarity improvements — column guide in `audit` output.

### Fixed

- `SuchSums` amount variants now serialize correctly (fixed via upgrade path).

---

## [1.0.0] — 2026-03-16

Initial release.

### Features

- **Download** enrolled bill XML from Congress.gov API. See [Download Bills from Congress.gov](../how-to/download-bills.md).
- **Parse** congressional XML with `roxmltree` (pure Rust). See [The Extraction Pipeline](../explanation/pipeline.md).
- **Extract** spending provisions via Claude with parallel chunk processing. See [Extract Provisions from a Bill](../how-to/extract-provisions.md).
- **Deterministic verification** of dollar amounts against source text — no LLM in the verification loop. See [How Verification Works](../explanation/verification.md).
- **`search` command** with filters by type, agency, account, keyword, bill. See [Filter and Search Provisions](../how-to/filter-and-search.md).
- **`summary` command** with budget authority totals computed from provisions. See [Budget Authority Calculation](../explanation/budget-authority.md).
- **`compare` command** for account-level diffs between bill sets. See [Compare Two Bills](../tutorials/compare-two-bills.md).
- **CSV and JSON export formats** for all query commands. See [Export Data for Spreadsheets and Scripts](../tutorials/export-data.md).
- **Pre-extracted example data** for three 118th Congress bills:
  - H.R. 4366 — FY2024 omnibus (2,364 provisions, $846B budget authority)
  - H.R. 5860 — FY2024 continuing resolution (130 provisions, 13 CR substitutions)
  - H.R. 9468 — VA supplemental (7 provisions, $2.9B budget authority)

See [Included Example Bills](./example-bills.md) for detailed profiles of each bill.

---

## Version Numbering

This project uses [Semantic Versioning](https://semver.org/):

- **Major** (e.g., 2.0.0 → 3.0.0): Breaking changes to the CLI interface, JSON output schema, or library API. Existing scripts or integrations may need updates.
- **Minor** (e.g., 2.0.0 → 2.1.0): New features, new commands, new flags, new output fields. Backward-compatible — existing scripts continue to work.
- **Patch** (e.g., 3.0.0 → 3.0.1): Bug fixes, documentation improvements, dependency updates. No behavioral changes.

The extraction data schema has its own version (`schema_version` field in `extraction.json`). The `upgrade` command handles schema migrations without re-extraction.