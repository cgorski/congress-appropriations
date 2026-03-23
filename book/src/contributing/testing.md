# Testing Strategy

This chapter explains how the test suite is structured, how to run tests, what the key regression guards are, and how to add tests for new features.

## Test Overview

The project has two categories of tests:

| Category | Location | Count | What They Test |
|----------|----------|-------|----------------|
| **Unit tests** | Inline `#[cfg(test)] mod tests` in each module | ~130 | Individual functions, type round-trips, parsing logic, classification, link management |
| **Integration tests** | `tests/cli_tests.rs` | 42 | Full CLI commands against the `data/` data, including enrich, relate, link workflow, FY/subcommittee filtering, --show-advance, case-insensitive compare |
| **Total** | | **~172** | |

All tests run with `cargo test` and must pass before every commit.

## Running Tests

### Full test cycle (do this before every commit)

```bash
cargo fmt                           # Format code
cargo fmt --check                   # Verify formatting (CI does this)
cargo clippy -- -D warnings         # Lint (CI treats warnings as errors)
cargo test                          # Run all tests
```

All four must pass. The CI runs `fmt --check`, `clippy -D warnings`, and `cargo test` on every push to `main` and every pull request.

### Running specific tests

```bash
# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test cli_tests

# Run a specific test by name
cargo test budget_authority_totals

# Run tests with output visible (normally captured)
cargo test -- --nocapture

# Run tests matching a pattern
cargo test search
```

### Testing with verbose output

```bash
# See which tests are running
cargo test -- --test-threads=1

# See stdout/stderr from tests
cargo test -- --nocapture
```

## The Critical Regression Guard

The single most important test in the suite is `budget_authority_totals_match_expected`:

```rust
#[test]
fn budget_authority_totals_match_expected() {
    let output = cmd()
        .args(["summary", "--dir", "data", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    let expected: Vec<(&str, i64, i64)> = vec![
        ("H.R. 4366", 846_137_099_554, 24_659_349_709),
        ("H.R. 5860", 16_000_000_000, 0),
        ("H.R. 9468", 2_882_482_000, 0),
    ];

    for (bill, expected_ba, expected_resc) in &expected {
        let entry = data
            .iter()
            .find(|b| b["identifier"].as_str().unwrap() == *bill)
            .unwrap_or_else(|| panic!("Missing bill: {bill}"));

        let ba = entry["budget_authority"].as_i64().unwrap();
        let resc = entry["rescissions"].as_i64().unwrap();

        assert_eq!(ba, *expected_ba, "{bill} budget authority mismatch");
        assert_eq!(resc, *expected_resc, "{bill} rescissions mismatch");
    }
}
```

This test **hardcodes the exact budget authority and rescission totals** for every example bill:

| Bill | Budget Authority | Rescissions |
|------|-----------------|-------------|
| H.R. 4366 | $846,137,099,554 | $24,659,349,709 |
| H.R. 5860 | $16,000,000,000 | $0 |
| H.R. 9468 | $2,882,482,000 | $0 |

Any change to the extraction data, the `compute_totals()` function, the provision parsing logic, or the budget authority calculation that would alter these numbers is caught immediately. This is the tool's financial integrity guard.

**If this test fails, stop and investigate.** Either the change was intentional (and the test values need updating with justification) or the change introduced a regression in the budget authority calculation.

## Unit Test Patterns

Unit tests are inline in each module, in a `#[cfg(test)] mod tests` block at the bottom of the file:

```rust
// Example from ontology.rs:

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provision_round_trip_appropriation() {
        let json = r#"{
            "provision_type": "appropriation",
            "account_name": "Test Account",
            "agency": "Test Agency",
            "amount": {
                "value": {"kind": "specific", "dollars": 1000000},
                "semantics": "new_budget_authority",
                "text_as_written": "$1,000,000"
            },
            "detail_level": "top_level",
            "section": "SEC. 101",
            "confidence": 0.95,
            "raw_text": "For necessary expenses..."
        }"#;

        let p: Provision = serde_json::from_str(json).unwrap();
        assert_eq!(p.provision_type_str(), "appropriation");
        assert_eq!(p.account_name(), "Test Account");
        assert_eq!(p.section(), "SEC. 101");

        // Round-trip: serialize back to JSON and re-parse
        let serialized = serde_json::to_string(&p).unwrap();
        let p2: Provision = serde_json::from_str(&serialized).unwrap();
        assert_eq!(p2.provision_type_str(), "appropriation");
        assert_eq!(p2.account_name(), "Test Account");
    }

    #[test]
    fn compute_totals_excludes_sub_allocations() {
        // Create a bill extraction with a top-level and sub-allocation
        // Verify that only top-level counts toward BA
        // ...
    }
}
```

### What to unit test

