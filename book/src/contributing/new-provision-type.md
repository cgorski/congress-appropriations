# Adding a New Provision Type

This guide walks through the complete process of adding a new provision type to the extraction schema. It's the most common contributor task and touches seven files across the codebase. We'll use a hypothetical `authorization_extension` type as a worked example.

## When You Need This

Add a new provision type when the existing 11 types don't adequately capture a recurring legislative pattern. Signs that a new type is warranted:

- **Multiple `other` provisions share a pattern.** If you see 20+ provisions in the `other` catch-all with similar `llm_classification` values, they probably deserve their own type.
- **The pattern has distinct fields.** A new type should have at least one field that doesn't exist on any current type. If it can be fully represented by an existing type's fields, consider improving the LLM prompt to classify it correctly instead of adding a new type.
- **The pattern recurs across bills.** A one-off provision in a single bill doesn't justify a new type. A pattern that appears in every omnibus does.

## The Checklist (7 Files)

Every new provision type requires changes in these files, in this order:

| Step | File | What to Add |
|------|------|------------|
| 1 | `src/approp/ontology.rs` | New variant on the `Provision` enum with type-specific fields |
| 2 | `src/approp/ontology.rs` | Accessor method arms for the new variant (raw_text, section, etc.) |
| 3 | `src/approp/from_value.rs` | Match arm in `parse_provision()` for the new type |
| 4 | `src/approp/prompts.rs` | Type definition and example in the LLM system prompt |
| 5 | `src/main.rs` | Table rendering for the new type; add to `KNOWN_PROVISION_TYPES` |
| 6 | `src/approp/query.rs` | Update search/summary logic if the type has special display needs |
| 7 | `tests/cli_tests.rs` | Integration test for the new type |

## Step 1: Add the Enum Variant (ontology.rs)

Add a new variant to the `Provision` enum. Every variant must include the common fields (`section`, `division`, `title`, `confidence`, `raw_text`, `notes`, `cross_references`) plus its type-specific fields.

```rust
// In src/approp/ontology.rs, inside the Provision enum:

AuthorizationExtension {
    /// The program being reauthorized
    #[serde(default)]
    program_name: String,
    /// The statute being extended
    #[serde(default)]
    statutory_reference: String,
    /// New authorization level, if specified
    #[serde(default)]
    amount: Option<DollarAmount>,
    /// How long the authorization is extended
    #[serde(default)]
    extension_period: Option<String>,
    /// New expiration date or fiscal year
    #[serde(default)]
    expires: Option<String>,
    // Common fields (must be on every variant):
    #[serde(default)]
    section: String,
    #[serde(default)]
    division: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    confidence: f32,
    #[serde(default)]
    raw_text: String,
    #[serde(default)]
    notes: Vec<String>,
    #[serde(default)]
    cross_references: Vec<CrossReference>,
},
```

### Important conventions

- **Use `#[serde(default)]`** on every field. This ensures that missing fields in JSON input get their default values rather than causing a deserialization error.
- **Use `Option<T>`** for fields that may not always be present.
- **Use `String`** (not `&str`) for owned text fields.
- **Include all 7 common fields.** The accessor methods expect them on every variant.

## Step 2: Add Accessor Method Arms (ontology.rs)

Every accessor method on `Provision` exhaustively matches all variants. You must add a match arm for your new variant to each one. The compiler will tell you which methods are missing — look for "non-exhaustive patterns" errors.

Key methods that need arms:

```rust
// raw_text() — returns &str
Provision::AuthorizationExtension { raw_text, .. } => raw_text,

// section() — returns &str
Provision::AuthorizationExtension { section, .. } => section,

// division() — returns Option<&str>
Provision::AuthorizationExtension { division, .. } => division,

// title() — returns Option<&str>
Provision::AuthorizationExtension { title, .. } => title,

// confidence() — returns f32
Provision::AuthorizationExtension { confidence, .. } => *confidence,

// notes() — returns &[String]
Provision::AuthorizationExtension { notes, .. } => notes,

// cross_references() — returns &[CrossReference]
Provision::AuthorizationExtension { cross_references, .. } => cross_references,

// account_name() — returns &str
// If your type has an account_name field, return it. Otherwise return "".
Provision::AuthorizationExtension { .. } => "",

// agency() — returns &str
// Same pattern — return "" if not applicable.
Provision::AuthorizationExtension { .. } => "",

// amount() — returns Option<&DollarAmount>
Provision::AuthorizationExtension { amount, .. } => amount.as_ref(),

// description() — return a meaningful description
Provision::AuthorizationExtension { program_name, .. } => program_name,

// provision_type_str() — returns &str
Provision::AuthorizationExtension { .. } => "authorization_extension",
```

