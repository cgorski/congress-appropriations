use crate::approp::ontology::*;
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;
use tracing::debug;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ConversionReport {
    pub provisions_parsed: usize,
    pub provisions_failed: usize,
    pub null_to_default: usize,
    pub type_coercions: usize,
    pub unknown_provision_types: usize,
    pub warnings: Vec<String>,
}

impl ConversionReport {
    /// Merge another report into this one (for multi-pass extraction).
    pub fn merge(&mut self, other: &ConversionReport) {
        self.provisions_parsed += other.provisions_parsed;
        self.provisions_failed += other.provisions_failed;
        self.null_to_default += other.null_to_default;
        self.type_coercions += other.type_coercions;
        self.unknown_provision_types += other.unknown_provision_types;
        self.warnings.extend(other.warnings.iter().cloned());
    }
}

pub fn parse_bill_extraction(value: &Value) -> Result<(BillExtraction, ConversionReport)> {
    let obj = value.as_object().context("Expected root object")?;
    let mut report = ConversionReport::default();

    let bill = parse_bill_info(obj.get("bill").context("Missing 'bill'")?)?;
    let summary = parse_summary(obj.get("summary").context("Missing 'summary'")?)?;

    let provisions_arr = obj
        .get("provisions")
        .and_then(|v| v.as_array())
        .context("Missing 'provisions' array")?;

    let mut provisions = Vec::with_capacity(provisions_arr.len());
    for (i, item) in provisions_arr.iter().enumerate() {
        match parse_provision(item, &mut report) {
            Ok(p) => {
                provisions.push(p);
                report.provisions_parsed += 1;
            }
            Err(e) => {
                report.provisions_failed += 1;
                report.warnings.push(format!("Provision {i}: {e}"));
                debug!("Failed to parse provision {i}: {e}");
            }
        }
    }

    Ok((
        BillExtraction {
            schema_version: None,
            bill,
            provisions,
            summary,
            chunk_map: vec![],
        },
        report,
    ))
}

fn parse_bill_info(value: &Value) -> Result<BillInfo> {
    let obj = value.as_object().context("bill not an object")?;
    Ok(BillInfo {
        identifier: get_str(obj, "identifier"),
        classification: match get_str(obj, "classification").as_str() {
            "regular" => BillClassification::Regular,
            "continuing_resolution" => BillClassification::ContinuingResolution,
            "omnibus" => BillClassification::Omnibus,
            "supplemental" => BillClassification::Supplemental,
            "rescissions" => BillClassification::Rescissions,
            "minibus" => BillClassification::Minibus,
            other => BillClassification::Other(other.to_string()),
        },
        short_title: get_opt_str(obj, "short_title"),
        fiscal_years: obj
            .get("fiscal_years")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_i64().map(|n| n as u32))
                    .collect()
            })
            .unwrap_or_default(),
        divisions: get_string_array(obj, "divisions"),
        public_law: get_opt_str(obj, "public_law"),
    })
}