| Module | What to Test |
|--------|-------------|
| `ontology.rs` | Provision serialization round-trips, `compute_totals()` with various scenarios, accessor methods |
| `from_value.rs` | Resilient parsing: missing fields, wrong types, unknown provision types, edge cases |
| `verification.rs` | Amount checking logic, raw text matching tiers, completeness calculation |
| `embeddings.rs` | Cosine similarity, vector normalization, load/save round-trip |
| `staleness.rs` | Hash computation, staleness detection |
| `query.rs` | Search filters, compare matching, summarize aggregation, rollup logic |
| `xml.rs` | XML parsing edge cases, chunk splitting |
| `text_index.rs` | Dollar pattern detection, section header detection |

### Unit test conventions

1. **Place tests at the bottom of the module** they test, in `#[cfg(test)] mod tests { use super::*; ... }`
2. **Name tests descriptively** — `compute_totals_excludes_sub_allocations` is better than `test_compute`
3. **Test edge cases** — empty inputs, null fields, zero-dollar amounts, maximum values
4. **Use real-world-ish data** — test with provision structures similar to what the LLM actually produces
5. **Keep tests fast** — no file I/O, no network calls, no sleeping

## Integration Test Patterns

Integration tests live in `tests/cli_tests.rs` and run the actual compiled binary against the `data/` data:

```rust
use assert_cmd::Command;
use std::str;

fn cmd() -> Command {
    Command::cargo_bin("congress-approp").unwrap()
}

#[test]
fn summary_table_runs_successfully() {
    cmd()
        .args(["summary", "--dir", "data"])
        .assert()
        .success()
        .stdout(predicates::str::contains("H.R. 4366"))
        .stdout(predicates::str::contains("H.R. 5860"))
        .stdout(predicates::str::contains("H.R. 9468"))
        .stdout(predicates::str::contains("Omnibus"))
        .stdout(predicates::str::contains("Continuing Resolution"))
        .stdout(predicates::str::contains("Supplemental"));
}
```

### Existing integration tests

The test suite covers these commands and scenarios:

| Test | What It Checks |
|------|---------------|
| `budget_authority_totals_match_expected` | **Critical** — exact BA and rescission totals for all three bills |
| `summary_table_runs_successfully` | Summary command outputs all three bills with correct classifications |
| `summary_json_output_is_valid` | JSON output parses correctly with expected fields |
| `summary_csv_output_has_header` | CSV output includes a header row |
| `summary_by_agency_shows_departments` | `--by-agency` flag produces department rollup |
| `search_by_type_appropriation` | Type filter returns results with correct type |
| `search_by_type_rescission` | Rescission search returns results |
| `search_by_type_cr_substitution` | CR substitution search returns 13 results |
| `search_by_agency` | Agency filter narrows results |
| `search_by_keyword` | Keyword search finds provisions containing the term |
| `search_json_output_is_valid` | JSON output parses with expected fields |
| `search_csv_output` | CSV output is parseable |
| `search_list_types` | `--list-types` flag shows all provision types |
| `compare_runs_successfully` | Compare command produces output with expected accounts |
| `compare_json_output_is_valid` | Compare JSON output parses correctly |
| `audit_runs_successfully` | Audit command shows all three bills |
| `audit_shows_zero_not_found` | **Critical** — NotFound = 0 for all bills |
| `upgrade_dry_run` | Upgrade dry run completes without modifying files |

### Writing new integration tests

```rust
#[test]
fn my_new_command_works() {
    // 1. Run the command against example data
    let output = cmd()
        .args(["my-command", "--dir", "data", "--format", "json"])
        .output()
        .unwrap();

    // 2. Check it succeeded
    assert!(output.status.success(), "Command failed: {}", 
        str::from_utf8(&output.stderr).unwrap());

    // 3. Parse the output
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    // 4. Verify expected properties
    assert!(!data.is_empty(), "Expected at least one result");
    assert!(data[0]["some_field"].is_string(), "Expected some_field to be a string");
}
```

### Integration test conventions

1. **Always use `--dir data`** — the included example data is the test fixture
2. **Test all output formats** (`table`, `json`, `csv`) for new commands
3. **Parse JSON output and verify structure** — don't just check for substring matches on JSON
4. **Check for specific expected values** where possible (like the budget authority totals)
5. **Test error cases** — what happens with a bad `--dir` path, an invalid `--type` value, etc.
6. **Don't test semantic search in CI** — there's no `OPENAI_API_KEY` in the CI environment. Cosine similarity and vector loading have unit tests instead.

## What Is NOT Tested

### Semantic search (no API key in CI)

The GitHub Actions CI environment does not have an `OPENAI_API_KEY`. This means:

- `search --semantic` is not tested in CI
- `embed` is not tested in CI
- The OpenAI API client is not tested in CI

These are tested locally by the developer. The underlying cosine similarity, vector loading, and embedding text construction functions have unit tests that don't require API access.

### LLM extraction quality

There are no automated tests that verify the quality of LLM extraction — that would require calling the Anthropic API and comparing results to ground truth. Instead:

- Budget authority totals serve as a proxy for extraction quality (if totals match, major provisions are correct)
- The verification pipeline (`audit`) provides automated quality metrics
- Manual review of new extractions is expected before committing example data

### Performance benchmarks

There are no automated performance tests. The performance characteristics documented in the architecture chapter are based on manual measurement and informal benchmarking.

