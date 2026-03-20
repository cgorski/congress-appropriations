# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [5.1.0] — 2026-03-20

### Breaking Changes
- **`examples/` renamed to `data/`** with congress-prefixed directory naming. Bill directories are now `{congress}-{type}{number}` (e.g., `118-hr4366`, `119-hr7148`). This format is globally unique, collision-free across congresses, and matches the Congress.gov identifier scheme. The `test-data/` directory (3 small bills, ~500KB) ships with the crate for tests; `data/` (14 bills, 186MB) is in git only.
- **Default `--dir` changed from `./examples` to `./data`** across all commands. If you have scripts using the old default, update them.
- **Provision references now use congress prefix:** `118-hr9468:0` instead of `hr9468:0` for `relate`, `--similar`, and `link` commands.
- **Implicit agency normalization removed.** The hardcoded `SUB_AGENCY_TO_PARENT` lookup table (35 entries) and comma-splitting logic have been removed from `query.rs`. These produced 109 potential silent wrong merges (e.g., merging DOT headquarters S&E with DOT Inspector General S&E into one $300M number). Compare now uses exact lowercased matching by default. To restore cross-bill agency matching with explicit, auditable rules, run `normalize suggest-text-match`.
- **`compare()` library API** gains `agency_groups: &[AgencyGroup]` and `account_aliases: &[AccountAlias]` parameters. Pass empty slices for exact matching.
- **`CompareRow` struct** gains `normalized: bool` field indicating whether the match used entity resolution rules.
- **Crate no longer includes bill data.** Package size reduced from 5.4MB to ~500KB compressed. Install via `git clone` for the full 14-bill dataset with embeddings, or use `download` + `extract` to process your own bills.

### Added
- **`dataset.json`** — user-managed entity resolution file at the data root. Contains agency groups and account aliases for cross-bill matching. No cached or derived data — only knowledge that cannot be computed from scanning bill files. Created by `normalize` commands or hand-edited.
- **`normalize suggest-text-match` command** — discovers agency naming variants by analyzing cross-FY orphan pairs and structural regex patterns (prefix expansion, preposition variants, US abbreviations). No API calls, runs in milliseconds. Filters out generic account names to avoid false groupings.
- **`normalize suggest-llm` command** — sends unresolved ambiguous account clusters to Claude with XML heading context for SAME/DIFFERENT classification. Each cluster includes all appearances of an account across all bills with dollar amounts and structural XML markers. Batched API calls (configurable `--batch-size`). Requires `ANTHROPIC_API_KEY`.
- **`normalize list` command** — displays current agency groups and account aliases from `dataset.json`.
- **`compare --exact` flag** — disables all normalization from `dataset.json`, uses exact lowercased string matching only. Useful for verifying raw matching results.
- **`(normalized)` marker** in compare table output on rows where agency groups from `dataset.json` were applied. CSV output has a separate `normalized` column (`true`/`false`) instead of a status suffix.
- **Orphan-pair hint** in compare output — when unresolved orphans exist, stderr suggests running `normalize suggest-text-match`.
- **Congress number in all output** — summary table shows `H.R. 7148 (119th)`, compare header shows `Comparing: H.R. 4366 (118th) → H.R. 7148 (119th)`, search CSV/JSON includes `congress` field, semantic search table includes congress.
- **`congress` field** in `BillSummary` struct and search output JSON/CSV.
- **`format_bill_id()` helper** for consistent congress number display.
- **`fiscal_year()` and `detail_level()` accessor methods** on the `Provision` enum.
- **`fiscal_years` field** in `BillSummary` and "FYs" column in summary table.
- **`fiscal_year`, `detail_level`, `confidence`, `provision_index`, `match_tier`** columns in search CSV output. Search JSON also includes these fields.
- **Smart export warning** — stderr shows provision count by semantics type when exporting CSV/JSON/JSONL with mixed semantics.
- **`test-data/` directory** with 3 small bills (118-hr9468, 118-hr5860, 118-hr2872) for crate integration tests. Tier 1 tests use `test-data/` (always available), Tier 2 tests use `data/` (auto-skip if absent).
- **Download command creates flat directories** — `{congress}-{type}{number}` format (e.g., `118-hr9468`) instead of nested `{congress}/{type}/{number}`.
- **H.R. 2882 (FY2024 second omnibus)** — 2,582 provisions covering Defense, Financial Services, Homeland Security, Labor-HHS, Legislative Branch, State-Foreign Operations. FY2024 now has all 12 subcommittees. Dataset totals: 14 bills, 11,136 provisions, $8.9 trillion.
- **Export Data section in README** with quick export patterns and sub-allocation warning.
- **6 new integration tests** for normalize commands, `--exact` flag, CSV normalized column, and orphan hint. Total: 207 tests (156 unit + 51 integration).

