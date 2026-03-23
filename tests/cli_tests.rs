//! CLI integration tests.
//!
//! These tests run the actual binary against the `data/` and `test-data/` data
//! to guard against regressions in output format and data integrity.

use assert_cmd::Command;
use std::str;

fn cmd() -> Command {
    Command::cargo_bin("congress-approp").unwrap()
}

/// Returns true if the full `data/` directory is available (git clone).
/// Tier 2 tests call this and return early if false.
fn has_full_data() -> bool {
    // Check for a bill that only exists in the full dataset, not test-data/
    std::path::Path::new("data/118-hr4366/extraction.json").exists()
}

/// Tier 2 tests that need embeddings call this and return early if false.
/// Embeddings may not exist if vectors.bin was purged during re-extraction.
fn has_embeddings() -> bool {
    has_full_data()
        && std::path::Path::new("data/118-hr4366/vectors.bin").exists()
        && std::path::Path::new("data/118-hr4366/embeddings.json").exists()
}

// ─── Budget Authority Totals (critical regression guard) ─────────────────────

#[test]
fn budget_authority_totals_match_expected() {
    if !has_full_data() {
        return;
    }
    let output = cmd()
        .args(["summary", "--dir", "data", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    let expected: Vec<(&str, i64, i64)> = vec![
        ("H.R. 4366", 921_196_642_442, 24_659_349_709),
        ("H.R. 5860", 16_000_000_000, 0),
        ("H.R. 9468", 2_882_482_000, 0),
        ("H.R. 133", 3_378_417_630_375, 592_527_866_970),
        ("H.R. 2471", 3_030_890_491_454, 11_385_190_503),
        ("H.R. 2617", 3_379_029_309_541, 25_470_881_313),
        ("H.R. 2882", 2_450_574_266_121, 38_038_396_359),
        ("H.R. 7148", 2_840_611_498_956, 34_192_835_670),
    ];

    // data/ may contain more bills than the original 3;
    // verify the original 3 are present with correct totals.
    assert!(
        data.len() >= expected.len(),
        "Expected at least {} bills, found {}",
        expected.len(),
        data.len()
    );

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

// ─── Summary Command ─────────────────────────────────────────────────────────

#[test]
fn summary_table_runs_successfully() {
    cmd()
        .args(["summary", "--dir", "test-data"])
        .assert()
        .success()
        .stdout(predicates::str::contains("H.R. 5860"))
        .stdout(predicates::str::contains("H.R. 9468"))
        .stdout(predicates::str::contains("H.R. 2872"))
        .stdout(predicates::str::contains("Continuing Resolution"))
        .stdout(predicates::str::contains("Supplemental"))
        .stdout(predicates::str::contains("Provisions"))
        .stdout(predicates::str::contains("audit"));
}

#[test]
fn summary_json_is_valid() {
    let output = cmd()
        .args(["summary", "--dir", "test-data", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();
    assert!(
        data.len() >= 3,
        "Expected at least 3 bills, found {}",
        data.len()
    );
}

// ─── Audit Command ───────────────────────────────────────────────────────────

#[test]
fn audit_shows_zero_not_found() {
    cmd()
        .args(["audit", "--dir", "test-data"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Verified"))
        .stdout(predicates::str::contains("NotFound"))
        .stdout(predicates::str::contains("Coverage"));

    // Parse the table output and verify NotFound column is 0 for all rows
    let output = cmd()
        .args(["audit", "--dir", "test-data"])
        .output()
        .unwrap();
    let _stdout = str::from_utf8(&output.stdout).unwrap();

    // The TOTAL row should show 0 in the NotFound column
    // Table format: │ TOTAL ... ┆ ... ┆ 0 ┆ ...
    // We verify by checking the verbose output doesn't list any NOT FOUND amounts
    let verbose_output = cmd()
        .args(["audit", "--dir", "test-data", "--verbose"])
        .output()
        .unwrap();
    let verbose_stdout = str::from_utf8(&verbose_output.stdout).unwrap();
    assert!(
        !verbose_stdout.contains("NOT FOUND"),
        "Found NOT FOUND amounts in audit output"
    );
}

#[test]
fn audit_report_alias_works() {
    // The old "report" command should still work as an alias
    cmd()
        .args(["report", "--dir", "test-data"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Verified"));
}

// ─── Search Command ──────────────────────────────────────────────────────────

#[test]
fn search_appropriations_returns_results() {
    cmd()
        .args(["search", "--dir", "test-data", "--type", "appropriation"])
        .assert()
        .success()
        .stdout(predicates::str::contains("$")) // column header
        .stdout(predicates::str::contains("Amount status")); // legend
}

#[test]
fn search_json_has_correct_fields() {
    let output = cmd()
        .args([
            "search",
            "--dir",
            "test-data/118-hr9468",
            "--type",
            "appropriation",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    assert_eq!(data.len(), 2, "hr9468 should have 2 appropriations");

    let first = &data[0];
    // Verify new field names are present
    assert!(
        first.get("amount_status").is_some(),
        "Missing amount_status field"
    );
    assert!(
        first.get("verified").is_none(),
        "Old 'verified' field should not exist"
    );
    assert!(
        first.get("match_tier").is_some(),
        "Missing match_tier field"
    );
    assert!(
        first.get("provision_type").is_some(),
        "Missing provision_type field"
    );

    // Verify the actual values
    assert_eq!(first["amount_status"].as_str().unwrap(), "found");
    assert_eq!(first["bill"].as_str().unwrap(), "H.R. 9468 (118th)");
    assert_eq!(first["dollars"].as_i64().unwrap(), 2_285_513_000);
}

#[test]
fn search_csv_has_correct_headers() {
    let output = cmd()
        .args([
            "search",
            "--dir",
            "test-data/118-hr9468",
            "--type",
            "appropriation",
            "--format",
            "csv",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let first_line = stdout.lines().next().unwrap();

    assert!(
        first_line.contains("amount_status"),
        "CSV should have amount_status column"
    );
    assert!(
        !first_line.contains(",verified,"),
        "CSV should NOT have old 'verified' column"
    );
    assert!(
        first_line.contains("provision_type"),
        "CSV should have provision_type column"
    );
    assert!(
        first_line.contains("fiscal_year"),
        "CSV should have fiscal_year column"
    );
    assert!(
        first_line.contains("detail_level"),
        "CSV should have detail_level column"
    );
    assert!(
        first_line.contains("confidence"),
        "CSV should have confidence column"
    );
    assert!(
        first_line.contains("provision_index"),
        "CSV should have provision_index column"
    );
    assert!(
        first_line.contains("match_tier"),
        "CSV should have match_tier column"
    );
}

#[test]
fn search_csv_new_columns_populated() {
    let output = cmd()
        .args([
            "search",
            "--dir",
            "test-data/118-hr9468",
            "--type",
            "appropriation",
            "--format",
            "csv",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    // Should have header + 2 data rows (hr9468 has 2 appropriations)
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 2, "Expected header + at least 1 data row");

    // Parse as CSV and check new column values
    let mut reader = csv::ReaderBuilder::new().from_reader(stdout.as_bytes());
    let headers = reader.headers().unwrap().clone();

    let fy_idx = headers.iter().position(|h| h == "fiscal_year").unwrap();
    let dl_idx = headers.iter().position(|h| h == "detail_level").unwrap();
    let conf_idx = headers.iter().position(|h| h == "confidence").unwrap();
    let pi_idx = headers.iter().position(|h| h == "provision_index").unwrap();

    let first_row = reader.records().next().unwrap().unwrap();
    assert_eq!(
        &first_row[fy_idx], "2024",
        "fiscal_year should be 2024 for hr9468"
    );
    assert_eq!(
        &first_row[dl_idx], "top_level",
        "detail_level should be top_level"
    );
    assert!(
        !first_row[conf_idx].is_empty(),
        "confidence should not be empty"
    );
    assert_eq!(&first_row[pi_idx], "0", "first provision_index should be 0");
}

#[test]
fn search_csv_stderr_warns_mixed_semantics() {
    // Search all bills for appropriations — this includes reference_amount provisions
    let output = cmd()
        .args([
            "search",
            "--dir",
            "test-data",
            "--type",
            "appropriation",
            "--format",
            "csv",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("reference_amount"),
        "stderr should warn about reference_amount provisions: {stderr}"
    );
    assert!(
        stderr.contains("filter to semantics"),
        "stderr should suggest filtering by semantics: {stderr}"
    );
}

#[test]
fn summary_table_shows_fiscal_years() {
    let output = cmd()
        .args(["summary", "--dir", "test-data"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(
        stdout.contains("FYs"),
        "Summary table should have FYs column header"
    );
    // All test-data bills cover FY2024
    assert!(
        stdout.contains("2024"),
        "Summary should show fiscal year 2024"
    );
}

#[test]
fn search_shows_ambiguous_marker() {
    // Search for appropriations in the omnibus — some should have ≈ (found_multiple)
    let output = cmd()
        .args([
            "search",
            "--dir",
            "data/118-hr4366",
            "--type",
            "appropriation",
            "--agency",
            "Veterans",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();

    // Should have both ✓ (found unique) and ≈ (found multiple)
    assert!(
        stdout.contains('✓'),
        "Should contain ✓ for uniquely found amounts"
    );
    assert!(
        stdout.contains('≈'),
        "Should contain ≈ for multiply-found amounts"
    );
}

#[test]
fn search_cr_substitution_table_format() {
    cmd()
        .args([
            "search",
            "--dir",
            "test-data/118-hr5860",
            "--type",
            "cr_substitution",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("New ($)"))
        .stdout(predicates::str::contains("Old ($)"))
        .stdout(predicates::str::contains("Delta ($)"))
        .stdout(predicates::str::contains("13 provisions found"));
}

#[test]
fn search_empty_result_no_error() {
    cmd()
        .args([
            "search",
            "--dir",
            "test-data",
            "--keyword",
            "XYZNONEXISTENT",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("No matching provisions found"));
}

// ─── Type Validation ─────────────────────────────────────────────────────────

#[test]
fn search_unknown_type_warns() {
    let output = cmd()
        .args(["search", "--dir", "test-data", "--type", "apppropriation"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("unknown provision type"),
        "Should warn about unknown type"
    );
    assert!(stderr.contains("appropriation"), "Should list valid types");
}

// ─── Compare Command ─────────────────────────────────────────────────────────

#[test]
fn compare_cross_type_shows_warning() {
    let output = cmd()
        .args([
            "compare",
            "--base",
            "test-data/118-hr5860",
            "--current",
            "test-data/118-hr9468",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("Comparing Continuing Resolution to Supplemental"),
        "Should warn about cross-type comparison"
    );

    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(
        stdout.contains("only in base") || stdout.contains("only in current"),
        "Should use new status labels"
    );
    assert!(
        !stdout.contains("eliminated"),
        "Should NOT use old 'eliminated' label"
    );
}

#[test]
fn compare_same_type_no_warning() {
    let output = cmd()
        .args([
            "compare",
            "--base",
            "data/118-hr4366",
            "--current",
            "data/118-hr4366",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    // Same type comparison should NOT produce a warning
    assert!(
        !stderr.contains("Comparing"),
        "Should not warn when comparing same type"
    );
}

// ─── Extract Dry Run ─────────────────────────────────────────────────────────

#[test]
fn extract_dry_run_works_without_api_key() {
    // Unset API key to verify dry-run doesn't need it
    let output = cmd()
        .args(["extract", "--dry-run", "--dir", "test-data/118-hr9468"])
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "extract --dry-run should work without ANTHROPIC_API_KEY"
    );
}

// ─── Schema Versioning ───────────────────────────────────────────────────────

#[test]
fn example_data_has_schema_version() {
    if !has_full_data() {
        return;
    }
    for dir in &[
        "data/118-hr4366",
        "test-data/118-hr5860",
        "test-data/118-hr9468",
    ] {
        let ext_path = format!("{dir}/extraction.json");
        let ext_text = std::fs::read_to_string(&ext_path)
            .unwrap_or_else(|_| panic!("Failed to read {ext_path}"));
        let ext: serde_json::Value = serde_json::from_str(&ext_text).unwrap();

        assert_eq!(
            ext["schema_version"].as_str(),
            Some("1.0"),
            "{dir}/extraction.json should have schema_version 1.0"
        );

        let ver_path = format!("{dir}/verification.json");
        let ver_text = std::fs::read_to_string(&ver_path)
            .unwrap_or_else(|_| panic!("Failed to read {ver_path}"));
        let ver: serde_json::Value = serde_json::from_str(&ver_text).unwrap();

        assert_eq!(
            ver["schema_version"].as_str(),
            Some("1.0"),
            "{dir}/verification.json should have schema_version 1.0"
        );

        let meta_path = format!("{dir}/metadata.json");
        assert!(
            std::path::Path::new(&meta_path).exists(),
            "{dir}/metadata.json should exist"
        );
    }
}

// ─── Empty Directory Handling ────────────────────────────────────────────────

#[test]
fn summary_empty_dir_no_crash() {
    let dir = tempfile::tempdir().unwrap();
    cmd()
        .args(["summary", "--dir", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("No extracted bills found"));
}

#[test]
fn audit_empty_dir_no_crash() {
    let dir = tempfile::tempdir().unwrap();
    cmd()
        .args(["audit", "--dir", dir.path().to_str().unwrap()])
        .assert()
        .success();
}

// ─── Enrich Command ──────────────────────────────────────────────────────────

#[test]
fn enrich_dry_run_writes_nothing() {
    // Copy a small bill to a temp dir so we don't modify test-data/
    let dir = tempfile::tempdir().unwrap();
    let src = std::path::Path::new("test-data/118-hr9468");
    let dst = dir.path().join("118-hr9468");
    copy_dir(src, &dst);

    // Remove bill_meta.json if it was copied
    let meta_path = dst.join("bill_meta.json");
    let _ = std::fs::remove_file(&meta_path);
    assert!(
        !meta_path.exists(),
        "bill_meta.json should not exist before dry run"
    );

    cmd()
        .args(["enrich", "--dir", dir.path().to_str().unwrap(), "--dry-run"])
        .assert()
        .success()
        .stderr(predicates::str::contains("would enrich"));

    assert!(
        !meta_path.exists(),
        "bill_meta.json should not exist after dry run"
    );
}

#[test]
fn enrich_creates_bill_meta() {
    let dir = tempfile::tempdir().unwrap();
    let src = std::path::Path::new("test-data/118-hr9468");
    let dst = dir.path().join("118-hr9468");
    copy_dir(src, &dst);

    // Remove bill_meta.json if copied
    let meta_path = dst.join("bill_meta.json");
    let _ = std::fs::remove_file(&meta_path);

    cmd()
        .args(["enrich", "--dir", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stderr(predicates::str::contains("enriched"));

    assert!(meta_path.exists(), "bill_meta.json should be created");

    // Validate the JSON structure
    let content = std::fs::read_to_string(&meta_path).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(meta["schema_version"].as_str(), Some("1.0"));
    assert!(meta["extraction_sha256"].as_str().is_some());
    assert!(meta["bill_nature"].as_str().is_some());
    assert!(meta["fiscal_years"].as_array().is_some());
}

#[test]
fn enrich_skips_existing() {
    if !has_full_data() {
        return;
    }
    // Run against data/ which already have bill_meta.json
    cmd()
        .args(["enrich", "--dir", "data"])
        .assert()
        .success()
        .stderr(predicates::str::contains("skip"))
        .stderr(predicates::str::contains("skipped 32"));
}

#[test]
fn enrich_force_re_enriches() {
    let dir = tempfile::tempdir().unwrap();
    let src = std::path::Path::new("test-data/118-hr9468");
    let dst = dir.path().join("118-hr9468");
    copy_dir(src, &dst);

    // First enrich
    cmd()
        .args(["enrich", "--dir", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // Second enrich without --force should skip
    cmd()
        .args(["enrich", "--dir", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stderr(predicates::str::contains("skip"));

    // With --force should re-enrich
    cmd()
        .args(["enrich", "--dir", dir.path().to_str().unwrap(), "--force"])
        .assert()
        .success()
        .stderr(predicates::str::contains("enriched"));
}

// ─── FY Filtering ────────────────────────────────────────────────────────────

#[test]
fn summary_fy_filter_narrows_bills() {
    if !has_full_data() {
        return;
    }
    let output = cmd()
        .args([
            "summary", "--dir", "data", "--fy", "2026", "--format", "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    // FY2026 bills: H.R. 5371, H.R. 6938, H.R. 7148, S. 870
    assert!(
        data.len() >= 3 && data.len() <= 8,
        "Expected 3-8 FY2026 bills, found {}",
        data.len()
    );

    // H.R. 4366 (FY2024) should NOT be present
    let has_hr4366 = data
        .iter()
        .any(|b| b["identifier"].as_str() == Some("H.R. 4366"));
    assert!(
        !has_hr4366,
        "H.R. 4366 (FY2024) should not appear in FY2026 filter"
    );

    // H.R. 7148 (FY2026) should be present
    let has_hr7148 = data
        .iter()
        .any(|b| b["identifier"].as_str() == Some("H.R. 7148"));
    assert!(
        has_hr7148,
        "H.R. 7148 (FY2026) should appear in FY2026 filter"
    );
}

#[test]
fn search_fy_filter_excludes_other_years() {
    if !has_full_data() {
        return;
    }
    let output = cmd()
        .args([
            "search",
            "--dir",
            "data",
            "--type",
            "appropriation",
            "--fy",
            "2026",
            "--account",
            "Tenant-Based Rental",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    // Should find TBRA in FY2026 bills but NOT in H.R. 4366 (FY2024)
    for item in &data {
        assert_ne!(
            item["bill"].as_str(),
            Some("H.R. 4366"),
            "FY2024 bill H.R. 4366 should not appear with --fy 2026"
        );
    }
    assert!(
        !data.is_empty(),
        "Should find at least one TBRA provision in FY2026"
    );
}

// ─── Subcommittee Filtering ──────────────────────────────────────────────────

#[test]
fn summary_subcommittee_filter() {
    if !has_full_data() {
        return;
    }
    let output = cmd()
        .args([
            "summary",
            "--dir",
            "data",
            "--fy",
            "2026",
            "--subcommittee",
            "thud",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    // Should be exactly 1 bill: H.R. 7148 (the only FY2026 bill with a THUD division)
    assert_eq!(
        data.len(),
        1,
        "Expected 1 bill for FY2026 THUD, found {}",
        data.len()
    );
    assert_eq!(data[0]["identifier"].as_str(), Some("H.R. 7148"));

    // Provision count should be the THUD division only (618), not full bill (2837)
    let provisions = data[0]["provisions"].as_i64().unwrap();
    assert!(
        provisions < 1000,
        "THUD division should have <1000 provisions, got {provisions}"
    );
}

#[test]
fn subcommittee_without_enrich_gives_clear_error() {
    if !has_full_data() {
        return;
    }
    // Use a temp dir with a bill that has no bill_meta.json
    let dir = tempfile::tempdir().unwrap();
    let src = std::path::Path::new("test-data/118-hr9468");
    let dst = dir.path().join("118-hr9468");
    copy_dir(src, &dst);

    // Remove bill_meta.json
    let _ = std::fs::remove_file(dst.join("bill_meta.json"));

    cmd()
        .args([
            "summary",
            "--dir",
            dir.path().to_str().unwrap(),
            "--subcommittee",
            "thud",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("enrich"));
}

#[test]
fn subcommittee_invalid_slug_gives_error() {
    if !has_full_data() {
        return;
    }
    cmd()
        .args(["summary", "--dir", "data", "--subcommittee", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("Unknown subcommittee"));
}

// ─── Show Advance ────────────────────────────────────────────────────────────

#[test]
fn summary_show_advance_milcon_va() {
    if !has_full_data() {
        return;
    }
    let output = cmd()
        .args([
            "summary",
            "--dir",
            "data",
            "--fy",
            "2026",
            "--subcommittee",
            "milcon-va",
            "--show-advance",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    assert_eq!(
        data.len(),
        1,
        "Should have exactly 1 bill for FY2026 MilCon-VA"
    );

    let bill = &data[0];
    assert_eq!(bill["identifier"].as_str(), Some("H.R. 5371"));

    // MilCon-VA is ~79.5% advance — advance should be much larger than current
    let current = bill["current_year_ba"].as_i64().unwrap();
    let advance = bill["advance_ba"].as_i64().unwrap();
    let total = bill["budget_authority"].as_i64().unwrap();

    assert!(
        advance > current,
        "MilCon-VA advance ({advance}) should exceed current ({current})"
    );
    assert!(
        advance > 300_000_000_000,
        "MilCon-VA advance should be >$300B, got {advance}"
    );
    assert!(
        current > 50_000_000_000 && current < 200_000_000_000,
        "MilCon-VA current should be $50-200B, got {current}"
    );
    // current + advance should approximately equal total BA
    // (may not be exact due to supplemental/unknown provisions)
    let sum = current + advance;
    assert!(
        (sum - total).abs() < 1_000_000_000,
        "current ({current}) + advance ({advance}) = {sum} should be close to total ({total})"
    );
}

// ─── FY-Based Compare ────────────────────────────────────────────────────────

#[test]
fn compare_base_fy_current_fy() {
    if !has_full_data() {
        return;
    }
    let output = cmd()
        .args([
            "compare",
            "--base-fy",
            "2024",
            "--current-fy",
            "2026",
            "--subcommittee",
            "thud",
            "--dir",
            "data",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();

    // Should show meaningful comparison (43 changed, 12 unchanged)
    assert!(
        stdout.contains("changed"),
        "Compare output should contain 'changed' status"
    );
    assert!(
        stdout.contains("Comparing:"),
        "Compare output should show base → current description"
    );
    // Should have matched some accounts (not all orphans)
    assert!(
        stdout.contains("unchanged"),
        "Some accounts should be unchanged between FY2024 and FY2026 THUD"
    );
}

#[test]
fn compare_requires_base_and_current() {
    // Neither --base/--current nor --base-fy/--current-fy
    cmd()
        .args(["compare", "--format", "json"])
        .assert()
        .failure();
}

// ─── Compare Case-Insensitive Matching ───────────────────────────────────────

#[test]
fn compare_case_insensitive_grants_in_aid() {
    if !has_full_data() {
        return;
    }
    // Compare FY2024→FY2026 THUD and verify Grants-in-Aid for Airports matches
    // (it has case variants: "Grants-In-Aid" vs "Grants-in-Aid" vs "Grants-in-aid")
    let output = cmd()
        .args([
            "compare",
            "--base-fy",
            "2024",
            "--current-fy",
            "2026",
            "--subcommittee",
            "thud",
            "--dir",
            "data",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let rows: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    // Find Grants-in-Aid for Airports — should be "changed" (matched), not orphaned
    let grants = rows.iter().find(|r| {
        r["account_name"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .contains("grants-in-aid for airports")
    });

    assert!(
        grants.is_some(),
        "Grants-in-Aid for Airports should appear in compare output"
    );

    let status = grants.unwrap()["status"].as_str().unwrap();
    assert_eq!(
        status, "changed",
        "Grants-in-Aid should be 'changed' (matched across case variants), got '{status}'"
    );
}

// ─── Budget Totals Unchanged After All Changes ───────────────────────────────

#[test]
fn budget_totals_unchanged_after_phase1() {
    if !has_full_data() {
        return;
    }
    // Re-verify the critical regression guard after all Phase 1 changes
    let output = cmd()
        .args(["summary", "--dir", "data", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    // The unfiltered summary should show all 32 bills
    assert!(
        data.len() >= 32,
        "Expected at least 32 bills, found {}",
        data.len()
    );

    // Pinned totals must still match exactly
    let pinned = vec![
        ("H.R. 4366", 921_196_642_442_i64),
        ("H.R. 5860", 16_000_000_000_i64),
        ("H.R. 9468", 2_882_482_000_i64),
    ];

    for (bill, expected_ba) in &pinned {
        let entry = data
            .iter()
            .find(|b| b["identifier"].as_str().unwrap() == *bill)
            .unwrap_or_else(|| panic!("Missing bill: {bill}"));
        let ba = entry["budget_authority"].as_i64().unwrap();
        assert_eq!(ba, *expected_ba, "{bill} budget authority changed!");
    }
}

// ─── Relate Command ──────────────────────────────────────────────────────────

#[test]
fn relate_table_output() {
    if !has_embeddings() {
        return;
    }
    let output = cmd()
        .args(["relate", "118-hr9468:0", "--dir", "data"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();

    // Should show the source provision
    assert!(
        stdout.contains("Compensation and Pensions"),
        "Should show source account name"
    );
    assert!(stdout.contains("H.R. 9468"), "Should show source bill");
    // Should show matches with hashes
    assert!(
        stdout.contains("Same Account:"),
        "Should have a Same Account section"
    );
    assert!(
        stdout.contains("verified"),
        "Should show verified confidence for name-matched provisions"
    );
    // Should have 8-char hashes
    // Hashes are deterministic but depend on provision indices which change
    // across re-extractions. Just verify that 8-char hex hashes are present.
    let has_hash = stdout.lines().any(|line| {
        line.split_whitespace().any(|word| word.len() == 8 && word.chars().all(|c| c.is_ascii_hexdigit()))
    });
    assert!(
        has_hash,
        "Should show deterministic 8-char hex hashes in output"
    );
}

#[test]
fn relate_with_fy_timeline() {
    if !has_embeddings() {
        return;
    }
    let output = cmd()
        .args(["relate", "118-hr9468:0", "--dir", "data", "--fy-timeline"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();

    assert!(
        stdout.contains("Fiscal Year Timeline:"),
        "Should show timeline section"
    );
    assert!(stdout.contains("2024"), "Timeline should include FY2024");
    assert!(stdout.contains("2026"), "Timeline should include FY2026");
}

#[test]
fn relate_json_output() {
    if !has_embeddings() {
        return;
    }
    let output = cmd()
        .args([
            "relate",
            "118-hr9468:0",
            "--dir",
            "data",
            "--format",
            "json",
            "--fy-timeline",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(stdout).unwrap();

    assert_eq!(report["source_bill"].as_str(), Some("H.R. 9468"));
    assert_eq!(report["source_index"].as_u64(), Some(0));
    assert!(report["same_account"].as_array().unwrap().len() >= 3);
    assert!(report["timeline"].as_array().is_some());

    // Check that hashes are present and 8 chars
    let first_hash = report["same_account"][0]["hash"].as_str().unwrap();
    assert_eq!(first_hash.len(), 8, "Hash should be 8 hex chars");
}

#[test]
fn relate_hashes_output() {
    if !has_embeddings() {
        return;
    }
    let output = cmd()
        .args([
            "relate",
            "118-hr9468:0",
            "--dir",
            "data",
            "--format",
            "hashes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let hashes: Vec<&str> = stdout.trim().lines().collect();

    assert!(hashes.len() >= 3, "Should output at least 3 hashes");
    for hash in &hashes {
        assert_eq!(hash.len(), 8, "Each hash should be 8 hex chars");
    }

    // Hashes should be deterministic
    let output2 = cmd()
        .args([
            "relate",
            "118-hr9468:0",
            "--dir",
            "data",
            "--format",
            "hashes",
        ])
        .output()
        .unwrap();
    let stdout2 = str::from_utf8(&output2.stdout).unwrap();
    assert_eq!(
        stdout, stdout2,
        "Hashes should be deterministic across runs"
    );
}

#[test]
fn relate_invalid_reference() {
    if !has_full_data() {
        return;
    }
    // Missing colon — no index separator
    cmd()
        .args(["relate", "118-hr9468", "--dir", "data"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("Invalid provision reference"));

    // Non-existent bill
    cmd()
        .args(["relate", "nonexistent:0", "--dir", "data"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not found"));

    // Out-of-range provision index
    cmd()
        .args(["relate", "118-hr9468:99", "--dir", "data"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("out of range"));
}

// ─── Link Commands ───────────────────────────────────────────────────────────

#[test]
fn link_suggest_produces_candidates() {
    if !has_embeddings() {
        return;
    }
    let output = cmd()
        .args([
            "link", "suggest", "--dir", "data", "--scope", "cross", "--limit", "5", "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let candidates: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    assert!(
        !candidates.is_empty(),
        "Should produce at least one candidate"
    );
    // Each candidate should have a hash, similarity, and confidence
    let first = &candidates[0];
    assert!(first["hash"].as_str().is_some());
    assert!(first["similarity"].as_f64().is_some());
    assert!(first["confidence"].as_str().is_some());
}

#[test]
fn link_full_workflow() {
    if !has_full_data() {
        return;
    }
    // Use a temp dir to avoid polluting data/
    let dir = tempfile::tempdir().unwrap();
    // Copy two small bills with embeddings
    for bill in &["118-hr9468", "118-hr5860"] {
        let src = std::path::Path::new("data").join(bill);
        let dst = dir.path().join(bill);
        copy_dir_with_vectors(&src, &dst);
    }

    // Step 1: Suggest links
    let output = cmd()
        .args([
            "link",
            "suggest",
            "--dir",
            dir.path().to_str().unwrap(),
            "--scope",
            "all",
            "--limit",
            "50",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let candidates: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    if candidates.is_empty() {
        // No candidates between these two small bills — that's OK, skip rest
        return;
    }

    // Step 2: Accept first candidate by hash
    let first_hash = candidates[0]["hash"].as_str().unwrap().to_string();
    cmd()
        .args([
            "link",
            "accept",
            "--dir",
            dir.path().to_str().unwrap(),
            &first_hash,
        ])
        .assert()
        .success()
        .stderr(predicates::str::contains("Accepted 1 links"));

    // Step 3: List shows the accepted link
    let output = cmd()
        .args([
            "link",
            "list",
            "--dir",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let links: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["hash"].as_str().unwrap(), first_hash);

    // Step 4: Remove the link
    cmd()
        .args([
            "link",
            "remove",
            "--dir",
            dir.path().to_str().unwrap(),
            &first_hash,
        ])
        .assert()
        .success()
        .stderr(predicates::str::contains("Removed 1 links"));

    // Step 5: List is now empty
    let output = cmd()
        .args([
            "link",
            "list",
            "--dir",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let links: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();
    assert!(links.is_empty());
}

#[test]
fn link_accept_auto() {
    if !has_embeddings() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for bill in &["118-hr9468", "118-hr4366"] {
        let src = std::path::Path::new("data").join(bill);
        let dst = dir.path().join(bill);
        copy_dir_with_vectors(&src, &dst);
    }

    // Must run link suggest first to populate the cache
    cmd()
        .args([
            "link",
            "suggest",
            "--dir",
            dir.path().to_str().unwrap(),
            "--scope",
            "all",
            "--limit",
            "10",
        ])
        .assert()
        .success();

    cmd()
        .args([
            "link",
            "accept",
            "--dir",
            dir.path().to_str().unwrap(),
            "--auto",
        ])
        .assert()
        .success()
        .stderr(predicates::str::contains("Auto-accepted"));
}

#[test]
fn link_list_empty_no_crash() {
    let dir = tempfile::tempdir().unwrap();
    cmd()
        .args(["link", "list", "--dir", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("No links file found"));
}

#[test]
fn link_suggest_invalid_scope() {
    if !has_full_data() {
        return;
    }
    cmd()
        .args(["link", "suggest", "--dir", "data", "--scope", "invalid"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("Invalid scope"));
}

// ─── Normalize & Entity Resolution Tests ─────────────────────────────────────

#[test]
fn normalize_list_without_dataset() {
    // test-data has no dataset.json — should show helpful message
    cmd()
        .args(["normalize", "list", "--dir", "test-data"])
        .assert()
        .success()
        .stdout(predicates::str::contains("No dataset.json"));
}

#[test]
fn normalize_list_with_dataset() {
    let dir = tempfile::tempdir().unwrap();
    // Create a minimal dataset.json
    let dataset = serde_json::json!({
        "schema_version": "1.0",
        "entities": {
            "agency_groups": [{
                "canonical": "Department of Defense",
                "members": ["Department of the Army"]
            }],
            "account_aliases": []
        }
    });
    std::fs::write(
        dir.path().join("dataset.json"),
        serde_json::to_string_pretty(&dataset).unwrap(),
    )
    .unwrap();

    cmd()
        .args(["normalize", "list", "--dir", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("Department of Defense"))
        .stdout(predicates::str::contains("Department of the Army"));
}

#[test]
fn normalize_suggest_text_match_outputs_hashes() {
    if !has_full_data() {
        return;
    }
    // suggest-text-match is read-only — it caches results but never writes dataset.json
    let output = cmd()
        .args(["normalize", "suggest-text-match", "--dir", "data"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(
        stdout.contains("suggested agency groups"),
        "Should report suggestions: {stdout}"
    );
    // Should show hashes in the output
    assert!(
        stdout.contains('['),
        "Should show hash brackets in table output: {stdout}"
    );
    let stderr = str::from_utf8(&output.stderr).unwrap();
    // Should suggest using accept command
    assert!(
        stderr.contains("normalize accept"),
        "Should suggest accept command: {stderr}"
    );
    // Verify no dataset.json was created (suggest is read-only)
    assert!(
        !std::path::Path::new("data/dataset.json").exists(),
        "suggest should not create dataset.json"
    );

    // Test --format hashes outputs clean hash list
    let output2 = cmd()
        .args([
            "normalize",
            "suggest-text-match",
            "--dir",
            "data",
            "--format",
            "hashes",
        ])
        .output()
        .unwrap();

    assert!(output2.status.success());
    let stdout2 = str::from_utf8(&output2.stdout).unwrap();
    let lines: Vec<&str> = stdout2.lines().collect();
    assert!(
        lines.len() > 5,
        "Should output many hashes, got {}",
        lines.len()
    );
    // Each line should be an 8-char hex hash
    for line in &lines[..3] {
        assert_eq!(line.len(), 8, "Hash should be 8 chars: '{line}'");
        assert!(
            line.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should be hex: '{line}'"
        );
    }
}

#[test]
fn compare_exact_no_normalization() {
    if !has_full_data() {
        return;
    }
    // Run compare --exact and verify no rows are marked normalized
    let output = cmd()
        .args([
            "compare",
            "--base-fy",
            "2024",
            "--current-fy",
            "2026",
            "--subcommittee",
            "thud",
            "--dir",
            "data",
            "--exact",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let rows: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();
    let normalized_count = rows
        .iter()
        .filter(|r| r["normalized"].as_bool() == Some(true))
        .count();
    assert_eq!(
        normalized_count, 0,
        "compare --exact should produce 0 normalized rows"
    );
}

#[test]
fn compare_csv_has_normalized_column() {
    if !has_full_data() {
        return;
    }
    let output = cmd()
        .args([
            "compare",
            "--base-fy",
            "2024",
            "--current-fy",
            "2026",
            "--subcommittee",
            "thud",
            "--dir",
            "data",
            "--format",
            "csv",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let first_line = stdout.lines().next().unwrap();
    assert!(
        first_line.contains("normalized"),
        "CSV header should have normalized column: {first_line}"
    );
    // Status field should NOT contain "(normalized)" suffix in CSV
    assert!(
        !stdout.contains("(normalized)"),
        "CSV status should not have (normalized) suffix — it should be a separate column"
    );
    // The normalized column should have true/false values
    assert!(
        stdout.contains("false") || stdout.contains("true"),
        "normalized column should contain true/false values"
    );
}

#[test]
fn compare_orphan_hint() {
    // Use test-data (small bills, no dataset.json) — should show orphan hint
    let output = cmd()
        .args([
            "compare",
            "--base",
            "test-data/118-hr9468",
            "--current",
            "test-data/118-hr5860",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    // These bills have different accounts so there will be orphans
    // The hint should suggest normalize suggest-text-match
    if stderr.contains("orphan pairs") {
        assert!(
            stderr.contains("normalize suggest-text-match"),
            "Orphan hint should suggest normalize command: {stderr}"
        );
    }
    // If no orphans (unlikely but possible), that's fine too
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Recursively copy a directory, skipping vectors.bin (large) and chunks/ (unnecessary).
fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip large/unnecessary files for test speed
        if name == "vectors.bin" || name == "chunks" {
            continue;
        }

        if ty.is_dir() {
            copy_dir(&src_path, &dst_path);
        } else {
            std::fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}

/// Copy a directory INCLUDING vectors.bin (needed for link suggest which requires embeddings).
fn copy_dir_with_vectors(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip only chunks/ (unnecessary for tests)
        if name == "chunks" {
            continue;
        }

        if ty.is_dir() {
            copy_dir_with_vectors(&src_path, &dst_path);
        } else {
            std::fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}