## Data Integrity Check (Manual)

In addition to `cargo test`, the project includes a manual data integrity check that can be run as a shell command:

```bash
./target/release/congress-approp summary --dir data --format json | python3 -c "
import sys, json
expected = {'H.R. 4366': 846137099554, 'H.R. 5860': 16000000000, 'H.R. 9468': 2882482000}
for b in json.load(sys.stdin):
    assert b['budget_authority'] == expected[b['identifier']]
print('Data integrity: OK')
"
```

This is the same check as the `budget_authority_totals_match_expected` test but runs against the release binary. It's useful as a final verification before committing or publishing.

## CI/CD Pipeline

GitHub Actions (`.github/workflows/ci.yml`) runs on every push to `main` and every pull request:

```yaml
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Check formatting
        run: cargo fmt --check
      - name: Clippy
        run: cargo clippy -- -D warnings
      - name: Test
        run: cargo test
```

Three checks, all must pass:

1. **`cargo fmt --check`** — Code must be formatted according to `rustfmt` rules
2. **`cargo clippy -- -D warnings`** — No clippy warnings allowed (warnings are errors)
3. **`cargo test`** — All unit and integration tests must pass

The CI does NOT:
- Run semantic search tests (no `OPENAI_API_KEY`)
- Run extraction tests (no `ANTHROPIC_API_KEY`)
- Run download tests (no `CONGRESS_API_KEY`)
- Test against real API endpoints

## Adding Tests for New Features

### For a new CLI command

1. Add at least three integration tests:
   - Basic execution with `--dir data` succeeds
   - JSON output parses correctly with expected fields
   - Filters work as expected
2. Add unit tests for the library function it calls

### For a new provision type

1. Add a unit test in `ontology.rs` for serialization round-trip
2. Add a unit test in `from_value.rs` for resilient parsing (missing fields, wrong types)
3. Verify `budget_authority_totals_match_expected` still passes — your new type shouldn't change existing totals unless deliberately designed to

### For a new search filter

1. Add an integration test in `cli_tests.rs` that exercises the filter
2. Verify the filter works with `--format json` (check the output structure)
3. Test the filter in combination with existing filters

### For a new output format

1. Add integration tests for the new format on at least `search` and `summary` commands
2. Verify the output is parseable by its target consumer (e.g., valid CSV, valid JSON)

## Debugging Test Failures

### "budget_authority_totals_match_expected" failed

This means the budget authority or rescission totals changed. Possible causes:

1. **Example data changed** — was `extraction.json` modified accidentally?
2. **`compute_totals()` logic changed** — did the filtering criteria for budget authority change?
3. **`from_value.rs` parsing changed** — did a change in the resilient parser alter how amounts are parsed?
4. **A new provision type was added** that unintentionally contributes to budget authority

Investigation steps:

```bash
# Check the actual values
./target/release/congress-approp summary --dir data --format json | python3 -c "
import sys, json
for b in json.load(sys.stdin):
    print(f\"{b['identifier']}: BA={b['budget_authority']}, Resc={b['rescissions']}\")
"

# Compare to expected
# H.R. 4366: BA=846137099554, Resc=24659349709
# H.R. 5860: BA=16000000000, Resc=0
# H.R. 9468: BA=2882482000, Resc=0
```

### Tests pass locally but fail in CI

Common causes:

1. **Unformatted code** — run `cargo fmt` locally (CI checks with `cargo fmt --check`)
2. **Clippy warnings** — run `cargo clippy -- -D warnings` locally (CI treats warnings as errors)
3. **Platform differences** — the CI runs on Ubuntu; if you develop on macOS, there may be subtle differences in text handling
4. **Missing `cargo build`** — integration tests need the binary; `cargo test` builds it automatically, but sometimes caching can cause stale binaries

### A test is flaky (passes sometimes, fails sometimes)

This shouldn't happen in the current test suite because there's no randomness, no network calls, and no timing dependencies. If you encounter a flaky test:

1. Run it with `--test-threads=1` to rule out parallelism issues
2. Check if it depends on filesystem ordering (use `sort` on any directory listings)
3. Check if it depends on HashMap iteration order (use `BTreeMap` or sort results)

## Summary

| Rule | Reason |
|------|--------|
| Run `cargo fmt && cargo clippy -- -D warnings && cargo test` before every commit | CI rejects improperly formatted or warning-producing code |
| Never change the expected budget authority totals without justification | They're the tool's financial integrity guard |
| Test all output formats for new commands | Users depend on JSON/CSV parsability |
| Unit test library functions, integration test CLI commands | Two layers of confidence |
| Don't test semantic search in CI | No API keys in CI; test cosine similarity with unit tests instead |

## Next Steps

- **[Style Guide and Conventions](./style-guide.md)** — coding standards
- **[Adding a New CLI Command](./new-command.md)** — the full process for new subcommands
- **[Adding a New Provision Type](./new-provision-type.md)** — the full process for new types
- **[Architecture Overview](./architecture.md)** — the big-picture design