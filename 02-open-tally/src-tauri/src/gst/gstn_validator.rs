use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GstnValidationIssue {
    pub section: String,
    pub field: String,
    pub message: String,
    pub severity: String, // "Error" | "Warning"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GstnValidationReport {
    pub is_valid: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub errors: Vec<GstnValidationIssue>,
    pub warnings: Vec<GstnValidationIssue>,
}

/// Converts ISO `YYYY-MM-DD` to GSTN portal `DD-MM-YYYY` format.
pub fn iso_to_gstn_date(iso: &str) -> String {
    let trimmed = iso.trim();
    if trimmed.len() >= 10 && trimmed.as_bytes()[4] == b'-' && trimmed.as_bytes()[7] == b'-' {
        format!("{}-{}-{}", &trimmed[8..10], &trimmed[5..7], &trimmed[0..4])
    } else {
        trimmed.to_string()
    }
}

fn is_valid_gstn_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[2] != b'-' || b[5] != b'-' {
        return false;
    }
    let dd: u32 = match s[0..2].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mm: u32 = match s[3..5].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let yyyy: u32 = match s[6..10].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    (1..=31).contains(&dd) && (1..=12).contains(&mm) && (2000..=2100).contains(&yyyy)
}

fn is_valid_gstn_doc_num(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 16
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-')
}

fn parse_nonneg_decimal(val: Option<&Value>) -> Option<Decimal> {
    let v = val?;
    let d = if let Some(s) = v.as_str() {
        Decimal::from_str(s).ok()?
    } else {
        let n = v.as_f64()?;
        Decimal::from_str(&format!("{:.2}", n)).ok()?
    };
    if d < Decimal::ZERO {
        None
    } else {
        Some(d)
    }
}