### Fixed
- **Documentation:** Export tutorial column table now matches actual CSV output. Bold warning about sub-allocation summing trap added. "Computing Totals Correctly" subsection with Excel, jq, and Python examples.
- **Documentation:** All references updated from `examples/` to `data/` across README and ~30 book chapters. Dataset stats updated to 14 bills, 11,136 provisions, $8.9 trillion.
- **Inconsistent `--dir` defaults** — previously 8 commands defaulted to `./data` and 5 to `./examples`. All 13 now default to `./data`.

## [4.2.1] — 2026-03-19

### Added
- **H.R. 2882 (FY2024 second omnibus)** — extracted, enriched, and embedded. Covers Defense, Financial Services, Homeland Security, Labor-HHS, Legislative Branch, and State-Foreign Operations. 2,582 provisions, $2.45 trillion in budget authority, 0 unverifiable dollar amounts, 95.3% coverage. FY2024 now has 12/12 appropriations subcommittees covered. Dataset totals: 14 bills, 11,136 provisions, $8.9 trillion.
- Updated `enrich_skips_existing` test to reflect 14-bill dataset.

## [4.2.0] — 2026-03-19

### Added
- **`fiscal_year`, `detail_level`, `confidence`, `provision_index`, and `match_tier` columns** in `search --format csv` output. These fields were previously available only in JSON output (except `fiscal_year`, `detail_level`, and `confidence` which are new to all formats). The CSV now matches the documented column set.
- **`fiscal_year()` and `detail_level()` accessor methods** on the `Provision` enum in the library API, following the same pattern as existing accessors like `account_name()` and `agency()`.
- **`fiscal_years` field** in `BillSummary` struct and a new "FYs" column in the `summary` table output showing which fiscal years each bill covers (e.g., "2024", "2026", "2024, 2025, 2026, 2027, 2028").
- **Smart export warning** — when exporting search results to CSV/JSON/JSONL, a stderr summary shows the count of provisions by semantics type and total budget authority. When the export includes sub-allocations or reference amounts, a warning reminds users to filter by `semantics=new_budget_authority` for correct totals. The warning does not fire when all exported provisions are `new_budget_authority`.
- **Export Data section in README** — quick cheat sheet with three common export patterns and a prominent warning about the sub-allocation summing trap.
- **3 new integration tests** (`search_csv_new_columns_populated`, `search_csv_stderr_warns_mixed_semantics`, `summary_table_shows_fiscal_years`) plus 5 new assertions on existing `search_csv_has_correct_headers` test. Total: 191 tests (146 unit + 45 integration).

### Fixed
- **Documentation:** The export tutorial listed `detail_level`, `provision_index`, and `match_tier` as CSV columns, but the CLI did not emit them. The code now matches the documentation. Added bold warning about the sub-allocation summing trap and a "Computing Totals Correctly" subsection with Excel, jq, and Python examples.
- **Documentation:** Fixed claim that "The CSV includes every field the JSON output has" — changed to "The CSV includes the same fields as the JSON output, flattened into columns."

## [4.1.0] — 2026-03-19