fn parse_summary(value: &Value) -> Result<ExtractionSummary> {
    let obj = value.as_object().context("summary not an object")?;
    Ok(ExtractionSummary {
        total_provisions: obj
            .get("total_provisions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        by_division: parse_string_usize_map(obj.get("by_division")),
        by_type: parse_string_usize_map(obj.get("by_type")),
        total_budget_authority: obj
            .get("total_budget_authority")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        total_rescissions: obj
            .get("total_rescissions")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        sections_with_no_provisions: get_string_array(obj, "sections_with_no_provisions"),
        flagged_issues: get_string_array(obj, "flagged_issues"),
    })
}

fn parse_string_usize_map(value: Option<&Value>) -> HashMap<String, usize> {
    value
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0) as usize))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_provision(value: &Value, report: &mut ConversionReport) -> Result<Provision> {
    let obj = value.as_object().context("provision not an object")?;

    // These are guaranteed by the output_config schema
    let provision_type = get_str(obj, "provision_type");
    let section = get_str(obj, "section");
    let raw_text = get_str(obj, "raw_text");
    let confidence = get_f32(obj, "confidence", report);

    // Common optional fields
    let division = get_opt_str(obj, "division");
    let title = get_opt_str(obj, "title");
    let notes = get_string_array(obj, "notes");
    let cross_references = parse_cross_references(obj.get("cross_references"));

    match provision_type.as_str() {
        "appropriation" => Ok(Provision::Appropriation {
            account_name: get_str_or_warn(obj, "account_name", report),
            agency: get_opt_str(obj, "agency"),
            program: get_opt_str(obj, "program"),
            amount: parse_dollar_amount(obj.get("amount"), report).unwrap_or_else(|| {
                report
                    .warnings
                    .push(format!("SEC {section}: missing amount on appropriation"));
                DollarAmount::zero(AmountSemantics::Other("indefinite".into()))
            }),
            fiscal_year: get_opt_u32(obj, "fiscal_year"),
            availability: parse_availability(obj.get("availability")),
            provisos: parse_provisos(obj.get("provisos")),
            earmarks: parse_earmarks(obj.get("earmarks")),
            detail_level: get_str_or_warn(obj, "detail_level", report),
            parent_account: get_opt_str(obj, "parent_account"),
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        "rescission" => Ok(Provision::Rescission {
            account_name: get_str_or_warn(obj, "account_name", report),
            agency: get_opt_str(obj, "agency"),
            amount: parse_dollar_amount(obj.get("amount"), report).unwrap_or_else(|| {
                report
                    .warnings
                    .push(format!("SEC {section}: missing amount on rescission"));
                DollarAmount::zero(AmountSemantics::Rescission)
            }),
            reference_law: get_opt_str(obj, "reference_law"),
            fiscal_years: get_opt_str(obj, "fiscal_years"),
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        "transfer_authority" => Ok(Provision::TransferAuthority {
            from_scope: get_str_or_warn(obj, "from_scope", report),
            to_scope: get_str_or_warn(obj, "to_scope", report),
            limit: parse_transfer_limit(obj.get("limit")),
            conditions: get_string_array(obj, "conditions"),
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        "limitation" => Ok(Provision::Limitation {
            description: get_str_or_warn(obj, "description", report),
            amount: obj
                .get("amount")
                .and_then(|v| parse_dollar_amount(Some(v), report)),
            account_name: get_opt_str(obj, "account_name"),
            parent_account: get_opt_str(obj, "parent_account"),
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        "directed_spending" => Ok(Provision::DirectedSpending {
            account_name: get_opt_str(obj, "account_name"),
            amount: parse_dollar_amount(obj.get("amount"), report)
                .unwrap_or_else(|| DollarAmount::zero(AmountSemantics::Other("indefinite".into()))),
            earmark: parse_single_earmark(obj).unwrap_or(Earmark {
                recipient: String::new(),
                location: String::new(),
                requesting_member: None,
            }),
            detail_level: get_str_or_warn(obj, "detail_level", report),
            parent_account: get_opt_str(obj, "parent_account"),
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        "cr_substitution" => Ok(Provision::CrSubstitution {
            reference_act: get_str_or_warn(obj, "reference_act", report),
            reference_section: get_str_or_warn(obj, "reference_section", report),
            new_amount: parse_dollar_amount(obj.get("new_amount"), report)
                .unwrap_or_else(|| DollarAmount::zero(AmountSemantics::Other("indefinite".into()))),
            old_amount: parse_dollar_amount(obj.get("old_amount"), report)
                .unwrap_or_else(|| DollarAmount::zero(AmountSemantics::ReferenceAmount)),
            account_name: get_opt_str(obj, "account_name"),
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        "mandatory_spending_extension" => Ok(Provision::MandatorySpendingExtension {
            program_name: get_str_or_warn(obj, "program_name", report),
            statutory_reference: get_str_or_warn(obj, "statutory_reference", report),
            amount: obj
                .get("amount")
                .and_then(|v| parse_dollar_amount(Some(v), report)),
            period: get_opt_str(obj, "period"),
            extends_through: get_opt_str(obj, "extends_through"),
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        "directive" => Ok(Provision::Directive {
            description: get_str_or_warn(obj, "description", report),
            deadlines: get_string_array(obj, "deadlines"),
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        "rider" => Ok(Provision::Rider {
            description: get_str_or_warn(obj, "description", report),
            policy_area: get_opt_str(obj, "policy_area"),
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        "continuing_resolution_baseline" => Ok(Provision::ContinuingResolutionBaseline {
            reference_year: get_opt_u32(obj, "reference_year").unwrap_or(0),
            reference_laws: get_string_array(obj, "reference_laws"),
            rate: get_str_or_warn(obj, "rate", report),
            duration: get_opt_str(obj, "duration"),
            anomalies: Vec::new(), // Complex nested type — skip for now
            section,
            division,
            title,
            confidence,
            raw_text,
            notes,
            cross_references,
        }),
        unknown => {
            report.unknown_provision_types += 1;
            Ok(Provision::Other {
                llm_classification: unknown.to_string(),
                description: get_str_or_warn(obj, "description", report),
                amounts: obj
                    .get("amount")
                    .and_then(|v| parse_dollar_amount(Some(v), report))
                    .into_iter()
                    .collect(),
                references: get_string_array(obj, "references"),
                metadata: obj
                    .iter()
                    .filter(|(k, _)| {
                        ![
                            "provision_type",
                            "section",
                            "division",
                            "title",
                            "confidence",
                            "raw_text",
                            "notes",
                            "cross_references",
                            "description",
                            "amount",
                            "references",
                        ]
                        .contains(&k.as_str())
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                section,
                division,
                title,
                confidence,
                raw_text,
                notes,
                cross_references,
            })
        }
    }
}

// ─── Dollar Amount Parser ───────────────────────────────────────────

fn parse_dollar_amount(
    value: Option<&Value>,
    report: &mut ConversionReport,
) -> Option<DollarAmount> {
    let obj = value?.as_object()?;

    // Check for AmountValue kind — handle "such_sums" and "none" before reading dollars
    let kind = obj
        .get("kind")
        .or_else(|| obj.get("value").and_then(|v| v.get("kind")))
        .and_then(|k| k.as_str());

    if kind == Some("such_sums") {
        let semantics = match obj
            .get("semantics")
            .and_then(|v| v.as_str())
            .unwrap_or("other")
        {
            "new_budget_authority" => AmountSemantics::NewBudgetAuthority,
            "transfer_ceiling" => AmountSemantics::TransferCeiling,
            "rescission" => AmountSemantics::Rescission,
            "limitation" => AmountSemantics::Limitation,
            "reference_amount" => AmountSemantics::ReferenceAmount,
            "mandatory_spending" => AmountSemantics::MandatorySpending,
            other => AmountSemantics::Other(other.to_string()),
        };
        let text = obj
            .get("text_as_written")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        return Some(DollarAmount::such_sums(semantics, text));
    }
    if kind == Some("none") {
        return None;
    }

    let dollars = match obj.get("dollars") {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => {
            report.type_coercions += 1;
            s.replace([',', '$'], "").parse().unwrap_or(0)
        }
        _ => return None,
    };
    let semantics = match obj
        .get("semantics")
        .and_then(|v| v.as_str())
        .unwrap_or("other")
    {
        "new_budget_authority" => AmountSemantics::NewBudgetAuthority,
        "transfer_ceiling" => AmountSemantics::TransferCeiling,
        "rescission" => AmountSemantics::Rescission,
        "limitation" => AmountSemantics::Limitation,
        "reference_amount" => AmountSemantics::ReferenceAmount,
        "mandatory_spending" => AmountSemantics::MandatorySpending,
        other => AmountSemantics::Other(other.to_string()),
    };
    let text_as_written = obj
        .get("text_as_written")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some(DollarAmount::from_dollars(
        dollars,
        semantics,
        text_as_written,
    ))
}

// ─── Helper functions ───────────────────────────────────────────────

fn get_str(obj: &Map<String, Value>, key: &str) -> String {
    obj.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn get_opt_str(obj: &Map<String, Value>, key: &str) -> Option<String> {
    obj.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn get_str_or_warn(obj: &Map<String, Value>, key: &str, report: &mut ConversionReport) -> String {
    match obj.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => {
            report.null_to_default += 1;
            String::new()
        }
        Some(other) => {
            report.type_coercions += 1;
            other.to_string()
        }
    }
}

fn get_opt_u32(obj: &Map<String, Value>, key: &str) -> Option<u32> {
    match obj.get(key) {
        Some(Value::Number(n)) => n.as_u64().map(|n| n as u32),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

fn get_f32(obj: &Map<String, Value>, key: &str, report: &mut ConversionReport) -> f32 {
    match obj.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) as f32,
        Some(Value::String(s)) => {
            report.type_coercions += 1;
            s.parse().unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

fn get_string_array(obj: &Map<String, Value>, key: &str) -> Vec<String> {
    obj.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_cross_references(value: Option<&Value>) -> Vec<CrossReference> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let obj = v.as_object()?;
                    Some(CrossReference {
                        ref_type: get_str(obj, "ref_type"),
                        target: get_str(obj, "target"),
                        description: get_opt_str(obj, "description"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_availability(value: Option<&Value>) -> Option<FundAvailability> {
    let v = value?;
    if v.is_null() {
        return None;
    }
    if let Some(s) = v.as_str() {
        return match s {
            "no_year" => Some(FundAvailability::NoYear),
            other => Some(FundAvailability::Other(other.to_string())),
        };
    }
    if let Some(obj) = v.as_object() {
        if obj.contains_key("no_year") {
            return Some(FundAvailability::NoYear);
        }
        if let Some(fy) = obj.get("fiscal_year").and_then(|v| v.as_u64()) {
            return Some(FundAvailability::OneYear {
                fiscal_year: fy as u32,
            });
        }
        if let Some(through) = obj.get("through").and_then(|v| v.as_u64()) {
            return Some(FundAvailability::MultiYear {
                through: through as u32,
            });
        }
    }
    Some(FundAvailability::Other(v.to_string()))
}

fn parse_transfer_limit(value: Option<&Value>) -> TransferLimit {
    match value {
        Some(Value::Object(obj)) => {
            if let Some(pct) = obj.get("percentage").and_then(|v| v.as_f64()) {
                TransferLimit::Percentage(pct)
            } else if let Some(amt_val) = obj.get("fixed_amount") {
                let mut tmp_report = ConversionReport::default();
                if let Some(amt) = parse_dollar_amount(Some(amt_val), &mut tmp_report) {
                    TransferLimit::FixedAmount(amt)
                } else {
                    TransferLimit::Other(value.unwrap().to_string())
                }
            } else {
                // Maybe it IS a dollar amount directly
                let mut tmp_report = ConversionReport::default();
                if let Some(amt) = parse_dollar_amount(value, &mut tmp_report) {
                    TransferLimit::FixedAmount(amt)
                } else {
                    TransferLimit::Other(value.unwrap().to_string())
                }
            }
        }
        Some(Value::String(s)) => TransferLimit::Other(s.clone()),
        _ => TransferLimit::Other("unknown".to_string()),
    }
}

fn parse_provisos(value: Option<&Value>) -> Vec<Proviso> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let obj = v.as_object()?;
                    Some(Proviso {
                        proviso_type: match get_str(obj, "proviso_type").as_str() {
                            "limitation" => ProvisoType::Limitation,
                            "transfer" => ProvisoType::Transfer,
                            "reporting" => ProvisoType::Reporting,
                            "condition" => ProvisoType::Condition,
                            "prohibition" => ProvisoType::Prohibition,
                            other => ProvisoType::Other(other.to_string()),
                        },
                        description: get_str(obj, "description"),
                        amount: parse_dollar_amount(
                            obj.get("amount"),
                            &mut ConversionReport::default(),
                        ),
                        references: get_string_array(obj, "references"),
                        raw_text: get_str(obj, "raw_text"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_earmarks(value: Option<&Value>) -> Vec<Earmark> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| parse_single_earmark(v.as_object()?).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_single_earmark(obj: &Map<String, Value>) -> Result<Earmark> {
    Ok(Earmark {
        recipient: get_opt_str(obj, "recipient").unwrap_or_default(),
        location: get_opt_str(obj, "location").unwrap_or_default(),
        requesting_member: get_opt_str(obj, "requesting_member"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_extraction() {
        let json = serde_json::json!({
            "bill": {"identifier": "H.R. 1", "classification": "regular", "fiscal_years": [2024], "divisions": ["A"]},
            "provisions": [],
            "summary": {"total_provisions": 0, "total_budget_authority": 0, "total_rescissions": 0}
        });
        let (extraction, report) = parse_bill_extraction(&json).unwrap();
        assert_eq!(extraction.bill.identifier, "H.R. 1");
        assert_eq!(extraction.provisions.len(), 0);
        assert_eq!(report.provisions_parsed, 0);
    }

    #[test]
    fn parse_unknown_provision_type() {
        let json = serde_json::json!({
            "bill": {"identifier": "H.R. 1", "classification": "regular", "fiscal_years": [2024], "divisions": []},
            "provisions": [
                {"provision_type": "weird_new_thing", "section": "SEC. 1", "raw_text": "test", "confidence": 0.5, "description": "something"}
            ],
            "summary": {"total_provisions": 1, "total_budget_authority": 0, "total_rescissions": 0}
        });
        let (extraction, report) = parse_bill_extraction(&json).unwrap();
        assert_eq!(extraction.provisions.len(), 1);
        assert_eq!(report.unknown_provision_types, 1);
    }

    #[test]
    fn parse_appropriation_provision() {
        let json = serde_json::json!({
            "bill": {"identifier": "H.R. 2", "classification": "omnibus", "fiscal_years": [2025], "divisions": ["A"]},
            "provisions": [
                {
                    "provision_type": "appropriation",
                    "section": "SEC. 101",
                    "raw_text": "For salaries and expenses, $1,000,000.",
                    "confidence": 0.95,
                    "account_name": "Salaries and Expenses",
                    "agency": "Department of Testing",
                    "amount": {
                        "dollars": 1000000,
                        "semantics": "new_budget_authority",
                        "text_as_written": "$1,000,000"
                    }
                }
            ],
            "summary": {"total_provisions": 1, "total_budget_authority": 1000000, "total_rescissions": 0}
        });
        let (extraction, report) = parse_bill_extraction(&json).unwrap();
        assert_eq!(extraction.provisions.len(), 1);
        assert_eq!(report.provisions_parsed, 1);
        assert_eq!(report.provisions_failed, 0);
        match &extraction.provisions[0] {
            Provision::Appropriation {
                account_name,
                amount,
                ..
            } => {
                assert_eq!(account_name, "Salaries and Expenses");
                assert_eq!(amount.dollars(), Some(1000000));
            }
            other => panic!("Expected Appropriation, got {:?}", other),
        }
    }

    #[test]
    fn parse_dollar_amount_with_string_coercion() {
        let mut report = ConversionReport::default();
        let val = serde_json::json!({
            "dollars": "$1,234,567",
            "semantics": "new_budget_authority",
            "text_as_written": "$1,234,567"
        });
        let amt = parse_dollar_amount(Some(&val), &mut report).unwrap();
        assert_eq!(amt.dollars(), Some(1234567));
        assert_eq!(report.type_coercions, 1);
    }

    #[test]
    fn parse_missing_root_field_returns_error() {
        let json = serde_json::json!({
            "bill": {"identifier": "H.R. 1", "classification": "regular", "fiscal_years": [], "divisions": []},
            "summary": {"total_provisions": 0, "total_budget_authority": 0, "total_rescissions": 0}
        });
        assert!(parse_bill_extraction(&json).is_err());
    }
}
