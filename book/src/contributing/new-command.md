# Adding a New CLI Command

This guide walks through the process of adding a new subcommand to `congress-approp`. The pattern is consistent: define the command in clap, write a library function, create a CLI handler, and add tests.

## Overview

Every CLI command follows the same architecture:

```text
1. Define command + flags     →  main.rs (Commands enum, clap derive)
2. Write library function     →  query.rs or new module (pure function, no I/O)
3. Write CLI handler          →  main.rs (parse args → call library → format output)
4. Wire into main()           →  main.rs (match arm in the main dispatch)
5. Add integration test       →  tests/cli_tests.rs
6. Update documentation       →  book/src/reference/cli.md + relevant chapters
```

The key principle: **library function first, CLI second.** The library function does the computation; the CLI handler does the I/O and formatting.

## Step 1: Define the Command (main.rs)

Add a new variant to the `Commands` enum with clap derive attributes:

```rust
// In the Commands enum in main.rs:

/// Show the top N provisions by dollar amount
Top {
    /// Data directory containing extracted bills
    #[arg(long, default_value = "./data")]
    dir: String,

    /// Number of provisions to show
    #[arg(long, short = 'n', default_value = "10")]
    count: usize,

    /// Filter by provision type
    #[arg(long, short = 't')]
    r#type: Option<String>,

    /// Output format: table, json, jsonl, csv
    #[arg(long, default_value = "table")]
    format: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
},
```

### Conventions for flags

| Pattern | Convention |
|---------|-----------|
| Data directory | `--dir` with default `"./data"` |
| Output format | `--format` with default `"table"`, options: `table`, `json`, `jsonl`, `csv` |
| Provision type filter | `--type` / `-t` (use `r#type` for the Rust keyword) |
| Agency filter | `--agency` / `-a` |
| Dry run | `--dry-run` flag |
| Verbose | `-v` / `--verbose` (also available as global flag) |

Look at existing commands for consistent naming and help text style.

## Step 2: Write the Library Function (query.rs)

Add a pure function to `src/approp/query.rs` that takes `&[LoadedBill]` and returns a data struct:

```rust
// In src/approp/query.rs:

/// A provision ranked by dollar amount.
#[derive(Debug, Serialize)]
pub struct TopProvision {
    pub bill_identifier: String,
    pub provision_index: usize,
    pub provision_type: String,
    pub account_name: String,
    pub agency: String,
    pub dollars: i64,
    pub semantics: String,
    pub section: String,
    pub division: String,
}

/// Return the top N provisions by absolute dollar amount.
pub fn top_provisions(
    bills: &[LoadedBill],
    count: usize,
    provision_type: Option<&str>,
) -> Vec<TopProvision> {
    let mut results: Vec<TopProvision> = Vec::new();

    for loaded in bills {
        let bill_id = &loaded.extraction.bill.identifier;

        for (i, p) in loaded.extraction.provisions.iter().enumerate() {
            // Apply type filter
            if let Some(ptype) = provision_type {
                if p.provision_type_str() != ptype {
                    continue;
                }
            }

            // Only include provisions with specific dollar amounts
            if let Some(amt) = p.amount() {
                if let Some(dollars) = amt.dollars() {
                    results.push(TopProvision {
                        bill_identifier: bill_id.clone(),
                        provision_index: i,
                        provision_type: p.provision_type_str().to_string(),
                        account_name: p.account_name().to_string(),
                        agency: p.agency().to_string(),
                        dollars,
                        semantics: format!("{}", amt.semantics),
                        section: p.section().to_string(),
                        division: p.division().unwrap_or("").to_string(),
                    });
                }
            }
        }
    }

    // Sort by absolute dollar amount descending
    results.sort_by(|a, b| b.dollars.abs().cmp(&a.dollars.abs()));
    results.truncate(count);
    results
}
```

### Library function conventions

- **Take `&[LoadedBill]`** — never a file path. I/O is the CLI's job.
- **Return a struct that derives `Serialize`** — enables JSON/JSONL/CSV output for free.
- **No formatting, no printing, no side effects.**
- **Document with doc comments** (`///`) — these appear in `cargo doc` output.

## Step 3: Write the CLI Handler (main.rs)

Create a handler function in `main.rs` that bridges the CLI arguments to the library function and formats the output:

```rust
fn handle_top(dir: &str, count: usize, ptype: Option<&str>, format: &str) -> Result<()> {
    let start = Instant::now();
    let bills = loading::load_bills(Path::new(dir))?;

    if bills.is_empty() {
        println!("No extracted bills found in {dir}");
        return Ok(());
    }

    let results = query::top_provisions(&bills, count, ptype);

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        "jsonl" => {
            for r in &results {
                println!("{}", serde_json::to_string(r)?);
            }
        }
        "csv" => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            for r in &results {
                wtr.serialize(r)?;
            }
            wtr.flush()?;
        }
        _ => {
            // Table output
            let mut table = Table::new();
            table.load_preset(UTF8_FULL_CONDENSED);
            table.set_header(vec![
                Cell::new("Bill"),
                Cell::new("Type"),
                Cell::new("Account"),
                Cell::new("Amount ($)").set_alignment(CellAlignment::Right),
                Cell::new("Section"),
                Cell::new("Div"),
            ]);

            for r in &results {
                table.add_row(vec![
                    Cell::new(&r.bill_identifier),
                    Cell::new(&r.provision_type),
                    Cell::new(truncate(&r.account_name, 45)),
                    Cell::new(format_dollars(r.dollars))
                        .set_alignment(CellAlignment::Right),
                    Cell::new(&r.section),
                    Cell::new(&r.division),
                ]);
            }

            println!("{table}");
            println!("\n{} provisions shown", results.len());
        }
    }

    tracing::debug!("Completed in {:?}", start.elapsed());
    Ok(())
}
```

### Handler conventions

- **Name:** `handle_<command>` (e.g., `handle_top`)
- **Signature:** Takes parsed arguments as simple types, returns `Result<()>`
- **Pattern:** Load bills → call library function → format output based on `--format` flag
- **Table formatting:** Use `comfy_table` with `UTF8_FULL_CONDENSED` preset (matching existing commands)
- **Timing:** Use `Instant::now()` + `tracing::debug!` for elapsed time (visible with `-v`)
- **Empty results:** Handle gracefully with a message, don't panic

### Async or sync?

- If your handler makes **no API calls**, make it a regular `fn` (sync).
- If it needs to call an external API (like `handle_embed` or `handle_semantic_search`), make it `async fn` and `.await` the API calls.

**Important:** Don't use `block_on()` inside an async function — this causes "cannot start a runtime from within a runtime" panics. If your handler is async, the entire call chain from `main()` must use `.await`.

## Step 4: Wire into main() Dispatch

In the `main()` function, add a match arm for your new command:

```rust
// In the main() function's match on cli.command:

Commands::Top {
    dir,
    count,
    r#type,
    format,
    verbose: _,
} => {
    handle_top(&dir, count, r#type.as_deref(), &format)?;
}
```

For async handlers:

```rust
Commands::Top { dir, count, r#type, format, verbose: _ } => {
    handle_top(&dir, count, r#type.as_deref(), &format).await?;
}
```

## Step 5: Add Integration Tests (cli_tests.rs)

Add tests in `tests/cli_tests.rs` that run the actual binary against the example data:

```rust
// In tests/cli_tests.rs:

#[test]
fn top_runs_successfully() {
    cmd()
        .args(["top", "--dir", "data", "-n", "5"])
        .assert()
        .success()
        .stdout(predicates::str::contains("H.R. 4366"));
}

#[test]
fn top_json_output_is_valid() {
    let output = cmd()
        .args(["top", "--dir", "data", "-n", "3", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();
    assert_eq!(data.len(), 3);

    // Verify the top result has the largest dollar amount
    let first_dollars = data[0]["dollars"].as_i64().unwrap();
    let second_dollars = data[1]["dollars"].as_i64().unwrap();
    assert!(first_dollars.abs() >= second_dollars.abs());
}

#[test]
fn top_with_type_filter() {
    let output = cmd()
        .args(["top", "--dir", "data", "-n", "5", "--type", "rescission", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    for entry in &data {
        assert_eq!(entry["provision_type"].as_str().unwrap(), "rescission");
    }
}
```

### Test conventions

- Use the `cmd()` helper function (defined at the top of `cli_tests.rs`) to get a `Command` for the binary
- Test with `--dir data` to use the included example data
- Test all output formats (`table`, `json`, `csv`)
- Test filter combinations
- Verify JSON output parses correctly
- **Never change the expected budget authority totals** — the `budget_authority_totals_match_expected` test is a critical regression guard