### Added
- **`--real` flag on `compare`** — adds an inflation-adjusted "Real Δ %*" column showing which programs received real increases (▲) and which lost purchasing power to inflation (▼). Uses CPI-U (Consumer Price Index for All Urban Consumers) from the Bureau of Labor Statistics, with fiscal-year-weighted averages computed from monthly data. The asterisk marks these as computed values based on an external price index, not numbers verified against bill text.
- **`--cpi-file <PATH>` flag on `compare`** — overrides the bundled CPI-U data with a user-provided JSON file containing any price index (GDP deflator, PCE, sector-specific indices). The output footer automatically displays the source from the provided file.
- **`inflation.rs` module** — CPI data loading (bundled via `include_str!` or from file), fiscal-year-weighted average computation, inflation rate calculation, real delta computation, and inflation flags (real_increase, real_cut, inflation_erosion, unchanged). 16 unit tests.
- **Bundled CPI data** (`data/cpi.json`) — monthly CPI-U values from Jan 2013 through Feb 2026, sourced from the BLS Public Data API. Updated with each release. No network access required at runtime.
- **Inflation flags in output** — each compared account shows ▲ (real increase), ▼ (real cut or inflation erosion), or — (unchanged). Summary line counts programs that beat inflation vs fell behind.
- **Inflation-aware CSV and JSON output** — CSV adds `real_delta_pct` and `inflation_flag` columns. JSON wraps rows in an `inflation` metadata object with source, rates, and data completeness.
- **Staleness warning** — when bundled CPI data is more than 60 days old, prints a hint to use `--cpi-file` for more recent data.
- **Inflation adjustment how-to chapter** (`book/src/how-to/inflation-adjustment.md`) — 218-line guide covering quick start, methodology, custom deflator files, output interpretation, caveats about CPI-U vs sector-specific inflation, and partial-year data handling.

## [4.0.0] — 2026-03-19

### Added
- **`enrich` command** — generates `bill_meta.json` per bill directory with fiscal year metadata, subcommittee/jurisdiction mappings, advance appropriation classification, bill nature enrichment, and canonical account names. Requires no API keys — uses XML parsing and deterministic keyword matching.
- **`--fy <YEAR>` flag** on `summary`, `search`, and `compare` — filter to bills covering a specific fiscal year. Uses `bill.fiscal_years` from extraction data (no `enrich` required for basic FY filtering).
- **`--subcommittee <SLUG>` flag** on `summary`, `search`, and `compare` — filter by appropriations subcommittee jurisdiction (e.g., `defense`, `thud`, `cjs`, `milcon-va`). Requires `bill_meta.json` (run `enrich` first). Maps division letters to canonical jurisdictions per-bill, solving the problem where Division A means Defense in one bill but CJS in another.
- **`--base-fy` and `--current-fy` flags** on `compare` — compare all bills for one fiscal year against all bills for another, with optional `--subcommittee` scoping. Use with `--dir` to point at the data directory.
- **`bill_meta.json`** — new per-bill metadata file containing congress number, fiscal years, enriched bill nature (omnibus/minibus/full-year CR with appropriations/supplemental/etc.), subcommittee mappings with classification source provenance, advance/current/supplemental timing for each BA provision, and canonical (case-normalized, prefix-stripped) account names.
- **Advance appropriation detection** — the `enrich` command classifies each budget authority provision as current-year, advance, or supplemental using a fiscal-year-aware algorithm. Detects "shall become available on October 1, YYYY" and "for the first quarter of fiscal year YYYY" patterns, comparing the availability date to the bill's fiscal year. Correctly identifies $1.49 trillion in advance appropriations across the 13-bill dataset.
- **Hash chain extended** to cover `bill_meta.json`: the file records `extraction_sha256`, and staleness detection warns when the extraction has changed since enrichment.
- **13 new integration tests** covering `enrich`, `--fy`, `--subcommittee`, `--base-fy`/`--current-fy`, case-insensitive compare matching, and budget total regression guards.
- **33 new unit tests** in `bill_meta.rs` covering jurisdiction classification, advance detection, account normalization, bill nature classification, and save/load roundtrip.
- Pre-enriched `bill_meta.json` for all 13 example bills.

### Changed
- **Compare uses case-insensitive account matching.** Account names are now lowercased and em-dash/en-dash prefix-stripped before comparison. This resolves 52 false orphans across the 13-bill dataset caused by capitalization differences like "Grants-In-Aid" vs "Grants-in-Aid" vs "Grants-in-aid".
- **Compare handler consolidated.** The `handle_compare` function in `main.rs` now calls `query::compare()` instead of reimplementing the comparison logic. Duplicate `build_account_map`, `normalize_account_name`, and `describe_bills` functions removed from `main.rs`.
- **`CompareRow` field names** aligned with CLI JSON output: `account` → `account_name`, `base_amount` → `base_dollars`, `current_amount` → `current_dollars`.
- **`LoadedBill` struct** now includes `bill_meta: Option<BillMeta>` — loaded automatically from `bill_meta.json` if present, `None` otherwise. Also derives `Clone`.
- **`normalize_account_name` is now public** and lowercases in addition to stripping em-dash prefixes.
- Version bumped to 4.0.0.

