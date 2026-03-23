# NEXT STEPS — Context Handoff

This file is gitignored. It lives in the repo root for passing context between sessions. It contains everything a new developer or AI assistant needs to pick up where we left off: how to build and test, how the code flows, what's been built, what hasn't, and plans for future work.

Last updated: 2026-03-22 (v6.0.0 local)

## Current State (v6.0.0)

### Dataset
- **32 bills** across 4 congresses (116th–119th), FY2019–FY2026
- **34,568 provisions** extracted, verified, and enriched
- **$21.5 trillion** in budget authority
- **100% source traceability** — every provision has a `source_span` byte position in the enrolled bill
- **99.995% dollar verification** — 18,583/18,584 dollar amounts confirmed in source text
- **99.4% TAS resolution** — 6,645/6,685 top-level appropriations mapped to Federal Account Symbols
- **1,051 authorities** in the account registry, 937 linked across multiple bills, 40 rename events detected
- **32/32 bills embedded** for semantic search

### Pipeline (fully implemented and tested)
```
download → extract → verify-text → enrich → resolve-tas → embed → authority build
                                                                        ↓
                                                          trace / compare / search
```

Each step adds files without modifying previous outputs. The hash chain detects staleness.

### New in v6.0.0 (built this session)
- **`verify-text` command** — deterministic raw_text repair + source_span byte positions. 3-tier algorithm (prefix → substring → normalized). Zero LLM calls. Module: `src/approp/text_repair.rs` (675 lines, 9 unit tests).
- **`resolve-tas` command** — Treasury Account Symbol resolution via FAST Book + Claude Opus. Two-tier: deterministic (55.8%) + LLM (43.6%). Module: `src/approp/tas.rs` (1,188 lines, 16 unit tests).
- **`authority build/list` commands + `trace` command** — account registry with name variants, rename events, fiscal year timelines. Module: `src/approp/authority.rs` (1,044 lines, 11 unit tests).
- **`compare --use-authorities`** — TAS-based orphan rescue in cross-bill comparisons. Reduced THUD FY2024→FY2026 orphans from 24 to 4.
- **`fas_reference.json`** — 2,768 FAS codes from the FAST Book (bundled reference data).
- **Authority events** — 40 rename events detected, variant classification (canonical/case/prefix/name_change/inconsistent).
- **All 32 bills re-extracted, verified, enriched, TAS-resolved, and embedded.**

### Code quality
- **~21,000 lines** of Rust across `src/`
- **~3,000 new lines** in 3 modules (text_repair, tas, authority)
- **255 tests** (204 unit + 51 integration), all passing
- **Clippy clean** (`cargo clippy -- -D warnings`)
- **No breaking changes** to existing commands (all new features are additive)

### What's NOT shipped (future work)
- **Documentation** — README updated, CHANGELOG written, but book chapters for new features not yet written
- **`process` command** — single command to run the full pipeline
- **`compare --use-authorities` as default** — currently opt-in via flag
- **Deterministic TAS rate improvement** (55.8% → 80%+) — expand agency name mapping table
- **Authority events: splits, merges, agency moves** — only renames are detected currently
- **Additional congresses** — 115th (FY2018/2019), 114th (FY2016/2017) for longer historical coverage
- **`relate --analyze`** — LLM-generated budget analysis narrative
- **Web interface** — serve the library API over HTTP
- **Visualization exports** — chart-ready data formats

### How to Build, Test, and Ship

#### Prerequisites
- **Rust 1.93+** with edition 2024
- For extraction: `ANTHROPIC_API_KEY`
- For TAS resolution (LLM tier): `ANTHROPIC_API_KEY`
- For embeddings: `OPENAI_API_KEY`
- For downloading: `CONGRESS_API_KEY`
- **None needed** for querying pre-extracted data or running verify-text/enrich/authority build

#### Build and test
```bash
cargo install --path .
cargo fmt
cargo clippy -- -D warnings
cargo test                          # 255 tests (204 unit + 51 integration)
```

#### Process a new bill
```bash
congress-approp download --congress 119 --type hr --number 9999
congress-approp extract --dir data/119-hr9999 --parallel 5
congress-approp verify-text --dir data --bill 119-hr9999 --repair
congress-approp enrich --dir data/119-hr9999
congress-approp resolve-tas --dir data --bill 119-hr9999
congress-approp embed --dir data/119-hr9999
congress-approp authority build --dir data --force
```

#### API key locations
```bash
source ~/congress_api.source          # CONGRESS_API_KEY
source ~/anthropic_key.source         # ANTHROPIC_API_KEY
source ~/openai-cantina-gorski.source # OPENAI_API_KEY
```

### Codebase stats (v6.0.0)
- `main.rs`: ~5,500 lines (CLI handlers + clap definitions)
- `query.rs`: ~1,600 lines (library API)
- `bill_meta.rs`: ~1,360 lines (enrichment types + classification)
- `tas.rs`: ~1,190 lines (TAS resolution — NEW)
- `authority.rs`: ~1,050 lines (authority registry — NEW)
- `normalize.rs`: ~1,100 lines (entity resolution)
- `ontology.rs`: ~900 lines (all data types, dead code removed)
- `extraction.rs`: ~860 lines (parallel chunk extraction)
- `from_value.rs`: ~690 lines (resilient LLM JSON parsing)
- `text_repair.rs`: ~675 lines (verify-text — NEW)
- `text_index.rs`: ~670 lines (dollar/section indexing)
- `xml.rs`: ~590 lines (congressional XML parser)
- `links.rs`: ~790 lines (cross-bill link types + suggest)
- `cache.rs`: ~420 lines (suggest/accept workflow caching)
- `inflation.rs`: ~430 lines (CPI data + real delta computation)
- `verification.rs`: ~370 lines (deterministic verification)
- `prompts.rs`: ~310 lines (LLM system prompt)
- `embeddings.rs`: ~260 lines (embedding storage + cosine similarity)
- `loading.rs`: ~340 lines (directory walking + bill loading)
- `staleness.rs`: ~165 lines (hash chain checking)
- `progress.rs`: ~170 lines (extraction progress bar)
- Total: ~21,000 lines of Rust across src/
- Tests: 255 (204 unit + 51 integration)
- Documentation: ~16,000 lines across ~49 mdbook chapters (updates pending)
- test-data/: 3 bills, ~500KB (ships with crate)
- data/: 32 bills, ~500MB (git only)

### All CLI commands (v6.0.0)
```
congress-approp download       --congress N [--type hr --number N] [--enacted-only] [--dry-run]
congress-approp extract        --dir DIR [--parallel N] [--model MODEL] [--force]
congress-approp verify-text    --dir DIR [--repair] [--bill BILL]                    ← NEW
congress-approp enrich         --dir DIR [--dry-run] [--force]
congress-approp resolve-tas    --dir DIR [--bill BILL] [--no-llm] [--dry-run]        ← NEW
congress-approp embed          --dir DIR [--model M] [--dimensions D]
congress-approp authority build --dir DIR [--force]                                   ← NEW
congress-approp authority list  --dir DIR [--agency CODE] [--format F]                ← NEW
congress-approp trace          QUERY --dir DIR [--format F]                           ← NEW
congress-approp search         --dir DIR [--semantic Q] [--keyword KW] [--type T] [--fy Y]
congress-approp summary        --dir DIR [--fy Y] [--subcommittee S] [--show-advance]
congress-approp compare        --base-fy Y --current-fy Y --dir DIR [--use-authorities] ← NEW FLAG
congress-approp audit          --dir DIR [--verbose]
congress-approp relate         SOURCE --dir DIR [--top N] [--fy-timeline]
congress-approp link suggest   --dir DIR [--threshold F] [--scope S]
congress-approp link accept    --dir DIR [HASHES...] [--auto]
congress-approp normalize suggest-text-match --dir DIR
congress-approp normalize accept --dir DIR [HASHES...] [--auto]
```

## How the Code Flows (v6.0.0)

### Code structure overview
```
src/
  main.rs                    ← CLI entry point, clap arg parsing, output formatting (~5,500 lines)
  lib.rs                     ← Re-exports: api::, approp::, load_bills, query, bill_meta, normalize
  api/
    anthropic/               ← Claude API client (extraction, TAS resolution, normalize suggest-llm)
    congress/                ← Congress.gov API client (bill download)
    openai/                  ← OpenAI API client (embeddings only)
  approp/
    authority.rs             ← NEW v6.0: Account authority registry — build, query, events (~1,050 lines)
    tas.rs                   ← NEW v6.0: TAS resolution — FAST Book matching + LLM fallback (~1,190 lines)
    text_repair.rs           ← NEW v6.0: verify-text — 3-tier raw_text repair + source spans (~675 lines)
    bill_meta.rs             ← Bill metadata types + classification functions (~1,360 lines)
    cache.rs                 ← Suggest/accept workflow caching (~/.congress-approp/cache/)
    normalize.rs             ← Entity resolution types + suggest algorithms + LLM support (~1,100 lines)
    links.rs                 ← Cross-bill link types + suggest algorithm (~790 lines)
    ontology.rs              ← ALL data types — Provision enum, BillExtraction, TextSpan (~900 lines)
    extraction.rs            ← ExtractionPipeline: chunk splitting, parallel LLM calls (~860 lines)
    from_value.rs            ← Resilient JSON→Provision parsing (handles LLM quirks) (~690 lines)
    xml.rs                   ← Congressional bill XML parsing via roxmltree (~590 lines)
    text_index.rs            ← Dollar amount indexing, section detection, chunking (~670 lines)
    inflation.rs             ← CPI data loading, fiscal-year-weighted averages (~430 lines)
    prompts.rs               ← System prompt for Claude + TAS resolution prompt (~310 lines)
    verification.rs          ← Deterministic dollar-amount + raw-text verification (~370 lines)
    loading.rs               ← Directory walking, JSON loading, LoadedBill assembly (~340 lines)
    query.rs                 ← Library API: search, compare, summarize, audit, relate (~1,600 lines)
    embeddings.rs            ← Embedding storage (JSON+binary), cosine similarity (~260 lines)
    staleness.rs             ← Hash chain checking (XML→extraction→embeddings→bill_meta) (~165 lines)
    progress.rs              ← Extraction progress bar (~170 lines)
tests/
  cli_tests.rs               ← 51 integration tests (Tier 1: test-data, Tier 2: data with auto-skip)
test-data/                   ← 3 small bills for crate tests (ships with crate)
data/                        ← Full dataset (git only, excluded from crate)
  {congress}-{type}{number}/ ← Bill directories (e.g., 118-hr4366, 119-hr7148)
  fas_reference.json         ← FAST Book reference (2,768 FAS codes, bundled)
  authorities.json           ← Account authority registry (built by authority build)
  dataset.json               ← Entity resolution rules (created by normalize accept)
  links/links.json           ← Provision-level relationships (created by link accept)
docs/
  ARCHITECTURE.md
  FIELD_REFERENCE.md         ← Per-field documentation including all JSON schemas
book/
  src/                       ← mdbook documentation (~53 chapters including 4 new)
```

### Key architectural decisions (v6.0.0)

1. **FAS codes as stable identifiers.** Cross-bill account tracking uses Federal Account Symbols from the Treasury's FAST Book — government-assigned codes that persist through renames and reorganizations. No invented identifiers.

2. **Two-tier TAS resolution.** Deterministic string matching for unambiguous names (~56%, zero false positives), Claude Opus for ambiguous cases (~44%, verified against FAST Book). One-time cost per bill.

3. **100% source traceability.** Every provision carries a `source_span` with UTF-8 byte offsets into the enrolled bill text. The invariant `source_bytes[start..end] == raw_text` is mechanically verifiable. Achieved via deterministic 3-tier repair (prefix → substring → normalized position mapping) with zero LLM calls.

4. **Layered pipeline, each step adds data.** extract → verify-text → enrich → resolve-tas → embed → authority build. Each step produces new files without modifying previous outputs. The hash chain detects staleness.

5. **No implicit normalization.** Agency matching uses exact lowercased strings by default. All normalization is explicit via dataset.json (normalize commands) or TAS codes (resolve-tas + authority system).

6. **Suggest/accept pattern with cache.** Both normalize and link commands use: suggest → cache → accept → persistent storage. No implicit recomputation.

7. **Authority events from data.** Rename detection is automatic — when an authority's name variants show a clear temporal boundary (one name before FY X, another after), a rename event is recorded. No manual annotation needed.

8. **Inline source_span on provisions.** Written at the JSON Value level (not through the typed Provision enum) so it's available to Python, JavaScript, and other consumers without modifying the 11-variant Rust enum. Rust's deserializer ignores the field (Serde skips unknown fields).

### How TAS resolution works
1. Load `fas_reference.json` (bundled FAST Book data: 2,768 FAS codes)
2. For each top-level BA provision, try deterministic matching:
   - Direct: lowercase account name == FAS short title (unique match)
   - Short-title: first comma segment of account name == FAS short title
   - Suffix: strip em-dash prefix, match remainder
   - Agency disambiguation: when multiple FAS codes share the name, use CGAC agency code
   - DOD branch detection: "Operation and Maintenance, Army" under "Department of Defense" → code 021
3. If no unambiguous match, send to Claude Opus with the agency's FAS codes
4. Verify every LLM result against the FAST Book
5. Write `tas_mapping.json` per bill

### How the authority system works
1. `authority build` scans all `tas_mapping.json` files in the data directory
2. Groups provisions by FAS code into authorities
3. For each authority: collects name variants, classifies them (case/prefix/name_change/inconsistent)
4. Detects rename events by finding temporal boundaries in name usage
5. Writes `authorities.json` with 1,051 authorities, 937 cross-bill links, 40 rename events
6. `trace` queries the registry by FAS code or name search

### How entity resolution works (older system, still functional)
1. `normalize suggest-text-match --dir data` → scans bills for orphan pairs, caches results
2. User reviews suggestions, optionally runs `suggest-llm` for ambiguous ones
3. `normalize accept HASH1 HASH2 --dir data` → reads cache, writes dataset.json
4. `compare` loads dataset.json, applies agency groups during matching
5. `compare --exact` ignores dataset.json entirely
6. `compare --use-authorities` uses TAS-based matching (supersedes entity resolution for most cases)

### How the cache works
- Location: ~/.congress-approp/cache/
- Files: suggest-text-match-{key}.json, suggest-llm-{key}.json, link-suggest-{key}.json
- Key: SHA-256 of canonical data directory path (12 hex chars)
- Invalidation: data_hash field = SHA-256 of extraction.json modification times

## Comprehensive Future Work

### Immediate (next session)

1. **Push to git and publish** — 3 local commits ahead of origin, 255 tests passing, clippy clean
2. **Update remaining book chapters** — ~10 existing chapters need updated numbers (pipeline diagram, data directory reference, etc.)

### Short-term enhancements

