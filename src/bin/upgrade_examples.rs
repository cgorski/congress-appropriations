//! One-time script to upgrade example extraction data to schema v1.0.
//!
//! This script:
//! 1. Loads each example bill's extraction.json
//! 2. Adds schema_version = "1.0"
//! 3. Fixes SuchSums provisions (semantics "missing" + dollars 0 → SuchSums + "indefinite")
//! 4. Re-parses the source XML and re-runs deterministic verification
//! 5. Writes updated extraction.json, verification.json, and metadata.json
//!
//! Usage: cargo run --bin upgrade_examples

use anyhow::{Context, Result};
use congress_appropriations::approp::loading;
use congress_appropriations::approp::text_index::build_text_index;
use congress_appropriations::approp::verification;
use congress_appropriations::approp::xml;
use serde_json::Value;
use std::path::Path;

fn main() -> Result<()> {
    let examples_dir = Path::new("examples");

    let bill_dirs = ["hr4366", "hr5860", "hr9468"];

    for dir_name in &bill_dirs {
        let bill_dir = examples_dir.join(dir_name);
        println!("Upgrading {}...", bill_dir.display());

        // Step 1: Load and patch extraction.json using raw JSON manipulation
        // (We use raw JSON so we can fix the schema before serde tries to deserialize)
        let ext_path = bill_dir.join("extraction.json");
        let ext_text =
            std::fs::read_to_string(&ext_path).context("Failed to read extraction.json")?;
        let mut ext_json: Value =
            serde_json::from_str(&ext_text).context("Failed to parse extraction.json")?;

        // Add schema_version
        ext_json["schema_version"] = Value::String("1.0".to_string());

        // Fix SuchSums provisions
        let mut fixed_count = 0usize;
        if let Some(provisions) = ext_json["provisions"].as_array_mut() {
            for prov in provisions.iter_mut() {
                // Check the amount field
                if let Some(amount) = prov.get_mut("amount") {
                    if fix_such_sums(amount) {
                        fixed_count += 1;
                    }
                }
                // Also check new_amount and old_amount (for cr_substitution)
                if let Some(new_amount) = prov.get_mut("new_amount") {
                    if fix_such_sums(new_amount) {
                        fixed_count += 1;
                    }
                }
                if let Some(old_amount) = prov.get_mut("old_amount") {
                    if fix_such_sums(old_amount) {
                        fixed_count += 1;
                    }
                }
                // Check amounts array (for "other" provisions)
                if let Some(amounts) = prov.get_mut("amounts") {
                    if let Some(arr) = amounts.as_array_mut() {
                        for amt in arr.iter_mut() {
                            if fix_such_sums(amt) {
                                fixed_count += 1;
                            }
                        }
                    }
                }
            }
        }

        // Write patched extraction.json
        let ext_pretty = serde_json::to_string_pretty(&ext_json)?;
        std::fs::write(&ext_path, &ext_pretty)?;
        println!("  Migrated: {fixed_count} provisions fixed (SuchSums/missing → indefinite)");

        // Step 2: Now load via serde for verification
        let bills = loading::load_bills(&bill_dir)?;
        if bills.is_empty() {
            println!("  WARNING: No bills loaded from {}", bill_dir.display());
            continue;
        }
        let loaded = &bills[0];
        let bill_id = &loaded.extraction.bill.identifier;

        // Step 3: Parse source XML
        let xml_files: Vec<_> = std::fs::read_dir(&bill_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "xml")
            })
            .filter(|e| {
                e.path()
                    .file_stem()
                    .is_some_and(|n| n.to_string_lossy().starts_with("BILLS-"))
            })
            .map(|e| e.path())
            .collect();

        if xml_files.is_empty() {
            println!("  WARNING: No XML source found, skipping re-verification");
            continue;
        }

        let parsed = xml::parse_bill_xml(&xml_files[0], 3000)?;
        let index = build_text_index(&parsed.full_text);

        // Step 4: Re-run verification
        let mut report =
            verification::verify_provisions(&loaded.extraction.provisions, &parsed.full_text, &index);
        report.schema_version = Some("1.0".to_string());

        // Write updated verification.json
        let ver_path = bill_dir.join("verification.json");
        std::fs::write(&ver_path, serde_json::to_string_pretty(&report)?)?;

        println!(
            "  Re-verified: {} provisions, {} not_found, {:.1}% coverage",
            report.summary.total_provisions,
            report.summary.amounts_not_found,
            report.summary.completeness_pct
        );

        // Step 5: Generate metadata.json
        let text_hash = congress_appropriations::approp::text_index::TextIndex::text_hash(&parsed.full_text);
        let metadata = serde_json::json!({
            "extraction_version": "1.1.0",
            "prompt_version": "v3",
            "model": "claude-opus-4-6",
            "schema_version": "1.0",
            "source_pdf_sha256": null,
            "extracted_text_sha256": text_hash,
            "timestamp": "unknown"
        });
        let meta_path = bill_dir.join("metadata.json");
        std::fs::write(&meta_path, serde_json::to_string_pretty(&metadata)?)?;

        println!(
            "  Updated: extraction.json, verification.json, metadata.json"
        );
        println!(
            "  Schema: (none) → 1.0 for {bill_id}"
        );
        println!();
    }

    println!("Done. All examples upgraded to schema v1.0.");
    Ok(())
}

/// Fix a single amount object: if it has kind=specific, dollars=0, semantics=missing,
/// convert to kind=such_sums, semantics=indefinite.
fn fix_such_sums(amount: &mut Value) -> bool {
    if !amount.is_object() {
        return false;
    }

    let semantics_is_missing = amount
        .get("semantics")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s == "missing");

    let value_obj = amount.get("value");
    let kind_is_specific = value_obj
        .and_then(|v| v.get("kind"))
        .and_then(|v| v.as_str())
        .is_some_and(|s| s == "specific");
    let dollars_is_zero = value_obj
        .and_then(|v| v.get("dollars"))
        .and_then(|v| v.as_i64())
        .is_some_and(|d| d == 0);

    let text_is_empty = amount
        .get("text_as_written")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.is_empty());

    if semantics_is_missing && kind_is_specific && dollars_is_zero && text_is_empty {
        // Convert to SuchSums
        amount["value"] = serde_json::json!({"kind": "such_sums"});
        amount["semantics"] = Value::String("indefinite".to_string());
        return true;
    }

    // Also fix cases where semantics is "missing" but dollars is non-zero
    // (just fix the semantics label)
    if semantics_is_missing {
        amount["semantics"] = Value::String("indefinite".to_string());
        return true;
    }

    false
}