### Known Limitations
- **Sub-agency vs parent department mismatches** create approximately 20 false orphans per subcommittee comparison (e.g., "Maritime Administration" in one bill vs "Department of Transportation" in another). A sub-agency normalization lookup table is planned for a future release.
- **Cross-semantics orphan rescue** not yet implemented — Transit Formula Grants ($14.6B) still shows as "only in current" when classified with different semantics across bills. Planned for a follow-up.
- **`--subcommittee` requires `enrich`** — the flag produces a clear error message if `bill_meta.json` is not found. `--fy` works without `enrich`.
- **17 supplemental policy division titles** (e.g., "FEND Off Fentanyl Act") are classified as `other` jurisdiction by default.

## [3.2.0] — 2026-03-18

### Added
- **`--continue-on-error` flag on `extract`** — opt-in to saving partial results when some chunks fail during extraction. Without this flag, the tool now aborts a bill's extraction if any chunk permanently fails (after all retries), and does not write `extraction.json`. This prevents garbage partial extractions from being saved to disk and mistaken for valid data.

### Changed
- **Extract aborts on chunk failure by default.** Previously, if some chunks failed during extraction, the tool would warn and save a partial `extraction.json` anyway — potentially with most of the bill missing. Now it aborts the bill (without writing files) and continues to the next bill. The error message tells you how many chunks failed and suggests `--continue-on-error` if you want the old behavior. This prevents the scenario where a rate-limited extraction produces a 21-provision file for a 2,300-provision omnibus bill.
- **Per-bill error handling in multi-bill runs.** When extracting multiple bills, a failure on one bill no longer aborts the entire run. The failed bill is skipped (no files written), and extraction continues with the remaining bills.
- Version bumped to 3.2.0.

## [3.1.0] — 2026-03-18

### Added
- **`--all-versions` flag on `download`** — explicitly download all text versions (introduced, engrossed, enrolled, etc.) when needed for conference tracking or bill comparison workflows.
- **`--force` flag on `extract`** — re-extract bills even if `extraction.json` already exists. Without this flag, already-extracted bills are automatically skipped, making it safe to re-run after partial failures.

### Changed
- **Download defaults to enrolled only.** The `download` command now fetches only the enrolled (signed into law) XML by default, instead of every available text version. This prevents downloading 4–6 unnecessary files per bill and avoids wasted API calls during extraction. Use `--version` to request a specific version or `--all-versions` for all versions.
- **Extract prefers enrolled XML.** When a bill directory contains multiple `BILLS-*.xml` files, the `extract` command automatically uses only the enrolled version (`*enr.xml`) and ignores other versions. Falls back to all files if no enrolled version exists.
- **Extract skips already-extracted bills.** If `extraction.json` already exists in a bill directory, `extract` skips it with an informational message. Use `--force` to override. The `ANTHROPIC_API_KEY` is not required when all bills are already extracted.
- **Extract is resilient to parse failures.** If an XML file fails to parse (e.g., a non-enrolled version with an unexpected structure), the tool logs a warning and continues to the next bill instead of aborting the entire run.
- **Better error messages on XML parse failure.** Parse errors now include the filename that failed, making it clear which file caused the issue.
- Version bumped to 3.1.0.

## [3.0.0] — 2026-03-17

### Added
- **Semantic search** — `--semantic "query"` on the `search` command ranks provisions by meaning similarity using OpenAI embeddings. Finds "Child Nutrition Programs" from "school lunch programs for kids" with zero keyword overlap.
- **Find similar** — `--similar bill_dir:index` finds provisions most similar to a specific one across all loaded bills. Useful for cross-bill matching and year-over-year tracking.
- **`embed` command** — generates embeddings for extracted bills using OpenAI `text-embedding-3-large`. Writes `embeddings.json` (metadata) + `vectors.bin` (binary float32 vectors) per bill directory. Skips up-to-date bills automatically.
- **Pre-generated embeddings** for all three example bills (1024 dimensions, ~10 MB total). Semantic search works on example data without running `embed`.
- **OpenAI API client** (`src/api/openai/`) for the embeddings endpoint.
- **Hash chain** — `source_xml_sha256` in metadata.json, `extraction_sha256` in embeddings.json. Enables staleness detection across the full pipeline.
- **Staleness detection** (`src/approp/staleness.rs`) — checks whether downstream artifacts are consistent with their inputs. Warns but never blocks.
- **`--top N`** flag on `search` for controlling semantic/similar result count (default 20).
- Cosine similarity utilities in `embeddings.rs` with unit tests.
- `build_embedding_text()` in `query.rs` — deterministic text builder for provision embeddings.
- Semantic Search section in README with setup instructions and examples.