### Tip: Let the compiler guide you

After adding the variant, run `cargo build`. The compiler will emit errors for every `match` expression that doesn't cover the new variant. Fix them one by one — this is faster and more reliable than trying to find all match sites manually.

## Step 3: Add Parsing Logic (from_value.rs)

In `from_value.rs`, the `parse_provision()` function has a `match provision_type.as_str()` block that dispatches to type-specific parsing. Add a new arm:

```rust
"authorization_extension" => Ok(Provision::AuthorizationExtension {
    program_name: get_str_or_warn(obj, "program_name", report),
    statutory_reference: get_str_or_warn(obj, "statutory_reference", report),
    amount: parse_dollar_amount(obj.get("amount"), report),
    extension_period: get_opt_str(obj, "extension_period"),
    expires: get_opt_str(obj, "expires"),
    section,
    division,
    title,
    confidence,
    raw_text,
    notes,
    cross_references,
}),
```

### Parsing conventions

- Use `get_str(obj, "field")` for required string fields that default to empty string if missing
- Use `get_str_or_warn(obj, "field", report)` for string fields where absence should be logged
- Use `get_opt_str(obj, "field")` for optional string fields (returns `Option<String>`)
- Use `get_opt_u32(obj, "field")` for optional integers
- Use `parse_dollar_amount(obj.get("amount"), report)` for dollar amount fields
- Use `get_string_array(obj, "field")` for arrays of strings

**The existing `unknown =>` arm (at the bottom of the match) will catch any provision the LLM outputs with your new type name before you add this arm.** It wraps them as `Provision::Other` with the original classification preserved. This means historical extractions that already contain your new type (classified as `other`) will still load correctly. After upgrading, they'll be parsed into the proper new variant.

## Step 4: Update the System Prompt (prompts.rs)

In `prompts.rs`, the `EXTRACTION_SYSTEM` constant contains the instructions for Claude. Add your new type to the `PROVISION TYPES` section:

```text
- authorization_extension: Extension or reauthorization of an existing program's authorization
  - MUST have program_name (the program being reauthorized)
  - MUST have statutory_reference (the statute being amended)
  - May have an amount (new authorization level) and extension_period
```

Also add a JSON example in the examples section of the prompt:

```json
{
  "provision_type": "authorization_extension",
  "program_name": "Community Health Centers",
  "statutory_reference": "Section 330 of the Public Health Service Act (42 U.S.C. 254b)",
  "amount": {
    "value": {"kind": "specific", "dollars": 4000000000},
    "semantics": "mandatory_spending",
    "text_as_written": "$4,000,000,000"
  },
  "extension_period": "2 years",
  "expires": "September 30, 2026",
  "section": "SEC. 201",
  "division": "B",
  "confidence": 0.95,
  "raw_text": "Section 330(r)(1) of the Public Health Service Act is amended by striking '2024' and inserting '2026'."
}
```

> **Caution:** Changing the system prompt invalidates all existing extractions. Bills extracted with the old prompt won't have provisions classified under the new type — they'll be in the `other` catch-all or classified as something else. You'll need to re-extract any bills where you want the new type to be used. The `upgrade` command can re-parse existing data but cannot re-classify provisions — that requires re-extraction.

## Step 5: Update CLI Display (main.rs)

### Add to KNOWN_PROVISION_TYPES

In `main.rs`, find the `KNOWN_PROVISION_TYPES` constant (around line 943) and add your new type:

```rust
const KNOWN_PROVISION_TYPES: &[(&str, &str)] = &[
    ("appropriation", "Budget authority grant"),
    ("rescission", "Cancellation of prior budget authority"),
    // ... existing types ...
    ("authorization_extension", "Extension of program authorization"),
    ("other", "Unclassified provisions"),
];
```

This makes the new type appear in `--list-types` output.

### Update table rendering

If your type needs special table columns (like CR substitutions show New/Old/Delta), add the rendering logic in the `handle_search` function. If it uses the standard display (Description/Account, Amount, Section, Div), no changes are needed — the default rendering handles it.

### Update the Match struct

In the `Match` struct within `handle_search`, ensure the new type's fields are mapped correctly to the output fields (`account_name`, `description`, `dollars`, etc.).

## Step 6: Update Query Logic (query.rs)

