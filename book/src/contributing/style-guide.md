# Style Guide and Conventions

Coding standards and practices for contributing to `congress-approp`. These conventions are enforced by CI — pull requests that don't follow them will be rejected automatically.

## The Non-Negotiables

These three checks run on every push and every pull request. All must pass.

### 1. Format with rustfmt

```bash
cargo fmt
```

Run this before every commit. The CI checks with `cargo fmt --check` and rejects improperly formatted code. There is no `.rustfmt.toml` override — the project uses the default `rustfmt` configuration.

### 2. No clippy warnings

```bash
cargo clippy -- -D warnings
```

Clippy warnings are treated as errors in CI. Fix every warning at its root cause.

**Do NOT suppress warnings with `#[allow]` annotations** unless there is a compelling reason and the team agrees. The most common exception is `#[allow(clippy::too_many_arguments)]` on functions that genuinely need many parameters (like provision constructors), but even this should be used sparingly.

**Do NOT use `_` prefixes on variable names** just to suppress "unused variable" warnings. If a variable is unused, remove it. If it's a function parameter that must exist for API compatibility but isn't used in the current implementation, use `_name` (single underscore prefix) — but consider whether the function signature should change instead.

### 3. All tests pass

```bash
cargo test
```

All ~172 tests (130 unit + 42 integration) must pass. See [Testing Strategy](./testing.md) for details.

### The full cycle

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

Run this as a single command before every commit. If any step fails, fix it before proceeding.

## Code Organization

### Library function first, CLI second

New logic goes in library modules (`query.rs`, `embeddings.rs`, or a new module under `src/approp/`). The CLI handler in `main.rs` calls the library function and formats the output.

```rust
// Good: Library function is pure, CLI handler formats
// In query.rs:
pub fn top_provisions(bills: &[LoadedBill], count: usize) -> Vec<TopProvision> { ... }

// In main.rs:
fn handle_top(dir: &str, count: usize, format: &str) -> Result<()> {
    let bills = loading::load_bills(Path::new(dir))?;
    let results = query::top_provisions(&bills, count);
    // ... format and print results ...
}
```

```rust
// Bad: Business logic in main.rs
fn handle_top(dir: &str, count: usize, format: &str) -> Result<()> {
    let bills = loading::load_bills(Path::new(dir))?;
    let mut all_provisions = Vec::new();
    for bill in &bills {
        for p in &bill.extraction.provisions {
            // ... inline filtering and sorting logic ...
        }
    }
    // ... 200 lines of inline computation ...
}
```

### All query functions take `&[LoadedBill]`

Library functions in `query.rs` take loaded data as input and return plain structs. They never do I/O, never format output, never call APIs, and never print anything.

```rust
// Good: Pure function
pub fn summarize(bills: &[LoadedBill]) -> Vec<BillSummary> { ... }

// Bad: Does I/O
pub fn summarize(dir: &Path) -> Result<()> { ... }

// Bad: Formats output
pub fn summarize(bills: &[LoadedBill]) -> String { ... }
```

### Serde for everything

All data types derive `Serialize` and `Deserialize`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyType {
    pub field: String,
    pub amount: i64,
}
```

Output structs (returned by library functions for CLI consumption) derive at least `Serialize`:

```rust
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub bill: String,
    pub dollars: Option<i64>,
    // ...
}
```

This enables JSON, JSONL, and CSV output for free — the CLI handler just calls `serde_json::to_string()` or `csv::Writer::serialize()`.

### Tests in the same file

Unit tests go in a `#[cfg(test)] mod tests` block at the bottom of the module they test:

```rust
// At the bottom of src/approp/query.rs:

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_computes_correct_totals() {
        // ...
    }

    #[test]
    fn search_filters_by_type() {
        // ...
    }
}
```

Integration tests (which run the actual binary) go in `tests/cli_tests.rs`.

## Naming Conventions

### Files and modules

