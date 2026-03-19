//! Inflation adjustment for cross-fiscal-year appropriations comparisons.
//!
//! Loads CPI-U monthly data (bundled or from a user-provided file), computes
//! fiscal-year-weighted averages, and provides inflation rates and flags for
//! the `compare --real` feature.
//!
//! The bundled CPI data is embedded at compile time via `include_str!` from
//! `data/cpi.json`. Users can override it with `--cpi-file <PATH>`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ─── Embedded CPI Data ───────────────────────────────────────────────────────

/// Bundled CPI-U data, embedded at compile time from data/cpi.json.
const BUNDLED_CPI_JSON: &str = include_str!("cpi.json");

// ─── Types ───────────────────────────────────────────────────────────────────

/// Parsed CPI data file (bundled or user-provided).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpiData {
    /// Human-readable description of the data source.
    pub source: String,
    /// Date the data was last retrieved.
    #[serde(default)]
    pub retrieved: String,
    /// Methodology note displayed in output footer.
    #[serde(default)]
    pub note: String,
    /// Monthly CPI values keyed by "YYYY-MM" (e.g., "2024-01": 308.417).
    /// Used for precise fiscal-year-weighted averages.
    #[serde(default)]
    pub monthly: HashMap<String, f64>,
    /// Calendar-year annual averages as fallback when monthly data is unavailable.
    /// Keyed by year string (e.g., "2024": 313.685).
    #[serde(default)]
    pub annual_averages: HashMap<String, f64>,
    /// Years with fewer than 12 months of data.
    #[serde(default)]
    pub partial_years: HashMap<String, PartialYearInfo>,
}

/// Metadata about a partial year's data completeness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialYearInfo {
    pub months: u32,
    pub through: String,
}

/// Inflation context for a specific fiscal year comparison.
/// Included in output to document the methodology.
#[derive(Debug, Clone, Serialize)]
pub struct InflationContext {
    /// Data source (e.g., "Bureau of Labor Statistics, CPI-U All Items")
    pub source: String,
    /// Base fiscal year
    pub base_fy: u32,
    /// Current fiscal year
    pub current_fy: u32,
    /// CPI value for base fiscal year
    pub base_cpi: f64,
    /// CPI value for current fiscal year
    pub current_cpi: f64,
    /// Inflation rate as a fraction (e.g., 0.039 for 3.9%)
    pub rate: f64,
    /// Number of months of CPI data available for the current FY
    pub current_fy_months: u32,
    /// Human-readable note about data completeness
    pub note: String,
}

/// Inflation flag for a single comparison row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InflationFlag {
    /// Nominal increase exceeded inflation — real purchasing power grew.
    RealIncrease,
    /// Nominal decrease — real cut regardless of inflation.
    RealCut,
    /// Nominal increase but below inflation — purchasing power decreased.
    InflationErosion,
    /// No change in either nominal or real terms.
    Unchanged,
}

impl std::fmt::Display for InflationFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InflationFlag::RealIncrease => write!(f, "▲"),
            InflationFlag::RealCut => write!(f, "▼"),
            InflationFlag::InflationErosion => write!(f, "▼"),
            InflationFlag::Unchanged => write!(f, "—"),
        }
    }
}

impl InflationFlag {
    /// Slug for CSV/JSON output.
    pub fn slug(&self) -> &'static str {
        match self {
            InflationFlag::RealIncrease => "real_increase",
            InflationFlag::RealCut => "real_cut",
            InflationFlag::InflationErosion => "inflation_erosion",
            InflationFlag::Unchanged => "unchanged",
        }
    }
}

// ─── Loading ─────────────────────────────────────────────────────────────────

/// Load CPI data from the bundled JSON.
pub fn load_bundled() -> Result<CpiData> {
    serde_json::from_str(BUNDLED_CPI_JSON)
        .context("Failed to parse bundled CPI data (data/cpi.json)")
}

/// Load CPI data from a user-provided file.
pub fn load_from_file(path: &Path) -> Result<CpiData> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read CPI file: {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse CPI file: {}", path.display()))
}

/// Load CPI data — from file if specified, otherwise bundled.
pub fn load_cpi(cpi_file: Option<&Path>) -> Result<CpiData> {
    match cpi_file {
        Some(path) => load_from_file(path),
        None => load_bundled(),
    }
}

// ─── Computation ─────────────────────────────────────────────────────────────