/// Validates a GSTR-1 JSON payload against GSTN Offline Tool v3.x schema rules.
pub fn validate_gstr1_gstn_schema(payload: &Value) -> GstnValidationReport {
    let mut errors: Vec<GstnValidationIssue> = Vec::new();
    let mut warnings: Vec<GstnValidationIssue> = Vec::new();

    // 1. Validate GSTIN
    match payload.get("gstin").and_then(|v| v.as_str()) {
        Some(g) => {
            if let Err(e) = crate::gst::validate_gstin(g, None) {
                errors.push(GstnValidationIssue {
                    section: "Header".to_string(),
                    field: "gstin".to_string(),
                    message: format!("Invalid GSTIN '{}': {}", g, e),
                    severity: "Error".to_string(),
                });
            }
        }
        None => errors.push(GstnValidationIssue {
            section: "Header".to_string(),
            field: "gstin".to_string(),
            message: "Missing supplier GSTIN".to_string(),
            severity: "Error".to_string(),
        }),
    }

    // 2. Validate Filing Period (fp: MMYYYY)
    match payload.get("fp").and_then(|v| v.as_str()) {
        Some(fp) if fp.len() == 6 && fp.chars().all(|c| c.is_ascii_digit()) => {
            let mm: u32 = fp[0..2].parse().unwrap_or(0);
            if !(1..=12).contains(&mm) {
                errors.push(GstnValidationIssue {
                    section: "Header".to_string(),
                    field: "fp".to_string(),
                    message: format!("Invalid filing period month in '{}'", fp),
                    severity: "Error".to_string(),
                });
            }
        }
        other => errors.push(GstnValidationIssue {
            section: "Header".to_string(),
            field: "fp".to_string(),
            message: format!("Invalid filing period {:?}: expected MMYYYY", other),
            severity: "Error".to_string(),
        }),
    }

    // 3. Validate B2B Invoices
    if let Some(b2b) = payload.get("b2b").and_then(|v| v.as_array()) {
        for party in b2b {
            if let Some(ctin) = party.get("ctin").and_then(|v| v.as_str()) {
                if let Err(e) = crate::gst::validate_gstin(ctin, None) {
                    errors.push(GstnValidationIssue {
                        section: "B2B".to_string(),
                        field: "ctin".to_string(),
                        message: format!("Invalid counterparty GSTIN '{}': {}", ctin, e),
                        severity: "Error".to_string(),
                    });
                }
            }
            if let Some(invs) = party.get("inv").and_then(|v| v.as_array()) {
                for inv in invs {
                    let inum = inv.get("inum").and_then(|v| v.as_str()).unwrap_or("");
                    if !is_valid_gstn_doc_num(inum) {
                        errors.push(GstnValidationIssue {
                            section: "B2B".to_string(),
                            field: "inum".to_string(),
                            message: format!("Invoice '{}' must be <=16 chars [A-Za-z0-9/-]", inum),
                            severity: "Error".to_string(),
                        });
                    }
                    let idt = inv.get("idt").and_then(|v| v.as_str()).unwrap_or("");
                    if !is_valid_gstn_date(idt) {
                        errors.push(GstnValidationIssue {
                            section: "B2B".to_string(),
                            field: "idt".to_string(),
                            message: format!(
                                "Invoice '{}' date '{}' must be DD-MM-YYYY",
                                inum, idt
                            ),
                            severity: "Error".to_string(),
                        });
                    }
                    let pos = inv.get("pos").and_then(|v| v.as_str()).unwrap_or("");
                    if !crate::gst::is_valid_state_code(pos) && pos != "96" {
                        errors.push(GstnValidationIssue {
                            section: "B2B".to_string(),
                            field: "pos".to_string(),
                            message: format!("Invalid POS state code '{}' on '{}'", pos, inum),
                            severity: "Error".to_string(),
                        });
                    }
                    let val = parse_nonneg_decimal(inv.get("val"));
                    if val.is_none() {
                        errors.push(GstnValidationIssue {
                            section: "B2B".to_string(),
                            field: "val".to_string(),
                            message: format!("Invalid or negative invoice value on '{}'", inum),
                            severity: "Error".to_string(),
                        });
                    }
                    if let (Some(inv_val), Some(itms)) =
                        (val, inv.get("itms").and_then(|v| v.as_array()))
                    {
                        let mut sum_items = Decimal::ZERO;
                        for itm in itms {
                            if let Some(det) = itm.get("itm_det") {
                                let tx =
                                    parse_nonneg_decimal(det.get("txval")).unwrap_or(Decimal::ZERO);
                                let c =
                                    parse_nonneg_decimal(det.get("camt")).unwrap_or(Decimal::ZERO);
                                let s =
                                    parse_nonneg_decimal(det.get("samt")).unwrap_or(Decimal::ZERO);
                                let i =
                                    parse_nonneg_decimal(det.get("iamt")).unwrap_or(Decimal::ZERO);
                                let cs =
                                    parse_nonneg_decimal(det.get("csamt")).unwrap_or(Decimal::ZERO);
                                sum_items += tx + c + s + i + cs;
                            }
                        }
                        if (inv_val - sum_items).abs() > Decimal::new(1, 0) {
                            errors.push(GstnValidationIssue {
                                section: "B2B".to_string(),
                                field: "val".to_string(),
                                message: format!(
                                    "Invoice '{}' val ({}) != txval + tax ({})",
                                    inum, inv_val, sum_items
                                ),
                                severity: "Error".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // 4. Validate Table 6A Exports (exp)
    if let Some(exp_groups) = payload.get("exp").and_then(|v| v.as_array()) {
        for grp in exp_groups {
            if let Some(invs) = grp.get("inv").and_then(|v| v.as_array()) {
                for inv in invs {
                    let inum = inv.get("inum").and_then(|v| v.as_str()).unwrap_or("");
                    if !is_valid_gstn_doc_num(inum) {
                        errors.push(GstnValidationIssue {
                            section: "EXP (Table 6A)".to_string(),
                            field: "inum".to_string(),
                            message: format!("Export invoice '{}' exceeds 16 chars", inum),
                            severity: "Error".to_string(),
                        });
                    }
                    let idt = inv.get("idt").and_then(|v| v.as_str()).unwrap_or("");
                    if !is_valid_gstn_date(idt) {
                        errors.push(GstnValidationIssue {
                            section: "EXP (Table 6A)".to_string(),
                            field: "idt".to_string(),
                            message: format!(
                                "Export invoice '{}' date '{}' must be DD-MM-YYYY",
                                inum, idt
                            ),
                            severity: "Error".to_string(),
                        });
                    }
                    if let Some(sbdt) = inv.get("sbdt").and_then(|v| v.as_str()) {
                        if !sbdt.is_empty() && !is_valid_gstn_date(sbdt) {
                            errors.push(GstnValidationIssue {
                                section: "EXP (Table 6A)".to_string(),
                                field: "sbdt".to_string(),
                                message: format!(
                                    "Shipping bill date '{}' must be DD-MM-YYYY",
                                    sbdt
                                ),
                                severity: "Error".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // 5. Validate Table 13 Document Issue presence
    if payload.get("doc_issue").is_none() {
        warnings.push(GstnValidationIssue {
            section: "Table 13".to_string(),
            field: "doc_issue".to_string(),
            message: "Missing Table 13 (doc_issue) summary".to_string(),
            severity: "Warning".to_string(),
        });
    }

    let error_count = errors.len();
    let warning_count = warnings.len();
    GstnValidationReport {
        is_valid: error_count == 0,
        error_count,
        warning_count,
        errors,
        warnings,
    }
}
