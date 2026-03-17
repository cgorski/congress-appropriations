# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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