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

    assert_eq!(data.len(), expected.len(), "Wrong number of bills");

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
    assert_eq!(data.len(), 3);
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
