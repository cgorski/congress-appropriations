# Use the Library API from Rust

> **You will need:** A Rust project with `congress-appropriations` as a dependency.
>
> **You will learn:** How to load extracted bill data, query it programmatically using the library API, and build custom analysis tools on top of the structured provision data.

`congress-appropriations` is both a CLI tool and a Rust library. The library exposes the same query functions the CLI uses — `summarize`, `search`, `compare`, `audit`, `rollup_by_department`, and `build_embedding_text` — as pure functions that take loaded bill data and return plain data structs. No I/O, no formatting, no side effects.

This guide shows you how to use the library in your own Rust projects.

## Add the Dependency

Add `congress-appropriations` to your `Cargo.toml`:

```toml
[dependencies]
congress-appropriations = "3.0"
```

The crate re-exports the key types you need:

```rust
use congress_appropriations::{load_bills, query, LoadedBill};
use congress_appropriations::approp::query::SearchFilter;
```

## Load Bills

The entry point is `load_bills()`, which recursively walks a directory to find all `extraction.json` files and loads them along with their sibling verification and metadata files:

```rust
use congress_appropriations::load_bills;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let bills = load_bills(Path::new("examples"))?;
    println!("Loaded {} bills", bills.len());

    for bill in &bills {
        println!(
            "  {} ({}) — {} provisions",
            bill.extraction.bill.identifier,
            bill.extraction.bill.classification,
            bill.extraction.provisions.len()
        );
    }

    Ok(())
}
```

Expected output with the included example data:

```text
Loaded 3 bills
  H.R. 4366 (Omnibus) — 2364 provisions
  H.R. 5860 (Continuing Resolution) — 130 provisions
  H.R. 9468 (Supplemental) — 7 provisions
```

### What `LoadedBill` Contains

Each `LoadedBill` has three fields:

```rust
pub struct LoadedBill {
    /// Path to the bill directory on disk
    pub dir: PathBuf,
    /// The extraction output: bill info, provisions array, summary
    pub extraction: BillExtraction,
    /// Verification report (if verification.json exists)
    pub verification: Option<VerificationReport>,
    /// Extraction metadata (if metadata.json exists)
    pub metadata: Option<ExtractionMetadata>,
}
```

Only `extraction` is required — `verification` and `metadata` are loaded if their files exist but are `None` otherwise. This means you can use the library on data that was only partially extracted.

## Summarize Bills

The `summarize` function computes per-bill budget authority, rescissions, and net BA:

```rust
use congress_appropriations::{load_bills, query};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let bills = load_bills(Path::new("examples"))?;
    let summaries = query::summarize(&bills);

    for s in &summaries {
        println!(
            "{}: ${:>15} BA, ${:>13} rescissions, ${:>15} net",
            s.identifier,
            format_dollars(s.budget_authority),
            format_dollars(s.rescissions),
            format_dollars(s.net_ba),
        );
    }

    Ok(())
}

fn format_dollars(n: i64) -> String {
    // Simple comma formatting for display
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 && c != '-' {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
```

### `BillSummary` Fields

```rust
pub struct BillSummary {
    pub identifier: String,      // e.g., "H.R. 4366"
    pub classification: String,  // e.g., "Omnibus"
    pub provisions: usize,       // total provision count
    pub budget_authority: i64,   // sum of new_budget_authority provisions
    pub rescissions: i64,        // sum of rescission provisions (absolute)
    pub net_ba: i64,             // budget_authority - rescissions
    pub completeness_pct: Option<f64>, // from verification, if available
}
```

Budget authority is computed from the actual provisions — it sums all `Appropriation` provisions where `semantics == NewBudgetAuthority` and `detail_level` is not `sub_allocation` or `proviso_amount`. The LLM's self-reported totals are never used.

## Search Provisions

The `search` function takes a `SearchFilter` and returns matching provisions:

```rust
use congress_appropriations::approp::query::{SearchFilter, SearchResult};

let results = query::search(&bills, &SearchFilter {
    provision_type: Some("appropriation"),
    agency: Some("Veterans"),
    min_dollars: Some(1_000_000_000),
    ..Default::default()
});

for r in &results {
    println!(
        "[{}] {} — ${:?}",
        r.bill_identifier, r.account_name, r.dollars
    );
}
```

### `SearchFilter` Fields