### Changed
- `handle_search` is now async to support OpenAI embedding API calls.
- README: removed coverage percentages from intro and bill table (was confusing). Updated summary table example to match current output.
- `chunks/` directory renamed from `.chunks/` — LLM artifacts kept as local provenance (gitignored, not part of hash chain).
- Example `metadata.json` files updated with `source_xml_sha256` field.

## [2.1.0] — 2026-03-17

### Added
- `--division` filter on `search` command — scope results to a single division letter (e.g., `--division A` for MilCon-VA)
- `--min-dollars` and `--max-dollars` filters on `search` command — find provisions within a dollar range
- `--format jsonl` output on `search` and `summary` — one JSON object per line, pipeable with `jq`
- Enhanced `--dry-run` on `extract` — now shows chunk count and estimated input tokens
- Footer on `summary` table showing count of unverified dollar amounts across all bills
- This changelog

### Changed
- `summary` table no longer shows the `Coverage` column — it was routinely misinterpreted as an accuracy metric when it actually measures what percentage of dollar strings in the source text were matched to a provision. Many unmatched dollar strings (statutory references, loan ceilings, old amounts being struck) are *correctly* excluded. The coverage metric remains available in `audit` and in `--format json` output as `completeness_pct`.

### Fixed
- `cargo fmt` and `cargo clippy` clean

## [2.0.0] — 2026-03-17

### Added
- `--model` flag and `APPROP_MODEL` environment variable on `extract` command — override the default LLM model
- `upgrade` command — migrate extraction data to the latest schema version and re-verify without LLM
- `audit` command (replaces `report`) — detailed verification breakdown per bill
- `compare` command warns when comparing different bill classifications (e.g., supplemental vs. omnibus)
- `amount_status` field in search output — `found`, `found_multiple`, or `not_found`
- `quality` field in search output — `strong`, `moderate`, or `weak` derived from verification data
- `match_tier` field in search output — `exact`, `normalized`, `spaceless`, or `no_match`
- `schema_version` field in `extraction.json` and `verification.json`
- 18 integration tests covering all CLI commands with pinned budget authority totals

### Changed
- `report` command renamed to `audit` (`report` kept as alias)
- Search output field `verified` renamed to `amount_status` with richer values
- `compare` output status labels changed: `eliminated` → `only in base`, `new` → `only in current`
- `arithmetic_checks` field in `verification.json` deprecated — omitted from new files, old files still load

### Removed
- `hallucinated` terminology removed from all output and documentation

## [1.2.0] — 2026-03-16

### Added
- `audit` command with column guide explaining every metric
- `compare` command guard rails for cross-classification comparisons

### Changed
- Terminology overhaul: `report` → `audit` throughout documentation

## [1.1.0] — 2026-03-16

### Added
- Schema versioning (`schema_version: "1.0"`) in extraction and verification files
- `upgrade` command for migrating pre-versioned data
- Verification clarity improvements — column guide in `audit` output

### Fixed
- `SuchSums` amount variants now serialize correctly (fixed via upgrade path)

## [1.0.0] — 2026-03-16

Initial release.

### Features
- Download enrolled bill XML from Congress.gov API
- Parse congressional XML with `roxmltree` (pure Rust)
- Extract spending provisions via Claude with parallel chunk processing
- Deterministic verification of dollar amounts against source text
- `search` command with filters by type, agency, account, keyword, bill
- `summary` command with budget authority totals computed from provisions
- `compare` command for account-level diffs between bill sets
- CSV and JSON export formats
- Pre-extracted example data for three 118th Congress bills (FY2024 omnibus, continuing resolution, VA supplemental)