3. **`process` command** — single command to run the full pipeline (download → authority build). ~100 lines. Skips already-completed steps per bill.
4. **`compare --use-authorities` as default** — when `tas_mapping.json` files exist, use TAS-based matching automatically. Fall back to name-based matching when TAS mappings are absent.
5. **Improve deterministic TAS rate** (55.8% → 80%+) — expand the `AGENCY_NAME_TO_CODE` table from ~80 entries to ~200. The 37% of LLM-resolved provisions that failed because of unmapped agency names would become deterministic matches. Reduces per-bill LLM cost from ~$2-4 to ~$0.50-1.
6. **Extraction resume/checkpoint** — `.extraction_progress.json` for recovering from interrupted omnibus extractions. ~80 lines in extraction.rs. Currently if extraction fails partway, no results are saved (unless `--continue-on-error`).
7. **Resolve-tas cost tracking** — add `tokens.json`-style output for `resolve-tas` LLM calls, like `extract` already has.

### Medium-term features

8. **Additional congresses** — 115th (FY2018/2019), 114th (FY2016/2017), 113th (FY2014/2015) for longer historical coverage. XML is available on Congress.gov. The pipeline handles them — just needs download + extract + resolve-tas.
9. **Authority events: splits, merges, agency moves** — currently only renames are detected. Account splits (one account becomes two) and merges (two become one) require comparing provision counts and dollar magnitudes across fiscal years. Agency moves (Secret Service: Treasury → DHS) require historical FAST Book editions or manual annotation.
10. **Structural identity from XML headings** — use `<appropriations-major>` and `<appropriations-intermediate>` tags as a matching signal alongside FAS codes. These headings are ground truth from the Government Publishing Office and are more stable than LLM-extracted agency names.
11. **`relate --analyze`** — send matched provisions to Claude for LLM-generated budget analysis narrative with trend descriptions and caveats. The library function produces structured data; the CLI enriches with LLM narrative.
12. **Attribution confidence scoring** — combine amount uniqueness (how many times the dollar string appears in source) with text match tier to produce a per-provision confidence score for citation safety. Designed in v4.0 spec but never built.

### Long-term vision

13. **Web interface** — serve the library API over HTTP. Allow browser-based querying, timeline visualization, and comparison dashboards.
14. **Visualization exports** — chart-ready data formats for D3, Observable, Plotly. The `trace --format json` output is already usable but lacks aggregation features.
15. **Mandatory spending coverage** — the current tool covers discretionary appropriations (26% of federal spending). Extending to mandatory spending (Social Security, Medicare) requires different source documents (authorizing legislation, not appropriations bills).
16. **USASpending cross-reference** — compare extracted budget authority against USASpending obligation data by FAS code. Requires understanding that BA ≠ obligations ≠ outlays.
17. **Historical FAST Book editions** — download archived FAST Books from the Wayback Machine (editions back to 2005 exist as PDFs) to extend the FAS reference for pre-2003 accounts (Secret Service under Treasury, pre-DHS FEMA, etc.).
18. **Per-unique-name TAS resolution** — instead of resolving per-provision (6,685 LLM calls), resolve per-unique-account-name (~600 names), cache the mapping, and apply deterministically. Reduces LLM cost to ~$5 for the full dataset.

## Files to Know (v6.0.0)

| File | Lines | Purpose | When to Edit |
|------|-------|---------|-------------|
| `src/main.rs` | ~5,500 | CLI handlers, clap definitions, output formatting | Adding new commands or flags |
| `src/approp/query.rs` | ~1,600 | Library API: search, compare, summarize, audit, relate | Adding query functions |
| `src/approp/tas.rs` | ~1,190 | TAS resolution: FAST Book matching, LLM fallback, agency mapping | TAS features |
| `src/approp/authority.rs` | ~1,050 | Authority registry: build, query, events, name variants | Authority features |
| `src/approp/text_repair.rs` | ~675 | verify-text: 3-tier repair, source spans | Source traceability |
| `src/approp/bill_meta.rs` | ~1,360 | Enrichment: FY, subcommittees, advance detection | Classification |
| `src/approp/normalize.rs` | ~1,100 | Entity resolution: suggest algorithms, LLM support | Entity resolution |
| `src/approp/ontology.rs` | ~900 | All types: Provision (11 variants), TextSpan, BillExtraction | Type changes |
| `src/approp/extraction.rs` | ~860 | ExtractionPipeline, parallel chunk processing | Extraction |
| `src/approp/links.rs` | ~790 | Cross-bill links: types, suggest, accept/remove | Link features |
| `tests/cli_tests.rs` | ~1,600 | 51 integration tests (Tier 1 + Tier 2) | New command tests |
| `data/fas_reference.json` | ~1.5MB | FAST Book reference data (2,768 FAS codes) | When FAST Book is updated |
| `scripts/convert_fast_book.py` | ~380 | Convert FAST Book Excel → JSON | When updating reference data |

### Key patterns to follow when adding code
1. **Library function first, CLI second.** New logic goes in the appropriate `approp/` module. CLI handler calls library and formats output.
2. **All query functions take `&[LoadedBill]` and return structs.** No I/O, no formatting, no side effects.
3. **Serde for everything.** All data types derive `Serialize`/`Deserialize`. Output structs derive `Serialize`.
4. **Tests in the same file.** Unit tests go in `#[cfg(test)] mod tests { }` at the bottom.
5. **Clippy clean with `-D warnings`.** Fix at root cause, not with `#[allow]`.
6. **No team member names in code, comments, or docs.** (NEXT_STEPS methodology section is the exception.)
7. **Source spans use UTF-8 byte offsets.** Document this explicitly on any new position-based types.
8. **TAS confidence is binary for deterministic matches.** `verified` (mechanically provable) or `unmatched` (needs LLM). No guessing.
9. **Documentation tone is direct and factual.** No marketing language ("Turn spending bills into data!"), no breathless phrasing ("Copy-paste and go!"), no audience labels ("For Journalists"). State what the tool does, let the data speak, describe limitations plainly. Specific dataset numbers belong only in the cookbook dataset card and accuracy-metrics appendix — other pages use relative language and link to those references.

## Technical Decisions and Rationale (v6.0.0)

### Why FAS codes instead of invented identifiers
The Federal Account Symbol is the government's own identifier — assigned by Treasury, published in the FAST Book, used across USASpending.gov and OMB budget data. Using it means our identifiers are interoperable with the entire federal financial ecosystem. Inventing our own IDs (UUIDs, ULIDs) would require a mapping table back to FAS anyway.

### Why two-tier TAS resolution instead of all-LLM
Deterministic matching on 56% of provisions costs $0 and runs instantly. The LLM is reserved for genuinely ambiguous cases (151 agencies with "Salaries and Expenses", DOD service branch disambiguation). This keeps costs proportional to ambiguity, not dataset size. A $85 one-time cost for 32 bills is acceptable.

### Why source spans use byte offsets, not character offsets
Rust's native `str` indexing operates on UTF-8 byte positions. Using byte offsets means the invariant `source[span.start..span.end] == raw_text` works directly in Rust without conversion. Languages that use character-based indexing (Python, JavaScript) need byte-level slicing — this is documented in the `TextSpan` type and the verify-data how-to chapter.

### Why inline source_span on provisions instead of a parallel array
The Python repair script established the inline convention across all 32 bills before the Rust implementation was built. The Rust `verify-text` command works at the `serde_json::Value` level to read and write this field without modifying the 11-variant Provision enum. The parallel array approach was prototyped but rejected because it creates index-sync fragility.

### Why containment matching was removed from TAS resolution
The original deterministic matcher used substring containment: if a FAS title contained the provision name or vice versa, it matched. This produced 1,618 false positives — 902 provisions from every agency matched to a single Senate Legal Counsel account because all contained "Salaries and Expenses." The fix: only match when the name is unique or the agency code disambiguates. Zero false positives.

### Why authority events detect renames but not splits/merges
Rename detection is straightforward: when the same FAS code has different names with a clear temporal boundary, record the transition. Split and merge detection requires comparing provision counts and dollar magnitudes across fiscal years — a more complex heuristic with higher false-positive risk. Renames cover the most common case; splits/merges are deferred.

### Why the tool covers discretionary appropriations only
Discretionary spending flows through the 12 annual appropriations bills — structured, enacted legislation available as XML from Congress.gov. Mandatory spending (Social Security, Medicare) is authorized by permanent law, not annual appropriations. Different source documents require different extraction approaches. The scope limitation is documented prominently in the README and authority system chapter.

## Gotchas and Things That Tripped Us Up

1. **Byte offsets vs character offsets.** Rust's `str::find` returns byte offsets. Python's `str.find` returns character offsets. In files with multi-byte UTF-8 characters (curly quotes = 3 bytes each), these differ. The `TextSpan` fields are byte offsets. Python consumers must use `open(path, 'rb').read()[start:end].decode('utf-8')`.

2. **"Salaries and Expenses" is the most common account name.** 151 FAS codes share it. Any matching strategy that doesn't consider agency context will produce false positives.

3. **DOD service branches have their own agency codes.** Army = 021, Navy/Marines = 017, Air Force/Space Force = 057, DOD umbrella = 097. The LLM often extracts agency as "Department of Defense" even for service-specific accounts. The TAS resolver detects service branches from account names (", Army", ", Navy", etc.).

4. **The LLM prompt's batch indices must match provision indices.** When batching unmatched provisions for LLM resolution, the prompt must label each provision with its real `provision_index` from the extraction, not a batch-local sequential number. Otherwise `apply_llm_results` can't match responses to provisions.

5. **Multi-FY bills appear in multiple fiscal year timelines.** H.R. 815 covers FY2024-2026. Its provisions show up in all three FYs in the `trace` output. This is correct (the bill provides authority for those years) but can cause confusion if interpreted as separate spending.

6. **CR and supplemental labels prevent misinterpretation.** FY2025 Secret Service shows $231M from H.R. 9747 — a CR supplement, not the full-year level. The `(CR)` label in `trace` output prevents journalists from reporting a 92% budget cut.

7. **The FAST Book is updated periodically.** When Treasury adds or renames accounts, run `scripts/convert_fast_book.py` to regenerate `fas_reference.json`, then `resolve-tas --force` to re-resolve.

8. **`compare --use-authorities` matches by dollar amount.** The TAS rescue logic finds orphan rows where the base_dollars match a FAS code's base-side total and the current-side FAS total fills in the missing value. This means it can't rescue orphans where the dollar amounts were already partially matched by name-based comparison.

## Verified Metrics (v6.0.0)

These numbers are measured, not estimated. Each can be reproduced by running the indicated command.

| Metric | Value | How to verify |
|--------|-------|--------------|
| Source traceability | 100.000% (34,568/34,568) | `congress-approp verify-text --dir data` |
| Dollar verification | 99.995% (18,583/18,584) | `congress-approp audit --dir data` |
| TAS resolution | 99.4% (6,645/6,685) | Sum from all `tas_mapping.json` files |
| Budget regression | 8/8 pinned bills match | `cargo test budget_authority_totals` |
| Tests | 255 passing (204 unit + 51 integration) | `cargo test` |
| Clippy | Clean | `cargo clippy -- -D warnings` |
| Authorities | 1,051 with 937 cross-bill links | `authorities.json` summary |
| Rename events | 40 detected | `authorities.json` total_events |
| Name variants classified | 1,643 across 443 authorities | `authorities.json` |

## Reference Data Sources