All fields are optional and use AND logic — every field that is `Some` must match:

```rust
pub struct SearchFilter<'a> {
    pub provision_type: Option<&'a str>,  // e.g., "appropriation"
    pub agency: Option<&'a str>,          // case-insensitive substring
    pub account: Option<&'a str>,         // case-insensitive substring
    pub keyword: Option<&'a str>,         // search in raw_text
    pub bill: Option<&'a str>,            // exact bill identifier
    pub division: Option<&'a str>,        // division letter, e.g., "A"
    pub min_dollars: Option<i64>,         // minimum absolute dollar amount
    pub max_dollars: Option<i64>,         // maximum absolute dollar amount
}
```

You can construct a filter with defaults for all fields and override just the ones you care about:

```rust
let filter = SearchFilter {
    provision_type: Some("rescission"),
    min_dollars: Some(100_000_000),
    ..Default::default()
};
```

## Compare Bills

The `compare` function computes account-level deltas between two sets of bills:

```rust
let base_bills = load_bills(Path::new("data/118-hr4366"))?;
let current_bills = load_bills(Path::new("data/118-hr9468"))?;

let deltas = query::compare(&base_bills, &current_bills, None);

for d in &deltas {
    println!(
        "{}: base=${}, current=${}, delta={} ({})",
        d.account_name,
        d.base_dollars,
        d.current_dollars,
        d.delta,
        d.status,
    );
}
```

The optional third parameter is an agency filter (`Option<&str>`) that restricts the comparison to accounts from a specific agency.

### `AccountDelta` Fields

```rust
pub struct AccountDelta {
    pub agency: String,
    pub account_name: String,
    pub base_dollars: i64,
    pub current_dollars: i64,
    pub delta: i64,
    pub delta_pct: f64,
    pub status: String,  // "changed", "unchanged", "only in base", "only in current"
}
```

Results are sorted by the absolute value of `delta`, largest changes first.

## Audit Bills

The `audit` function returns per-bill verification metrics:

```rust
let audit_rows = query::audit(&bills);

for row in &audit_rows {
    println!(
        "{}: {} provisions, {} verified, {} not found, {:.1}% coverage",
        row.identifier,
        row.provisions,
        row.verified,
        row.not_found,
        row.completeness_pct.unwrap_or(0.0),
    );
}
```

### `AuditRow` Fields

```rust
pub struct AuditRow {
    pub identifier: String,
    pub provisions: usize,
    pub verified: usize,       // dollar amounts found at unique position
    pub not_found: usize,      // dollar amounts NOT found in source
    pub ambiguous: usize,      // dollar amounts found at multiple positions
    pub exact: usize,          // raw_text byte-identical match
    pub normalized: usize,     // raw_text normalized match
    pub spaceless: usize,      // raw_text spaceless match
    pub no_match: usize,       // raw_text not found
    pub completeness_pct: Option<f64>,
}
```

The critical metric is `not_found` — it should be 0 for every bill. Across the included example data, it is.

## Roll Up by Department

The `rollup_by_department` function aggregates budget authority by parent department. This is a query-time computation — it never modifies stored data:

```rust
let agencies = query::rollup_by_department(&bills);

for a in &agencies {
    println!(
        "{}: ${} BA, ${} rescissions, {} provisions",
        a.department,
        a.budget_authority,
        a.rescissions,
        a.provision_count,
    );
}
```

Agency names are split at the first comma to extract the parent department (e.g., "Salaries and Expenses, Federal Bureau of Investigation" → "Federal Bureau of Investigation"). The exception is "Office of Inspector General, ..." which takes the text after the comma.

Results are sorted by budget authority descending.

## Build Embedding Text

The `build_embedding_text` function constructs the deterministic text representation used for embedding a provision. This is useful if you want to use your own embedding model instead of OpenAI:

```rust
use congress_appropriations::approp::ontology::Provision;

for provision in &bills[0].extraction.provisions[..3] {
    let text = query::build_embedding_text(provision);
    println!("Embedding text ({} chars): {}...",
        text.len(),
        &text[..text.len().min(100)]
    );
}
```

The text concatenates the provision's meaningful fields (account name, agency, program, raw text) in a consistent format. The same provision always produces the same text, regardless of when or where you call the function.

## Access Provision Fields Directly