## Step 6: Update Documentation

### CLI Reference (book/src/reference/cli.md)

Add a section for your new command following the existing format:

```markdown
## top

Show the top N provisions by dollar amount.

\`\`\`text
congress-approp top [OPTIONS]
\`\`\`

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--dir` | | path | `./data` | Data directory |
| `--count` | `-n` | integer | `10` | Number of provisions to show |
| `--type` | `-t` | string | — | Filter by provision type |
| `--format` | | string | `table` | Output format: table, json, jsonl, csv |

### Examples

\`\`\`bash
congress-approp top --dir data -n 5
congress-approp top --dir data -n 10 --type rescission
congress-approp top --dir data -n 20 --format csv > top_provisions.csv
\`\`\`
```

### Other documentation

- Update the **SUMMARY.md** table of contents if the command deserves its own how-to guide
- Add a mention in **what-this-tool-does.md** if the command represents a significant new capability
- Update the **CHANGELOG.md** with the new feature

## Complete Test Cycle

Before committing, run the full test cycle:

```bash
cargo fmt                           # Format code
cargo fmt --check                   # Verify formatting (CI does this)
cargo clippy -- -D warnings         # Lint (CI treats warnings as errors)
cargo test                          # Run all tests

# Data integrity check (budget totals must be unchanged):
./target/release/congress-approp summary --dir data --format json | python3 -c "
import sys, json
expected = {'H.R. 4366': 846137099554, 'H.R. 5860': 16000000000, 'H.R. 9468': 2882482000}
for b in json.load(sys.stdin):
    assert b['budget_authority'] == expected[b['identifier']]
print('Data integrity: OK')
"
```

All must pass. The CI runs `fmt --check`, `clippy -D warnings`, and `cargo test` on every push.

## Commit Message Format

```text
Add `top` command — show provisions ranked by dollar amount

Adds a new CLI subcommand that ranks provisions by absolute dollar
amount across all loaded bills. Supports --type filter and all
output formats (table/json/jsonl/csv).

Library function: query::top_provisions()
CLI handler: handle_top()

Verified:
- cargo fmt/clippy/test: clean, 98 tests pass (77 unit + 21 integration)
- Budget totals unchanged: $846B/$16B/$2.9B
```

## Gotchas

1. **`handle_search` is async** because the `--semantic` path calls OpenAI. If your new command doesn't call any APIs, keep it sync — don't make it async just because other handlers are.

2. **The `format_dollars` and `truncate` helper functions** are in `main.rs` (not in a shared module). You can use them directly in your handler.

3. **Provision accessor methods return `&str`, not `Option<&str>`** in some cases. `p.account_name()` returns `""` (not `None`) for provisions without accounts. Check with `.is_empty()` if you need to handle the empty case.

4. **The `r#type` naming** is required because `type` is a Rust keyword. Use `r#type` in the struct definition and `r#type.as_deref()` when passing to functions that expect `Option<&str>`.

5. **CSV output uses `serde_json::to_string(r)?` for each row** in some handlers, but the cleaner approach is `csv::Writer::from_writer` with `wtr.serialize(r)?` as shown above. Make sure your output struct derives `Serialize`.

6. **Run `cargo install --path .`** after making changes to test the actual installed binary (integration tests use the debug binary from `cargo test`, not the installed release binary).

## Example: Reviewing Existing Commands

The best way to learn the patterns is to read existing handlers. Start with these as templates:

| If your command is like... | Study this handler |
|---------------------------|--------------------|
| Read-only query, no API calls | `handle_summary()` (~160 lines, sync) |
| Query with filters | `handle_search()` (~530 lines, async because of semantic path) |
| Two-directory comparison | `handle_compare()` (~210 lines, sync) |
| API-calling command | `handle_embed()` (~120 lines, async) |
| Schema migration command | `handle_upgrade()` (~150 lines, sync) |

## Next Steps

- **[Code Map](./code-map.md)** — where every file lives and what it does
- **[Testing Strategy](./testing.md)** — how the test suite is structured
- **[Style Guide and Conventions](./style-guide.md)** — coding standards
- **[Adding a New Provision Type](./new-provision-type.md)** — the other common contributor task