| Source | URL | What we use it for |
|--------|-----|-------------------|
| **FAST Book Part II** | [tfx.treasury.gov/reference-books/fast-book](https://tfx.treasury.gov/reference-books/fast-book) | FAS code reference (2,768 accounts). Downloaded as Excel, converted to `fas_reference.json` via `scripts/convert_fast_book.py`. |
| **USASpending API** | [api.usaspending.gov](https://api.usaspending.gov) | Historical sub-TAS name variants. No API key required. Used in Python POC for validation. |
| **Congress.gov API** | [api.congress.gov](https://api.congress.gov) | Bill XML download. Free API key. |
| **OMB Historical Tables** | [whitehouse.gov/omb](https://www.whitehouse.gov/omb/information-resources/budget/historical-tables/) | Agency-level budget data back to FY1962. Used for validation, not integrated into the tool. |
| **CPI-U (BLS)** | Bundled in `src/approp/cpi.json` | Inflation adjustment for `compare --real`. Monthly CPI-U values, fiscal-year-weighted averages. |

---

## Historical Context (v4.0–v5.1)

The sections below are preserved for reference. They document the design decisions and implementation details of earlier versions. The v6.0.0 sections above supersede them for current development.

---

### v4.0.0 (shipped 2026-03-19)
- All v3.2.0 commands: `download`, `extract`, `search`, `summary`, `compare`, `audit`, `upgrade`, `embed`
- **New in v4.0:** `enrich`, `relate`, `link suggest`, `link accept`, `link remove`, `link list`
- `--semantic "query"` and `--similar bill:index` on search (uses OpenAI embeddings)
- `--fy <YEAR>` on summary/search/compare — fiscal year filtering
- `--subcommittee <SLUG>` on summary/search/compare — jurisdiction scoping (requires `enrich`)
- `--show-advance` on summary — separates current-year from advance budget authority
- `--base-fy`/`--current-fy` on compare — FY-based year-over-year comparison
- `--use-links` on compare — link-aware matching for renames
- `--fy-timeline` on relate — fiscal year timeline with advance/current/supplemental split
- Case-insensitive account matching in compare (52 false orphans resolved)
- Sub-agency normalization in compare (35-entry lookup table, slash handling, US/U.S. variants)
- Cross-semantics orphan rescue in compare (Transit Formula Grants $14.6B recovered)
- Enriched bill classifications (Full-Year CR with Appropriations, Minibus, etc.)
- FY-aware advance appropriation detection ($1.49T identified across 13 bills)
- Deterministic link hashes bridging relate (discovery) and links (persistence)
- Helpful empty-result messages showing available FYs and subcommittees
- `bill_meta.rs` module: types, XML parsing, jurisdiction classification, advance detection
- `links.rs` module: types, suggest algorithm, accept/remove, load/save
- `query.rs` expanded: `relate()`, `compute_link_hash()`, `normalize_agency()`, `compute_advance_split()`
- Summary handler consolidated to call `query::summarize()` (eliminated inline reimplementation)
- Pre-generated embeddings for all 13 example bills (3072 dimensions, text-embedding-3-large)
- Pre-enriched bill_meta.json for all 13 example bills
- 172 tests (130 unit + 42 integration), all passing
- Comprehensive mdbook documentation (~49 chapters) deployed to GitHub Pages
- ARCHITECTURE.md, CHANGELOG.md, FIELD_REFERENCE.md all updated for v4.0

### Dataset: 13 enacted appropriations bills
All in `examples/` directory, each with source XML, extraction.json, verification.json, metadata.json, embeddings.json, and vectors.bin.

**118th Congress (FY2024/FY2025):**
| Directory | Bill | Type | Provisions | Budget Auth |
|-----------|------|------|-----------|------------|
| `examples/hr4366/` | H.R. 4366 | FY2024 omnibus (MilCon-VA, Ag, CJS, E&W, Interior, THUD) | 2,364 | $846B |
| `examples/hr5860/` | H.R. 5860 | FY2024 initial CR + 13 anomalies | 130 | $16B |
| `examples/hr9468/` | H.R. 9468 | VA supplemental | 7 | $2.9B |
| `examples/hr815/` | H.R. 815 | Ukraine/Israel/Taiwan supplemental | 303 | $95B |
| `examples/hr2872/` | H.R. 2872 | Further CR (FY2024) | 31 | $0 |
| `examples/hr6363/` | H.R. 6363 | Further CR + extensions | 74 | ~$0 |
| `examples/hr7463/` | H.R. 7463 | CR extension | 10 | $0 |
| `examples/hr9747/` | H.R. 9747 | CR + extensions (FY2025) | 114 | $383M |
| `examples/s870/` | S. 870 | Fire Admin authorization | 49 | $0 |

**119th Congress (FY2025/FY2026):**
| Directory | Bill | Type | Provisions | Budget Auth |
|-----------|------|------|-----------|------------|
| `examples/hr1968/` | H.R. 1968 | Full-year CR (FY2025) — contains full-year appropriations | 526 | $1,786B |
| `examples/hr5371/` | H.R. 5371 | Minibus: CR + Ag + LegBranch + MilCon-VA | 1,048 | $681B |
| `examples/hr6938/` | H.R. 6938 | Minibus: CJS + Energy-Water + Interior | 1,061 | $196B |
| `examples/hr7148/` | H.R. 7148 | Omnibus: Defense + Labor-HHS + THUD + FinServ + State-ForeignOps | 2,837 | $2,788B |

**Totals:** 8,554 provisions, 0 unverifiable dollar amounts, 95.5% raw text exact match.

**Missing:** H.R. 2882 (FY2024 second omnibus covering Defense, Labor-HHS, Homeland, State, FinServ, LegBranch). Extraction failed due to 15 persistent chunk failures in Division F-VII and Division G. The enrolled XML is available on Congress.gov if someone wants to retry.

### What's NOT shipped (future work)
- **Extraction resume/checkpoint** — `.extraction_progress.json` for interrupted extractions. H.R. 2882 (FY2024 Defense omnibus) failed on 15 chunks with no way to resume. ~80 lines of changes to extraction.rs.
- **`link suggest --verify`** — LLM-assisted verification of uncertain link candidates. Sends ambiguous pairs to Claude for SAME/DIFFERENT classification. Requires ANTHROPIC_API_KEY.
- **`relate --analyze`** — LLM-generated budget analysis narrative. Sends matched provisions to Claude for trend analysis and publishable language. Requires ANTHROPIC_API_KEY.
- **`--show-advance` on compare** — per-account advance/current split in comparison deltas. Currently only on summary.
- **`--multi-query` on search** — try multiple phrasings and union results for semantic search.

### Known limitations (documented in CHANGELOG)
- **Sub-agency mismatches** — ~5-15 false orphans per subcommittee in compare from agencies not in the 35-entry lookup table. CJS worst at 78% match rate.
- **`compare --use-links`** uses string containment matching on labels — could produce false matches for very short account names. No confirmed issues.
- **appendix/example-bills.md** still lists 3 bills (cosmetic, not user-facing).
- **main.rs** is 4,200+ lines — the search handler has substantial inline formatting. Tech debt.
- **Integration tests** take ~60s due to link tests copying vectors.bin.

### Codebase stats
- `main.rs`: ~4,200 lines (CLI handlers + clap definitions)
- `query.rs`: ~1,300 lines (library API — summarize, search, compare, audit, relate, link hashes)
- `bill_meta.rs`: ~1,280 lines (enrichment types + classification functions + 33 unit tests)
- `links.rs`: ~790 lines (link types + suggest algorithm + I/O + 10 unit tests)
- `ontology.rs`: ~960 lines (all data types)
- `extraction.rs`: ~840 lines (parallel chunk extraction)
- `from_value.rs`: ~690 lines (resilient LLM JSON parsing)
- `xml.rs`: ~590 lines (congressional XML parser)
- `text_index.rs`: ~670 lines (dollar/section indexing)
- `verification.rs`: ~370 lines (deterministic verification)
- `embeddings.rs`: ~260 lines (embedding storage + cosine similarity)
- `loading.rs`: ~340 lines (directory walking + bill loading + bill_meta)
- `prompts.rs`: ~310 lines (LLM system prompt)
- `staleness.rs`: ~165 lines (hash chain checking + bill_meta staleness)
- `progress.rs`: ~170 lines (extraction progress bar)
- Total: ~16,000 lines of Rust across src/
- Tests: ~1,200 lines in `tests/cli_tests.rs`
- Documentation: ~15,500 lines across ~49 mdbook chapters

### API keys used
| Key | Environment Variable | Used by | Required for |
|-----|---------------------|---------|-------------|
| Congress.gov | `CONGRESS_API_KEY` | `download` command | Downloading bill XML |
| Anthropic | `ANTHROPIC_API_KEY` | `extract` command | LLM extraction of provisions |
| OpenAI | `OPENAI_API_KEY` | `embed` command, `--semantic` search | Generating and querying embeddings |

None are needed for querying pre-extracted/pre-embedded example data (except `--semantic` which needs OpenAI to embed the query at search time).

---

## How the Code Flows

### Code structure overview
```
src/
  main.rs                    ← CLI entry point, clap arg parsing, output formatting (~4,200 lines)
  lib.rs                     ← Re-exports: api::, approp::, load_bills, query, bill_meta
  api/
    mod.rs                   ← pub mod anthropic; pub mod congress; pub mod openai;
    anthropic/               ← Claude API client (extraction)
    congress/                ← Congress.gov API client (bill download)
    openai/                  ← OpenAI API client (embeddings only, ~75 lines)
  approp/
    mod.rs                   ← pub mod for all submodules
    bill_meta.rs             ← NEW v4.0: Bill metadata types + classification functions (~1,280 lines)
    links.rs                 ← NEW v4.0: Cross-bill link types + suggest algorithm (~790 lines)
    ontology.rs              ← ALL data types (Provision enum, BillExtraction, etc.)
    extraction.rs            ← ExtractionPipeline: chunk splitting, parallel LLM calls
    from_value.rs            ← Resilient JSON→Provision parsing (handles LLM quirks)
    xml.rs                   ← Congressional bill XML parsing via roxmltree
    text_index.rs            ← Dollar amount indexing, section detection, chunking
    prompts.rs               ← System prompt for Claude (~300 lines of instructions)
    verification.rs          ← Deterministic dollar-amount + raw-text verification
    loading.rs               ← Directory walking, JSON loading, LoadedBill assembly (incl bill_meta)
    query.rs                 ← Library API: search, compare, summarize, audit, relate, link hashes
    embeddings.rs            ← Embedding storage (JSON+binary), cosine similarity
    staleness.rs             ← Hash chain checking (XML→extraction→embeddings→bill_meta)
    progress.rs              ← Extraction progress bar
tests/
  cli_tests.rs               ← 42 integration tests against examples/ data
docs/
  ARCHITECTURE.md            ← Comprehensive architecture doc
  FIELD_REFERENCE.md         ← Per-field documentation for JSON files
book/
  src/                       ← mdbook documentation (48 chapters)
examples/
  hr4366/                    ← FY2024 omnibus (2,364 provisions)
  hr5860/                    ← FY2024 continuing resolution (130 provisions)
  hr9468/                    ← VA supplemental (7 provisions)
  hr815/                     ← Ukraine/Israel supplemental (303 provisions)
  hr2872/                    ← Further CR (31 provisions)
  hr6363/                    ← Further CR + extensions (74 provisions)
  hr7463/                    ← CR extension (10 provisions)
  hr9747/                    ← CR + extensions (114 provisions)
  s870/                      ← Fire Admin authorization (49 provisions)
  hr1968/                    ← Full-year CR FY2025 (526 provisions)
  hr5371/                    ← Minibus: Ag+LegBranch+MilCon-VA (1,048 provisions)
  hr6938/                    ← Minibus: CJS+E&W+Interior (1,061 provisions)
  hr7148/                    ← Omnibus: Defense+Labor-HHS+THUD+FinServ+State (2,837 provisions)
  Each bill dir contains: BILLS-*.xml, extraction.json, verification.json,
    metadata.json, tokens.json, bill_meta.json, embeddings.json, vectors.bin
```

### The Provision enum (ontology.rs)

11 variants, each with different fields. All share common fields: `section`, `division`, `title`, `confidence`, `raw_text`, `notes`, `cross_references`.

| Variant | What it is | Key fields |
|---------|-----------|------------|
| `Appropriation` | Grant of budget authority | account_name, agency, amount, fiscal_year, detail_level, parent_account |
| `Rescission` | Cancellation of prior funds | account_name, amount, reference_law |
| `TransferAuthority` | Permission to move funds | from_scope, to_scope, limit, conditions |
| `Limitation` | Cap or prohibition | description, amount, account_name |
| `DirectedSpending` | Earmark/community project | account_name, amount, earmark |
| `CrSubstitution` | CR anomaly (substituting amounts) | new_amount, old_amount, account_name |
| `MandatorySpendingExtension` | Amends existing law | program_name, statutory_reference, amount |
| `Directive` | Reporting requirement | description, deadlines |
| `Rider` | Policy provision (not spending) | description, policy_area |
| `ContinuingResolutionBaseline` | Core CR mechanism | reference_year, rate, duration |
| `Other` | Fallback for unclassifiable | llm_classification, description |

Tagged serde: `#[serde(tag = "provision_type", rename_all = "snake_case")]`.

### How `search --semantic` flows through the code
1. User runs: `congress-approp search --dir examples --semantic "school lunch" --fy 2026 --subcommittee thud --top 5`
2. `main()` matches `Commands::Search` → calls `handle_search()` (async)
3. `handle_search()` applies FY filter (bills covering FY2026)
4. Detects `semantic.is_some()` → early return to `handle_semantic_search()` with FY-filtered bills (NOT subcommittee-filtered, to preserve vector indices)
5. `handle_semantic_search()`:
   a. Loads `embeddings::load()` for each bill to get vectors
   b. Calls `OpenAIClient::from_env()?.embed()` to embed the query text (single API call, ~100ms)
   c. For each provision in each bill: applies hard filters (type, division, dollars, subcommittee via bill_meta jurisdiction check), then computes `cosine_similarity(query_vector, provision_vector)`
   d. Subcommittee filter is applied inline during scoring (not via filter_bills_to_subcommittee) to preserve original provision indices for correct vector lookups
   e. Sorts by similarity descending, truncates to `top_n`
   f. Formats output (table/json/jsonl/csv)

### How `--similar` flows
Same as semantic but looks up the source provision's pre-computed vector from `vectors.bin` — no API call needed.

### How `enrich` works (NEW v4.0)
1. `handle_enrich()` loads all bills via `loading::load_bills()`
2. For each bill, skips if `bill_meta.json` exists (unless `--force`)
3. Finds XML source via `bill_meta::find_xml_in_dir()`
4. Parses division titles from XML via `roxmltree` (`<division><enum>` + `<header>`)
5. Classifies each division title to a jurisdiction via pattern matching
6. Classifies bill nature from provision type distribution + subcommittee count
7. For each BA provision: classifies advance/current/supplemental via FY-aware algorithm
8. Normalizes all account names (lowercase, em-dash prefix stripped)
9. Computes `sha256(extraction.json)` for hash chain
10. Writes `bill_meta.json`
**No API calls.** Entire process runs offline.

### How `relate` works (NEW v4.0)
1. `handle_relate()` loads all bills + embeddings
2. Finds source provision by `bill_dir:index` reference
3. Computes cosine similarity against every provision in every bill
4. Classifies matches: name match → "verified", sim≥0.65+same agency → "high", else → "uncertain"
5. Computes deterministic 8-char hash per match (sha256 of src:idx→tgt:idx:model)
6. If `--fy-timeline`: groups same-account matches by FY, looks up timing from bill_meta
7. Formats output (table with hashes/similarity/timing, json, or hashes-only for piping)

### How `link suggest` works (NEW v4.0)
1. Loads all bills + embeddings
2. For each top-level BA provision in each bill, computes similarity against all provisions in all other bills
3. Deduplicates bidirectional matches (A→B same as B→A, keep higher sim)
4. Classifies confidence: verified (name match), high (sim≥0.65+same agency), uncertain (0.55-0.65)
5. Outputs candidates with hashes for `link accept`

### How `embed` works
1. `handle_embed()` loads all bills via `loading::load_bills()`
2. For each bill, checks staleness: `sha256(extraction.json)` vs `embeddings.extraction_sha256`
3. If stale or missing: builds embedding text per provision via `query::build_embedding_text()`
4. Batches provisions (100 per API call) through `OpenAIClient::embed()`
5. Collects all float32 vectors, calls `embeddings::save()` which writes `embeddings.json` + `vectors.bin`

### How the hash chain works
```
BILLS-*.xml ──sha256──▶ metadata.json.source_xml_sha256
                              ↓ (if mismatch: "extraction is stale")
extraction.json ──sha256──▶ bill_meta.json.extraction_sha256
                              ↓ (if mismatch: "bill metadata is stale")
extraction.json ──sha256──▶ embeddings.json.extraction_sha256
                              ↓ (if mismatch: "embeddings are stale")
vectors.bin ──sha256──▶ embeddings.json.vectors_sha256
```

`staleness::check()` computes these hashes and compares. Warnings go to stderr, never block execution. The `BillMetaStale` variant was added in v4.0.

### How tests work
- **Unit tests** are inline `#[cfg(test)] mod tests` in each module. 130 total across bill_meta.rs (33), links.rs (10), query.rs (21), and others.
- **Integration tests** are in `tests/cli_tests.rs`. 42 tests run the actual binary against `examples/` data.
- **Budget total pinning**: `budget_authority_totals_match_expected` test hardcodes $846,137,099,554 / $16,000,000,000 / $2,882,482,000. These 3 original bills are always checked.
- **Enrich tests** copy small bills to temp dirs to avoid modifying examples/.
- **Link tests** use `copy_dir_with_vectors` to include embeddings. These take ~60s.
- **No semantic search integration test** because CI lacks OPENAI_API_KEY.
- **`--show-advance` test** verifies MilCon-VA advance split ($394B advance > $102B current).

---

## Problems Discovered During Analysis

The following issues were identified through comprehensive testing of the 13-bill dataset. Each problem has been validated with real data and real LLM/embedding experiments. These directly inform the v4.0 design.

### Problem 1: Division letters are bill-internal
Division "A" in H.R. 7148 is "Defense," in H.R. 6938 is "CJS," and in H.R. 5371 is "Continuing Appropriations." The `--division A` flag across multiple bills mixes completely unrelated provisions. The tool needs subcommittee/jurisdiction metadata that maps division letters to canonical jurisdictions per-bill.

**Validated:** Parsed XML `<toc-entry level="division">` elements from all 13 bills. Found 49 unique division titles, 32 matched by pattern to known jurisdictions, 17 required LLM classification (supplemental policy divisions like "FEND Off Fentanyl Act").

### Problem 2: No fiscal year scoping
The summary total ($6.4T across all 13 bills) mixes FY2024, FY2025, and FY2026. There's no way to ask "what's the FY2026 total?" The tool needs fiscal year metadata and `--fy` filtering.

**Validated:** Built FY2026 coverage map from XML metadata — H.R. 7148 covers Defense+Labor-HHS+THUD+FinServ+State, H.R. 6938 covers CJS+E&W+Interior, H.R. 5371 covers Ag+LegBranch+MilCon-VA. All 12 subcommittees are covered for FY2026.

### Problem 3: Case-sensitive account matching in compare
"Grants-In-Aid for Airports" and "Grants-in-Aid for Airports" are the same account but treated as different. This creates false orphans in the compare output.

**Validated:** In the THUD FY2024→FY2026 comparison, case normalization recovers the Grants-in-Aid match ($3.88B → $4.58B). Highway Infrastructure Programs has the same issue.

### Problem 4: Compare only matches new_budget_authority provisions
"Transit Formula Grants" exists in FY2024 as a $14B limitation and in FY2026 as a $14.6B appropriation. Because the FY2024 version has `semantics: "limitation"` instead of `new_budget_authority`, it's excluded from the compare and shows as "only in FY2026" — a false positive.

**Validated:** Found Transit Formula Grants in both XML files with nearly identical statutory language. The LLM classified it differently between the two bills.

### Problem 5: "Salaries and Expenses" ambiguity
455 provisions across 105 different agencies share the account name "Salaries and Expenses." Embedding similarity helps (the embedding text includes the agency field), but when the LLM fails to extract the agency correctly (as with OPM S&E in H.R. 7148), `--similar` can match the wrong agency's S&E account.

**Validated:** Confirmed OPM S&E in H.R. 7148 extracted with empty agency field. The XML heading says "Office of personnel management" but the LLM missed it.

### Problem 6: Advance appropriation confusion
30% of total budget authority across the major bills ($1.888 trillion) is advance appropriations — money enacted now but available in a future fiscal year. Without flagging these, a journalist comparing year-over-year numbers gets wrong results. The VA Comp & Pensions account has $182B that looks like FY2024 but is actually an advance appropriation for FY2025.

**Validated:** Tested heuristic detection ("shall become available on October 1") — caught most cases but also flagged Medicaid as advance when it's actually no-year current funding. LLM classification (Sonnet) got 5/5 correct including catching the Medicaid false positive. Embedding-based exemplar classification also got 5/5 correct using just 3 exemplars per class.

### Problem 7: H.R. 1968 classification
H.R. 1968 is classified as "continuing_resolution" but contains $1.786 trillion in full-year appropriations across 526 provisions for Defense, Homeland Security, Labor-HHS, and other subcommittees. It's a "full-year CR with appropriations" — a hybrid that the current classification system doesn't handle.

**Validated:** Provision type distribution shows 260 appropriations + 80 CR substitutions + 55 riders. The $840B Defense and $420B VA amounts are genuine full-year funding, not CR baseline references.

### Problem 8: Empty account names
46 provisions totaling $11.8B have no account name — they're lump-sum provisions in General Provisions sections that say "For an additional amount for the Department of Defense, $8,000,000,000." These can't be matched by account name but CAN be matched by embedding similarity.

**Validated:** The $8B DoD provision in H.R. 1968 matches well via embeddings to other Defense provisions but has no structured account name to match on.

### Problem 9: Cross-bill link ephemeral
`--similar` finds matches but doesn't persist them. The same 118K-comparison search runs every time. Links would let users save matches, annotate renames, and use them in compare.

**Validated:** Full cross-FY link suggest simulation found 93 candidates between 118th and 119th Congress bills, with 87 exact name matches, 6 name mismatches (5 genuine renames at 0.79-0.97 similarity, 1 false positive at 0.78). LLM verification correctly classified all 7 ambiguous pairs.

---

## Empirical Data for Threshold Calibration

These numbers come from computing pairwise cosine similarities across 2,364 (H.R. 4366) × 2,837 (H.R. 7148) = 6.7M provision pairs, sampled and analyzed.

### Similarity score distributions
**Random pair background (n=2000):** mean=0.397, stdev=0.089, max=0.695
**Known same-account pairs (n=450):** mean=0.783, stdev=0.121, min=0.540, median=0.794
**Same-name same-agency true matches (n=63):** min=0.787, median=0.995, max=1.000
**Different-name same-agency potential false positives (n=1353):** median=0.561, p95=0.664, max=0.876

### False positive / false negative rates by threshold
| Threshold | FPR (random→accept) | FNR (same→reject) | Recommendation |
|-----------|---------------------|-------------------|----------------|
| 0.55 | 3.90% | 0.6% | Uncertain zone floor |
| 0.60 | 1.00% | 4.9% | Acceptable |
| 0.65 | 0.30% | 19.8% | HIGH confidence — auto-accept with name match |
| 0.70 | 0.10% | 32.7% | HIGH confidence |
| 0.80 | 0.05% | 46.3% | Too conservative for auto-accept (misses half of genuine matches) |

### Key insight
The 0.80 threshold used in the original design misses 46% of genuine same-account matches. The 0.65 threshold captures 80% of genuine matches with only 0.3% false positive rate. But similarity alone should NOT determine confidence — the evidence TYPE matters more:

- **Name match** (case-insensitive, prefix-stripped) → VERIFIED regardless of similarity score
- **sim ≥ 0.65 AND same agency** → HIGH confidence
- **sim 0.55-0.65 OR name mismatch in 0.65+ zone** → UNCERTAIN — send to LLM
- **sim < 0.55** → AUTO-REJECT

### LLM verification experiments
- **4 ambiguous pairs** from FY2024→FY2026 comparison sent to Claude Sonnet: 4/4 correct (1 SAME, 3 DIFFERENT). Input: 538 tokens, output: 535 tokens.
- **7 ambiguous pairs** from the full cross-FY link suggest: 7/7 correct (1 SAME, 6 DIFFERENT). Input: 592 tokens, output: 649 tokens.
- **17 unknown division titles** from supplemental bills sent for jurisdiction classification: 17/17 correct. Input: 368 tokens, output: 661 tokens.
- **5 provisions** sent for advance/current-year classification: 5/5 correct (including catching a false positive from heuristic detection). Input: 502 tokens, output: 372 tokens.
- **Embedding-based exemplar classification** for advance/current: 5/5 correct using pre-computed exemplar vectors (3 per class). No API call at classification time — just dot products.

### Cross-bill matching at account level (FY2024 THUD → FY2026 THUD)
- 82 accounts in each FY
- 72 matched by exact normalized name
- Only 2 genuine orphans (both <$12M)
- Embedding-based matching found 83 of 85 provision-level pairs at >0.80 similarity

---

## v4.0 Implementation — What Was Actually Built

> **Note:** The sections below ("Phase 1", "Phase 2", "Phase 3") were the ORIGINAL design specifications written before implementation. They are preserved as historical context showing what was planned. For what was actually built (including deviations), see "Current State (v4.0.0)" above and the "Key deviations from the original v4.0 plan" section immediately below.

### Design Principles (revised from original plan)
1. **Keyword/regex for structural classification, NOT embedding exemplars.** Testing showed embedding exemplars achieved only 62% accuracy for advance/current classification because the signal (availability text) wasn't encoded in the embeddings. FY-aware keyword matching achieves 100%. XML parsing + regex achieves reliable jurisdiction classification. Embeddings are reserved for semantic tasks (cross-bill matching, relate).
2. **Evidence-based confidence, not arbitrary similarity thresholds.** Link confidence is determined by evidence type (name match, LLM verdict, statistical significance), not by raw cosine score. Thresholds (0.55/0.65) are empirically calibrated from 6.7M pairwise comparisons.
3. **Each pipeline layer only depends on layers below.** Links reference embeddings via hash; changing an extraction invalidates embeddings which invalidates links. Links never modify extractions.
4. **Backward compatible.** All existing commands work identically. New flags are additive. New files (`bill_meta.json`, `links/links.json`) are optional.
5. **Enumerations over strings.** Jurisdiction, bill nature, funding timing — use Rust enums with serde, not free-form strings that require pattern matching.
6. **Every classification records its provenance.** `ClassificationSource` enum tracks whether a decision came from XML structure, pattern matching, fiscal year comparison, note text, or a default rule.
7. **Enrich runs offline.** No API keys for the `enrich` command. All classification is deterministic.

### Key deviations from the original v4.0 plan
- **`exemplars.rs` was NOT built.** Embedding exemplars don't work for advance/current or jurisdiction classification. Keywords + XML parsing are more accurate and require no API calls.
- **`scripts/generate_exemplars.py` was NOT needed.** No pre-computed exemplar vectors.
- **Sub-agency normalization was added** (not in original plan). 35-entry lookup table resolves agency granularity mismatches.
- **Cross-semantics orphan rescue was added** (not in original plan). Rescues provisions with same account name but different semantics.
- **`--show-advance` was added** (not explicitly in original plan as a separate flag). Separates advance from current-year in summary.
- **Summary handler was consolidated** to call `query::summarize()` instead of reimplementing inline.
- **Advance classification uses `FundAvailability::Other(String)`** — the availability field is always a raw string from the LLM, never the structured `OneYear`/`MultiYear`/`NoYear` variants.

### Implemented pipeline
```
Stage 1: BILLS-*.xml              ← immutable source (download)
Stage 2: extraction.json          ← LLM output (extract)
         verification.json        ← deterministic checks
         metadata.json            ← provenance
Stage 2.5: bill_meta.json         ← NEW: fiscal year, subcommittees, advance classification (enrich)
Stage 3: embeddings.json          ← metadata (embed)
         vectors.bin              ← binary vectors
Stage 4: links/links.json         ← NEW: persistent cross-bill relationships (link suggest/accept)
Stage 5: Query                    ← enriched with bill_meta + links
         summary --fy             ← NEW: fiscal year scoping
         search --subcommittee    ← NEW: jurisdiction filtering
         compare --use-links      ← NEW: link-aware matching
         relate                   ← NEW: deep-dive with LLM analysis
```

### Extended hash chain
```
BILLS-*.xml ──sha256──▶ metadata.json (source_xml_sha256)
extraction.json ──sha256──▶ bill_meta.json (extraction_sha256)    ← NEW
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
embeddings.json (per bill) ──sha256──▶ links/links.json (bill_hashes)  ← NEW
```

Staleness levels for links:
- Embeddings changed but extraction didn't → "soft stale" — similarity scores may differ but provision indices are valid. Warn.
- Extraction changed → "hard stale" — provision indices may have shifted. Warn strongly, suggest re-running `link suggest`.

---

## v4.0 CLI Specification

### New commands

#### `enrich` — Generate bill metadata and classify provisions
```
congress-approp enrich [OPTIONS]
  --dir <DIR>       Data directory [default: ./examples]
  --dry-run         Preview without writing files
  --force           Re-enrich even if bill_meta.json exists
```

Generates `bill_meta.json` per bill directory containing:
- Congress number (parsed from XML filename)
- Fiscal years (from extraction)
- Subcommittee/jurisdiction mapping (division letter → Jurisdiction enum)
- Bill nature (enriched classification: omnibus, full_year_cr_with_appropriations, etc.)
- Provision timing (advance/current/supplemental for top-level BA provisions)
- Canonical account names (case-normalized, prefix-stripped)

Uses keyword matching + fiscal-year-aware date comparison for advance/current classification (no API call). Falls back to LLM for novel division titles that don't match patterns. (Original plan said "embedding exemplars" but testing showed 62% accuracy — keywords achieve 100%.)

#### `link suggest` — Compute link candidates from embeddings
```
congress-approp link suggest [OPTIONS]
  --dir <DIR>          Data directory
  --threshold <F>      Minimum similarity [default: 0.55]
  --scope <SCOPE>      intra (within-FY), cross (across-FY), all [default: all]
  --verify             Send ambiguous pairs to LLM for SAME/DIFFERENT classification
  --limit <N>          Max candidates [default: 100]
  --format <FORMAT>    table/json/jsonl/hashes
```

#### `link accept` — Persist link candidates
```
congress-approp link accept [OPTIONS] <HASHES...>
  --dir <DIR>          Data directory
  --note <TEXT>        Optional annotation (e.g., "Account renamed from X to Y")
  --auto               Accept all verified + high-confidence candidates
```

#### `link remove` — Remove accepted links
```
congress-approp link remove --dir <DIR> <HASHES...>
```

#### `link list` — Show accepted links
```
congress-approp link list [OPTIONS]
  --dir <DIR>          Data directory
  --format <FORMAT>    table/json/jsonl
  --bill <BILL>        Filter to links involving this bill
  --stale              Show only stale links
```

#### `relate` — Deep-dive on one provision across all bills
```
congress-approp relate <BILL_DIR:INDEX> [OPTIONS]
  --dir <DIR>          Data directory [default: ./examples]
  --top <N>            Max related provisions [default: 10]
  --format <FORMAT>    table/json
  --analyze            Include LLM budget analysis (requires ANTHROPIC_API_KEY)
  --fy-timeline        Show fiscal year timeline from linked provisions
```

### Enhanced existing commands

#### `summary` — new flags
```
  --fy <YEAR>          Filter to bills covering this fiscal year
  --congress <N>       Filter to bills from this congress
  --show-advance       Separate advance appropriations in output
```

#### `search` — new flags
```
  --fy <YEAR>          Filter to bills covering this fiscal year
  --congress <N>       Filter to bills from this congress
  --subcommittee <SLUG>  Filter by jurisdiction (defense, thud, cjs, etc.)
  --show-confidence    Show attribution_confidence column
```

#### `compare` — new flags and behavior
```
  --base-fy <YEAR>     Use all bills for this FY as base (alternative to --base)
  --current-fy <YEAR>  Use all bills for this FY as current (alternative to --current)
  --dir <DIR>          Required with --base-fy/--current-fy
  --subcommittee <SLUG>  Scope comparison to one jurisdiction
  --use-links          Use accepted links for matching across renames
```

Behavior changes:
- Case-insensitive account name matching (normalize to lowercase before comparison)
- Cross-provision-type matching (limitation→appropriation shows as "reclassified" not "only in current")
- With `--use-links`: linked accounts show as "linked (renamed)" in the Status column

#### `audit` — new flags
```
  --attribution        Show attribution confidence breakdown per bill
  --advance            Flag advance appropriation misclassifications
```

### Attribution confidence scoring (used by `--show-confidence` and `audit --attribution`)

Computed from existing verification data — no LLM needed. Two signals are combined:

**Amount uniqueness score** (from `verification.json` `amount_checks`):
- Dollar string found at exactly 1 position in source → 3 (unique)
- Found at 2-5 positions → 2 (few)
- Found at 6+ positions → 1 (many, e.g., "$5,000,000" appears 46 times)
- Not found → 0 (verification failure)
- No dollar amount on this provision → N/A

**Text match tier score** (from `verification.json` `raw_text_checks`):
- `exact` → 3 (byte-identical substring of source)
- `normalized` → 2 (matches after whitespace/quote normalization)
- `spaceless` → 1 (matches after removing all spaces)
- `no_match` → 0 (not found at any tier)

**Combined score** = amount_score + text_score (range 0-6):
- **5-6 → HIGH** — unique amount + exact/normalized text. Safe to cite without manual verification.
- **3-4 → MEDIUM** — ambiguous amount but exact text, OR unique amount but normalized text. Safe to cite; the exact text match provides attribution even when the dollar string is ambiguous.
- **1-2 → LOW** — ambiguous amount + poor text match. Verify manually against source XML before citing.
- **0 → UNVERIFIABLE** — dollar amount not found in source. Do not cite without manual verification.
- **N/A** — provision has no dollar amount (riders, directives). Attribution is not applicable.

**Empirical distribution (H.R. 7148, 2,837 provisions):**
- HIGH: 1,109 (39.1%)
- MEDIUM: 458 (16.1%)
- LOW: 4 (0.1%)
- UNVERIFIABLE: 0 (0.0%)
- N/A: 1,266 (44.6%)

### LLM prompt templates for v4.0 features

**Link verification prompt** (used by `link suggest --verify`):
```
System: You are an expert on U.S. federal appropriations. For each pair of
provisions from different bills, determine if they are the SAME program
(renamed/reorganized) or DIFFERENT programs.
Respond in JSON: [{"id": N, "verdict": "SAME"/"DIFFERENT", "confidence": 0.0-1.0, "reasoning": "brief"}]

User: Classify these {N} provision pairs from FY2024 vs FY2026:
1. sim=0.75 | A: "{src_acct}" ({src_agency}) ${src_dollars} | B: "{tgt_acct}" ({tgt_agency}) ${tgt_dollars} [{tgt_bill}]
...
```

**Division jurisdiction classification prompt** (used by `enrich` for novel titles):
```
System: Classify each division title from a U.S. appropriations bill into one of:
defense, labor-hhs, thud, financial-services, cjs, energy-water, interior,
agriculture, legislative-branch, milcon-va, state-foreign-ops, homeland-security,
continuing-resolution, extenders, policy, budget-process, other.
Respond as JSON: [{"title": "...", "category": "...", "brief_reason": "..."}]

User: Classify these division titles:
1. Fend Off Fentanyl Act
2. Health Care Extenders
...
```

**Budget analysis prompt** (used by `relate --analyze`):
```
System: You are a federal budget analyst. Given a source provision and its
matched counterparts, produce a JSON analysis:
{"timeline": [{"fy": N, "current_year_ba": N, "advance_ba": N, "supplemental": N, "source_bills": ["..."]}],
 "trend": "one sentence on year-over-year change using COMPARABLE numbers",
 "caveats": ["..."],
 "suggested_language": "one sentence a journalist could publish"}

User: Source: {bill} — {account} — ${dollars} ({classification})
Matched provisions:
  {bill} FY{fy}: {account} — ${dollars}
    Availability: "{availability_text}"
    Text: "{raw_text[:120]}"
...
```

### Scope filtering for `link suggest`

The `--scope` flag controls which bill pairs are compared:

**`--scope intra`:** Only compare bills within the same fiscal year. This matches CR provisions to their omnibus counterparts, supplemental provisions to their regular appropriation counterparts, etc. Uses `bill_meta.fiscal_years` to determine which bills share a fiscal year.

**`--scope cross`:** Only compare bills across different fiscal years. This is for year-over-year tracking — finding the FY2026 version of an FY2024 program. Bills that share a fiscal year are excluded.

**`--scope all` (default):** Compare every bill pair regardless of fiscal year. This produces the most candidates but may include noise from comparing CRs to unrelated regular bills.

**Performance note:** For 13 bills with ~8,500 provisions, `--scope all` computes ~118K pairwise similarities and takes ~12 seconds on a modern laptop. For 50+ bills, consider using `--scope cross` or `--scope intra` to reduce the comparison space, or increase `--threshold` to reduce candidates.

---

## v4.0 Implementation Details

### Phase 1: Foundation — Bill Metadata and Account Normalization [SHIPPED in v4.0.0]

#### New file: `src/approp/bill_meta.rs` (~300 lines)

Types:
```rust
pub struct BillMeta {
    pub schema_version: String,
    pub congress: Option<u32>,
    pub fiscal_years: Vec<u32>,
    pub bill_nature: BillNature,
    pub subcommittees: Vec<SubcommitteeMapping>,
    pub provision_timing: Vec<ProvisionTiming>,
    pub canonical_accounts: Vec<CanonicalAccount>,
}

pub enum BillNature {
    Regular,
    Omnibus,
    Minibus,
    ContinuingResolution,
    FullYearCrWithAppropriations,
    Supplemental,
    Authorization,
    Other(String),
}

pub struct SubcommitteeMapping {
    pub division: String,
    pub jurisdiction: Jurisdiction,
    pub title: String,
}

pub enum Jurisdiction {
    Defense, LaborHhs, Thud, FinancialServices,
    Cjs, EnergyWater, Interior, Agriculture,
    LegislativeBranch, MilconVa, StateForeignOps,
    HomelandSecurity, ContinuingResolution,
    Extenders, Policy, BudgetProcess, Other,
}

pub struct ProvisionTiming {
    pub provision_index: usize,
    pub timing: FundingTiming,
    pub available_fy: Option<u32>,
}

pub enum FundingTiming {
    CurrentYear,
    Advance,
    Supplemental,
}

pub struct CanonicalAccount {
    pub provision_index: usize,
    pub canonical_name: String,  // lowercase, prefix-stripped
}
```

Functions:
- `parse_congress_from_xml(path) -> Option<u32>`
- `parse_subcommittees_from_xml(path) -> Vec<SubcommitteeMapping>` — uses XML parsing + regex pattern matching for jurisdiction classification (NOT embedding exemplars — see deviations above)
- `classify_bill_nature(extraction, subcommittees) -> BillNature`
- `classify_provision_timing(provision, bill_fiscal_years) -> FundingTiming` — FY-aware keyword matching on availability text, no API call (NOT embedding dot products — see deviations above)
- `normalize_account_name(name) -> String` — lowercase, strip whitespace, strip em-dash prefixes
- `load_bill_meta(dir) -> Option<BillMeta>`
- `save_bill_meta(dir, meta)`

#### ~~New file: `src/approp/exemplars.rs` (~100 lines)~~ — NOT BUILT

This file was in the original plan but was NOT built. Testing showed embedding exemplars achieved only 62% accuracy for advance/current classification (the signal isn't encoded in the embeddings because `build_embedding_text()` doesn't include the `availability` field). Keyword matching with fiscal-year-aware date comparison achieves 100% accuracy on 1,748 provisions. See "Key deviations from the original v4.0 plan" above.

#### Changes to existing files

`src/approp/mod.rs` — add `pub mod bill_meta; pub mod exemplars;`

`src/approp/loading.rs` — add `bill_meta: Option<BillMeta>` to `LoadedBill` struct, load from `bill_meta.json` alongside other files.

`src/approp/query.rs`:
- New: `filter_bills_by_fy(bills, fy) -> Vec<&LoadedBill>`
- New: `resolve_subcommittee(bills, jurisdiction) -> Vec<(bill_idx, division)>`
- `SearchFilter` gets `fy: Option<u32>`, `congress: Option<u32>`, `subcommittee: Option<&str>`
- `build_account_map()` applies `normalize_account_name()` (case-insensitive)
- `build_account_map()` includes all provision types, not just Appropriation+NewBudgetAuthority
- New `CompareRow` fields: `base_semantics`, `current_semantics`
- New status: `"reclassified"` when same account has different semantics

`src/main.rs`:
- New handler: `handle_enrich()` (~150 lines)
- `summary` handler: filter by FY/congress, show advance split
- `search` handler: filter by FY/congress/subcommittee, show attribution confidence
- `compare` handler: `--base-fy`/`--current-fy`/`--subcommittee`/`--use-links` flags

`src/lib.rs` — re-export `bill_meta` types.

### Phase 2: Links [SHIPPED in v4.0.0]

#### New file: `src/approp/links.rs` (~500 lines)

Types:
```rust
pub struct LinksFile {
    pub schema_version: String,
    pub embedding_model: String,
    pub embedding_dimensions: usize,
    pub bill_hashes: HashMap<String, BillHashes>,
    pub accepted: Vec<AcceptedLink>,
}

pub struct BillHashes {
    pub extraction_sha256: String,
    pub embeddings_sha256: String,
}

pub struct AcceptedLink {
    pub hash: String,                    // 8-char deterministic
    pub source: ProvisionRef,
    pub target: ProvisionRef,
    pub similarity: f32,
    pub relationship: LinkRelationship,
    pub evidence: LinkEvidence,
    pub accepted_at: String,
    pub note: Option<String>,
}

pub struct ProvisionRef {
    pub bill_dir: String,
    pub provision_index: usize,
}

pub enum LinkRelationship {
    SameAccount,
    Renamed,
    Reclassified,
    Related,
}

pub enum LinkEvidence {
    NameMatch,
    LlmVerified { confidence: f32, reasoning: String },
    HighSimilarity,
    Manual,
}

pub struct LinkCandidate {
    pub hash: String,
    pub source: ProvisionRef,
    pub target: ProvisionRef,
    pub similarity: f32,
    pub confidence: LinkConfidence,
    pub llm_verdict: Option<LlmVerdict>,
    pub already_accepted: bool,
    pub source_label: String,
    pub target_label: String,
}

pub enum LinkConfidence {
    Verified,       // name match or LLM SAME
    High,           // sim >= 0.65 AND same agency
    Uncertain,      // 0.55-0.65, or name mismatch in 0.65+ zone
    Rejected,       // LLM DIFFERENT
}
```

Functions:
- `compute_link_hash(source, target, model) -> String` — sha256 of `"{src}:{idx}→{tgt}:{idx}:{model}"`, take first 8 hex chars
- `suggest(bills, embeddings, threshold, scope, existing_links) -> Vec<LinkCandidate>` — the core matching engine using calibrated thresholds
- `verify_candidates(candidates, anthropic_client) -> Vec<LinkCandidate>` — batch LLM verification of uncertain pairs
- `load_links(dir) -> Result<Option<LinksFile>>`
- `save_links(dir, links) -> Result<()>` — atomic write (temp + rename)
- `check_link_staleness(links, bills) -> Vec<StaleLinkWarning>`

Link suggest algorithm:
1. Load all bills and embeddings
2. For each top-level BA provision P in bill A, for each other bill B:
   a. Find top-1 match in B above threshold (using `embeddings::top_n_similar`)
   b. Check if normalized account names match → Verified
   c. Check if sim >= 0.65 AND same agency → High
   d. Otherwise → Uncertain
3. Deduplicate: if A→B and B→A both suggest links, keep higher-sim direction
4. Sort by similarity descending, truncate to limit
5. If `--verify`: send Uncertain candidates to LLM in one batch call

Links file location: `<dir>/links/links.json` (at the data root, not inside any bill directory).

#### Changes to main.rs
- New handlers: `handle_link_suggest()`, `handle_link_accept()`, `handle_link_remove()`, `handle_link_list()` (~400 lines total)
- `Commands` enum: new `Link { action: LinkCommands }` variant with subcommands
- `handle_compare()`: when `--use-links`, load links and use them for matching

#### Changes to loading.rs
- Load `links/links.json` from the data root (separate from per-bill loading)

#### Changes to staleness.rs
- New `StaleLinkWarning` variant
- `check()` extended to check link hashes

### Phase 3: Relate Command [SHIPPED in v4.0.0]

#### New handler: `handle_relate()` in main.rs (~300 lines)

Steps:
1. Parse `bill_dir:index` reference
2. Load all bills + embeddings + bill_meta + links
3. Compute cosine similarity against all provisions
4. Tier results: same_account (>0.65 + name match), related (0.55-0.65), accepted links
5. If `--fy-timeline`: group same-account by FY using bill_meta, separate advance/current
6. If `--analyze`: send matches to Claude for structured budget analysis

#### New library function in query.rs (~150 lines)

```rust
pub struct RelateReport<'a> {
    pub source: ProvisionSummary,
    pub same_account: Vec<(f32, ProvisionSummary)>,
    pub related: Vec<(f32, ProvisionSummary)>,
    pub accepted_links: Vec<AcceptedLink>,
    pub fy_timeline: Option<Vec<FyTimelineEntry>>,
}

pub struct FyTimelineEntry {
    pub fy: u32,
    pub current_year_ba: i64,
    pub advance_ba: i64,
    pub supplemental_ba: i64,
    pub source_bills: Vec<String>,
}

pub fn relate(bills, embeddings, source_ref, top_n, links, bill_metas) -> RelateReport
```

The LLM analysis for `--analyze` is done in the CLI handler, not the library function. The library produces the structured data; the CLI optionally enriches it with LLM narrative. This keeps the library pure (no API calls).

### Phase 4: Extraction Resume [NOT YET BUILT]

#### Changes to extraction.rs (~80 lines)

Checkpoint file: `.extraction_progress.json` (gitignored, transient)

```json
{
  "total_chunks": 92,
  "completed_chunks": ["A-I", "A-II"],
  "failed_chunks": ["F-VIIg", "F-VIIh"],
  "chunk_artifacts": {"A-I": "01KKWW9T.json", "A-II": "01KKWWA2.json"},
  "model": "claude-opus-4-6",
  "started_at": "2026-03-18T..."
}
```

Changes to `extract_bill_parallel()`:
1. Before starting: check for `.extraction_progress.json`
2. If exists: load completed chunk labels, load their provisions from chunk artifacts in `chunks/`
3. Only send un-completed chunks to the API
4. After each chunk succeeds: update progress file
5. When all chunks complete: delete progress file, write extraction.json
6. When chunks fail and `!continue_on_error`: update progress with failed list, abort (progress is saved for next run)

---

## v4.0 Documentation Plan

### New book chapters (4 chapters, ~2,000 lines)
| Chapter | Location | Content |
|---------|----------|---------|
| Enrich Your Data | `how-to/enrich-data.md` | Running `enrich`, what `bill_meta.json` contains, FY scoping, subcommittee mapping |
| Working with Links | `how-to/working-with-links.md` | Full link workflow: suggest → verify → accept → use in compare |
| Cross-Year Comparison | `tutorials/cross-year-compare.md` | Tutorial: compare THUD FY2024→FY2026, interpret results, handle renames |
| The Relate Command | `tutorials/relate-provision.md` | Tutorial: deep-dive on VA Comp & Pensions across 3 fiscal years |

### Updated chapters (~10 chapters, ~500 lines of edits)
| Chapter | Changes |
|---------|---------|
| `reference/cli.md` | Add `enrich`, `link`, `relate` commands and new flags on existing commands |
| `explanation/pipeline.md` | Add Stage 2.5 (enrich) and Stage 4 (links) to pipeline diagram |
| `explanation/provision-types.md` | Add `attribution_confidence` and `funding_timing` fields |
| `reference/extraction-json.md` | Document `bill_meta.json` format |
| `reference/data-directory.md` | Add `bill_meta.json` and `links/links.json` |
| `reference/glossary.md` | Add: jurisdiction, link, funding timing, advance appropriation, bill nature, enrich |
| `explanation/hash-chain.md` | Add links layer to hash chain diagram |
| `explanation/semantic-search.md` | Add section on exemplar-based classification |
| `appendix/changelog.md` | Add v4.0 entry |
| `appendix/example-bills.md` | Update for 13 bills, add FY coverage map |
| `index.md` | Update version to v4.0.x |

### New SUMMARY.md entries
```markdown
# Tutorials
- [Cross-Year Comparison](./tutorials/cross-year-compare.md)
- [The Relate Command](./tutorials/relate-provision.md)

# How-To Guides
- [Enrich Your Data](./how-to/enrich-data.md)
- [Working with Links](./how-to/working-with-links.md)
```

---

## Execution Order and Dependencies

```
Phase 1a: bill_meta.rs + exemplars.rs          [no dependencies]
Phase 1b: enrich command in main.rs            [depends on 1a]
Phase 1c: summary/search FY + subcommittee     [depends on 1a loading changes]
Phase 1d: compare enhancements                 [depends on 1a + 1c]
  ── v4.0-alpha: test, commit, push, crates.io, docs ──

Phase 2a: links.rs                             [depends on 1a for normalization]
Phase 2b: link CLI commands                    [depends on 2a]
Phase 2c: compare --use-links                  [depends on 2a + 1d]
  ── v4.0-beta: test, commit, push, crates.io, docs ──

Phase 3: relate command                        [depends on 2a + 1a]
  ── v4.0-rc: test, commit, push, crates.io, docs ──

Phase 4: extraction resume                     [no dependencies, can parallel]
Phase 5: documentation completion              [after each phase]
  ── v4.0 final: test, commit, push, crates.io, docs ──
```

### Estimated effort per phase
| Phase | New Lines | Changed Lines | New Tests | Notes |
|-------|-----------|---------------|-----------|-------|
| 1a. bill_meta + exemplars | ~400 | ~30 | 8 unit | Need to generate exemplar vectors |
| 1b. enrich command | ~150 | ~20 | 2 integration | LLM calls for novel division titles |
| 1c. summary/search filtering | ~80 | ~60 | 4 integration | |
| 1d. compare enhancements | ~120 | ~80 | 4 integration | |
| 2a. links.rs | ~500 | ~20 | 10 unit | Core matching engine |
| 2b. link CLI commands | ~400 | ~30 | 6 integration | |
| 2c. compare --use-links | ~60 | ~40 | 2 integration | |
| 3. relate command | ~450 | ~10 | 4 integration | LLM analysis handler |
| 4. extraction resume | ~80 | ~60 | 2 unit | Checkpoint file management |
| 5. documentation | ~2,500 | ~500 | — | 4 new chapters + 10 updated |
| **Total** | **~4,740** | **~850** | **~42** | |

### New dependencies
None required. All functionality uses existing crates (serde, sha2, reqwest, tokio, etc.).

### Breaking changes
None. All existing commands work identically. New flags are additive. New files are optional.

---

## Simulated Workflow Outputs

These are real outputs from Python simulations against the actual 13-bill dataset. They show what the v4.0 commands would produce.

### Workflow A: "What's the FY2026 THUD budget?"

```
$ congress-approp summary --dir examples --fy 2026 --subcommittee thud

Bill: H.R. 7148, Division D (Transportation, Housing and Urban Development)
  Budget Authority: $XXX,XXX,XXX,XXX
  Rescissions: $X,XXX,XXX,XXX
  Provisions: 618
```

### Workflow B: "Compare THUD FY2024 → FY2026"

```
$ congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir examples

FY2024 THUD: H.R. 4366 Division F (82 accounts)
FY2026 THUD: H.R. 7148 Division D (82 accounts)

  Changed: 43  Reclassified: 1  Only in FY2024: 2  Only in FY2026: 1  Unchanged: 12

  Account                                     FY2024          FY2026           Delta     Status
  Tenant-Based Rental Assistance    $28,386,831,000 $34,438,557,000 $+6,051,726,000  changed
  Transit Formula Grants            $13,990,000,000 $14,642,000,000 $  +652,000,000  reclassified
  ...
```

### Workflow C: "Trace VA Comp & Pensions across all years"

```
$ congress-approp relate hr9468:0 --dir examples --analyze --fy-timeline

Provision: H.R. 9468 [0] — Compensation and Pensions ($2,285,513,000)

Same Account:
  0.86  H.R. 4366    FY[2024]  appropriation  $182,310,515,000  [verified (name)]
  0.85  H.R. 5371    FY[2026]  appropriation  $246,630,525,000  [verified (name)]
  0.85  H.R. 1968    FY[2025]  appropriation   $30,242,064,000  [high]
  0.83  H.R. 1968    FY[2025]  appropriation  $227,240,071,000  [high]
  ...

Timeline:
  FY2024: $199,668,416,000 (current=$17,358M advance=$182,311M supp=$2,286M)
  FY2025: $257,482,135,000 (current=$30,242M advance=$227,240M)
  FY2026: $252,480,525,000 (current=$5,850M advance=$246,631M)

Trend: Total funding increases 28.9% from FY2024 to FY2025, then decreases 1.9% to FY2026.

Caveats:
  ⚠ FY2024 includes $2.3B supplemental for VA funding shortfall
  ⚠ Large advance appropriations provide budget certainty but limit annual oversight
  ⚠ These are mandatory entitlement estimates, not discretionary spending caps
```

### Workflow D: "Find and save cross-year links"

```
$ congress-approp link suggest --dir examples --scope cross --verify --limit 20

  THUD cross-FY link candidates:
    Verified (name match): 73
    High (sim >= 0.65, no name): 1
    Uncertain → LLM verified: 0 SAME, 1 DIFFERENT

$ congress-approp link accept --dir examples --auto
  Auto-accepted: 73 links (73 name-match + 0 LLM-verified)

$ congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --use-links --dir examples
  Matched: 73  Renamed: 0  Only FY2024: 2  Only FY2026: 1
```

---

## Data Directory Layout (v4.0)

```
examples/                           ← --dir path
├── hr4366/                         ← bill directory
│   ├── BILLS-118hr4366enr.xml      ← source XML from Congress.gov
│   ├── extraction.json             ← structured provisions (REQUIRED)
│   ├── verification.json           ← deterministic verification
│   ├── metadata.json               ← extraction provenance
│   ├── tokens.json                 ← LLM token usage
│   ├── bill_meta.json              ← NEW: FY, subcommittees, advance classification
│   ├── embeddings.json             ← embedding metadata
│   ├── vectors.bin                 ← raw float32 vectors
│   └── chunks/                     ← per-chunk LLM artifacts (gitignored)
├── hr5860/
│   └── ...
├── hr7148/
│   └── ...
└── links/                          ← NEW: cross-bill relationships
    └── links.json                  ← append-only via link accept
```

### Hash chain (v4.0)
```
BILLS-*.xml ──sha256──▶ metadata.json (source_xml_sha256)
extraction.json ──sha256──▶ bill_meta.json (extraction_sha256)
extraction.json ──sha256──▶ embeddings.json (extraction_sha256)
vectors.bin ──sha256──▶ embeddings.json (vectors_sha256)
embeddings.json ──sha256──▶ links/links.json (bill_hashes per bill)
```

### Immutability model
Every file except links.json is write-once. Links.json is append-only (accept adds, remove deletes). Write:read ratio is ~1:500.

---

## Technical Decisions and Rationale

### Why embedding exemplars instead of regex
Bill text language changes between congresses. "Shall become available on October 1, 2024" might become "funds made available beginning October 1, 2025" in a future bill. Regex would break; embedding similarity captures the meaning regardless of exact wording. Pre-computed exemplar vectors require zero API calls at classification time — just dot products.

### Why enums for jurisdiction instead of strings
Free-form strings require pattern matching everywhere they're used. Enums with serde ensure:
- Typos are caught at compile time
- Exhaustive matching is enforced
- New jurisdictions require explicit code changes (not silent misclassification)
- JSON serialization uses snake_case slugs automatically

### Why LLM verification is optional (--verify flag)
Most users don't need it. The name-match and high-similarity tiers handle 95%+ of cases. LLM verification is for the <5% ambiguous zone where a human or LLM must decide. Making it opt-in keeps the default workflow fast and requires no additional API keys.

### Why relate --analyze is a CLI feature, not a library function
The library returns structured data (RelateReport). The CLI handler optionally enriches it with LLM narrative. This keeps the library pure — no API calls, no side effects, no formatting. The LLM analysis is a presentation layer concern.

### Why links live at the data root, not in bill directories
Links are between bills, not properties of a single bill. A link from hr4366:42 to hr7148:1369 doesn't belong in either bill's directory. The `links/` directory at the data root is the natural home.

### Why NOT to use LLM for dollar verification
Dollar amount verification is deterministic string matching — the exact `text_as_written` string is searched in the source XML. This is correct by construction. Adding LLM to this step would introduce non-determinism and potential hallucination into the one part of the pipeline that is currently 100% reliable.

### Why NOT to auto-accept links without human review (except --auto)
The link system is designed for trust. A journalist using `compare --use-links` needs to know that every link was either name-matched, LLM-verified, or human-reviewed. The `--auto` flag is the "I trust the system" shortcut; the default requires explicit acceptance.

---

## Files to Know (updated for v4.0)

| File | Lines | Purpose | When to Edit |
|------|-------|---------|-------------|
| `src/main.rs` | ~4,200 | CLI handlers, clap definitions, output formatting | Adding new commands or flags |
| `src/approp/query.rs` | ~840 | Library API: search, compare, summarize, audit, rollup, build_embedding_text | Adding new query functions |
| `src/approp/ontology.rs` | ~960 | All types: Provision (11 variants), BillExtraction, DollarAmount, AmountSemantics | Adding new fields |
| `src/approp/embeddings.rs` | ~260 | Embedding load/save, cosine_similarity, top_n_similar | Similarity functions |
| `src/approp/staleness.rs` | ~100 | Hash chain checking | Adding staleness checks for links |
| `src/approp/loading.rs` | ~340 | Directory walking, LoadedBill assembly (incl bill_meta) | Adding new artifact loading |
| `src/approp/extraction.rs` | ~840 | ExtractionPipeline, parallel chunk processing | Extraction resume |
| `src/approp/verification.rs` | ~370 | Deterministic verification | Attribution confidence |
| `src/approp/from_value.rs` | ~690 | Resilient JSON→Provision parsing | New provision variants |
| `src/approp/xml.rs` | ~590 | Congressional bill XML parsing | Division title parsing |
| `src/approp/prompts.rs` | ~310 | System prompt for Claude | Prompt improvements |
| `tests/cli_tests.rs` | ~1,200 | 42 integration tests against examples/ | New command tests |

**Files added in v4.0 (now shipped):**
| File | Lines | Purpose |
|------|-------|---------|
| `src/approp/bill_meta.rs` | ~1,280 | Bill metadata types, XML parsing, jurisdiction classification, FY-aware advance detection, account normalization. 33 unit tests. |
| `src/approp/links.rs` | ~790 | Cross-bill link types, suggest algorithm, accept/remove, load/save. 10 unit tests. |

**NOT built (original plan items that were rejected after testing):**
| File | Reason Not Built |
|------|-----------------|
| `src/approp/exemplars.rs` | Embedding exemplars achieved only 62% accuracy for advance/current classification. Keyword + FY-aware approach achieves 100%. |
| `scripts/generate_exemplars.py` | Not needed since exemplars.rs was not built. |

### Key patterns to follow when adding code
1. **Library function first, CLI second.** New logic goes in `query.rs` (or new module). CLI handler calls library and formats output.
2. **All query functions take `&[LoadedBill]` and return structs.** No I/O, no formatting, no side effects.
3. **Serde for everything.** All data types derive `Serialize`/`Deserialize`. Output structs derive `Serialize`.
4. **Tests in the same file.** Unit tests go in `#[cfg(test)] mod tests { }` at the bottom.
5. **Clippy clean with `-D warnings`.** Fix at root cause, not with `#[allow]`.
6. **Format with `cargo fmt`** before committing.

### All CLI commands (v4.0)
```
congress-approp download   --congress N --type hr --number N --output-dir DIR [--enacted-only] [--all-versions] [--dry-run]
congress-approp extract    --dir DIR [--parallel N] [--model MODEL] [--force] [--continue-on-error] [--dry-run]
congress-approp enrich     --dir DIR [--dry-run] [--force]
congress-approp embed      --dir DIR [--model M] [--dimensions D] [--batch-size N] [--dry-run]
congress-approp search     --dir DIR [-t TYPE] [-a AGENCY] [--account A] [-k KW] [--bill B] [--division D] [--min-dollars N] [--max-dollars N] [--semantic Q] [--similar S] [--top N] [--fy Y] [--subcommittee S] [--format F] [--list-types]
congress-approp summary    --dir DIR [--format F] [--by-agency] [--fy Y] [--subcommittee S] [--show-advance]
congress-approp compare    --base DIR --current DIR [-a AGENCY] [--format F] [--subcommittee S] [--use-links]
congress-approp compare    --base-fy Y --current-fy Y --dir DIR [-a AGENCY] [--format F] [--subcommittee S] [--use-links]
congress-approp relate     SOURCE --dir DIR [--top N] [--format F] [--fy-timeline]
congress-approp link suggest --dir DIR [--threshold F] [--scope S] [--limit N] [--format F]
congress-approp link accept  --dir DIR [HASHES...] [--note TEXT] [--auto]
congress-approp link remove  --dir DIR HASHES...
congress-approp link list    --dir DIR [--format F] [--bill B]
congress-approp audit      --dir DIR [--verbose]
congress-approp upgrade    --dir DIR [--dry-run]
congress-approp api test
congress-approp api bill list --congress N [--type T] [--offset N] [--limit N]
congress-approp api bill get --congress N --type T --number N
congress-approp api bill text --congress N --type T --number N
```

---

## Use Cases Validated by Real Data

### Use Case 1: FY-scoped totals
**Query:** "What's the total FY2026 discretionary budget?"
**Status:** Simulated. Bill metadata enables correct FY filtering. FY2026 is fully covered by H.R. 7148 + H.R. 6938 + H.R. 5371.

### Use Case 2: Cross-FY subcommittee compare
**Query:** "How did THUD funding change from FY2024 to FY2026?"
**Status:** Simulated. 43 accounts matched, only 2 genuine orphans. Case normalization and cross-type matching recover all meaningful comparisons.

### Use Case 3: CR anomaly → final funding
**Query:** "The CR cut NSF Research. What was the final level?"
**Status:** Validated with `--similar`. CR sub at 0.82 matches omnibus appropriation. All 13 CR subs traced to counterparts.

### Use Case 4: Program timeline across fiscal years
**Query:** "Show me VA Comp & Pensions across all years."
**Status:** Simulated with `relate`. Timeline correctly assembled: FY2024 ($199.7B) → FY2025 ($257.5B) → FY2026 ($252.5B) with advance/current/supplemental split. LLM analysis correctly identified advance vs current-year.

### Use Case 5: Renamed account detection
**Query:** "Were any programs renamed between FY2024 and FY2026?"
**Status:** Simulated. Found 6 name mismatches at >0.75 similarity. LLM correctly classified 5 as genuine renames and 1 as a false positive.

### Use Case 6: Cross-bill FEMA tracking
**Query:** "Show me everything Congress did for FEMA."
**Status:** Validated with `--semantic`. Found FEMA provisions across 7 bills ranked by relevance.

### Use Case 7: Publication-ready analysis
**Query:** "Give me a publishable number for Section 8 housing."
**Status:** Simulated. Attribution confidence scoring identifies which provisions are safe to cite (HIGH = unique amount + exact text = 39% of provisions, MEDIUM = ambiguous amount + exact text = 16%, LOW = 0.1%).

### Use Case 8: Advance appropriation separation
**Query:** "How much of the FY2026 VA budget is advance for FY2027?"
**Status:** Simulated. Embedding-based exemplar classification (3 exemplars per class, no API call at classification time) correctly identified advance vs. current-year for 5/5 test provisions. LLM verification also got 5/5, including catching a false positive from heuristic detection. The `enrich` command stores this classification in `bill_meta.json` for use by `summary --show-advance` and `relate --fy-timeline`.

### Use Case 9: Cross-bill search by subcommittee
**Query:** "Show me all Defense appropriations across all FY2026 bills."
**Status:** Requires `bill_meta.json` to resolve "defense" → H.R. 7148 Division A. The `--subcommittee defense --fy 2026` flags would filter to the correct bill and division automatically, producing only Defense provisions regardless of which division letter each bill uses.

---

## Gotchas and Things That Tripped Us Up

1. **`handle_search` is async** because the `--semantic` path calls OpenAI. The non-semantic path doesn't need async but lives inside the same async function. Don't add `block_on()` inside it.

2. **`main.rs` has two search functions**: `handle_search()` (the dispatcher) and `handle_semantic_search()`. The semantic path returns early from handle_search.

3. **Provision methods return `&str`, not `Option<&str>` in some cases.** `provision.account_name()` returns `""` for provisions without accounts, not `None`. Check with `.is_empty()`.

4. **The `from_value.rs` module exists because Claude doesn't always produce perfect JSON.** If you add a new provision variant, you must also add handling in `from_value.rs`.

5. **crates.io has a 10MB upload limit.** The `vectors.bin` files are excluded via `Cargo.toml` `exclude` field.

6. **The `summary` table no longer shows Coverage.** Removed in v2.1.0. Coverage lives in `audit` only.

7. **Embedding dimensions must be consistent across all bills.** 3072 for text-embedding-3-large. You can't compare vectors of different dimensions.

8. **Division letters are bill-internal.** Division A means Defense in H.R. 7148 but CJS in H.R. 6938. Never use division letters for cross-bill filtering — use subcommittee/jurisdiction metadata.

9. **30% of budget authority is advance appropriations.** The $182B VA Comp & Pensions in H.R. 4366 is for FY2025, not FY2024. The `relate` command must separate advance from current-year to produce correct timelines.

10. **H.R. 1968 is classified as continuing_resolution but has $1.786T in full-year appropriations.** It's a hybrid — "full-year CR with appropriations." The bill_nature enum handles this.

11. **455 provisions named "Salaries and Expenses" across 105 agencies.** Account name matching without agency context is unreliable for this account. Embeddings help because they include the raw text which mentions the specific agency.

12. **Extraction aborts on chunk failure by default (v3.2.0).** Use `--continue-on-error` to save partial results. Use `--force` to re-extract already-extracted bills.

13. **Download defaults to enrolled-only XML (v3.1.0).** Use `--all-versions` for intermediate versions. Non-enrolled versions may have different XML structures that crash the parser.

14. **Integration tests expect at least 3 bills in examples/ but allow more.** The budget authority regression guard checks the original 3 bills' exact totals. New bills don't need to be added to the test expectations.

15. **The FY2024 second omnibus (H.R. 2882) is missing.** 15 chunks in Division F-VII and Division G consistently fail with empty API responses. This may be a content-specific issue rather than rate limiting — the same chunks fail across multiple attempts with cooldown periods.

---

## Crates to Evaluate for v4.0

These are well-maintained, popular Rust crates that could help with v4.0 features. Evaluate before adding — only add if they save significant effort over manual implementation.

| Crate | Stars | Purpose | Where It Helps |
|-------|-------|---------|---------------|
| `ordered-float` | 1.2K | `OrderedFloat<f32>` for sortable similarity scores | Link candidates sorted by similarity; avoids `partial_cmp` boilerplate on f32 |
| `strsim` | 1.0K | String similarity (Levenshtein, Jaro-Winkler, etc.) | Account name fuzzy matching as a secondary signal alongside embeddings |
| `rust-embed` | 1.8K | Embed files into the binary at compile time | Could embed exemplar vectors directly into the binary instead of const arrays |
| `indicatif` | 4.2K | Progress bars and spinners | Better progress display for `link suggest` and `enrich` operations (current progress.rs is custom) |
| `tempfile` | (already a dev-dep) | Atomic file writes via temp+rename | Already used in tests; could use for atomic `links.json` writes |
| `directories` | 2.4K | Platform-specific user data directories | Future: default data location instead of always requiring `--dir` |

**Recommendation:** `ordered-float` is the strongest candidate — it eliminates all the `partial_cmp` and `unwrap_or` noise when sorting by f32 similarity scores, which v4.0 does extensively in `links.rs` and `relate`. The rest are nice-to-have but not essential.

**Do NOT add:**
- `sqlx` / `rusqlite` — the JSON-on-disk model is the right abstraction for this read-dominated workload. Adding a database would be over-engineering.
- `ndarray` / `nalgebra` — the cosine similarity implementation is 3 lines of code. A linear algebra crate adds compilation time for no benefit.
- `tera` / `handlebars` — template engines for the LLM prompt. The current const string approach is simpler and more auditable.

---

## Demos, Recipes, and Visualization Ideas

This section catalogs concrete demos, data recipes, and visualization concepts that showcase the tool's capabilities. Each is described with enough detail to implement. They are organized by audience.

### Recipes for Journalists

**Recipe 1: "How much did Congress spend on border security since DHS was created?"**

Trace all border-related accounts across fiscal years and sum:

```bash
# Find all CBP, ICE, and Border Patrol accounts
congress-approp authority list --dir data --agency 070 --format json | \
  python3 -c "
import sys, json
authorities = json.load(sys.stdin)
border_keywords = ['customs and border', 'immigration and customs', 'border patrol']
for a in authorities:
    if any(kw in a['fas_title'].lower() for kw in border_keywords):
        print(f'{a[\"fas_code\"]}  {a[\"fas_title\"][:60]}')
"

# Trace each account
congress-approp trace 070-0530 --dir data --format json  # CBP Ops
congress-approp trace 070-0540 --dir data --format json  # ICE Ops
congress-approp trace 070-0532 --dir data --format json  # CBP Procurement
```

Aggregate the JSON timelines, adjust for inflation with CPI data, and produce a single "border security spending FY2019-FY2026" chart.

**Recipe 2: "Which programs got real cuts this year?"**

```bash
congress-approp compare --base-fy 2024 --current-fy 2026 --dir data \
    --use-authorities --real --format json | \
  python3 -c "
import sys, json
rows = json.load(sys.stdin)
cuts = [r for r in rows if r.get('inflation_flag') == 'real_cut']
cuts.sort(key=lambda r: r.get('real_delta_pct', 0))
for r in cuts[:20]:
    print(f'{r[\"account_name\"][:45]:45s} nominal={r[\"delta_pct\"]:+.1f}%  real={r[\"real_delta_pct\"]:+.1f}%')
"
```

This produces a list of programs where Congress increased nominal funding but inflation eroded the real value. Every row has source traceability to the enrolled bill.

**Recipe 3: "The CR penalty dashboard"**

For every CR substitution provision, find the matching final omnibus provision via TAS codes and compute the delta:

```bash
# Find all CR substitution provisions
congress-approp search --dir data --type cr_substitution --format json > cr_subs.json

# For each, the new_amount is what the CR provides and old_amount is what it replaces.
# The difference shows what programs lost (or gained) during the CR period.
python3 -c "
import json
subs = json.load(open('cr_subs.json'))
subs.sort(key=lambda s: abs(s.get('dollars', 0) - s.get('old_dollars', 0)), reverse=True)
for s in subs[:15]:
    delta = (s.get('dollars') or 0) - (s.get('old_dollars') or 0)
    print(f'{s[\"bill\"]:15s} {s.get(\"description\",\"\")[:40]:40s} delta=\${delta:>15,}')
"
```

**Recipe 4: "Track any account by name"**

The `trace` command accepts name fragments, not just FAS codes:

```bash
congress-approp trace "disaster relief" --dir data
congress-approp trace "child nutrition" --dir data
congress-approp trace "military personnel army" --dir data
congress-approp trace "FBI salaries" --dir data
```

Each produces a fiscal year timeline with bill citations and name variants. Add `--format json` to pipe into any charting tool.

### Recipes for Congressional Staffers

**Recipe 5: "302(b) scorecard by subcommittee"**

```bash
# For each subcommittee, get total BA by fiscal year
for sub in defense labor-hhs thud financial-services cjs energy-water interior agriculture legislative-branch milcon-va state-foreign-ops homeland-security; do
  echo "=== $sub ==="
  for fy in 2020 2021 2022 2023 2024 2025 2026; do
    total=$(congress-approp summary --dir data --fy $fy --subcommittee $sub --format json 2>/dev/null | \
      python3 -c "import sys,json; bills=json.load(sys.stdin); print(sum(b['budget_authority'] for b in bills))" 2>/dev/null)
    echo "  FY$fy: \$$total"
  done
done
```

This produces the subcommittee allocation trend that every committee chair asks for — 8 years of data, automatically aggregated from the underlying bills.

**Recipe 6: "New program tracker"**

```bash
# Find FAS codes that appear in FY2026 but not in FY2024
python3 -c "
import json
reg = json.load(open('data/authorities.json'))
for auth in reg['authorities']:
    fys = set(auth['fiscal_years'])
    if 2026 in fys and 2024 not in fys and 2023 not in fys:
        print(f'{auth[\"fas_code\"]:12s} {auth[\"fas_title\"][:55]}  (FYs: {sorted(fys)})')
"
```

**Recipe 7: "Advance appropriation exposure"**

```bash
# Show advance vs current-year split for VA
congress-approp summary --dir data --fy 2026 --subcommittee milcon-va --show-advance
```

The `--show-advance` flag separates current-year budget authority from advance appropriations (money committed for future fiscal years). For VA, advance appropriations are typically 60-70% of the total — understanding this split is critical for comparing year-over-year.

**Recipe 8: "Renamed accounts across congresses"**

```bash
python3 -c "
import json
reg = json.load(open('data/authorities.json'))
for auth in reg['authorities']:
    for ev in auth.get('events', []):
        if ev['event_type']['type'] == 'rename':
            print(f'FY{ev[\"fiscal_year\"]}: {auth[\"fas_code\"]}')
            print(f'  FROM: \"{ev[\"event_type\"][\"from\"]}\"')
            print(f'  TO:   \"{ev[\"event_type\"][\"to\"]}\"')
            print()
"
```

Lists all 40 detected rename events with fiscal year boundaries.

### Recipes for Data Analysis and Export

**Recipe 9: "Export full timeline for all accounts to CSV"**

```bash
python3 -c "
import json, csv, sys
reg = json.load(open('data/authorities.json'))
writer = csv.writer(sys.stdout)
writer.writerow(['fas_code', 'agency_code', 'title', 'fiscal_year', 'dollars', 'bills'])
for auth in reg['authorities']:
    for prov in auth['provisions']:
        for fy in prov['fiscal_years']:
            writer.writerow([
                auth['fas_code'], auth['agency_code'], auth['fas_title'],
                fy, prov.get('dollars', ''), prov['bill_identifier']
            ])
" > budget_timeline.csv
```

Produces a flat CSV suitable for Excel, R, or pandas with every provision-FY combination.

**Recipe 10: "Compare two subcommittees side by side"**

```bash
congress-approp compare --base-fy 2024 --current-fy 2026 \
    --subcommittee defense --dir data --use-authorities --format csv > defense_delta.csv

congress-approp compare --base-fy 2024 --current-fy 2026 \
    --subcommittee thud --dir data --use-authorities --format csv > thud_delta.csv
```

**Recipe 11: "Full-text search with source verification"**

```bash
# Find all provisions mentioning "opioid" and verify each against source
congress-approp search --dir data --keyword "opioid" --format json | \
  python3 -c "
import json, sys
results = json.load(sys.stdin)
for r in results:
    span = r.get('source_span', {})
    if span.get('verified'):
        print(f'{r[\"bill\"]:15s} {r[\"provision_type\"]:20s} [{span[\"start\"]}:{span[\"end\"]}] {r.get(\"raw_text\",\"\")[:60]}')
"
```

### Visualization Ideas

**Viz 1: Treemap — "Where does FY2026 money go?"**

Data source: `congress-approp summary --dir data --fy 2026 --by-agency --format json`

Layout: 12 subcommittees as top-level rectangles, agencies within each, individual accounts within agencies. Rectangle size proportional to budget authority. Color by year-over-year change (green = increase, red = decrease vs FY2024).

Implementation: D3.js treemap with `trace --format json` data for drill-down on click.

**Viz 2: Slope chart — "FY2020 → FY2026 structural shifts"**

Data source: `authorities.json` — filter to top 50 accounts by total_dollars.

Layout: Left axis = FY2020 rank position, right axis = FY2026 rank position. Lines connecting each account. Color by direction (rose up = blue, fell = red). Thickness proportional to dollar amount.

Shows which programs are growing in priority and which are shrinking, independent of inflation.

**Viz 3: Timeline sparklines in account table**

Data source: `authority list --format json`

Layout: Standard table with FAS code, title, total BA columns. Add a 100px-wide sparkline column showing the 8-year BA trend for each account. Use SVG inline in an HTML table.

Makes the static authority list immediately scannable for trends.

**Viz 4: Rename river — alluvial/Sankey diagram**

Data source: `authorities.json` — filter to authorities with `name_change` classification.

Layout: Vertical axis = fiscal years (FY2019-FY2026). Each account is a horizontal band whose width = BA. When a name changes, the band splits at the transition FY and reconnects with the new label. Color by subcommittee.

Visually demonstrates that the same money flows through even when Congress changes the name — the FAS code (and thus the band) stays continuous.

**Viz 5: Verification confidence heatmap**

Data source: `verification.json` from each bill.

Layout: X axis = bills (sorted by size), Y axis = verification categories (Verified, Ambiguous, NotFound, Exact text, Normalized text, No match). Cell color intensity proportional to count. Shows at a glance which bills and which verification tiers have the most issues.

**Viz 6: TAS resolution waterfall**

Data source: All `tas_mapping.json` files.

Layout: Stacked horizontal bar per bill. Three segments: deterministic match (green), LLM resolved (blue), unmatched (red). Sorted by total provisions. Shows the two-tier resolution working across the dataset.

**Viz 7: "The Federal Budget at a Glance" interactive explorer**

Data source: `authorities.json` + `trace --format json` for drill-down.

Layout: Start with 12 subcommittee cards, each showing total BA and sparkline. Click a subcommittee to see its agencies. Click an agency to see its accounts. Click an account to see the full `trace` timeline with bill citations and rename events.

This is the "capstone" visualization — it makes the entire dataset navigable for non-technical users.

### Demo Scripts

**Demo 1: "5-minute quickstart" — no API keys**

```bash
git clone https://github.com/cgorski/congress-appropriations.git
cd congress-appropriations
cargo install --path .

# What bills do we have?
congress-approp summary --dir data

# How much did FEMA get for disaster relief?
congress-approp trace "disaster relief" --dir data

# What changed in transportation funding?
congress-approp compare --base-fy 2024 --current-fy 2026 \
    --subcommittee thud --dir data --use-authorities

# Search by meaning
source ~/openai-cantina-gorski.source  # only needed for semantic search
congress-approp search --dir data --semantic "veterans healthcare" --top 5
```

**Demo 2: "Process a new bill end-to-end"**

```bash
# 1. Download (free)
congress-approp download --congress 119 --type hr --number 9999

# 2. Extract (~$5-15 per omnibus, requires ANTHROPIC_API_KEY)
congress-approp extract --dir data/119-hr9999 --parallel 5

# 3. Verify and repair (free, ~1 second)
congress-approp verify-text --dir data --bill 119-hr9999 --repair

# 4. Enrich (free, ~1 second)
congress-approp enrich --dir data/119-hr9999

# 5. Resolve TAS (~$2-4 per omnibus, requires ANTHROPIC_API_KEY)
congress-approp resolve-tas --dir data --bill 119-hr9999

# 6. Embed (~$0.50, requires OPENAI_API_KEY)
congress-approp embed --dir data/119-hr9999

# 7. Rebuild authority registry (free, ~1 second)
congress-approp authority build --dir data --force

# Now trace any account and the new bill is included
congress-approp trace 070-0400 --dir data
```

**Demo 3: "Source traceability proof"**

```bash
# Show that every provision is traceable
congress-approp verify-text --dir data

# Pick one provision and prove it
python3 -c "
import json
ext = json.load(open('data/118-hr2882/extraction.json'))
p = ext['provisions'][0]
span = p['source_span']
print(f'Provision 0: {p[\"provision_type\"]}')
print(f'Account: {p.get(\"account_name\", \"\")}')
print(f'Source span: bytes {span[\"start\"]}-{span[\"end\"]} in {span[\"file\"]}')
print()

# Verify mechanically
source_bytes = open(f'data/118-hr2882/{span[\"file\"]}', 'rb').read()
actual = source_bytes[span['start']:span['end']].decode('utf-8')
print(f'From source file: \"{actual[:100]}\"')
print(f'From raw_text:    \"{p[\"raw_text\"][:100]}\"')
print(f'Match: {actual == p[\"raw_text\"]}')
"
```

**Demo 4: "TAS resolution quality check"**

```bash
# Dry-run shows what would be resolved and estimated cost
congress-approp resolve-tas --dir data --dry-run

# Run deterministic only (free)
congress-approp resolve-tas --dir data --no-llm

# Check a specific mapping
python3 -c "
import json
m = json.load(open('data/118-hr2882/tas_mapping.json'))
for mp in m['mappings'][:5]:
    fas = mp.get('fas_code', 'NONE')
    print(f'  {fas:12s} [{mp[\"confidence\"]:10s}] {mp[\"method\"]:20s} {mp[\"account_name\"][:40]}')
print(f'Match rate: {m[\"summary\"][\"match_rate_pct\"]:.1f}%')
"
```

**Demo 5: "Account rename detection"**

```bash
# Show all detected rename events
congress-approp trace 000-0438 --dir data

# Output shows:
#   Events:
#     ⟹  FY2025: renamed from "Allowances and Expenses"
#                            to "Members' Representational Allowances"
```

## Development Methodology

### How the v6.0.0 work was developed

This plan was developed through a structured analysis process using simulated expert personas, real tool execution, real LLM API calls, and real data computations against the actual 32-bill dataset. The methodology is documented here so it can be reproduced for future planning.

### The Expert Panel Approach

The analysis is conducted by assembling a panel of simulated domain experts, each bringing a specific lens to the problem. **The panel members are personas, not real people.** They represent the perspectives needed to evaluate the tool from every angle — journalism, legislative process, type systems, data quality, LLM reliability, systems architecture, technical writing, statistics, data integrity, information science, data structures, visualization, string alignment, workflow design, and implementation planning.

**There are 15 personas. Each gets a detailed bio (3-5 sentences) printed at the start of the session. The bio includes: years of experience, specific organizations worked at, area of deep expertise, and the specific question or principle they bring to every analysis.**

**The personas and their roles:**

**1. Investigative Journalist** — Represents the end user who needs to find, verify, and publish spending numbers. Tests the tool by asking real journalistic questions ("How much did Congress give FEMA?") and evaluating whether the output is publication-ready. Identifies when numbers could be misleading (advance vs. current-year, mandatory vs. discretionary). Key question: *"Can I publish this number without getting burned?"*

**2. Congressional Staffer** — Represents the domain expert who knows how appropriations actually work. Validates that the tool's classifications match legislative reality. Identifies when account names, division letters, or bill classifications could confuse users. Tests CR substitution tracing, year-over-year comparison, and subcommittee-level analysis. Key question: *"Does this tool understand appropriations the way the Clerk's office does?"*

**3. Rust Systems Engineer / Type Theory Specialist** — Evaluates the code architecture: whether types enforce correctness, whether the enum design is sound, whether error handling is robust. Proposes type-level solutions (enums instead of strings, tagged unions for provision types). Reviews the `from_value.rs` resilient parsing and the hash chain design. Key question: *"Do the types make illegal states unrepresentable?"*

**4. Big Data / Fuzzy Matching Architect** — Focuses on entity resolution: how to match "National Science Foundation—Research and Related Activities" to "Research and Related Activities" across bills. Quantifies the matching problem (151 "Salaries and Expenses" accounts across agencies). Tests embedding-based and TAS-based cross-bill matching and identifies failure modes. Key question: *"How many false merges are hiding in your entity resolution, and how do you know?"*

**5. LLM Systems Researcher** — Evaluates the trust model: what the LLM can and can't guarantee, where hallucination risk exists, how verification provides guardrails. Designs the LLM-assisted TAS resolution and link verification pipelines. Quantifies the 0 NotFound metric and what it actually proves vs. doesn't prove. Key question: *"Where exactly in this pipeline can the model hallucinate, and what happens when it does?"*

**6. Solutions Architect** — Designs the end-to-end system: pipeline stages, hash chain, immutability model, file formats, CLI command structure. Ensures backward compatibility. Maps user workflows to CLI commands. Identifies operational issues (multiple --dir, deduplication, data layout). Key question: *"Can a user pick this up 6 months from now and trust their old data still works?"*

**7. Technical Writer / Documentation Architect** — Designs the documentation structure using the Diátaxis framework (tutorials, how-to guides, explanations, reference). Ensures every audience (journalists, staffers, developers, auditors, contributors) has a clear entry point. Reviews chapter content for accuracy and completeness. Key question: *"Can someone who has never seen this tool accomplish a real task in under 5 minutes?"*

**8. Computational Statistician** — Challenges the team's intuitive thresholds with empirical distribution analysis. Validates that claimed metrics (99.4% TAS, 100% traceability) are measured against real data, not assumed. Proposes evidence-based confidence tiers based on evidence type rather than raw scores. Key question: *"Where are you using intuition where you should be using distributions?"*

**9. Data Integrity Researcher** — The skeptic who demands proof. Asks "what does 0 NotFound actually prove?" and tests whether cross-year comparisons produce correct numbers. Catches false positives in matching (the 902 "Salaries and Expenses" catastrophe). Validates the source_span invariant mechanically. Key question: *"What does your verification actually prove, and what does it silently assume?"*

**10. Information Science / Knowledge Organization Specialist** — Brings expertise in authority records, stable identifiers, and entity lifecycle tracking. Designed the FAS-code-based authority system modeled on Library of Congress authority records and NARA identifier schemes. Identified the FAST Book and USASpending API as reference data sources. Key question: *"What is the identity of this thing, and how do you know it's the same thing 30 years later when everything about it has changed?"*

**11. Data Structures / Persistent Data Systems Expert** — Evaluates the minimum structure needed for efficient historical queries. Designed the append-only authority model, identified the FAS code as the natural primary key, and rejected UUID/ULID alternatives. Key question: *"What is the minimum structure needed to answer any historical query in O(log n), and what must be immutable to guarantee that past answers never change?"*

**12. Data Visualization / Narrative Analytics Expert** — Evaluates how the data translates into charts and stories. Cataloged 20 visualization types possible with the dataset. Validated that `trace --format json` produces chart-ready data structures. Key question: *"What is the one chart that makes the reader say 'I had no idea' — and how do we make sure it's not lying?"*

**13. Computational Linguistics / String Alignment Specialist** — Designed the 3-tier deterministic raw_text repair algorithm (prefix → substring → normalized position mapping). Diagnosed the byte-offset vs character-offset confusion. Classified all 581 raw_text mismatches by failure mode and proved the repair achieves 100% with zero LLM calls. Key question: *"Where exactly does the alignment break, what is the systematic cause, and can you prove the repair is correct?"*

**14. DevOps & Developer Workflow Architect** — Maps the complete pipeline from download to authority build. Defines minimum viable paths per audience (journalist, staffer, researcher). Ensures each step has clear inputs, outputs, costs, and error recovery. Key question: *"If I hand this to someone who's never seen it, can they produce correct results on the first try, and can they tell if something went wrong?"*

**15. Technical Program Manager / Implementation Planner** — Turns architectural designs into work breakdown structures with clear interfaces, milestones, and test criteria. Tracks every assumption, every file that needs changing, and every test that needs writing. Manages phase dependencies and risk registers. Key question: *"What's the exact interface contract between each component, and what's the simplest test that proves it works?"*

### How the panel works (instructions for reproduction)

**CRITICAL: The panel members talk and work simultaneously — they do NOT do bulk reading/analysis before discussion.**

The methodology is:

1. **Print the expert bios immediately.** Each persona gets a 3-5 sentence bio describing their background, expertise, and the specific question they bring to every analysis.

2. **Start the conversation immediately after the bios.** No preamble, no "let me first read all the files." The experts begin talking right away, proposing what to investigate.

3. **Run tools DURING the conversation, not before.** When an expert says "let me check if the THUD accounts match across fiscal years," they run the actual tool command or Python script right then. The results appear inline in the conversation and the experts react to them in real time.

4. **No bulk operations.** Don't read every file before discussing. Don't run every possible query before analyzing results. Each tool call is motivated by a specific question from a specific expert. This keeps the analysis focused and prevents wasted computation.

5. **Experts challenge each other constructively.** The statistician challenges the claimed metrics. The data integrity researcher catches false positives. The journalist challenges whether the output is actually publishable. The staffer challenges whether the classifications match legislative reality. The string alignment specialist diagnoses matching failures at the character level.

6. **Use real data, real APIs, real tools.** Every claim in the analysis is backed by actual command output against the 32-bill dataset, actual LLM API calls (Claude Opus for TAS resolution, OpenAI for embeddings), and actual Python computations against real `extraction.json` files.

7. **Report findings immediately, not in a summary at the end.** When an expert discovers a problem (like the 902 false positives from containment matching), they report it in the conversation right when they find it. Other experts react and propose solutions in real time.

8. **The conversation produces the plan.** The design emerges from the discussion — it's not presented top-down and then validated. The experts discover the problems, propose solutions, test them, and refine them through dialogue.

9. **Do retros between phases.** After each implementation phase, every panel member reports good/bad findings. This catches issues early and adjusts the plan before the next phase begins.

10. **First-principles checks periodically.** At key milestones, every panel member steps back from the details and evaluates whether the overall approach is correct. This prevents tunnel vision on local optimizations that miss systemic issues.

### What this methodology produces

- **Validated use cases** — every use case was tested against real data with real tool runs
- **Discovered and fixed bugs** — the "Salaries and Expenses" false positive (902 provisions matched to the wrong account) was caught by running real data through the matcher and inspecting the output
- **Validated external data sources** — the FAST Book, USASpending API, and TAS code structure were investigated with real API calls and data analysis
- **Calibrated thresholds** — TAS match rates measured across all 24 eligible bills, not assumed from small samples
- **Proved system invariants** — 100% source traceability verified mechanically across 34,568 provisions
- **Quantified effort and cost** — LLM costs measured from actual API usage ($85 for TAS resolution), not estimated

### How to reproduce this methodology for future planning

1. Load this NEXT_STEPS.md for full context on the current state
2. Assemble the expert panel — print all 15 bios with their full backgrounds and key questions
3. Give them a specific question, feature area, or problem to investigate
4. **Let them start talking IMMEDIATELY after the bios are printed** — no "let me first read the codebase" phase
5. **Run tools DURING the conversation** — when an expert asks "what happens if we search for FEMA across all bills?" they run the actual command right then. When they want to verify a number against XML, they grep the XML right then. When they want to test a TAS mapping, they make the actual LLM call right then.
6. **NO bulk operations before or after discussion** — don't read every file first, don't run every query first, don't summarize at the end. The discussion IS the exploration. Each tool call is motivated by a specific question from a specific expert in the flow of conversation.
7. **Experts REACT to tool output in real time** — when a command returns unexpected results (like 902 provisions matching a Senate account), the experts discuss what went wrong and propose solutions immediately, not in a later analysis phase
8. **Let the plan emerge from the conversation** — don't present a plan and validate it. Let the experts discover problems, test solutions, and refine designs through dialogue
9. **Validate every claim with a real tool run, real API call, or real data computation** — no hypothetical results, no assumed outputs
10. **Do retros after each phase** — every panel member gives good/bad assessment before proceeding. Plans are adjusted based on retro findings.
11. **Step back to first principles periodically** — have every expert evaluate whether the overall direction is correct, not just whether the current task is done well

### Example of correct methodology flow (v6.0.0 session)

```
Data Integrity: "Let me check if the TAS matcher has false positives."
[runs: Python analysis of all 6,177 matched provisions grouped by FAS code]
Data Integrity: "FAS 000-0171 has 902 matches from every agency — that's Senate Legal Counsel.
  The containment matcher is matching every 'Salaries and Expenses' to the first FAS entry."
Fuzzy Matching: "That's catastrophic. We need to remove containment matching entirely."
Systems Engineer: "I'll restructure the lookup to a multi-map and add agency disambiguation."
[implements fix, runs tests]
Statistician: "The match rate dropped from 92% to 58% — but it's 58% CORRECT, not 92% with hidden errors."
LLM Researcher: "The remaining 42% is exactly what the LLM tier should handle."
[runs: resolve-tas with LLM on H.R. 3401]
LLM Researcher: "11/11 correct. The LLM resolved all DHS sub-agency accounts."
All: "Ship it. Deterministic for certainty, LLM for ambiguity, verify everything against FAST Book."
```

### Example of INCORRECT methodology (don't do this)

```
[reads all 20 source files]
[runs 50 queries to gather data]
[writes a summary document]
[presents the summary to the team]
[team discusses the summary]
```

This is wrong because it separates exploration from discussion. The value is in the experts reacting to real-time results, not in a pre-computed summary. The surprises (the 902 false positives, the byte-offset vs character-offset confusion, the $75B budget regression) only emerge when experts are looking at real data and questioning assumptions in real time.
```

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']

[Error: Tool calls are disabled in this context. Attempted to call 'terminal']