- **snake_case** for all file and module names: `from_value.rs`, `text_index.rs`, `cli_tests.rs`
- Module names should describe what they contain, not what they do: `ontology.rs` (types), not `define_types.rs`

### Types and enums

- **CamelCase** for all type names: `BillExtraction`, `AmountSemantics`, `LoadedBill`
- Enum variants are also CamelCase: `Provision::Appropriation`, `AmountValue::Specific`
- Acronyms are treated as words: `CrSubstitution` (not `CRSubstitution`), `XmlParser` (not `XMLParser`)

### Functions and methods

- **snake_case** for all function names: `compute_totals()`, `load_bills()`, `parse_provision()`
- CLI handler functions are prefixed with `handle_`: `handle_search()`, `handle_summary()`, `handle_extract()`
- Boolean-returning methods use `is_` prefix: `is_definite()`, `is_empty()`
- Getter methods use the field name without `get_` prefix: `account_name()`, `division()`, `amount()`

### Constants

- **SCREAMING_SNAKE_CASE** for constants: `DEFAULT_MODEL`, `MAX_TOKENS`, `KNOWN_PROVISION_TYPES`

### Command-line flags

- **kebab-case** for multi-word flags: `--dry-run`, `--output-dir`, `--min-dollars`, `--by-agency`
- Single-character short flags where natural: `-v` (verbose), `-t` (type), `-a` (agency), `-k` (keyword), `-n` (count)
- Use `r#type` in Rust (since `type` is a keyword): `r#type: Option<String>`

## Error Handling

### Use `anyhow` for CLI code

```rust
use anyhow::{Context, Result};

fn handle_summary(dir: &str) -> Result<()> {
    let bills = loading::load_bills(Path::new(dir))
        .context("Failed to load bills")?;
    // ...
    Ok(())
}
```

The `.context()` method adds human-readable context to errors. Use it on every fallible operation that could fail for user-facing reasons (file not found, API error, parse error).

### Use `thiserror` for library errors

If a library module needs typed errors (rather than `anyhow::Error`), define them with `thiserror`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LoadError {
    #[error("No extraction.json found in {0}")]
    NoExtraction(PathBuf),
    #[error("Failed to parse {path}: {source}")]
    ParseError {
        path: PathBuf,
        source: serde_json::Error,
    },
}
```

### Never panic in library code

Library functions should return `Result<T>` instead of panicking. Use `.unwrap()` only in tests or when the invariant is provably guaranteed (e.g., after a `.is_some()` check).

```rust
// Good:
pub fn load_bills(dir: &Path) -> Result<Vec<LoadedBill>> { ... }

// Bad:
pub fn load_bills(dir: &Path) -> Vec<LoadedBill> {
    // panics on error — caller can't handle it gracefully
}
```

### Panicking is fine in CLI handlers

CLI handlers (the `handle_*` functions in `main.rs`) can use `?` freely since errors propagate to `main()` and are displayed to the user. The `anyhow` crate formats the error chain nicely.

## Documentation

### Doc comments on public items

Every public function, type, and module should have a `///` doc comment:

```rust
/// Compute (total_budget_authority, total_rescissions) from the actual provisions.
///
/// This is deterministic — does not use the LLM's self-reported summary.
/// Budget authority includes all `Appropriation` provisions where
/// `semantics == NewBudgetAuthority` and `detail_level` is not
/// `sub_allocation` or `proviso_amount`.
pub fn compute_totals(&self) -> (i64, i64) {
    // ...
}
```

### Module-level documentation

Each module should have a `//!` doc comment at the top explaining its purpose:

```rust
//! Query operations over loaded bill data.
//!
//! These functions take `&[LoadedBill]` and return plain data structs
//! suitable for any output format. The CLI layer handles formatting.
```

### Inline comments

Use `//` comments sparingly — prefer self-documenting code (descriptive names, small functions). When you do comment, explain *why*, not *what*:

```rust
// Good: Explains why
// Exclude sub-allocations and proviso amounts — they are
// breakdowns of a parent account, not additional money.
if dl != "sub_allocation" && dl != "proviso_amount" {
    ba += amt.dollars().unwrap_or(0);
}

// Bad: Restates the code
// Add dollars to ba if detail level is not sub_allocation or proviso_amount
if dl != "sub_allocation" && dl != "proviso_amount" {
    ba += amt.dollars().unwrap_or(0);
}
```

## Serde Conventions

### Use `#[serde(default)]` on all provision fields

```rust
Appropriation {
    #[serde(default)]
    account_name: String,
    #[serde(default)]
    agency: Option<String>,
    // ...
}
```

This ensures that missing fields in JSON input (which is common with LLM-generated JSON) get default values rather than causing deserialization errors.

### Use `#[serde(tag = "...", rename_all = "snake_case")]` for tagged enums

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provision_type", rename_all = "snake_case")]
pub enum Provision {
    Appropriation { ... },
    Rescission { ... },
    // ...
}
```

### Use `#[non_exhaustive]` on enums that may grow

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AmountValue {
    Specific { dollars: i64 },
    SuchSums,
    None,
}
```

This prevents external code from exhaustively matching, ensuring forward compatibility when new variants are added.

## Async Conventions

### Only use async when calling external APIs

Most of the codebase is synchronous. Only these operations are async:

- `handle_extract()` — calls the Anthropic API
- `handle_embed()` — calls the OpenAI API
- `handle_search()` — the `--semantic` path calls the OpenAI API
- `handle_download()` — calls the Congress.gov API

If your new code doesn't call an external API, keep it synchronous.

### Never use `block_on()` inside an async function

```rust
// WRONG — causes "cannot start a runtime from within a runtime" panic
async fn handle_my_command() {
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(some_async_fn()); // PANIC!
}

// RIGHT — use .await
async fn handle_my_command() {
    let result = some_async_fn().await;
}
```

### The main function is async

The `main()` function uses `#[tokio::main]` and dispatches to handler functions. Async handlers are `.await`ed; sync handlers are called directly.

## Commit Messages

Use this format:

```text
Short summary of the change (imperative mood, ≤72 characters)

Longer description of what changed and why. Wrap at 72 characters.
Explain the motivation, not just the mechanics.

Verified:
- cargo fmt/clippy/test: clean, N tests pass
- Budget totals unchanged: $846B/$16B/$2.9B
```

Examples:

```text
Add --division filter to search command

Scopes search results to a single division letter (e.g., --division A
for MilCon-VA in the FY2024 omnibus). Uses case-insensitive exact
match against the provision's division field.

Verified:
- cargo fmt/clippy/test: clean, 172 tests pass
- Budget totals unchanged: $846B/$16B/$2.9B
```

```text
Fix SuchSums serialization in upgrade path

The upgrade command was not correctly re-serializing SuchSums amount
variants — they were missing the "kind" tag. Fixed by normalizing
through the current AmountValue enum during upgrade.

Verified:
- cargo fmt/clippy/test: clean, 95 tests pass
- Budget totals unchanged: $846B/$16B/$2.9B
```

### Verification line

Always include the verification line in your commit message. It tells reviewers that you ran the full test cycle and checked data integrity. The budget total shorthand ($846B/$16B/$2.9B) refers to the three example bills' budget authority.

## Dependencies

### Adding new dependencies

Before adding a new crate dependency:

1. **Check if an existing dependency can do the job.** The project already uses `reqwest`, `serde`, `serde_json`, `tokio`, `anyhow`, `thiserror`, `sha2`, `chrono`, `walkdir`, `comfy-table`, and `csv`.
2. **Prefer pure-Rust crates.** The project avoids C dependencies (uses `roxmltree` instead of `libxml2`, `rustls-tls` instead of OpenSSL).
3. **Check the crate's maintenance status.** Prefer well-maintained crates with recent releases.
4. **Keep the dependency count low.** Each new dependency is a maintenance burden and a potential supply-chain risk.