/// Compute the average CPI for a federal fiscal year (Oct prior year – Sep).
///
/// FY2024 = Oct 2023 through Sep 2024.
/// Uses monthly data for precise weighting. Falls back to calendar-year
/// annual averages if monthly data is insufficient.
///
/// Returns `(average, months_used)`.
pub fn fiscal_year_cpi(cpi: &CpiData, fy: u32) -> Option<(f64, u32)> {
    let mut values = Vec::new();

    // Oct-Dec of prior year
    for month in 10..=12 {
        let key = format!("{}-{:02}", fy - 1, month);
        if let Some(&v) = cpi.monthly.get(&key) {
            values.push(v);
        }
    }

    // Jan-Sep of FY year
    for month in 1..=9 {
        let key = format!("{}-{:02}", fy, month);
        if let Some(&v) = cpi.monthly.get(&key) {
            values.push(v);
        }
    }

    if values.len() >= 3 {
        // Use fiscal-year-weighted average if we have at least 3 months
        let avg = values.iter().sum::<f64>() / values.len() as f64;
        return Some((avg, values.len() as u32));
    }

    // Fallback: calendar-year annual average
    if let Some(&avg) = cpi.annual_averages.get(&fy.to_string()) {
        let months = cpi
            .partial_years
            .get(&fy.to_string())
            .map(|p| p.months)
            .unwrap_or(12);
        return Some((avg, months));
    }

    None
}

/// Compute inflation context for comparing two fiscal years.
pub fn compute_inflation_context(
    cpi: &CpiData,
    base_fy: u32,
    current_fy: u32,
) -> Option<InflationContext> {
    let (base_cpi, _base_months) = fiscal_year_cpi(cpi, base_fy)?;
    let (current_cpi, current_months) = fiscal_year_cpi(cpi, current_fy)?;

    let rate = current_cpi / base_cpi - 1.0;

    let note = if current_months < 12 {
        let partial = cpi.partial_years.get(&current_fy.to_string());
        let through = partial.map(|p| p.through.as_str()).unwrap_or("partial");
        format!("FY{current_fy} based on {current_months} months of data (through {through})")
    } else {
        format!("FY{current_fy} based on full 12 months of data")
    };

    Some(InflationContext {
        source: cpi.source.clone(),
        base_fy,
        current_fy,
        base_cpi,
        current_cpi,
        rate,
        current_fy_months: current_months,
        note,
    })
}

/// Compute the real (inflation-adjusted) percentage change.
///
/// Given a nominal percentage change and an inflation rate,
/// returns the real percentage change.
///
/// Formula: real_pct = ((1 + nominal_pct/100) / (1 + inflation_rate) - 1) * 100
pub fn real_delta_pct(nominal_delta_pct: f64, inflation_rate: f64) -> f64 {
    ((1.0 + nominal_delta_pct / 100.0) / (1.0 + inflation_rate) - 1.0) * 100.0
}

/// Determine the inflation flag for a comparison row.
pub fn compute_flag(nominal_delta_pct: Option<f64>, inflation_rate: f64) -> InflationFlag {
    let Some(nominal) = nominal_delta_pct else {
        return InflationFlag::Unchanged;
    };

    if nominal.abs() < 0.001 && inflation_rate.abs() < 0.001 {
        return InflationFlag::Unchanged;
    }

    let real = real_delta_pct(nominal, inflation_rate);

    if real > 0.05 {
        InflationFlag::RealIncrease
    } else if real < -0.05 {
        if nominal < -0.05 {
            InflationFlag::RealCut
        } else {
            // Nominal was zero or positive, but real is negative
            InflationFlag::InflationErosion
        }
    } else {
        // Real change is essentially zero (within ±0.05%)
        InflationFlag::Unchanged
    }
}