If your new type:

- **Should contribute to budget authority totals** — update `BillExtraction::compute_totals()` in `ontology.rs`
- **Has special search display needs** — update `search()` in `query.rs` to include the type in relevant filters
- **Should appear in comparisons** — update `compare()` in `query.rs` if the type should be matched across bills

For most new types, no changes to `query.rs` are needed — the existing search filter (`--type authorization_extension`) will work automatically because the filter matches against `provision_type_str()`.

## Step 7: Add Tests

### Unit test (ontology.rs)

Add a test in the `#[cfg(test)] mod tests` block at the bottom of `ontology.rs`:

```rust
#[test]
fn authorization_extension_round_trip() {
    let json = r#"{
        "provision_type": "authorization_extension",
        "program_name": "Test Program",
        "statutory_reference": "Section 100 of Test Act",
        "section": "SEC. 201",
        "confidence": 0.95,
        "raw_text": "Test raw text"
    }"#;

    let prov: Provision = serde_json::from_str(json).unwrap();
    assert_eq!(prov.provision_type_str(), "authorization_extension");
    assert_eq!(prov.section(), "SEC. 201");
    assert_eq!(prov.raw_text(), "Test raw text");
}
```

### Integration test (cli_tests.rs)

If the example data contains provisions that would be classified under your new type, add a test. Otherwise, the existing tests should still pass — your changes shouldn't affect the example data's provision counts or budget totals.

**Critical:** Run the budget authority regression test:

```bash
cargo test budget_authority_totals_match_expected
```

If this fails, your changes inadvertently affected the budget authority calculation. The expected values are:

| Bill | Budget Authority | Rescissions |
|------|-----------------|-------------|
| H.R. 4366 | $846,137,099,554 | $24,659,349,709 |
| H.R. 5860 | $16,000,000,000 | $0 |
| H.R. 9468 | $2,882,482,000 | $0 |

## Testing Your Changes

Run the full test cycle:

```bash
cargo fmt                           # Format code
cargo fmt --check                   # Verify formatting
cargo clippy -- -D warnings         # Lint (CI treats warnings as errors)
cargo test                          # Run all tests (77 unit + 18 integration)
```

All four must pass before committing.

## Backward Compatibility

Adding a new provision type is **backward-compatible** by design:

- **Old data loads fine.** Provisions in existing `extraction.json` files that were classified as `other` (because the new type didn't exist yet) will continue to load as `other`. The `from_value.rs` `unknown =>` arm catches them.
- **The `upgrade` command helps.** After adding the new type, running `upgrade` re-deserializes existing data through the updated parsing logic. If any `other` provisions have `llm_classification` matching your new type name, they'll be re-parsed into the proper variant.
- **Re-extraction is optional.** Only needed if you want the LLM to actively use the new type (which requires the updated prompt).

## What NOT to Do

1. **Don't add a type for a single provision.** If only one provision in one bill would use the type, leave it as `other` — the catch-all exists for exactly this purpose.

2. **Don't duplicate existing types.** Before adding a new type, check whether the pattern is actually a variant of an existing type (e.g., a `limitation` with special characteristics, or an `appropriation` with a unique availability pattern).

3. **Don't add fields to existing types** unless you also handle missing fields in `from_value.rs`. Existing extractions won't have the new field, so `#[serde(default)]` is mandatory.

4. **Don't suppress clippy warnings with `#[allow]`.** Fix them at the root cause. The CI rejects code with clippy warnings.

## Summary Checklist

- [ ] Added variant to `Provision` enum in `ontology.rs` with all common fields
- [ ] Added match arms to all accessor methods in `ontology.rs`
- [ ] Added parsing arm in `parse_provision()` in `from_value.rs`
- [ ] Added type definition and example in `EXTRACTION_SYSTEM` prompt in `prompts.rs`
- [ ] Added to `KNOWN_PROVISION_TYPES` in `main.rs`
- [ ] Updated table rendering in `main.rs` if needed
- [ ] Updated `query.rs` if the type has special search/compare/summary behavior
- [ ] Added unit test for round-trip serialization
- [ ] Verified budget authority totals unchanged: `cargo test budget_authority_totals_match_expected`
- [ ] Full test cycle passes: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`

## Next Steps

- **[Adding a New CLI Command](./new-command.md)** — if your new type needs a dedicated command
- **[Testing Strategy](./testing.md)** — how the test suite is structured
- **[Architecture Overview](./architecture.md)** — understanding the full codebase