The `Provision` enum has 11 variants. Accessor methods provide a uniform interface across all variants:

```rust
use congress_appropriations::approp::ontology::{Provision, AmountSemantics};

for bill in &bills {
    for p in &bill.extraction.provisions {
        // These methods work on all provision variants:
        let ptype = p.provision_type_str();   // e.g., "appropriation"
        let account = p.account_name();       // "" if not applicable
        let agency = p.agency();              // "" if not applicable
        let section = p.section();            // e.g., "SEC. 101"
        let division = p.division();          // Some("A") or None
        let raw_text = p.raw_text();          // bill text excerpt
        let confidence = p.confidence();      // 0.0-1.0

        // Amount access returns Option<&DollarAmount>
        if let Some(amt) = p.amount() {
            if matches!(amt.semantics, AmountSemantics::NewBudgetAuthority) {
                if let Some(dollars) = amt.dollars() {
                    println!("{}: ${}", account, dollars);
                }
            }
        }
    }
}
```

### Key accessor methods

| Method | Returns | Notes |
|--------|---------|-------|
| `provision_type_str()` | `&str` | e.g., `"appropriation"`, `"rescission"` |
| `account_name()` | `&str` | Empty string for types without accounts |
| `agency()` | `&str` | Empty string for types without agencies |
| `section()` | `&str` | e.g., `"SEC. 101"` or empty |
| `division()` | `Option<&str>` | `Some("A")` or `None` |
| `raw_text()` | `&str` | Bill text excerpt (~150 chars) |
| `confidence()` | `f32` | LLM self-assessed confidence, 0.0–1.0 |
| `amount()` | `Option<&DollarAmount>` | The primary dollar amount, if any |
| `description()` | `&str` | Description field, if applicable |

### Pattern matching for type-specific fields

When you need fields specific to a provision type, use pattern matching:

```rust
match p {
    Provision::Appropriation {
        account_name,
        agency,
        amount,
        detail_level,
        parent_account,
        fiscal_year,
        availability,
        ..
    } => {
        println!("Appropriation: {} (detail: {})", account_name, detail_level);
        if let Some(parent) = parent_account {
            println!("  Sub-allocation of: {}", parent);
        }
    }
    Provision::CrSubstitution {
        account_name,
        new_amount,
        old_amount,
        ..
    } => {
        let new_d = new_amount.dollars().unwrap_or(0);
        let old_d = old_amount.dollars().unwrap_or(0);
        println!("CR Sub: {} — ${} → ${} (delta: ${})",
            account_name.as_deref().unwrap_or("unnamed"),
            old_d, new_d, new_d - old_d);
    }
    Provision::Rescission {
        account_name,
        amount,
        reference_law,
        ..
    } => {
        println!("Rescission: {} — ${}", account_name, amount.dollars().unwrap_or(0));
        if let Some(law) = reference_law {
            println!("  From: {}", law);
        }
    }
    _ => {
        // Handle other provision types generically
        println!("{}: {}", p.provision_type_str(), p.description());
    }
}
```

## Compute Budget Authority Manually

The `BillExtraction` struct has a `compute_totals()` method that returns `(budget_authority, rescissions)`:

```rust
for bill in &bills {
    let (ba, rescissions) = bill.extraction.compute_totals();
    let net = ba - rescissions;
    println!("{}: BA=${}, Rescissions=${}, Net=${}",
        bill.extraction.bill.identifier, ba, rescissions, net);
}
```

This uses the same logic as the `summary` command: it sums `Appropriation` provisions where `semantics == NewBudgetAuthority` and `detail_level` is not `sub_allocation` or `proviso_amount`.

## Full Working Example

Here's a complete program that loads all example bills, finds the top 10 appropriations by dollar amount, and prints them:

```rust
use congress_appropriations::{load_bills, query};
use congress_appropriations::approp::query::SearchFilter;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Load all bills under data/
    let bills = load_bills(Path::new("examples"))?;
    println!("Loaded {} bills with {} total provisions\n",
        bills.len(),
        bills.iter().map(|b| b.extraction.provisions.len()).sum::<usize>()
    );

    // Search for all appropriations
    let results = query::search(&bills, &SearchFilter {
        provision_type: Some("appropriation"),
        ..Default::default()
    });

    // Sort by dollars descending, take top 10
    let mut with_dollars: Vec<_> = results.iter()
        .filter(|r| r.dollars.is_some())
        .collect();
    with_dollars.sort_by(|a, b| b.dollars.unwrap().abs().cmp(&a.dollars.unwrap().abs()));

    println!("Top 10 appropriations by dollar amount:");
    println!("{:<50} {:>20} {}", "Account", "Amount", "Bill");
    println!("{}", "-".repeat(85));

    for r in with_dollars.iter().take(10) {
        println!("{:<50} ${:>18} {}",
            &r.account_name[..r.account_name.len().min(48)],
            r.dollars.unwrap(),
            r.bill_identifier,
        );
    }

    // Budget summary
    println!("\nBudget Summary:");
    for s in query::summarize(&bills) {
        println!("  {}: ${} BA, ${} rescissions",
            s.identifier, s.budget_authority, s.rescissions);
    }

    Ok(())
}
```

## Design Principles

The library API follows these conventions:

1. **All query functions are pure.** They take `&[LoadedBill]` and return data. No side effects, no I/O, no API calls, no formatting.

2. **The CLI formats; the library computes.** `main.rs` handles table/JSON/CSV/JSONL rendering. The library returns structs that derive `Serialize` for easy JSON output.

3. **Semantic search is separate.** Embedding loading and cosine similarity live in `embeddings.rs`, not `query.rs`. This keeps the library usable without OpenAI. The CLI wires them together for `--semantic` and `--similar` searches.

4. **Error handling uses `anyhow`.** All fallible functions return `anyhow::Result<T>`. For library consumers who prefer typed errors, the underlying error types from `thiserror` are also available.

5. **Serde for everything.** All data types derive `Serialize` and `Deserialize`. You can serialize any query result to JSON with `serde_json::to_string(&results)?`.

## Working with Embeddings

The embeddings module is separate from the query module. If you want to work with embedding vectors directly:

```rust
use congress_appropriations::approp::embeddings;
use std::path::Path;

// Load embeddings for a bill
if let Some(loaded) = embeddings::load(Path::new("data/118-hr9468"))? {
    println!("Loaded {} vectors of {} dimensions",
        loaded.count(), loaded.dimensions());

    // Get the vector for provision 0
    let vec0 = loaded.vector(0);
    println!("First 5 dimensions: {:?}", &vec0[..5]);

    // Compute cosine similarity between two provisions
    let sim = embeddings::cosine_similarity(loaded.vector(0), loaded.vector(1));
    println!("Similarity between provisions 0 and 1: {:.4}", sim);
}
```

### Key embedding functions

| Function | Description |
|----------|-------------|
| `embeddings::load(dir)` | Load embeddings from a bill directory. Returns `Option<LoadedEmbeddings>`. |
| `embeddings::save(dir, metadata, vectors)` | Save embeddings to a bill directory. |
| `embeddings::cosine_similarity(a, b)` | Compute cosine similarity between two vectors. |
| `embeddings::normalize(vec)` | L2-normalize a vector in place. |
| `loaded.vector(i)` | Get the embedding vector for provision at index `i`. |
| `loaded.count()` | Number of provisions with embeddings. |
| `loaded.dimensions()` | Number of dimensions per vector (e.g., 3072). |

## Tips

1. **Load once, query many times.** `load_bills()` does all the file I/O. After that, all query functions work on in-memory data and are extremely fast.

2. **Use `SearchFilter::default()` as a base.** Override only the fields you need — all `None` fields are unrestricted.

3. **Check `provision_type_str()` instead of pattern matching** when you just need the type name as a string.

4. **The `amount()` accessor returns `None` for provisions without dollar amounts.** Riders, directives, and some other types don't carry amounts. Always handle the `None` case.

5. **Budget authority totals should match the CLI.** If `compute_totals()` returns different numbers than `congress-approp summary`, something is wrong. The included example data produces these exact totals: H.R. 4366 = $846,137,099,554 BA; H.R. 5860 = $16,000,000,000 BA; H.R. 9468 = $2,882,482,000 BA.

## Next Steps

- **[Architecture Overview](../contributing/architecture.md)** — understand how the crate is structured internally
- **[extraction.json Fields](../reference/extraction-json.md)** — complete field reference for the data structures
- **[Adding a New Provision Type](../contributing/new-provision-type.md)** — extend the library with new provision types