/// Check if the bundled CPI data is stale (retrieved more than 60 days ago).
pub fn check_staleness(cpi: &CpiData) -> Option<String> {
    if cpi.retrieved.is_empty() {
        return None;
    }
    if let Ok(retrieved) = chrono::NaiveDate::parse_from_str(&cpi.retrieved, "%Y-%m-%d") {
        let today = chrono::Utc::now().date_naive();
        let age = today.signed_duration_since(retrieved).num_days();
        if age > 60 {
            return Some(format!(
                "Bundled CPI data last updated {} ({} days ago). Use --cpi-file for more recent data.",
                cpi.retrieved, age
            ));
        }
    }
    None
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_cpi() -> CpiData {
        let mut monthly = HashMap::new();
        // FY2024 = Oct 2023 - Sep 2024
        // Simulated values around 310
        for month in 10..=12 {
            monthly.insert(format!("2023-{:02}", month), 305.0 + month as f64);
        }
        for month in 1..=9 {
            monthly.insert(format!("2024-{:02}", month), 310.0 + month as f64);
        }
        // FY2026 = Oct 2025 - Sep 2026
        // Only 4 months available
        for month in 10..=12 {
            monthly.insert(format!("2025-{:02}", month), 320.0 + month as f64);
        }
        monthly.insert("2026-01".to_string(), 333.0);

        let mut partial_years = HashMap::new();
        partial_years.insert(
            "2026".to_string(),
            PartialYearInfo {
                months: 1,
                through: "2026-01".to_string(),
            },
        );

        CpiData {
            source: "Test data".to_string(),
            retrieved: "2026-01-01".to_string(),
            note: "Test".to_string(),
            monthly,
            annual_averages: HashMap::new(),
            partial_years,
        }
    }

    #[test]
    fn test_fiscal_year_cpi_full_year() {
        let cpi = make_test_cpi();
        let (avg, months) = fiscal_year_cpi(&cpi, 2024).unwrap();
        assert_eq!(months, 12);
        // Oct=315, Nov=316, Dec=317, Jan=311, ..., Sep=319
        // Sum = (315+316+317) + (311+312+313+314+315+316+317+318+319) = 948 + 2835 = 3783
        // Avg = 3783 / 12 = 315.25
        assert!((avg - 315.25).abs() < 0.01);
    }

    #[test]
    fn test_fiscal_year_cpi_partial() {
        let cpi = make_test_cpi();
        let (avg, months) = fiscal_year_cpi(&cpi, 2026).unwrap();
        // FY2026: Oct 2025 (330), Nov 2025 (331), Dec 2025 (332), Jan 2026 (333) = 4 months
        assert_eq!(months, 4);
        assert!((avg - 331.5).abs() < 0.01);
    }

    #[test]
    fn test_fiscal_year_cpi_missing() {
        let cpi = make_test_cpi();
        assert!(fiscal_year_cpi(&cpi, 2030).is_none());
    }

    #[test]
    fn test_inflation_context() {
        let cpi = make_test_cpi();
        let ctx = compute_inflation_context(&cpi, 2024, 2026).unwrap();
        assert_eq!(ctx.base_fy, 2024);
        assert_eq!(ctx.current_fy, 2026);
        assert!(ctx.rate > 0.0); // Inflation should be positive
        assert_eq!(ctx.current_fy_months, 4);
    }

    #[test]
    fn test_real_delta_pct() {
        // 10% nominal increase with 3% inflation
        let real = real_delta_pct(10.0, 0.03);
        // (1.10 / 1.03 - 1) * 100 ≈ 6.796%
        assert!((real - 6.796).abs() < 0.01);
    }

    #[test]
    fn test_real_delta_pct_zero_inflation() {
        let real = real_delta_pct(5.0, 0.0);
        assert!((real - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_real_delta_pct_erosion() {
        // 2% nominal increase with 4% inflation = real decrease
        let real = real_delta_pct(2.0, 0.04);
        assert!(real < 0.0);
    }

    #[test]
    fn test_flag_real_increase() {
        let flag = compute_flag(Some(10.0), 0.03);
        assert_eq!(flag, InflationFlag::RealIncrease);
    }

    #[test]
    fn test_flag_real_cut() {
        let flag = compute_flag(Some(-5.0), 0.03);
        assert_eq!(flag, InflationFlag::RealCut);
    }

    #[test]
    fn test_flag_inflation_erosion() {
        let flag = compute_flag(Some(2.0), 0.04);
        assert_eq!(flag, InflationFlag::InflationErosion);
    }

    #[test]
    fn test_flag_unchanged() {
        let flag = compute_flag(Some(0.0), 0.0);
        assert_eq!(flag, InflationFlag::Unchanged);
    }

    #[test]
    fn test_flag_none_nominal() {
        let flag = compute_flag(None, 0.03);
        assert_eq!(flag, InflationFlag::Unchanged);
    }

    #[test]
    fn test_load_bundled() {
        let cpi = load_bundled().unwrap();
        assert!(!cpi.monthly.is_empty());
        assert!(cpi.source.contains("Bureau of Labor Statistics"));
        // Should have data for 2024
        assert!(fiscal_year_cpi(&cpi, 2024).is_some());
    }

    #[test]
    fn test_inflation_flag_display() {
        assert_eq!(format!("{}", InflationFlag::RealIncrease), "▲");
        assert_eq!(format!("{}", InflationFlag::RealCut), "▼");
        assert_eq!(format!("{}", InflationFlag::InflationErosion), "▼");
        assert_eq!(format!("{}", InflationFlag::Unchanged), "—");
    }

    #[test]
    fn test_inflation_flag_slug() {
        assert_eq!(InflationFlag::RealIncrease.slug(), "real_increase");
        assert_eq!(InflationFlag::RealCut.slug(), "real_cut");
        assert_eq!(InflationFlag::InflationErosion.slug(), "inflation_erosion");
        assert_eq!(InflationFlag::Unchanged.slug(), "unchanged");
    }

    #[test]
    fn test_save_load_roundtrip() {
        let cpi = make_test_cpi();
        let json = serde_json::to_string(&cpi).unwrap();
        let loaded: CpiData = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.source, "Test data");
        assert_eq!(loaded.monthly.len(), cpi.monthly.len());
    }
}
