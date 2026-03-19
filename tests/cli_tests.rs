//! CLI integration tests.
//!
//! These tests run the actual binary against the `examples/` data
//! to guard against regressions in output format and data integrity.

use assert_cmd::Command;
use std::str;

fn cmd() -> Command {
    Command::cargo_bin("congress-approp").unwrap()
}

// ─── Budget Authority Totals (critical regression guard) ─────────────────────

#[test]
fn budget_authority_totals_match_expected() {
    let output = cmd()
        .args(["summary", "--dir", "examples", "--format", "json"])
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

    // examples/ may contain more bills than the original 3;
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
        .args(["summary", "--dir", "examples"])
        .assert()
        .success()
        .stdout(predicates::str::contains("H.R. 4366"))
        .stdout(predicates::str::contains("H.R. 5860"))
        .stdout(predicates::str::contains("H.R. 9468"))
        .stdout(predicates::str::contains("Omnibus"))
        .stdout(predicates::str::contains("Continuing Resolution"))
        .stdout(predicates::str::contains("Supplemental"))
        .stdout(predicates::str::contains("Provisions"))
        .stdout(predicates::str::contains("audit"));
}

#[test]
fn summary_json_is_valid() {
    let output = cmd()
        .args(["summary", "--dir", "examples", "--format", "json"])
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
        .args(["audit", "--dir", "examples"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Verified"))
        .stdout(predicates::str::contains("NotFound"))
        .stdout(predicates::str::contains("Coverage"));

    // Parse the table output and verify NotFound column is 0 for all rows
    let output = cmd().args(["audit", "--dir", "examples"]).output().unwrap();
    let _stdout = str::from_utf8(&output.stdout).unwrap();

    // The TOTAL row should show 0 in the NotFound column
    // Table format: │ TOTAL ... ┆ ... ┆ 0 ┆ ...
    // We verify by checking the verbose output doesn't list any NOT FOUND amounts
    let verbose_output = cmd()
        .args(["audit", "--dir", "examples", "--verbose"])
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
        .args(["report", "--dir", "examples"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Verified"));
}

// ─── Search Command ──────────────────────────────────────────────────────────

#[test]
fn search_appropriations_returns_results() {
    cmd()
        .args(["search", "--dir", "examples", "--type", "appropriation"])
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
            "examples/hr9468",
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
    assert_eq!(first["bill"].as_str().unwrap(), "H.R. 9468");
    assert_eq!(first["dollars"].as_i64().unwrap(), 2_285_513_000);
}

#[test]
fn search_csv_has_correct_headers() {
    let output = cmd()
        .args([
            "search",
            "--dir",
            "examples/hr9468",
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
            "examples/hr9468",
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
    // Search all examples for appropriations — this includes reference_amount provisions
    let output = cmd()
        .args([
            "search",
            "--dir",
            "examples",
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
        .args(["summary", "--dir", "examples"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(
        stdout.contains("FYs"),
        "Summary table should have FYs column header"
    );
    // H.R. 4366 covers FY2024
    assert!(
        stdout.contains("2024"),
        "Summary should show fiscal year 2024"
    );
    // H.R. 7148 covers FY2026
    assert!(
        stdout.contains("2026"),
        "Summary should show fiscal year 2026"
    );
}

#[test]
fn search_shows_ambiguous_marker() {
    // Search for appropriations in the omnibus — some should have ≈ (found_multiple)
    let output = cmd()
        .args([
            "search",
            "--dir",
            "examples/hr4366",
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
            "examples/hr5860",
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
        .args(["search", "--dir", "examples", "--keyword", "XYZNONEXISTENT"])
        .assert()
        .success()
        .stdout(predicates::str::contains("No matching provisions found"));
}

// ─── Type Validation ─────────────────────────────────────────────────────────

#[test]
fn search_unknown_type_warns() {
    let output = cmd()
        .args(["search", "--dir", "examples", "--type", "apppropriation"])
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
            "examples/hr5860",
            "--current",
            "examples/hr9468",
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
            "examples/hr4366",
            "--current",
            "examples/hr4366",
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
        .args(["extract", "--dry-run", "--dir", "examples/hr9468"])
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
    for dir in &["examples/hr4366", "examples/hr5860", "examples/hr9468"] {
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
    // Copy a small bill to a temp dir so we don't modify examples/
    let dir = tempfile::tempdir().unwrap();
    let src = std::path::Path::new("examples/hr9468");
    let dst = dir.path().join("hr9468");
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
    let src = std::path::Path::new("examples/hr9468");
    let dst = dir.path().join("hr9468");
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
    // Run against examples/ which already have bill_meta.json
    cmd()
        .args(["enrich", "--dir", "examples"])
        .assert()
        .success()
        .stderr(predicates::str::contains("skip"))
        .stderr(predicates::str::contains("skipped 13"));
}

#[test]
fn enrich_force_re_enriches() {
    let dir = tempfile::tempdir().unwrap();
    let src = std::path::Path::new("examples/hr9468");
    let dst = dir.path().join("hr9468");
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
    let output = cmd()
        .args([
            "summary", "--dir", "examples", "--fy", "2026", "--format", "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    // FY2026 bills: H.R. 5371, H.R. 6938, H.R. 7148, S. 870
    assert!(
        data.len() >= 3 && data.len() <= 5,
        "Expected 3-5 FY2026 bills, found {}",
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
    let output = cmd()
        .args([
            "search",
            "--dir",
            "examples",
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
    let output = cmd()
        .args([
            "summary",
            "--dir",
            "examples",
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
    // Use a temp dir with a bill that has no bill_meta.json
    let dir = tempfile::tempdir().unwrap();
    let src = std::path::Path::new("examples/hr9468");
    let dst = dir.path().join("hr9468");
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
    cmd()
        .args([
            "summary",
            "--dir",
            "examples",
            "--subcommittee",
            "nonexistent",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("Unknown subcommittee"));
}

// ─── Show Advance ────────────────────────────────────────────────────────────

#[test]
fn summary_show_advance_milcon_va() {
    let output = cmd()
        .args([
            "summary",
            "--dir",
            "examples",
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
            "examples",
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
            "examples",
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
    // Re-verify the critical regression guard after all Phase 1 changes
    let output = cmd()
        .args(["summary", "--dir", "examples", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let data: Vec<serde_json::Value> = serde_json::from_str(stdout).unwrap();

    // The unfiltered summary should still show all 13 bills
    assert!(
        data.len() >= 13,
        "Expected at least 13 bills, found {}",
        data.len()
    );

    // The 3 original pinned totals must still match exactly
    let pinned = vec![
        ("H.R. 4366", 846_137_099_554_i64),
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
    let output = cmd()
        .args(["relate", "hr9468:0", "--dir", "examples"])
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
    assert!(
        stdout.contains("b7e688d7"),
        "Should show deterministic hash for first match"
    );
}

#[test]
fn relate_with_fy_timeline() {
    let output = cmd()
        .args(["relate", "hr9468:0", "--dir", "examples", "--fy-timeline"])
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
    let output = cmd()
        .args([
            "relate",
            "hr9468:0",
            "--dir",
            "examples",
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
    let output = cmd()
        .args([
            "relate", "hr9468:0", "--dir", "examples", "--format", "hashes",
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
            "relate", "hr9468:0", "--dir", "examples", "--format", "hashes",
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
    // Missing colon
    cmd()
        .args(["relate", "hr9468", "--dir", "examples"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("Invalid provision reference"));

    // Non-existent bill
    cmd()
        .args(["relate", "nonexistent:0", "--dir", "examples"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not found"));
}

// ─── Link Commands ───────────────────────────────────────────────────────────

#[test]
fn link_suggest_produces_candidates() {
    let output = cmd()
        .args([
            "link", "suggest", "--dir", "examples", "--scope", "cross", "--limit", "5", "--format",
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
    // Use a temp dir to avoid polluting examples/
    let dir = tempfile::tempdir().unwrap();
    // Copy two small bills with embeddings
    for bill in &["hr9468", "hr5860"] {
        let src = std::path::Path::new("examples").join(bill);
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
    let dir = tempfile::tempdir().unwrap();
    for bill in &["hr9468", "hr4366"] {
        let src = std::path::Path::new("examples").join(bill);
        let dst = dir.path().join(bill);
        copy_dir_with_vectors(&src, &dst);
    }

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
    cmd()
        .args(["link", "suggest", "--dir", "examples", "--scope", "invalid"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("Invalid scope"));
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