### Feature flags

Use feature flags to keep optional dependencies from bloating the binary:

```toml
# In Cargo.toml:
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "stream"] }
```

## Logging

### Use `tracing` for structured logging

```rust
use tracing::{debug, info, warn};

debug!(bill = %loaded.extraction.bill.identifier, "Loaded bill");
info!(chunks = chunks.len(), "Starting parallel extraction");
warn!(bill = %identifier, "Embeddings are stale");
```

### Log levels

| Level | When to Use |
|-------|------------|
| `error!` | Something failed and the operation can't continue |
| `warn!` | Something unexpected happened but the operation continues (e.g., stale embeddings) |
| `info!` | High-level progress updates (e.g., "Loaded 3 bills", "Extraction complete") |
| `debug!` | Detailed progress for debugging (e.g., per-provision details, timing) |
| `trace!` | Very detailed internal state (rarely used) |

Users see `info!` and above by default. The `-v` flag enables `debug!` level.

### Never log to stdout

All logging goes to stderr via `tracing-subscriber`. Stdout is reserved for command output (tables, JSON, CSV) so it can be piped and redirected cleanly.

## Summary

| Rule | Why |
|------|-----|
| `cargo fmt` before every commit | CI rejects unformatted code |
| `cargo clippy -- -D warnings` before every commit | CI rejects code with warnings |
| Fix clippy at root cause, not with `#[allow]` | Suppressing warnings hides real issues |
| Library function first, CLI second | Separates computation from presentation |
| All query functions take `&[LoadedBill]` | Keeps library functions pure and testable |
| Serde on everything | Enables all output formats for free |
| Tests in the same file | Easy to find, easy to maintain |
| `anyhow` for CLI, `thiserror` for library | Right error handling tool for each context |
| Never `block_on()` in async | Causes runtime panics |
| Include verification line in commits | Proves you ran the full test cycle |

## Writing and Documentation Tone

All documentation, comments, commit messages, and user-facing text should be **direct, factual, and professional**. The project's credibility depends on the data and the verification methodology — not on persuasive language.

### Do

- State what the tool does and how: *"Dollar amounts are verified by deterministic string matching against the enrolled bill text."*
- Let the data speak: *"99.995% of dollar amounts confirmed in source text (18,583 of 18,584)."*
- Describe limitations plainly: *"FY2025 subcommittee filtering is not available because H.R. 1968 wraps all jurisdictions into a single division."*
- Use precise language: *"budget authority"* not *"spending"*; *"enrolled bill"* not *"the law"*.

### Do not

- Use marketing language: ~~"Turn federal spending bills into searchable, structured data."~~
- Use breathless phrasing: ~~"Copy-paste and go!"~~, ~~"Zero keyword overlap — yet it's the top result!"~~
- Label features by audience: ~~"For Journalists"~~, ~~"For Staffers"~~. Describe the task instead.
- Use callout labels like "Trust callout" or "Key insight" — if the information is important, state it directly.
- Editorialize about what numbers mean: ~~"That's a story-saving feature."~~ Describe the data; let the reader draw conclusions.

### README and book chapter guidelines

- The README and book chapters should read like technical documentation, not a product landing page.
- Embed specific numbers only in the cookbook dataset card and the accuracy-metrics appendix. Other pages should use relative language (*"across the full dataset"*) and link to those reference pages. This prevents staleness when bills are added.
- Every command example should use output that was verified against the actual dataset. Do not fabricate or approximate CLI output.

## Next Steps

- **[Testing Strategy](./testing.md)** — how to write and run tests
- **[Adding a New Provision Type](./new-provision-type.md)** — the most common contributor task
- **[Adding a New CLI Command](./new-command.md)** — the full process for new subcommands
- **[Code Map](./code-map.md)** — where every file lives